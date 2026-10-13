//! Typed data model shared by the native backend and (via serde camelCase JSON) the frontend.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TargetKind {
    /// A Win32 executable file.
    Executable,
    /// A `.lnk` shortcut that is launched through the shell (keeps its own semantics).
    Shortcut,
    /// A packaged (MSIX/UWP) app activated by AppUserModelID.
    Packaged,
    /// A protocol URI (http/https/ms-settings/...). Only approved schemes.
    Uri,
}

/// Everything required to launch something. Arguments are always an array.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutableTarget {
    pub kind: TargetKind,
    /// Absolute path for `Executable`/`Shortcut`; empty for `Packaged`/`Uri`.
    pub path: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub working_dir: Option<String>,
    /// AppUserModelID for `Packaged`.
    #[serde(default)]
    pub aumid: Option<String>,
    /// URI for `Uri`.
    #[serde(default)]
    pub uri: Option<String>,
}

impl ExecutableTarget {
    pub fn executable(path: impl Into<String>) -> Self {
        Self {
            kind: TargetKind::Executable,
            path: path.into(),
            args: vec![],
            working_dir: None,
            aumid: None,
            uri: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AppSource {
    StartMenuUser,
    StartMenuCommon,
    Packaged,
    /// `.lnk` files from the desktop, only when the user opts in.
    Desktop,
    /// Registered by the user (portable or otherwise).
    Manual,
}

impl AppSource {
    pub fn as_db(&self) -> &'static str {
        match self {
            AppSource::StartMenuUser => "start_menu_user",
            AppSource::StartMenuCommon => "start_menu_common",
            AppSource::Packaged => "packaged",
            AppSource::Desktop => "desktop",
            AppSource::Manual => "manual",
        }
    }
    pub fn from_db(s: &str) -> Self {
        match s {
            "start_menu_user" => AppSource::StartMenuUser,
            "start_menu_common" => AppSource::StartMenuCommon,
            "packaged" => AppSource::Packaged,
            "desktop" => AppSource::Desktop,
            _ => AppSource::Manual,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AppCategory {
    #[default]
    None,
    DeveloperTools,
    Productivity,
}

impl AppCategory {
    pub fn as_db(&self) -> &'static str {
        match self {
            AppCategory::None => "none",
            AppCategory::DeveloperTools => "developer_tools",
            AppCategory::Productivity => "productivity",
        }
    }
    pub fn from_db(s: &str) -> Self {
        match s {
            "developer_tools" => AppCategory::DeveloperTools,
            "productivity" => AppCategory::Productivity,
            _ => AppCategory::None,
        }
    }
}

/// Persisted application registration (discovered or manual).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    pub user_renamed: bool,
    pub source: AppSource,
    pub target: ExecutableTarget,
    /// For shortcuts: the executable the `.lnk` points to (used for icon, metadata, identity).
    pub resolved_path: Option<String>,
    /// Canonical identity used for de-duplication across sources.
    pub identity_key: String,
    /// Where the entry was found (e.g. the `.lnk` path). Empty for manual.
    pub provenance: String,
    pub publisher: Option<String>,
    pub version: Option<String>,
    pub category: AppCategory,
    pub tags: Vec<String>,
    pub favorite: bool,
    pub hidden: bool,
    /// Entry was not seen by the most recent scan of its source.
    pub stale: bool,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
    pub collection_ids: Vec<String>,
    pub launch_count: i64,
    pub last_launched_at: Option<String>,
}

impl AppEntry {
    pub fn is_portable(&self) -> bool {
        self.source == AppSource::Manual && self.target.kind == TargetKind::Executable
    }
}

/// Output of discovery before persistence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredApp {
    pub name: String,
    pub source: AppSource,
    pub target: ExecutableTarget,
    pub resolved_path: Option<String>,
    pub identity_key: String,
    pub provenance: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanIssue {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub found: usize,
    pub added: usize,
    pub updated: usize,
    pub marked_stale: usize,
    pub duplicates_merged: usize,
    pub skipped_uninstallers: usize,
    pub skipped_no_target: usize,
    pub issues: Vec<ScanIssue>,
    pub scanned_roots: Vec<String>,
    pub finished_at: String,
    pub duration_ms: u64,
}

/// Verified metadata from the file system / PE version resource. All optional.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppMetadata {
    pub version: Option<String>,
    pub publisher: Option<String>,
    pub description: Option<String>,
    pub file_size: Option<u64>,
    pub modified_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TargetStatus {
    Ok,
    MissingTarget,
    MissingWorkingDir,
    /// Packaged apps / URIs: existence cannot be verified from the file system.
    Unverified,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub name: String,
    /// Set for built-in collections such as `zeropulse`.
    pub system_key: Option<String>,
    pub sort_order: i64,
    pub app_count: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewAsset {
    pub id: String,
    pub app_id: String,
    pub file_name: String,
    pub caption: Option<String>,
    pub source: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LaunchOutcomeKind {
    /// The OS accepted the launch request. This does NOT prove the app stayed running.
    Accepted,
    Failed,
    MissingTarget,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchEvent {
    pub id: i64,
    pub app_id: Option<String>,
    pub app_name: String,
    pub at: String,
    pub outcome: LaunchOutcomeKind,
    pub pid: Option<u32>,
    pub detail: Option<String>,
    pub args_display: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchResult {
    pub outcome: LaunchOutcomeKind,
    pub pid: Option<u32>,
    /// `Some(true)` only when the platform verified the process was alive shortly after launch.
    pub verified_running: Option<bool>,
    pub message: String,
    pub command_preview: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutBinding {
    pub action: String,
    pub accelerator: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UserPreferences {
    pub theme: String,
    pub accent: String,
    pub density: String,
    pub reduce_motion: bool,
    pub translucency: bool,
    pub text_scale: f32,
    pub high_contrast: bool,
    pub launcher_shortcut: String,
    pub close_on_blur: bool,
    pub close_to_tray: bool,
    pub start_with_windows: bool,
    pub show_web_search: bool,
    pub web_search_url: String,
    pub index_files: bool,
    pub index_roots: Vec<String>,
    pub index_exclusions: Vec<String>,
    pub record_history: bool,
    pub developer_command_execution: bool,
    pub onboarding_done: bool,
    pub desktop_shortcuts_scan: bool,
    pub ollama_enabled: bool,
    pub ollama_endpoint: String,
    pub ollama_model: String,
    /// Optional localhost/CLI endpoint the user configures for PulseFlow. Empty means not set.
    pub pulseflow_endpoint: String,
    /// Optional URL the user configures for OpenPulse. Empty means not set.
    pub openpulse_url: String,
}

impl Default for UserPreferences {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            accent: "#4388FF".into(),
            density: "comfortable".into(),
            reduce_motion: false,
            translucency: true,
            text_scale: 1.0,
            high_contrast: false,
            launcher_shortcut: "Alt+Space".into(),
            close_on_blur: true,
            close_to_tray: true,
            start_with_windows: false,
            show_web_search: true,
            web_search_url: "https://duckduckgo.com/?q={query}".into(),
            index_files: false,
            index_roots: vec![],
            index_exclusions: vec!["node_modules".into(), ".git".into(), "target".into()],
            record_history: true,
            developer_command_execution: false,
            onboarding_done: false,
            desktop_shortcuts_scan: false,
            ollama_enabled: false,
            ollama_endpoint: "http://127.0.0.1:11434".into(),
            ollama_model: String::new(),
            pulseflow_endpoint: String::new(),
            openpulse_url: String::new(),
        }
    }
}
