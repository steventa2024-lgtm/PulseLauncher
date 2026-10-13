use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use pulslauncher_core::discovery::{discover_installed, DiscoverOptions};
use pulslauncher_core::files::{icon_data_url, index_roots, read_preview_data_url, save_preview};
use pulslauncher_core::integrations::{suggest_with_ollama, test_ollama, test_pulseflow};
use pulslauncher_core::launch::{launch_app, Host};
use pulslauncher_core::platform::{set_autostart, SystemHost};
use pulslauncher_core::search::{search, SearchInput};
use pulslauncher_core::security::home_dir;
use pulslauncher_core::security::validate_user_json_path;
use pulslauncher_core::storage::{
    add_preview, apply_import, apply_scan, apps_by_id, clear_history, collection_counts,
    collection_members, create_collection, delete_collection, delete_manual, delete_preview,
    diagnostics, export_bundle, get_app, get_preview, get_workflow, latest_run, list_apps,
    list_collections, list_events, list_indexed_files, list_previews, list_runs, list_workflows,
    load_preferences, open_database, preview_import, record_launch, register_app,
    rename_collection, replace_index, reset_template, save_preferences, save_run, save_workflow,
    set_collection_members, set_favorite, set_hidden, set_pinned, update_app,
};
use pulslauncher_core::workflows::{assess, execute, preview, Confirmer, EngineContext};
use pulslauncher_core::{
    now_iso, AppError, LaunchOutcome, LaunchPreview, RegisterRequest, ScanReport, SearchHit,
    UpdateAppRequest, UserPreferences, WorkflowRecord, WorkflowRun, WorkflowView,
};
use rusqlite::Connection;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, State, WebviewWindow};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_notification::NotificationExt;

struct PendingConfirm {
    sender: mpsc::Sender<bool>,
}

struct ShellState {
    db: Mutex<Connection>,
    data_dir: std::path::PathBuf,
    db_path: std::path::PathBuf,
    confirms: Mutex<HashMap<String, PendingConfirm>>,
    cancels: Mutex<HashMap<String, Arc<AtomicBool>>>,
    quitting: Arc<AtomicBool>,
}

fn db(state: &ShellState) -> std::sync::MutexGuard<'_, Connection> {
    state.db.lock().unwrap_or_else(|err| err.into_inner())
}

fn host(app: &AppHandle) -> SystemHost {
    let app = app.clone();
    SystemHost {
        notifier: Some(std::sync::Arc::new(move |title, body| {
            let _ = app.emit(
                "app-notification",
                serde_json::json!({ "title": title, "body": body }),
            );
            let _ = app.notification().builder().title(title).body(body).show();
        })),
    }
}

#[tauri::command]
fn command_diagnostics(
    state: State<ShellState>,
) -> Result<pulslauncher_core::Diagnostics, AppError> {
    diagnostics(
        &db(&state),
        env!("CARGO_PKG_VERSION"),
        &state.data_dir,
        &state.db_path,
    )
}

#[tauri::command]
fn command_preferences_get(state: State<ShellState>) -> Result<UserPreferences, AppError> {
    load_preferences(&db(&state))
}

#[tauri::command]
fn command_preferences_set(
    app: AppHandle,
    state: State<ShellState>,
    preferences: UserPreferences,
) -> Result<UserPreferences, AppError> {
    let previous = load_preferences(&db(&state))?;
    save_preferences(&db(&state), &preferences)?;
    let saved = load_preferences(&db(&state))?;
    if saved.shortcut != previous.shortcut || saved.active_shortcut != previous.active_shortcut {
        match register_hotkey(&app, &saved.shortcut) {
            Ok((active, warning)) => {
                let mut updated = saved.clone();
                updated.active_shortcut = active;
                updated.shortcut_warning = warning;
                save_preferences(&db(&state), &updated)?;
            }
            Err(err) => {
                let _ = save_preferences(&db(&state), &previous);
                let _ = register_hotkey(&app, &previous.shortcut);
                return Err(err);
            }
        }
    }
    let saved = load_preferences(&db(&state))?;
    if saved.start_with_os != previous.start_with_os {
        apply_autostart(saved.start_with_os)?;
    }
    Ok(saved)
}

