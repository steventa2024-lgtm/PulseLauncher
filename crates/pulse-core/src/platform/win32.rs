//! Win32 integration. Unsafe code stays in this module.
//!
//! Launching uses `ShellExecuteExW` (shortcuts, executables, http(s), `ms-settings:`) or
//! `IApplicationActivationManager` (packaged apps). Icons are read with `PrivateExtractIconsW`
//! and drawn into a bitmap; the executable is never run to obtain an icon.

use super::*;
use std::path::PathBuf;
use ::windows::core::{w, PCWSTR};
use ::windows::Win32::Foundation::{CloseHandle, HANDLE, STILL_ACTIVE};
use ::windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject, BITMAPINFO,
    BITMAPINFOHEADER, DIB_RGB_COLORS, HBITMAP,
};
use ::windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
use ::windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED,
};
use ::windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
use ::windows::Win32::UI::Shell::{
    ExtractIconExW, SHGetKnownFolderPath, ShellExecuteExW, AO_NONE, ApplicationActivationManager,
    FOLDERID_CommonPrograms, FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Downloads, FOLDERID_Programs,
    IApplicationActivationManager, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
};
use ::windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, DrawIconEx, DI_NORMAL, HICON, SW_SHOWNORMAL};

pub struct NativePlatform;

impl NativePlatform {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NativePlatform {
    fn default() -> Self {
        Self::new()
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn from_wide_ptr(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    unsafe {
        let mut len = 0usize;
        while *p.add(len) != 0 && len < 4096 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
    }
}

fn known_folder(id: *const ::windows::core::GUID) -> Option<PathBuf> {
    unsafe {
        let pw = SHGetKnownFolderPath(id, Default::default(), None).ok()?;
        let s = from_wide_ptr(pw.0);
        CoTaskMemFree(Some(pw.0 as *const _));
        if s.is_empty() {
            None
        } else {
            Some(PathBuf::from(s))
        }
    }
}

fn com_init() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
}

impl Platform for NativePlatform {
    fn name(&self) -> &'static str {
        "windows"
    }

    fn launch(&self, target: &ExecutableTarget) -> Result<LaunchResult> {
        let preview = crate::security::command_preview(target);
        match target.kind {
            TargetKind::Packaged => self.launch_packaged(target, preview),
            TargetKind::Uri => {
                let uri = target.uri.clone().unwrap_or_default();
                shell_execute(&uri, "", target.working_dir.as_deref(), preview)
            }
            TargetKind::Executable | TargetKind::Shortcut => {
                let params = if target.kind == TargetKind::Shortcut {
                    String::new()
                } else {
                    crate::security::join_windows_args(&target.args)
                };
                shell_execute(&target.path, &params, target.working_dir.as_deref(), preview)
            }
        }
    }

    fn open_path(&self, path: &str) -> Result<()> {
        shell_execute(path, "", None, path.to_string()).map(|_| ())
    }

    fn open_url(&self, url: &str) -> Result<()> {
        shell_execute(url, "", None, url.to_string()).map(|_| ())
    }

    fn open_settings_uri(&self, uri: &str) -> Result<()> {
        shell_execute(uri, "", None, uri.to_string()).map(|_| ())
    }

    fn file_metadata(&self, path: &str) -> AppMetadata {
        let mut meta = fs_metadata(path);
        if let Some((ver, company, desc)) = version_info(path) {
            meta.version = ver;
            meta.publisher = company;
            meta.description = desc;
        }
        meta
    }

    fn icon_png(&self, path: &str) -> Option<Vec<u8>> {
        extract_icon_png(path, 32)
    }

    fn start_menu_roots(&self) -> Vec<(PathBuf, AppSource)> {
        let mut roots = Vec::new();
        if let Some(p) = known_folder(&FOLDERID_Programs) {
            roots.push((p, AppSource::StartMenuUser));
        }
        if let Some(p) = known_folder(&FOLDERID_CommonPrograms) {
            roots.push((p, AppSource::StartMenuCommon));
        }
        roots
    }

