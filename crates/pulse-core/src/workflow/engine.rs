//! Sequential workflow executor with variable substitution, conditions, error policies,
//! retries (retry-safe actions only), timeouts, cancellation and redacted logs.
//!
//! Execution state machine (per run):
//! `Running -> Succeeded | Failed | Cancelled`; per step:
//! `Pending -> Running -> Succeeded | Failed | Skipped | Cancelled`.
//! Cancellation is cooperative: it is observed between steps, during `Wait`, while waiting for a
//! `Confirm` answer (host-dependent) and while a `RunCommand` child is running (the child is killed).

use super::model::*;
use crate::error::{CoreError, Result};
use crate::models::LaunchResult;
use crate::models::LaunchOutcomeKind;
use crate::security::{self, redact};
use crate::storage::{new_id, now_iso};
use chrono::{DateTime, Local};
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const MAX_EXECUTED_STEPS: usize = 200;
const MAX_OUTPUT_BYTES: usize = 16 * 1024;
const MAX_COPY_BYTES: usize = 1024 * 1024;
const MAX_FIELD_LEN: usize = 4096;
const DEFAULT_COMMAND_TIMEOUT_MS: u64 = 60_000;

#[derive(Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum WorkflowEvent {
    RunStarted { run_id: String, workflow_id: String },
    StepStarted { run_id: String, step_id: String, label: String, preview: String },
    StepFinished { run_id: String, result: WorkflowStepResult },
    RunFinished { run: WorkflowRun },
}

/// Everything the engine needs from the outside world. Real implementation lives in the app;
/// tests provide controlled fakes so no personal folders or apps are touched.
pub trait WorkflowHost: Send + Sync {
    fn launch_app(&self, app_id: &str, extra_args: &[String]) -> Result<LaunchResult>;
    fn launch_path(&self, path: &str, args: &[String], working_dir: Option<&str>) -> Result<LaunchResult>;
    fn open_path(&self, path: &str) -> Result<()>;
    fn open_url(&self, url: &str) -> Result<()>;
    fn open_settings_uri(&self, uri: &str) -> Result<()>;
    fn copy_text(&self, text: &str) -> Result<()>;
    fn notify(&self, title: &str, body: &str) -> Result<()>;
    /// Ask the user. Must return `Ok(false)` on decline/timeout.
    fn confirm(&self, message: &str, timeout: Duration, cancel: &CancelToken) -> Result<bool>;
    fn app_exists(&self, app_id: &str) -> bool;
    fn collection_apps(&self, system_key: &str) -> Result<Vec<(String, String)>>;
    fn developer_commands_enabled(&self) -> bool;
    fn is_windows(&self) -> bool {
        cfg!(windows)
    }
    fn now(&self) -> DateTime<Local> {
        Local::now()
    }
    fn known_folder(&self, name: &str) -> Option<String> {
        let p = match name.to_ascii_lowercase().as_str() {
            "downloads" => dirs::download_dir(),
            "documents" => dirs::document_dir(),
            "desktop" => dirs::desktop_dir(),
            "home" => dirs::home_dir(),
            _ => None,
        };
        p.map(|p| p.to_string_lossy().into_owned())
    }
    fn path_exists(&self, path: &str) -> bool {
        security::fs_exists(path)
    }
    fn emit(&self, _event: &WorkflowEvent) {}
}

const ENV_ALLOWLIST: &[&str] = &[
    "SystemRoot", "windir", "SystemDrive", "USERPROFILE", "APPDATA", "LOCALAPPDATA", "ProgramFiles",
    "ProgramFiles(x86)", "ProgramData", "TEMP", "TMP", "USERNAME", "HOME", "COMPUTERNAME",
];

static VAR_NAME: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[A-Za-z_][A-Za-z0-9_]{0,63}$").unwrap());

pub struct Substituter<'a> {
    pub vars: &'a BTreeMap<String, String>,
    pub host: &'a dyn WorkflowHost,
}