#[tauri::command]
fn command_apps_list(
    state: State<ShellState>,
    include_hidden: Option<bool>,
) -> Result<Vec<pulslauncher_core::AppEntry>, AppError> {
    list_apps(&db(&state), include_hidden.unwrap_or(false))
}

#[tauri::command]
fn command_apps_get(
    state: State<ShellState>,
    id: String,
) -> Result<pulslauncher_core::AppEntry, AppError> {
    get_app(&db(&state), &id)
}

#[tauri::command]
fn command_apps_scan(app: AppHandle, state: State<ShellState>) -> Result<ScanReport, AppError> {
    let prefs = load_preferences(&db(&state))?;
    let batch = discover_installed(&scan_options(prefs.index_desktop_shortcuts));
    let report = apply_scan(&db(&state), &batch.apps, &batch.errors)?;
    let _ = app.emit("scan-finished", &report);
    Ok(report)
}

#[tauri::command]
fn command_apps_register(
    state: State<ShellState>,
    request: RegisterRequest,
) -> Result<pulslauncher_core::AppEntry, AppError> {
    register_app(&db(&state), request)
}

#[tauri::command]
fn command_apps_update(
    state: State<ShellState>,
    request: UpdateAppRequest,
) -> Result<pulslauncher_core::AppEntry, AppError> {
    update_app(&db(&state), request)
}

#[tauri::command]
fn command_apps_hide(state: State<ShellState>, id: String, hidden: bool) -> Result<(), AppError> {
    set_hidden(&db(&state), &id, hidden)
}

#[tauri::command]
fn command_apps_delete_manual(state: State<ShellState>, id: String) -> Result<(), AppError> {
    delete_manual(&db(&state), &id)
}

#[tauri::command]
fn command_apps_favorite(
    state: State<ShellState>,
    id: String,
    favorite: bool,
) -> Result<(), AppError> {
    set_favorite(&db(&state), &id, favorite)
}

#[tauri::command]
fn command_apps_pin(state: State<ShellState>, id: String, pinned: bool) -> Result<(), AppError> {
    set_pinned(&db(&state), &id, pinned)
}

#[tauri::command]
fn command_apps_launch(state: State<ShellState>, id: String) -> Result<LaunchOutcome, AppError> {
    let entry = get_app(&db(&state), &id)?;
    let outcome = launch_app(&host_silent(), &entry, &[], false);
    match &outcome {
        Ok(value) => record_launch(&db(&state), Some(&id), None, "accepted", &value.message)?,
        Err(err) => record_launch(&db(&state), Some(&id), None, "rejected", &err.to_string())?,
    }
    outcome
}

#[tauri::command]
fn command_apps_reveal(state: State<ShellState>, id: String) -> Result<LaunchOutcome, AppError> {
    let entry = get_app(&db(&state), &id)?;
    let target = entry
        .resolved_target
        .clone()
        .unwrap_or(entry.launch_target.clone());
    let path = Path::new(&target);
    let folder = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()
            .ok_or_else(|| AppError::new("PATH", "This entry has no folder to reveal."))?
            .to_path_buf()
    };
    host_silent().open_folder(&folder)
}

#[tauri::command]
fn command_apps_copy_path(state: State<ShellState>, id: String) -> Result<(), AppError> {
    let entry = get_app(&db(&state), &id)?;
    let text = entry.resolved_target.clone().unwrap_or(entry.launch_target);
    host_silent().copy_text(&text)
}

#[tauri::command]
fn command_icons_read(path: String) -> Result<Option<String>, AppError> {
    icon_data_url(Path::new(&path))
}

#[tauri::command]
fn command_collections_list(
    state: State<ShellState>,
) -> Result<Vec<pulslauncher_core::Collection>, AppError> {
    list_collections(&db(&state))
}

#[tauri::command]
fn command_collections_create(
    state: State<ShellState>,
    name: String,
) -> Result<pulslauncher_core::Collection, AppError> {
    create_collection(&db(&state), &name)
}

#[tauri::command]
fn command_collections_rename(
    state: State<ShellState>,
    id: String,
    name: String,
) -> Result<(), AppError> {
    rename_collection(&db(&state), &id, &name)
}

