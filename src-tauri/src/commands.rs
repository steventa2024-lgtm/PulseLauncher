//! Narrow command surface. There is no command that executes an arbitrary string.

use crate::host;
use crate::state::{self, AppState, ShortcutStatus};
use pulse_core::discovery::scan_start_menu;
use pulse_core::error::{CoreError, Result};
use pulse_core::fileindex;
use pulse_core::import_export::{self, ImportPreview};
use pulse_core::launch::{self, app_status};
use pulse_core::models::*;
use pulse_core::platform::Platform;
use pulse_core::search::{self, SearchInputs, SearchResult};
use pulse_core::security;
use pulse_core::storage::{now_iso, AppPatch, Storage};
use pulse_core::workflow::engine::{check_prerequisites, effective_values, readiness, validate_definition};
use pulse_core::workflow::model::{WorkflowDefinition, WorkflowRun, WorkflowStatus};
use pulse_core::workflow::templates::{self, ZEROPULSE_COLLECTION};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppDetails {
    pub app: AppEntry,
    pub status: TargetStatus,
    pub metadata: AppMetadata,
    pub events: Vec<LaunchEvent>,
    pub previews: Vec<pulse_core::models::PreviewAsset>,
    pub duplicates: Vec<AppEntry>,
    pub icon_data_url: Option<String>,
    pub command_preview: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateApp {
    pub name: Option<String>,
    pub path: Option<String>,
    pub args: Option<Vec<String>>,
    pub working_dir: Option<String>,
    pub category: Option<AppCategory>,
    pub tags: Option<Vec<String>>,
    pub favorite: Option<bool>,
    pub hidden: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowCard {
    pub definition: WorkflowDefinition,
    pub status: WorkflowStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    pub version: String,
    pub platform: String,
    pub data_dir: String,
    pub db_path: String,
    pub schema_version: u32,
    pub app_count: usize,
    pub indexed_files: i64,
    pub last_scan: Option<String>,
    pub shortcut: ShortcutStatus,
    pub packaged_apps_supported: bool,
    pub developer_commands: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaStatus {
    pub enabled: bool,
    pub reachable: bool,
    pub models: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationSlot {
    pub label: String,
    pub registered_app_id: Option<String>,
    pub registered_name: Option<String>,
    pub endpoint: String,
    pub endpoint_ok: Option<bool>,
    pub detail: String,
}

fn icon_for(state: &AppState, app: &AppEntry) -> Option<String> {
    let cache = state.data_dir.join("icons").join(format!("{}.png", app.id));
    if let Ok(bytes) = std::fs::read(&cache) {
        if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
            return Some(state::data_url(&bytes, "image/png"));
        }
    }
    let source = app.resolved_path.clone().filter(|p| security::fs_exists(p)).unwrap_or_else(|| app.target.path.clone());
    let png = state.platform.icon_png(&source).or_else(|| state.platform.icon_png(&app.target.path))?;
    if let Some(dir) = cache.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&cache, &png);
    Some(state::data_url(&png, "image/png"))
}

fn card_from(state: &AppState, def: &WorkflowDefinition) -> Result<WorkflowCard> {
    let values = effective_values(def, &BTreeMap::new());
    let checker = StatusHost { state: state.clone() };
    let prerequisites = check_prerequisites(def, &values, &checker);
    let last_run = state.storage.list_runs(Some(&def.id), 1)?.into_iter().next();
    Ok(WorkflowCard {
        definition: def.clone(),
        status: WorkflowStatus { state: readiness(&prerequisites), prerequisites, last_run },
    })
}

/// Prerequisite checks only. Launch methods are unreachable here.
struct StatusHost {
    state: AppState,
}

impl pulse_core::workflow::engine::WorkflowHost for StatusHost {
    fn launch_app(&self, _: &str, _: &[String]) -> Result<LaunchResult> {
        Err(CoreError::Unsupported("status check does not launch".into()))
    }
    fn launch_path(&self, _: &str, _: &[String], _: Option<&str>) -> Result<LaunchResult> {
        Err(CoreError::Unsupported("status check does not launch".into()))
    }
    fn open_path(&self, _: &str) -> Result<()> {
        Err(CoreError::Unsupported("status check does not open files".into()))
    }
    fn open_url(&self, _: &str) -> Result<()> {
        Err(CoreError::Unsupported("status check does not open URLs".into()))
    }
    fn open_settings_uri(&self, _: &str) -> Result<()> {
        Err(CoreError::Unsupported("status check does not open settings".into()))
    }
    fn copy_text(&self, _: &str) -> Result<()> {
        Err(CoreError::Unsupported("status check does not use the clipboard".into()))
    }
    fn notify(&self, _: &str, _: &str) -> Result<()> {
        Err(CoreError::Unsupported("status check does not notify".into()))
    }
    fn confirm(&self, _: &str, _: std::time::Duration, _: &pulse_core::workflow::engine::CancelToken) -> Result<bool> {
        Ok(false)
    }
    fn app_exists(&self, app_id: &str) -> bool {
        self.state.storage.get_app(app_id).is_ok()
    }
    fn collection_apps(&self, system_key: &str) -> Result<Vec<(String, String)>> {
        let cols = self.state.storage.list_collections()?;
        let Some(col) = cols.into_iter().find(|c| c.system_key.as_deref() == Some(system_key)) else {
            return Ok(vec![]);
        };
        Ok(self
            .state
            .storage
            .list_apps(false)?
            .into_iter()
            .filter(|a| a.collection_ids.iter().any(|id| id == &col.id))
            .map(|a| (a.id, a.name))
            .collect())
    }
    fn developer_commands_enabled(&self) -> bool {
        false
    }
    fn known_folder(&self, name: &str) -> Option<String> {
        self.state.platform.known_folder(name).map(|p| p.to_string_lossy().into_owned())
    }
    fn path_exists(&self, path: &str) -> bool {
        security::fs_exists(path)
    }
}

#[tauri::command]
pub fn list_apps(state: State<AppState>, include_hidden: bool) -> Result<Vec<AppEntry>> {
    state.storage.list_apps(include_hidden)
}

#[tauri::command]
pub fn app_details(state: State<AppState>, id: String) -> Result<AppDetails> {
    let app = state.storage.get_app(&id)?;
    let meta_path = app.resolved_path.clone().unwrap_or_else(|| app.target.path.clone());
    let mut metadata = if meta_path.is_empty() { AppMetadata::default() } else { state.platform.file_metadata(&meta_path) };
    if metadata.version.is_none() {
        metadata.version = app.version.clone();
    }
    if metadata.publisher.is_none() {
        metadata.publisher = app.publisher.clone();
    }
    if let (Some(v), true) = (metadata.version.clone(), app.version.is_none()) {
        let _ = state.storage.set_app_metadata(&app.id, metadata.publisher.as_deref(), Some(&v));
    }
    Ok(AppDetails {
        status: app_status(&app),
        metadata,
        events: state.storage.launch_events(Some(&id), 30)?,
        previews: state.storage.list_preview_assets(&id)?,
        duplicates: state.storage.find_duplicates_of(&id)?,
        icon_data_url: icon_for(&state, &app),
        command_preview: security::command_preview(&app.target),
        app,
    })
}

#[tauri::command]
pub fn search_apps(state: State<AppState>, query: String) -> Result<Vec<SearchResult>> {
    let prefs = state.storage.get_preferences()?;
    let apps = state.storage.list_apps(false)?;
    let workflows = state.storage.list_workflows()?;
    let files = if prefs.index_files { state.storage.list_indexed_files()? } else { vec![] };
    let inp = SearchInputs {
        apps: &apps,
        workflows: &workflows,
        files: &files,
        files_enabled: prefs.index_files,
        web_enabled: prefs.show_web_search,
        web_url_template: &prefs.web_search_url,
        now: chrono::Utc::now(),
    };
    Ok(search::search(&inp, &query, 30))
}

#[tauri::command]
pub fn launch_app(state: State<AppState>, id: String, extra_args: Option<Vec<String>>) -> Result<LaunchResult> {
    let prefs = state.storage.get_preferences()?;
    launch::launch_app(&state.storage, state.platform.as_ref(), &id, extra_args.as_deref().unwrap_or(&[]), prefs.record_history)
}

#[tauri::command]
pub fn open_path(state: State<AppState>, path: String) -> Result<()> {
    if !security::is_absolute_any(&path) || !security::fs_exists(&path) {
        return Err(CoreError::NotFound(format!("not found: {path}")));
    }
    state.platform.open_path(&path)
}

#[tauri::command]
pub fn open_url(state: State<AppState>, url: String) -> Result<()> {
    let url = security::validate_web_url(&url)?;
    state.platform.open_url(url.as_str())
}

#[tauri::command]
pub fn reveal_path(state: State<AppState>, path: String) -> Result<()> {
    state.platform.reveal_in_folder(&path)
}

#[tauri::command]
pub fn copy_text(text: String) -> Result<()> {
    if text.len() > 1024 * 1024 {
        return Err(CoreError::invalid("text is too large to copy"));
    }
    let mut clip = arboard::Clipboard::new().map_err(|e| CoreError::Launch(format!("clipboard unavailable: {e}")))?;
    clip.set_text(text).map_err(|e| CoreError::Launch(format!("could not copy: {e}")))
}

#[tauri::command]
pub fn add_app(state: State<AppState>, name: String, path: String, args: Option<Vec<String>>, working_dir: Option<String>, category: Option<AppCategory>) -> Result<AppEntry> {
    let ext = std::path::Path::new(&path).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let kind = if ext == "lnk" { TargetKind::Shortcut } else { TargetKind::Executable };
    let target = ExecutableTarget {
        kind,
        path,
        args: args.unwrap_or_default(),
        working_dir: working_dir.filter(|w| !w.is_empty()),
        aumid: None,
        uri: None,
    };
    let entry = state.storage.add_manual_app(&name, target, category.unwrap_or(AppCategory::None))?;
    if entry.target.kind == TargetKind::Shortcut {
        if let Ok(bytes) = std::fs::read(&entry.target.path) {
            if let Ok(link) = pulse_core::lnk::parse(&bytes) {
                if let Some(resolved) = pulse_core::lnk::resolve_target(&link, std::path::Path::new(&entry.target.path)) {
                    let _ = state.storage.set_resolved_path(&entry.id, Some(&resolved));
                }
            }
        }
    }
    state.storage.get_app(&entry.id)
}

#[tauri::command]
pub fn update_app(state: State<AppState>, id: String, patch: UpdateApp) -> Result<AppEntry> {
    let current = state.storage.get_app(&id)?;
    let target = if patch.path.is_some() || patch.args.is_some() || patch.working_dir.is_some() {
        let mut t = current.target.clone();
        if let Some(p) = patch.path {
            t.path = p;
        }
        if let Some(a) = patch.args {
            t.args = a;
        }
        if let Some(w) = patch.working_dir {
            t.working_dir = if w.is_empty() { None } else { Some(w) };
        }
        Some(t)
    } else {
        None
    };
    let updated = state.storage.update_app(
        &id,
        AppPatch { name: patch.name, target, category: patch.category, tags: patch.tags, favorite: patch.favorite, hidden: patch.hidden },
    )?;
    let cache = state.data_dir.join("icons").join(format!("{id}.png"));
    let _ = std::fs::remove_file(cache);
    Ok(updated)
}

#[tauri::command]
pub fn remove_app(state: State<AppState>, id: String) -> Result<bool> {
    let deleted = state.storage.remove_app(&id)?;
    if deleted {
        let _ = std::fs::remove_file(state.data_dir.join("icons").join(format!("{id}.png")));
    }
    Ok(deleted)
}

#[tauri::command]
pub fn reorder_apps(state: State<AppState>, ids: Vec<String>) -> Result<()> {
    state.storage.reorder_apps(&ids)
}

pub fn perform_scan(state: &AppState) -> Result<ScanReport> {
    let started = Instant::now();
    let prefs = state.storage.get_preferences()?;
    let mut roots = state.platform.start_menu_roots();
    if prefs.desktop_shortcuts_scan {
        if let Some(desk) = state.platform.known_folder("desktop") {
            roots.push((desk, AppSource::Desktop));
        }
    }
    let mut scanned: Vec<AppSource> = roots.iter().map(|(_, s)| *s).collect();
    let mut output = scan_start_menu(&roots);
    if state.platform.supports_packaged_apps() {
        match state.platform.scan_packaged_apps() {
            Ok(apps) => {
                output.apps.extend(apps);
                scanned.push(AppSource::Packaged);
            }
            Err(e) => output.issues.push(ScanIssue { path: "packaged apps".into(), reason: e.to_string() }),
        }
    }
    let (added, updated, stale) = state.storage.reconcile_scan(&output.apps, &scanned)?;
    let report = ScanReport {
        found: output.apps.len(),
        added,
        updated,
        marked_stale: stale,
        duplicates_merged: output.duplicates_merged,
        skipped_uninstallers: output.skipped_uninstallers,
        skipped_no_target: output.skipped_no_target,
        issues: output.issues,
        scanned_roots: output.scanned_roots,
        finished_at: now_iso(),
        duration_ms: started.elapsed().as_millis() as u64,
    };
    let _ = state.storage.set_meta("last_scan", &serde_json::to_string(&report).unwrap_or_default());
    state::append_log(state, &format!("scan finished: {} found, {} added, {} updated", report.found, report.added, report.updated));
    Ok(report)
}

#[tauri::command]
pub fn scan(state: State<AppState>) -> Result<ScanReport> {
    perform_scan(&state)
}

#[tauri::command]
pub fn list_collections(state: State<AppState>) -> Result<Vec<Collection>> {
    state.storage.list_collections()
}

#[tauri::command]
pub fn create_collection(state: State<AppState>, name: String) -> Result<Collection> {
    state.storage.create_collection(&name)
}

#[tauri::command]
pub fn rename_collection(state: State<AppState>, id: String, name: String) -> Result<Collection> {
    state.storage.rename_collection(&id, &name)
}

#[tauri::command]
pub fn delete_collection(state: State<AppState>, id: String) -> Result<()> {
    state.storage.delete_collection(&id)
}

#[tauri::command]
pub fn set_membership(state: State<AppState>, app_id: String, collection_id: String, member: bool) -> Result<()> {
    if member {
        state.storage.add_to_collection(&app_id, &collection_id)
    } else {
        state.storage.remove_from_collection(&app_id, &collection_id)
    }
}

fn image_ext(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']) {
        Some("png")
    } else if bytes.len() > 3 && bytes[0] == 0xff && bytes[1] == 0xd8 && bytes[2] == 0xff {
        Some("jpg")
    } else {
        None
    }
}

#[tauri::command]
pub fn import_preview(state: State<AppState>, app_id: String, source_path: String) -> Result<pulse_core::models::PreviewAsset> {
    if !security::is_absolute_any(&source_path) {
        return Err(CoreError::invalid("choose a file on this computer"));
    }
    let bytes = std::fs::read(&source_path)?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(CoreError::invalid("image is larger than 8 MB"));
    }
    let ext = image_ext(&bytes).ok_or_else(|| CoreError::invalid("only PNG and JPEG images can be used as previews"))?;
    let name = format!("{}.{}", pulse_core::storage::new_id(), ext);
    let dir = state.data_dir.join("previews");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(&name), &bytes)?;
    state.storage.add_preview_asset(&app_id, &name, None, "user import")
}