    fn scan_packaged_apps(&self) -> Result<Vec<DiscoveredApp>> {
        let mut cmd = std::process::Command::new("powershell.exe");
        cmd.args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Get-StartApps | ConvertTo-Json -Compress",
        ]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        let out = cmd.output().map_err(|e| CoreError::Launch(format!("could not run Get-StartApps: {e}")))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            return Err(CoreError::Launch(format!("Get-StartApps failed: {err}")));
        }
        let text = String::from_utf8_lossy(&out.stdout);
        super::parse_start_apps_json(&text)
    }

    fn supports_packaged_apps(&self) -> bool {
        true
    }

    fn reveal_in_folder(&self, path: &str) -> Result<()> {
        if !crate::security::fs_exists(path) {
            return Err(CoreError::NotFound(format!("not found: {path}")));
        }
        let param = format!("/select,{}", path);
        shell_execute("explorer.exe", &param, None, format!("explorer.exe {param}")).map(|_| ())
    }

    fn known_folder(&self, name: &str) -> Option<PathBuf> {
        let id: *const ::windows::core::GUID = match name.to_ascii_lowercase().as_str() {
            "downloads" => &FOLDERID_Downloads,
            "documents" => &FOLDERID_Documents,
            "desktop" => &FOLDERID_Desktop,
            "programs" => &FOLDERID_Programs,
            _ => return None,
        };
        known_folder(id).or_else(|| match name {
            "home" => dirs::home_dir(),
            _ => None,
        })
    }
}

impl NativePlatform {
    fn launch_packaged(&self, target: &ExecutableTarget, preview: String) -> Result<LaunchResult> {
        let aumid = target.aumid.clone().unwrap_or_default();
        com_init();
        let wide_id = wide(&aumid);
        let pid = unsafe {
            let mgr: IApplicationActivationManager =
                CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_LOCAL_SERVER)
                    .map_err(|e| CoreError::Launch(format!("packaged-app activation is unavailable: {e}")))?;
            mgr.ActivateApplication(PCWSTR(wide_id.as_ptr()), PCWSTR::null(), AO_NONE)
                .map_err(|e| CoreError::Launch(format!("Windows refused to activate {aumid}: {e}")))?
        };
        Ok(LaunchResult {
            outcome: LaunchOutcomeKind::Accepted,
            pid: Some(pid),
            verified_running: None,
            message: format!("Windows accepted the activation request (pid {pid}). This does not prove the app stayed open."),
            command_preview: preview,
        })
    }
}

fn shell_execute(file: &str, params: &str, dir: Option<&str>, preview: String) -> Result<LaunchResult> {
    let file_w = wide(file);
    let params_w = wide(params);
    let dir_w = dir.filter(|d| !d.is_empty()).map(wide);
    let verb = w!("open");
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_FLAG_NO_UI,
        lpVerb: verb,
        lpFile: PCWSTR(file_w.as_ptr()),
        lpParameters: PCWSTR(params_w.as_ptr()),
        lpDirectory: dir_w.as_ref().map(|d| PCWSTR(d.as_ptr())).unwrap_or(PCWSTR::null()),
        nShow: SW_SHOWNORMAL.0 as i32,
        ..Default::default()
    };
    unsafe {
        ShellExecuteExW(&mut info).map_err(|e| CoreError::Launch(format!("Windows refused to start '{file}': {e}")))?;
    }
    let handle = info.hProcess;
    let (pid, verified) = inspect_process(handle);
    if !handle.is_invalid() {
        unsafe {
            let _ = CloseHandle(handle);
        }
    }
    let message = match verified {
        Some(true) => format!("Windows accepted the launch and the process was still running (pid {}).", pid.unwrap_or(0)),
        Some(false) => "Windows accepted the launch. The process exited quickly; PulseLauncher does not treat that as a failed request.".into(),
        None => "Windows accepted the launch request. No process handle was returned, so PulseLauncher cannot confirm it stayed running.".into(),
    };
    Ok(LaunchResult {
        outcome: LaunchOutcomeKind::Accepted,
        pid,
        verified_running: verified,
        message,
        command_preview: preview,
    })
}

fn inspect_process(handle: HANDLE) -> (Option<u32>, Option<bool>) {
    if handle.is_invalid() {
        return (None, None);
    }
    unsafe {
        let _ = WaitForSingleObject(handle, 150);
        let mut code = 0u32;
        if GetExitCodeProcess(handle, &mut code).is_err() {
            return (None, None);
        }
        let pid = ::windows::Win32::System::Threading::GetProcessId(handle);
        let pid = if pid == 0 { None } else { Some(pid) };
        (pid, Some(code == STILL_ACTIVE.0 as u32))
    }
}