#[tauri::command]
fn command_collections_delete(state: State<ShellState>, id: String) -> Result<(), AppError> {
    delete_collection(&db(&state), &id)
}

#[tauri::command]
fn command_collections_set_members(
    state: State<ShellState>,
    id: String,
    app_ids: Vec<String>,
) -> Result<(), AppError> {
    set_collection_members(&db(&state), &id, &app_ids)
}

#[tauri::command]
fn command_workflows_list(state: State<ShellState>) -> Result<Vec<WorkflowView>, AppError> {
    workflow_views(&db(&state))
}

#[tauri::command]
fn command_workflows_get(state: State<ShellState>, id: String) -> Result<WorkflowView, AppError> {
    let record = get_workflow(&db(&state), &id)?;
    enrich(&db(&state), record)
}

#[tauri::command]
fn command_workflows_save(
    state: State<ShellState>,
    mut record: WorkflowRecord,
) -> Result<WorkflowView, AppError> {
    record.customized = true;
    record.updated_at = now_iso();
    save_workflow(&db(&state), &record)?;
    enrich(&db(&state), get_workflow(&db(&state), &record.id)?)
}

#[tauri::command]
fn command_workflows_reset(state: State<ShellState>, id: String) -> Result<WorkflowView, AppError> {
    let record = reset_template(&db(&state), &id)?;
    enrich(&db(&state), record)
}

#[tauri::command]
fn command_workflows_preview(
    state: State<ShellState>,
    id: String,
) -> Result<Vec<pulslauncher_core::StepPreview>, AppError> {
    let record = get_workflow(&db(&state), &id)?;
    let apps = apps_by_id(&db(&state))?;
    let collections = collection_members(&db(&state))?;
    let prefs = load_preferences(&db(&state))?;
    let ctx = EngineContext {
        apps: &apps,
        collections: &collections,
        developer_commands: prefs.developer_commands_enabled,
    };
    preview(&record, &ctx, &host_silent())
}

#[tauri::command]
fn command_workflows_start(
    app: AppHandle,
    state: State<ShellState>,
    id: String,
) -> Result<WorkflowRun, AppError> {
    let record = get_workflow(&db(&state), &id)?;
    let apps = apps_by_id(&db(&state))?;
    let collections = collection_members(&db(&state))?;
    let counts = collection_counts(&db(&state))?;
    let prefs = load_preferences(&db(&state))?;
    let gate = assess(&record, &host_silent(), &apps, &counts);
    let run_id = uuid_id();
    if gate.status != pulslauncher_core::TemplateStatus::Ready {
        let run = WorkflowRun {
            id: run_id,
            workflow_id: record.id.clone(),
            status: match gate.status {
                pulslauncher_core::TemplateStatus::NeedsSetup => "needsSetup".into(),
                pulslauncher_core::TemplateStatus::Unavailable => "unavailable".into(),
                pulslauncher_core::TemplateStatus::Ready => "completed".into(),
            },
            started_at: now_iso(),
            finished_at: Some(now_iso()),
            summary: gate.message.clone(),
            steps: Vec::new(),
        };
        save_run(&db(&state), &run)?;
        return Ok(run);
    }
    let (sender, receiver) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    state
        .confirms
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .insert(run_id.clone(), PendingConfirm { sender });
    state
        .cancels
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .insert(run_id.clone(), cancel.clone());
    let run = WorkflowRun {
        id: run_id.clone(),
        workflow_id: record.id.clone(),
        status: "running".into(),
        started_at: now_iso(),
        finished_at: None,
        summary: "Running.".into(),
        steps: Vec::new(),
    };
    save_run(&db(&state), &run)?;
    let developer = prefs.developer_commands_enabled;
    let started_at = run.started_at.clone();
    std::thread::spawn(move || {
        let shell = app.state::<ShellState>();
        struct ChannelConfirm {
            receiver: mpsc::Receiver<bool>,
            app: AppHandle,
            run_id: String,
        }
        impl Confirmer for ChannelConfirm {
            fn confirm(&mut self, message: &str) -> Result<bool, AppError> {
                let _ = self.app.emit(
                    "workflow-confirm",
                    serde_json::json!({ "runId": self.run_id, "message": message }),
                );
                match self.receiver.recv_timeout(Duration::from_secs(120)) {
                    Ok(answer) => Ok(answer),
                    Err(_) => Err(AppError::new(
                        "CONFIRM",
                        "Confirmation timed out after 120 seconds.",
                    )),
                }
            }
        }
        let ctx = EngineContext {
            apps: &apps,
            collections: &collections,
            developer_commands: developer,
        };
        let mut confirmer = ChannelConfirm {
            receiver,
            app: app.clone(),
            run_id: run_id.clone(),
        };
        let _ = app.emit(
            "workflow-progress",
            serde_json::json!({ "runId": run_id, "status": "running" }),
        );
        let report = execute(&record, &ctx, &host(&app), &mut confirmer, &cancel);
        let finished = WorkflowRun {
            id: run_id.clone(),
            workflow_id: record.id.clone(),
            status: report.status.clone(),
            started_at,
            finished_at: Some(now_iso()),
            summary: report.summary.clone(),
            steps: report.steps.clone(),
        };
        let _ = save_run(&db(&shell), &finished);
        let _ = record_launch(
            &db(&shell),
            None,
            Some(&record.id),
            if report.status == "completed" {
                "accepted"
            } else {
                "rejected"
            },
            &report.summary,
        );
        let _ = app.emit("workflow-progress", &finished);
        shell
            .confirms
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .remove(&run_id);
        shell
            .cancels
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .remove(&run_id);
    });
    Ok(run)
}