impl Substituter<'_> {
    /// Expand `{{name}}`, `{{now:%Y-%m-%d}}`, `{{known:downloads}}`, `{{env:SystemRoot}}`.
    /// Substituted values are never re-scanned, so user data cannot inject further templates.
    pub fn expand(&self, input: &str) -> Result<String> {
        let chars: Vec<char> = input.chars().collect();
        let (s, _) = self.parse(&chars, 0, 0)?;
        if s.contains('\0') {
            return Err(CoreError::invalid("value contains NUL"));
        }
        Ok(s)
    }

    fn parse(&self, c: &[char], mut i: usize, depth: usize) -> Result<(String, usize)> {
        if depth > 6 {
            return Err(CoreError::invalid("template nesting too deep"));
        }
        let mut out = String::new();
        while i < c.len() {
            if c[i] == '{' && i + 1 < c.len() && c[i + 1] == '{' {
                let (inner, next) = self.parse(c, i + 2, depth + 1)?;
                out.push_str(&self.resolve(inner.trim())?);
                i = next;
            } else if c[i] == '}' && i + 1 < c.len() && c[i + 1] == '}' && depth > 0 {
                return Ok((out, i + 2));
            } else {
                out.push(c[i]);
                i += 1;
            }
        }
        if depth > 0 {
            return Err(CoreError::invalid("unterminated '{{' in template"));
        }
        Ok((out, i))
    }

    fn resolve(&self, key: &str) -> Result<String> {
        if let Some(fmt) = key.strip_prefix("now:") {
            use std::fmt::Write;
            let mut s = String::new();
            write!(s, "{}", self.host.now().format(fmt))
                .map_err(|_| CoreError::invalid(format!("invalid date format '{fmt}'")))?;
            return Ok(s);
        }
        if let Some(n) = key.strip_prefix("known:") {
            return self
                .host
                .known_folder(n)
                .ok_or_else(|| CoreError::NotFound(format!("known folder '{n}' is not available")));
        }
        if let Some(n) = key.strip_prefix("env:") {
            if !ENV_ALLOWLIST.iter().any(|a| a.eq_ignore_ascii_case(n)) {
                return Err(CoreError::Security(format!("environment variable '{n}' is not exposed to workflows")));
            }
            return std::env::var(n)
                .or_else(|_| {
                    std::env::vars().find(|(k, _)| k.eq_ignore_ascii_case(n)).map(|(_, v)| v).ok_or(std::env::VarError::NotPresent)
                })
                .map_err(|_| CoreError::NotFound(format!("environment variable '{n}' is not set")));
        }
        if !VAR_NAME.is_match(key) {
            return Err(CoreError::invalid(format!("invalid variable reference '{key}'")));
        }
        self.vars
            .get(key)
            .cloned()
            .ok_or_else(|| CoreError::invalid(format!("undefined variable '{key}'")))
    }
}

// ------------------------------------------------------------------ validation

fn check_len(s: &str, what: &str) -> Result<()> {
    if s.len() > MAX_FIELD_LEN {
        return Err(CoreError::invalid(format!("{what} is too long")));
    }
    Ok(())
}

fn count_steps(steps: &[Step]) -> usize {
    steps
        .iter()
        .map(|s| {
            1 + match &s.action {
                Action::If { then_steps, else_steps, .. } => count_steps(then_steps) + count_steps(else_steps),
                _ => 0,
            }
        })
        .sum()
}

/// Structural validation used for templates, editor saves and imported JSON.
pub fn validate_definition(def: &WorkflowDefinition) -> Result<()> {
    security::validate_display_name(&def.name)?;
    check_len(&def.description, "description")?;
    if def.id.is_empty() || def.id.len() > 100 {
        return Err(CoreError::invalid("invalid workflow id"));
    }
    if def.params.len() > MAX_PARAMS {
        return Err(CoreError::invalid("too many parameters"));
    }
    let mut names = std::collections::BTreeSet::new();
    for p in &def.params {
        if !VAR_NAME.is_match(&p.name) {
            return Err(CoreError::invalid(format!("invalid parameter name '{}'", p.name)));
        }
        if !names.insert(p.name.clone()) {
            return Err(CoreError::invalid(format!("duplicate parameter '{}'", p.name)));
        }
        check_len(&p.label, "parameter label")?;
        if p.kind == ParamKind::Choice && p.choices.is_empty() {
            return Err(CoreError::invalid(format!("choice parameter '{}' has no choices", p.name)));
        }
    }
    for (k, v) in &def.values {
        check_len(v, "parameter value")?;
        if !names.contains(k) {
            return Err(CoreError::invalid(format!("value for unknown parameter '{k}'")));
        }
    }
    if def.steps.is_empty() {
        return Err(CoreError::invalid("workflow has no steps"));
    }
    if def.kind == WorkflowKind::Command && def.steps.len() != 1 {
        return Err(CoreError::invalid("a command must have exactly one step"));
    }
    if count_steps(&def.steps) > MAX_STEPS {
        return Err(CoreError::invalid(format!("more than {MAX_STEPS} steps")));
    }
    let mut ids = std::collections::BTreeSet::new();
    validate_steps(&def.steps, 0, &mut ids, &names)
}

fn validate_cond(c: &Condition) -> Result<()> {
    match c {
        Condition::Exists { path } => check_len(path, "condition path"),
        Condition::NotEmpty { value } => check_len(value, "condition value"),
        Condition::Equals { left, right } => {
            check_len(left, "condition")?;
            check_len(right, "condition")
        }
        Condition::Not { inner } => validate_cond(inner),
    }
}