#[tauri::command]
pub fn delete_preview(state: State<AppState>, id: String) -> Result<()> {
    if let Some(asset) = state.storage.delete_preview_asset(&id)? {
        let _ = std::fs::remove_file(state.data_dir.join("previews").join(&asset.file_name));
    }
    Ok(())
}

#[tauri::command]
pub fn preview_data(state: State<AppState>, id: String) -> Result<Option<String>> {
    find_preview(&state, &id)
}

fn find_preview(state: &AppState, id: &str) -> Result<Option<String>> {
    let apps = state.storage.list_apps(true)?;
    for app in apps {
        for asset in state.storage.list_preview_assets(&app.id)? {
            if asset.id == id {
                let path = state.data_dir.join("previews").join(&asset.file_name);
                let bytes = std::fs::read(&path)?;
                let mime = if asset.file_name.ends_with(".png") { "image/png" } else { "image/jpeg" };
                return Ok(Some(state::data_url(&bytes, mime)));
            }
        }
    }
    Ok(None)
}

#[tauri::command]
pub fn list_workflows(state: State<AppState>) -> Result<Vec<WorkflowCard>> {
    state.storage.list_workflows()?.iter().map(|w| card_from(&state, w)).collect()
}

#[tauri::command]
pub fn workflow_status(state: State<AppState>, id: String) -> Result<WorkflowCard> {
    let def = state.storage.get_workflow(&id)?;
    card_from(&state, &def)
}

