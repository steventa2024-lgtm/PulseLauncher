use std::collections::{BTreeMap, HashSet};
use std::sync::{atomic::Ordering, Arc};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use super::{AppState, RunControl};
use crate::error::{AppError, AppResult};
use crate::models::LaunchOutcome;
use crate::workflows::{
    self, templates, Actions, Facts, Host, Readiness, ReadinessState, Step, WorkflowDefinition,
    WorkflowKind, WorkflowRun,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowView {
    #[serde(flatten)]
    pub definition: WorkflowDefinition,
    pub readiness: Readiness,
    pub last_run: Option<WorkflowRun>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowEdit {
    pub name: String,
    pub description: String,
    pub params: Vec<workflows::ParamDef>,
    pub steps: Vec<Step>,
}

struct RealActions<'a> {
    db: &'a crate::storage::Db,
    workflow_id: &'a str,
}

impl Actions for RealActions<'_> {
    fn launch_app(&self, app_id: &str, args: &[String]) -> AppResult<LaunchOutcome> {
        let app = self.db.get_app(app_id)?;
        let override_args = (!args.is_empty()).then_some(args);
        let result = crate::launch::launch_app(&app, override_args);
        self.record(Some(app_id), &app.name, &result);
        result
    }
    fn launch_path(&self, path: &str, args: &[String]) -> AppResult<LaunchOutcome> {
        let result = crate::launch::launch_path(path, args, None);
        self.record(None, path, &result);
        result
    }
    fn open_folder(&self, path: &str) -> AppResult<LaunchOutcome> {
        crate::launch::open_folder(path)
    }
    fn open_url(&self, url: &str) -> AppResult<LaunchOutcome> {
        crate::launch::open_url(url)
    }
    fn open_uri(&self, uri: &str) -> AppResult<LaunchOutcome> {
        crate::launch::open_uri(uri)
    }
    fn write_note(&self, folder: &str, prefix: &str, content: &str) -> AppResult<String> {
        Ok(workflows::notes::write_note(folder, prefix, content)?
            .to_string_lossy()
            .into_owned())
    }
    fn run_command(&self, program: &str, args: &[String]) -> AppResult<LaunchOutcome> {
        let captured = crate::launch::run_captured(program, args, Duration::from_secs(30))?;
        let result = Ok(LaunchOutcome {
            accepted: true,
            process_id: None,
            message: format!(
                "Exited {}. {}",
                captured.exit_code.unwrap_or(0),
                captured.output.trim()
            ),
            executed: crate::security::preview_command(program, args),
        });
        self.record(None, program, &result);
        result
    }
    fn app_exists(&self, app_id: &str) -> bool {
        self.db.get_app(app_id).is_ok()
    }
}

impl RealActions<'_> {
    fn record(&self, app_id: Option<&str>, label: &str, result: &AppResult<LaunchOutcome>) {
        if !self
            .db
            .preferences()
            .map(|p| p.record_history)
            .unwrap_or(true)
        {
            return;
        }
        let (ok, pid, message) = match result {
            Ok(o) => (true, o.process_id, o.message.as_str()),
            Err(e) => (false, None, e.message.as_str()),
        };
        let _ = self
            .db
            .record_launch(app_id, Some(self.workflow_id), label, ok, pid, message);
    }
}

struct TauriHost {
    app: AppHandle,
    control: Arc<RunControl>,
    workflow_id: String,
}

impl Host for TauriHost {
    fn confirm(&self, message: &str) -> bool {
        let (tx, rx) = std::sync::mpsc::channel();
        *self
            .control
            .confirm
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = Some(tx);
        let _ = self.app.emit(
            "workflow-confirm",
            serde_json::json!({ "workflowId": self.workflow_id, "message": message }),
        );
        rx.recv_timeout(Duration::from_secs(120)).unwrap_or(false)
    }
    fn notify(&self, title: &str, body: &str) {
        let _ = self.app.emit(
            "app-notice",
            serde_json::json!({ "title": title, "body": body }),
        );
        use tauri_plugin_notification::NotificationExt;
        let _ = self
            .app
            .notification()
            .builder()
            .title(title)
            .body(body)
            .show();
    }
    fn copy(&self, text: &str) -> AppResult<()> {
        use tauri_plugin_clipboard_manager::ClipboardExt;
        self.app
            .clipboard()
            .write_text(text)
            .map_err(|e| AppError::internal(e.to_string()))
    }
    fn cancelled(&self) -> bool {
        self.control.cancel.load(Ordering::Relaxed)
    }
    fn progress(&self, run: &WorkflowRun) {
        let _ = self.app.emit("workflow-progress", run);
    }
}

fn known_apps(db: &crate::storage::Db) -> AppResult<HashSet<String>> {
    Ok(db.list_apps(true)?.into_iter().map(|a| a.id).collect())
}

fn view(db: &crate::storage::Db, def: WorkflowDefinition) -> AppResult<WorkflowView> {
    let apps = known_apps(db)?;
    let readiness = workflows::readiness(&def, &Facts::current(), &apps);
    let last_run = db.workflow_runs(&def.id, 1)?.into_iter().next();
    Ok(WorkflowView {
        definition: def,
        readiness,
        last_run,
    })
}

