//! Engine integration tests with a controlled fake host (no personal folders or apps touched).

use pulse_core::error::{CoreError, Result};
use pulse_core::models::*;
use pulse_core::workflow::engine::*;
use pulse_core::workflow::model::*;
use pulse_core::workflow::templates;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Mutex;
use std::time::Duration;

struct Fake {
    calls: Mutex<Vec<String>>,
    apps: HashMap<String, String>,
    collection: Vec<(String, String)>,
    existing: HashSet<String>,
    confirm: bool,
    dev_cmd: bool,
    clipboard: Mutex<Option<String>>,
    folders: HashMap<String, String>,
    windows: bool,
    fail_launch: bool,
}

impl Fake {
    fn new() -> Self {
        Self {
            calls: Mutex::new(vec![]),
            apps: HashMap::new(),
            collection: vec![],
            existing: HashSet::new(),
            confirm: true,
            dev_cmd: false,
            clipboard: Mutex::new(None),
            folders: HashMap::new(),
            windows: true,
            fail_launch: false,
        }
    }
    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
    fn log(&self, s: String) {
        self.calls.lock().unwrap().push(s);
    }
}

fn accepted(msg: &str) -> LaunchResult {
    LaunchResult { outcome: LaunchOutcomeKind::Accepted, pid: Some(1), verified_running: None, message: msg.into(), command_preview: String::new() }
}

impl WorkflowHost for Fake {
    fn launch_app(&self, id: &str, args: &[String]) -> Result<LaunchResult> {
        self.log(format!("launch_app {id} {args:?}"));
        if self.fail_launch {
            return Ok(LaunchResult { outcome: LaunchOutcomeKind::Failed, pid: None, verified_running: None, message: "boom".into(), command_preview: String::new() });
        }
        Ok(accepted("ok"))
    }
    fn launch_path(&self, p: &str, args: &[String], _wd: Option<&str>) -> Result<LaunchResult> {
        self.log(format!("launch_path {p} {args:?}"));
        Ok(accepted("ok"))
    }
    fn open_path(&self, p: &str) -> Result<()> {
        self.log(format!("open_path {p}"));
        Ok(())
    }
    fn open_url(&self, u: &str) -> Result<()> {
        self.log(format!("open_url {u}"));
        Ok(())
    }
    fn open_settings_uri(&self, u: &str) -> Result<()> {
        self.log(format!("settings {u}"));
        Ok(())
    }
    fn copy_text(&self, t: &str) -> Result<()> {
        *self.clipboard.lock().unwrap() = Some(t.to_string());
        Ok(())
    }
    fn notify(&self, t: &str, b: &str) -> Result<()> {
        self.log(format!("notify {t}|{b}"));
        Ok(())
    }
    fn confirm(&self, m: &str, _t: Duration, _c: &CancelToken) -> Result<bool> {
        self.log(format!("confirm {m}"));
        Ok(self.confirm)
    }
    fn app_exists(&self, id: &str) -> bool {
        self.apps.contains_key(id)
    }
    fn collection_apps(&self, _k: &str) -> Result<Vec<(String, String)>> {
        Ok(self.collection.clone())
    }
    fn developer_commands_enabled(&self) -> bool {
        self.dev_cmd
    }
    fn is_windows(&self) -> bool {
        self.windows
    }
    fn now(&self) -> chrono::DateTime<chrono::Local> {
        use chrono::TimeZone;
        chrono::Local.with_ymd_and_hms(2026, 10, 12, 14, 5, 9).unwrap()
    }
    fn known_folder(&self, n: &str) -> Option<String> {
        self.folders.get(n).cloned()
    }
    fn path_exists(&self, p: &str) -> bool {
        self.existing.contains(p) || std::path::Path::new(p).exists()
    }
}

fn run(def: &WorkflowDefinition, vals: &[(&str, &str)], host: &Fake) -> WorkflowRun {
    let o: BTreeMap<String, String> = vals.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    run_workflow(def, &o, host, &CancelToken::new())
}

fn tpl(id: &str) -> WorkflowDefinition {
    templates::template(id).unwrap()
}