#[tauri::command]
pub fn save_workflow(state: State<AppState>, definition: WorkflowDefinition) -> Result<()> {
    validate_definition(&definition)?;
    if definition.template_id.is_some() {
        let existing = state.storage.get_workflow(&definition.id)?;
        if existing.template_id != definition.template_id {
            return Err(CoreError::Security("template identity cannot be reassigned".into()));
        }
    }
    state.storage.upsert_workflow(&definition)
}

#[tauri::command]
pub fn reset_template(state: State<AppState>, template_id: String) -> Result<WorkflowDefinition> {
    templates::reset(&state.storage, &template_id)
}

#[tauri::command]
pub fn delete_workflow(state: State<AppState>, id: String) -> Result<()> {
    let w = state.storage.get_workflow(&id)?;
    if w.template_id.is_some() {
        return Err(CoreError::Unsupported("built-in templates can be reset, not deleted".into()));
    }
    state.storage.delete_workflow(&id)
}

#[tauri::command]
pub async fn run_workflow(app: AppHandle, state: State<'_, AppState>, id: String, values: BTreeMap<String, String>) -> Result<WorkflowRun> {
    let state = state.inner().clone();
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || host::execute(&app2, &state, &id, values))
        .await
        .map_err(|e| CoreError::Launch(format!("workflow task stopped: {e}")))?
}

