//! OS adapters. Windows is the supported platform; other hosts get a small
//! development fallback so the UI and engine can be built and tested.

use std::path::PathBuf;

use crate::error::AppResult;
use crate::models::DiscoveredSource;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::*;

#[cfg(not(windows))]
mod fallback;
#[cfg(not(windows))]
pub use self::fallback::*;

/// What a shell-open request returned.
#[derive(Debug, Clone)]
pub struct ShellOpenResult {
    pub process_id: Option<u32>,
}

pub fn is_windows() -> bool {
    cfg!(windows)
}

pub fn platform_label() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux-dev"
    } else {
        "other-dev"
    }
}

pub fn downloads_dir() -> Option<PathBuf> {
    dirs::download_dir()
}

pub fn documents_dir() -> Option<PathBuf> {
    dirs::document_dir()
}

/// Detected terminal candidates as (label, absolute path). Only paths that
/// exist on this machine are returned.
pub fn terminal_candidates() -> Vec<(String, String)> {
    let mut out = Vec::new();
    if cfg!(windows) {
        let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
        let sys = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
        let pf = std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".into());
        let cands = [
            (
                "Windows Terminal",
                format!("{local}\\Microsoft\\WindowsApps\\wt.exe"),
            ),
            ("PowerShell 7", format!("{pf}\\PowerShell\\7\\pwsh.exe")),
            (
                "Windows PowerShell",
                format!("{sys}\\System32\\WindowsPowerShell\\v1.0\\powershell.exe"),
            ),
            ("Command Prompt", format!("{sys}\\System32\\cmd.exe")),
        ];
        for (label, path) in cands {
            if std::path::Path::new(&path).exists() {
                out.push((label.to_string(), path));
            }
        }
    } else {
        for (label, path) in [
            ("GNOME Terminal", "/usr/bin/gnome-terminal"),
            ("Konsole", "/usr/bin/konsole"),
            ("xterm", "/usr/bin/xterm"),
        ] {
            if std::path::Path::new(path).exists() {
                out.push((label.to_string(), path.to_string()));
            }
        }
    }
    out
}

pub fn task_manager_path() -> Option<String> {
    if !cfg!(windows) {
        return None;
    }
    let sys = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
    let p = format!("{sys}\\System32\\Taskmgr.exe");
    std::path::Path::new(&p).is_file().then_some(p)
}

/// Shared signature implemented by each OS module:
/// `shell_open(file, args, working_dir) -> AppResult<ShellOpenResult>`.
pub type ShellOpenFn = fn(&str, &[String], Option<&str>) -> AppResult<ShellOpenResult>;

#[allow(dead_code)]
fn _assert_signature() {
    let _f: ShellOpenFn = shell_open;
    let _r: fn(bool) -> Vec<(DiscoveredSource, PathBuf)> = start_menu_roots;
}
