use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, OnceLock};

use crate::error::{AppError, AppResult};
use crate::launch::{folder_name, spawn_program, Host};
use crate::models::{FolderKind, LaunchOutcome};
use crate::security::{find_on_path, home_dir, validate_existing_directory, validate_url};

pub type Notifier = Arc<dyn Fn(&str, &str) + Send + Sync>;

#[derive(Clone, Default)]
pub struct SystemHost {
    pub notifier: Option<Notifier>,
}

struct ClipJob {
    text: String,
    done: Sender<Result<(), String>>,
}

fn clipboard_sender() -> &'static Sender<ClipJob> {
    static SENDER: OnceLock<Sender<ClipJob>> = OnceLock::new();
    SENDER.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<ClipJob>();
        std::thread::Builder::new()
            .name("pulslauncher-clipboard".into())
            .spawn(move || {
                let opened = arboard::Clipboard::new().map_err(|err| err.to_string());
                match opened {
                    Ok(mut clipboard) => {
                        while let Ok(job) = rx.recv() {
                            let result =
                                clipboard.set_text(job.text).map_err(|err| err.to_string());
                            let _ = job.done.send(result);
                        }
                    }
                    Err(err) => {
                        while let Ok(job) = rx.recv() {
                            let _ = job.done.send(Err(err.clone()));
                        }
                    }
                }
            })
            .expect("clipboard thread");
        tx
    })
}

impl Host for SystemHost {
    fn launch_program(
        &self,
        program: &Path,
        args: &[String],
        cwd: Option<&Path>,
        wait: bool,
    ) -> AppResult<LaunchOutcome> {
        spawn_program(program, args, cwd, wait, None)
    }

    fn open_shell_target(&self, target: &str) -> AppResult<LaunchOutcome> {
        open::that(target).map_err(|err| {
            AppError::new(
                "LAUNCH",
                format!("The shell did not accept '{target}': {err}"),
            )
        })?;
        Ok(shell_accepted(
            "The operating system accepted the shell open request. A running process was not verified.",
        ))
    }

    fn open_folder(&self, path: &Path) -> AppResult<LaunchOutcome> {
        let path = validate_existing_directory(path)?;
        open::that(&path).map_err(|err| {
            AppError::new(
                "LAUNCH",
                format!("Could not open {}: {err}", path.display()),
            )
        })?;
        Ok(shell_accepted(
            "The file manager was asked to open the folder.",
        ))
    }

    fn open_url(&self, url: &str) -> AppResult<LaunchOutcome> {
        let url = validate_url(url)?;
        if url.to_ascii_lowercase().starts_with("ms-settings:") && !cfg!(windows) {
            return Err(AppError::new(
                "PLATFORM",
                "ms-settings links are only available on Windows.",
            ));
        }
        open::that(&url)
            .map_err(|err| AppError::new("LAUNCH", format!("Could not open URL: {err}")))?;
        Ok(shell_accepted(
            "The default handler was asked to open the URL.",
        ))
    }

    fn copy_text(&self, text: &str) -> AppResult<()> {
        // On X11 the selection is cleared when the clipboard handle is dropped.
        // One thread keeps the handle for the life of the process.
        let (done_tx, done_rx) = mpsc::channel();
        clipboard_sender()
            .send(ClipJob {
                text: text.to_string(),
                done: done_tx,
            })
            .map_err(|_| AppError::new("CLIPBOARD", "Clipboard thread stopped."))?;
        match done_rx.recv() {
            Ok(Ok(())) => Ok(()),
            Ok(Err(err)) => Err(AppError::new("CLIPBOARD", err)),
            Err(_) => Err(AppError::new("CLIPBOARD", "Clipboard thread stopped.")),
        }
    }

    fn notify(&self, title: &str, body: &str) -> AppResult<()> {
        if let Some(notifier) = &self.notifier {
            notifier(title, body);
        }
        Ok(())
    }

    fn known_folder(&self, folder: FolderKind) -> AppResult<PathBuf> {
        known_folder(folder)
    }
}

fn shell_accepted(message: &str) -> LaunchOutcome {
    LaunchOutcome {
        accepted: true,
        pid: None,
        exit_code: None,
        waited: false,
        message: message.into(),
    }
}

pub fn platform_name() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "other"
    }
}

pub fn known_folder(folder: FolderKind) -> AppResult<PathBuf> {
    #[cfg(windows)]
    {
        return windows_known_folder(folder);
    }
    #[cfg(not(windows))]
    {
        linux_known_folder(folder)
    }
}