#[tauri::command]
fn command_workflows_cancel(state: State<ShellState>, run_id: String) -> Result<(), AppError> {
    if let Some(flag) = state
        .cancels
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .get(&run_id)
    {
        flag.store(true, Ordering::Relaxed);
        Ok(())
    } else {
        Err(AppError::new(
            "NOT_FOUND",
            "That workflow run is not active.",
        ))
    }
}

#[tauri::command]
fn command_workflows_confirm(
    state: State<ShellState>,
    run_id: String,
    accepted: bool,
) -> Result<(), AppError> {
    let pending = state
        .confirms
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .remove(&run_id);
    match pending {
        Some(pending) => pending
            .sender
            .send(accepted)
            .map_err(|_| AppError::new("CONFIRM", "The workflow is no longer waiting.")),
        None => Err(AppError::new(
            "NOT_FOUND",
            "No confirmation is waiting for that run.",
        )),
    }
}

#[tauri::command]
fn command_workflows_runs(
    state: State<ShellState>,
    id: String,
) -> Result<Vec<WorkflowRun>, AppError> {
    list_runs(&db(&state), &id)
}

#[tauri::command]
fn command_search(state: State<ShellState>, query: String) -> Result<Vec<SearchHit>, AppError> {
    let prefs = load_preferences(&db(&state))?;
    let apps = list_apps(&db(&state), false)?;
    let workflows = list_workflows(&db(&state))?;
    let files = if prefs.search_files {
        list_indexed_files(&db(&state))?
    } else {
        Vec::new()
    };
    Ok(search(SearchInput {
        query: &query,
        apps: &apps,
        workflows: &workflows,
        files: &files,
        web_search: prefs.web_search_enabled,
        limit: 40,
    }))
}

