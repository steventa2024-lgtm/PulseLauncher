//! The twelve built-in workflows. They are seeded once, keyed by
//! `template_key`, and never inserted again on a later start.

use std::collections::BTreeMap;

use super::*;

fn step(action: Action) -> Step {
    Step {
        on_error: OnError::Stop,
        retries: 0,
        action,
    }
}

fn skip(action: Action) -> Step {
    Step {
        on_error: OnError::Skip,
        retries: 0,
        action,
    }
}

fn cont(action: Action) -> Step {
    Step {
        on_error: OnError::Continue,
        retries: 0,
        action,
    }
}

struct Spec {
    key: &'static str,
    name: &'static str,
    kind: WorkflowKind,
    what: &'static str,
    prereq: &'static str,
    params: Vec<ParamDef>,
    values: BTreeMap<String, String>,
    steps: Vec<Step>,
}

fn build(s: Spec) -> WorkflowDefinition {
    WorkflowDefinition {
        schema: SCHEMA,
        id: format!("template:{}", s.key),
        name: s.name.into(),
        description: s.what.into(),
        what_it_does: s.what.into(),
        prerequisites: s.prereq.into(),
        kind: s.kind,
        template_key: Some(s.key.into()),
        template_version: Some(1),
        params: s.params,
        values: s.values,
        steps: s.steps,
        hotkey: None,
        trusted: true,
    }
}

fn text(key: &str, label: &str, help: &str, required: bool) -> ParamDef {
    ParamDef {
        key: key.into(),
        label: label.into(),
        help: help.into(),
        kind: ParamKind::Text,
        required,
        default_value: String::new(),
        choices: vec![],
    }
}

fn folder(key: &str, label: &str, help: &str) -> ParamDef {
    ParamDef {
        key: key.into(),
        label: label.into(),
        help: help.into(),
        kind: ParamKind::Folder,
        required: true,
        default_value: String::new(),
        choices: vec![],
    }
}

fn file(key: &str, label: &str, help: &str) -> ParamDef {
    ParamDef {
        key: key.into(),
        label: label.into(),
        help: help.into(),
        kind: ParamKind::File,
        required: true,
        default_value: String::new(),
        choices: vec![],
    }
}

fn path_param(key: &str, label: &str, help: &str) -> ParamDef {
    ParamDef {
        key: key.into(),
        label: label.into(),
        help: help.into(),
        kind: ParamKind::Path,
        required: true,
        default_value: String::new(),
        choices: vec![],
    }
}

fn url(key: &str, label: &str, help: &str) -> ParamDef {
    ParamDef {
        key: key.into(),
        label: label.into(),
        help: help.into(),
        kind: ParamKind::Url,
        required: true,
        default_value: String::new(),
        choices: vec![],
    }
}

fn app(key: &str, label: &str) -> ParamDef {
    ParamDef {
        key: key.into(),
        label: label.into(),
        help: "A program you have already registered in PulseLauncher.".into(),
        kind: ParamKind::App,
        required: false,
        default_value: String::new(),
        choices: vec![],
    }
}