#[tauri::command]
pub fn cancel_workflow(state: State<AppState>) -> Result<()> {
    if let Some(token) = state.cancel.lock().unwrap_or_else(|p| p.into_inner()).as_ref() {
        token.cancel();
        Ok(())
    } else {
        Err(CoreError::invalid("no workflow is running"))
    }
}

#[tauri::command]
pub fn confirm_workflow(state: State<AppState>, accepted: bool) -> Result<()> {
    host::answer_confirm(&state, accepted);
    Ok(())
}

#[tauri::command]
pub fn list_runs(state: State<AppState>, workflow_id: Option<String>) -> Result<Vec<WorkflowRun>> {
    state.storage.list_runs(workflow_id.as_deref(), 40)
}

#[tauri::command]
pub fn get_preferences(state: State<AppState>) -> Result<UserPreferences> {
    state.storage.get_preferences()
}

#[tauri::command]
pub fn set_preferences(app: AppHandle, state: State<AppState>, prefs: UserPreferences) -> Result<UserPreferences> {
    if !prefs.web_search_url.contains("{query}") || security::validate_web_url(&prefs.web_search_url.replace("{query}", "test")).is_err() {
        return Err(CoreError::invalid("web search URL must be http(s) and contain {query}"));
    }
    if prefs.ollama_enabled {
        security::validate_loopback_url(&prefs.ollama_endpoint)?;
    }
    if !prefs.pulseflow_endpoint.is_empty() {
        security::validate_loopback_url(&prefs.pulseflow_endpoint)?;
    }
    if !prefs.openpulse_url.is_empty() {
        security::validate_web_url(&prefs.openpulse_url)?;
    }
    let accent_ok = prefs.accent.len() == 7 && prefs.accent.starts_with('#') && prefs.accent.chars().skip(1).all(|c| c.is_ascii_hexdigit());
    if !accent_ok {
        return Err(CoreError::invalid("accent must be a #RRGGBB color"));
    }
    if !(0.85..=1.6).contains(&prefs.text_scale) {
        return Err(CoreError::invalid("text scale is out of range"));
    }
    state.storage.set_preferences(&prefs)?;
    crate::shell::apply_shortcut(&app, &state, &prefs.launcher_shortcut);
    crate::shell::apply_autostart(&app, prefs.start_with_windows);
    if prefs.index_files {
        let _ = reindex_files(&state, &prefs);
    } else if state.storage.count_indexed_files().unwrap_or(0) > 0 {
        let _ = state.storage.clear_file_index();
    }
    state.storage.get_preferences()
}