#[tauri::command]
fn command_search_activate(
    app: AppHandle,
    state: State<ShellState>,
    query: String,
    hit_id: String,
) -> Result<String, AppError> {
    let hits = command_search(state.clone(), query)?;
    let hit = hits
        .into_iter()
        .find(|hit| hit.id == hit_id)
        .ok_or_else(|| {
            AppError::new(
                "NOT_FOUND",
                "That result is no longer in the list. Search again.",
            )
        })?;
    match hit.kind.as_str() {
        "app" => {
            let id = hit.app_id.unwrap_or_default();
            command_apps_launch(state, id)?;
            Ok("Launched.".into())
        }
        "workflow" => {
            let id = hit.workflow_id.unwrap_or_default();
            let _ = show_dashboard(&app);
            command_workflows_start(app, state, id)?;
            Ok("Workflow started. Open Commands to watch it.".into())
        }
        "calculator" => {
            let value = hit.calculator_value.unwrap_or_default();
            host_silent().copy_text(&value)?;
            Ok(format!("Copied {value}"))
        }
        "web" => {
            let prefs = load_preferences(&db(&state))?;
            if !prefs.web_search_enabled {
                return Err(AppError::new(
                    "WEB",
                    "Web search is turned off in Settings.",
                ));
            }
            let url = hit.url.unwrap_or_default();
            host_silent().open_url(&url)?;
            Ok("Asked the browser to open the search.".into())
        }
        "file" => {
            let path = hit.path.unwrap_or_default();
            let indexed = list_indexed_files(&db(&state))?;
            if !indexed.iter().any(|file| file.path == path) {
                return Err(AppError::new(
                    "PATH",
                    "That file is not in the opt-in index.",
                ));
            }
            host_silent().open_shell_target(&path)?;
            Ok("Asked the system to open the file.".into())
        }
        _ => Err(AppError::new("SEARCH", "This result cannot be activated.")),
    }
}

#[tauri::command]
fn command_history_list(
    state: State<ShellState>,
    app_id: Option<String>,
) -> Result<Vec<pulslauncher_core::LaunchEvent>, AppError> {
    list_events(&db(&state), app_id.as_deref())
}

#[tauri::command]
fn command_history_clear(state: State<ShellState>) -> Result<(), AppError> {
    clear_history(&db(&state))
}

#[tauri::command]
fn command_backup_export(
    state: State<ShellState>,
    include_history: Option<bool>,
) -> Result<pulslauncher_core::ExportBundle, AppError> {
    export_bundle(&db(&state), include_history.unwrap_or(false))
}

#[tauri::command]
fn command_backup_preview(json: String) -> Result<pulslauncher_core::ImportPreview, AppError> {
    preview_import(&json)
}

#[tauri::command]
fn command_backup_apply(
    state: State<ShellState>,
    json: String,
) -> Result<pulslauncher_core::ImportPreview, AppError> {
    apply_import(&db(&state), &json)
}

#[tauri::command]
fn command_backup_write(
    state: State<ShellState>,
    path: String,
    include_history: Option<bool>,
) -> Result<(), AppError> {
    let path = validate_user_json_path(Path::new(&path), false)?;
    let bundle = export_bundle(&db(&state), include_history.unwrap_or(false))?;
    let json = serde_json::to_string_pretty(&bundle)?;
    if json.len() > 8_000_000 {
        return Err(AppError::new("IMPORT", "The export is larger than 8 MB."));
    }
    std::fs::write(path, json).map_err(|err| AppError::new("IO", err.to_string()))
}

#[tauri::command]
fn command_backup_read(path: String) -> Result<String, AppError> {
    let path = validate_user_json_path(Path::new(&path), true)?;
    let meta = std::fs::metadata(&path)?;
    if meta.len() > 2_000_000 {
        return Err(AppError::new(
            "IMPORT",
            "Import files must be 2 MB or smaller.",
        ));
    }
    let json = std::fs::read_to_string(path)?;
    preview_import(&json)?;
    Ok(json)
}

#[tauri::command]
fn command_preview_import(
    state: State<ShellState>,
    app_id: String,
    source_path: String,
) -> Result<pulslauncher_core::PreviewAsset, AppError> {
    let _ = get_app(&db(&state), &app_id)?;
    let asset = save_preview(&state.data_dir, &app_id, Path::new(&source_path))?;
    add_preview(&db(&state), &asset)?;
    Ok(asset)
}

#[tauri::command]
fn command_preview_delete(state: State<ShellState>, id: String) -> Result<(), AppError> {
    delete_preview(&db(&state), &id)
}

#[tauri::command]
fn command_previews_list(
    state: State<ShellState>,
    app_id: String,
) -> Result<Vec<pulslauncher_core::PreviewAsset>, AppError> {
    list_previews(&db(&state), &app_id)
}

#[tauri::command]
fn command_preview_data(state: State<ShellState>, id: String) -> Result<String, AppError> {
    let asset = get_preview(&db(&state), &id)?;
    read_preview_data_url(&state.data_dir, Path::new(&asset.path))
}

