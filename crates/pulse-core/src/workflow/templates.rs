//! The twelve built-in, editable starter templates. They are seeded once (keyed by template id)
//! and never duplicated. A template reports `NeedsSetup` until its prerequisites are satisfied.

use super::model::*;
use crate::error::Result;
use crate::storage::Storage;
use std::collections::BTreeMap;

pub const TEMPLATE_VERSION: u32 = 1;
pub const ZEROPULSE_COLLECTION: &str = "zeropulse";

fn base(id: &str, kind: WorkflowKind, name: &str, desc: &str) -> WorkflowDefinition {
    WorkflowDefinition {
        id: id.into(),
        kind,
        name: name.into(),
        description: desc.into(),
        template_id: Some(id.into()),
        template_version: Some(TEMPLATE_VERSION),
        params: vec![],
        values: BTreeMap::new(),
        prerequisites: vec![],
        steps: vec![],
        enabled: true,
    }
}

fn param(name: &str, label: &str, kind: ParamKind, required: bool, help: &str) -> ParamDef {
    ParamDef { name: name.into(), label: label.into(), kind, required, default: None, choices: vec![], help: Some(help.into()) }
}

fn choice(v: &str, l: &str) -> ParamChoice {
    ParamChoice { value: v.into(), label: l.into() }
}

