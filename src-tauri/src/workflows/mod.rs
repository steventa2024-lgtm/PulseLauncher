//! Sequential workflow engine.
//!
//! State machine for a run:
//! `running` → `succeeded` | `failed` | `cancelled`.
//! Awaiting a confirmation blocks inside `running` until the host answers,
//! the user cancels, or 120 seconds pass (treated as declined).
//!
//! Steps run in order. `checkExists` is the only branch. There is no goto, so
//! a definition cannot cycle; depth and step count are bounded anyway.
//! Variable substitution happens once, per string, and never splits a value
//! into extra arguments.

use std::collections::{BTreeMap, HashSet};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult, ErrorCode};
use crate::models::LaunchOutcome;
use crate::security;

pub mod notes;
pub mod templates;

pub const SCHEMA: u32 = 1;
const MAX_STEPS: usize = 64;
const MAX_DEPTH: usize = 8;
const MAX_DEFINITION_BYTES: usize = 256 * 1024;
const MAX_RUN: Duration = Duration::from_secs(5 * 60);
const MAX_WAIT_MS: u64 = 60_000;
const MAX_RETRIES: u8 = 2;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowKind {
    Command,
    Workflow,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ParamKind {
    Text,
    /// Absolute folder that must exist.
    Folder,
    /// Absolute file that must exist.
    File,
    /// Absolute path that is allowed not to exist yet (existence checks).
    Path,
    Url,
    /// Id of one registered app.
    App,
    /// JSON array of registered app ids.
    Apps,
    Choice,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ParamChoice {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ParamDef {
    pub key: String,
    pub label: String,
    pub help: String,
    pub kind: ParamKind,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default_value: String,
    #[serde(default)]
    pub choices: Vec<ParamChoice>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OnError {
    #[default]
    Stop,
    Continue,
    Skip,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Action {
    LaunchApp {
        app_id: String,
        #[serde(default)]
        args: Vec<String>,
    },
    LaunchEach {
        /// JSON array of app ids, usually `${apps}`.
        apps: String,
    },
    LaunchPath {
        path: String,
        #[serde(default)]
        args: Vec<String>,
    },
    OpenFolder {
        path: String,
    },
    OpenUrl {
        url: String,
    },
    OpenUri {
        uri: String,
    },
    CopyText {
        text: String,
    },
    Wait {
        millis: u64,
    },
    Notify {
        title: String,
        body: String,
    },
    WriteNote {
        folder: String,
        #[serde(default)]
        prefix: String,
        #[serde(default)]
        content: String,
    },
    CheckExists {
        path: String,
        #[serde(default)]
        then_steps: Vec<Step>,
        #[serde(default)]
        else_steps: Vec<Step>,
    },
    Confirm {
        message: String,
    },
    /// Arbitrary program. Executes only when developer command execution is
    /// enabled and the workflow has been explicitly trusted.
    RunCommand {
        program: String,
        #[serde(default)]
        args: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    #[serde(default)]
    pub on_error: OnError,
    #[serde(default)]
    pub retries: u8,
    #[serde(flatten)]
    pub action: Action,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowDefinition {
    pub schema: u32,
    pub id: String,
    pub name: String,
    pub description: String,
    pub what_it_does: String,
    pub prerequisites: String,
    pub kind: WorkflowKind,
    pub template_key: Option<String>,
    pub template_version: Option<u32>,
    pub params: Vec<ParamDef>,
    #[serde(default)]
    pub values: BTreeMap<String, String>,
    pub steps: Vec<Step>,
    pub hotkey: Option<String>,
    #[serde(default)]
    pub trusted: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StepLog {
    pub index: String,
    pub action: String,
    pub status: String,
    pub message: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRun {
    pub id: String,
    pub workflow_id: String,
    pub name: String,
    pub status: RunStatus,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub steps: Vec<StepLog>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReadinessState {
    Ready,
    NeedsSetup,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Readiness {
    pub state: ReadinessState,
    pub details: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalCandidate {
    pub label: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Facts {
    pub is_windows: bool,
    pub downloads: Option<String>,
    pub documents: Option<String>,
    pub task_manager: Option<String>,
    pub terminals: Vec<TerminalCandidate>,
}

impl Facts {
    pub fn current() -> Self {
        Self {
            is_windows: crate::platform::is_windows(),
            downloads: crate::platform::downloads_dir().map(|p| p.to_string_lossy().into_owned()),
            documents: crate::platform::documents_dir().map(|p| p.to_string_lossy().into_owned()),
            task_manager: crate::platform::task_manager_path(),
            terminals: crate::platform::terminal_candidates()
                .into_iter()
                .map(|(label, path)| TerminalCandidate { label, path })
                .collect(),
        }
    }
}

pub trait Actions {
    fn launch_app(&self, app_id: &str, args: &[String]) -> AppResult<LaunchOutcome>;
    fn launch_path(&self, path: &str, args: &[String]) -> AppResult<LaunchOutcome>;
    fn open_folder(&self, path: &str) -> AppResult<LaunchOutcome>;
    fn open_url(&self, url: &str) -> AppResult<LaunchOutcome>;
    fn open_uri(&self, uri: &str) -> AppResult<LaunchOutcome>;
    fn write_note(&self, folder: &str, prefix: &str, content: &str) -> AppResult<String>;
    fn run_command(&self, program: &str, args: &[String]) -> AppResult<LaunchOutcome>;
    fn app_exists(&self, app_id: &str) -> bool;
}

pub trait Host {
    fn confirm(&self, message: &str) -> bool;
    fn notify(&self, title: &str, body: &str);
    fn copy(&self, text: &str) -> AppResult<()>;
    fn cancelled(&self) -> bool;
    fn progress(&self, run: &WorkflowRun);
}

// ----------------------------------------------------------------- values

pub fn builtin_values(user: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut v = user.clone();
    let now = chrono::Local::now();
    let iso = now.format("%Y-%m-%d").to_string();
    let long = now.format("%A, %B %d, %Y").to_string();
    v.insert("today_iso".into(), iso.clone());
    v.insert("today_long".into(), long.clone());
    v.insert("now_file".into(), now.format("%Y%m%d-%H%M%S").to_string());
    let formatted = if v.get("format").map(String::as_str) == Some("long") {
        long
    } else {
        iso
    };
    v.insert("date_text".into(), formatted);
    if let Some(p) = crate::platform::downloads_dir() {
        v.insert("downloads".into(), p.to_string_lossy().into_owned());
    }
    if let Some(p) = crate::platform::documents_dir() {
        v.insert("documents".into(), p.to_string_lossy().into_owned());
    }
    if let Some(p) = crate::platform::task_manager_path() {
        v.insert("taskmgr".into(), p);
    }
    v
}

pub fn substitute(input: &str, values: &BTreeMap<String, String>) -> AppResult<String> {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '$' && chars.get(i + 1) == Some(&'{') {
            let start = i + 2;
            let Some(rel) = chars[start..].iter().position(|c| *c == '}') else {
                return Err(AppError::invalid(format!(
                    "Unclosed placeholder in \"{input}\""
                )));
            };
            let key: String = chars[start..start + rel].iter().collect();
            let key = key.trim();
            if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return Err(AppError::invalid(format!("Invalid placeholder \"{key}\"")));
            }
            let Some(value) = values.get(key) else {
                return Err(AppError::new(
                    ErrorCode::InvalidInput,
                    format!("\"{key}\" is not configured"),
                ));
            };
            out.push_str(value);
            i = start + rel + 1;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    if out.len() > 8192 {
        return Err(AppError::invalid("A substituted value is too long"));
    }
    Ok(out)
}

fn subst_args(args: &[String], values: &BTreeMap<String, String>) -> AppResult<Vec<String>> {
    args.iter().map(|a| substitute(a, values)).collect()
}

// --------------------------------------------------------------- validate

fn valid_key(key: &str) -> bool {
    !key.is_empty() && key.len() <= 32 && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub fn validate_definition(def: &WorkflowDefinition) -> AppResult<()> {
    if def.schema != SCHEMA {
        return Err(AppError::invalid(format!(
            "Unsupported workflow schema {}. This version reads schema {SCHEMA}.",
            def.schema
        )));
    }
    security::bounded_text(&def.name, 120, "Workflow name")?;
    if def.description.chars().count() > 2000 || def.what_it_does.chars().count() > 2000 {
        return Err(AppError::invalid("Description is too long"));
    }
    if def.params.len() > 32 {
        return Err(AppError::invalid(
            "A workflow can have at most 32 parameters",
        ));
    }
    let mut keys = HashSet::new();
    for p in &def.params {
        if !valid_key(&p.key) || !keys.insert(p.key.clone()) {
            return Err(AppError::invalid(format!(
                "Invalid parameter name \"{}\"",
                p.key
            )));
        }
    }
    for (k, v) in &def.values {
        if !valid_key(k) {
            return Err(AppError::invalid(format!("Invalid value name \"{k}\"")));
        }
        if v.len() > 4000 {
            return Err(AppError::invalid(format!("Value \"{k}\" is too long")));
        }
    }
    let mut count = 0usize;
    validate_steps(&def.steps, 1, &mut count)?;
    if serde_json::to_vec(def)?.len() > MAX_DEFINITION_BYTES {
        return Err(AppError::invalid("Workflow definition is too large"));
    }
    Ok(())
}

fn validate_steps(steps: &[Step], depth: usize, count: &mut usize) -> AppResult<()> {
    if depth > MAX_DEPTH {
        return Err(AppError::invalid("Workflow is nested too deeply"));
    }
    for step in steps {
        *count += 1;
        if *count > MAX_STEPS {
            return Err(AppError::invalid(format!(
                "A workflow can have at most {MAX_STEPS} steps"
            )));
        }
        if let Action::CheckExists {
            then_steps,
            else_steps,
            ..
        } = &step.action
        {
            validate_steps(then_steps, depth + 1, count)?;
            validate_steps(else_steps, depth + 1, count)?;
        }
        if let Action::Wait { millis } = &step.action {
            if *millis > MAX_WAIT_MS {
                return Err(AppError::invalid(format!(
                    "A wait cannot be longer than {MAX_WAIT_MS} ms"
                )));
            }
        }
    }
    Ok(())
}

pub fn validate_values(
    def: &WorkflowDefinition,
    values: &BTreeMap<String, String>,
    known_apps: &HashSet<String>,
) -> AppResult<()> {
    for (key, value) in values {
        if !valid_key(key) {
            return Err(AppError::invalid(format!("Invalid value name \"{key}\"")));
        }
        if value.len() > 4000 {
            return Err(AppError::invalid(format!("Value \"{key}\" is too long")));
        }
        if value.trim().is_empty() {
            continue;
        }
        let Some(param) = def.params.iter().find(|p| p.key == *key) else {
            continue;
        };
        match param.kind {
            ParamKind::Text => {
                security::bounded_text(value, 4000, &param.label)?;
            }
            ParamKind::Folder => {
                let p = security::validate_absolute_path(value, &param.label)?;
                if !p.is_dir() {
                    return Err(AppError::new(
                        ErrorCode::TargetMissing,
                        format!("{} not found: {}", param.label, p.display()),
                    ));
                }
            }
            ParamKind::File => {
                let p = security::validate_absolute_path(value, &param.label)?;
                if !p.is_file() {
                    return Err(AppError::new(
                        ErrorCode::TargetMissing,
                        format!("{} not found: {}", param.label, p.display()),
                    ));
                }
            }
            ParamKind::Path => {
                security::validate_absolute_path(value, &param.label)?;
            }
            ParamKind::Url => {
                security::validate_web_url(value)?;
            }
            ParamKind::App => {
                if !known_apps.contains(value) {
                    return Err(AppError::not_found(format!(
                        "No registered app with id \"{value}\""
                    )));
                }
            }
            ParamKind::Apps => {
                let ids: Vec<String> = serde_json::from_str(value)
                    .map_err(|_| AppError::invalid("App list must be a JSON array of ids"))?;
                if ids.len() > 32 {
                    return Err(AppError::invalid("At most 32 apps in a list"));
                }
                for id in ids {
                    if !known_apps.contains(&id) {
                        return Err(AppError::not_found(format!(
                            "No registered app with id \"{id}\""
                        )));
                    }
                }
            }
            ParamKind::Choice => {
                if !param.choices.is_empty() && !param.choices.iter().any(|c| c.value == *value) {
                    return Err(AppError::invalid(format!(
                        "\"{value}\" is not one of the choices for {}",
                        param.label
                    )));
                }
            }
        }
    }
    Ok(())
}

pub fn contains_run_command(steps: &[Step]) -> bool {
    steps.iter().any(|s| match &s.action {
        Action::RunCommand { .. } => true,
        Action::CheckExists {
            then_steps,
            else_steps,
            ..
        } => contains_run_command(then_steps) || contains_run_command(else_steps),
        _ => false,
    })
}

pub fn parse_import(json: &str) -> AppResult<Vec<WorkflowDefinition>> {
    if json.len() > MAX_DEFINITION_BYTES {
        return Err(AppError::invalid("Import file is larger than 256 KB"));
    }
    let defs: Vec<WorkflowDefinition> = serde_json::from_str(json)?;
    if defs.len() > 200 {
        return Err(AppError::invalid(
            "An import can contain at most 200 workflows",
        ));
    }
    for def in &defs {
        validate_definition(def)?;
    }
    Ok(defs)
}

// -------------------------------------------------------------- readiness

fn filled<'a>(def: &'a WorkflowDefinition, key: &str) -> Option<&'a str> {
    def.values
        .get(key)
        .map(String::as_str)
        .filter(|s| !s.trim().is_empty())
}

fn configured_count(def: &WorkflowDefinition, keys: &[&str], apps: &HashSet<String>) -> usize {
    keys.iter()
        .filter(|k| match filled(def, k) {
            Some(v)
                if def
                    .params
                    .iter()
                    .any(|p| p.key == **k && p.kind == ParamKind::Folder) =>
            {
                std::path::Path::new(v).is_dir()
            }
            Some(v)
                if def
                    .params
                    .iter()
                    .any(|p| p.key == **k && p.kind == ParamKind::App) =>
            {
                apps.contains(v)
            }
            Some(_) => true,
            None => false,
        })
        .count()
}

pub fn readiness(def: &WorkflowDefinition, facts: &Facts, apps: &HashSet<String>) -> Readiness {
    let ready = |details: Vec<String>| Readiness {
        state: ReadinessState::Ready,
        details,
    };
    let setup = |details: Vec<String>| Readiness {
        state: ReadinessState::NeedsSetup,
        details,
    };
    let unavailable = |details: Vec<String>| Readiness {
        state: ReadinessState::Unavailable,
        details,
    };

    match def.template_key.as_deref() {
        Some("open-downloads") => match &facts.downloads {
            Some(p) => ready(vec![format!("Opens {p}")]),
            None => unavailable(vec![
                "The Downloads folder could not be resolved on this system.".into(),
            ]),
        },
        Some("open-documents") => match &facts.documents {
            Some(p) => ready(vec![format!("Opens {p}")]),
            None => unavailable(vec![
                "The Documents folder could not be resolved on this system.".into(),
            ]),
        },
        Some("open-settings") => {
            if facts.is_windows {
                ready(vec![
                    "Opens ms-settings: with the Windows association.".into()
                ])
            } else {
                unavailable(vec!["Windows Settings links only open on Windows.".into()])
            }
        }
        Some("open-task-manager") => match &facts.task_manager {
            Some(p) => ready(vec![format!("Launches {p}")]),
            None => unavailable(vec![
                "Task Manager was not found at the expected Windows path.".into(),
            ]),
        },
        Some("open-terminal") => match filled(def, "terminal") {
            Some(p) if std::path::Path::new(p).is_file() => ready(vec![format!("Launches {p}")]),
            Some(p) => setup(vec![format!(
                "{p} is no longer present. Choose another terminal."
            )]),
            None => setup(vec![if facts.terminals.is_empty() {
                "No terminal was detected. Choose an installed terminal program.".into()
            } else {
                "Choose which terminal to open.".into()
            }]),
        },
        Some("open-project") => match filled(def, "folder") {
            Some(p) if std::path::Path::new(p).is_dir() => ready(vec![format!("Opens {p}")]),
            Some(_) => setup(vec!["The saved folder no longer exists.".into()]),
            None => setup(vec!["Choose a project folder.".into()]),
        },
        Some("dev-workspace") => {
            let n = configured_count(def, &["app1", "app2", "folder"], apps);
            if n >= 2 {
                ready(vec![format!(
                    "{n} items configured. Missing items are skipped."
                )])
            } else {
                setup(vec![
                    "Configure at least two apps or a folder. Nothing runs until you do.".into(),
                ])
            }
        }
        Some("copy-date") => ready(vec![format!(
            "Copies today's date ({})",
            if filled(def, "format") == Some("long") {
                "long form"
            } else {
                "ISO"
            }
        )]),
        Some("quick-note") => match filled(def, "folder") {
            Some(p) if std::path::Path::new(p).is_dir() => {
                ready(vec![format!("Creates a note in {p}")])
            }
            Some(_) => setup(vec!["The saved folder no longer exists.".into()]),
            None => setup(vec!["Choose a writable folder for notes.".into()]),
        },
        Some("open-website") => match filled(def, "url") {
            Some(u) if security::validate_web_url(u).is_ok() => ready(vec![format!("Opens {u}")]),
            Some(_) => setup(vec!["The saved address is not an http(s) URL.".into()]),
            None => setup(vec!["Enter an http:// or https:// address.".into()]),
        },
        Some("zeropulse-stack") => {
            let ids = filled(def, "apps")
                .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
                .unwrap_or_default();
            let present: Vec<_> = ids
                .iter()
                .filter(|id| apps.contains(*id))
                .cloned()
                .collect();
            if present.is_empty() {
                setup(vec!["No apps selected. Register PulseCut, PulseFlow, OpenPulse or any other program, then add it. Nothing is launched until you do.".into()])
            } else {
                ready(vec![format!(
                    "{} app(s) will be launched. Missing ones are reported, not invented.",
                    present.len()
                )])
            }
        }
        Some("check-then-launch") => match filled(def, "target") {
            Some(p) if security::validate_absolute_path(p, "Target").is_ok() => {
                if std::path::Path::new(p).is_file() {
                    ready(vec![format!(
                        "{p} exists and will be launched after confirmation."
                    )])
                } else {
                    ready(vec![format!(
                        "{p} is not present. Running reports that instead of launching."
                    )])
                }
            }
            Some(_) => setup(vec!["Target must be an absolute path.".into()]),
            None => setup(vec!["Choose the program or file to check.".into()]),
        },
        _ => {
            let missing: Vec<_> = def
                .params
                .iter()
                .filter(|p| p.required && filled(def, &p.key).is_none())
                .map(|p| format!("Set {}.", p.label))
                .collect();
            if missing.is_empty() {
                ready(vec!["Required parameters are set.".into()])
            } else {
                setup(missing)
            }
        }
    }
}

// ------------------------------------------------------------------ run

struct Counters {
    steps: usize,
    started: Instant,
}

fn guard(host: &dyn Host, c: &Counters, depth: usize) -> AppResult<()> {
    if host.cancelled() {
        return Err(AppError::new(ErrorCode::Cancelled, "Cancelled"));
    }
    if c.started.elapsed() > MAX_RUN {
        return Err(AppError::new(
            ErrorCode::Timeout,
            "The workflow ran for more than 5 minutes and was stopped",
        ));
    }
    if c.steps > MAX_STEPS {
        return Err(AppError::policy("Step limit reached; the run was stopped"));
    }
    if depth > MAX_DEPTH {
        return Err(AppError::policy(
            "Nesting limit reached; the run was stopped",
        ));
    }
    Ok(())
}

fn action_label(action: &Action) -> String {
    match action {
        Action::LaunchApp { .. } => "Launch app",
        Action::LaunchEach { .. } => "Launch apps",
        Action::LaunchPath { .. } => "Launch program",
        Action::OpenFolder { .. } => "Open folder",
        Action::OpenUrl { .. } => "Open URL",
        Action::OpenUri { .. } => "Open link",
        Action::CopyText { .. } => "Copy text",
        Action::Wait { .. } => "Wait",
        Action::Notify { .. } => "Notify",
        Action::WriteNote { .. } => "Create note",
        Action::CheckExists { .. } => "Check exists",
        Action::Confirm { .. } => "Confirm",
        Action::RunCommand { .. } => "Run command",
    }
    .to_string()
}

fn retryable(action: &Action) -> bool {
    matches!(
        action,
        Action::LaunchApp { .. }
            | Action::OpenFolder { .. }
            | Action::OpenUrl { .. }
            | Action::OpenUri { .. }
            | Action::CopyText { .. }
            | Action::Notify { .. }
            | Action::LaunchPath { .. }
            | Action::LaunchEach { .. }
    )
}

pub fn describe(def: &WorkflowDefinition) -> AppResult<Vec<String>> {
    let values = builtin_values(&def.values);
    let mut lines = Vec::new();
    describe_steps(&def.steps, &values, &mut lines)?;
    Ok(lines)
}

fn describe_steps(
    steps: &[Step],
    values: &BTreeMap<String, String>,
    lines: &mut Vec<String>,
) -> AppResult<()> {
    for step in steps {
        let detail = describe_action(&step.action, values)?;
        lines.push(detail);
        if let Action::CheckExists {
            then_steps,
            else_steps,
            ..
        } = &step.action
        {
            describe_steps(then_steps, values, lines)?;
            describe_steps(else_steps, values, lines)?;
        }
    }
    Ok(())
}

fn describe_action(action: &Action, values: &BTreeMap<String, String>) -> AppResult<String> {
    Ok(match action {
        Action::LaunchApp { app_id, args } => {
            let id = substitute(app_id, values).unwrap_or_else(|_| "(not configured)".into());
            let args = subst_args(args, values).unwrap_or_default();
            format!(
                "Launch registered app {id} {}",
                security::join_windows_args(&args)
            )
        }
        Action::LaunchEach { apps } => {
            let raw = substitute(apps, values).unwrap_or_else(|_| "[]".into());
            format!("Launch each configured app in {raw}")
        }
        Action::LaunchPath { path, args } => {
            let path = substitute(path, values).unwrap_or_else(|_| "(not configured)".into());
            let args = subst_args(args, values).unwrap_or_default();
            format!("Launch {}", security::preview_command(&path, &args))
        }
        Action::OpenFolder { path } => format!(
            "Open folder {}",
            substitute(path, values).unwrap_or_else(|_| "(not configured)".into())
        ),
        Action::OpenUrl { url } => format!(
            "Open URL {}",
            substitute(url, values).unwrap_or_else(|_| "(not configured)".into())
        ),
        Action::OpenUri { uri } => format!(
            "Open {}",
            substitute(uri, values).unwrap_or_else(|_| "(not configured)".into())
        ),
        Action::CopyText { text } => format!(
            "Copy \"{}\" to the clipboard",
            substitute(text, values).unwrap_or_else(|_| "(not configured)".into())
        ),
        Action::Wait { millis } => format!("Wait {millis} ms (cancellable)"),
        Action::Notify { title, body } => format!(
            "Notify \"{}\" — {}",
            substitute(title, values).unwrap_or_else(|_| title.clone()),
            substitute(body, values).unwrap_or_else(|_| body.clone())
        ),
        Action::WriteNote { folder, prefix, .. } => {
            format!(
                "Create a new timestamped note in {} (never overwrites)",
                substitute(folder, values).unwrap_or_else(|_| folder.clone()),
            ) + &format!(
                " named like {}-*.txt",
                if prefix.is_empty() { "note" } else { prefix }
            )
        }
        Action::CheckExists { path, .. } => format!(
            "If {} exists",
            substitute(path, values).unwrap_or_else(|_| "(not configured)".into())
        ),
        Action::Confirm { message } => format!(
            "Ask: {}",
            substitute(message, values).unwrap_or_else(|_| message.clone())
        ),
        Action::RunCommand { program, args } => format!(
            "Run {} — only if developer commands are enabled and this workflow is trusted",
            security::preview_command(
                &substitute(program, values).unwrap_or_else(|_| program.clone()),
                &subst_args(args, values).unwrap_or_default()
            )
        ),
    })
}

pub fn execute(
    def: &WorkflowDefinition,
    actions: &dyn Actions,
    host: &dyn Host,
    developer_commands: bool,
) -> WorkflowRun {
    let mut run = WorkflowRun {
        id: uuid::Uuid::new_v4().to_string(),
        workflow_id: def.id.clone(),
        name: def.name.clone(),
        status: RunStatus::Running,
        started_at: crate::storage::now(),
        finished_at: None,
        steps: vec![],
        error: None,
    };
    host.progress(&run);
    let values = builtin_values(&def.values);
    let mut counters = Counters {
        steps: 0,
        started: Instant::now(),
    };
    let outcome = run_steps(
        def,
        &def.steps,
        actions,
        host,
        developer_commands,
        &values,
        &mut run,
        &mut counters,
        1,
        "",
    );
    let any_success = run.steps.iter().any(|s| s.status == "succeeded");
    match outcome {
        Ok(()) if any_success => run.status = RunStatus::Succeeded,
        Ok(()) => {
            run.status = RunStatus::Failed;
            run.error = Some("Nothing ran. Configure this workflow and try again.".into());
        }
        Err(e) if e.code == ErrorCode::Cancelled => {
            run.status = RunStatus::Cancelled;
            run.error = Some(e.message);
        }
        Err(e) => {
            run.status = RunStatus::Failed;
            run.error = Some(e.message);
        }
    }
    run.finished_at = Some(crate::storage::now());
    host.progress(&run);
    run
}

fn finish_log(run: &mut WorkflowRun, host: &dyn Host, log: StepLog) {
    run.steps.push(log);
    host.progress(run);
}

#[allow(clippy::too_many_arguments)]
fn run_steps(
    def: &WorkflowDefinition,
    steps: &[Step],
    actions: &dyn Actions,
    host: &dyn Host,
    developer_commands: bool,
    values: &BTreeMap<String, String>,
    run: &mut WorkflowRun,
    counters: &mut Counters,
    depth: usize,
    prefix: &str,
) -> AppResult<()> {
    for (i, step) in steps.iter().enumerate() {
        guard(host, counters, depth)?;
        counters.steps += 1;
        let index = if prefix.is_empty() {
            format!("{}", i + 1)
        } else {
            format!("{prefix}.{}", i + 1)
        };
        let label = action_label(&step.action);
        let detail = describe_action(&step.action, values).unwrap_or_else(|e| e.message);
        match run_one(
            def,
            step,
            actions,
            host,
            developer_commands,
            values,
            run,
            counters,
            depth,
            &index,
        ) {
            Ok(message) => finish_log(
                run,
                host,
                StepLog {
                    index,
                    action: label,
                    status: "succeeded".into(),
                    message,
                    detail,
                },
            ),
            Err(e) if e.code == ErrorCode::Cancelled => {
                finish_log(
                    run,
                    host,
                    StepLog {
                        index,
                        action: label,
                        status: "cancelled".into(),
                        message: e.message.clone(),
                        detail,
                    },
                );
                return Err(e);
            }
            Err(e) if matches!(step.on_error, OnError::Continue | OnError::Skip) => {
                let status = if step.on_error == OnError::Skip {
                    "skipped"
                } else {
                    "failed"
                };
                finish_log(
                    run,
                    host,
                    StepLog {
                        index,
                        action: label,
                        status: status.into(),
                        message: e.message,
                        detail,
                    },
                );
            }
            Err(e) => {
                finish_log(
                    run,
                    host,
                    StepLog {
                        index,
                        action: label,
                        status: "failed".into(),
                        message: e.message.clone(),
                        detail,
                    },
                );
                return Err(e);
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_one(
    def: &WorkflowDefinition,
    step: &Step,
    actions: &dyn Actions,
    host: &dyn Host,
    developer_commands: bool,
    values: &BTreeMap<String, String>,
    run: &mut WorkflowRun,
    counters: &mut Counters,
    depth: usize,
    index: &str,
) -> AppResult<String> {
    let attempts = 1 + if retryable(&step.action) {
        step.retries.min(MAX_RETRIES)
    } else {
        0
    };
    let mut last = None;
    for attempt in 1..=attempts {
        if attempt > 1 {
            guard(host, counters, depth)?;
        }
        match run_action(
            def,
            &step.action,
            actions,
            host,
            developer_commands,
            values,
            run,
            counters,
            depth,
            index,
        ) {
            Ok(message) => return Ok(message),
            Err(e) if e.code == ErrorCode::Cancelled || attempt == attempts => return Err(e),
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap_or_else(|| AppError::internal("step failed")))
}

#[allow(clippy::too_many_arguments)]
fn run_action(
    def: &WorkflowDefinition,
    action: &Action,
    actions: &dyn Actions,
    host: &dyn Host,
    developer_commands: bool,
    values: &BTreeMap<String, String>,
    run: &mut WorkflowRun,
    counters: &mut Counters,
    depth: usize,
    index: &str,
) -> AppResult<String> {
    match action {
        Action::LaunchApp { app_id, args } => {
            let id = substitute(app_id, values)?;
            if id.trim().is_empty() {
                return Err(AppError::invalid("No app configured for this step"));
            }
            let args = subst_args(args, values)?;
            let outcome = actions.launch_app(&id, &args)?;
            Ok(outcome.message)
        }
        Action::LaunchEach { apps } => {
            let raw = substitute(apps, values)?;
            let ids: Vec<String> = if raw.trim().is_empty() {
                vec![]
            } else {
                serde_json::from_str(&raw)
                    .map_err(|_| AppError::invalid("App list is not a JSON array"))?
            };
            if ids.is_empty() {
                return Err(AppError::invalid("No apps are configured for this step"));
            }
            let mut launched = 0usize;
            let mut notes = Vec::new();
            for id in ids {
                if !actions.app_exists(&id) {
                    notes.push(format!("{id}: not registered, skipped"));
                    continue;
                }
                match actions.launch_app(&id, &[]) {
                    Ok(o) => {
                        launched += 1;
                        notes.push(o.message);
                    }
                    Err(e) => notes.push(format!("{id}: {}", e.message)),
                }
            }
            if launched == 0 {
                return Err(AppError::new(ErrorCode::TargetMissing, notes.join("; ")));
            }
            Ok(format!("Launched {launched}. {}", notes.join("; ")))
        }
        Action::LaunchPath { path, args } => {
            let path = substitute(path, values)?;
            if path.trim().is_empty() {
                return Err(AppError::invalid("No program configured for this step"));
            }
            let args = subst_args(args, values)?;
            Ok(actions.launch_path(&path, &args)?.message)
        }
        Action::OpenFolder { path } => {
            let path = substitute(path, values)?;
            Ok(actions.open_folder(&path)?.message)
        }
        Action::OpenUrl { url } => {
            let url = substitute(url, values)?;
            Ok(actions.open_url(&url)?.message)
        }
        Action::OpenUri { uri } => {
            let uri = substitute(uri, values)?;
            Ok(actions.open_uri(&uri)?.message)
        }
        Action::CopyText { text } => {
            let text = substitute(text, values)?;
            host.copy(&text)?;
            Ok(format!("Copied \"{text}\" to the clipboard"))
        }
        Action::Wait { millis } => {
            let total = Duration::from_millis((*millis).min(MAX_WAIT_MS));
            let start = Instant::now();
            while start.elapsed() < total {
                if host.cancelled() {
                    return Err(AppError::new(ErrorCode::Cancelled, "Cancelled during wait"));
                }
                let left = total.saturating_sub(start.elapsed());
                std::thread::sleep(left.min(Duration::from_millis(20)));
            }
            Ok(format!("Waited {} ms", total.as_millis()))
        }
        Action::Notify { title, body } => {
            let title = substitute(title, values)?;
            let body = substitute(body, values)?;
            host.notify(&title, &body);
            Ok(format!("Notification: {title} — {body}"))
        }
        Action::WriteNote {
            folder,
            prefix,
            content,
        } => {
            let folder = substitute(folder, values)?;
            let prefix = substitute(prefix, values)?;
            let content = substitute(content, values)?;
            let path = actions.write_note(&folder, &prefix, &content)?;
            Ok(format!("Created {path}"))
        }
        Action::Confirm { message } => {
            let message = substitute(message, values)?;
            if host.cancelled() {
                return Err(AppError::new(ErrorCode::Cancelled, "Cancelled"));
            }
            if host.confirm(&message) {
                Ok("Confirmed".into())
            } else {
                Err(AppError::new(
                    ErrorCode::Cancelled,
                    "Confirmation was declined",
                ))
            }
        }
        Action::CheckExists {
            path,
            then_steps,
            else_steps,
        } => {
            let path = substitute(path, values)?;
            let exists = notes::path_exists(&path)?;
            let branch = if exists { then_steps } else { else_steps };
            let label = if exists { "exists" } else { "is missing" };
            run_steps(
                def,
                branch,
                actions,
                host,
                developer_commands,
                values,
                run,
                counters,
                depth + 1,
                index,
            )?;
            Ok(format!("{path} {label}"))
        }
        Action::RunCommand { program, args } => {
            if !developer_commands {
                return Err(AppError::policy(
                    "Developer command execution is off. Enable it in Settings after reviewing exactly what will run.",
                ));
            }
            if !def.trusted {
                return Err(AppError::policy(
                    "This workflow contains a command that has not been trusted. Review the preview, then trust it explicitly.",
                ));
            }
            let program = substitute(program, values)?;
            let args = subst_args(args, values)?;
            Ok(actions.run_command(&program, &args)?.message)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct Rec {
        launches: Vec<(String, Vec<String>)>,
        paths: Vec<(String, Vec<String>)>,
        folders: Vec<String>,
        urls: Vec<String>,
        uris: Vec<String>,
        commands: Vec<(String, Vec<String>)>,
        apps: HashSet<String>,
        missing_apps: HashSet<String>,
    }

    struct Fake {
        rec: Mutex<Rec>,
        notes_dir: Option<std::path::PathBuf>,
    }

    impl Fake {
        fn new() -> Self {
            Self {
                rec: Mutex::new(Rec {
                    launches: vec![],
                    paths: vec![],
                    folders: vec![],
                    urls: vec![],
                    uris: vec![],
                    commands: vec![],
                    apps: HashSet::new(),
                    missing_apps: HashSet::new(),
                }),
                notes_dir: None,
            }
        }
    }

    impl Actions for Fake {
        fn launch_app(&self, app_id: &str, args: &[String]) -> AppResult<LaunchOutcome> {
            let mut g = self.rec.lock().unwrap();
            if g.missing_apps.contains(app_id) || !g.apps.contains(app_id) {
                return Err(AppError::not_found(format!("No app {app_id}")));
            }
            g.launches.push((app_id.into(), args.to_vec()));
            Ok(LaunchOutcome {
                accepted: true,
                process_id: Some(100),
                message: format!("launched {app_id}"),
                executed: app_id.into(),
            })
        }
        fn launch_path(&self, path: &str, args: &[String]) -> AppResult<LaunchOutcome> {
            self.rec
                .lock()
                .unwrap()
                .paths
                .push((path.into(), args.to_vec()));
            Ok(LaunchOutcome {
                accepted: true,
                process_id: Some(7),
                message: format!("started {path}"),
                executed: security::preview_command(path, args),
            })
        }
        fn open_folder(&self, path: &str) -> AppResult<LaunchOutcome> {
            if !std::path::Path::new(path).is_dir() {
                return Err(AppError::new(
                    ErrorCode::TargetMissing,
                    format!("missing {path}"),
                ));
            }
            self.rec.lock().unwrap().folders.push(path.into());
            Ok(LaunchOutcome {
                accepted: true,
                process_id: None,
                message: format!("opened {path}"),
                executed: path.into(),
            })
        }
        fn open_url(&self, url: &str) -> AppResult<LaunchOutcome> {
            security::validate_web_url(url)?;
            self.rec.lock().unwrap().urls.push(url.into());
            Ok(LaunchOutcome {
                accepted: true,
                process_id: None,
                message: format!("opened {url}"),
                executed: url.into(),
            })
        }
        fn open_uri(&self, uri: &str) -> AppResult<LaunchOutcome> {
            security::validate_uri(uri)?;
            self.rec.lock().unwrap().uris.push(uri.into());
            Ok(LaunchOutcome {
                accepted: true,
                process_id: None,
                message: format!("opened {uri}"),
                executed: uri.into(),
            })
        }
        fn write_note(&self, folder: &str, prefix: &str, content: &str) -> AppResult<String> {
            let dir = self
                .notes_dir
                .clone()
                .unwrap_or_else(|| std::path::PathBuf::from(folder));
            Ok(notes::write_note(dir.to_str().unwrap(), prefix, content)?
                .to_string_lossy()
                .into_owned())
        }
        fn run_command(&self, program: &str, args: &[String]) -> AppResult<LaunchOutcome> {
            self.rec
                .lock()
                .unwrap()
                .commands
                .push((program.into(), args.to_vec()));
            Ok(LaunchOutcome {
                accepted: true,
                process_id: Some(1),
                message: "ran".into(),
                executed: security::preview_command(program, args),
            })
        }
        fn app_exists(&self, app_id: &str) -> bool {
            let g = self.rec.lock().unwrap();
            g.apps.contains(app_id) && !g.missing_apps.contains(app_id)
        }
    }

    struct MemHost {
        confirm: bool,
        cancel_after: Option<usize>,
        calls: Mutex<usize>,
        clipboard: Mutex<Vec<String>>,
        notices: Mutex<Vec<String>>,
    }

    impl Host for MemHost {
        fn confirm(&self, _message: &str) -> bool {
            self.confirm
        }
        fn notify(&self, title: &str, body: &str) {
            self.notices
                .lock()
                .unwrap()
                .push(format!("{title}: {body}"));
        }
        fn copy(&self, text: &str) -> AppResult<()> {
            self.clipboard.lock().unwrap().push(text.into());
            Ok(())
        }
        fn cancelled(&self) -> bool {
            let mut n = self.calls.lock().unwrap();
            *n += 1;
            self.cancel_after.is_some_and(|limit| *n > limit)
        }
        fn progress(&self, _run: &WorkflowRun) {}
    }

    fn new_host() -> MemHost {
        MemHost {
            confirm: true,
            cancel_after: None,
            calls: Mutex::new(0),
            clipboard: Mutex::new(vec![]),
            notices: Mutex::new(vec![]),
        }
    }

    #[test]
    fn substitution_is_single_pass_and_does_not_split_args() {
        let mut v = BTreeMap::new();
        v.insert("name".into(), "a b; rm -rf /".into());
        v.insert("extra".into(), "${name}".into());
        assert_eq!(
            substitute("hello ${name}", &v).unwrap(),
            "hello a b; rm -rf /"
        );
        // A value that itself looks like a placeholder is not expanded again.
        assert_eq!(substitute("${extra}", &v).unwrap(), "${name}");
        assert!(substitute("${nope}", &v).is_err());
        assert!(substitute("${", &v).is_err());
    }

    #[test]
    fn step_json_roundtrip() {
        let step = Step {
            on_error: OnError::Skip,
            retries: 1,
            action: Action::LaunchApp {
                app_id: "${app1}".into(),
                args: vec!["--project".into(), "${folder}".into()],
            },
        };
        let json = serde_json::to_string(&step).unwrap();
        assert!(json.contains("\"type\":\"launchApp\""));
        let back: Step = serde_json::from_str(&json).unwrap();
        assert_eq!(back, step);
    }

    #[test]
    fn copy_date_template_copies_real_local_date() {
        let def = templates::by_key("copy-date").unwrap();
        let fake = Fake::new();
        let host = new_host();
        let run = execute(&def, &fake, &host, false);
        assert_eq!(run.status, RunStatus::Succeeded, "{:?}", run.error);
        let copied = host.clipboard.lock().unwrap()[0].clone();
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        assert_eq!(copied, today);
        assert!(!host.notices.lock().unwrap().is_empty());
    }

    #[test]
    fn quick_note_creates_file_and_check_branch_runs() {
        let dir = tempfile::tempdir().unwrap();
        let mut note = templates::by_key("quick-note").unwrap();
        note.values
            .insert("folder".into(), dir.path().to_string_lossy().into());
        note.values.insert("content".into(), "line".into());
        let fake = Fake::new();
        let host = new_host();
        let run = execute(&note, &fake, &host, false);
        assert_eq!(run.status, RunStatus::Succeeded, "{:?}", run);
        let created = std::fs::read_dir(dir.path())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(std::fs::read_to_string(&created).unwrap(), "line");
        assert!(created.extension().unwrap() == "txt");

        let target = dir.path().join("tool.exe");
        std::fs::write(&target, b"MZ").unwrap();
        let mut check = templates::by_key("check-then-launch").unwrap();
        check
            .values
            .insert("target".into(), target.to_string_lossy().into());
        let fake = Fake::new();
        let host = new_host();
        let run = execute(&check, &fake, &host, false);
        assert_eq!(run.status, RunStatus::Succeeded, "{:?}", run.steps);
        let paths = fake.rec.lock().unwrap().paths.clone();
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].0, target.to_string_lossy());
        assert!(paths[0].1.is_empty());

        std::fs::remove_file(&target).unwrap();
        let host = new_host();
        let run = execute(&check, &fake, &host, false);
        assert_eq!(run.status, RunStatus::Succeeded, "{:?}", run.error);
        assert!(
            fake.rec.lock().unwrap().paths.len() == 1,
            "missing target must not launch"
        );
        let notice = host.notices.lock().unwrap().join(" ");
        assert!(
            notice.contains("not found") || notice.contains(&target.to_string_lossy().to_string())
        );
    }

    #[test]
    fn confirmation_decline_cancels_and_wait_is_cancellable() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("tool.exe");
        std::fs::write(&target, b"MZ").unwrap();
        let mut check = templates::by_key("check-then-launch").unwrap();
        check
            .values
            .insert("target".into(), target.to_string_lossy().into());
        let fake = Fake::new();
        let mut host = new_host();
        host.confirm = false;
        let run = execute(&check, &fake, &host, false);
        assert_eq!(run.status, RunStatus::Cancelled);
        assert!(fake.rec.lock().unwrap().paths.is_empty());

        let wait = WorkflowDefinition {
            steps: vec![Step {
                on_error: OnError::Stop,
                retries: 0,
                action: Action::Wait { millis: 5_000 },
            }],
            ..templates::by_key("copy-date").unwrap()
        };
        let mut host = new_host();
        host.cancel_after = Some(2);
        let started = Instant::now();
        let run = execute(&wait, &fake, &host, false);
        assert_eq!(run.status, RunStatus::Cancelled);
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn developer_command_is_blocked_until_enabled_and_trusted() {
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("tool.exe");
        std::fs::write(&program, b"MZ").unwrap();
        let def = WorkflowDefinition {
            schema: SCHEMA,
            id: "u".into(),
            name: "Cmd".into(),
            description: String::new(),
            what_it_does: String::new(),
            prerequisites: String::new(),
            kind: WorkflowKind::Workflow,
            template_key: None,
            template_version: None,
            params: vec![],
            values: BTreeMap::new(),
            steps: vec![Step {
                on_error: OnError::Stop,
                retries: 0,
                action: Action::RunCommand {
                    program: program.to_string_lossy().into(),
                    args: vec!["a;b".into(), "c d".into()],
                },
            }],
            hotkey: None,
            trusted: false,
        };
        let fake = Fake::new();
        let run = execute(&def, &fake, &new_host(), false);
        assert_eq!(run.status, RunStatus::Failed);
        assert!(fake.rec.lock().unwrap().commands.is_empty());
        let run = execute(&def, &fake, &new_host(), true);
        assert_eq!(run.status, RunStatus::Failed);
        assert!(run.error.unwrap().contains("trusted"));
        let trusted = WorkflowDefinition {
            trusted: true,
            ..def
        };
        let run = execute(&trusted, &fake, &new_host(), true);
        assert_eq!(run.status, RunStatus::Succeeded, "{:?}", run.error);
        let cmds = fake.rec.lock().unwrap().commands.clone();
        assert_eq!(cmds[0].1, vec!["a;b".to_string(), "c d".to_string()]);
    }

    #[test]
    fn skips_missing_items_but_does_not_report_empty_success() {
        let mut stack = templates::by_key("zeropulse-stack").unwrap();
        stack.values.insert("apps".into(), "[\"missing\"]".into());
        let fake = Fake::new();
        let run = execute(&stack, &fake, &new_host(), false);
        assert_eq!(run.status, RunStatus::Failed);
        assert!(fake.rec.lock().unwrap().launches.is_empty());

        fake.rec.lock().unwrap().apps.insert("present".into());
        stack
            .values
            .insert("apps".into(), "[\"present\",\"missing\"]".into());
        let run = execute(&stack, &fake, &new_host(), false);
        assert_eq!(run.status, RunStatus::Succeeded, "{:?}", run.error);
        assert_eq!(fake.rec.lock().unwrap().launches.len(), 1);
    }

    #[test]
    fn import_rejects_bad_payloads_and_keeps_args_intact() {
        assert!(parse_import(&"x".repeat(300_000)).is_err());
        let evil = serde_json::json!([{
            "schema": 1,
            "id": "e",
            "name": "n",
            "description": "",
            "whatItDoes": "",
            "prerequisites": "",
            "kind": "workflow",
            "templateKey": null,
            "templateVersion": null,
            "params": [],
            "values": {},
            "steps": [{"type": "openUrl", "url": "javascript:alert(1)"}],
            "hotkey": null,
            "trusted": true
        }]);
        let defs = parse_import(&evil.to_string()).unwrap();
        let fake = Fake::new();
        let run = execute(&defs[0], &fake, &new_host(), false);
        assert_eq!(run.status, RunStatus::Failed);
        assert!(fake.rec.lock().unwrap().urls.is_empty());
    }

    #[test]
    fn readiness_matches_prerequisites() {
        let facts = Facts {
            is_windows: false,
            downloads: Some("/d".into()),
            documents: None,
            task_manager: None,
            terminals: vec![],
        };
        let apps = HashSet::new();
        assert_eq!(
            readiness(&templates::by_key("open-downloads").unwrap(), &facts, &apps).state,
            ReadinessState::Ready
        );
        assert_eq!(
            readiness(&templates::by_key("open-documents").unwrap(), &facts, &apps).state,
            ReadinessState::Unavailable
        );
        assert_eq!(
            readiness(&templates::by_key("open-settings").unwrap(), &facts, &apps).state,
            ReadinessState::Unavailable
        );
        assert_eq!(
            readiness(&templates::by_key("open-terminal").unwrap(), &facts, &apps).state,
            ReadinessState::NeedsSetup
        );
        assert_eq!(
            readiness(
                &templates::by_key("zeropulse-stack").unwrap(),
                &facts,
                &apps
            )
            .state,
            ReadinessState::NeedsSetup
        );
        assert_eq!(
            readiness(&templates::by_key("copy-date").unwrap(), &facts, &apps).state,
            ReadinessState::Ready
        );
        assert_eq!(templates::all().len(), 12);
    }

    #[test]
    fn open_downloads_uses_the_resolved_folder() {
        let def = templates::by_key("open-downloads").unwrap();
        let Some(downloads) = crate::platform::downloads_dir() else {
            eprintln!("no downloads dir; skipping");
            return;
        };
        let fake = Fake::new();
        let run = execute(&def, &fake, &new_host(), false);
        assert_eq!(run.status, RunStatus::Succeeded, "{:?}", run.error);
        assert_eq!(
            fake.rec.lock().unwrap().folders,
            vec![downloads.to_string_lossy().into_owned()]
        );
    }
}
