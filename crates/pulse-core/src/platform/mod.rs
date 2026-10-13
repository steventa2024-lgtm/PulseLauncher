//! OS abstraction. `NativePlatform` is the Win32 implementation on Windows and a minimal,
//! development-only implementation elsewhere (Linux CI / contributors). The generic one never
//! pretends to support Windows-only features.

use crate::error::{CoreError, Result};
use crate::models::*;
use std::path::PathBuf;

#[cfg(windows)]
pub mod win32;
#[cfg(windows)]
pub use win32::NativePlatform;

#[cfg(not(windows))]
pub mod generic;
#[cfg(not(windows))]
pub use generic::NativePlatform;

pub trait Platform: Send + Sync {
    fn name(&self) -> &'static str;
    /// Launch an already-validated target. Must never use a shell string built from user input.
    fn launch(&self, target: &ExecutableTarget) -> Result<LaunchResult>;
    /// Open a file or folder through its shell association.
    fn open_path(&self, path: &str) -> Result<()>;
    /// Open an already-validated http(s) URL in the default browser.
    fn open_url(&self, url: &str) -> Result<()>;
    /// Open an already-validated `ms-settings:` URI.
    fn open_settings_uri(&self, uri: &str) -> Result<()> {
        let _ = uri;
        Err(CoreError::Unsupported("Windows Settings is only available on Windows".into()))
    }
    fn file_metadata(&self, path: &str) -> AppMetadata;
    /// PNG bytes for the executable's icon. Must not execute the file.
    fn icon_png(&self, path: &str) -> Option<Vec<u8>>;
    fn start_menu_roots(&self) -> Vec<(PathBuf, AppSource)>;
    /// Show the file in its folder. Default opens the parent folder.
    fn reveal_in_folder(&self, path: &str) -> Result<()> {
        let parent = std::path::Path::new(path)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .ok_or_else(|| CoreError::NotFound(format!("no parent folder for {path}")))?;
        if !parent.exists() {
            return Err(CoreError::NotFound(format!("folder not found: {}", parent.display())));
        }
        self.open_path(&parent.to_string_lossy())
    }
    fn known_folder(&self, name: &str) -> Option<PathBuf> {
        match name.to_ascii_lowercase().as_str() {
            "downloads" => dirs::download_dir(),
            "documents" => dirs::document_dir(),
            "desktop" => dirs::desktop_dir(),
            "home" => dirs::home_dir(),
            _ => None,
        }
    }
    fn scan_packaged_apps(&self) -> Result<Vec<DiscoveredApp>> {
        Err(CoreError::Unsupported("packaged app discovery is only available on Windows".into()))
    }
    fn supports_packaged_apps(&self) -> bool {
        false
    }
}

/// Metadata every platform can provide from the file system.
pub fn fs_metadata(path: &str) -> AppMetadata {
    let mut m = AppMetadata::default();
    if let Ok(md) = std::fs::metadata(path) {
        if md.is_file() {
            m.file_size = Some(md.len());
        }
        if let Ok(t) = md.modified() {
            let dt: chrono::DateTime<chrono::Utc> = t.into();
            m.modified_at = Some(dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
        }
    }
    m
}

/// Parse the JSON produced by `Get-StartApps | ConvertTo-Json -Compress` into packaged-app entries.
/// Only entries with a real AppUserModelID (`PackageFamilyName!AppId`) are accepted.
pub fn parse_start_apps_json(json: &str) -> Result<Vec<DiscoveredApp>> {
    #[derive(serde::Deserialize)]
    struct Item {
        #[serde(rename = "Name")]
        name: Option<String>,
        #[serde(rename = "AppID")]
        app_id: Option<String>,
    }
    let json = json.trim().trim_start_matches('\u{feff}');
    if json.is_empty() {
        return Ok(vec![]);
    }
    let items: Vec<Item> = if json.starts_with('[') {
        serde_json::from_str(json)?
    } else {
        vec![serde_json::from_str(json)?]
    };
    let mut out = Vec::new();
    for i in items {
        let (Some(name), Some(id)) = (i.name, i.app_id) else { continue };
        if !id.contains('!') || id.contains('\\') || id.contains(' ') || name.trim().is_empty() {
            continue;
        }
        let target = ExecutableTarget { kind: TargetKind::Packaged, path: String::new(), args: vec![], working_dir: None, aumid: Some(id.clone()), uri: None };
        if crate::security::validate_target(&target).is_err() {
            continue;
        }
        out.push(DiscoveredApp {
            name: name.trim().to_string(),
            source: AppSource::Packaged,
            identity_key: crate::security::target_identity(&target),
            target,
            resolved_path: None,
            provenance: format!("Get-StartApps: {id}"),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_start_apps() {
        let j = r#"[{"Name":"Calculator","AppID":"Microsoft.WindowsCalculator_8wekyb3d8bbwe!App"},
                   {"Name":"Notepad","AppID":"C:\\Windows\\notepad.exe"},
                   {"Name":"Bad","AppID":"x y!z"},
                   {"Name":"Photos","AppID":"Microsoft.Windows.Photos_8wekyb3d8bbwe!App"}]"#;
        let apps = parse_start_apps_json(j).unwrap();
        assert_eq!(apps.iter().map(|a| a.name.as_str()).collect::<Vec<_>>(), vec!["Calculator", "Photos"]);
        assert_eq!(parse_start_apps_json("").unwrap().len(), 0);
        assert_eq!(parse_start_apps_json(r#"{"Name":"One","AppID":"A.B_c!App"}"#).unwrap().len(), 1);
        assert!(parse_start_apps_json("not json").is_err());
    }
}
