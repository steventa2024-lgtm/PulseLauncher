//! Windows implementations. All `unsafe` FFI for the launch path is confined
//! to this file.

use std::path::PathBuf;

use windows::core::{GUID, HSTRING, PCWSTR};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Com::{
    CoInitializeEx, CoTaskMemFree, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
};
use windows::Win32::System::Threading::GetProcessId;
use windows::Win32::UI::Shell::{
    FOLDERID_CommonPrograms, FOLDERID_Desktop, FOLDERID_Programs, FOLDERID_PublicDesktop,
    SHGetKnownFolderPath, ShellExecuteExW, KF_FLAG_DEFAULT, SEE_MASK_FLAG_NO_UI,
    SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use super::ShellOpenResult;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::models::DiscoveredSource;

fn known_folder(id: &GUID) -> Option<PathBuf> {
    // SAFETY: SHGetKnownFolderPath allocates a PWSTR that we free with
    // CoTaskMemFree after copying it.
    unsafe {
        let p = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).ok()?;
        let s = p.to_string().ok();
        CoTaskMemFree(Some(p.0 as *const _));
        s.map(PathBuf::from)
    }
}

pub fn start_menu_roots(include_desktop: bool) -> Vec<(DiscoveredSource, PathBuf)> {
    let mut v = Vec::new();
    if let Some(p) = known_folder(&FOLDERID_Programs) {
        v.push((DiscoveredSource::StartMenuUser, p));
    }
    if let Some(p) = known_folder(&FOLDERID_CommonPrograms) {
        v.push((DiscoveredSource::StartMenuCommon, p));
    }
    if include_desktop {
        if let Some(p) = known_folder(&FOLDERID_Desktop) {
            v.push((DiscoveredSource::Desktop, p));
        }
        if let Some(p) = known_folder(&FOLDERID_PublicDesktop) {
            v.push((DiscoveredSource::Desktop, p));
        }
    }
    v
}

/// Open a file, folder, shortcut, URI or packaged-app path through
/// `ShellExecuteExW` with an explicit parameter string. No command shell is
/// involved, so metacharacters in arguments are passed through literally.
pub fn shell_open(
    file: &str,
    args: &[String],
    working_dir: Option<&str>,
) -> AppResult<ShellOpenResult> {
    let params = crate::security::join_windows_args(args);
    let file_h = HSTRING::from(file);
    let params_h = HSTRING::from(params.as_str());
    let dir_h = HSTRING::from(working_dir.unwrap_or(""));
    let verb_h = HSTRING::from("open");
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_FLAG_NO_UI,
        lpVerb: PCWSTR(verb_h.as_ptr()),
        lpFile: PCWSTR(file_h.as_ptr()),
        lpParameters: if params.is_empty() {
            PCWSTR::null()
        } else {
            PCWSTR(params_h.as_ptr())
        },
        lpDirectory: if working_dir.is_some() {
            PCWSTR(dir_h.as_ptr())
        } else {
            PCWSTR::null()
        },
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    // SAFETY: all string pointers reference HSTRINGs that outlive the call;
    // `info` is a correctly sized SHELLEXECUTEINFOW. The returned process
    // handle (if any) is closed exactly once.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
        if let Err(e) = ShellExecuteExW(&mut info) {
            let code = (e.code().0 as u32) & 0xFFFF;
            let kind = match code {
                2 | 3 => ErrorCode::TargetMissing,
                5 => ErrorCode::AccessDenied,
                1223 => ErrorCode::Cancelled,
                _ => ErrorCode::LaunchFailed,
            };
            let msg = match code {
                1223 => {
                    "The launch was cancelled (for example, a UAC prompt was declined).".to_string()
                }
                1155 => "No application is associated with this file type.".to_string(),
                _ => e.message().to_string(),
            };
            return Err(AppError::new(kind, msg));
        }
        let pid = if !info.hProcess.is_invalid() {
            let pid = GetProcessId(info.hProcess);
            let _ = CloseHandle(info.hProcess);
            (pid != 0).then_some(pid)
        } else {
            None
        };
        Ok(ShellOpenResult { process_id: pid })
    }
}
