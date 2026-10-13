use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AppKind {
    /// A `.lnk` shell link. Launched by opening the link itself so Windows
    /// applies its intended semantics (args, working dir, advertised MSI links).
    Shortcut,
    /// A user-registered or discovered `.exe`.
    Executable,
    /// A packaged (MSIX/UWP) app activated by its AppUserModelID.
    Packaged,
    /// A user-approved URI / protocol (e.g. `ms-settings:`).
    Uri,
}

impl AppKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            AppKind::Shortcut => "shortcut",
            AppKind::Executable => "executable",
            AppKind::Packaged => "packaged",
            AppKind::Uri => "uri",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "shortcut" => AppKind::Shortcut,
            "executable" => AppKind::Executable,
            "packaged" => AppKind::Packaged,
            "uri" => AppKind::Uri,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveredSource {
    StartMenuUser,
    StartMenuCommon,
    Desktop,
    Packaged,
    Manual,
}

impl DiscoveredSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            DiscoveredSource::StartMenuUser => "start_menu_user",
            DiscoveredSource::StartMenuCommon => "start_menu_common",
            DiscoveredSource::Desktop => "desktop",
            DiscoveredSource::Packaged => "packaged",
            DiscoveredSource::Manual => "manual",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "start_menu_user" => DiscoveredSource::StartMenuUser,
            "start_menu_common" => DiscoveredSource::StartMenuCommon,
            "desktop" => DiscoveredSource::Desktop,
            "packaged" => DiscoveredSource::Packaged,
            "manual" => DiscoveredSource::Manual,
            _ => return None,
        })
    }
    /// Priority when deduplicating the same target found in several places.
    pub fn priority(&self) -> u8 {
        match self {
            DiscoveredSource::Manual => 0,
            DiscoveredSource::StartMenuUser => 1,
            DiscoveredSource::StartMenuCommon => 2,
            DiscoveredSource::Packaged => 3,
            DiscoveredSource::Desktop => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TargetStatus {
    Ok,
    Missing,
    Unknown,
}

/// What PulseLauncher will actually hand to the OS when launching.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExecutableTarget {
    pub kind: AppKind,
    /// Path to the .lnk / .exe, the AUMID for packaged apps, or the URI.
    pub launch_target: String,
    /// Resolved executable behind a shortcut, when it could be parsed.
    pub resolved_target: Option<String>,
    /// Arguments as an array. For shortcuts these are informational: the
    /// shortcut is opened directly so Windows applies its own arguments.
    pub arguments: Vec<String>,
    /// Raw argument string stored inside a .lnk (display only).
    pub link_arguments: Option<String>,
    pub working_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppMetadata {
    pub version: Option<String>,
    pub publisher: Option<String>,
    pub description: Option<String>,
    pub product_name: Option<String>,
    pub file_size: Option<u64>,
    pub modified_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    pub original_name: String,
    pub source: DiscoveredSource,
    pub target: ExecutableTarget,
    pub icon_path: Option<String>,
    pub icon_index: i32,
    pub favorite: bool,
    pub hidden: bool,
    pub portable: bool,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub collection_ids: Vec<String>,
    pub status: TargetStatus,
    pub metadata: AppMetadata,
    pub launch_count: i64,
    pub last_launched_at: Option<String>,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
    pub last_seen_at: Option<String>,
    pub preview_asset_id: Option<String>,
}

/// A candidate found by a discovery adapter before reconciliation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredApp {
    pub name: String,
    pub source: DiscoveredSource,
    pub target: ExecutableTarget,
    pub icon_path: Option<String>,
    pub icon_index: i32,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub system_key: Option<String>,
    pub sort_order: i64,
    pub app_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchEvent {
    pub id: i64,
    pub app_id: Option<String>,
    pub workflow_id: Option<String>,
    pub label: String,
    pub at: String,
    pub success: bool,
    pub process_id: Option<u32>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchOutcome {
    /// The OS accepted the launch request.
    pub accepted: bool,
    /// Process id when Windows returned a process handle. `None` does not
    /// mean failure: shell links, URIs and packaged apps may not expose one.
    pub process_id: Option<u32>,
    pub message: String,
    /// Exact description of what was handed to the OS.
    pub executed: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewAsset {
    pub id: String,
    pub app_id: String,
    pub file_path: String,
    pub source: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub started_at: String,
    pub finished_at: String,
    pub found: usize,
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    pub marked_missing: usize,
    pub errors: Vec<String>,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutBinding {
    pub accelerator: String,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct UserPreferences {
    pub theme: String,
    pub accent: String,
    pub density: String,
    pub reduce_motion: bool,
    pub translucency: bool,
    pub text_scale: f64,
    pub high_contrast: bool,
    pub launcher_hotkey: String,
    pub launcher_hotkey_fallbacks: Vec<String>,
    pub hide_launcher_on_blur: bool,
    pub close_to_tray: bool,
    pub start_minimized: bool,
    pub launch_at_startup: bool,
    pub search_files: bool,
    pub index_roots: Vec<String>,
    pub index_exclusions: Vec<String>,
    pub index_max_entries: usize,
    pub include_desktop_shortcuts: bool,
    pub include_packaged_apps: bool,
    pub web_search_enabled: bool,
    pub web_search_template: String,
    pub record_history: bool,
    pub developer_commands_enabled: bool,
    pub ollama_enabled: bool,
    pub ollama_endpoint: String,
    pub ollama_model: String,
    pub pulseflow_url: String,
    pub onboarding_complete: bool,
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
            launcher_hotkey: "Alt+Space".into(),
            launcher_hotkey_fallbacks: vec!["Ctrl+Alt+Space".into()],
            hide_launcher_on_blur: true,
            close_to_tray: true,
            start_minimized: false,
            launch_at_startup: false,
            search_files: false,
            index_roots: vec![],
            index_exclusions: vec![
                "node_modules".into(),
                ".git".into(),
                "target".into(),
                "AppData".into(),
            ],
            index_max_entries: 20_000,
            include_desktop_shortcuts: false,
            include_packaged_apps: true,
            web_search_enabled: false,
            web_search_template: "https://duckduckgo.com/?q={query}".into(),
            record_history: true,
            developer_commands_enabled: false,
            ollama_enabled: false,
            ollama_endpoint: "http://127.0.0.1:11434".into(),
            ollama_model: String::new(),
            pulseflow_url: String::new(),
            onboarding_complete: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyStatus {
    pub requested: String,
    pub active: Option<String>,
    pub warning: Option<String>,
}
