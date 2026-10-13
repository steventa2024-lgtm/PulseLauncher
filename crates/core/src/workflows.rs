use std::collections::{BTreeMap, HashMap};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use regex::Regex;

use crate::error::{AppError, AppResult};
use crate::launch::{launch_app, Host};
use crate::models::{
    now_iso, today_friendly, today_iso, AppEntry, ErrorPolicy, FolderKind, ParamKind, ParameterDef,
    Prerequisite, RunReport, Step, StepLog, StepPreview, TemplateStatus, TrustedTool,
    WorkflowDocument, WorkflowRecord, TEMPLATE_VERSION, ZEROPULSE_COLLECTION_ID,
};
use crate::platform::{detect_terminal, platform_name, task_manager_path};
use crate::security::{
    bounded_text, redact, validate_arguments, validate_existing_directory, validate_url,
};

pub struct EngineContext<'a> {
    pub apps: &'a HashMap<String, AppEntry>,
    pub collections: &'a HashMap<String, Vec<AppEntry>>,
    pub developer_commands: bool,
}

pub trait Confirmer {
    fn confirm(&mut self, message: &str) -> AppResult<bool>;
}

pub struct AutoConfirm(pub bool);

impl Confirmer for AutoConfirm {
    fn confirm(&mut self, _message: &str) -> AppResult<bool> {
        Ok(self.0)
    }
}

pub fn builtin_templates() -> Vec<WorkflowRecord> {
    vec![
        template(
            "tpl-open-downloads",
            "open-downloads",
            "Open Downloads",
            "Opens the real Downloads folder.",
            "Resolves the Downloads known folder and opens it in the file manager.",
            "The Downloads folder must exist for this account.",
            vec![],
            BTreeMap::new(),
            vec![Step::OpenKnownFolder {
                id: "open".into(),
                folder: FolderKind::Downloads,
                on_error: ErrorPolicy::Stop,
            }],
        ),
        template(
            "tpl-open-documents",
            "open-documents",
            "Open Documents",
            "Opens the real Documents folder.",
            "Resolves the Documents known folder and opens it in the file manager.",
            "The Documents folder must exist for this account.",
            vec![],
            BTreeMap::new(),
            vec![Step::OpenKnownFolder {
                id: "open".into(),
                folder: FolderKind::Documents,
                on_error: ErrorPolicy::Stop,
            }],
        ),
        template(
            "tpl-open-settings",
            "open-settings",
            "Open Windows Settings",
            "Opens the Windows Settings app.",
            "Asks Windows to open the ms-settings: link.",
            "Windows is required. This template is unavailable on other systems.",
            vec![],
            BTreeMap::new(),
            vec![Step::OpenUrl {
                id: "open".into(),
                url: "ms-settings:".into(),
                on_error: ErrorPolicy::Stop,
            }],
        ),
        template(
            "tpl-open-task-manager",
            "open-task-manager",
            "Open Task Manager",
            "Launches the Windows Task Manager executable.",
            "Launches the detected Taskmgr.exe. It does not search the disk.",
            "Windows must provide System32\\Taskmgr.exe.",
            vec![],
            BTreeMap::new(),
            vec![Step::LaunchTrustedTool {
                id: "launch".into(),
                tool: TrustedTool::TaskManager,
                on_error: ErrorPolicy::Stop,
            }],
        ),
        template(
            "tpl-open-terminal",
            "open-terminal",
            "Open Terminal",
            "Opens a terminal you select, or a detected one.",
            "Launches the configured terminal executable. Windows Terminal is used only when its path is detected or you choose it.",
            "Choose a terminal executable, or install one PulseLauncher can detect.",
            vec![param("terminalPath", "Terminal executable", ParamKind::Path, false, "Leave blank to use a detected terminal.")],
            BTreeMap::new(),
            vec![Step::LaunchTrustedTool {
                id: "launch".into(),
                tool: TrustedTool::Terminal,
                on_error: ErrorPolicy::Stop,
            }],
        ),
        template(
            "tpl-open-project-folder",
            "open-project-folder",
            "Open Project Folder",
            "Opens a project folder you choose.",
            "Opens the saved folder through the file manager.",
            "Choose an existing folder.",
            vec![param("projectPath", "Project folder", ParamKind::Path, true, "Absolute path to an existing folder.")],
            BTreeMap::new(),
            vec![Step::OpenFolder {
                id: "open".into(),
                path: "{{projectPath}}".into(),
                skip_if_blank: false,
                on_error: ErrorPolicy::Stop,
            }],
        ),
        template(
            "tpl-dev-workspace",
            "dev-workspace",
            "Launch Development Workspace",
            "Starts two registered apps and an optional folder.",
            "Launches each configured app in order and records the per-step result.",
            "Register at least two real apps, then select them here.",
            vec![
                param("appId1", "First app", ParamKind::AppId, true, "A registered app id."),
                param("appId2", "Second app", ParamKind::AppId, true, "A second registered app id."),
                param("folder", "Folder", ParamKind::Path, false, "Optional folder to open after the apps."),
            ],
            BTreeMap::new(),
            vec![
                Step::LaunchApp {
                    id: "app1".into(),
                    app_id: "{{appId1}}".into(),
                    args: Vec::new(),
                    on_error: ErrorPolicy::Stop,
                },
                Step::LaunchApp {
                    id: "app2".into(),
                    app_id: "{{appId2}}".into(),
                    args: Vec::new(),
                    on_error: ErrorPolicy::Skip,
                },
                Step::OpenFolder {
                    id: "folder".into(),
                    path: "{{folder}}".into(),
                    skip_if_blank: true,
                    on_error: ErrorPolicy::Skip,
                },
            ],
        ),
        template(
            "tpl-copy-date",
            "copy-date",
            "Copy Today's Date",
            "Copies the local date to the clipboard.",
            "Formats today's date and copies it. Nothing is uploaded.",
            "The clipboard must be available when you run this on the desktop.",
            vec![param_enum(
                "dateFormat",
                "Date format",
                vec!["iso".into(), "friendly".into()],
                "ISO is YYYY-MM-DD.",
            )],
            BTreeMap::from([("dateFormat".into(), "iso".into())]),
            vec![Step::CopyText {
                id: "copy".into(),
                text: "{{date}}".into(),
                on_error: ErrorPolicy::Stop,
            }],
        ),
        template(
            "tpl-quick-note",
            "quick-note",
            "Timestamped Quick Note",
            "Creates a new timestamped text file.",
            "Writes a unique .txt file in a folder you choose. It never overwrites an existing file.",
            "Choose a writable folder.",
            vec![
                param("directory", "Folder", ParamKind::Path, true, "Existing folder for the note."),
                param("body", "Note text", ParamKind::Text, false, "Optional text. The file is still created if this is empty."),
            ],
            BTreeMap::new(),
            vec![Step::CreateNote {
                id: "note".into(),
                directory: "{{directory}}".into(),
                body: "{{body}}".into(),
                on_error: ErrorPolicy::Stop,
            }],
        ),
        template(
            "tpl-open-website",
            "open-website",
            "Open Project Website",
            "Opens an http(s) URL in the default browser.",
            "Validates the URL and asks the OS to open it.",
            "Enter a trusted http or https URL.",
            vec![param("url", "Website", ParamKind::Url, true, "https://example.com")],
            BTreeMap::new(),
            vec![Step::OpenUrl {
                id: "open".into(),
                url: "{{url}}".into(),
                on_error: ErrorPolicy::Stop,
            }],
        ),
        template(
            "tpl-zeropulse-stack",
            "zeropulse-stack",
            "Launch ZeroPulse Stack",
            "Launches apps in the ZeroPulse collection.",
            "Launches each real app currently in the ZeroPulse collection. Empty until you add some.",
            "Register PulseCut, PulseFlow, OpenPulse, or any other real app and add it to the collection.",
            vec![],
            BTreeMap::new(),
            vec![Step::LaunchCollection {
                id: "stack".into(),
                collection_id: ZEROPULSE_COLLECTION_ID.into(),
                skip_missing: true,
                on_error: ErrorPolicy::Stop,
            }],
        ),
        template(
            "tpl-check-then-launch",
            "check-then-launch",
            "Check Then Launch",
            "Checks a path, then launches or explains that it is missing.",
            "If the path exists, launches the selected app. Otherwise it shows a precise missing-target notification.",
            "Choose a path to test and a registered app to launch when the path exists.",
            vec![
                param("target", "Path to check", ParamKind::Path, true, "Absolute file or folder path."),
                param("appId", "App to launch", ParamKind::AppId, true, "Registered app id."),
            ],
            BTreeMap::new(),
            vec![
                Step::CheckExists {
                    id: "check".into(),
                    path: "{{target}}".into(),
                    then_id: "launch".into(),
                    else_id: "missing".into(),
                },
                Step::LaunchApp {
                    id: "launch".into(),
                    app_id: "{{appId}}".into(),
                    args: Vec::new(),
                    on_error: ErrorPolicy::Stop,
                },
                Step::Notify {
                    id: "missing".into(),
                    title: "Target missing".into(),
                    body: "{{target}} does not exist.".into(),
                    on_error: ErrorPolicy::Stop,
                },
            ],
        ),
    ]
}

