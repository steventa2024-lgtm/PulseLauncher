use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use super::AppState;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::integrations::{self, OllamaStatus, Probe, Suggestion};
use crate::models::*;
use crate::search::{self, WorkflowSearchItem};
use crate::security;
use crate::workflows::{self, WorkflowDefinition, WorkflowKind};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub platform: &'static str,
    pub windows_native: bool,
    pub data_dir: String,
    pub db_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    pub info: AppInfo,
    pub schema_version: i64,
    pub app_count: usize,
    pub portable_count: usize,
    pub favorite_count: usize,
    pub file_index_count: i64,
    pub last_scan: Option<ScanReport>,
    pub hotkey: HotkeyStatus,
    pub recent_events: Vec<LaunchEvent>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexReport {
    pub indexed: usize,
    pub truncated: bool,
    pub errors: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PortableApp {
    name: String,
    kind: AppKind,
    launch_target: String,
    arguments: Vec<String>,
    working_dir: Option<String>,
    category: Option<String>,
    tags: Vec<String>,
    favorite: bool,
    portable: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportedCollection {
    name: String,
    system_key: Option<String>,
    targets: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Bundle {
    schema: u32,
    exported_at: String,
    preferences: UserPreferences,
    manual_apps: Vec<PortableApp>,
    collections: Vec<ExportedCollection>,
    workflows: Vec<WorkflowDefinition>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub applied: bool,
    pub apps_added: usize,
    pub apps_skipped: Vec<String>,
    pub workflows: usize,
    pub collections: usize,
    pub notes: Vec<String>,
}

fn info(state: &AppState) -> AppInfo {
    AppInfo {
        name: "PulseLauncher",
        version: env!("CARGO_PKG_VERSION"),
        platform: crate::platform::platform_label(),
        windows_native: crate::platform::is_windows(),
        data_dir: state.data_dir.display().to_string(),
        db_path: state.db_path.display().to_string(),
    }
}

#[tauri::command]
pub fn app_info(state: State<AppState>) -> AppInfo {
    info(&state)
}

#[tauri::command]
pub fn get_preferences(state: State<AppState>) -> AppResult<UserPreferences> {
    state.db.preferences()
}

pub(crate) fn validate_preferences(prefs: &UserPreferences) -> AppResult<()> {
    if prefs.theme != "dark" && prefs.theme != "light" {
        return Err(AppError::invalid("Theme must be dark or light"));
    }
    if prefs.density != "comfortable" && prefs.density != "compact" {
        return Err(AppError::invalid("Density must be comfortable or compact"));
    }
    if !(0.85..=1.6).contains(&prefs.text_scale) {
        return Err(AppError::invalid("Text scale must be between 0.85 and 1.6"));
    }
    let accent = prefs.accent.trim();
    let hex = accent.strip_prefix('#').unwrap_or("");
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AppError::invalid("Accent must be a color like #4388FF"));
    }
    crate::shell::parse_accelerator(&prefs.launcher_hotkey)?;
    for fallback in &prefs.launcher_hotkey_fallbacks {
        crate::shell::parse_accelerator(fallback)?;
    }
    if prefs.index_roots.len() > 20 {
        return Err(AppError::invalid("At most 20 indexed folders"));
    }
    for root in &prefs.index_roots {
        let p = security::validate_absolute_path(root, "Indexed folder")?;
        if !p.is_dir() {
            return Err(AppError::new(
                ErrorCode::TargetMissing,
                format!("Indexed folder not found: {}", p.display()),
            ));
        }
    }
    if prefs.web_search_enabled {
        if !prefs.web_search_template.contains("{query}") {
            return Err(AppError::invalid(
                "The web search address must contain {query}",
            ));
        }
        let sample = prefs.web_search_template.replace("{query}", "pulse");
        security::validate_web_url(&sample)?;
    }
    if prefs.ollama_enabled {
        integrations::validate_endpoint(&prefs.ollama_endpoint)?;
    }
    if !prefs.pulseflow_url.trim().is_empty() {
        let url = security::validate_web_url(prefs.pulseflow_url.trim())?;
        if !matches!(
            url.host_str(),
            Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
        ) {
            return Err(AppError::policy(
                "The PulseFlow address must be on this computer (localhost).",
            ));
        }
    }
    Ok(())
}

#[tauri::command]
pub fn save_preferences(app: AppHandle, prefs: UserPreferences) -> AppResult<HotkeyStatus> {
    validate_preferences(&prefs)?;
    let state = app.state::<AppState>();
    state.db.save_preferences(&prefs)?;
    let status = crate::shell::apply_shortcuts(&app);
    *state.hotkey.lock().unwrap_or_else(|p| p.into_inner()) = status.clone();
    if let Err(e) = crate::shell::apply_autostart(&app, prefs.launch_at_startup) {
        let mut current = state.hotkey.lock().unwrap_or_else(|p| p.into_inner());
        let extra = format!("Startup setting was not changed: {e}");
        current.warning = Some(match &current.warning {
            Some(w) => format!("{w} {extra}"),
            None => extra,
        });
        return Ok(current.clone());
    }
    Ok(status)
}

#[tauri::command]
pub fn search(state: State<AppState>, query: String) -> AppResult<Vec<search::SearchResult>> {
    if query.chars().count() > 300 {
        return Err(AppError::invalid("Search text is too long"));
    }
    let prefs = state.db.preferences()?;
    let apps = state.db.list_apps(false)?;
    let files = if prefs.search_files {
        state.db.file_index_entries()?
    } else {
        vec![]
    };
    let workflows = state
        .db
        .list_workflows()?
        .into_iter()
        .map(|w| {
            let apps_ids: std::collections::HashSet<String> =
                apps.iter().map(|a| a.id.clone()).collect();
            let ready = workflows::readiness(&w, &workflows::Facts::current(), &apps_ids);
            WorkflowSearchItem {
                id: w.id,
                name: w.name,
                description: w.description,
                is_command: w.kind == WorkflowKind::Command,
                needs_setup: ready.state != workflows::ReadinessState::Ready,
            }
        })
        .collect::<Vec<_>>();
    Ok(search::search(
        &query,
        &search::SearchContext {
            apps: &apps,
            workflows: &workflows,
            files: &files,
            prefs: &prefs,
            limit: 40,
        },
    ))
}

#[tauri::command]
pub fn open_url(url: String) -> AppResult<LaunchOutcome> {
    crate::launch::open_url(&url)
}

#[tauri::command]
pub fn open_indexed(state: State<AppState>, path: String) -> AppResult<LaunchOutcome> {
    if !state.db.is_indexed_path(&path)? {
        return Err(AppError::policy(
            "That path is not in the folders you chose to index.",
        ));
    }
    if std::path::Path::new(&path).is_dir() {
        crate::launch::open_folder(&path)
    } else {
        crate::launch::open_file(&path)
    }
}

#[tauri::command]
pub fn copy_text(app: AppHandle, text: String) -> AppResult<()> {
    if text.len() > 100_000 {
        return Err(AppError::invalid("Text is too long to copy"));
    }
    use tauri_plugin_clipboard_manager::ClipboardExt;
    app.clipboard()
        .write_text(text)
        .map_err(|e| AppError::internal(e.to_string()))
}

#[tauri::command]
pub fn clear_history(state: State<AppState>) -> AppResult<()> {
    state.db.clear_history()
}

#[tauri::command]
pub fn reindex_files(state: State<AppState>) -> AppResult<IndexReport> {
    let prefs = state.db.preferences()?;
    if !prefs.search_files {
        state.db.replace_file_index(&[])?;
        return Ok(IndexReport {
            indexed: 0,
            truncated: false,
            errors: vec!["File search is off, so the index was cleared.".into()],
        });
    }
    let cancel = AtomicBool::new(false);
    let outcome = search::files::build_index(
        &prefs.index_roots,
        &prefs.index_exclusions,
        prefs.index_max_entries,
        &cancel,
    );
    state.db.replace_file_index(&outcome.entries)?;
    Ok(IndexReport {
        indexed: outcome.entries.len(),
        truncated: outcome.truncated,
        errors: outcome.errors,
    })
}

#[tauri::command]
pub fn diagnostics(state: State<AppState>) -> AppResult<Diagnostics> {
    let apps = state.db.list_apps(false)?;
    Ok(Diagnostics {
        info: info(&state),
        schema_version: state.db.schema_version()?,
        app_count: apps.len(),
        portable_count: apps
            .iter()
            .filter(|a| a.portable || a.source == DiscoveredSource::Manual)
            .count(),
        favorite_count: apps.iter().filter(|a| a.favorite).count(),
        file_index_count: state.db.file_index_count()?,
        last_scan: state.db.last_scan()?,
        hotkey: state
            .hotkey
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone(),
        recent_events: state.db.launch_events(None, 40)?,
    })
}

#[tauri::command]
pub fn environment() -> workflows::Facts {
    workflows::Facts::current()
}

#[tauri::command]
pub fn ollama_status(state: State<AppState>) -> AppResult<OllamaStatus> {
    let prefs = state.db.preferences()?;
    Ok(integrations::ollama_status(
        prefs.ollama_enabled,
        &prefs.ollama_endpoint,
    ))
}

#[tauri::command]
pub fn ollama_suggest(state: State<AppState>, prompt: String) -> AppResult<Suggestion> {
    let prefs = state.db.preferences()?;
    if !prefs.ollama_enabled {
        return Err(AppError::policy("Local AI is turned off in Settings."));
    }
    integrations::ollama_suggest(&prefs.ollama_endpoint, &prefs.ollama_model, &prompt)
}

#[tauri::command]
pub fn pulseflow_status(state: State<AppState>) -> AppResult<Probe> {
    Ok(integrations::pulseflow_probe(
        &state.db.preferences()?.pulseflow_url,
    ))
}

#[tauri::command]
pub fn show_launcher(app: AppHandle) {
    crate::shell::show_launcher(&app);
}

#[tauri::command]
pub fn hide_launcher(app: AppHandle) {
    crate::shell::hide_launcher(&app);
}

#[tauri::command]
pub fn show_main(app: AppHandle) {
    crate::shell::show_main(&app);
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub fn export_bundle(state: State<AppState>) -> AppResult<String> {
    let prefs = state.db.preferences()?;
    let manual_apps = state
        .db
        .list_apps(true)?
        .into_iter()
        .filter(|a| a.source == DiscoveredSource::Manual)
        .map(|a| PortableApp {
            name: a.name,
            kind: a.target.kind,
            launch_target: a.target.launch_target,
            arguments: a.target.arguments,
            working_dir: a.target.working_dir,
            category: a.category,
            tags: a.tags,
            favorite: a.favorite,
            portable: a.portable,
        })
        .collect();
    let apps = state.db.list_apps(true)?;
    let collections = state
        .db
        .list_collections()?
        .into_iter()
        .map(|c| ExportedCollection {
            targets: c
                .app_ids
                .iter()
                .filter_map(|id| {
                    apps.iter()
                        .find(|a| &a.id == id)
                        .map(|a| a.target.launch_target.clone())
                })
                .collect(),
            name: c.name,
            system_key: c.system_key,
        })
        .collect();
    let bundle = Bundle {
        schema: 1,
        exported_at: crate::storage::now(),
        preferences: prefs,
        manual_apps,
        collections,
        workflows: state.db.list_workflows()?,
    };
    Ok(serde_json::to_string_pretty(&bundle)?)
}

#[tauri::command]
pub fn import_bundle(app: AppHandle, json: String, apply: bool) -> AppResult<ImportReport> {
    if json.len() > 2 * 1024 * 1024 {
        return Err(AppError::invalid("Import file is larger than 2 MB"));
    }
    let bundle: Bundle = serde_json::from_str(&json)?;
    if bundle.schema != 1 {
        return Err(AppError::invalid(
            "This export was made by a newer PulseLauncher and cannot be read.",
        ));
    }
    validate_preferences(&bundle.preferences)?;
    let mut report = ImportReport {
        applied: apply,
        apps_added: 0,
        apps_skipped: vec![],
        workflows: 0,
        collections: 0,
        notes: vec![],
    };
    if !apply {
        report.notes.push(format!(
            "Would import {} preferences, {} manual apps, {} collections and {} workflows.",
            1,
            bundle.manual_apps.len(),
            bundle.collections.len(),
            bundle.workflows.len()
        ));
        return Ok(report);
    }
    let state = app.state::<AppState>();
    state.db.save_preferences(&bundle.preferences)?;
    let status = crate::shell::apply_shortcuts(&app);
    *state.hotkey.lock().unwrap_or_else(|p| p.into_inner()) = status;
    for app_def in bundle.manual_apps {
        match state.db.add_manual_app(crate::storage::apps::NewManualApp {
            name: app_def.name.clone(),
            kind: app_def.kind,
            launch_target: app_def.launch_target,
            arguments: app_def.arguments,
            working_dir: app_def.working_dir,
            icon_path: None,
            category: app_def.category,
            portable: app_def.portable,
            tags: app_def.tags,
        }) {
            Ok(saved) => {
                if app_def.favorite {
                    let _ = state.db.update_app(
                        &saved.id,
                        crate::storage::apps::AppUpdate {
                            favorite: Some(true),
                            ..Default::default()
                        },
                    );
                }
                report.apps_added += 1;
            }
            Err(e) => report
                .apps_skipped
                .push(format!("{}: {}", app_def.name, e.message)),
        }
    }
    let apps = state.db.list_apps(true)?;
    for collection in bundle.collections {
        let id = if let Some(key) = &collection.system_key {
            state.db.ensure_system_collection(key, &collection.name)?;
            state
                .db
                .list_collections()?
                .into_iter()
                .find(|c| c.system_key.as_ref() == Some(key))
                .map(|c| c.id)
        } else if let Some(existing) = state
            .db
            .list_collections()?
            .into_iter()
            .find(|c| c.name == collection.name)
        {
            Some(existing.id)
        } else {
            Some(state.db.create_collection(&collection.name, None)?.id)
        };
        if let Some(id) = id {
            for target in &collection.targets {
                if let Some(app) = apps.iter().find(|a| &a.target.launch_target == target) {
                    state.db.set_collection_membership(&id, &app.id, true)?;
                }
            }
            report.collections += 1;
        }
    }
    for mut wf in bundle.workflows {
        wf.trusted = false;
        if let Some(key) = &wf.template_key {
            if let Some(existing) = state
                .db
                .list_workflows()?
                .into_iter()
                .find(|w| w.template_key.as_ref() == Some(key))
            {
                let apps_ids = apps.iter().map(|a| a.id.clone()).collect();
                if workflows::validate_values(&existing, &wf.values, &apps_ids).is_ok() {
                    state
                        .db
                        .update_workflow_values(&existing.id, wf.values, &apps_ids)?;
                    report.workflows += 1;
                } else {
                    report.notes.push(format!(
                        "Skipped settings for template {key}: a value did not validate."
                    ));
                }
                continue;
            }
        }
        wf.id = uuid::Uuid::new_v4().to_string();
        wf.template_key = None;
        wf.template_version = None;
        match state.db.save_workflow(&wf) {
            Ok(()) => report.workflows += 1,
            Err(e) => report
                .notes
                .push(format!("Skipped workflow {}: {}", wf.name, e.message)),
        }
    }
    Ok(report)
}
