//! Authorised launch service. Only typed, validated targets reach the OS:
//! registered apps (by id), existing folders, validated URLs and allow-listed
//! URIs. Nothing here builds a shell command string.

use std::path::Path;

use crate::error::{AppError, AppResult, ErrorCode};
use crate::models::*;
use crate::platform;
use crate::security;
use crate::storage::apps::target_status;

pub struct LaunchPlan {
    pub file: String,
    pub args: Vec<String>,
    pub working_dir: Option<String>,
    pub description: String,
}

/// Work out exactly what would be executed, without executing it.
pub fn plan_app_launch(app: &AppEntry, override_args: Option<&[String]>) -> AppResult<LaunchPlan> {
    let t = &app.target;
    match t.kind {
        AppKind::Executable => {
            let args = override_args
                .map(|a| a.to_vec())
                .unwrap_or_else(|| t.arguments.clone());
            security::validate_args(&args)?;
            security::validate_absolute_path(&t.launch_target, "Executable")?;
            Ok(LaunchPlan {
                description: security::preview_command(&t.launch_target, &args),
                file: t.launch_target.clone(),
                args,
                working_dir: t.working_dir.clone(),
            })
        }
        AppKind::Shortcut => {
            if override_args.is_some_and(|a| !a.is_empty()) {
                return Err(AppError::unsupported(
                    "Shortcuts are opened as-is so Windows applies their own arguments.",
                ));
            }
            Ok(LaunchPlan {
                description: format!("Open shortcut {}", t.launch_target),
                file: t.launch_target.clone(),
                args: vec![],
                working_dir: None,
            })
        }
        AppKind::Packaged => {
            if !platform::is_windows() {
                return Err(AppError::unsupported(
                    "Packaged apps can only be activated on Windows.",
                ));
            }
            security::validate_registration(t)?;
            Ok(LaunchPlan {
                description: format!("Activate packaged app {}", t.launch_target),
                file: format!("shell:AppsFolder\\{}", t.launch_target),
                args: vec![],
                working_dir: None,
            })
        }
        AppKind::Uri => {
            let uri = security::validate_uri(&t.launch_target)?;
            Ok(LaunchPlan {
                description: format!("Open {uri}"),
                file: uri,
                args: vec![],
                working_dir: None,
            })
        }
    }
}

pub fn launch_app(app: &AppEntry, override_args: Option<&[String]>) -> AppResult<LaunchOutcome> {
    if target_status(&app.target) == TargetStatus::Missing {
        let path = app
            .target
            .resolved_target
            .as_deref()
            .filter(|_| {
                app.target.kind == AppKind::Shortcut
                    && Path::new(&app.target.launch_target).exists()
            })
            .unwrap_or(&app.target.launch_target);
        return Err(AppError::new(
            ErrorCode::TargetMissing,
            format!(
                "\"{}\" cannot be launched because its target is missing: {path}",
                app.name
            ),
        ));
    }
    if let Some(wd) = &app.target.working_dir {
        if app.target.kind == AppKind::Executable && !Path::new(wd).is_dir() {
            return Err(AppError::new(
                ErrorCode::TargetMissing,
                format!("Working directory not found: {wd}"),
            ));
        }
    }
    let plan = plan_app_launch(app, override_args)?;
    let r = platform::shell_open(&plan.file, &plan.args, plan.working_dir.as_deref())?;
    Ok(LaunchOutcome {
        accepted: true,
        process_id: r.process_id,
        message: match r.process_id {
            Some(pid) => format!("Windows started \"{}\" (process {pid}).", app.name),
            None => format!(
                "Windows accepted the request to open \"{}\". No process id was returned, so the running process was not verified.",
                app.name
            ),
        },
        executed: plan.description,
    })
}

pub fn open_folder(path: &str) -> AppResult<LaunchOutcome> {
    let p = security::validate_absolute_path(path, "Folder")?;
    if !p.is_dir() {
        return Err(AppError::new(
            ErrorCode::TargetMissing,
            format!("Folder not found: {}", p.display()),
        ));
    }
    let s = p.to_string_lossy().into_owned();
    let r = platform::shell_open(&s, &[], None)?;
    Ok(LaunchOutcome {
        accepted: true,
        process_id: r.process_id,
        message: format!("Opened folder {s}"),
        executed: format!("Open folder {s}"),
    })
}