fn reindex_files(state: &AppState, prefs: &UserPreferences) -> Result<()> {
    state.storage.clear_file_index()?;
    for root in &prefs.index_roots {
        let path = PathBuf::from(root);
        match fileindex::index_root(&path, &prefs.index_exclusions) {
            Ok((files, report)) => {
                state.storage.replace_indexed_files(root, &files)?;
                state::append_log(state, &format!("indexed {root}: {} entries{}", report.indexed, if report.truncated { " (truncated)" } else { "" }));
            }
            Err(e) => state::append_log(state, &format!("index {root}: {e}")),
        }
    }
    Ok(())
}

#[tauri::command]
pub fn reindex(state: State<AppState>) -> Result<i64> {
    let prefs = state.storage.get_preferences()?;
    if !prefs.index_files {
        return Err(CoreError::invalid("file indexing is turned off"));
    }
    reindex_files(&state, &prefs)?;
    state.storage.count_indexed_files()
}

#[tauri::command]
pub fn export_backup(state: State<AppState>) -> Result<String> {
    Ok(serde_json::to_string_pretty(&import_export::export_bundle(&state.storage)?)?)
}

#[tauri::command]
pub fn preview_backup(state: State<AppState>, json: String) -> Result<ImportPreview> {
    let bundle = import_export::parse_bundle(&json)?;
    import_export::preview_import(&state.storage, &bundle)
}