fn version_info(path: &str) -> Option<(Option<String>, Option<String>, Option<String>)> {
    let w = wide(path);
    let size = unsafe { GetFileVersionInfoSizeW(PCWSTR(w.as_ptr()), None) };
    if size == 0 {
        return None;
    }
    let mut buf = vec![0u8; size as usize];
    let ok = unsafe { GetFileVersionInfoW(PCWSTR(w.as_ptr()), None, size, buf.as_mut_ptr() as *mut _).is_ok() };
    if !ok {
        return None;
    }
    let mut trans: *mut std::ffi::c_void = std::ptr::null_mut();
    let mut trans_len = 0u32;
    let query = wide("\\VarFileInfo\\Translation");
    let ok = unsafe { VerQueryValueW(buf.as_ptr() as *const _, PCWSTR(query.as_ptr()), &mut trans, &mut trans_len).as_bool() };
    let (lang, cp) = if ok && trans_len >= 4 && !trans.is_null() {
        let bytes = unsafe { std::slice::from_raw_parts(trans as *const u8, 4) };
        (u16::from_le_bytes([bytes[0], bytes[1]]), u16::from_le_bytes([bytes[2], bytes[3]]))
    } else {
        (0x0409, 0x04b0)
    };
    let ver = query_string(&buf, lang, cp, "FileVersion").or_else(|| query_string(&buf, lang, cp, "ProductVersion"));
    let company = query_string(&buf, lang, cp, "CompanyName");
    let desc = query_string(&buf, lang, cp, "FileDescription");
    Some((ver, company, desc))
}

fn query_string(buf: &[u8], lang: u16, cp: u16, key: &str) -> Option<String> {
    let path = format!("\\StringFileInfo\\{lang:04x}{cp:04x}\\{key}");
    let w = wide(&path);
    let mut val: *mut std::ffi::c_void = std::ptr::null_mut();
    let mut len = 0u32;
    let ok = unsafe { VerQueryValueW(buf.as_ptr() as *const _, PCWSTR(w.as_ptr()), &mut val, &mut len).as_bool() };
    if !ok || val.is_null() || len == 0 {
        return None;
    }
    let s = from_wide_ptr(val as *const u16);
    let s = s.trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn extract_icon_png(path: &str, size: i32) -> Option<Vec<u8>> {
    let w = wide(path);
    let mut icon = HICON::default();
    let n = unsafe { ExtractIconExW(PCWSTR(w.as_ptr()), 0, Some(&mut icon), None, 1) };
    if n == 0 || icon.is_invalid() {
        return None;
    }
    let png = hicon_to_png(icon, size);
    unsafe {
        let _ = DestroyIcon(icon);
    }
    png
}

fn hicon_to_png(icon: HICON, size: i32) -> Option<Vec<u8>> {
    unsafe {
        let screen = GetDC(None);
        if screen.is_invalid() {
            return None;
        }
        let header = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size,
            biHeight: -size,
            biPlanes: 1,
            biBitCount: 32,
            ..Default::default()
        };
        let info = BITMAPINFO { bmiHeader: header, ..Default::default() };
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let dib: HBITMAP = CreateDIBSection(Some(screen), &info, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
        let mem = CreateCompatibleDC(Some(screen));
        let old = SelectObject(mem, dib.into());
        let drawn = DrawIconEx(mem, 0, 0, icon, size, size, 0, None, DI_NORMAL);
        let pixels = if drawn.is_ok() && !bits.is_null() {
            let count = (size as usize) * (size as usize) * 4;
            let raw = std::slice::from_raw_parts(bits as *const u8, count);
            let mut rgba = Vec::with_capacity(count);
            for px in raw.chunks_exact(4) {
                rgba.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
            }
            Some(rgba)
        } else {
            None
        };
        SelectObject(mem, old);
        let _ = DeleteDC(mem);
        let _ = DeleteObject(dib.into());
        ReleaseDC(None, screen);
        let rgba = pixels?;
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, size as u32, size as u32);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut writer = enc.write_header().ok()?;
            writer.write_image_data(&rgba).ok()?;
        }
        Some(out)
    }
}