pub fn assess(
    record: &WorkflowRecord,
    host: &dyn Host,
    apps: &HashMap<String, AppEntry>,
    collection_counts: &HashMap<String, usize>,
) -> Prerequisite {
    if let Some(message) = missing_required(record) {
        return Prerequisite {
            status: TemplateStatus::NeedsSetup,
            message,
        };
    }
    match record.template_key.as_deref() {
        Some("open-settings") if platform_name() != "windows" => Prerequisite {
            status: TemplateStatus::Unavailable,
            message: "Windows Settings links are unavailable on this operating system.".into(),
        },
        Some("open-task-manager") => match task_manager_path() {
            Some(path) => Prerequisite {
                status: TemplateStatus::Ready,
                message: format!("Task Manager found at {}.", path.display()),
            },
            None if platform_name() != "windows" => Prerequisite {
                status: TemplateStatus::Unavailable,
                message: "Task Manager is a Windows program and is not present here.".into(),
            },
            None => Prerequisite {
                status: TemplateStatus::NeedsSetup,
                message: "Taskmgr.exe was not found in System32.".into(),
            },
        },
        Some("open-downloads") => folder_ready(host, FolderKind::Downloads),
        Some("open-documents") => folder_ready(host, FolderKind::Documents),
        Some("open-terminal") => terminal_ready(record),
        Some("open-project-folder") => path_ready(record, "projectPath", true),
        Some("quick-note") => path_ready(record, "directory", true),
        Some("open-website") => url_ready(record),
        Some("dev-workspace") => workspace_ready(record, apps),
        Some("zeropulse-stack") => {
            let count = collection_counts
                .get(ZEROPULSE_COLLECTION_ID)
                .copied()
                .unwrap_or(0);
            if count == 0 {
                Prerequisite {
                    status: TemplateStatus::NeedsSetup,
                    message: "The ZeroPulse collection is empty. Add a real registered app before running this.".into(),
                }
            } else {
                Prerequisite {
                    status: TemplateStatus::Ready,
                    message: format!(
                        "{count} app(s) will be launched from the ZeroPulse collection."
                    ),
                }
            }
        }
        Some("check-then-launch") => check_ready(record, apps),
        _ => Prerequisite {
            status: TemplateStatus::Ready,
            message: "Ready to run.".into(),
        },
    }
}