fn statuses(r: &WorkflowRun) -> Vec<StepStatus> {
    r.steps.iter().map(|s| s.status).collect()
}

#[test]
fn t01_open_downloads_uses_known_folder() {
    let mut h = Fake::new();
    let d = tempfile::tempdir().unwrap();
    h.folders.insert("downloads".into(), d.path().to_string_lossy().into());
    let r = run(&tpl("tpl.open_downloads"), &[], &h);
    assert_eq!(r.status, RunStatus::Succeeded, "{r:?}");
    assert_eq!(h.calls(), vec![format!("open_path {}", d.path().display())]);
}

#[test]
fn t01_missing_known_folder_fails_honestly() {
    let h = Fake::new();
    let r = run(&tpl("tpl.open_downloads"), &[], &h);
    assert_eq!(r.status, RunStatus::Failed);
    assert!(r.steps[0].message.contains("known folder"));
    assert!(h.calls().is_empty());
}

#[test]
fn t02_open_documents() {
    let mut h = Fake::new();
    let d = tempfile::tempdir().unwrap();
    h.folders.insert("documents".into(), d.path().to_string_lossy().into());
    let r = run(&tpl("tpl.open_documents"), &[], &h);
    assert_eq!(r.status, RunStatus::Succeeded);
    assert!(h.calls()[0].starts_with("open_path "));
}

#[test]
fn t03_settings_uri() {
    let h = Fake::new();
    let r = run(&tpl("tpl.open_settings"), &[], &h);
    assert_eq!(r.status, RunStatus::Succeeded);
    assert_eq!(h.calls(), vec!["settings ms-settings:"]);
}

#[test]
fn t04_task_manager_needs_setup_when_absent_and_runs_when_present() {
    let def = tpl("tpl.open_task_manager");
    let mut h = Fake::new();
    std::env::set_var("SystemRoot", "/fake/windows");
    let vals = effective_values(&def, &BTreeMap::new());
    let pre = check_prerequisites(&def, &vals, &h);
    assert_eq!(readiness(&pre), ReadyState::NeedsSetup, "{pre:?}");
    h.existing.insert("/fake/windows\\System32\\Taskmgr.exe".into());
    assert_eq!(readiness(&check_prerequisites(&def, &vals, &h)), ReadyState::Ready);
    h.windows = false;
    assert_eq!(readiness(&check_prerequisites(&def, &vals, &h)), ReadyState::NeedsSetup);
}

#[test]
fn t05_terminal_requires_choice_or_detection() {
    let def = tpl("tpl.open_terminal");
    let h = Fake::new();
    std::env::set_var("LOCALAPPDATA", "/nonexistent-local");
    let r = run(&def, &[], &h);
    assert_eq!(r.status, RunStatus::Failed, "no silent assumption of a terminal");
    assert!(r.steps.iter().any(|s| s.message.contains("No terminal")));
    assert!(h.calls().is_empty());

    let d = tempfile::tempdir().unwrap();
    let term = d.path().join("term.exe");
    std::fs::write(&term, b"").unwrap();
    let h = Fake::new();
    let r = run(&def, &[("terminal", term.to_str().unwrap())], &h);
    assert_eq!(r.status, RunStatus::Succeeded, "{r:?}");
    assert!(h.calls()[0].starts_with("launch_path "));

    let root = tempfile::tempdir().unwrap();
    let local = root.path().to_string_lossy().to_string();
    std::env::set_var("LOCALAPPDATA", &local);
    // The template uses Windows separators. On Windows that is a real nested path; elsewhere the
    // backslashes are literal, so the fixture is created at the exact expanded location.
    let wt = format!("{local}\\Microsoft\\WindowsApps\\wt.exe");
    if cfg!(windows) {
        let p = std::path::Path::new(&wt);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"MZ").unwrap();
    } else {
        std::fs::write(&wt, b"MZ").unwrap();
    }
    let h = Fake::new();
    let r = run(&def, &[], &h);
    let _ = std::fs::remove_file(&wt);
    assert_eq!(r.status, RunStatus::Succeeded, "{r:?}");
    assert!(h.calls()[0].contains("wt.exe"));
}