#[tauri::command]
fn command_apps_launch_preview(
    state: State<ShellState>,
    id: String,
) -> Result<LaunchPreview, AppError> {
    let entry = get_app(&db(&state), &id)?;
    pulslauncher_core::launch::preview_app(&entry, &[])
}

#[tauri::command]
fn command_openpulse_launch(state: State<ShellState>) -> Result<LaunchOutcome, AppError> {
    let prefs = load_preferences(&db(&state))?;
    let target = prefs.openpulse_target.trim().to_string();
    if target.is_empty() {
        return Err(AppError::new(
            "OPENPULSE",
            "OpenPulse is not configured. Choose an executable or an https URL in Settings.",
        ));
    }
    let host = host_silent();
    if target.starts_with("http://") || target.starts_with("https://") {
        return host.open_url(&target);
    }
    let path = pulslauncher_core::security::validate_existing_program(Path::new(&target))?;
    host.launch_program(&path, &[], None, false)
}

#[tauri::command]
fn command_index_run(state: State<ShellState>) -> Result<usize, AppError> {
    let prefs = load_preferences(&db(&state))?;
    if !prefs.search_files {
        return Err(AppError::new(
            "INDEX",
            "File search is off. Enable it and choose folders in Settings first.",
        ));
    }
    let cancel = AtomicBool::new(false);
    let files = index_roots(
        &prefs.index_roots,
        &prefs.index_exclusions,
        &prefs.index_extensions,
        &cancel,
    )?;
    let count = files.len();
    replace_index(&db(&state), &files)?;
    Ok(count)
}

#[tauri::command]
fn command_ollama_test(
    state: State<ShellState>,
) -> Result<pulslauncher_core::OllamaReport, AppError> {
    let prefs = load_preferences(&db(&state))?;
    Ok(test_ollama(
        prefs.ollama_enabled,
        &prefs.ollama_endpoint,
        &prefs.ollama_model,
    ))
}

#[tauri::command]
fn command_ollama_suggest(
    state: State<ShellState>,
    prompt: String,
) -> Result<pulslauncher_core::Suggestion, AppError> {
    let prefs = load_preferences(&db(&state))?;
    if !prefs.ollama_enabled {
        return Err(AppError::new("OLLAMA", "Local AI is turned off."));
    }
    suggest_with_ollama(&prefs.ollama_endpoint, &prefs.ollama_model, &prompt)
}

#[tauri::command]
fn command_pulseflow_test(
    state: State<ShellState>,
) -> Result<pulslauncher_core::PulseFlowReport, AppError> {
    let prefs = load_preferences(&db(&state))?;
    Ok(test_pulseflow(
        &prefs.pulseflow_executable,
        &prefs.pulseflow_endpoint,
    ))
}

#[tauri::command]
fn command_calculate(expression: String) -> Result<String, AppError> {
    let value = pulslauncher_core::calculator::evaluate(&expression)?;
    Ok(pulslauncher_core::calculator::format_number(value))
}

#[tauri::command]
fn command_show_launcher(app: AppHandle) {
    toggle_launcher(&app, true);
}

#[tauri::command]
fn command_hide_launcher(app: AppHandle) {
    if let Some(window) = app.get_webview_window("launcher") {
        let _ = window.hide();
    }
}

#[tauri::command]
fn command_open_details(app: AppHandle, id: String) {
    show_dashboard(&app);
    let _ = app.emit("open-details", serde_json::json!({ "appId": id }));
}

fn host_silent() -> SystemHost {
    SystemHost::default()
}

fn scan_options(include_desktop_shortcuts: bool) -> DiscoverOptions {
    DiscoverOptions {
        include_desktop_shortcuts,
        ..DiscoverOptions::default()
    }
}

fn workflow_views(conn: &Connection) -> Result<Vec<WorkflowView>, AppError> {
    list_workflows(conn)?
        .into_iter()
        .map(|record| enrich(conn, record))
        .collect()
}

fn enrich(conn: &Connection, record: WorkflowRecord) -> Result<WorkflowView, AppError> {
    let apps = apps_by_id(conn)?;
    let counts = collection_counts(conn)?;
    let prerequisite = assess(&record, &host_silent(), &apps, &counts);
    let last_run = latest_run(conn, &record.id)?;
    Ok(WorkflowView {
        record,
        prerequisite,
        last_run,
    })
}