pub fn preview(
    record: &WorkflowRecord,
    ctx: &EngineContext<'_>,
    host: &dyn Host,
) -> AppResult<Vec<StepPreview>> {
    let vars = variables(&record.document);
    record
        .document
        .steps
        .iter()
        .map(|step| describe_step(step, &vars, ctx, host))
        .collect()
}

pub fn execute(
    record: &WorkflowRecord,
    ctx: &EngineContext<'_>,
    host: &dyn Host,
    confirmer: &mut dyn Confirmer,
    cancel: &AtomicBool,
) -> RunReport {
    let gate = assess(record, host, ctx.apps, &collection_counts(ctx));
    if gate.status != TemplateStatus::Ready {
        return RunReport {
            status: match gate.status {
                TemplateStatus::NeedsSetup => "needsSetup".into(),
                TemplateStatus::Unavailable => "unavailable".into(),
                TemplateStatus::Ready => "completed".into(),
            },
            summary: gate.message,
            steps: Vec::new(),
        };
    }
    let vars = variables(&record.document);
    let steps = &record.document.steps;
    let mut index = 0;
    let mut executed = 0;
    let mut logs = Vec::new();
    let mut failed = false;
    while index < steps.len() {
        if cancel.load(Ordering::Relaxed) {
            return RunReport {
                status: "cancelled".into(),
                summary: "Cancelled before the next step.".into(),
                steps: logs,
            };
        }
        executed += 1;
        if executed > 48 {
            logs.push(log(
                "engine",
                "failed",
                "Stopped because the step limit was exceeded.",
            ));
            return RunReport {
                status: "failed".into(),
                summary: "The workflow was stopped because it looked cyclic or unbounded.".into(),
                steps: logs,
            };
        }
        let step = &steps[index];
        let started = now_iso();
        let outcome = run_step(step, &vars, ctx, host, confirmer, cancel);
        match outcome {
            StepFlow::Next(message) => {
                logs.push(StepLog {
                    step_id: step.id().to_string(),
                    status: "completed".into(),
                    message: redact(&message),
                    started_at: started,
                    finished_at: now_iso(),
                });
                index += 1;
            }
            StepFlow::Skip(message) => {
                logs.push(StepLog {
                    step_id: step.id().to_string(),
                    status: "skipped".into(),
                    message: redact(&message),
                    started_at: started,
                    finished_at: now_iso(),
                });
                index += 1;
            }
            StepFlow::Jump(target) => {
                logs.push(StepLog {
                    step_id: step.id().to_string(),
                    status: "completed".into(),
                    message: format!("Continuing at {target}."),
                    started_at: started.clone(),
                    finished_at: now_iso(),
                });
                let Some(next) = steps.iter().position(|candidate| candidate.id() == target) else {
                    logs.push(log(&target, "failed", "Branch target does not exist."));
                    failed = true;
                    break;
                };
                let branched = &steps[next];
                let branch_started = now_iso();
                match run_step(branched, &vars, ctx, host, confirmer, cancel) {
                    StepFlow::Next(message) | StepFlow::Skip(message) => {
                        logs.push(StepLog {
                            step_id: branched.id().to_string(),
                            status: "completed".into(),
                            message: redact(&message),
                            started_at: branch_started,
                            finished_at: now_iso(),
                        });
                    }
                    StepFlow::Fail(message) => {
                        logs.push(StepLog {
                            step_id: branched.id().to_string(),
                            status: "failed".into(),
                            message: redact(&message),
                            started_at: branch_started,
                            finished_at: now_iso(),
                        });
                        if policy_of(branched) != ErrorPolicy::Skip {
                            failed = true;
                        }
                    }
                    StepFlow::Cancelled => {
                        return RunReport {
                            status: "cancelled".into(),
                            summary: "Workflow cancelled.".into(),
                            steps: logs,
                        };
                    }
                    StepFlow::Jump(_) => {
                        logs.push(log(
                            branched.id(),
                            "failed",
                            "Nested branches are not supported.",
                        ));
                        failed = true;
                    }
                }
                break;
            }
            StepFlow::Fail(message) => {
                let policy = policy_of(step);
                logs.push(StepLog {
                    step_id: step.id().to_string(),
                    status: "failed".into(),
                    message: redact(&message),
                    started_at: started,
                    finished_at: now_iso(),
                });
                if policy == ErrorPolicy::Skip {
                    index += 1;
                } else {
                    failed = true;
                    break;
                }
            }
            StepFlow::Cancelled => {
                logs.push(log(step.id(), "cancelled", "Cancelled during this step."));
                return RunReport {
                    status: "cancelled".into(),
                    summary: "Workflow cancelled.".into(),
                    steps: logs,
                };
            }
        }
    }
    RunReport {
        status: if failed { "failed" } else { "completed" }.into(),
        summary: if failed {
            "Workflow stopped on a failed step.".into()
        } else {
            "Workflow completed.".into()
        },
        steps: logs,
    }
}

enum StepFlow {
    Next(String),
    Skip(String),
    Jump(String),
    Fail(String),
    Cancelled,
}