#[test]
fn t06_project_folder_validates_and_opens() {
    let def = tpl("tpl.open_project_folder");
    let h = Fake::new();
    let r = run(&def, &[], &h);
    assert_eq!(r.status, RunStatus::Failed);
    assert!(r.steps[0].message.contains("required"));
    let r = run(&def, &[("folder", "/definitely/not/here")], &h);
    assert_eq!(r.status, RunStatus::Failed);
    let d = tempfile::tempdir().unwrap();
    let r = run(&def, &[("folder", d.path().to_str().unwrap())], &h);
    assert_eq!(r.status, RunStatus::Succeeded);
}

#[test]
fn t07_dev_workspace_sequential_with_skip_and_stop_policies() {
    let def = tpl("tpl.dev_workspace");
    let mut h = Fake::new();
    h.apps.insert("a1".into(), "Editor".into());
    let d = tempfile::tempdir().unwrap();
    let gone = d.path().join("gone");
    let items = [("item1", "app:a1"), ("item2", gone.to_str().unwrap()), ("item3", d.path().to_str().unwrap())];
    let r = run(&def, &items, &h);
    assert_eq!(r.status, RunStatus::Failed, "a skipped failure still marks the run failed: {r:?}");
    let opens: Vec<_> = r.steps.iter().filter(|s| s.step_id.starts_with("open")).map(|s| (s.step_id.as_str(), s.status)).collect();
    assert_eq!(
        opens,
        vec![("open1", StepStatus::Succeeded), ("open2", StepStatus::Failed), ("open3", StepStatus::Succeeded), ("open4", StepStatus::Skipped)]
    );
    assert_eq!(h.calls().len(), 2, "app launched, folder opened, missing one skipped");

    let h2 = {
        let mut h = Fake::new();
        h.apps.insert("a1".into(), "Editor".into());
        h
    };
    let r = run(&def, &[("item1", gone.to_str().unwrap()), ("item2", "app:a1"), ("on_missing", "stop")], &h2);
    assert_eq!(r.status, RunStatus::Failed);
    assert!(h2.calls().is_empty(), "stop policy must not continue to item 2");

    let r = run(&def, &[("item1", "app:a1")], &h2);
    assert_eq!(r.status, RunStatus::Failed);
    assert!(r.steps[0].message.contains("Item 2"), "{r:?}");
}

#[test]
fn t08_copy_date_formats() {
    let def = tpl("tpl.copy_date");
    let h = Fake::new();
    let r = run(&def, &[], &h);
    assert_eq!(r.status, RunStatus::Succeeded, "{r:?}");
    assert_eq!(h.clipboard.lock().unwrap().as_deref(), Some("2026-10-12"));
    let r = run(&def, &[("format", "%A, %B %-d, %Y")], &h);
    assert_eq!(r.status, RunStatus::Succeeded);
    assert_eq!(h.clipboard.lock().unwrap().as_deref(), Some("Monday, October 12, 2026"));
    let r = run(&def, &[("format", "%Q-evil")], &h);
    assert_eq!(r.status, RunStatus::Failed, "format outside the choice list is rejected");
}

#[test]
fn t09_quick_note_creates_unique_files_and_never_overwrites() {
    let def = tpl("tpl.quick_note");
    let h = Fake::new();
    let d = tempfile::tempdir().unwrap();
    let dir = d.path().to_str().unwrap();
    let r = run(&def, &[("folder", dir), ("text", "hello ü")], &h);
    assert_eq!(r.status, RunStatus::Succeeded, "{r:?}");
    let first = d.path().join("note-20261012-140509.txt");
    assert_eq!(std::fs::read_to_string(&first).unwrap(), "hello ü\n");
    let r = run(&def, &[("folder", dir), ("text", "second")], &h);
    assert_eq!(r.status, RunStatus::Succeeded);
    assert_eq!(std::fs::read_to_string(&first).unwrap(), "hello ü\n", "first note must be untouched");
    assert!(d.path().join("note-20261012-140509-1.txt").exists());
    assert_eq!(std::fs::read_dir(d.path()).unwrap().count(), 2);
}