fn validate_steps(
    steps: &[Step],
    depth: usize,
    ids: &mut std::collections::BTreeSet<String>,
    params: &std::collections::BTreeSet<String>,
) -> Result<()> {
    if depth > MAX_NEST_DEPTH {
        return Err(CoreError::invalid("conditions nested too deeply"));
    }
    for s in steps {
        if s.id.is_empty() || s.id.len() > 64 || !ids.insert(s.id.clone()) {
            return Err(CoreError::invalid(format!("step id '{}' is empty or duplicated", s.id)));
        }
        check_len(&s.label, "step label")?;
        if s.retries > 3 {
            return Err(CoreError::invalid("at most 3 retries"));
        }
        if s.retries > 0 && !s.action.is_retry_safe() {
            return Err(CoreError::invalid(format!(
                "step '{}' ({}) has side effects and cannot be retried automatically",
                s.label,
                s.action.type_name()
            )));
        }
        if let Some(t) = s.timeout_ms {
            if t == 0 || t > MAX_TIMEOUT_MS {
                return Err(CoreError::invalid("timeout out of range"));
            }
        }
        if let Some(p) = &s.on_error_param {
            if !params.contains(p) {
                return Err(CoreError::invalid(format!("unknown error-policy parameter '{p}'")));
            }
        }
        if let Some(c) = &s.when {
            validate_cond(c)?;
        }
        match &s.action {
            Action::Wait { ms } if *ms > MAX_WAIT_MS => return Err(CoreError::invalid("wait too long (max 5 minutes)")),
            Action::SetVar { name, .. } if !VAR_NAME.is_match(name) => return Err(CoreError::invalid("invalid variable name")),
            Action::LaunchApp { args, .. } | Action::LaunchPath { args, .. } | Action::RunCommand { args, .. } => {
                security::validate_args(args)?
            }
            Action::If { condition, then_steps, else_steps } => {
                validate_cond(condition)?;
                validate_steps(then_steps, depth + 1, ids, params)?;
                validate_steps(else_steps, depth + 1, ids, params)?;
            }
            Action::LaunchCollection { system_key } if system_key.is_empty() => return Err(CoreError::invalid("collection key missing")),
            _ => {}
        }
        // Field length bounds for all string-bearing actions.
        let json = serde_json::to_string(&s.action)?;
        if json.len() > 64 * 1024 {
            return Err(CoreError::invalid("step is too large"));
        }
    }
    Ok(())
}

pub fn effective_values(def: &WorkflowDefinition, overrides: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut v = BTreeMap::new();
    for p in &def.params {
        let val = overrides
            .get(&p.name)
            .or_else(|| def.values.get(&p.name))
            .cloned()
            .or_else(|| p.default.clone())
            .unwrap_or_default();
        v.insert(p.name.clone(), val);
    }
    v
}

pub fn validate_param_values(def: &WorkflowDefinition, values: &BTreeMap<String, String>, host: &dyn WorkflowHost) -> Result<()> {
    for p in &def.params {
        let v = values.get(&p.name).map(|s| s.trim()).unwrap_or("");
        if v.is_empty() {
            if p.required {
                return Err(CoreError::invalid(format!("'{}' is required", p.label)));
            }
            continue;
        }
        let bad = |why: &str| CoreError::invalid(format!("'{}': {}", p.label, why));
        match p.kind {
            ParamKind::Text => {}
            ParamKind::Url => {
                security::validate_web_url(v).map_err(|e| bad(&e.to_string()))?;
            }
            ParamKind::File => {
                if !security::is_absolute_any(v) {
                    return Err(bad("must be an absolute path"));
                }
            }
            ParamKind::Folder => {
                if !security::is_absolute_any(v) {
                    return Err(bad("must be an absolute folder path"));
                }
                if !Path::new(v).is_dir() {
                    return Err(bad("folder does not exist"));
                }
            }
            ParamKind::App => {
                if !host.app_exists(v.strip_prefix("app:").unwrap_or(v)) {
                    return Err(bad("app is not registered"));
                }
            }
            ParamKind::Target => {
                if let Some(id) = v.strip_prefix("app:") {
                    if !host.app_exists(id) {
                        return Err(bad("app is not registered"));
                    }
                } else if !security::is_absolute_any(v) {
                    return Err(bad("must be a registered app or an absolute path"));
                }
            }
            ParamKind::Choice => {
                if !p.choices.iter().any(|c| c.value == v) {
                    return Err(bad("not one of the allowed choices"));
                }
            }
            ParamKind::Bool => {
                if v != "true" && v != "false" {
                    return Err(bad("must be true or false"));
                }
            }
            ParamKind::Number => {
                if v.parse::<f64>().is_err() {
                    return Err(bad("must be a number"));
                }
            }
        }
    }
    Ok(())
}