pub fn all_templates() -> Vec<WorkflowDefinition> {
    let mut v = Vec::new();

    let mut t = base("tpl.open_downloads", WorkflowKind::Command, "Open Downloads", "Opens your real Downloads folder (resolved through Windows Known Folders) in Explorer.");
    t.steps = vec![Step::new("open", "Open Downloads folder", Action::OpenPath { path: "{{known:downloads}}".into() })];
    v.push(t);

    let mut t = base("tpl.open_documents", WorkflowKind::Command, "Open Documents", "Opens your real Documents folder (resolved through Windows Known Folders) in Explorer.");
    t.steps = vec![Step::new("open", "Open Documents folder", Action::OpenPath { path: "{{known:documents}}".into() })];
    v.push(t);

    let mut t = base("tpl.open_settings", WorkflowKind::Command, "Open Windows Settings", "Opens Windows Settings through the documented ms-settings: URI.");
    t.prerequisites = vec![Prerequisite::Windows];
    t.steps = vec![Step::new("open", "Open ms-settings:", Action::OpenSettingsUri { uri: "ms-settings:".into() })];
    v.push(t);

    let mut t = base("tpl.open_task_manager", WorkflowKind::Command, "Open Task Manager", "Starts the Windows Task Manager from %SystemRoot%\\System32\\Taskmgr.exe. Windows itself handles any elevation prompt.");
    t.prerequisites = vec![
        Prerequisite::Windows,
        Prerequisite::FileExists { label: "Taskmgr.exe is present".into(), path: "{{env:SystemRoot}}\\System32\\Taskmgr.exe".into() },
    ];
    t.steps = vec![Step::new("launch", "Start Task Manager", Action::LaunchPath { path: "{{env:SystemRoot}}\\System32\\Taskmgr.exe".into(), args: vec![], working_dir: None })];
    v.push(t);

    let mut t = base("tpl.open_terminal", WorkflowKind::Workflow, "Open Terminal", "Opens the terminal you choose. If none is chosen, Windows Terminal is used only when it is actually detected.");
    t.params = vec![param("terminal", "Terminal executable (optional)", ParamKind::File, false, "Leave empty to use Windows Terminal when detected, or choose e.g. powershell.exe / pwsh.exe / cmd.exe.")];
    t.prerequisites = vec![Prerequisite::AnyFileExists { label: "A terminal is chosen or Windows Terminal is detected".into(), paths: vec!["{{terminal}}".into(), "{{env:LOCALAPPDATA}}\\Microsoft\\WindowsApps\\wt.exe".into()] }];
    t.steps = vec![
        Step { when: Some(Condition::NotEmpty { value: "{{terminal}}".into() }), ..Step::new("custom", "Launch chosen terminal", Action::LaunchPath { path: "{{terminal}}".into(), args: vec![], working_dir: None }) },
        Step {
            when: Some(Condition::Not { inner: Box::new(Condition::NotEmpty { value: "{{terminal}}".into() }) }),
            ..Step::new(
                "detect",
                "Use Windows Terminal if detected",
                Action::If {
                    condition: Condition::Exists { path: "{{env:LOCALAPPDATA}}\\Microsoft\\WindowsApps\\wt.exe".into() },
                    then_steps: vec![Step::new("wt", "Launch Windows Terminal", Action::LaunchPath { path: "{{env:LOCALAPPDATA}}\\Microsoft\\WindowsApps\\wt.exe".into(), args: vec![], working_dir: None })],
                    else_steps: vec![Step::new("none", "No terminal available", Action::Fail { message: "No terminal is configured and Windows Terminal was not detected. Choose a terminal in Configure.".into() })],
                },
            )
        },
    ];
    v.push(t);

    let mut t = base("tpl.open_project_folder", WorkflowKind::Command, "Open Project Folder", "Opens your chosen project folder in Explorer.");
    t.params = vec![param("folder", "Project folder", ParamKind::Folder, true, "An existing folder on this PC.")];
    t.prerequisites = vec![Prerequisite::ParamsSet { names: vec!["folder".into()] }];
    t.steps = vec![Step::new("open", "Open project folder", Action::OpenPath { path: "{{folder}}".into() })];
    v.push(t);

    let mut t = base("tpl.dev_workspace", WorkflowKind::Workflow, "Launch Development Workspace", "Starts 2 to 4 of your real apps and folders, one after another, with a result for each.");
    let mut policy = param("on_missing", "If an item fails", ParamKind::Choice, false, "Skip continues with the next item; Stop ends the workflow.");
    policy.choices = vec![choice("continue", "Skip it and continue"), choice("stop", "Stop the workflow")];
    policy.default = Some("continue".into());
    t.params = vec![
        param("item1", "Item 1", ParamKind::Target, true, "A registered app or a folder/file path."),
        param("item2", "Item 2", ParamKind::Target, true, "A registered app or a folder/file path."),
        param("item3", "Item 3 (optional)", ParamKind::Target, false, "A registered app or a folder/file path."),
        param("item4", "Item 4 (optional)", ParamKind::Target, false, "A registered app or a folder/file path."),
        policy,
    ];
    t.prerequisites = vec![Prerequisite::ParamsSet { names: vec!["item1".into(), "item2".into()] }];
    t.steps = (1..=4)
        .flat_map(|i| {
            let mut open = Step::new(&format!("open{i}"), &format!("Open item {i}"), Action::OpenTarget { target: format!("{{{{item{i}}}}}") });
            open.when = Some(Condition::NotEmpty { value: format!("{{{{item{i}}}}}") });
            open.on_error_param = Some("on_missing".into());
            let mut v = vec![open];
            if i < 4 {
                let mut w = Step::new(&format!("pause{i}"), "Short pause", Action::Wait { ms: 400 });
                w.when = Some(Condition::NotEmpty { value: format!("{{{{item{}}}}}", i + 1) });
                v.push(w);
            }
            v
        })
        .collect();
    v.push(t);

    let mut t = base("tpl.copy_date", WorkflowKind::Command, "Copy Today's Date", "Copies the local system date to the clipboard in the format you choose.");
    let mut f = param("format", "Date format", ParamKind::Choice, false, "Uses your PC's local date.");
    f.choices = vec![choice("%Y-%m-%d", "ISO (2026-10-12)"), choice("%A, %B %-d, %Y", "Long (Monday, October 12, 2026)"), choice("%d/%m/%Y", "Day/Month/Year"), choice("%m/%d/%Y", "Month/Day/Year")];
    f.default = Some("%Y-%m-%d".into());
    t.params = vec![f];
    t.steps = vec![Step::new("copy", "Copy date", Action::CopyText { text: "{{now:{{format}}}}".into() })];
    v.push(t);

    let mut t = base("tpl.quick_note", WorkflowKind::Workflow, "Timestamped Quick Note", "Creates a new, uniquely named timestamped .txt file in a folder you choose. Existing files are never overwritten.");
    t.params = vec![
        param("folder", "Notes folder", ParamKind::Folder, true, "A writable folder you choose."),
        param("text", "Note text (optional)", ParamKind::Text, false, "Written into the new file."),
    ];
    t.prerequisites = vec![Prerequisite::ParamsSet { names: vec!["folder".into()] }];
    t.steps = vec![Step::new("note", "Create note file", Action::CreateNote { dir: "{{folder}}".into(), file_name: "note-{{now:%Y%m%d-%H%M%S}}.txt".into(), content: "{{text}}\n".into() })];
    v.push(t);

    let mut t = base("tpl.open_website", WorkflowKind::Command, "Open Project Website", "Opens a http(s) address you provide in your default browser.");
    t.params = vec![param("url", "Website address", ParamKind::Url, true, "Must start with http:// or https://.")];
    t.prerequisites = vec![Prerequisite::ParamsSet { names: vec!["url".into()] }];
    t.steps = vec![Step::new("open", "Open website", Action::OpenUrl { url: "{{url}}".into() })];
    v.push(t);

    let mut t = base("tpl.zeropulse_stack", WorkflowKind::Workflow, "Launch ZeroPulse Stack", "Launches every app you have registered in the ZeroPulse collection. Empty until you add real apps.");
    t.prerequisites = vec![Prerequisite::CollectionNotEmpty { label: "ZeroPulse collection contains registered apps".into(), system_key: ZEROPULSE_COLLECTION.into() }];
    t.steps = vec![Step::new("stack", "Launch ZeroPulse collection", Action::LaunchCollection { system_key: ZEROPULSE_COLLECTION.into() })];
    v.push(t);

    let mut t = base("tpl.check_then_launch", WorkflowKind::Workflow, "Check Then Launch", "Verifies that your chosen program exists, then launches it; otherwise reports exactly which target is missing.");
    t.params = vec![
        param("target", "Program to launch", ParamKind::File, true, "Absolute path to an .exe."),
        param("arguments", "Arguments (optional)", ParamKind::Text, false, "Passed as a single argument."),
    ];
    t.prerequisites = vec![Prerequisite::ParamsSet { names: vec!["target".into()] }];
    t.steps = vec![Step::new(
        "check",
        "Check target exists",
        Action::If {
            condition: Condition::Exists { path: "{{target}}".into() },
            then_steps: vec![
                Step { when: Some(Condition::NotEmpty { value: "{{arguments}}".into() }), ..Step::new("run_args", "Launch with argument", Action::LaunchPath { path: "{{target}}".into(), args: vec!["{{arguments}}".into()], working_dir: None }) },
                Step { when: Some(Condition::Not { inner: Box::new(Condition::NotEmpty { value: "{{arguments}}".into() }) }), ..Step::new("run", "Launch", Action::LaunchPath { path: "{{target}}".into(), args: vec![], working_dir: None }) },
            ],
            else_steps: vec![Step::new("missing", "Report missing target", Action::Fail { message: "Target not found: {{target}}. Move it back or choose the new location in Configure.".into() })],
        },
    )];
    v.push(t);

    v
}