fn run_step(
    step: &Step,
    vars: &BTreeMap<String, String>,
    ctx: &EngineContext<'_>,
    host: &dyn Host,
    confirmer: &mut dyn Confirmer,
    cancel: &AtomicBool,
) -> StepFlow {
    match step {
        Step::LaunchApp { app_id, args, .. } => {
            let app_id = match subst(app_id, vars) {
                Ok(value) => value,
                Err(err) => return StepFlow::Fail(err.to_string()),
            };
            let Some(app) = ctx.apps.get(&app_id) else {
                return StepFlow::Fail(format!("No registered app with id {app_id}."));
            };
            let extra = match subst_all(args, vars) {
                Ok(value) => value,
                Err(err) => return StepFlow::Fail(err.to_string()),
            };
            match launch_app(host, app, &extra, false) {
                Ok(outcome) => StepFlow::Next(outcome.message),
                Err(err) => StepFlow::Fail(err.to_string()),
            }
        }
        Step::LaunchTrustedTool { tool, .. } => match tool {
            TrustedTool::TaskManager => match task_manager_path() {
                Some(path) => match host.launch_program(&path, &[], None, false) {
                    Ok(outcome) => {
                        StepFlow::Next(format!("{} ({})", outcome.message, path.display()))
                    }
                    Err(err) => StepFlow::Fail(err.to_string()),
                },
                None => StepFlow::Fail("Task Manager was not found.".into()),
            },
            TrustedTool::Terminal => {
                let configured = vars.get("terminalPath").cloned().unwrap_or_default();
                let path = if configured.trim().is_empty() {
                    detect_terminal()
                } else {
                    Some(Path::new(configured.trim()).to_path_buf())
                };
                let Some(path) = path else {
                    return StepFlow::Fail("No terminal is configured or detected.".into());
                };
                match host.launch_program(&path, &[], None, false) {
                    Ok(outcome) => {
                        StepFlow::Next(format!("{} ({})", outcome.message, path.display()))
                    }
                    Err(err) => StepFlow::Fail(err.to_string()),
                }
            }
        },
        Step::OpenFolder {
            path,
            skip_if_blank,
            ..
        } => {
            let path = match subst(path, vars) {
                Ok(value) => value,
                Err(err) => return StepFlow::Fail(err.to_string()),
            };
            if *skip_if_blank && path.trim().is_empty() {
                return StepFlow::Skip("Folder was blank, so this step was skipped.".into());
            }
            match validate_existing_directory(Path::new(&path))
                .and_then(|dir| host.open_folder(&dir))
            {
                Ok(outcome) => StepFlow::Next(outcome.message),
                Err(err) => StepFlow::Fail(err.to_string()),
            }
        }
        Step::OpenKnownFolder { folder, .. } => match host
            .known_folder(*folder)
            .and_then(|path| host.open_folder(&path))
        {
            Ok(outcome) => StepFlow::Next(outcome.message),
            Err(err) => StepFlow::Fail(err.to_string()),
        },
        Step::OpenUrl { url, .. } => {
            let url = match subst(url, vars).and_then(|url| validate_url(&url)) {
                Ok(url) => url,
                Err(err) => return StepFlow::Fail(err.to_string()),
            };
            match host.open_url(&url) {
                Ok(outcome) => StepFlow::Next(outcome.message),
                Err(err) => StepFlow::Fail(err.to_string()),
            }
        }
        Step::CopyText { text, .. } => {
            let text = match subst(text, vars) {
                Ok(value) => value,
                Err(err) => return StepFlow::Fail(err.to_string()),
            };
            match host.copy_text(&text) {
                Ok(()) => StepFlow::Next(format!("Copied {text}")),
                Err(err) => StepFlow::Fail(err.to_string()),
            }
        }
        Step::Delay { ms, .. } => {
            if *ms > 60_000 {
                return StepFlow::Fail("Delay cannot exceed 60 seconds.".into());
            }
            let mut waited = 0;
            while waited < *ms {
                if cancel.load(Ordering::Relaxed) {
                    return StepFlow::Cancelled;
                }
                let slice = (*ms - waited).min(20);
                std::thread::sleep(Duration::from_millis(slice));
                waited += slice;
            }
            StepFlow::Next(format!("Waited {ms} ms."))
        }
        Step::Notify { title, body, .. } => {
            let title = match subst(title, vars) {
                Ok(value) => value,
                Err(err) => return StepFlow::Fail(err.to_string()),
            };
            let body = match subst(body, vars) {
                Ok(value) => value,
                Err(err) => return StepFlow::Fail(err.to_string()),
            };
            match host.notify(&title, &body) {
                Ok(()) => StepFlow::Next(format!("Notification: {title}")),
                Err(err) => StepFlow::Fail(err.to_string()),
            }
        }
        Step::CreateNote {
            directory, body, ..
        } => {
            let directory = match subst(directory, vars) {
                Ok(value) => value,
                Err(err) => return StepFlow::Fail(err.to_string()),
            };
            let body = match subst(body, vars) {
                Ok(value) => value,
                Err(err) => return StepFlow::Fail(err.to_string()),
            };
            match create_note(Path::new(&directory), &body) {
                Ok(path) => StepFlow::Next(format!("Created {}", path.display())),
                Err(err) => StepFlow::Fail(err.to_string()),
            }
        }
        Step::CheckExists {
            path,
            then_id,
            else_id,
            ..
        } => {
            let path = match subst(path, vars) {
                Ok(value) => value,
                Err(err) => return StepFlow::Fail(err.to_string()),
            };
            if path.trim().is_empty() || !Path::new(path.trim()).is_absolute() {
                return StepFlow::Fail("The path to check must be absolute.".into());
            }
            if Path::new(path.trim()).exists() {
                StepFlow::Jump(then_id.clone())
            } else {
                StepFlow::Jump(else_id.clone())
            }
        }
        Step::Confirm {
            message,
            then_id,
            else_id,
            ..
        } => {
            let message = match subst(message, vars) {
                Ok(value) => value,
                Err(err) => return StepFlow::Fail(err.to_string()),
            };
            match confirmer.confirm(&message) {
                Ok(true) => StepFlow::Jump(then_id.clone()),
                Ok(false) => StepFlow::Jump(else_id.clone()),
                Err(err) => StepFlow::Fail(err.to_string()),
            }
        }
        Step::RunCommand {
            program,
            args,
            working_directory,
            timeout_ms,
            ..
        } => {
            if !ctx.developer_commands {
                return StepFlow::Fail(
                    "Developer command execution is off. Enable it in Settings after reviewing the exact program and arguments.".into(),
                );
            }
            let program = match subst(program, vars) {
                Ok(value) => value,
                Err(err) => return StepFlow::Fail(err.to_string()),
            };
            let args = match subst_all(args, vars) {
                Ok(value) => value,
                Err(err) => return StepFlow::Fail(err.to_string()),
            };
            if let Err(err) = validate_arguments(&args) {
                return StepFlow::Fail(err.to_string());
            }
            let cwd = working_directory.as_ref().and_then(|value| {
                subst(value, vars)
                    .ok()
                    .filter(|text| !text.trim().is_empty())
            });
            let timeout = Duration::from_millis((*timeout_ms).unwrap_or(15_000).min(120_000));
            match crate::launch::spawn_program(
                Path::new(&program),
                &args,
                cwd.as_deref().map(Path::new),
                true,
                Some(timeout),
            ) {
                Ok(outcome) => StepFlow::Next(outcome.message),
                Err(err) => StepFlow::Fail(err.to_string()),
            }
        }
        Step::LaunchCollection {
            collection_id,
            skip_missing,
            ..
        } => {
            let apps = ctx
                .collections
                .get(collection_id)
                .cloned()
                .unwrap_or_default();
            if apps.is_empty() {
                return StepFlow::Fail(format!("Collection {collection_id} has no apps."));
            }
            let mut notes = Vec::new();
            for app in apps {
                if app.missing && *skip_missing {
                    notes.push(format!("Skipped missing {}", app.display_name));
                    continue;
                }
                match launch_app(host, &app, &[], false) {
                    Ok(outcome) => notes.push(format!("{}: {}", app.display_name, outcome.message)),
                    Err(err) if *skip_missing => {
                        notes.push(format!("Skipped {}: {err}", app.display_name))
                    }
                    Err(err) => return StepFlow::Fail(err.to_string()),
                }
            }
            StepFlow::Next(notes.join("; "))
        }
    }
}

