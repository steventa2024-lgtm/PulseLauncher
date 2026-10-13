//! Versioned settings/registration backup with validation and a preview before anything is written.
//! Never exported: launch history, run history, discovered apps, cached icons, preview images.

use crate::error::{CoreError, Result};
use crate::models::*;
use crate::security;
use crate::storage::{AppPatch, Storage};
use crate::workflow::engine::validate_definition;
use crate::workflow::model::*;
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;
const MAX_APPS: usize = 5_000;
const MAX_WORKFLOWS: usize = 500;
const MAX_COLLECTIONS: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedApp {
    pub name: String,
    pub target: ExecutableTarget,
    #[serde(default)]
    pub category: AppCategory,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub collections: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bundle {
    pub schema_version: u32,
    pub app: String,
    pub exported_at: String,
    pub preferences: UserPreferences,
    pub apps: Vec<ExportedApp>,
    pub collections: Vec<String>,
    pub workflows: Vec<WorkflowDefinition>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub schema_version: u32,
    pub new_apps: usize,
    pub updated_apps: usize,
    pub apps_with_missing_target: Vec<String>,
    pub new_collections: usize,
    pub new_workflows: usize,
    pub updated_workflows: usize,
    pub workflows_with_commands: Vec<String>,
    pub preferences_changed: bool,
    pub warnings: Vec<String>,
}

pub fn export_bundle(storage: &Storage) -> Result<Bundle> {
    let colls = storage.list_collections()?;
    let apps = storage
        .list_apps(true)?
        .into_iter()
        .filter(|a| a.source == AppSource::Manual)
        .map(|a| ExportedApp {
            collections: a
                .collection_ids
                .iter()
                .filter_map(|id| colls.iter().find(|c| &c.id == id).map(|c| c.name.clone()))
                .collect(),
            name: a.name,
            target: a.target,
            category: a.category,
            tags: a.tags,
            favorite: a.favorite,
        })
        .collect();
    Ok(Bundle {
        schema_version: SCHEMA_VERSION,
        app: "PulseLauncher".into(),
        exported_at: crate::storage::now_iso(),
        preferences: storage.get_preferences()?,
        apps,
        collections: colls.iter().filter(|c| c.system_key.is_none()).map(|c| c.name.clone()).collect(),
        workflows: storage.list_workflows()?,
    })
}

pub fn parse_bundle(json: &str) -> Result<Bundle> {
    if json.len() > security::MAX_IMPORT_BYTES {
        return Err(CoreError::invalid("import file is too large"));
    }
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| CoreError::invalid(format!("not valid JSON: {e}")))?;
    let ver = v.get("schemaVersion").and_then(|x| x.as_u64()).ok_or_else(|| CoreError::invalid("missing schemaVersion"))?;
    if ver as u32 != SCHEMA_VERSION {
        return Err(CoreError::invalid(format!("unsupported schema version {ver} (this build reads {SCHEMA_VERSION})")));
    }
    if v.get("app").and_then(|x| x.as_str()) != Some("PulseLauncher") {
        return Err(CoreError::invalid("file was not exported by PulseLauncher"));
    }
    let b: Bundle = serde_json::from_value(v).map_err(|e| CoreError::invalid(format!("invalid backup structure: {e}")))?;
    if b.apps.len() > MAX_APPS || b.workflows.len() > MAX_WORKFLOWS || b.collections.len() > MAX_COLLECTIONS {
        return Err(CoreError::invalid("backup contains too many items"));
    }
    for a in &b.apps {
        security::validate_display_name(&a.name)?;
        security::validate_target_opts(&a.target, false)?;
        crate::storage::normalize_tags(&a.tags)?;
    }
    for c in &b.collections {
        security::validate_display_name(c)?;
    }
    for w in &b.workflows {
        validate_definition(w).map_err(|e| CoreError::invalid(format!("workflow '{}': {e}", w.name)))?;
    }
    Ok(b)
}