pub fn template(id: &str) -> Option<WorkflowDefinition> {
    all_templates().into_iter().find(|t| t.id == id)
}

/// Insert missing templates only. Existing (possibly user-edited) rows are never touched.
pub fn seed(storage: &Storage) -> Result<usize> {
    let mut n = 0;
    for t in all_templates() {
        if storage.get_workflow_by_template(t.id_str())?.is_none() {
            storage.upsert_workflow(&t)?;
            n += 1;
        }
    }
    Ok(n)
}

impl WorkflowDefinition {
    fn id_str(&self) -> &str {
        self.template_id.as_deref().unwrap_or(&self.id)
    }
}

/// Restore a template to its shipped definition, discarding edits and saved configuration.
pub fn reset(storage: &Storage, template_id: &str) -> Result<WorkflowDefinition> {
    let t = template(template_id).ok_or_else(|| crate::error::CoreError::NotFound("unknown template".into()))?;
    storage.upsert_workflow(&t)?;
    Ok(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::engine::validate_definition;

    #[test]
    fn twelve_valid_unique_templates() {
        let all = all_templates();
        assert_eq!(all.len(), 12);
        let mut ids: Vec<_> = all.iter().map(|t| t.id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 12);
        for t in &all {
            validate_definition(t).unwrap_or_else(|e| panic!("{}: {e}", t.id));
            assert!(t.template_id.is_some());
        }
    }

    #[test]
    fn seeding_is_idempotent_and_keeps_user_edits() {
        let s = Storage::open_in_memory().unwrap();
        assert_eq!(seed(&s).unwrap(), 12);
        let mut w = s.get_workflow("tpl.open_downloads").unwrap();
        w.name = "My Downloads".into();
        s.upsert_workflow(&w).unwrap();
        assert_eq!(seed(&s).unwrap(), 0);
        assert_eq!(s.list_workflows().unwrap().len(), 12);
        assert_eq!(s.get_workflow("tpl.open_downloads").unwrap().name, "My Downloads");
        assert_eq!(reset(&s, "tpl.open_downloads").unwrap().name, "Open Downloads");
        assert_eq!(s.get_workflow("tpl.open_downloads").unwrap().name, "Open Downloads");
    }
}
