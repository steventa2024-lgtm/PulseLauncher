//! `WorkflowHost` backed by the real storage, platform and desktop shell.

use crate::state::AppState;
use pulse_core::error::{CoreError, Result};
use pulse_core::launch;
use pulse_core::platform::Platform;
use pulse_core::models::*;
use pulse_core::security;
use pulse_core::workflow::engine::{CancelToken, WorkflowEvent, WorkflowHost};
use pulse_core::workflow::model::WorkflowRun;
use std::sync::mpsc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

pub struct Host {
    pub app: AppHandle,
    pub state: AppState,
}

impl Host {
    fn prefs(&self) -> UserPreferences {
        self.state.storage.get_preferences().unwrap_or_default()
    }
}

impl WorkflowHost for Host {
    fn launch_app(&self, app_id: &str, extra_args: &[String]) -> Result<LaunchResult> {
        launch::launch_app(
            &self.state.storage,
            self.state.platform.as_ref(),
            app_id,
            extra_args,
            self.prefs().record_history,
        )
    }

    fn launch_path(&self, path: &str, args: &[String], working_dir: Option<&str>) -> Result<LaunchResult> {
        let target = ExecutableTarget {
            kind: TargetKind::Executable,
            path: path.to_string(),
            args: args.to_vec(),
            working_dir: working_dir.filter(|w| !w.is_empty()).map(str::to_string),
            aumid: None,
            uri: None,
        };
        let res = launch::launch_target(self.state.platform.as_ref(), &target);
        let args_display = if args.is_empty() { None } else { Some(security::join_windows_args(args)) };
        let _ = self.state.storage.record_launch(
            None,
            path,
            res.outcome,
            res.pid,
            Some(&res.message),
            args_display.as_deref(),
            self.prefs().record_history,
        );
        Ok(res)
    }

    fn open_path(&self, path: &str) -> Result<()> {
        if !security::is_absolute_any(path) || !security::fs_exists(path) {
            return Err(CoreError::NotFound(format!("not found: {path}")));
        }
        self.state.platform.open_path(path)
    }

    fn open_url(&self, url: &str) -> Result<()> {
        let url = security::validate_web_url(url)?;
        self.state.platform.open_url(url.as_str())
    }

    fn open_settings_uri(&self, uri: &str) -> Result<()> {
        let uri = security::validate_settings_uri(uri)?;
        self.state.platform.open_settings_uri(uri.as_str())
    }

    fn copy_text(&self, text: &str) -> Result<()> {
        let mut clip = arboard::Clipboard::new().map_err(|e| CoreError::Launch(format!("clipboard unavailable: {e}")))?;
        clip.set_text(text).map_err(|e| CoreError::Launch(format!("could not copy: {e}")))
    }

    fn notify(&self, title: &str, body: &str) -> Result<()> {
        let _ = self.app.emit("app-notice", serde_json::json!({ "title": title, "body": body }));
        use tauri_plugin_notification::NotificationExt;
        if let Err(e) = self.app.notification().builder().title(title).body(body).show() {
            crate::state::append_log(&self.state, &format!("native notification unavailable ({e}); showed an in-app notice"));
        }
        Ok(())
    }

    fn confirm(&self, message: &str, timeout: Duration, cancel: &CancelToken) -> Result<bool> {
        let (tx, rx) = mpsc::channel();
        *self.state.confirm.lock().unwrap_or_else(|p| p.into_inner()) = Some(tx);
        let _ = self.app.emit("workflow-confirm", message);
        let start = std::time::Instant::now();
        loop {
            match rx.recv_timeout(Duration::from_millis(80)) {
                Ok(answer) => return Ok(answer),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if cancel.is_cancelled() || start.elapsed() > timeout {
                        return Ok(false);
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(false),
            }
        }
    }

    fn app_exists(&self, app_id: &str) -> bool {
        self.state.storage.get_app(app_id).is_ok()
    }

    fn collection_apps(&self, system_key: &str) -> Result<Vec<(String, String)>> {
        let cols = self.state.storage.list_collections()?;
        let Some(col) = cols.into_iter().find(|c| c.system_key.as_deref() == Some(system_key)) else {
            return Ok(vec![]);
        };
        let apps = self.state.storage.list_apps(false)?;
        Ok(apps
            .into_iter()
            .filter(|a| a.collection_ids.iter().any(|id| id == &col.id))
            .map(|a| (a.id, a.name))
            .collect())
    }

    fn developer_commands_enabled(&self) -> bool {
        self.prefs().developer_command_execution
    }

    fn known_folder(&self, name: &str) -> Option<String> {
        self.state.platform.known_folder(name).map(|p| p.to_string_lossy().into_owned())
    }

    fn path_exists(&self, path: &str) -> bool {
        security::fs_exists(path)
    }

    fn emit(&self, event: &WorkflowEvent) {
        let _ = self.app.emit("workflow-event", event);
    }
}

pub fn answer_confirm(state: &AppState, accepted: bool) {
    if let Some(tx) = state.confirm.lock().unwrap_or_else(|p| p.into_inner()).take() {
        let _ = tx.send(accepted);
    }
}

/// Persist values, then run. Emits step events while it executes.
pub fn execute(app: &AppHandle, state: &AppState, id: &str, values: std::collections::BTreeMap<String, String>) -> Result<WorkflowRun> {
    if state.cancel.lock().unwrap_or_else(|p| p.into_inner()).is_some() {
        return Err(CoreError::invalid("a workflow is already running"));
    }
    let mut def = state.storage.get_workflow(id)?;
    for (k, v) in &values {
        if def.params.iter().any(|p| &p.name == k) {
            def.values.insert(k.clone(), v.clone());
        }
    }
    state.storage.upsert_workflow(&def)?;
    let token = CancelToken::new();
    *state.cancel.lock().unwrap_or_else(|p| p.into_inner()) = Some(token.clone());
    let host = Host { app: app.clone(), state: state.clone() };
    let run = pulse_core::workflow::engine::run_workflow(&def, &values, &host, &token);
    *state.cancel.lock().unwrap_or_else(|p| p.into_inner()) = None;
    state.storage.save_run(&run)?;
    Ok(run)
}
