mod commands;
mod host;
mod shell;
mod state;

use state::AppState;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

fn to_io(e: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::other(e.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--autostart"])))
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = shell::show_main(app);
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| shell::shortcut_handler(app, shortcut, event))
                .build(),
        )
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let storage = Arc::new(pulse_core::storage::Storage::open(&dir.join("pulse.sqlite")).map_err(to_io)?);
            commands::seed(&storage).map_err(to_io)?;
            let state = AppState {
                storage,
                platform: Arc::new(pulse_core::platform::NativePlatform::new()),
                data_dir: dir.clone(),
                cancel: Arc::new(Mutex::new(None)),
                confirm: Arc::new(Mutex::new(None)),
                shortcut: Arc::new(Mutex::new(state::ShortcutStatus {
                    requested: "Alt+Space".into(),
                    active: None,
                    fallback_used: false,
                    error: None,
                })),
                log_path: dir.join("pulse.log"),
            };
            let prefs = state.storage.get_preferences()?;
            app.manage(state.clone());
            shell::install(app.handle(), &state)?;
            shell::apply_shortcut(app.handle(), &state, &prefs.launcher_shortcut);
            shell::apply_autostart(app.handle(), prefs.start_with_windows);
            let handle = app.handle().clone();
            let scan_state = state.clone();
            std::thread::spawn(move || match commands::perform_scan(&scan_state) {
                Ok(report) => {
                    let _ = handle.emit("scan-finished", &report);
                }
                Err(e) => state::append_log(&scan_state, &format!("startup scan failed: {e}")),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_apps,
            commands::app_details,
            commands::search_apps,
            commands::launch_app,
            commands::open_path,
            commands::open_url,
            commands::reveal_path,
            commands::copy_text,
            commands::add_app,
            commands::update_app,
            commands::remove_app,
            commands::reorder_apps,
            commands::scan,
            commands::list_collections,
            commands::create_collection,
            commands::rename_collection,
            commands::delete_collection,
            commands::set_membership,
            commands::import_preview,
            commands::delete_preview,
            commands::preview_data,
            commands::list_workflows,
            commands::workflow_status,
            commands::save_workflow,
            commands::reset_template,
            commands::delete_workflow,
            commands::run_workflow,
            commands::cancel_workflow,
            commands::confirm_workflow,
            commands::list_runs,
            commands::get_preferences,
            commands::set_preferences,
            commands::reindex,
            commands::export_backup,
            commands::preview_backup,
            commands::apply_backup,
            commands::clear_history,
            commands::diagnostics,
            commands::read_log,
            commands::ollama_status,
            commands::explain_with_ollama,
            commands::integration_status,
            commands::complete_onboarding,
            commands::shortcut_status,
            commands::open_details,
            commands::hide_launcher,
            commands::show_main,
            commands::quit,
        ])
        .run(tauri::generate_context!())
        .expect("PulseLauncher failed to start");
}