#[test]
fn t09b_note_path_traversal_is_blocked() {
    let mut def = tpl("tpl.quick_note");
    def.steps = vec![Step::new("n", "n", Action::CreateNote { dir: "{{folder}}".into(), file_name: "{{text}}".into(), content: "x".into() })];
    let h = Fake::new();
    let d = tempfile::tempdir().unwrap();
    let r = run(&def, &[("folder", d.path().to_str().unwrap()), ("text", "../escape.txt")], &h);
    assert_eq!(r.status, RunStatus::Failed);
    assert!(!d.path().parent().unwrap().join("escape.txt").exists());
}

#[test]
fn t10_website_only_http_s() {
    let def = tpl("tpl.open_website");
    let h = Fake::new();
    assert_eq!(run(&def, &[("url", "https://example.com/x")], &h).status, RunStatus::Succeeded);
    assert_eq!(h.calls(), vec!["open_url https://example.com/x"]);
    for bad in ["javascript:alert(1)", "file:///c:/windows/system32/cmd.exe", "ftp://x.com", ""] {
        let r = run(&def, &[("url", bad)], &h);
        assert_eq!(r.status, RunStatus::Failed, "{bad}");
    }
    assert_eq!(h.calls().len(), 1);
}

#[test]
fn t11_zeropulse_stack_empty_is_not_a_fake_success() {
    let def = tpl("tpl.zeropulse_stack");
    let mut h = Fake::new();
    let vals = effective_values(&def, &BTreeMap::new());
    assert_eq!(readiness(&check_prerequisites(&def, &vals, &h)), ReadyState::NeedsSetup);
    let r = run(&def, &[], &h);
    assert_eq!(r.status, RunStatus::Failed);
    assert!(r.steps[0].message.contains("no registered apps"));

    h.collection = vec![("a".into(), "PulseCut".into()), ("b".into(), "PulseFlow".into())];
    assert_eq!(readiness(&check_prerequisites(&def, &vals, &h)), ReadyState::Ready);
    let r = run(&def, &[], &h);
    assert_eq!(r.status, RunStatus::Succeeded);
    assert_eq!(h.calls().len(), 2);
    assert!(r.steps[0].output.as_ref().unwrap().contains("PulseFlow"));

    h.fail_launch = true;
    assert_eq!(run(&def, &[], &h).status, RunStatus::Failed);
}

#[test]
fn t12_check_then_launch() {
    let def = tpl("tpl.check_then_launch");
    let h = Fake::new();
    let d = tempfile::tempdir().unwrap();
    let exe = d.path().join("tool.exe");
    std::fs::write(&exe, b"").unwrap();
    let r = run(&def, &[("target", exe.to_str().unwrap()), ("arguments", "--x y")], &h);
    assert_eq!(r.status, RunStatus::Succeeded, "{r:?}");
    assert!(h.calls()[0].contains("--x y"));
    let missing = d.path().join("nope.exe");
    let r = run(&def, &[("target", missing.to_str().unwrap())], &h);
    assert_eq!(r.status, RunStatus::Failed);
    assert!(r.steps.iter().any(|s| s.message.contains("Target not found") && s.message.contains("nope.exe")), "{r:?}");
    assert_eq!(h.calls().len(), 1);
}

#[test]
fn user_input_cannot_inject_templates() {
    let mut def = tpl("tpl.copy_date");
    def.params.push(ParamDef { name: "text".into(), label: "t".into(), kind: ParamKind::Text, required: false, default: None, choices: vec![], help: None });
    def.steps = vec![Step::new("c", "c", Action::CopyText { text: "{{text}}".into() })];
    let h = Fake::new();
    let r = run(&def, &[("text", "{{env:USERPROFILE}} {{now:%Y}}")], &h);
    assert_eq!(r.status, RunStatus::Succeeded);
    assert_eq!(h.clipboard.lock().unwrap().as_deref(), Some("{{env:USERPROFILE}} {{now:%Y}}"));
    def.steps = vec![Step::new("c", "c", Action::CopyText { text: "{{env:AWS_SECRET_ACCESS_KEY}}".into() })];
    assert_eq!(run(&def, &[], &h).status, RunStatus::Failed, "env allow-list");
    def.steps = vec![Step::new("c", "c", Action::CopyText { text: "{{undefined_var}}".into() })];
    assert_eq!(run(&def, &[], &h).status, RunStatus::Failed);
}