pub(crate) fn run_blocking(app: &AppHandle, workflow_id: &str) -> AppResult<WorkflowRun> {
    let state = app.state::<AppState>();
    let def = state.db.get_workflow(workflow_id)?;
    let prefs = state.db.preferences()?;
    let apps = known_apps(&state.db)?;
    let readiness = workflows::readiness(&def, &Facts::current(), &apps);
    if readiness.state != ReadinessState::Ready {
        let code = if readiness.state == ReadinessState::Unavailable {
            crate::error::ErrorCode::Unsupported
        } else {
            crate::error::ErrorCode::InvalidInput
        };
        return Err(AppError::new(code, readiness.details.join(" ")));
    }
    let control = Arc::new(RunControl {
        cancel: std::sync::atomic::AtomicBool::new(false),
        confirm: std::sync::Mutex::new(None),
    });
    state
        .runs
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(workflow_id.to_string(), control.clone());
    let actions = RealActions {
        db: &state.db,
        workflow_id,
    };
    let host = TauriHost {
        app: app.clone(),
        control,
        workflow_id: workflow_id.to_string(),
    };
    let run = workflows::execute(&def, &actions, &host, prefs.developer_commands_enabled);
    let _ = state.db.save_run(&run);
    state
        .runs
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .remove(workflow_id);
    Ok(run)
}

#[tauri::command]
pub fn list_workflows(state: State<AppState>) -> AppResult<Vec<WorkflowView>> {
    let defs = state.db.list_workflows()?;
    defs.into_iter().map(|d| view(&state.db, d)).collect()
}

#[tauri::command]
pub fn get_workflow(state: State<AppState>, id: String) -> AppResult<WorkflowView> {
    view(&state.db, state.db.get_workflow(&id)?)
}

#[tauri::command]
pub fn configure_workflow(
    state: State<AppState>,
    id: String,
    values: BTreeMap<String, String>,
) -> AppResult<WorkflowView> {
    let apps = known_apps(&state.db)?;
    let def = state.db.update_workflow_values(&id, values, &apps)?;
    view(&state.db, def)
}

#[tauri::command]
pub fn save_workflow_steps(
    state: State<AppState>,
    id: String,
    edit: WorkflowEdit,
) -> AppResult<WorkflowView> {
    let mut def = state.db.get_workflow(&id)?;
    def.name = edit.name;
    def.description = edit.description;
    def.what_it_does = if def.template_key.is_some() {
        def.what_it_does
    } else {
        def.description.clone()
    };
    def.params = edit.params;
    def.steps = edit.steps;
    def.trusted = def.template_key.is_some() && !workflows::contains_run_command(&def.steps);
    if workflows::contains_run_command(&def.steps) {
        def.trusted = false;
    }
    state.db.save_workflow(&def)?;
    view(&state.db, state.db.get_workflow(&id)?)
}

#[tauri::command]
pub fn create_workflow(state: State<AppState>, name: String) -> AppResult<WorkflowView> {
    let name = crate::security::bounded_text(&name, 120, "Workflow name")?;
    let def = WorkflowDefinition {
        schema: workflows::SCHEMA,
        id: uuid::Uuid::new_v4().to_string(),
        name,
        description: "A workflow you created.".into(),
        what_it_does: "A workflow you created.".into(),
        prerequisites: "Add steps, then run it.".into(),
        kind: WorkflowKind::Workflow,
        template_key: None,
        template_version: None,
        params: vec![],
        values: BTreeMap::new(),
        steps: vec![],
        hotkey: None,
        trusted: true,
    };
    state.db.save_workflow(&def)?;
    view(&state.db, def)
}

#[tauri::command]
pub fn reset_template(state: State<AppState>, id: String) -> AppResult<WorkflowView> {
    view(&state.db, state.db.reset_template(&id)?)
}

#[tauri::command]
pub fn delete_workflow(state: State<AppState>, id: String) -> AppResult<()> {
    state.db.delete_workflow(&id)
}

#[tauri::command]
pub fn trust_workflow(state: State<AppState>, id: String) -> AppResult<WorkflowView> {
    let enabled = state.db.preferences()?.developer_commands_enabled;
    view(&state.db, state.db.trust_workflow(&id, enabled)?)
}

#[tauri::command]
pub fn describe_workflow(state: State<AppState>, id: String) -> AppResult<Vec<String>> {
    let def = state.db.get_workflow(&id)?;
    workflows::describe(&def)
}

#[tauri::command]
pub async fn run_workflow(app: AppHandle, id: String) -> AppResult<WorkflowRun> {
    tauri::async_runtime::spawn_blocking(move || run_blocking(&app, &id))
        .await
        .map_err(|e| AppError::internal(e.to_string()))?
}

#[tauri::command]
pub fn confirm_workflow(
    state: State<AppState>,
    workflow_id: String,
    accepted: bool,
) -> AppResult<()> {
    let runs = state.runs.lock().unwrap_or_else(|p| p.into_inner());
    let Some(control) = runs.get(&workflow_id) else {
        return Err(AppError::not_found(
            "That workflow is not waiting for confirmation.",
        ));
    };
    if let Some(tx) = control
        .confirm
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .take()
    {
        let _ = tx.send(accepted);
    }
    Ok(())
}

#[tauri::command]
pub fn cancel_workflow(state: State<AppState>, workflow_id: String) -> AppResult<()> {
    let runs = state.runs.lock().unwrap_or_else(|p| p.into_inner());
    let Some(control) = runs.get(&workflow_id) else {
        return Err(AppError::not_found("That workflow is not running."));
    };
    control.cancel.store(true, Ordering::Relaxed);
    if let Some(tx) = control
        .confirm
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .take()
    {
        let _ = tx.send(false);
    }
    Ok(())
}

#[tauri::command]
pub fn workflow_history(state: State<AppState>, id: String) -> AppResult<Vec<WorkflowRun>> {
    state.db.workflow_runs(&id, 30)
}

#[tauri::command]
pub fn template_catalog() -> Vec<WorkflowDefinition> {
    templates::all()
}