fn has_command(steps: &[Step]) -> bool {
    steps.iter().any(|s| match &s.action {
        Action::RunCommand { .. } => true,
        Action::If { then_steps, else_steps, .. } => has_command(then_steps) || has_command(else_steps),
        _ => false,
    })
}

pub fn preview_import(storage: &Storage, b: &Bundle) -> Result<ImportPreview> {
    let manual: Vec<AppEntry> = storage.list_apps(true)?.into_iter().filter(|a| a.source == AppSource::Manual).collect();
    let colls = storage.list_collections()?;
    let existing_wf = storage.list_workflows()?;
    let mut p = ImportPreview { schema_version: b.schema_version, ..Default::default() };
    for a in &b.apps {
        let key = security::target_identity(&a.target);
        if manual.iter().any(|m| m.identity_key == key) {
            p.updated_apps += 1;
        } else {
            p.new_apps += 1;
        }
        if !security::fs_exists(&a.target.path) && matches!(a.target.kind, TargetKind::Executable | TargetKind::Shortcut) {
            p.apps_with_missing_target.push(a.name.clone());
        }
    }
    p.new_collections = b.collections.iter().filter(|n| !colls.iter().any(|c| c.name.eq_ignore_ascii_case(n))).count();
    for w in &b.workflows {
        if existing_wf.iter().any(|e| e.id == w.id) {
            p.updated_workflows += 1;
        } else {
            p.new_workflows += 1;
        }
        if has_command(&w.steps) {
            p.workflows_with_commands.push(w.name.clone());
        }
    }
    p.preferences_changed = b.preferences != storage.get_preferences()?;
    if !p.workflows_with_commands.is_empty() {
        p.warnings.push("Some workflows run commands. They will be imported with the 'trusted' flag removed, and will not run unless developer command execution is enabled.".into());
    }
    if !p.apps_with_missing_target.is_empty() {
        p.warnings.push("Some registered programs do not exist on this PC; they will show as missing until you edit their path.".into());
    }
    p.warnings.push("Developer command execution and Start with Windows are never changed by an import.".into());
    Ok(p)
}

pub fn apply_import(storage: &Storage, b: &Bundle) -> Result<ImportPreview> {
    let preview = preview_import(storage, b)?;
    let current = storage.get_preferences()?;
    let mut prefs = b.preferences.clone();
    prefs.developer_command_execution = current.developer_command_execution;
    prefs.start_with_windows = current.start_with_windows;
    prefs.onboarding_done = current.onboarding_done;
    storage.set_preferences(&prefs)?;

    for name in &b.collections {
        if !storage.list_collections()?.iter().any(|c| c.name.eq_ignore_ascii_case(name)) {
            storage.create_collection(name)?;
        }
    }
    for a in &b.apps {
        let key = security::target_identity(&a.target);
        let existing = storage.list_apps(true)?.into_iter().find(|m| m.source == AppSource::Manual && m.identity_key == key);
        let entry = match existing {
            Some(e) => storage.update_app(
                &e.id,
                AppPatch { name: Some(a.name.clone()), category: Some(a.category), tags: Some(a.tags.clone()), favorite: Some(a.favorite), ..Default::default() },
            )?,
            None => {
                let e = storage.add_manual_app_opts(&a.name, a.target.clone(), a.category, false)?;
                storage.update_app(&e.id, AppPatch { tags: Some(a.tags.clone()), favorite: Some(a.favorite), ..Default::default() })?
            }
        };
        for cname in &a.collections {
            if let Some(c) = storage.list_collections()?.iter().find(|c| c.name.eq_ignore_ascii_case(cname)) {
                storage.add_to_collection(&entry.id, &c.id)?;
            }
        }
    }
    for w in &b.workflows {
        let mut w = w.clone();
        strip_trust(&mut w.steps);
        storage.upsert_workflow(&w)?;
    }
    Ok(preview)
}