/// Open an existing file with its default association. Executables are
/// rejected here: running programs requires an explicit registration.
pub fn open_file(path: &str) -> AppResult<LaunchOutcome> {
    let p = security::validate_absolute_path(path, "File")?;
    if !p.is_file() {
        return Err(AppError::new(
            ErrorCode::TargetMissing,
            format!("File not found: {}", p.display()),
        ));
    }
    const RISKY: &[&str] = &[
        "exe",
        "com",
        "bat",
        "cmd",
        "ps1",
        "vbs",
        "vbe",
        "js",
        "jse",
        "wsf",
        "wsh",
        "msi",
        "msp",
        "scr",
        "pif",
        "lnk",
        "reg",
        "hta",
        "cpl",
        "msc",
        "jar",
        "appref-ms",
        "application",
    ];
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if RISKY.contains(&ext.as_str()) {
        return Err(AppError::policy(
            "Programs and scripts are not opened from file search. Register the program with Add App instead.",
        ));
    }
    let s = p.to_string_lossy().into_owned();
    let r = platform::shell_open(&s, &[], None)?;
    Ok(LaunchOutcome {
        accepted: true,
        process_id: r.process_id,
        message: format!("Opened {s}"),
        executed: format!("Open file {s}"),
    })
}

pub fn open_url(raw: &str) -> AppResult<LaunchOutcome> {
    let url = security::validate_web_url(raw)?;
    platform::shell_open(url.as_str(), &[], None)?;
    Ok(LaunchOutcome {
        accepted: true,
        process_id: None,
        message: format!("Opened {} in your default browser", url.as_str()),
        executed: format!("Open URL {}", url.as_str()),
    })
}

pub fn open_uri(raw: &str) -> AppResult<LaunchOutcome> {
    let uri = security::validate_uri(raw)?;
    if uri.starts_with("http://") || uri.starts_with("https://") {
        return open_url(&uri);
    }
    if !platform::is_windows() {
        return Err(AppError::unsupported(
            "Protocol activation is only available on Windows.",
        ));
    }
    platform::shell_open(&uri, &[], None)?;
    Ok(LaunchOutcome {
        accepted: true,
        process_id: None,
        message: format!("Windows accepted {uri}"),
        executed: format!("Open URI {uri}"),
    })
}

/// Launch an absolute program path with an argument array. Used by
/// workflow steps after validation and (for untrusted workflows) review.
pub fn launch_path(
    path: &str,
    args: &[String],
    working_dir: Option<&str>,
) -> AppResult<LaunchOutcome> {
    let p = security::validate_absolute_path(path, "Program")?;
    security::validate_args(args)?;
    if !p.is_file() {
        return Err(AppError::new(
            ErrorCode::TargetMissing,
            format!("Program not found: {}", p.display()),
        ));
    }
    let s = p.to_string_lossy().into_owned();
    let r = platform::shell_open(&s, args, working_dir)?;
    Ok(LaunchOutcome {
        accepted: true,
        process_id: r.process_id,
        message: match r.process_id {
            Some(pid) => format!("Started {s} (process {pid})"),
            None => format!("Windows accepted the request to open {s}"),
        },
        executed: security::preview_command(&s, args),
    })
}

#[derive(Debug)]
pub struct CapturedRun {
    pub output: String,
    pub exit_code: Option<i32>,
}

/// Run an absolute program with an argument vector and capture its output.
/// No shell is created, so metacharacters in arguments stay literal.
/// Output is capped and passed through secret redaction.
pub fn run_captured(
    program: &str,
    args: &[String],
    timeout: std::time::Duration,
) -> AppResult<CapturedRun> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::time::Instant;

    let path = security::validate_absolute_path(program, "Program")?;
    security::validate_args(args)?;
    if !path.is_file() {
        return Err(AppError::new(
            ErrorCode::TargetMissing,
            format!("Program not found: {}", path.display()),
        ));
    }
    #[cfg(windows)]
    {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ext != "exe" && ext != "com" {
            return Err(AppError::policy(
                "Only .exe programs can be run directly. PulseLauncher does not run scripts.",
            ));
        }
    }
    let program = path.to_string_lossy().into_owned();
    let mut cmd = Command::new(&program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd.spawn().map_err(|e| match e.kind() {
        std::io::ErrorKind::PermissionDenied => {
            AppError::new(ErrorCode::AccessDenied, e.to_string())
        }
        std::io::ErrorKind::NotFound => AppError::new(ErrorCode::TargetMissing, e.to_string()),
        _ => AppError::new(ErrorCode::LaunchFailed, e.to_string()),
    })?;
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(out) = stdout.as_mut() {
            let _ = out.take(16 * 1024).read_to_end(&mut buf);
        }
        buf.push(b'\n');
        if let Some(err) = stderr.as_mut() {
            let _ = err.take(16 * 1024).read_to_end(&mut buf);
        }
        buf
    });
    let started = Instant::now();
    let status = loop {
        if started.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            return Err(AppError::new(
                ErrorCode::Timeout,
                format!(
                    "The command exceeded {} seconds and was stopped.",
                    timeout.as_secs().max(1)
                ),
            ));
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(20)),
            Err(e) => return Err(AppError::new(ErrorCode::LaunchFailed, e.to_string())),
        }
    };
    let bytes = reader.join().unwrap_or_default();
    let mut output = security::redact(&String::from_utf8_lossy(&bytes));
    if output.chars().count() > 4000 {
        output = output.chars().take(4000).collect::<String>() + "…";
    }
    let code = status.code();
    if !status.success() {
        return Err(AppError::new(
            ErrorCode::LaunchFailed,
            format!(
                "The command exited with status {}. {}",
                code.map(|c| c.to_string())
                    .unwrap_or_else(|| "unknown".into()),
                output.trim()
            ),
        ));
    }
    Ok(CapturedRun {
        output,
        exit_code: code,
    })
}