pub fn create_note(directory: &Path, body: &str) -> AppResult<std::path::PathBuf> {
    if body.len() > 100_000 {
        return Err(AppError::new(
            "NOTE",
            "Note text is limited to 100000 characters.",
        ));
    }
    let directory = validate_existing_directory(directory)?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let name = format!(
        "pulse-note-{stamp}-{}.txt",
        &uuid::Uuid::new_v4().to_string()[..8]
    );
    let path = directory.join(name);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    file.write_all(body.as_bytes())?;
    Ok(path)
}

fn describe_step(
    step: &Step,
    vars: &BTreeMap<String, String>,
    ctx: &EngineContext<'_>,
    host: &dyn Host,
) -> AppResult<StepPreview> {
    let detail = match step {
        Step::LaunchApp { app_id, args, .. } => {
            let app_id = subst(app_id, vars)?;
            match ctx.apps.get(&app_id) {
                Some(app) => format!(
                    "Launch {} {}",
                    app.display_name,
                    crate::security::display_command("", args)
                ),
                None => format!("Launch missing app {app_id}"),
            }
        }
        Step::LaunchTrustedTool { tool, .. } => match tool {
            TrustedTool::TaskManager => format!(
                "Launch Task Manager at {}",
                task_manager_path()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "not found".into())
            ),
            TrustedTool::Terminal => format!(
                "Launch terminal {}",
                vars.get("terminalPath")
                    .cloned()
                    .filter(|value| !value.is_empty())
                    .or_else(|| { detect_terminal().map(|path| path.display().to_string()) })
                    .unwrap_or_else(|| "not configured".into())
            ),
        },
        Step::OpenFolder { path, .. } => format!("Open folder {}", subst(path, vars)?),
        Step::OpenKnownFolder { folder, .. } => match host.known_folder(*folder) {
            Ok(path) => format!(
                "Open {} ({})",
                crate::launch::folder_name(*folder),
                path.display()
            ),
            Err(err) => format!("Open known folder: {err}"),
        },
        Step::OpenUrl { url, .. } => format!("Open URL {}", subst(url, vars)?),
        Step::CopyText { text, .. } => format!("Copy {}", subst(text, vars)?),
        Step::Delay { ms, .. } => format!("Wait {ms} ms"),
        Step::Notify { title, body, .. } => {
            format!("Notify {}: {}", subst(title, vars)?, subst(body, vars)?)
        }
        Step::CreateNote { directory, .. } => {
            format!("Create a new note in {}", subst(directory, vars)?)
        }
        Step::CheckExists {
            path,
            then_id,
            else_id,
            ..
        } => {
            format!(
                "If {} exists go to {then_id}, otherwise {else_id}",
                subst(path, vars)?
            )
        }
        Step::Confirm { message, .. } => format!("Ask: {}", subst(message, vars)?),
        Step::RunCommand { program, args, .. } => format!(
            "Run only if developer commands are enabled: {}",
            crate::security::display_command(&subst(program, vars)?, &subst_all(args, vars)?)
        ),
        Step::LaunchCollection { collection_id, .. } => {
            let count = ctx
                .collections
                .get(collection_id)
                .map(|apps| apps.len())
                .unwrap_or(0);
            format!("Launch {count} app(s) from {collection_id}")
        }
    };
    Ok(StepPreview {
        step_id: step.id().to_string(),
        action: step_action(step).into(),
        detail,
    })
}

