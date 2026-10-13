//! Development fallback for non-Windows hosts. It exists so the UI and the
//! workflow engine can be exercised during development; it is not a
//! supported product target.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::ShellOpenResult;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::models::DiscoveredSource;

pub fn start_menu_roots(_include_desktop: bool) -> Vec<(DiscoveredSource, PathBuf)> {
    Vec::new()
}

pub fn shell_open(
    file: &str,
    args: &[String],
    working_dir: Option<&str>,
) -> AppResult<ShellOpenResult> {
    let p = Path::new(file);
    let is_uri = file.contains(':') && !p.exists();
    let mut cmd = if p.is_file() && args.is_empty() && !is_executable(p) || p.is_dir() || is_uri {
        let mut c = Command::new("xdg-open");
        c.arg(file);
        c
    } else {
        let mut c = Command::new(file);
        c.args(args);
        c
    };
    if let Some(wd) = working_dir {
        cmd.current_dir(wd);
    }
    let child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => {
                AppError::new(ErrorCode::TargetMissing, format!("Not found: {file}"))
            }
            std::io::ErrorKind::PermissionDenied => {
                AppError::new(ErrorCode::AccessDenied, e.to_string())
            }
            _ => AppError::new(ErrorCode::LaunchFailed, e.to_string()),
        })?;
    Ok(ShellOpenResult {
        process_id: Some(child.id()),
    })
}

fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata()
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}