pub fn all() -> Vec<WorkflowDefinition> {
    vec![
        build(Spec {
            key: "open-downloads",
            name: "Open Downloads",
            kind: WorkflowKind::Command,
            what: "Opens your Downloads folder in the file manager. The location comes from the operating system's known folder, not a guessed path.",
            prereq: "The Downloads known folder must resolve on this system.",
            params: vec![],
            values: BTreeMap::new(),
            steps: vec![step(Action::OpenFolder { path: "${downloads}".into() })],
        }),
        build(Spec {
            key: "open-documents",
            name: "Open Documents",
            kind: WorkflowKind::Command,
            what: "Opens your Documents folder in the file manager, using the operating system's known folder.",
            prereq: "The Documents known folder must resolve on this system.",
            params: vec![],
            values: BTreeMap::new(),
            steps: vec![step(Action::OpenFolder { path: "${documents}".into() })],
        }),
        build(Spec {
            key: "open-settings",
            name: "Open Windows Settings",
            kind: WorkflowKind::Command,
            what: "Opens Windows Settings through the documented ms-settings: link and the system's own association.",
            prereq: "Windows. Other systems report this template as unavailable.",
            params: vec![],
            values: BTreeMap::new(),
            steps: vec![step(Action::OpenUri { uri: "ms-settings:".into() })],
        }),
        build(Spec {
            key: "open-task-manager",
            name: "Open Task Manager",
            kind: WorkflowKind::Command,
            what: "Launches Task Manager from the detected Windows path. If that file is not there, the run fails with the reason instead of guessing another program.",
            prereq: "Taskmgr.exe must exist under the Windows directory.",
            params: vec![],
            values: BTreeMap::new(),
            steps: vec![step(Action::LaunchPath { path: "${taskmgr}".into(), args: vec![] })],
        }),
        build(Spec {
            key: "open-terminal",
            name: "Open Terminal",
            kind: WorkflowKind::Command,
            what: "Opens the terminal program you select. Windows Terminal is offered when it is installed; otherwise you pick an installed alternative. Nothing is assumed.",
            prereq: "Choose an installed terminal program.",
            params: vec![file("terminal", "Terminal", "Absolute path of the terminal program, such as wt.exe or pwsh.exe.")],
            values: BTreeMap::new(),
            steps: vec![step(Action::LaunchPath { path: "${terminal}".into(), args: vec![] })],
        }),
        build(Spec {
            key: "open-project",
            name: "Open Project Folder",
            kind: WorkflowKind::Command,
            what: "Opens a folder you choose, through the file manager. The choice is saved with this workflow.",
            prereq: "Choose an existing folder.",
            params: vec![folder("folder", "Project folder", "An existing folder to open.")],
            values: BTreeMap::new(),
            steps: vec![step(Action::OpenFolder { path: "${folder}".into() })],
        }),
        build(Spec {
            key: "dev-workspace",
            name: "Launch Development Workspace",
            kind: WorkflowKind::Workflow,
            what: "Starts the apps and folder you configure, one after another, and records each result. A missing item is skipped; if nothing was configured, the run fails.",
            prereq: "Register the apps first, then choose at least two apps or a folder.",
            params: vec![
                app("app1", "First app"),
                app("app2", "Second app"),
                ParamDef { required: false, ..folder("folder", "Folder", "An optional folder to open.") },
            ],
            values: BTreeMap::new(),
            steps: vec![
                skip(Action::LaunchApp { app_id: "${app1}".into(), args: vec![] }),
                skip(Action::LaunchApp { app_id: "${app2}".into(), args: vec![] }),
                skip(Action::OpenFolder { path: "${folder}".into() }),
            ],
        }),
        build(Spec {
            key: "copy-date",
            name: "Copy Today's Date",
            kind: WorkflowKind::Command,
            what: "Copies today's local date to the clipboard and shows a confirmation. Choose ISO (2026-10-12) or a long form.",
            prereq: "None. Uses the computer's clock.",
            params: vec![ParamDef {
                key: "format".into(),
                label: "Format".into(),
                help: "iso writes YYYY-MM-DD. long writes a date like Monday, October 12, 2026.".into(),
                kind: ParamKind::Choice,
                required: false,
                default_value: "iso".into(),
                choices: vec![
                    ParamChoice { value: "iso".into(), label: "ISO (2026-10-12)".into() },
                    ParamChoice { value: "long".into(), label: "Long (Monday, October 12, 2026)".into() },
                ],
            }],
            values: BTreeMap::from([("format".into(), "iso".into())]),
            steps: vec![
                step(Action::CopyText { text: "${date_text}".into() }),
                cont(Action::Notify { title: "Date copied".into(), body: "${date_text}".into() }),
            ],
        }),
        build(Spec {
            key: "quick-note",
            name: "Timestamped Quick Note",
            kind: WorkflowKind::Workflow,
            what: "Creates a new .txt file named with the current time in a folder you choose. An existing file is never overwritten.",
            prereq: "Choose a folder you can write to.",
            params: vec![
                folder("folder", "Folder", "Where the note will be created."),
                text("prefix", "File prefix", "Used in the file name. Defaults to note.", false),
                text("content", "Text", "Optional text to put in the file.", false),
            ],
            values: BTreeMap::from([("prefix".into(), "note".into()), ("content".into(), String::new())]),
            steps: vec![step(Action::WriteNote {
                folder: "${folder}".into(),
                prefix: "${prefix}".into(),
                content: "${content}".into(),
            })],
        }),
        build(Spec {
            key: "open-website",
            name: "Open Project Website",
            kind: WorkflowKind::Command,
            what: "Opens an http or https address you provide in your default browser. Other link types are rejected.",
            prereq: "Enter an http:// or https:// address.",
            params: vec![url("url", "Address", "The page to open.")],
            values: BTreeMap::new(),
            steps: vec![step(Action::OpenUrl { url: "${url}".into() })],
        }),
        build(Spec {
            key: "zeropulse-stack",
            name: "Launch ZeroPulse Stack",
            kind: WorkflowKind::Workflow,
            what: "Launches the registered programs you add here, in order. It starts empty: PulseCut, PulseFlow, OpenPulse and anything else appear only after you register them. A missing entry is reported and is not invented.",
            prereq: "Register at least one real program, then add it to this list.",
            params: vec![ParamDef {
                key: "apps".into(),
                label: "Apps".into(),
                help: "Registered programs to launch, in order.".into(),
                kind: ParamKind::Apps,
                required: true,
                default_value: "[]".into(),
                choices: vec![],
            }],
            values: BTreeMap::new(),
            steps: vec![step(Action::LaunchEach { apps: "${apps}".into() })],
        }),
        build(Spec {
            key: "check-then-launch",
            name: "Check Then Launch",
            kind: WorkflowKind::Workflow,
            what: "Checks that a program or file you name is present. If it is, asks you to confirm, then launches it. If it is not, shows exactly what is missing and launches nothing.",
            prereq: "Choose an absolute path. It does not have to exist yet.",
            params: vec![path_param("target", "Target", "Absolute path of the program or file to check.")],
            values: BTreeMap::new(),
            steps: vec![step(Action::CheckExists {
                path: "${target}".into(),
                then_steps: vec![
                    step(Action::Confirm { message: "Launch ${target}?".into() }),
                    step(Action::LaunchPath { path: "${target}".into(), args: vec![] }),
                ],
                else_steps: vec![step(Action::Notify {
                    title: "Target missing".into(),
                    body: "${target} was not found. Nothing was launched.".into(),
                })],
            })],
        }),
    ]
}

pub fn by_key(key: &str) -> Option<WorkflowDefinition> {
    all()
        .into_iter()
        .find(|d| d.template_key.as_deref() == Some(key))
}
