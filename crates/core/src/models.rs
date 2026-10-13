use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;
pub const ZEROPULSE_COLLECTION_ID: &str = "col-zeropulse";
pub const TEMPLATE_VERSION: i64 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AppKind {
    Win32,
    Portable,
    Packaged,
    Uri,
    Desktop,
    Shortcut,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SourceKind {
    StartMenuUser,
    StartMenuCommon,
    DesktopShortcuts,
    Manual,
    DesktopFile,
    Packaged,
    Uri,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum Density {
    Compact,
    #[default]
    Comfortable,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum ViewMode {
    #[default]
    Grid,
    List,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UserPreferences {
    pub theme: Theme,
    pub density: Density,
    pub blur: bool,
    pub reduce_motion: bool,
    pub high_contrast: bool,
    pub text_scale: f64,
    pub shortcut: String,
    pub close_on_blur: bool,
    pub minimize_to_tray: bool,
    pub close_to_tray: bool,
    pub start_with_os: bool,
    pub search_files: bool,
    pub index_roots: Vec<String>,
    pub index_exclusions: Vec<String>,
    pub index_extensions: Vec<String>,
    pub index_desktop_shortcuts: bool,
    pub developer_commands_enabled: bool,
    pub ollama_enabled: bool,
    pub ollama_endpoint: String,
    pub ollama_model: String,
    pub onboarding_complete: bool,
    pub web_search_enabled: bool,
    pub view_mode: ViewMode,
    pub active_shortcut: String,
    pub shortcut_warning: String,
    pub pulseflow_executable: String,
    pub pulseflow_endpoint: String,
    pub openpulse_target: String,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            theme: Theme::Dark,
            density: Density::Comfortable,
            blur: true,
            reduce_motion: false,
            high_contrast: false,
            text_scale: 1.0,
            shortcut: "Alt+Space".into(),
            close_on_blur: true,
            minimize_to_tray: true,
            close_to_tray: true,
            start_with_os: false,
            search_files: false,
            index_roots: Vec::new(),
            index_exclusions: vec![".git".into(), "node_modules".into(), "target".into()],
            index_extensions: vec![
                "txt".into(),
                "md".into(),
                "pdf".into(),
                "rs".into(),
                "ts".into(),
                "tsx".into(),
                "py".into(),
                "json".into(),
                "toml".into(),
            ],
            index_desktop_shortcuts: false,
            developer_commands_enabled: false,
            ollama_enabled: false,
            ollama_endpoint: "http://127.0.0.1:11434".into(),
            ollama_model: String::new(),
            onboarding_complete: false,
            web_search_enabled: false,
            view_mode: ViewMode::Grid,
            active_shortcut: String::new(),
            shortcut_warning: String::new(),
            pulseflow_executable: String::new(),
            pulseflow_endpoint: String::new(),
            openpulse_target: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppEntry {
    pub id: String,
    pub display_name: String,
    pub kind: AppKind,
    pub source: SourceKind,
    pub launch_target: String,
    pub resolved_target: Option<String>,
    pub arguments: Vec<String>,
    pub working_directory: Option<String>,
    pub icon_path: Option<String>,
    pub publisher: Option<String>,
    pub version: Option<String>,
    pub category: String,
    pub hidden: bool,
    pub favorite: bool,
    pub pinned: bool,
    pub user_modified: bool,
    pub discovered: bool,
    pub tags: Vec<String>,
    pub launch_count: i64,
    pub last_launched_at: Option<String>,
    pub missing: bool,
    pub identity_key: String,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredApp {
    pub display_name: String,
    pub kind: AppKind,
    pub source: SourceKind,
    pub launch_target: String,
    pub resolved_target: Option<String>,
    pub arguments: Vec<String>,
    pub working_directory: Option<String>,
    pub icon_path: Option<String>,
    pub publisher: Option<String>,
    pub version: Option<String>,
    pub category: String,
    pub missing: bool,
    pub identity_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryBatch {
    pub apps: Vec<DiscoveredApp>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub added: usize,
    pub updated: usize,
    pub marked_missing: usize,
    pub errors: Vec<String>,
    pub scanned_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RegisterRequest {
    pub display_name: String,
    pub target: String,
    #[serde(default)]
    pub arguments: Vec<String>,
    pub working_directory: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub collection_id: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAppRequest {
    pub id: String,
    pub display_name: String,
    pub target: String,
    #[serde(default)]
    pub arguments: Vec<String>,
    pub working_directory: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub category: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub built_in: bool,
    pub position: i64,
    pub created_at: String,
    pub app_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LaunchEvent {
    pub id: String,
    pub app_id: Option<String>,
    pub workflow_id: Option<String>,
    pub outcome: String,
    pub message: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LaunchOutcome {
    pub accepted: bool,
    pub pid: Option<u32>,
    pub exit_code: Option<i32>,
    pub waited: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LaunchPreview {
    pub program: String,
    pub arguments: Vec<String>,
    pub working_directory: Option<String>,
    pub display: String,
    pub via_shell: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum ErrorPolicy {
    #[default]
    Stop,
    Skip,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FolderKind {
    Downloads,
    Documents,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TrustedTool {
    TaskManager,
    Terminal,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ParamKind {
    String,
    Path,
    Url,
    AppId,
    Enum,
    Text,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ParameterDef {
    pub key: String,
    pub label: String,
    pub kind: ParamKind,
    pub required: bool,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub help: String,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Step {
    LaunchApp {
        id: String,
        app_id: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        on_error: ErrorPolicy,
    },
    LaunchTrustedTool {
        id: String,
        tool: TrustedTool,
        #[serde(default)]
        on_error: ErrorPolicy,
    },
    OpenFolder {
        id: String,
        path: String,
        #[serde(default)]
        skip_if_blank: bool,
        #[serde(default)]
        on_error: ErrorPolicy,
    },
    OpenKnownFolder {
        id: String,
        folder: FolderKind,
        #[serde(default)]
        on_error: ErrorPolicy,
    },
    OpenUrl {
        id: String,
        url: String,
        #[serde(default)]
        on_error: ErrorPolicy,
    },
    CopyText {
        id: String,
        text: String,
        #[serde(default)]
        on_error: ErrorPolicy,
    },
    Delay {
        id: String,
        ms: u64,
        #[serde(default)]
        on_error: ErrorPolicy,
    },
    Notify {
        id: String,
        title: String,
        body: String,
        #[serde(default)]
        on_error: ErrorPolicy,
    },
    CreateNote {
        id: String,
        directory: String,
        #[serde(default)]
        body: String,
        #[serde(default)]
        on_error: ErrorPolicy,
    },
    CheckExists {
        id: String,
        path: String,
        then_id: String,
        else_id: String,
    },
    Confirm {
        id: String,
        message: String,
        then_id: String,
        else_id: String,
    },
    RunCommand {
        id: String,
        program: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        working_directory: Option<String>,
        #[serde(default)]
        timeout_ms: Option<u64>,
        #[serde(default)]
        on_error: ErrorPolicy,
    },
    LaunchCollection {
        id: String,
        collection_id: String,
        #[serde(default = "default_true")]
        skip_missing: bool,
        #[serde(default)]
        on_error: ErrorPolicy,
    },
}

impl Step {
    pub fn id(&self) -> &str {
        match self {
            Self::LaunchApp { id, .. }
            | Self::LaunchTrustedTool { id, .. }
            | Self::OpenFolder { id, .. }
            | Self::OpenKnownFolder { id, .. }
            | Self::OpenUrl { id, .. }
            | Self::CopyText { id, .. }
            | Self::Delay { id, .. }
            | Self::Notify { id, .. }
            | Self::CreateNote { id, .. }
            | Self::CheckExists { id, .. }
            | Self::Confirm { id, .. }
            | Self::RunCommand { id, .. }
            | Self::LaunchCollection { id, .. } => id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowDocument {
    pub what_it_does: String,
    pub prerequisites_help: String,
    #[serde(default)]
    pub parameters: Vec<ParameterDef>,
    #[serde(default)]
    pub values: BTreeMap<String, String>,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TemplateStatus {
    Ready,
    NeedsSetup,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Prerequisite {
    pub status: TemplateStatus,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRecord {
    pub id: String,
    pub name: String,
    pub description: String,
    pub template_key: Option<String>,
    pub template_version: Option<i64>,
    pub customized: bool,
    pub document: WorkflowDocument,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowView {
    #[serde(flatten)]
    pub record: WorkflowRecord,
    pub prerequisite: Prerequisite,
    pub last_run: Option<WorkflowRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StepLog {
    pub step_id: String,
    pub status: String,
    pub message: String,
    pub started_at: String,
    pub finished_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RunReport {
    pub status: String,
    pub summary: String,
    pub steps: Vec<StepLog>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRun {
    pub id: String,
    pub workflow_id: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub summary: String,
    pub steps: Vec<StepLog>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StepPreview {
    pub step_id: String,
    pub action: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub subtitle: String,
    pub score: i32,
    pub indices: Vec<usize>,
    pub app_id: Option<String>,
    pub workflow_id: Option<String>,
    pub path: Option<String>,
    pub calculator_value: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IndexedFile {
    pub path: String,
    pub name: String,
    pub parent: String,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PreviewAsset {
    pub id: String,
    pub app_id: String,
    pub path: String,
    pub source: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LnkTarget {
    pub path: Option<String>,
    pub arguments: Option<String>,
    pub working_directory: Option<String>,
    pub icon_location: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    pub version: String,
    pub os: String,
    pub data_dir: String,
    pub db_path: String,
    pub app_count: i64,
    pub visible_app_count: i64,
    pub last_scan_at: Option<String>,
    pub last_scan_error: Option<String>,
    pub index_count: i64,
    pub last_index_at: Option<String>,
    pub active_shortcut: String,
    pub shortcut_warning: String,
    pub developer_commands_enabled: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExportBundle {
    pub schema_version: u32,
    pub exported_at: String,
    pub preferences: UserPreferences,
    pub apps: Vec<AppEntry>,
    pub collections: Vec<Collection>,
    pub workflows: Vec<WorkflowRecord>,
    #[serde(default)]
    pub launch_events: Vec<LaunchEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub apps: usize,
    pub collections: usize,
    pub workflows: usize,
    pub events: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OllamaReport {
    pub enabled: bool,
    pub reachable: bool,
    pub endpoint: String,
    pub models: Vec<String>,
    pub configured_model_present: Option<bool>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub text: String,
    pub executed: bool,
    pub model: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PulseFlowReport {
    pub configured: bool,
    pub executable: String,
    pub endpoint: String,
    pub reachable: Option<bool>,
    pub http_status: Option<u16>,
    pub message: String,
}

pub fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

pub fn today_iso() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

pub fn today_friendly() -> String {
    chrono::Local::now().format("%A, %B %d, %Y").to_string()
}
