use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::error::{AppError, AppResult};
use crate::models::{AppEntry, FolderKind, LaunchOutcome, LaunchPreview};
use crate::security::{
    display_command, is_shell_link, is_uri_target, validate_arguments, validate_existing_directory,
    validate_existing_program, validate_url,
};

pub trait Host {
    fn launch_program(
        &self,
        program: &Path,
        args: &[String],
        cwd: Option<&Path>,
        wait: bool,
    ) -> AppResult<LaunchOutcome>;
    fn open_shell_target(&self, target: &str) -> AppResult<LaunchOutcome>;
    fn open_folder(&self, path: &Path) -> AppResult<LaunchOutcome>;
    fn open_url(&self, url: &str) -> AppResult<LaunchOutcome>;
    fn copy_text(&self, text: &str) -> AppResult<()>;
    fn notify(&self, title: &str, body: &str) -> AppResult<()>;
    fn known_folder(&self, folder: FolderKind) -> AppResult<PathBuf>;
}

pub fn spawn_program(
    program: &Path,
    args: &[String],
    cwd: Option<&Path>,
    wait: bool,
    timeout: Option<Duration>,
) -> AppResult<LaunchOutcome> {
    validate_arguments(args)?;
    let program = validate_existing_program(program)?;
    if is_shell_link(&program) {
        return Err(AppError::new(
            "LAUNCH",
            "Shell shortcuts are opened through the shell adapter, not as a raw process.",
        ));
    }
    if let Some(dir) = cwd {
        validate_existing_directory(dir)?;
    }
    let mut command = Command::new(&program);
    command.args(args);
    command.stdin(Stdio::null());
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }
    let mut child = command.spawn().map_err(|err| {
        AppError::new(
            "LAUNCH",
            format!("The operating system rejected {}: {err}", program.display()),
        )
    })?;
    let pid = child.id();
    if !wait && timeout.is_none() {
        return Ok(LaunchOutcome {
            accepted: true,
            pid: Some(pid),
            exit_code: None,
            waited: false,
            message: "The operating system accepted the launch request. PulseLauncher does not claim the process is still running.".into(),
        });
    }
    let timeout = timeout.unwrap_or(Duration::from_secs(30));
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return Ok(LaunchOutcome {
                    accepted: true,
                    pid: Some(pid),
                    exit_code: status.code(),
                    waited: true,
                    message: format!(
                        "Process exited with status {}.",
                        status.code().unwrap_or(-1)
                    ),
                });
            }
            Ok(None) if started.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(AppError::new(
                    "TIMEOUT",
                    format!(
                        "Process {pid} exceeded {} ms and was stopped.",
                        timeout.as_millis()
                    ),
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(err) => return Err(AppError::new("LAUNCH", err.to_string())),
        }
    }
}

pub fn preview_app(app: &AppEntry, extra_args: &[String]) -> AppResult<LaunchPreview> {
    let mut args = app.arguments.clone();
    args.extend(extra_args.iter().cloned());
    validate_arguments(&args)?;
    if is_uri_target(&app.launch_target) {
        let url = validate_url(&app.launch_target)?;
        return Ok(LaunchPreview {
            program: url.clone(),
            arguments: Vec::new(),
            working_directory: None,
            display: url,
            via_shell: true,
        });
    }
    let program = app
        .resolved_target
        .clone()
        .filter(|path| !path.is_empty())
        .unwrap_or_else(|| app.launch_target.clone());
    if is_shell_link(Path::new(&program)) || is_shell_link(Path::new(&app.launch_target)) {
        return Ok(LaunchPreview {
            program: app.launch_target.clone(),
            arguments: args,
            working_directory: app.working_directory.clone(),
            display: format!("shell open {}", display_command(&app.launch_target, &[])),
            via_shell: true,
        });
    }
    Ok(LaunchPreview {
        program: program.clone(),
        arguments: args.clone(),
        working_directory: app.working_directory.clone(),
        display: display_command(&program, &args),
        via_shell: false,
    })
}

pub fn launch_app(
    host: &dyn Host,
    app: &AppEntry,
    extra_args: &[String],
    wait: bool,
) -> AppResult<LaunchOutcome> {
    if app.missing {
        return Err(AppError::new(
            "MISSING_TARGET",
            format!(
                "{} is missing. Edit the path or restore the file before launching.",
                app.display_name
            ),
        ));
    }
    let preview = preview_app(app, extra_args)?;
    if preview.via_shell {
        if is_uri_target(&preview.program) {
            return host.open_url(&preview.program);
        }
        return host.open_shell_target(&preview.program);
    }
    let cwd = app
        .working_directory
        .as_deref()
        .map(Path::new)
        .filter(|path| !path.as_os_str().is_empty());
    host.launch_program(Path::new(&preview.program), &preview.arguments, cwd, wait)
}

#[derive(Default)]
pub struct RecordHost {
    pub calls: std::sync::Mutex<Vec<String>>,
    pub folders: std::sync::Mutex<Vec<(String, PathBuf)>>,
    pub clipboard: std::sync::Mutex<String>,
    pub notifications: std::sync::Mutex<Vec<(String, String)>>,
    pub fail_launch: bool,
}

impl RecordHost {
    fn push(&self, call: String) {
        self.calls.lock().expect("record host").push(call);
    }
}