fn uuid_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn apply_autostart(enabled: bool) -> Result<(), AppError> {
    let exe = std::env::current_exe().map_err(|err| AppError::new("AUTOSTART", err.to_string()))?;
    let dir = home_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(".config")
        .join("autostart");
    set_autostart(&dir, enabled, &exe)
}

fn register_hotkey(app: &AppHandle, desired: &str) -> Result<(String, String), AppError> {
    let shortcut_api = app.global_shortcut();
    let _ = shortcut_api.unregister_all();
    match shortcut_api.on_shortcut(desired, on_hotkey) {
        Ok(()) => Ok((desired.to_string(), String::new())),
        Err(err) => {
            let fallback = "Ctrl+Alt+Space";
            if desired == fallback {
                return Err(AppError::new(
                    "SHORTCUT",
                    format!("Could not register {desired}: {err}"),
                ));
            }
            match shortcut_api.on_shortcut(fallback, on_hotkey) {
                Ok(()) => Ok((
                    fallback.to_string(),
                    format!("{desired} is unavailable ({err}). Using {fallback}."),
                )),
                Err(fallback_err) => Err(AppError::new(
                    "SHORTCUT",
                    format!("{desired} failed ({err}). Fallback failed ({fallback_err})."),
                )),
            }
        }
    }
}

fn on_hotkey(
    app: &AppHandle,
    _shortcut: &tauri_plugin_global_shortcut::Shortcut,
    event: tauri_plugin_global_shortcut::ShortcutEvent,
) {
    if event.state == ShortcutState::Pressed {
        toggle_launcher(app, false);
    }
}

fn toggle_launcher(app: &AppHandle, force_show: bool) {
    let Some(window) = app.get_webview_window("launcher") else {
        return;
    };
    let visible = window.is_visible().unwrap_or(false);
    if visible && !force_show {
        let _ = window.hide();
        return;
    }
    place_launcher(&window);
    let _ = window.show();
    let _ = window.set_focus();
}

fn place_launcher(window: &WebviewWindow) {
    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return;
    };
    let scale = monitor.scale_factor();
    let size = monitor.size();
    let position = monitor.position();
    let screen_width = size.width as f64 / scale;
    let width = 720.0_f64.min(screen_width - 32.0).max(560.0);
    let x = position.x as f64 / scale + (screen_width - width) / 2.0;
    let y = position.y as f64 / scale + 72.0;
    let _ = window.set_size(LogicalSize::new(width, 540.0));
    let _ = window.set_position(LogicalPosition::new(x, y));
}

fn show_dashboard(app: &AppHandle) -> Option<()> {
    let window = app.get_webview_window("dashboard")?;
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
    Some(())
}