fn step_action(step: &Step) -> &'static str {
    match step {
        Step::LaunchApp { .. } => "Launch app",
        Step::LaunchTrustedTool { .. } => "Launch tool",
        Step::OpenFolder { .. } | Step::OpenKnownFolder { .. } => "Open folder",
        Step::OpenUrl { .. } => "Open URL",
        Step::CopyText { .. } => "Copy text",
        Step::Delay { .. } => "Wait",
        Step::Notify { .. } => "Notify",
        Step::CreateNote { .. } => "Create note",
        Step::CheckExists { .. } => "Check path",
        Step::Confirm { .. } => "Confirm",
        Step::RunCommand { .. } => "Run command",
        Step::LaunchCollection { .. } => "Launch collection",
    }
}

fn policy_of(step: &Step) -> ErrorPolicy {
    match step {
        Step::LaunchApp { on_error, .. }
        | Step::LaunchTrustedTool { on_error, .. }
        | Step::OpenFolder { on_error, .. }
        | Step::OpenKnownFolder { on_error, .. }
        | Step::OpenUrl { on_error, .. }
        | Step::CopyText { on_error, .. }
        | Step::Delay { on_error, .. }
        | Step::Notify { on_error, .. }
        | Step::CreateNote { on_error, .. }
        | Step::RunCommand { on_error, .. }
        | Step::LaunchCollection { on_error, .. } => *on_error,
        Step::CheckExists { .. } | Step::Confirm { .. } => ErrorPolicy::Stop,
    }
}

fn variables(document: &WorkflowDocument) -> BTreeMap<String, String> {
    let mut vars = document.values.clone();
    for parameter in &document.parameters {
        vars.entry(parameter.key.clone()).or_default();
    }
    let format = vars.get("dateFormat").map(String::as_str).unwrap_or("iso");
    let date = if format == "friendly" {
        today_friendly()
    } else {
        today_iso()
    };
    vars.insert("date".into(), date);
    vars.insert("nowIso".into(), now_iso());
    vars
}

fn subst(input: &str, vars: &BTreeMap<String, String>) -> AppResult<String> {
    let re = Regex::new(r"\{\{\s*([A-Za-z0-9_]+)\s*\}\}").expect("substitution");
    let mut missing = None;
    let replaced = re.replace_all(input, |caps: &regex::Captures| {
        let key = &caps[1];
        match vars.get(key) {
            Some(value) => value.clone(),
            None => {
                missing = Some(key.to_string());
                caps[0].to_string()
            }
        }
    });
    if let Some(key) = missing {
        return Err(AppError::new(
            "VARIABLE",
            format!("Missing value for '{key}'."),
        ));
    }
    if replaced.len() > 8000 {
        return Err(AppError::new("VARIABLE", "Substituted text is too long."));
    }
    Ok(replaced.into_owned())
}

fn subst_all(values: &[String], vars: &BTreeMap<String, String>) -> AppResult<Vec<String>> {
    values.iter().map(|value| subst(value, vars)).collect()
}

fn missing_required(record: &WorkflowRecord) -> Option<String> {
    for parameter in &record.document.parameters {
        if parameter.required {
            let value = record
                .document
                .values
                .get(&parameter.key)
                .map(String::as_str)
                .unwrap_or("");
            if value.trim().is_empty() {
                return Some(format!("Needs setup: set {}.", parameter.label));
            }
        }
    }
    None
}

fn folder_ready(host: &dyn Host, folder: FolderKind) -> Prerequisite {
    match host.known_folder(folder) {
        Ok(path) if path.is_dir() => Prerequisite {
            status: TemplateStatus::Ready,
            message: format!("Folder is {}.", path.display()),
        },
        Ok(path) => Prerequisite {
            status: TemplateStatus::NeedsSetup,
            message: format!("{} is not a directory.", path.display()),
        },
        Err(err) => Prerequisite {
            status: TemplateStatus::NeedsSetup,
            message: err.to_string(),
        },
    }
}

fn terminal_ready(record: &WorkflowRecord) -> Prerequisite {
    let configured = record
        .document
        .values
        .get("terminalPath")
        .cloned()
        .unwrap_or_default();
    if !configured.trim().is_empty() {
        let path = Path::new(configured.trim());
        return if path.is_file() {
            Prerequisite {
                status: TemplateStatus::Ready,
                message: format!("Terminal set to {}.", path.display()),
            }
        } else {
            Prerequisite {
                status: TemplateStatus::NeedsSetup,
                message: format!("Terminal path does not exist: {}", path.display()),
            }
        };
    }
    match detect_terminal() {
        Some(path) => Prerequisite {
            status: TemplateStatus::Ready,
            message: format!("Detected terminal at {}.", path.display()),
        },
        None => Prerequisite {
            status: TemplateStatus::NeedsSetup,
            message: "No terminal was detected. Choose an executable in Configure.".into(),
        },
    }
}

fn path_ready(record: &WorkflowRecord, key: &str, must_exist: bool) -> Prerequisite {
    let value = record.document.values.get(key).cloned().unwrap_or_default();
    if value.trim().is_empty() {
        return Prerequisite {
            status: TemplateStatus::NeedsSetup,
            message: format!("Needs setup: choose {key}."),
        };
    }
    let path = Path::new(value.trim());
    if must_exist && !path.exists() {
        return Prerequisite {
            status: TemplateStatus::NeedsSetup,
            message: format!("{} does not exist.", path.display()),
        };
    }
    Prerequisite {
        status: TemplateStatus::Ready,
        message: format!("Using {}.", path.display()),
    }
}

fn url_ready(record: &WorkflowRecord) -> Prerequisite {
    let value = record
        .document
        .values
        .get("url")
        .cloned()
        .unwrap_or_default();
    match validate_url(&value) {
        Ok(url) => Prerequisite {
            status: TemplateStatus::Ready,
            message: format!("Will open {url}."),
        },
        Err(err) => Prerequisite {
            status: TemplateStatus::NeedsSetup,
            message: err.to_string(),
        },
    }
}

