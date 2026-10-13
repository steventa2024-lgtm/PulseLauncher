//! Workflow data model. Everything here is plain, validated, serialisable data.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_STEPS: usize = 64;
pub const MAX_NEST_DEPTH: usize = 3;
pub const MAX_PARAMS: usize = 32;
pub const MAX_WAIT_MS: u64 = 300_000;
pub const MAX_TIMEOUT_MS: u64 = 600_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkflowKind {
    /// A single saved action with optional parameters.
    Command,
    /// An ordered sequence of steps.
    Workflow,
}

impl WorkflowKind {
    pub fn as_db(&self) -> &'static str {
        match self {
            WorkflowKind::Command => "command",
            WorkflowKind::Workflow => "workflow",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ParamKind {
    Text,
    Url,
    /// Absolute file path.
    File,
    /// Existing absolute folder path.
    Folder,
    /// A registered app id.
    App,
    /// A registered app id or an absolute file/folder path.
    Target,
    Choice,
    Bool,
    Number,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamChoice {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamDef {
    pub name: String,
    pub label: String,
    pub kind: ParamKind,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    pub choices: Vec<ParamChoice>,
    #[serde(default)]
    pub help: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Condition {
    Exists { path: String },
    NotEmpty { value: String },
    Equals { left: String, right: String },
    Not { inner: Box<Condition> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Action {
    LaunchApp {
        app_id: String,
        #[serde(default)]
        args: Vec<String>,
    },
    LaunchPath {
        path: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        working_dir: Option<String>,
    },
    /// `app:<id>`-style app id or a path opened through its file association.
    OpenTarget { target: String },
    OpenPath { path: String },
    OpenUrl { url: String },
    OpenSettingsUri { uri: String },
    CopyText { text: String },
    Wait { ms: u64 },
    Notify { title: String, body: String },
    CreateNote {
        dir: String,
        file_name: String,
        #[serde(default)]
        content: String,
    },
    Confirm { message: String },
    If {
        condition: Condition,
        #[serde(default)]
        then_steps: Vec<Step>,
        #[serde(default)]
        else_steps: Vec<Step>,
    },
    Fail { message: String },
    SetVar { name: String, value: String },
    LaunchCollection { system_key: String },
    /// Requires the "developer command execution" preference. No shell is ever involved.
    RunCommand {
        program: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        working_dir: Option<String>,
        #[serde(default)]
        trusted: bool,
    },
}

impl Action {
    pub fn type_name(&self) -> &'static str {
        match self {
            Action::LaunchApp { .. } => "launchApp",
            Action::LaunchPath { .. } => "launchPath",
            Action::OpenTarget { .. } => "openTarget",
            Action::OpenPath { .. } => "openPath",
            Action::OpenUrl { .. } => "openUrl",
            Action::OpenSettingsUri { .. } => "openSettingsUri",
            Action::CopyText { .. } => "copyText",
            Action::Wait { .. } => "wait",
            Action::Notify { .. } => "notify",
            Action::CreateNote { .. } => "createNote",
            Action::Confirm { .. } => "confirm",
            Action::If { .. } => "if",
            Action::Fail { .. } => "fail",
            Action::SetVar { .. } => "setVar",
            Action::LaunchCollection { .. } => "launchCollection",
            Action::RunCommand { .. } => "runCommand",
        }
    }

    /// Only steps with no external side effect beyond an overwrite-style result may be retried.
    pub fn is_retry_safe(&self) -> bool {
        matches!(self, Action::CopyText { .. } | Action::SetVar { .. } | Action::Notify { .. })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorPolicy {
    #[default]
    Stop,
    Continue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub id: String,
    pub label: String,
    pub action: Action,
    #[serde(default)]
    pub when: Option<Condition>,
    #[serde(default)]
    pub on_error: ErrorPolicy,
    /// If set, the named choice param (`stop`/`continue`) overrides `on_error`.
    #[serde(default)]
    pub on_error_param: Option<String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub retries: u8,
}

impl Step {
    pub fn new(id: &str, label: &str, action: Action) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            action,
            when: None,
            on_error: ErrorPolicy::Stop,
            on_error_param: None,
            timeout_ms: None,
            retries: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum Prerequisite {
    Windows,
    FileExists { label: String, path: String },
    AnyFileExists { label: String, paths: Vec<String> },
    ParamsSet { names: Vec<String> },
    /// At least one of the params must be non-empty.
    AnyParamSet { label: String, names: Vec<String> },
    CollectionNotEmpty { label: String, system_key: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowDefinition {
    pub id: String,
    pub kind: WorkflowKind,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub template_id: Option<String>,
    #[serde(default)]
    pub template_version: Option<u32>,
    #[serde(default)]
    pub params: Vec<ParamDef>,
    /// Saved parameter values (configuration).
    #[serde(default)]
    pub values: BTreeMap<String, String>,
    #[serde(default)]
    pub prerequisites: Vec<Prerequisite>,
    pub steps: Vec<Step>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StepStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Skipped,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowStepResult {
    pub step_id: String,
    pub label: String,
    pub action: String,
    pub status: StepStatus,
    pub message: String,
    /// Exact target/command (after substitution) as shown to the user before execution.
    pub preview: String,
    pub attempts: u8,
    pub duration_ms: u64,
    pub output: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RunStatus {
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl RunStatus {
    pub fn as_db(&self) -> &'static str {
        match self {
            RunStatus::Running => "running",
            RunStatus::Succeeded => "succeeded",
            RunStatus::Failed => "failed",
            RunStatus::Cancelled => "cancelled",
        }
    }
    pub fn from_db(s: &str) -> Self {
        match s {
            "succeeded" => RunStatus::Succeeded,
            "failed" => RunStatus::Failed,
            "cancelled" => RunStatus::Cancelled,
            // A run persisted as "running" belonged to a process that has exited.
            _ => RunStatus::Failed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRun {
    pub id: String,
    pub workflow_id: String,
    pub workflow_name: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: RunStatus,
    pub steps: Vec<WorkflowStepResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReadyState {
    Ready,
    NeedsSetup,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrereqResult {
    pub label: String,
    pub ok: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowStatus {
    pub state: ReadyState,
    pub prerequisites: Vec<PrereqResult>,
    pub last_run: Option<WorkflowRun>,
}