pub fn check_prerequisites(def: &WorkflowDefinition, values: &BTreeMap<String, String>, host: &dyn WorkflowHost) -> Vec<PrereqResult> {
    let sub = Substituter { vars: values, host };
    let mut out = Vec::new();
    for p in &def.prerequisites {
        match p {
            Prerequisite::Windows => out.push(PrereqResult {
                label: "Runs on Windows".into(),
                ok: host.is_windows(),
                detail: if host.is_windows() { "Windows detected".into() } else { "This action is only available on Windows".into() },
            }),
            Prerequisite::FileExists { label, path } => match sub.expand(path) {
                Ok(p) => {
                    let ok = host.path_exists(&p);
                    out.push(PrereqResult { label: label.clone(), ok, detail: if ok { p } else { format!("not found: {p}") } })
                }
                Err(e) => out.push(PrereqResult { label: label.clone(), ok: false, detail: e.to_string() }),
            },
            Prerequisite::AnyFileExists { label, paths } => {
                let found = paths.iter().filter_map(|p| sub.expand(p).ok()).find(|p| host.path_exists(p));
                out.push(PrereqResult {
                    label: label.clone(),
                    ok: found.is_some(),
                    detail: found.unwrap_or_else(|| "none of the expected locations exist".into()),
                });
            }
            Prerequisite::ParamsSet { names } => {
                let missing: Vec<_> = names.iter().filter(|n| values.get(*n).map(|v| v.trim().is_empty()).unwrap_or(true)).cloned().collect();
                let label = def
                    .params
                    .iter()
                    .filter(|p| names.contains(&p.name))
                    .map(|p| p.label.clone())
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push(PrereqResult {
                    label: format!("Configured: {label}"),
                    ok: missing.is_empty(),
                    detail: if missing.is_empty() { "set".into() } else { "choose a value in Configure".into() },
                });
            }
            Prerequisite::AnyParamSet { label, names } => {
                let ok = names.iter().any(|n| values.get(n).map(|v| !v.trim().is_empty()).unwrap_or(false));
                out.push(PrereqResult { label: label.clone(), ok, detail: if ok { "set".into() } else { "choose a value in Configure".into() } });
            }
            Prerequisite::CollectionNotEmpty { label, system_key } => {
                let n = host.collection_apps(system_key).map(|v| v.len()).unwrap_or(0);
                out.push(PrereqResult {
                    label: label.clone(),
                    ok: n > 0,
                    detail: if n > 0 { format!("{n} registered app(s)") } else { "no apps registered in this collection yet".into() },
                });
            }
        }
    }
    // Parameter validity is part of readiness too.
    if let Err(e) = validate_param_values(def, values, host) {
        out.push(PrereqResult { label: "Configuration is valid".into(), ok: false, detail: e.to_string() });
    }
    out
}

pub fn readiness(results: &[PrereqResult]) -> ReadyState {
    if results.iter().all(|r| r.ok) {
        ReadyState::Ready
    } else {
        ReadyState::NeedsSetup
    }
}

// ------------------------------------------------------------------- execution

enum Flow {
    Continue,
    Stop(RunStatus),
}

struct Exec<'a> {
    host: &'a dyn WorkflowHost,
    cancel: &'a CancelToken,
    vars: BTreeMap<String, String>,
    params: BTreeMap<String, String>,
    results: Vec<WorkflowStepResult>,
    executed: usize,
    run_id: String,
    allow_dev_cmd: bool,
}

struct ActionOk {
    message: String,
    output: Option<String>,
}

enum ActionErr {
    Failed(String),
    Cancelled(String),
}

impl From<CoreError> for ActionErr {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::Cancelled => ActionErr::Cancelled("cancelled".into()),
            other => ActionErr::Failed(other.to_string()),
        }
    }
}

type ActionResult = std::result::Result<ActionOk, ActionErr>;

fn ok(msg: impl Into<String>) -> ActionResult {
    Ok(ActionOk { message: msg.into(), output: None })
}