fn workspace_ready(record: &WorkflowRecord, apps: &HashMap<String, AppEntry>) -> Prerequisite {
    for key in ["appId1", "appId2"] {
        let id = record.document.values.get(key).cloned().unwrap_or_default();
        match apps.get(&id) {
            Some(app) if !app.missing => {}
            Some(app) => {
                return Prerequisite {
                    status: TemplateStatus::NeedsSetup,
                    message: format!("{} is missing on disk.", app.display_name),
                };
            }
            None => {
                return Prerequisite {
                    status: TemplateStatus::NeedsSetup,
                    message: format!("Choose a registered app for {key}."),
                };
            }
        }
    }
    Prerequisite {
        status: TemplateStatus::Ready,
        message: "Two registered apps are configured.".into(),
    }
}

fn check_ready(record: &WorkflowRecord, apps: &HashMap<String, AppEntry>) -> Prerequisite {
    let target = record
        .document
        .values
        .get("target")
        .cloned()
        .unwrap_or_default();
    let app_id = record
        .document
        .values
        .get("appId")
        .cloned()
        .unwrap_or_default();
    if target.trim().is_empty() || !Path::new(target.trim()).is_absolute() {
        return Prerequisite {
            status: TemplateStatus::NeedsSetup,
            message: "Set an absolute path to check.".into(),
        };
    }
    match apps.get(&app_id) {
        Some(app) => Prerequisite {
            status: TemplateStatus::Ready,
            message: format!(
                "If the path exists, {} launches. If not, you get a missing-target notice.",
                app.display_name
            ),
        },
        None => Prerequisite {
            status: TemplateStatus::NeedsSetup,
            message: "Choose the registered app that should launch when the path exists.".into(),
        },
    }
}

fn collection_counts(ctx: &EngineContext<'_>) -> HashMap<String, usize> {
    ctx.collections
        .iter()
        .map(|(id, apps)| (id.clone(), apps.len()))
        .collect()
}

fn log(id: &str, status: &str, message: &str) -> StepLog {
    StepLog {
        step_id: id.into(),
        status: status.into(),
        message: message.into(),
        started_at: now_iso(),
        finished_at: now_iso(),
    }
}

#[allow(clippy::too_many_arguments)]
fn template(
    id: &str,
    key: &str,
    name: &str,
    description: &str,
    what: &str,
    help: &str,
    parameters: Vec<ParameterDef>,
    values: BTreeMap<String, String>,
    steps: Vec<Step>,
) -> WorkflowRecord {
    let stamp = "1970-01-01T00:00:00Z".to_string();
    WorkflowRecord {
        id: id.into(),
        name: name.into(),
        description: description.into(),
        template_key: Some(key.into()),
        template_version: Some(TEMPLATE_VERSION),
        customized: false,
        document: WorkflowDocument {
            what_it_does: what.into(),
            prerequisites_help: help.into(),
            parameters,
            values,
            steps,
        },
        created_at: stamp.clone(),
        updated_at: stamp,
    }
}

fn param(key: &str, label: &str, kind: ParamKind, required: bool, help: &str) -> ParameterDef {
    ParameterDef {
        key: key.into(),
        label: label.into(),
        kind,
        required,
        options: Vec::new(),
        help: help.into(),
    }
}

fn param_enum(key: &str, label: &str, options: Vec<String>, help: &str) -> ParameterDef {
    ParameterDef {
        key: key.into(),
        label: label.into(),
        kind: ParamKind::Enum,
        required: true,
        options,
        help: help.into(),
    }
}

