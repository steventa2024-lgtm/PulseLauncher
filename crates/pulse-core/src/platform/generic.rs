//! Non-Windows development platform. Exists so the core can be built and tested on Linux/macOS CI.

use super::*;
use std::process::{Command, Stdio};

#[derive(Default)]
pub struct NativePlatform;

impl NativePlatform {
    pub fn new() -> Self {
        Self
    }
}

impl Platform for NativePlatform {
    fn name(&self) -> &'static str {
        "generic (development only; not Windows)"
    }

    fn launch(&self, t: &ExecutableTarget) -> Result<LaunchResult> {
        let preview = crate::security::command_preview(t);
        match t.kind {
            TargetKind::Executable => {
                let mut c = Command::new(&t.path);
                c.args(&t.args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
                if let Some(wd) = t.working_dir.as_deref().filter(|w| !w.is_empty()) {
                    c.current_dir(wd);
                }
                let mut child = c.spawn().map_err(|e| CoreError::Launch(format!("could not start {}: {e}", t.path)))?;
                let pid = child.id();
                std::thread::sleep(std::time::Duration::from_millis(120));
                let running = matches!(child.try_wait(), Ok(None));
                if !running {
                    // reap
                    let _ = child.wait();
                } else {
                    std::thread::spawn(move || {
                        let _ = child.wait();
                    });
                }
                Ok(LaunchResult {
                    outcome: LaunchOutcomeKind::Accepted,
                    pid: Some(pid),
                    verified_running: Some(running),
                    message: if running { format!("Started (pid {pid})") } else { format!("Started (pid {pid}) and exited quickly") },
                    command_preview: preview,
                })
            }
            _ => Err(CoreError::Unsupported("this target type can only be launched on Windows".into())),
        }
    }

    fn open_path(&self, path: &str) -> Result<()> {
        Command::new("xdg-open")
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|e| CoreError::Launch(format!("xdg-open unavailable: {e}")))
    }

    fn open_url(&self, url: &str) -> Result<()> {
        self.open_path(url)
    }

    fn file_metadata(&self, path: &str) -> AppMetadata {
        fs_metadata(path)
    }

    fn icon_png(&self, _path: &str) -> Option<Vec<u8>> {
        None
    }

    fn start_menu_roots(&self) -> Vec<(PathBuf, AppSource)> {
        vec![]
    }
}