#[test]
fn cancellation_during_wait() {
    let mut def = tpl("tpl.open_website");
    def.params.clear();
    def.prerequisites.clear();
    def.kind = WorkflowKind::Workflow;
    def.steps = vec![Step::new("w", "Wait", Action::Wait { ms: 10_000 }), Step::new("n", "after", Action::Notify { title: "t".into(), body: "b".into() })];
    let h = Fake::new();
    let tok = CancelToken::new();
    let t2 = tok.clone();
    let th = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        t2.cancel();
    });
    let start = std::time::Instant::now();
    let r = run_workflow(&def, &BTreeMap::new(), &h, &tok);
    th.join().unwrap();
    assert!(start.elapsed() < Duration::from_secs(3));
    assert_eq!(r.status, RunStatus::Cancelled);
    assert_eq!(statuses(&r), vec![StepStatus::Cancelled]);
    assert!(h.calls().is_empty(), "later steps must not run");
}

#[test]
fn confirm_declined_cancels_run() {
    let mut def = tpl("tpl.open_website");
    def.params.clear();
    def.prerequisites.clear();
    def.kind = WorkflowKind::Workflow;
    def.steps = vec![Step::new("c", "Confirm", Action::Confirm { message: "Sure?".into() }), Step::new("n", "after", Action::Notify { title: "t".into(), body: "b".into() })];
    let mut h = Fake::new();
    h.confirm = false;
    let r = run(&def, &[], &h);
    assert_eq!(r.status, RunStatus::Cancelled);
    assert!(r.steps[0].message.contains("declined"));
    h.confirm = true;
    assert_eq!(run(&def, &[], &h).status, RunStatus::Succeeded);
}

#[test]
fn retries_only_for_safe_actions() {
    let mut def = tpl("tpl.open_website");
    def.params.clear();
    def.prerequisites.clear();
    def.steps = vec![Step { retries: 2, ..Step::new("o", "o", Action::OpenUrl { url: "https://x.com".into() }) }];
    assert!(validate_definition(&def).is_err());
    def.steps = vec![Step { retries: 2, ..Step::new("o", "o", Action::CopyText { text: "x".into() }) }];
    assert!(validate_definition(&def).is_ok());
}

#[test]
fn definition_limits() {
    let mut def = tpl("tpl.open_website");
    def.params.clear();
    def.prerequisites.clear();
    def.kind = WorkflowKind::Workflow;
    def.steps = (0..70).map(|i| Step::new(&format!("s{i}"), "w", Action::Wait { ms: 1 })).collect();
    assert!(validate_definition(&def).is_err(), "too many steps");
    def.steps = vec![Step::new("a", "w", Action::Wait { ms: 400_000 })];
    assert!(validate_definition(&def).is_err(), "wait too long");
    def.steps = vec![Step::new("a", "w", Action::Wait { ms: 1 }), Step::new("a", "w", Action::Wait { ms: 1 })];
    assert!(validate_definition(&def).is_err(), "duplicate ids");
    let mut nest = Step::new("deep", "d", Action::Wait { ms: 1 });
    for i in 0..6 {
        nest = Step::new(&format!("if{i}"), "if", Action::If { condition: Condition::NotEmpty { value: "x".into() }, then_steps: vec![nest], else_steps: vec![] });
    }
    def.steps = vec![nest];
    assert!(validate_definition(&def).is_err(), "nesting depth");
}