pub fn validate_document(document: &WorkflowDocument) -> AppResult<()> {
    if document.steps.is_empty() || document.steps.len() > 40 {
        return Err(AppError::new("WORKFLOW", "A workflow needs 1 to 40 steps."));
    }
    bounded_text(&document.what_it_does, 2000, "Description")?;
    let mut ids = std::collections::HashSet::new();
    for step in &document.steps {
        if !ids.insert(step.id().to_string()) {
            return Err(AppError::new(
                "WORKFLOW",
                format!("Duplicate step id {}.", step.id()),
            ));
        }
        if let Step::OpenUrl { url, .. } = step {
            if !url.contains("{{") {
                validate_url(url)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::RecordHost;
    use crate::models::{AppKind, SourceKind};
    use std::sync::atomic::AtomicBool;

    fn sample_app(id: &str, path: &Path) -> AppEntry {
        AppEntry {
            id: id.into(),
            display_name: "Demo".into(),
            kind: AppKind::Portable,
            source: SourceKind::Manual,
            launch_target: path.display().to_string(),
            resolved_target: Some(path.display().to_string()),
            arguments: Vec::new(),
            working_directory: None,
            icon_path: None,
            publisher: None,
            version: None,
            category: "Other".into(),
            hidden: false,
            favorite: false,
            pinned: false,
            user_modified: true,
            discovered: false,
            tags: Vec::new(),
            launch_count: 0,
            last_launched_at: None,
            missing: false,
            identity_key: format!("path:{}", path.display()),
            notes: None,
            created_at: now_iso(),
            updated_at: now_iso(),
            size_bytes: None,
        }
    }

    #[test]
    fn twelve_templates_are_unique() {
        let templates = builtin_templates();
        assert_eq!(templates.len(), 12);
        let mut keys = templates
            .iter()
            .map(|item| item.template_key.clone().unwrap())
            .collect::<Vec<_>>();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), 12);
    }

    #[test]
    fn copy_date_and_note_and_branch_have_real_effects() {
        let templates = builtin_templates();
        let copy = templates
            .iter()
            .find(|item| item.template_key.as_deref() == Some("copy-date"))
            .unwrap();
        let host = RecordHost::default();
        let ctx = EngineContext {
            apps: &HashMap::new(),
            collections: &HashMap::new(),
            developer_commands: false,
        };
        let cancel = AtomicBool::new(false);
        let report = execute(copy, &ctx, &host, &mut AutoConfirm(true), &cancel);
        assert_eq!(report.status, "completed", "{}", report.summary);
        let clipboard = host.clipboard.lock().unwrap().clone();
        assert!(clipboard.contains('-'), "{clipboard}");

        let dir = tempfile::tempdir().unwrap();
        let mut note = templates
            .iter()
            .find(|item| item.template_key.as_deref() == Some("quick-note"))
            .unwrap()
            .clone();
        note.document
            .values
            .insert("directory".into(), dir.path().display().to_string());
        note.document
            .values
            .insert("body".into(), "hello note".into());
        let report = execute(&note, &ctx, &host, &mut AutoConfirm(true), &cancel);
        assert_eq!(report.status, "completed", "{}", report.summary);
        let files: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(files.len(), 1);
        assert_eq!(std::fs::read_to_string(&files[0]).unwrap(), "hello note");
        let again = create_note(dir.path(), "second").unwrap();
        assert_ne!(again, files[0]);

        let binary = dir.path().join("tool");
        std::fs::write(&binary, b"x").unwrap();
        let app = sample_app("app-1", &binary);
        let mut apps = HashMap::new();
        apps.insert(app.id.clone(), app);
        let mut check = templates
            .iter()
            .find(|item| item.template_key.as_deref() == Some("check-then-launch"))
            .unwrap()
            .clone();
        check.document.values.insert(
            "target".into(),
            dir.path().join("missing-file").display().to_string(),
        );
        check.document.values.insert("appId".into(), "app-1".into());
        let ctx = EngineContext {
            apps: &apps,
            collections: &HashMap::new(),
            developer_commands: false,
        };
        let before = host.calls.lock().unwrap().len();
        let report = execute(&check, &ctx, &host, &mut AutoConfirm(true), &cancel);
        assert_eq!(report.status, "completed", "{:?}", report.steps);
        assert!(host
            .notifications
            .lock()
            .unwrap()
            .iter()
            .any(|(_, body)| body.contains("does not exist")));
        assert_eq!(host.calls.lock().unwrap().len(), before + 1);

        let present = dir.path().join("present.txt");
        std::fs::write(&present, b"ok").unwrap();
        check
            .document
            .values
            .insert("target".into(), present.display().to_string());
        let report = execute(&check, &ctx, &host, &mut AutoConfirm(true), &cancel);
        assert_eq!(report.status, "completed", "{:?}", report.steps);
        assert!(host
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|call| call.contains("launch")));
    }

    #[test]
    fn developer_commands_default_off_and_cancel_interrupts_delay() {
        let host = RecordHost::default();
        let ctx = EngineContext {
            apps: &HashMap::new(),
            collections: &HashMap::new(),
            developer_commands: false,
        };
        let record = template(
            "custom",
            "custom",
            "Custom",
            "d",
            "Runs a command",
            "help",
            vec![],
            BTreeMap::new(),
            vec![Step::RunCommand {
                id: "run".into(),
                program: "/bin/echo".into(),
                args: vec!["hi".into()],
                working_directory: None,
                timeout_ms: Some(1000),
                on_error: ErrorPolicy::Stop,
            }],
        );
        let cancel = AtomicBool::new(false);
        let report = execute(&record, &ctx, &host, &mut AutoConfirm(true), &cancel);
        assert_eq!(report.status, "failed");
        assert!(report.steps[0].message.contains("Developer command"));

        let delay = template(
            "delay",
            "delay",
            "Delay",
            "d",
            "Waits",
            "help",
            vec![],
            BTreeMap::new(),
            vec![
                Step::Delay {
                    id: "wait".into(),
                    ms: 5_000,
                    on_error: ErrorPolicy::Stop,
                },
                Step::Notify {
                    id: "after".into(),
                    title: "no".into(),
                    body: "no".into(),
                    on_error: ErrorPolicy::Stop,
                },
            ],
        );
        let cancel = AtomicBool::new(true);
        let report = execute(&delay, &ctx, &host, &mut AutoConfirm(true), &cancel);
        assert_eq!(report.status, "cancelled");
        assert!(report.steps.iter().all(|step| step.step_id != "after"));
    }

    #[test]
    fn empty_zeropulse_stack_does_not_pretend_to_launch() {
        let record = builtin_templates()
            .into_iter()
            .find(|item| item.template_key.as_deref() == Some("zeropulse-stack"))
            .unwrap();
        let host = RecordHost::default();
        let ctx = EngineContext {
            apps: &HashMap::new(),
            collections: &HashMap::new(),
            developer_commands: false,
        };
        let report = execute(
            &record,
            &ctx,
            &host,
            &mut AutoConfirm(true),
            &AtomicBool::new(false),
        );
        assert_eq!(report.status, "needsSetup");
        assert!(host.calls.lock().unwrap().is_empty());
        let _ = bounded_text("ok", 10, "Name");
    }

    #[test]
    fn import_rejects_a_javascript_url() {
        let mut record = builtin_templates()
            .into_iter()
            .find(|item| item.template_key.as_deref() == Some("open-website"))
            .unwrap();
        record.document.steps = vec![Step::OpenUrl {
            id: "open".into(),
            url: "javascript:alert(1)".into(),
            on_error: ErrorPolicy::Stop,
        }];
        assert!(validate_document(&record.document).is_err());
    }
}