pub fn run_workflow(
    def: &WorkflowDefinition,
    overrides: &BTreeMap<String, String>,
    host: &dyn WorkflowHost,
    cancel: &CancelToken,
) -> WorkflowRun {
    let run_id = new_id();
    let run = WorkflowRun {
        id: run_id.clone(),
        workflow_id: def.id.clone(),
        workflow_name: def.name.clone(),
        started_at: now_iso(),
        finished_at: None,
        status: RunStatus::Running,
        steps: vec![],
    };
    host.emit(&WorkflowEvent::RunStarted { run_id: run_id.clone(), workflow_id: def.id.clone() });

    let finish = |mut run: WorkflowRun, status: RunStatus, steps: Vec<WorkflowStepResult>| {
        run.status = status;
        run.steps = steps;
        run.finished_at = Some(now_iso());
        host.emit(&WorkflowEvent::RunFinished { run: run.clone() });
        run
    };

    if !def.enabled {
        let r = failed_result("validation", "Workflow", "workflow is disabled");
        return finish(run, RunStatus::Failed, vec![r]);
    }
    if let Err(e) = validate_definition(def) {
        let r = failed_result("validation", "Validate workflow", &e.to_string());
        return finish(run, RunStatus::Failed, vec![r]);
    }
    let values = effective_values(def, overrides);
    if let Err(e) = validate_param_values(def, &values, host) {
        let r = failed_result("validation", "Validate parameters", &e.to_string());
        return finish(run, RunStatus::Failed, vec![r]);
    }

    let mut ex = Exec {
        host,
        cancel,
        vars: values.clone(),
        params: values,
        results: vec![],
        executed: 0,
        run_id,
        allow_dev_cmd: host.developer_commands_enabled(),
    };
    let flow = ex.exec_steps(&def.steps, "");
    let status = match flow {
        Flow::Continue => {
            if ex.results.iter().any(|r| r.status == StepStatus::Failed) {
                RunStatus::Failed
            } else {
                RunStatus::Succeeded
            }
        }
        Flow::Stop(s) => s,
    };
    let steps = std::mem::take(&mut ex.results);
    finish(run, status, steps)
}

fn failed_result(id: &str, label: &str, msg: &str) -> WorkflowStepResult {
    WorkflowStepResult {
        step_id: id.into(),
        label: label.into(),
        action: "validation".into(),
        status: StepStatus::Failed,
        message: redact(msg),
        preview: String::new(),
        attempts: 0,
        duration_ms: 0,
        output: None,
    }
}

fn truncate_output(s: &str) -> String {
    if s.len() <= MAX_OUTPUT_BYTES {
        return s.to_string();
    }
    let mut end = MAX_OUTPUT_BYTES;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n... [output truncated]", &s[..end])
}

impl<'a> Exec<'a> {
    fn sub(&self, s: &str) -> Result<String> {
        Substituter { vars: &self.vars, host: self.host }.expand(s)
    }

    fn eval(&self, c: &Condition) -> Result<bool> {
        Ok(match c {
            Condition::Exists { path } => {
                let p = self.sub(path)?;
                !p.trim().is_empty() && self.host.path_exists(&p)
            }
            Condition::NotEmpty { value } => !self.sub(value)?.trim().is_empty(),
            Condition::Equals { left, right } => self.sub(left)? == self.sub(right)?,
            Condition::Not { inner } => !self.eval(inner)?,
        })
    }

    fn push(&mut self, r: WorkflowStepResult) {
        self.host.emit(&WorkflowEvent::StepFinished { run_id: self.run_id.clone(), result: r.clone() });
        self.results.push(r);
    }

    fn policy_continues(&self, step: &Step) -> bool {
        if let Some(p) = &step.on_error_param {
            if let Some(v) = self.params.get(p) {
                return v == "continue";
            }
        }
        step.on_error == ErrorPolicy::Continue
    }

