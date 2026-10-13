//! Launch service: validation, status, launching and recording history.

use crate::error::{CoreError, Result};
use crate::models::*;
use crate::platform::Platform;
use crate::security;
use crate::storage::Storage;
use std::path::Path;

pub fn app_status(app: &AppEntry) -> TargetStatus {
    match app.target.kind {
        TargetKind::Packaged | TargetKind::Uri => TargetStatus::Unverified,
        TargetKind::Executable | TargetKind::Shortcut => {
            if !security::fs_exists(&app.target.path) {
                return TargetStatus::MissingTarget;
            }
            if app.target.kind == TargetKind::Shortcut {
                if let Some(r) = &app.resolved_path {
                    if !security::fs_exists(r) {
                        return TargetStatus::MissingTarget;
                    }
                }
            }
            if let Some(wd) = app.target.working_dir.as_deref().filter(|w| !w.is_empty()) {
                if !Path::new(wd).is_dir() {
                    return TargetStatus::MissingWorkingDir;
                }
            }
            TargetStatus::Ok
        }
    }
}

fn failed(msg: String, preview: String, kind: LaunchOutcomeKind) -> LaunchResult {
    LaunchResult { outcome: kind, pid: None, verified_running: None, message: msg, command_preview: preview }
}

/// Launch a target. Expected failures are returned as `LaunchResult` with a non-`Accepted` outcome.
pub fn launch_target(platform: &dyn Platform, target: &ExecutableTarget) -> LaunchResult {
    let preview = security::command_preview(target);
    if let Err(e) = security::validate_target(target) {
        let kind = if matches!(e, CoreError::NotFound(_)) { LaunchOutcomeKind::MissingTarget } else { LaunchOutcomeKind::Failed };
        let msg = match &e {
            CoreError::NotFound(m) => format!("{m}. Use Edit Registration to point to its new location."),
            other => other.to_string(),
        };
        return failed(msg, preview, kind);
    }
    match platform.launch(target) {
        Ok(mut r) => {
            r.command_preview = preview;
            r
        }
        Err(e) => failed(e.to_string(), preview, LaunchOutcomeKind::Failed),
    }
}

/// Launch a registered app (optionally with extra arguments appended) and record the attempt.
pub fn launch_app(
    storage: &Storage,
    platform: &dyn Platform,
    app_id: &str,
    extra_args: &[String],
    record_history: bool,
) -> Result<LaunchResult> {
    let app = storage.get_app(app_id)?;
    let mut target = app.target.clone();
    if app.target.kind == TargetKind::Shortcut {
        if let Some(r) = &app.resolved_path {
            if !security::fs_exists(r) {
                let res = failed(
                    format!("The shortcut's target no longer exists: {r}. Rescan or hide this entry."),
                    security::command_preview(&target),
                    LaunchOutcomeKind::MissingTarget,
                );
                storage.record_launch(Some(&app.id), &app.name, res.outcome, None, Some(&res.message), None, record_history)?;
                return Ok(res);
            }
        }
        if !extra_args.is_empty() {
            return Err(CoreError::Unsupported("extra arguments are not supported for shortcuts".into()));
        }
    } else {
        target.args.extend(extra_args.iter().cloned());
    }
    let res = launch_target(platform, &target);
    let args_display = if target.args.is_empty() { None } else { Some(security::join_windows_args(&target.args)) };
    storage.record_launch(Some(&app.id), &app.name, res.outcome, res.pid, Some(&res.message), args_display.as_deref(), record_history)?;
    Ok(res)
}