#[tauri::command]
pub fn apply_backup(state: State<AppState>, json: String) -> Result<ImportPreview> {
    let bundle = import_export::parse_bundle(&json)?;
    import_export::apply_import(&state.storage, &bundle)
}

#[tauri::command]
pub fn clear_history(state: State<AppState>) -> Result<usize> {
    state.storage.clear_history()
}

#[tauri::command]
pub fn diagnostics(state: State<AppState>) -> Result<Diagnostics> {
    let prefs = state.storage.get_preferences()?;
    Ok(Diagnostics {
        version: env!("CARGO_PKG_VERSION").into(),
        platform: state.platform.name().into(),
        data_dir: state.data_dir.display().to_string(),
        db_path: state.data_dir.join("pulse.sqlite").display().to_string(),
        schema_version: state.storage.schema_version()?,
        app_count: state.storage.list_apps(true)?.len(),
        indexed_files: state.storage.count_indexed_files()?,
        last_scan: state.storage.get_meta("last_scan")?,
        shortcut: state.shortcut.lock().unwrap_or_else(|p| p.into_inner()).clone(),
        packaged_apps_supported: state.platform.supports_packaged_apps(),
        developer_commands: prefs.developer_command_execution,
    })
}

#[tauri::command]
pub fn read_log(state: State<AppState>) -> Result<String> {
    match std::fs::read_to_string(&state.log_path) {
        Ok(text) => {
            let lines: Vec<&str> = text.lines().collect();
            let start = lines.len().saturating_sub(200);
            Ok(lines[start..].join("\n"))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.into()),
    }
}

#[tauri::command]
pub fn ollama_status(state: State<AppState>) -> Result<OllamaStatus> {
    let prefs = state.storage.get_preferences()?;
    if !prefs.ollama_enabled {
        return Ok(OllamaStatus {
            enabled: false,
            reachable: false,
            models: vec![],
            message: "Local AI is off. Search, launch and workflows do not use it.".into(),
        });
    }
    let base = security::validate_loopback_url(&prefs.ollama_endpoint)?;
    let url = format!("{}api/tags", base.as_str().trim_end_matches('/').to_string() + "/");
    match ureq::get(&url).timeout(std::time::Duration::from_secs(3)).call() {
        Ok(resp) => {
            let body = resp.into_string().unwrap_or_default();
            let parsed: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
            let models: Vec<String> = parsed
                .get("models")
                .and_then(|m| m.as_array())
                .map(|arr| arr.iter().filter_map(|m| m.get("name").and_then(|n| n.as_str()).map(str::to_string)).collect())
                .unwrap_or_default();
            let message = if prefs.ollama_model.is_empty() {
                "Connected. Choose a model before asking for an explanation. Nothing runs automatically.".into()
            } else if models.iter().any(|m| m == &prefs.ollama_model) {
                format!("Connected. Model {} is installed. Suggestions still need your approval.", prefs.ollama_model)
            } else {
                format!("Connected, but model '{}' was not reported by Ollama.", prefs.ollama_model)
            };
            Ok(OllamaStatus { enabled: true, reachable: true, models, message })
        }
        Err(e) => Ok(OllamaStatus {
            enabled: true,
            reachable: false,
            models: vec![],
            message: format!("Could not reach Ollama at {}: {e}", prefs.ollama_endpoint),
        }),
    }
}