#[cfg(not(windows))]
fn linux_known_folder(folder: FolderKind) -> AppResult<PathBuf> {
    let key = match folder {
        FolderKind::Downloads => "DOWNLOAD",
        FolderKind::Documents => "DOCUMENTS",
    };
    if let Some(path) = xdg_user_dir(key) {
        return Ok(path);
    }
    let home =
        home_dir().ok_or_else(|| AppError::new("FOLDER", "Home directory is not available."))?;
    let path = home.join(match folder {
        FolderKind::Downloads => "Downloads",
        FolderKind::Documents => "Documents",
    });
    if path.is_dir() {
        Ok(path)
    } else {
        Err(AppError::new(
            "FOLDER",
            format!("Could not resolve the {} folder.", folder_name(folder)),
        ))
    }
}

#[cfg(not(windows))]
fn xdg_user_dir(key: &str) -> Option<PathBuf> {
    if !matches!(key, "DOWNLOAD" | "DOCUMENTS" | "DESKTOP") {
        return None;
    }
    let output = std::process::Command::new("xdg-user-dir")
        .arg(key)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let path = PathBuf::from(text.trim());
    path.is_dir().then_some(path)
}

pub fn user_desktop_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        return windows_known(known_folders::KnownFolder::Desktop);
    }
    #[cfg(not(windows))]
    {
        if let Some(path) = xdg_user_dir("DESKTOP") {
            return Some(path);
        }
        home_dir()
            .map(|home| home.join("Desktop"))
            .filter(|path| path.is_dir())
    }
}

pub fn task_manager_path() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        let path = PathBuf::from(root).join("System32").join("Taskmgr.exe");
        path.is_file().then_some(path)
    }
    #[cfg(not(windows))]
    {
        None
    }
}

pub fn detect_terminal() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let mut candidates = Vec::new();
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            candidates.push(PathBuf::from(local).join(r"Microsoft\WindowsApps\wt.exe"));
        }
        candidates.push(PathBuf::from(
            r"C:\Program Files\WindowsApps\Microsoft.WindowsTerminal_8wekyb3d8bbwe\wt.exe",
        ));
        for candidate in candidates {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        find_on_path("wt.exe").or_else(|| find_on_path("wt"))
    }
    #[cfg(not(windows))]
    {
        for name in [
            "x-terminal-emulator",
            "gnome-terminal",
            "konsole",
            "xfce4-terminal",
            "kitty",
            "alacritty",
        ] {
            if let Some(path) = find_on_path(name) {
                return Some(path);
            }
        }
        None
    }
}

pub fn set_autostart(dir: &Path, enabled: bool, exe: &Path) -> AppResult<()> {
    #[cfg(windows)]
    {
        let _ = dir;
        return set_windows_run_key(enabled, exe);
    }
    #[cfg(not(windows))]
    {
        set_desktop_autostart(dir, enabled, exe)
    }
}

#[cfg(not(windows))]
fn set_desktop_autostart(dir: &Path, enabled: bool, exe: &Path) -> AppResult<()> {
    fs::create_dir_all(dir)?;
    let path = dir.join("dev.zeropulse.pulslauncher.desktop");
    if !enabled {
        if path.exists() {
            fs::remove_file(&path)?;
        }
        return Ok(());
    }
    if !exe.is_file() {
        return Err(AppError::new(
            "AUTOSTART",
            "Cannot enable startup because the application executable was not found.",
        ));
    }
    let body = format!(
        "[Desktop Entry]\nType=Application\nName=PulseLauncher\nExec=\"{}\"\nX-GNOME-Autostart-enabled=true\n",
        exe.display()
    );
    fs::write(path, body)?;
    Ok(())
}

#[cfg(windows)]
fn set_windows_run_key(enabled: bool, exe: &Path) -> AppResult<()> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
    use winreg::RegKey;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey_with_flags(
            r"Software\Microsoft\Windows\CurrentVersion\Run",
            KEY_SET_VALUE,
        )
        .map_err(|err| AppError::new("AUTOSTART", err.to_string()))?;
    if enabled {
        key.set_value("PulseLauncher", &exe.to_string_lossy().to_string())
            .map_err(|err| AppError::new("AUTOSTART", err.to_string()))?;
    } else if let Err(err) = key.delete_value("PulseLauncher") {
        if err.kind() != std::io::ErrorKind::NotFound {
            return Err(AppError::new("AUTOSTART", err.to_string()));
        }
    }
    Ok(())
}