impl Host for RecordHost {
    fn launch_program(
        &self,
        program: &Path,
        args: &[String],
        cwd: Option<&Path>,
        wait: bool,
    ) -> AppResult<LaunchOutcome> {
        self.push(format!(
            "launch {} args={} cwd={} wait={wait}",
            program.display(),
            display_command("", args),
            cwd.map(|path| path.display().to_string())
                .unwrap_or_default()
        ));
        if self.fail_launch {
            return Err(AppError::new(
                "LAUNCH",
                "Recorded launch was configured to fail.",
            ));
        }
        Ok(LaunchOutcome {
            accepted: true,
            pid: None,
            exit_code: wait.then_some(0),
            waited: wait,
            message: "Recorded launch. No process was started.".into(),
        })
    }

    fn open_shell_target(&self, target: &str) -> AppResult<LaunchOutcome> {
        self.push(format!("shell {target}"));
        Ok(accepted("Recorded shell open."))
    }

    fn open_folder(&self, path: &Path) -> AppResult<LaunchOutcome> {
        self.push(format!("folder {}", path.display()));
        Ok(accepted("Recorded folder open."))
    }

    fn open_url(&self, url: &str) -> AppResult<LaunchOutcome> {
        let url = validate_url(url)?;
        self.push(format!("url {url}"));
        Ok(accepted("Recorded URL open."))
    }

    fn copy_text(&self, text: &str) -> AppResult<()> {
        *self.clipboard.lock().expect("clipboard") = text.to_string();
        self.push(format!("copy {text}"));
        Ok(())
    }

    fn notify(&self, title: &str, body: &str) -> AppResult<()> {
        self.notifications
            .lock()
            .expect("notifications")
            .push((title.to_string(), body.to_string()));
        self.push(format!("notify {title}"));
        Ok(())
    }

    fn known_folder(&self, folder: FolderKind) -> AppResult<PathBuf> {
        self.folders
            .lock()
            .expect("folders")
            .iter()
            .find(|(name, _)| name == folder_name(folder))
            .map(|(_, path)| path.clone())
            .ok_or_else(|| {
                AppError::new(
                    "FOLDER",
                    format!("{} is not configured.", folder_name(folder)),
                )
            })
    }
}

pub fn folder_name(folder: FolderKind) -> &'static str {
    match folder {
        FolderKind::Downloads => "downloads",
        FolderKind::Documents => "documents",
    }
}

fn accepted(message: &str) -> LaunchOutcome {
    LaunchOutcome {
        accepted: true,
        pid: None,
        exit_code: None,
        waited: false,
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_marker_source(path: &Path) {
        let mut file = std::fs::File::create(path).unwrap();
        writeln!(
            file,
            r#"
#include <stdio.h>
int main(int argc, char **argv) {{
    if (argc != 3) return 2;
    FILE *f = fopen(argv[2], "wb");
    if (!f) return 3;
    if (fputs(argv[1], f) < 0) {{
        fclose(f);
        return 4;
    }}
    fclose(f);
    return 0;
}}
"#
        )
        .unwrap();
    }

    #[test]
    fn launches_real_process_with_spaced_unicode_path_and_rejects_missing() {
        let compiler = which_cc();
        if compiler.is_none() && cfg!(windows) {
            let system = PathBuf::from(r"C:\Windows\System32\where.exe");
            if system.is_file() {
                let outcome = spawn_program(
                    &system,
                    &["where.exe".into()],
                    None,
                    true,
                    Some(Duration::from_secs(10)),
                )
                .unwrap();
                assert!(outcome.accepted);
                assert!(outcome.waited);
            }
            return;
        }
        let compiler = compiler.expect("a C compiler is required to verify process launch");
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("Pulse Tools ünïcode");
        std::fs::create_dir_all(&folder).unwrap();
        let source = folder.join("marker.c");
        let binary_name = if cfg!(windows) {
            "marker.exe"
        } else {
            "marker"
        };
        let binary = folder.join(binary_name);
        write_marker_source(&source);
        let status = std::process::Command::new(compiler)
            .arg(&source)
            .arg("-O2")
            .arg("-o")
            .arg(&binary)
            .status()
            .unwrap();
        assert!(status.success(), "compiler failed");
        let output = folder.join("result file.txt");
        let weird = "payload ; rm -rf / && $(nope)".to_string();
        let outcome = spawn_program(
            &binary,
            &[weird.clone(), output.to_string_lossy().to_string()],
            Some(&folder),
            true,
            Some(Duration::from_secs(10)),
        )
        .unwrap();
        assert!(outcome.accepted);
        assert_eq!(outcome.exit_code, Some(0));
        assert_eq!(std::fs::read_to_string(&output).unwrap(), weird);
        let moved = folder.join("moved-marker");
        std::fs::rename(&binary, &moved).unwrap();
        let error = spawn_program(&binary, &[], None, false, None).unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains("does not exist")
                || message.contains("MISSING_TARGET")
                || message.contains("Target")
        );
    }

    fn which_cc() -> Option<PathBuf> {
        for name in ["cc", "gcc", "clang"] {
            if let Some(path) = crate::security::find_on_path(name) {
                return Some(path);
            }
        }
        None
    }
}