#[tauri::command]
pub fn explain_with_ollama(state: State<AppState>, prompt: String) -> Result<String> {
    if prompt.trim().is_empty() || prompt.len() > 4_000 {
        return Err(CoreError::invalid("prompt is empty or too long"));
    }
    let prefs = state.storage.get_preferences()?;
    if !prefs.ollama_enabled {
        return Err(CoreError::Unsupported("local AI is turned off".into()));
    }
    if prefs.ollama_model.is_empty() {
        return Err(CoreError::invalid("choose an Ollama model first"));
    }
    let base = security::validate_loopback_url(&prefs.ollama_endpoint)?;
    let url = format!("{}api/generate", base.as_str().trim_end_matches('/').to_string() + "/");
    let body = serde_json::json!({
        "model": prefs.ollama_model,
        "prompt": format!("Explain this launcher request in plain language. Do not claim you executed anything.\n\n{prompt}"),
        "stream": false
    });
    let resp = ureq::post(&url)
        .timeout(std::time::Duration::from_secs(30))
        .send_json(body)
        .map_err(|e| CoreError::Launch(format!("Ollama request failed: {e}")))?;
    let parsed: serde_json::Value = resp.into_json().map_err(|e| CoreError::Launch(format!("Ollama response was not JSON: {e}")))?;
    let text = parsed.get("response").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if text.is_empty() {
        return Err(CoreError::Launch("Ollama returned an empty explanation".into()));
    }
    Ok(security::redact(&text))
}

fn slot(label: &str, apps: &[AppEntry], needle: &str, endpoint: &str, probe: bool) -> IntegrationSlot {
    let found = apps.iter().find(|a| a.name.to_lowercase().contains(&needle.to_lowercase()) && !a.hidden);
    let (endpoint_ok, detail) = if endpoint.is_empty() {
        (None, "No local endpoint configured.".into())
    } else if !probe {
        (None, "Endpoint is saved. It is not contacted until you test it.".into())
    } else {
        match security::validate_loopback_url(endpoint).or_else(|_| security::validate_web_url(endpoint)) {
            Ok(url) => match ureq::get(url.as_str()).timeout(std::time::Duration::from_secs(2)).call() {
                Ok(resp) => (Some(true), format!("HTTP {} from {}. This does not mean a workflow ran.", resp.status(), url)),
                Err(e) => (Some(false), format!("No response from {url}: {e}")),
            },
            Err(e) => (Some(false), e.to_string()),
        }
    };
    IntegrationSlot {
        label: label.into(),
        registered_app_id: found.map(|a| a.id.clone()),
        registered_name: found.map(|a| a.name.clone()),
        endpoint: endpoint.into(),
        endpoint_ok,
        detail,
    }
}

#[tauri::command]
pub fn integration_status(state: State<AppState>, probe: bool) -> Result<Vec<IntegrationSlot>> {
    let prefs = state.storage.get_preferences()?;
    let apps = state.storage.list_apps(false)?;
    Ok(vec![
        slot("PulseCut", &apps, "pulsecut", "", false),
        slot("PulseFlow", &apps, "pulseflow", &prefs.pulseflow_endpoint, probe),
        slot("OpenPulse", &apps, "openpulse", &prefs.openpulse_url, probe),
        slot("PulseApply", &apps, "pulseapply", "", false),
    ])
}

#[tauri::command]
pub fn complete_onboarding(state: State<AppState>) -> Result<()> {
    let mut prefs = state.storage.get_preferences()?;
    prefs.onboarding_done = true;
    state.storage.set_preferences(&prefs)
}

#[tauri::command]
pub fn shortcut_status(state: State<AppState>) -> Result<ShortcutStatus> {
    Ok(state.shortcut.lock().unwrap_or_else(|p| p.into_inner()).clone())
}

#[tauri::command]
pub fn open_details(app: AppHandle, id: String) -> Result<()> {
    let _ = app.emit("open-app", id);
    crate::shell::show_main(&app).map_err(|e| CoreError::Launch(e.to_string()))?;
    if let Some(win) = app.get_webview_window("launcher") {
        let _ = win.hide();
    }
    Ok(())
}

#[tauri::command]
pub fn hide_launcher(app: AppHandle) -> Result<()> {
    if let Some(win) = app.get_webview_window("launcher") {
        let _ = win.hide();
    }
    Ok(())
}

#[tauri::command]
pub fn show_main(app: AppHandle) -> Result<()> {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
    Ok(())
}

#[tauri::command]
pub fn quit(app: AppHandle) -> Result<()> {
    app.exit(0);
    Ok(())
}

pub fn seed(storage: &Storage) -> Result<()> {
    let _ = storage.ensure_system_collection(ZEROPULSE_COLLECTION, "ZeroPulse")?;
    let n = templates::seed(storage)?;
    if n > 0 {
        let _ = storage.set_meta("templates_seeded", &n.to_string());
    }
    Ok(())
}
