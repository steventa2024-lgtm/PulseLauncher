//! Windows, tray and the global shortcut. The launcher is a second window
//! in this same process, backed by the same database.

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewWindow, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::commands::{apps, workflows};
use crate::error::{AppError, AppResult};
use crate::models::HotkeyStatus;

pub fn parse_accelerator(value: &str) -> AppResult<Shortcut> {
    value
        .parse::<Shortcut>()
        .map_err(|_| AppError::invalid(format!("\"{value}\" is not a shortcut PulseLauncher understands. Try Alt+Space or Ctrl+Alt+Space.")))
}

pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn hide_launcher(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("launcher") {
        let _ = window.hide();
    }
}

pub fn show_launcher(app: &AppHandle) {
    let Some(window) = app.get_webview_window("launcher") else {
        return;
    };
    position_launcher(&window);
    let _ = window.show();
    let _ = window.set_focus();
}

pub fn toggle_launcher(app: &AppHandle) {
    let Some(window) = app.get_webview_window("launcher") else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
    } else {
        show_launcher(app);
    }
}

fn position_launcher(window: &WebviewWindow) {
    let _ = window.set_size(LogicalSize::new(720.0, 520.0));
    let Ok(cursor) = window.cursor_position() else {
        return;
    };
    let Ok(monitors) = window.available_monitors() else {
        return;
    };
    let containing = monitors.into_iter().find(|monitor| {
        let origin = monitor.position();
        let size = monitor.size();
        cursor.x >= origin.x as f64
            && cursor.x < origin.x as f64 + size.width as f64
            && cursor.y >= origin.y as f64
            && cursor.y < origin.y as f64 + size.height as f64
    });
    let Some(monitor) = containing
        .or(window.current_monitor().ok().flatten())
        .or(window.primary_monitor().ok().flatten())
    else {
        return;
    };
    let area = monitor.work_area();
    let scale = monitor.scale_factor().max(1.0);
    let width = (720.0 * scale) as i32;
    let x = area.position.x + ((area.size.width as i32).saturating_sub(width) / 2).max(0);
    let y = area.position.y + (64.0 * scale) as i32;
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

pub fn apply_autostart(app: &AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| e.to_string())
    } else if manager.is_enabled().unwrap_or(false) {
        manager.disable().map_err(|e| e.to_string())
    } else {
        Ok(())
    }
}

pub fn apply_shortcuts(app: &AppHandle) -> HotkeyStatus {
    let prefs = app
        .state::<crate::commands::AppState>()
        .db
        .preferences()
        .unwrap_or_default();
    let shortcuts = app.global_shortcut();
    let _ = shortcuts.unregister_all();
    let mut warning = None;
    let mut candidates = vec![prefs.launcher_hotkey.clone()];
    for extra in &prefs.launcher_hotkey_fallbacks {
        if !candidates.iter().any(|c| c.eq_ignore_ascii_case(extra)) {
            candidates.push(extra.clone());
        }
    }
    let mut active = None;
    for (index, accelerator) in candidates.iter().enumerate() {
        if parse_accelerator(accelerator).is_err() {
            warning = Some(format!("\"{accelerator}\" is not a valid shortcut."));
            continue;
        }
        let registered = shortcuts.on_shortcut(accelerator.as_str(), |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                toggle_launcher(app);
            }
        });
        match registered {
            Ok(()) => {
                if index > 0 {
                    warning = Some(format!(
                        "\"{}\" could not be registered, so \"{accelerator}\" is active instead.",
                        prefs.launcher_hotkey
                    ));
                }
                active = Some(accelerator.clone());
                break;
            }
            Err(e) => warning = Some(format!("Could not register \"{accelerator}\": {e}")),
        }
    }
    if let Ok(workflows) = app.state::<crate::commands::AppState>().db.list_workflows() {
        for workflow in workflows
            .into_iter()
            .filter(|w| w.hotkey.as_deref().is_some_and(|h| !h.is_empty()))
        {
            let Some(hotkey) = workflow.hotkey.clone() else {
                continue;
            };
            if active.as_deref() == Some(hotkey.as_str()) {
                continue;
            }
            let id = workflow.id.clone();
            if shortcuts
                .on_shortcut(hotkey.as_str(), move |app, _, event| {
                    if event.state == ShortcutState::Pressed {
                        let app = app.clone();
                        let id = id.clone();
                        std::thread::spawn(move || {
                            if let Err(e) = workflows::run_blocking(&app, &id) {
                                let _ = app.emit(
                                    "app-notice",
                                    serde_json::json!({ "title": "Workflow", "body": e.message }),
                                );
                            }
                        });
                    }
                })
                .is_err()
            {
                let note = format!("Workflow shortcut \"{hotkey}\" is unavailable.");
                warning = Some(match warning {
                    Some(w) => format!("{w} {note}"),
                    None => note,
                });
            }
        }
    }
    HotkeyStatus {
        requested: prefs.launcher_hotkey,
        active,
        warning,
    }
}