#[cfg(windows)]
pub fn windows_start_menu_dirs() -> Vec<(PathBuf, crate::models::SourceKind)> {
    use crate::models::SourceKind;
    let mut dirs = Vec::new();
    if let Some(path) = windows_known(known_folders::KnownFolder::Programs) {
        dirs.push((path, SourceKind::StartMenuUser));
    }
    if let Some(path) = windows_known(known_folders::KnownFolder::CommonPrograms) {
        dirs.push((path, SourceKind::StartMenuCommon));
    }
    dirs
}

#[cfg(windows)]
fn windows_known_folder(folder: FolderKind) -> AppResult<PathBuf> {
    let id = match folder {
        FolderKind::Downloads => known_folders::KnownFolder::Downloads,
        FolderKind::Documents => known_folders::KnownFolder::Documents,
    };
    windows_known(id).ok_or_else(|| {
        AppError::new(
            "FOLDER",
            format!("Windows did not return the {} folder.", folder_name(folder)),
        )
    })
}

#[cfg(windows)]
fn windows_known(folder: known_folders::KnownFolder) -> Option<PathBuf> {
    known_folders::get_known_folder_path(folder)
}

#[cfg(windows)]
pub fn resolve_shortcut_com(path: &Path) -> AppResult<crate::models::LnkTarget> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::{Interface, PCWSTR};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Storage::FileSystem::WIN32_FIND_DATAW;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, IPersistFile, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED, STGM_READ,
    };
    use windows::Win32::UI::Shell::{
        IShellLinkW, ShellLink, SLR_NOSEARCH, SLR_NOTRACK, SLR_NOUPDATE, SLR_NO_UI,
    };

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .map_err(|err| AppError::new("LNK_COM", err.to_string()))?;
        let persist: IPersistFile = link
            .cast()
            .map_err(|err| AppError::new("LNK_COM", err.to_string()))?;
        let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
        wide.push(0);
        persist
            .Load(PCWSTR(wide.as_ptr()), STGM_READ)
            .map_err(|err| AppError::new("LNK_COM", err.to_string()))?;
        let flags = SLR_NO_UI.0 | SLR_NOSEARCH.0 | SLR_NOUPDATE.0 | SLR_NOTRACK.0;
        let _ = link.Resolve(HWND::default(), flags);
        let mut target_buf = [0u16; 1024];
        let mut find_data = WIN32_FIND_DATAW::default();
        link.GetPath(&mut target_buf, Some(&mut find_data as *mut _), 0)
            .map_err(|err| AppError::new("LNK_COM", err.to_string()))?;
        let mut args_buf = [0u16; 1024];
        let _ = link.GetArguments(&mut args_buf);
        let mut dir_buf = [0u16; 1024];
        let _ = link.GetWorkingDirectory(&mut dir_buf);
        let mut icon_buf = [0u16; 1024];
        let mut icon_index = 0;
        let _ = link.GetIconLocation(&mut icon_buf, &mut icon_index);
        let mut name_buf = [0u16; 1024];
        let _ = link.GetDescription(&mut name_buf);
        Ok(crate::models::LnkTarget {
            path: utf16_trim(&target_buf),
            arguments: utf16_trim(&args_buf),
            working_directory: utf16_trim(&dir_buf),
            icon_location: utf16_trim(&icon_buf),
            name: utf16_trim(&name_buf),
        })
    }
}

#[cfg(windows)]
fn utf16_trim(buf: &[u16]) -> Option<String> {
    let end = buf.iter().position(|unit| *unit == 0).unwrap_or(buf.len());
    if end == 0 {
        None
    } else {
        Some(String::from_utf16_lossy(&buf[..end]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autostart_file_is_explicit_and_removable() {
        if cfg!(windows) {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("PulseLauncher");
        fs::write(&exe, b"bin").unwrap();
        set_autostart(dir.path(), true, &exe).unwrap();
        let desktop = dir.path().join("dev.zeropulse.pulslauncher.desktop");
        let text = fs::read_to_string(&desktop).unwrap();
        assert!(text.contains("PulseLauncher"));
        assert!(!text.contains("sudo"));
        set_autostart(dir.path(), false, &exe).unwrap();
        assert!(!desktop.exists());
    }

    #[test]
    fn known_download_folder_is_a_real_directory_when_present() {
        match known_folder(FolderKind::Downloads) {
            Ok(path) => assert!(path.is_dir(), "{}", path.display()),
            Err(err) => assert!(!err.to_string().is_empty()),
        }
    }
}