fn background_scan(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<ShellState>();
        let prefs = load_preferences(&db(&state)).unwrap_or_default();
        let batch = discover_installed(&scan_options(prefs.index_desktop_shortcuts));
        let report = {
            let connection = db(&state);
            apply_scan(&connection, &batch.apps, &batch.errors)
        };
        match report {
            Ok(report) => {
                let _ = app.emit("scan-finished", report);
            }
            Err(err) => eprintln!("scan: {err}"),
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            show_dashboard(app);
        }))
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("pulslauncher.db");
            let connection = open_database(&db_path)?;
            app.manage(ShellState {
                db: Mutex::new(connection),
                data_dir,
                db_path,
                confirms: Mutex::new(HashMap::new()),
                cancels: Mutex::new(HashMap::new()),
                quitting: Arc::new(AtomicBool::new(false)),
            });
            if let Err(err) = install_tray(app.handle()) {
                eprintln!("tray: {err}");
            }
            background_scan(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            let app = window.app_handle();
            let Some(state) = app.try_state::<ShellState>() else {
                return;
            };
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } if window.label() == "dashboard" => {
                    let prefs = load_preferences(&db(&state)).unwrap_or_default();
                    let quitting = state.quitting.load(Ordering::Relaxed);
                    if prefs.close_to_tray && !quitting {
                        api.prevent_close();
                        let _ = window.hide();
                    } else {
                        state.quitting.store(true, Ordering::Relaxed);
                        app.exit(0);
                    }
                }
                tauri::WindowEvent::Focused(false) if window.label() == "launcher" => {
                    let prefs = load_preferences(&db(&state)).unwrap_or_default();
                    if prefs.close_on_blur {
                        let _ = window.hide();
                    }
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            command_diagnostics,
            command_preferences_get,
            command_preferences_set,
            command_apps_list,
            command_apps_get,
            command_apps_scan,
            command_apps_register,
            command_apps_update,
            command_apps_hide,
            command_apps_delete_manual,
            command_apps_favorite,
            command_apps_pin,
            command_apps_launch,
            command_apps_reveal,
            command_apps_copy_path,
            command_icons_read,
            command_collections_list,
            command_collections_create,
            command_collections_rename,
            command_collections_delete,
            command_collections_set_members,
            command_workflows_list,
            command_workflows_get,
            command_workflows_save,
            command_workflows_reset,
            command_workflows_preview,
            command_workflows_start,
            command_workflows_cancel,
            command_workflows_confirm,
            command_workflows_runs,
            command_search,
            command_search_activate,
            command_history_list,
            command_history_clear,
            command_backup_export,
            command_backup_preview,
            command_backup_apply,
            command_backup_write,
            command_backup_read,
            command_preview_import,
            command_preview_delete,
            command_previews_list,
            command_preview_data,
            command_apps_launch_preview,
            command_openpulse_launch,
            command_index_run,
            command_ollama_test,
            command_ollama_suggest,
            command_pulseflow_test,
            command_calculate,
            command_show_launcher,
            command_hide_launcher,
            command_open_details
        ])
        .build(tauri::generate_context!())
        .expect("PulseLauncher failed to start")
        .run(|app, event| {
            if let tauri::RunEvent::Ready = event {
                register_shortcut_from_db(app);
            }
        });
}

fn register_shortcut_from_db(app: &AppHandle) {
    let shortcut = {
        let Some(state) = app.try_state::<ShellState>() else {
            return;
        };
        let connection = db(&state);
        match load_preferences(&connection) {
            Ok(prefs) => prefs.shortcut,
            Err(_) => return,
        }
    };
    match register_hotkey(app, &shortcut) {
        Ok((active, warning)) => {
            let Some(state) = app.try_state::<ShellState>() else {
                return;
            };
            let connection = db(&state);
            if let Ok(mut stored) = load_preferences(&connection) {
                drop(connection);
                stored.active_shortcut = active;
                stored.shortcut_warning = warning;
                let connection = db(&state);
                if let Err(err) = save_preferences(&connection, &stored) {
                    eprintln!("shortcut save: {err}");
                }
            }
        }
        Err(err) => eprintln!("shortcut: {err}"),
    }
}

fn install_tray(app: &AppHandle) -> tauri::Result<()> {
    let open_item = MenuItem::with_id(app, "open", "Open PulseLauncher", true, None::<&str>)?;
    let toggle_item = MenuItem::with_id(app, "toggle", "Toggle Launcher", true, None::<&str>)?;
    let rescan_item = MenuItem::with_id(app, "rescan", "Rescan Apps", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open_item, &toggle_item, &rescan_item, &quit_item])?;
    let mut builder = TrayIconBuilder::with_id("tray")
        .menu(&menu)
        .tooltip("PulseLauncher")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => {
                show_dashboard(app);
            }
            "toggle" => toggle_launcher(app, false),
            "rescan" => {
                if let Some(state) = app.try_state::<ShellState>() {
                    let prefs = load_preferences(&db(&state)).unwrap_or_default();
                    let batch = discover_installed(&scan_options(prefs.index_desktop_shortcuts));
                    let report = {
                        let connection = db(&state);
                        apply_scan(&connection, &batch.apps, &batch.errors)
                    };
                    if let Ok(report) = report {
                        let _ = app.emit("scan-finished", report);
                    }
                }
            }
            "quit" => {
                if let Some(state) = app.try_state::<ShellState>() {
                    state.quitting.store(true, Ordering::Relaxed);
                }
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_dashboard(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}