pub fn install_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open PulseLauncher", true, None::<&str>)?;
    let toggle = MenuItem::with_id(app, "toggle", "Toggle Launcher", true, None::<&str>)?;
    let rescan = MenuItem::with_id(app, "rescan", "Rescan Apps", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &toggle, &rescan, &quit])?;
    let mut builder = TrayIconBuilder::with_id("tray")
        .tooltip("PulseLauncher")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "toggle" => toggle_launcher(app),
            "rescan" => {
                let state = app.state::<crate::commands::AppState>();
                match apps::perform_scan(&state.db) {
                    Ok(report) => {
                        let _ = app.emit("scan-finished", &report);
                    }
                    Err(e) => {
                        let _ = app.emit(
                            "app-notice",
                            serde_json::json!({ "title": "Rescan failed", "body": e.message }),
                        );
                    }
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

pub fn handle_window_event(window: &tauri::Window, event: &WindowEvent) {
    match event {
        WindowEvent::CloseRequested { api, .. } if window.label() == "launcher" => {
            api.prevent_close();
            let _ = window.hide();
        }
        WindowEvent::CloseRequested { api, .. } if window.label() == "main" => {
            let to_tray = window
                .state::<crate::commands::AppState>()
                .db
                .preferences()
                .map(|p| p.close_to_tray)
                .unwrap_or(true);
            if to_tray {
                api.prevent_close();
                let _ = window.hide();
            }
        }
        WindowEvent::Focused(false) if window.label() == "launcher" => {
            let hide = window
                .state::<crate::commands::AppState>()
                .db
                .preferences()
                .map(|p| p.hide_launcher_on_blur)
                .unwrap_or(true);
            if hide {
                let _ = window.hide();
            }
        }
        _ => {}
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app)
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
            let state = crate::commands::AppState::open(dir).map_err(|e| e.to_string())?;
            app.manage(state);
            if let Err(e) = install_tray(app.handle()) {
                eprintln!("PulseLauncher tray was not created: {e}");
            }
            let status = apply_shortcuts(app.handle());
            *app.state::<crate::commands::AppState>()
                .hotkey
                .lock()
                .unwrap_or_else(|p| p.into_inner()) = status;
            let prefs = app
                .state::<crate::commands::AppState>()
                .db
                .preferences()
                .unwrap_or_default();
            if prefs.launch_at_startup {
                let _ = apply_autostart(app.handle(), true);
            }
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let state = handle.state::<crate::commands::AppState>();
                match apps::perform_scan(&state.db) {
                    Ok(report) => {
                        let _ = handle.emit("scan-finished", &report);
                    }
                    Err(e) => {
                        let _ = handle.emit(
                            "app-notice",
                            serde_json::json!({ "title": "App scan failed", "body": e.message }),
                        );
                    }
                }
            });
            let minimized =
                prefs.start_minimized || std::env::args().any(|arg| arg == "--minimized");
            if !minimized {
                show_main(app.handle());
            }
            Ok(())
        })
        .on_window_event(handle_window_event)
        .invoke_handler(tauri::generate_handler![
            apps::list_apps,
            apps::get_app,
            apps::app_icons,
            apps::scan_apps,
            apps::add_app,
            apps::update_app,
            apps::remove_app,
            apps::reorder_apps,
            apps::list_collections,
            apps::create_collection,
            apps::rename_collection,
            apps::delete_collection,
            apps::set_collection_membership,
            apps::reorder_collection,
            apps::launch_registered_app,
            apps::reveal_app,
            apps::import_preview,
            apps::clear_previews,
            workflows::list_workflows,
            workflows::get_workflow,
            workflows::configure_workflow,
            workflows::save_workflow_steps,
            workflows::create_workflow,
            workflows::reset_template,
            workflows::delete_workflow,
            workflows::trust_workflow,
            workflows::describe_workflow,
            workflows::run_workflow,
            workflows::confirm_workflow,
            workflows::cancel_workflow,
            workflows::workflow_history,
            workflows::template_catalog,
            crate::commands::system::app_info,
            crate::commands::system::get_preferences,
            crate::commands::system::save_preferences,
            crate::commands::system::search,
            crate::commands::system::open_url,
            crate::commands::system::open_indexed,
            crate::commands::system::copy_text,
            crate::commands::system::clear_history,
            crate::commands::system::reindex_files,
            crate::commands::system::diagnostics,
            crate::commands::system::environment,
            crate::commands::system::ollama_status,
            crate::commands::system::ollama_suggest,
            crate::commands::system::pulseflow_status,
            crate::commands::system::show_launcher,
            crate::commands::system::hide_launcher,
            crate::commands::system::show_main,
            crate::commands::system::quit_app,
            crate::commands::system::export_bundle,
            crate::commands::system::import_bundle,
        ])
        .run(tauri::generate_context!())
        .expect("PulseLauncher failed to start");
}
