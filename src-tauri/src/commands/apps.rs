use std::path::Path;

use base64::Engine;
use serde::Serialize;
use tauri::State;

use super::AppState;
use crate::discovery::{self, ScanOptions};
use crate::error::{AppError, AppResult};
use crate::metadata::{self, split_icon_location};
use crate::models::*;
use crate::security;
use crate::storage::apps::{AppUpdate, NewManualApp};
use crate::storage::now;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppDetails {
    pub app: AppEntry,
    pub events: Vec<LaunchEvent>,
    pub previews: Vec<PreviewView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewView {
    pub id: String,
    pub source: String,
    pub created_at: String,
    pub data_url: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveResult {
    /// True when the entry was deleted. False when an OS-discovered app was
    /// only hidden; PulseLauncher never uninstalls software.
    pub deleted: bool,
    pub message: String,
}

fn enrich(app: &mut AppEntry) {
    let candidate = app
        .target
        .resolved_target
        .clone()
        .filter(|p| Path::new(p).is_file())
        .or_else(|| {
            (app.target.kind == AppKind::Executable
                && Path::new(&app.target.launch_target).is_file())
            .then(|| app.target.launch_target.clone())
        });
    if let Some(path) = candidate {
        app.metadata = metadata::read_metadata(Path::new(&path));
    }
}

fn icon_bytes(app: &AppEntry) -> Option<Vec<u8>> {
    let mut locations = Vec::new();
    if let Some(icon) = &app.icon_path {
        let (path, index) = split_icon_location(icon, app.icon_index);
        locations.push((path, index));
    }
    if let Some(target) = &app.target.resolved_target {
        locations.push((target.clone(), 0));
    }
    if app.target.kind == AppKind::Executable {
        locations.push((app.target.launch_target.clone(), 0));
    }
    for (path, index) in locations {
        if let Some(bytes) = metadata::extract_icon(Path::new(&path), index) {
            return Some(bytes);
        }
    }
    None
}

pub(crate) fn perform_scan(db: &crate::storage::Db) -> AppResult<ScanReport> {
    let prefs = db.preferences()?;
    let started = now();
    let found = discovery::scan(&ScanOptions {
        include_desktop: prefs.include_desktop_shortcuts,
        include_packaged: prefs.include_packaged_apps,
    });
    let stats = db.reconcile_discovered(&found.apps, &found.scanned_sources)?;
    suggest_zeropulse(db)?;
    let report = ScanReport {
        started_at: started,
        finished_at: now(),
        found: found.apps.len(),
        added: stats.added,
        updated: stats.updated,
        removed: stats.removed,
        marked_missing: stats.marked_missing,
        errors: found.errors,
        sources: found.roots,
    };
    db.save_scan(&report)?;
    Ok(report)
}

/// Add newly discovered ZeroPulse-category apps to the collection once.
/// Removing one later is not undone by the next scan.
fn suggest_zeropulse(db: &crate::storage::Db) -> AppResult<()> {
    let collections = db.list_collections()?;
    let Some(collection) = collections
        .iter()
        .find(|c| c.system_key.as_deref() == Some("zeropulse"))
    else {
        return Ok(());
    };
    for app in db.list_apps(true)? {
        let first_sight = app.last_seen_at.as_deref() == Some(app.created_at.as_str());
        if app.source != DiscoveredSource::Manual
            && app.category.as_deref() == Some("ZeroPulse")
            && first_sight
            && !collection.app_ids.contains(&app.id)
        {
            db.set_collection_membership(&collection.id, &app.id, true)?;
        }
    }
    Ok(())
}

fn record(db: &crate::storage::Db, app: &AppEntry, result: &AppResult<LaunchOutcome>) {
    if !db.preferences().map(|p| p.record_history).unwrap_or(true) {
        return;
    }
    let (success, pid, message) = match result {
        Ok(o) => (true, o.process_id, o.message.as_str()),
        Err(e) => (false, None, e.message.as_str()),
    };
    let _ = db.record_launch(Some(&app.id), None, &app.name, success, pid, message);
}

#[tauri::command]
pub fn list_apps(state: State<AppState>, include_hidden: Option<bool>) -> AppResult<Vec<AppEntry>> {
    state.db.list_apps(include_hidden.unwrap_or(false))
}

#[tauri::command]
pub fn get_app(state: State<AppState>, id: String) -> AppResult<AppDetails> {
    let mut app = state.db.get_app(&id)?;
    enrich(&mut app);
    let events = state.db.launch_events(Some(&id), 50)?;
    let previews = preview_views(&state, &id)?;
    Ok(AppDetails {
        app,
        events,
        previews,
    })
}

fn preview_views(state: &AppState, app_id: &str) -> AppResult<Vec<PreviewView>> {
    let mut views = Vec::new();
    for asset in state.db.preview_assets(app_id)? {
        let data_url = std::fs::read(&asset.file_path)
            .ok()
            .filter(|b| b.len() <= 2_000_000)
            .map(|b| {
                let mime = match Path::new(&asset.file_path)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase()
                    .as_str()
                {
                    "jpg" | "jpeg" => "image/jpeg",
                    "gif" => "image/gif",
                    "webp" => "image/webp",
                    "bmp" => "image/bmp",
                    _ => "image/png",
                };
                format!(
                    "data:{mime};base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(b)
                )
            });
        views.push(PreviewView {
            id: asset.id,
            source: asset.source,
            created_at: asset.created_at,
            data_url,
        });
    }
    Ok(views)
}

#[tauri::command]
pub fn app_icons(
    state: State<AppState>,
    ids: Vec<String>,
) -> AppResult<Vec<(String, Option<String>)>> {
    let mut out = Vec::new();
    for id in ids.into_iter().take(120) {
        if let Some(cached) = state
            .icons
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&id)
            .cloned()
        {
            out.push((id, cached));
            continue;
        }
        let Ok(app) = state.db.get_app(&id) else {
            continue;
        };
        let encoded = icon_bytes(&app).map(|b| base64::engine::general_purpose::STANDARD.encode(b));
        state
            .icons
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(id.clone(), encoded.clone());
        out.push((id, encoded));
    }
    Ok(out)
}

#[tauri::command]
pub fn scan_apps(state: State<AppState>) -> AppResult<ScanReport> {
    state
        .icons
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clear();
    perform_scan(&state.db)
}

#[tauri::command]
pub fn add_app(
    state: State<AppState>,
    app: NewManualApp,
    collection_id: Option<String>,
) -> AppResult<AppEntry> {
    let saved = state.db.add_manual_app(app)?;
    if let Some(cid) = collection_id {
        state.db.set_collection_membership(&cid, &saved.id, true)?;
    }
    state.db.get_app(&saved.id)
}

#[tauri::command]
pub fn update_app(state: State<AppState>, id: String, update: AppUpdate) -> AppResult<AppEntry> {
    state
        .icons
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .remove(&id);
    state.db.update_app(&id, update)
}

#[tauri::command]
pub fn remove_app(state: State<AppState>, id: String) -> AppResult<RemoveResult> {
    let deleted = state.db.remove_app(&id)?;
    let paths = state.db.remove_preview_assets(&id).unwrap_or_default();
    for path in paths {
        let _ = remove_preview_file(&state, &path);
    }
    Ok(if deleted {
        RemoveResult {
            deleted: true,
            message: "The custom entry was removed. The file on disk was not touched.".into(),
        }
    } else {
        RemoveResult {
            deleted: false,
            message: "Hidden in PulseLauncher. The installed program was not uninstalled.".into(),
        }
    })
}

#[tauri::command]
pub fn reorder_apps(state: State<AppState>, ids: Vec<String>) -> AppResult<()> {
    if ids.len() > 10_000 {
        return Err(AppError::invalid("Too many ids"));
    }
    state.db.reorder_apps(&ids)
}

#[tauri::command]
pub fn list_collections(state: State<AppState>) -> AppResult<Vec<Collection>> {
    state.db.list_collections()
}

#[tauri::command]
pub fn create_collection(state: State<AppState>, name: String) -> AppResult<Collection> {
    state.db.create_collection(&name, None)
}

#[tauri::command]
pub fn rename_collection(state: State<AppState>, id: String, name: String) -> AppResult<()> {
    state.db.rename_collection(&id, &name)
}

#[tauri::command]
pub fn delete_collection(state: State<AppState>, id: String) -> AppResult<()> {
    let collections = state.db.list_collections()?;
    if collections
        .iter()
        .any(|c| c.id == id && c.system_key.is_some())
    {
        return Err(AppError::policy(
            "The ZeroPulse collection is built in and cannot be deleted.",
        ));
    }
    state.db.delete_collection(&id)
}

#[tauri::command]
pub fn set_collection_membership(
    state: State<AppState>,
    collection_id: String,
    app_id: String,
    member: bool,
) -> AppResult<()> {
    state.db.get_app(&app_id)?;
    state
        .db
        .set_collection_membership(&collection_id, &app_id, member)
}

#[tauri::command]
pub fn reorder_collection(
    state: State<AppState>,
    collection_id: String,
    app_ids: Vec<String>,
) -> AppResult<()> {
    state.db.reorder_collection(&collection_id, &app_ids)
}

#[tauri::command]
pub fn launch_registered_app(
    state: State<AppState>,
    id: String,
    args: Option<Vec<String>>,
) -> AppResult<LaunchOutcome> {
    let app = state.db.get_app(&id)?;
    let result = crate::launch::launch_app(&app, args.as_deref());
    record(&state.db, &app, &result);
    result
}

#[tauri::command]
pub fn reveal_app(state: State<AppState>, id: String) -> AppResult<LaunchOutcome> {
    let app = state.db.get_app(&id)?;
    let path = if Path::new(&app.target.launch_target).exists() {
        app.target.launch_target.clone()
    } else if let Some(resolved) = app.target.resolved_target.filter(|p| Path::new(p).exists()) {
        resolved
    } else {
        return Err(AppError::new(
            crate::error::ErrorCode::TargetMissing,
            "The file is not there, so its location cannot be opened.",
        ));
    };
    crate::launch::reveal_in_folder(&path)
}

const IMAGE_EXT: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp"];

#[tauri::command]
pub fn import_preview(
    state: State<AppState>,
    app_id: String,
    source_path: String,
) -> AppResult<PreviewView> {
    state.db.get_app(&app_id)?;
    let source = security::validate_absolute_path(&source_path, "Image")?;
    if !source.is_file() {
        return Err(AppError::new(
            crate::error::ErrorCode::TargetMissing,
            "Image file not found",
        ));
    }
    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !IMAGE_EXT.contains(&ext.as_str()) {
        return Err(AppError::invalid(
            "Choose a png, jpg, gif, webp or bmp image.",
        ));
    }
    let len = source.metadata()?.len();
    if len > 8 * 1024 * 1024 {
        return Err(AppError::invalid("Images are limited to 8 MB."));
    }
    let dir = state.data_dir.join("previews");
    std::fs::create_dir_all(&dir)?;
    let id = uuid::Uuid::new_v4().to_string();
    let dest = dir.join(format!("{id}.{ext}"));
    std::fs::copy(&source, &dest)?;
    let asset = state
        .db
        .add_preview_asset(&app_id, &dest.to_string_lossy(), "Imported by you")?;
    Ok(preview_views(&state, &app_id)?
        .into_iter()
        .find(|p| p.id == asset.id)
        .unwrap_or(PreviewView {
            id: asset.id,
            source: asset.source,
            created_at: asset.created_at,
            data_url: None,
        }))
}

fn remove_preview_file(state: &AppState, path: &str) -> AppResult<()> {
    let Ok(root) = state.data_dir.join("previews").canonicalize() else {
        return Ok(());
    };
    let Ok(candidate) = Path::new(path).canonicalize() else {
        return Ok(());
    };
    if candidate.starts_with(&root) {
        let _ = std::fs::remove_file(&candidate);
    }
    Ok(())
}

#[tauri::command]
pub fn clear_previews(state: State<AppState>, app_id: String) -> AppResult<()> {
    for path in state.db.remove_preview_assets(&app_id)? {
        remove_preview_file(&state, &path)?;
    }
    Ok(())
}
