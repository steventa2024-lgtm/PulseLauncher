//! Tray, global shortcut and window placement.

use crate::commands;
use crate::state::{AppState, ShortcutStatus};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub fn install(app: &AppHandle, _state: &AppState) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open PulseLauncher", true, None::<&str>)?;
    let launcher = MenuItem::with_id(app, "launcher", "Toggle Launcher", true, None::<&str>)?;
    let rescan = MenuItem::with_id(app, "rescan", "Rescan Apps", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &launcher, &rescan, &quit])?;
    let icon = app.default_window_icon().cloned();
    let mut tray = TrayIconBuilder::new().menu(&menu).tooltip("PulseLauncher").on_menu_event(|app, event| match event.id.as_ref() {
        "open" => {
            let _ = show_main(app);
        }
        "launcher" => toggle_launcher(app),
        "rescan" => {
            if let Some(state) = app.try_state::<AppState>() {
                match commands::perform_scan(state.inner()) {
                    Ok(report) => {
                        let _ = app.emit("scan-finished", &report);
                    }
                    Err(e) => crate::state::append_log(state.inner(), &e.to_string()),
                }
            }
        }
        "quit" => app.exit(0),
        _ => {}
    });
    if let Some(icon) = icon {
        tray = tray.icon(icon);
    }
    tray.on_tray_icon_event(|tray, event| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
            let _ = show_main(tray.app_handle());
        }
    })
    .build(app)?;

    if let Some(main) = app.get_webview_window("main") {
        let handle = app.clone();
        main.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let close_to_tray = handle
                    .try_state::<AppState>()
                    .and_then(|s| s.storage.get_preferences().ok())
                    .map(|p| p.close_to_tray)
                    .unwrap_or(true);
                if close_to_tray {
                    api.prevent_close();
                    if let Some(win) = handle.get_webview_window("main") {
                        let _ = win.hide();
                    }
                }
            }
        });
    }
    if let Some(launcher) = app.get_webview_window("launcher") {
        let handle = app.clone();
        launcher.on_window_event(move |event| {
            if let WindowEvent::Focused(false) = event {
                let close = handle
                    .try_state::<AppState>()
                    .and_then(|s| s.storage.get_preferences().ok())
                    .map(|p| p.close_on_blur)
                    .unwrap_or(true);
                if close {
                    if let Some(win) = handle.get_webview_window("launcher") {
                        let _ = win.hide();
                    }
                }
            }
        });
    }
    Ok(())
}

pub fn show_main(app: &AppHandle) -> tauri::Result<()> {
    if let Some(win) = app.get_webview_window("main") {
        win.show()?;
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
    Ok(())
}

pub fn toggle_launcher(app: &AppHandle) {
    let Some(win) = app.get_webview_window("launcher") else { return };
    let visible = win.is_visible().unwrap_or(false);
    if visible {
        let _ = win.hide();
        return;
    }
    if let Ok(Some(mon)) = win.current_monitor() {
        let size = mon.size();
        let pos = mon.position();
        let scale = mon.scale_factor();
        let width = (720.0 * scale) as i32;
        let x = pos.x + (size.width as i32 - width).max(0) / 2;
        let y = pos.y + (size.height as f64 * 0.12) as i32;
        let _ = win.set_position(PhysicalPosition::new(x, y));
    }
    let _ = win.show();
    let _ = win.set_focus();
    let _ = app.emit("launcher-shown", ());
}

pub fn apply_shortcut(app: &AppHandle, state: &AppState, requested: &str) {
    let gs = app.global_shortcut();
    let previous = state.shortcut.lock().unwrap_or_else(|p| p.into_inner()).active.clone();
    if let Some(prev) = previous {
        if let Ok(parsed) = prev.parse::<tauri_plugin_global_shortcut::Shortcut>() {
            let _ = gs.unregister(parsed);
        }
    }
    let status = match gs.register(requested) {
        Ok(()) => ShortcutStatus { requested: requested.into(), active: Some(requested.into()), fallback_used: false, error: None },
        Err(e) => {
            let fallback = "Ctrl+Alt+Space";
            if requested != fallback && gs.register(fallback).is_ok() {
                ShortcutStatus {
                    requested: requested.into(),
                    active: Some(fallback.into()),
                    fallback_used: true,
                    error: Some(format!("{requested} is not available ({e}). Using {fallback}.")),
                }
            } else {
                ShortcutStatus {
                    requested: requested.into(),
                    active: None,
                    fallback_used: false,
                    error: Some(format!("Could not register {requested}: {e}")),
                }
            }
        }
    };
    crate::state::append_log(state, &format!("shortcut: requested {}, active {:?}, fallback {}", status.requested, status.active, status.fallback_used));
    *state.shortcut.lock().unwrap_or_else(|p| p.into_inner()) = status.clone();
    let _ = app.emit("shortcut-status", status);
}

pub fn apply_autostart(app: &AppHandle, enabled: bool) {
    use tauri_plugin_autostart::ManagerExt;
    let auto = app.autolaunch();
    let result = if enabled { auto.enable() } else { auto.disable() };
    if let Err(e) = result {
        if let Some(state) = app.try_state::<AppState>() {
            crate::state::append_log(state.inner(), &format!("startup setting could not be applied: {e}"));
        }
    }
}

pub fn shortcut_handler(app: &AppHandle, _shortcut: &tauri_plugin_global_shortcut::Shortcut, event: tauri_plugin_global_shortcut::ShortcutEvent) {
    if event.state == ShortcutState::Pressed {
        toggle_launcher(app);
    }
}