pub fn reveal_in_folder(path: &str) -> AppResult<LaunchOutcome> {
    let p = security::validate_absolute_path(path, "File")?;
    if !p.exists() {
        return Err(AppError::new(
            ErrorCode::TargetMissing,
            format!("Not found: {}", p.display()),
        ));
    }
    let s = p.to_string_lossy().into_owned();
    if platform::is_windows() {
        let sys = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
        let explorer = format!("{sys}\\explorer.exe");
        platform::shell_open(&explorer, &["/select,".into(), s.clone()], None)?;
    } else {
        let dir = p
            .parent()
            .map(|d| d.to_string_lossy().into_owned())
            .unwrap_or(s.clone());
        platform::shell_open(&dir, &[], None)?;
    }
    Ok(LaunchOutcome {
        accepted: true,
        process_id: None,
        message: format!("Showing {s} in the file manager"),
        executed: format!("Reveal {s}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(kind: AppKind, target: &str) -> AppEntry {
        AppEntry {
            id: "x".into(),
            name: "X".into(),
            original_name: "X".into(),
            source: DiscoveredSource::Manual,
            target: ExecutableTarget {
                kind,
                launch_target: target.into(),
                resolved_target: None,
                arguments: vec!["--a".into(), "b c".into()],
                link_arguments: None,
                working_dir: None,
            },
            icon_path: None,
            icon_index: 0,
            favorite: false,
            hidden: false,
            portable: true,
            category: None,
            tags: vec![],
            collection_ids: vec![],
            status: TargetStatus::Unknown,
            metadata: Default::default(),
            launch_count: 0,
            last_launched_at: None,
            sort_order: 0,
            created_at: String::new(),
            updated_at: String::new(),
            last_seen_at: None,
            preview_asset_id: None,
        }
    }

    #[test]
    fn plans_show_exact_command() {
        let p =
            plan_app_launch(&app(AppKind::Executable, "C:\\Pulse Tools\\cut.exe"), None).unwrap();
        assert_eq!(p.description, "\"C:\\Pulse Tools\\cut.exe\" --a \"b c\"");
        assert_eq!(p.args, vec!["--a", "b c"]);
    }

    #[test]
    fn missing_executable_is_not_reported_as_success() {
        let err = launch_app(&app(AppKind::Executable, "C:\\nope\\missing.exe"), None).unwrap_err();
        assert_eq!(err.code, ErrorCode::TargetMissing);
    }

    #[test]
    fn rejects_unsafe_open_requests() {
        assert!(open_url("javascript:alert(1)").is_err());
        assert!(open_uri("ms-msdt:/id x").is_err());
        assert!(open_folder("relative/dir").is_err());
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("evil.bat");
        std::fs::write(&script, b"echo").unwrap();
        let e = open_file(script.to_str().unwrap()).unwrap_err();
        assert_eq!(e.code, ErrorCode::PolicyBlocked);
        assert_eq!(
            open_folder(dir.path().join("nope").to_str().unwrap())
                .unwrap_err()
                .code,
            ErrorCode::TargetMissing
        );
    }

    #[test]
    fn captured_command_does_not_use_a_shell() {
        if !std::path::Path::new("/bin/echo").exists() {
            eprintln!("/bin/echo missing; skipping");
            return;
        }
        let arg = "hello; $(uname) & password=hunter2".to_string();
        let args = [arg];
        let run = run_captured("/bin/echo", &args, std::time::Duration::from_secs(5)).unwrap();
        assert!(run.output.contains("hello; $(uname)"));
        assert!(!run.output.contains("hunter2"));
        if std::path::Path::new("/bin/sleep").is_file() {
            let timed = run_captured(
                "/bin/sleep",
                &["5".into()],
                std::time::Duration::from_millis(200),
            )
            .unwrap_err();
            assert_eq!(timed.code, ErrorCode::Timeout);
        }
        let missing =
            run_captured("/no/such/program", &[], std::time::Duration::from_secs(1)).unwrap_err();
        assert_eq!(missing.code, ErrorCode::TargetMissing);
    }

    #[test]
    fn shortcuts_reject_extra_args() {
        let a = app(AppKind::Shortcut, "C:\\x.lnk");
        assert!(plan_app_launch(&a, Some(&["x".to_string()])).is_err());
    }
}