#[test]
fn run_command_gated_confirmed_and_redacted() {
    let d = tempfile::tempdir().unwrap();
    let app = env!("CARGO_BIN_EXE_pulse-testapp");
    let marker = d.path().join("m.txt");
    let mut def = tpl("tpl.open_website");
    def.params.clear();
    def.prerequisites.clear();
    def.steps = vec![Step::new("r", "Run", Action::RunCommand { program: app.into(), args: vec![marker.to_string_lossy().into(), "a b".into(), "password=hunter2".into()], working_dir: None, trusted: false })];
    let mut h = Fake::new();
    let r = run(&def, &[], &h);
    assert_eq!(r.status, RunStatus::Failed);
    assert!(r.steps[0].message.contains("turned off"));
    assert!(!marker.exists(), "nothing may run when the feature is off");

    h.dev_cmd = true;
    h.confirm = false;
    assert_eq!(run(&def, &[], &h).status, RunStatus::Cancelled);
    assert!(!marker.exists());

    h.confirm = true;
    let r = run(&def, &[], &h);
    assert_eq!(r.status, RunStatus::Succeeded, "{r:?}");
    let text = std::fs::read_to_string(&marker).unwrap();
    assert!(text.contains("\"a b\""), "args passed as array: {text}");
    assert!(h.calls().iter().any(|c| c.starts_with("confirm Run this command?")));
    assert!(!r.steps[0].preview.is_empty());
}

#[test]
fn run_command_timeout_kills_process() {
    let d = tempfile::tempdir().unwrap();
    let app = env!("CARGO_BIN_EXE_pulse-testapp");
    let marker = d.path().join("m.txt");
    let mut def = tpl("tpl.open_website");
    def.params.clear();
    def.prerequisites.clear();
    let mut step = Step::new("r", "Run", Action::RunCommand { program: app.into(), args: vec![marker.to_string_lossy().into(), "--sleep-ms".into(), "30000".into()], working_dir: None, trusted: true });
    step.timeout_ms = Some(300);
    def.steps = vec![step];
    let mut h = Fake::new();
    h.dev_cmd = true;
    let start = std::time::Instant::now();
    let r = run(&def, &[], &h);
    assert!(start.elapsed() < Duration::from_secs(5));
    assert_eq!(r.status, RunStatus::Failed);
    assert!(r.steps[0].message.contains("timed out"));
}

#[test]
fn run_command_cancel_kills_process() {
    let d = tempfile::tempdir().unwrap();
    let app = env!("CARGO_BIN_EXE_pulse-testapp");
    let marker = d.path().join("m.txt");
    let mut def = tpl("tpl.open_website");
    def.params.clear();
    def.prerequisites.clear();
    def.steps = vec![Step::new("r", "Run", Action::RunCommand { program: app.into(), args: vec![marker.to_string_lossy().into(), "--sleep-ms".into(), "30000".into()], working_dir: None, trusted: true })];
    let mut h = Fake::new();
    h.dev_cmd = true;
    let tok = CancelToken::new();
    let t2 = tok.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(250));
        t2.cancel();
    });
    let start = std::time::Instant::now();
    let r = run_workflow(&def, &BTreeMap::new(), &h, &tok);
    assert!(start.elapsed() < Duration::from_secs(5));
    assert_eq!(r.status, RunStatus::Cancelled);
}

#[test]
fn step_results_are_redacted() {
    let mut def = tpl("tpl.open_website");
    def.params.clear();
    def.prerequisites.clear();
    def.steps = vec![Step::new("c", "copy", Action::CopyText { text: "token=abcd1234 and ghp_abcdefghijklmnopqrstuvwxyz0123456789".into() })];
    let h = Fake::new();
    let r = run(&def, &[], &h);
    let joined = format!("{} {}", r.steps[0].message, r.steps[0].preview);
    assert!(!joined.contains("abcd1234") && !joined.contains("ghp_abc"), "{joined}");
}

#[test]
fn disabled_workflow_does_not_run() {
    let mut def = tpl("tpl.open_settings");
    def.enabled = false;
    let h = Fake::new();
    let r = run(&def, &[], &h);
    assert_eq!(r.status, RunStatus::Failed);
    assert!(h.calls().is_empty());
}

#[test]
fn errors_are_structured() {
    let e = CoreError::NotFound("x".into());
    assert_eq!(serde_json::to_value(&e).unwrap()["code"], "not_found");
}