    fn exec_steps(&mut self, steps: &[Step], prefix: &str) -> Flow {
        for step in steps {
            let sid = format!("{prefix}{}", step.id);
            if self.cancel.is_cancelled() {
                self.push(WorkflowStepResult {
                    step_id: sid,
                    label: step.label.clone(),
                    action: step.action.type_name().into(),
                    status: StepStatus::Cancelled,
                    message: "cancelled before start".into(),
                    preview: String::new(),
                    attempts: 0,
                    duration_ms: 0,
                    output: None,
                });
                return Flow::Stop(RunStatus::Cancelled);
            }
            self.executed += 1;
            if self.executed > MAX_EXECUTED_STEPS {
                self.push(failed_result(&sid, &step.label, "step limit exceeded"));
                return Flow::Stop(RunStatus::Failed);
            }
            let mut base = WorkflowStepResult {
                step_id: sid.clone(),
                label: step.label.clone(),
                action: step.action.type_name().into(),
                status: StepStatus::Pending,
                message: String::new(),
                preview: String::new(),
                attempts: 0,
                duration_ms: 0,
                output: None,
            };
            if let Some(w) = &step.when {
                match self.eval(w) {
                    Ok(true) => {}
                    Ok(false) => {
                        base.status = StepStatus::Skipped;
                        base.message = "condition not met".into();
                        self.push(base);
                        continue;
                    }
                    Err(e) => {
                        base.status = StepStatus::Failed;
                        base.message = redact(&e.to_string());
                        self.push(base);
                        if self.policy_continues(step) {
                            continue;
                        }
                        return Flow::Stop(RunStatus::Failed);
                    }
                }
            }

            if let Action::If { condition, then_steps, else_steps } = &step.action {
                match self.eval(condition) {
                    Ok(branch) => {
                        base.status = StepStatus::Succeeded;
                        base.message = format!("condition is {branch}; running {} branch", if branch { "then" } else { "else" });
                        self.push(base);
                        let (steps, tag) = if branch { (then_steps, "then") } else { (else_steps, "else") };
                        match self.exec_steps(steps, &format!("{sid}.{tag}.")) {
                            Flow::Continue => {}
                            stop => return stop,
                        }
                    }
                    Err(e) => {
                        base.status = StepStatus::Failed;
                        base.message = redact(&e.to_string());
                        self.push(base);
                        if !self.policy_continues(step) {
                            return Flow::Stop(RunStatus::Failed);
                        }
                    }
                }
                continue;
            }

            let started = Instant::now();
            let max_attempts = 1 + step.retries.min(3);
            let mut last: Option<std::result::Result<ActionOk, ActionErr>> = None;
            for attempt in 1..=max_attempts {
                base.attempts = attempt;
                let r = self.run_action(step, &mut base);
                let done = matches!(r, Ok(_) | Err(ActionErr::Cancelled(_)));
                last = Some(r);
                if done || self.cancel.is_cancelled() {
                    break;
                }
            }
            base.duration_ms = started.elapsed().as_millis() as u64;
            match last.expect("at least one attempt") {
                Ok(o) => {
                    base.status = StepStatus::Succeeded;
                    base.message = redact(&o.message);
                    base.output = o.output.map(|s| truncate_output(&redact(&s)));
                    self.push(base);
                }
                Err(ActionErr::Cancelled(m)) => {
                    base.status = StepStatus::Cancelled;
                    base.message = redact(&m);
                    self.push(base);
                    return Flow::Stop(RunStatus::Cancelled);
                }
                Err(ActionErr::Failed(m)) => {
                    base.status = StepStatus::Failed;
                    base.message = redact(&m);
                    self.push(base);
                    if !self.policy_continues(step) {
                        return Flow::Stop(RunStatus::Failed);
                    }
                }
            }
        }
        Flow::Continue
    }