fn strip_trust(steps: &mut [Step]) {
    for s in steps {
        match &mut s.action {
            Action::RunCommand { trusted, .. } => *trusted = false,
            Action::If { then_steps, else_steps, .. } => {
                strip_trust(then_steps);
                strip_trust(else_steps);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_preview_apply() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("My Tool ü.exe");
        std::fs::write(&exe, b"MZ").unwrap();
        let a = Storage::open_in_memory().unwrap();
        crate::workflow::templates::seed(&a).unwrap();
        let e = a.add_manual_app("My Tool", ExecutableTarget::executable(exe.to_string_lossy()), AppCategory::DeveloperTools).unwrap();
        a.update_app(&e.id, AppPatch { tags: Some(vec!["dev".into()]), favorite: Some(true), ..Default::default() }).unwrap();
        let c = a.create_collection("Work").unwrap();
        a.add_to_collection(&e.id, &c.id).unwrap();
        let json = serde_json::to_string(&export_bundle(&a).unwrap()).unwrap();
        assert!(!json.contains("launchCount") && !json.contains("lastLaunchedAt"), "history must not be exported");

        let b = Storage::open_in_memory().unwrap();
        let bundle = parse_bundle(&json).unwrap();
        let p = preview_import(&b, &bundle).unwrap();
        assert_eq!((p.new_apps, p.new_collections, p.new_workflows), (1, 1, 12));
        assert!(b.list_apps(true).unwrap().is_empty(), "preview must not write");
        apply_import(&b, &bundle).unwrap();
        let apps = b.list_apps(true).unwrap();
        assert_eq!(apps.len(), 1);
        assert!(apps[0].favorite && apps[0].tags == vec!["dev"] && apps[0].collection_ids.len() == 1);
        assert_eq!(b.list_workflows().unwrap().len(), 12);
        let again = apply_import(&b, &bundle).unwrap();
        assert_eq!((again.new_apps, again.updated_apps), (0, 1));
        assert_eq!(b.list_apps(true).unwrap().len(), 1, "idempotent");
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse_bundle("nope").is_err());
        assert!(parse_bundle("{}").is_err());
        assert!(parse_bundle(r#"{"schemaVersion":99,"app":"PulseLauncher"}"#).is_err());
        assert!(parse_bundle(r#"{"schemaVersion":1,"app":"Other"}"#).is_err());
        assert!(parse_bundle(&"x".repeat(5 * 1024 * 1024)).is_err());
        let evil = r#"{"schemaVersion":1,"app":"PulseLauncher","exportedAt":"x","preferences":{},"collections":[],"workflows":[],
            "apps":[{"name":"x","target":{"kind":"uri","path":"","uri":"javascript:alert(1)"}}]}"#;
        assert!(parse_bundle(evil).is_err());
    }

    #[test]
    fn import_strips_trusted_and_protects_settings() {
        let s = Storage::open_in_memory().unwrap();
        let mut w = crate::workflow::templates::template("tpl.open_downloads").unwrap();
        w.id = "custom".into();
        w.template_id = None;
        w.steps = vec![Step::new("c", "cmd", Action::RunCommand { program: "C:\\x.exe".into(), args: vec![], working_dir: None, trusted: true })];
        w.kind = WorkflowKind::Command;
        let bundle = Bundle {
            schema_version: 1,
            app: "PulseLauncher".into(),
            exported_at: "x".into(),
            preferences: UserPreferences { developer_command_execution: true, start_with_windows: true, ..Default::default() },
            apps: vec![],
            collections: vec![],
            workflows: vec![w],
        };
        let json = serde_json::to_string(&bundle).unwrap();
        let parsed = parse_bundle(&json).unwrap();
        let p = preview_import(&s, &parsed).unwrap();
        assert_eq!(p.workflows_with_commands, vec!["Open Downloads"]);
        apply_import(&s, &parsed).unwrap();
        let prefs = s.get_preferences().unwrap();
        assert!(!prefs.developer_command_execution && !prefs.start_with_windows);
        match &s.get_workflow("custom").unwrap().steps[0].action {
            Action::RunCommand { trusted, .. } => assert!(!trusted),
            _ => panic!(),
        }
    }
}