    fn run_action(&mut self, step: &Step, res: &mut WorkflowStepResult) -> ActionResult {
        let host = self.host;
        let cancel = self.cancel;
        match &step.action {
            Action::LaunchApp { app_id, args } => {
                let id = self.sub(app_id)?;
                let id = id.strip_prefix("app:").unwrap_or(&id).to_string();
                let args = self.sub_all(args)?;
                res.preview = format!("launch registered app {id} {}", security::join_windows_args(&args));
                self.announce(step, res);
                self.launch_app_result(&id, &args)
            }
            Action::LaunchPath { path, args, working_dir } => {
                let path = self.sub(path)?;
                let args = self.sub_all(args)?;
                let wd = match working_dir {
                    Some(w) => Some(self.sub(w)?),
                    None => None,
                };
                let target = crate::models::ExecutableTarget {
                    kind: crate::models::TargetKind::Executable,
                    path: path.clone(),
                    args: args.clone(),
                    working_dir: wd.clone().filter(|w| !w.is_empty()),
                    aumid: None,
                    uri: None,
                };
                res.preview = security::command_preview(&target);
                self.announce(step, res);
                security::validate_target(&target)?;
                let r = host.launch_path(&path, &args, target.working_dir.as_deref())?;
                launch_to_action(r)
            }
            Action::OpenTarget { target } => {
                let t = self.sub(target)?;
                if let Some(id) = t.strip_prefix("app:") {
                    res.preview = format!("launch registered app {id}");
                    self.announce(step, res);
                    return self.launch_app_result(id, &[]);
                }
                res.preview = format!("open {t}");
                self.announce(step, res);
                self.open_path_checked(&t)
            }
            Action::OpenPath { path } => {
                let p = self.sub(path)?;
                res.preview = format!("open {p}");
                self.announce(step, res);
                self.open_path_checked(&p)
            }
            Action::OpenUrl { url } => {
                let u = self.sub(url)?;
                let parsed = security::validate_web_url(&u)?;
                res.preview = format!("open {} in the default browser", parsed);
                self.announce(step, res);
                host.open_url(parsed.as_str())?;
                ok(format!("Opened {}", parsed.host_str().unwrap_or("")))
            }
            Action::OpenSettingsUri { uri } => {
                let u = self.sub(uri)?;
                let parsed = security::validate_settings_uri(&u)?;
                res.preview = format!("open {parsed}");
                self.announce(step, res);
                host.open_settings_uri(parsed.as_str())?;
                ok(format!("Requested {parsed}"))
            }
            Action::CopyText { text } => {
                let t = self.sub(text)?;
                if t.len() > MAX_COPY_BYTES {
                    return Err(ActionErr::Failed("text too large to copy".into()));
                }
                res.preview = format!("copy to clipboard: {}", redact(&t));
                self.announce(step, res);
                host.copy_text(&t)?;
                ok(format!("Copied to clipboard: {}", redact(&t)))
            }
            Action::Wait { ms } => {
                res.preview = format!("wait {ms} ms");
                self.announce(step, res);
                let end = Instant::now() + Duration::from_millis((*ms).min(MAX_WAIT_MS));
                while Instant::now() < end {
                    if cancel.is_cancelled() {
                        return Err(ActionErr::Cancelled("cancelled while waiting".into()));
                    }
                    std::thread::sleep(Duration::from_millis(15).min(end.saturating_duration_since(Instant::now())));
                }
                ok(format!("Waited {ms} ms"))
            }
            Action::Notify { title, body } => {
                let (t, b) = (self.sub(title)?, self.sub(body)?);
                res.preview = format!("notify: {t} - {b}");
                self.announce(step, res);
                host.notify(&t, &b)?;
                ok("Notification shown")
            }
            Action::CreateNote { dir, file_name, content } => {
                let dir = self.sub(dir)?;
                let name = self.sub(file_name)?;
                let content = self.sub(content)?;
                res.preview = format!("create new text file {name} in {dir}");
                self.announce(step, res);
                let d = Path::new(&dir);
                if !security::is_absolute_any(&dir) || !d.is_dir() {
                    return Err(ActionErr::Failed(format!("folder does not exist: {dir}")));
                }
                let created = create_unique_file(d, &name, content.as_bytes())?;
                self.vars.insert("note_path".into(), created.clone());
                ok(format!("Created {created}"))
            }
            Action::Confirm { message } => {
                let m = self.sub(message)?;
                res.preview = format!("ask for confirmation: {m}");
                self.announce(step, res);
                let to = Duration::from_millis(step.timeout_ms.unwrap_or(120_000));
                if host.confirm(&m, to, cancel)? {
                    ok("Confirmed by user")
                } else {
                    Err(ActionErr::Cancelled("declined by user".into()))
                }
            }
            Action::Fail { message } => {
                let m = self.sub(message)?;
                res.preview = "stop with error".into();
                self.announce(step, res);
                Err(ActionErr::Failed(m))
            }
            Action::SetVar { name, value } => {
                let v = self.sub(value)?;
                res.preview = format!("set {name}");
                self.announce(step, res);
                self.vars.insert(name.clone(), v);
                ok(format!("Set {name}"))
            }
            Action::LaunchCollection { system_key } => {
                res.preview = format!("launch every app in collection '{system_key}' sequentially");
                self.announce(step, res);
                let apps = host.collection_apps(system_key)?;
                if apps.is_empty() {
                    return Err(ActionErr::Failed(format!(
                        "collection '{system_key}' has no registered apps. Add your real apps first."
                    )));
                }
                let mut lines = Vec::new();
                let mut failed = 0;
                for (id, name) in apps {
                    if cancel.is_cancelled() {
                        return Err(ActionErr::Cancelled("cancelled".into()));
                    }
                    match host.launch_app(&id, &[]) {
                        Ok(r) if r.outcome == LaunchOutcomeKind::Accepted => lines.push(format!("OK    {name}: {}", r.message)),
                        Ok(r) => {
                            failed += 1;
                            lines.push(format!("FAIL  {name}: {}", r.message))
                        }
                        Err(e) => {
                            failed += 1;
                            lines.push(format!("FAIL  {name}: {e}"))
                        }
                    }
                }
                let out = Some(lines.join("\n"));
                if failed > 0 {
                    Err(ActionErr::Failed(format!("{failed} app(s) failed to launch:\n{}", out.unwrap_or_default())))
                } else {
                    Ok(ActionOk { message: format!("Launch requested for {} app(s)", lines.len()), output: out })
                }
            }
            Action::RunCommand { program, args, working_dir, trusted } => {
                let program = self.sub(program)?;
                let args = self.sub_all(args)?;
                let wd = match working_dir {
                    Some(w) => Some(self.sub(w)?),
                    None => None,
                };
                let preview = {
                    let mut s = security::quote_windows_arg(&program);
                    if !args.is_empty() {
                        s.push(' ');
                        s.push_str(&security::join_windows_args(&args));
                    }
                    s
                };
                res.preview = preview.clone();
                self.announce(step, res);
                if !self.allow_dev_cmd {
                    return Err(ActionErr::Failed(
                        "developer command execution is turned off (Settings > Security). Nothing was run.".into(),
                    ));
                }
                security::validate_args(&args)?;
                if !security::is_absolute_any(&program) || !security::fs_is_file(&program) {
                    return Err(ActionErr::Failed(format!("program not found (absolute path required): {program}")));
                }
                if !*trusted {
                    let ask = format!("Run this command?\n\n{preview}");
                    if !host.confirm(&ask, Duration::from_millis(120_000), cancel)? {
                        return Err(ActionErr::Cancelled("declined by user".into()));
                    }
                }
                let timeout = Duration::from_millis(step.timeout_ms.unwrap_or(DEFAULT_COMMAND_TIMEOUT_MS));
                run_process(&program, &args, wd.as_deref(), timeout, cancel)
            }
            Action::If { .. } => unreachable!("handled in exec_steps"),
        }
    }

    fn announce(&self, step: &Step, res: &WorkflowStepResult) {
        self.host.emit(&WorkflowEvent::StepStarted {
            run_id: self.run_id.clone(),
            step_id: res.step_id.clone(),
            label: step.label.clone(),
            preview: redact(&res.preview),
        });
    }

    fn sub_all(&self, args: &[String]) -> Result<Vec<String>> {
        args.iter().map(|a| self.sub(a)).collect()
    }

    fn launch_app_result(&self, id: &str, args: &[String]) -> ActionResult {
        if !self.host.app_exists(id) {
            return Err(ActionErr::Failed(format!("app '{id}' is not registered")));
        }
        launch_to_action(self.host.launch_app(id, args)?)
    }

    fn open_path_checked(&self, p: &str) -> ActionResult {
        if !security::is_absolute_any(p) {
            return Err(ActionErr::Failed(format!("path must be absolute: {p}")));
        }
        if !self.host.path_exists(p) {
            return Err(ActionErr::Failed(format!("not found: {p}")));
        }
        self.host.open_path(p)?;
        ok(format!("Opened {p}"))
    }
}

fn launch_to_action(r: LaunchResult) -> ActionResult {
    match r.outcome {
        LaunchOutcomeKind::Accepted => ok(r.message),
        _ => Err(ActionErr::Failed(r.message)),
    }
}

/// Create a new file without ever overwriting: on collision a numeric suffix is appended.
fn create_unique_file(dir: &Path, name: &str, content: &[u8]) -> std::result::Result<String, ActionErr> {
    use std::io::Write;
    let first = security::safe_join(dir, name)?;
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
        _ => (name.to_string(), String::new()),
    };
    for i in 0..100 {
        let path = if i == 0 { first.clone() } else { security::safe_join(dir, &format!("{stem}-{i}{ext}"))? };
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                f.write_all(content).map_err(|e| ActionErr::Failed(format!("write failed: {e}")))?;
                return Ok(path.to_string_lossy().into_owned());
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(ActionErr::Failed(format!("cannot create file: {e}"))),
        }
    }
    Err(ActionErr::Failed("could not find a free file name".into()))
}

fn run_process(program: &str, args: &[String], wd: Option<&str>, timeout: Duration, cancel: &CancelToken) -> ActionResult {
    use std::process::{Command, Stdio};
    let mut cmd = Command::new(program);
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(w) = wd.filter(|w| !w.is_empty()) {
        cmd.current_dir(w);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn().map_err(|e| ActionErr::Failed(format!("could not start process: {e}")))?;
    let read = |mut r: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = (&mut r).take(MAX_OUTPUT_BYTES as u64 * 4).read_to_end(&mut buf);
            let mut sink = [0u8; 4096];
            while matches!(r.read(&mut sink), Ok(n) if n > 0) {}
            String::from_utf8_lossy(&buf).into_owned()
        })
    };
    let out_h = read(Box::new(child.stdout.take().expect("piped")));
    let err_h = read(Box::new(child.stderr.take().expect("piped")));
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) => {}
            Err(e) => return Err(ActionErr::Failed(format!("wait failed: {e}"))),
        }
        if cancel.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ActionErr::Cancelled("cancelled; process was terminated".into()));
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(ActionErr::Failed(format!("timed out after {} ms; process was terminated", timeout.as_millis())));
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let stdout = out_h.join().unwrap_or_default();
    let stderr = err_h.join().unwrap_or_default();
    let mut combined = String::new();
    if !stdout.is_empty() {
        combined.push_str(&format!("[stdout]\n{stdout}"));
    }
    if !stderr.is_empty() {
        combined.push_str(&format!("[stderr]\n{stderr}"));
    }
    let output = if combined.is_empty() { None } else { Some(combined) };
    if status.success() {
        Ok(ActionOk { message: "Process exited with code 0".into(), output })
    } else {
        Err(ActionErr::Failed(format!(
            "Process exited with {}{}",
            status.code().map(|c| format!("code {c}")).unwrap_or_else(|| "a signal".into()),
            output.map(|o| format!("\n{}", truncate_output(&redact(&o)))).unwrap_or_default()
        )))
    }
}
