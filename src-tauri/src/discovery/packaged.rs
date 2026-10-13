//! Packaged (MSIX/UWP) apps via the documented `Get-StartApps` cmdlet.
//! Only entries whose AppID is a packaged AUMID (`Family!App`) are returned;
//! classic apps are already covered by Start Menu shortcuts.

use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::models::*;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Deserialize)]
struct StartApp {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "AppID")]
    app_id: String,
}

pub fn list_packaged_apps() -> Result<Vec<DiscoveredApp>, String> {
    let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
    let ps = format!("{system_root}\\System32\\WindowsPowerShell\\v1.0\\powershell.exe");
    let mut child = Command::new(ps)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "[Console]::OutputEncoding=[Text.Encoding]::UTF8; Get-StartApps | Select-Object Name,AppID | ConvertTo-Json -Compress",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("could not query Start apps: {e}"))?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() > Duration::from_secs(20) => {
                let _ = child.kill();
                return Err("timed out listing packaged apps".into());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(e.to_string()),
        }
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&out.stdout);
    parse_start_apps(text.trim_start_matches('\u{feff}'))
}

pub fn parse_start_apps(json: &str) -> Result<Vec<DiscoveredApp>, String> {
    if json.trim().is_empty() {
        return Ok(vec![]);
    }
    let list: Vec<StartApp> = if json.trim_start().starts_with('[') {
        serde_json::from_str(json).map_err(|e| e.to_string())?
    } else {
        vec![serde_json::from_str(json).map_err(|e| e.to_string())?]
    };
    Ok(list
        .into_iter()
        .filter(|a| a.app_id.contains('!') && !a.app_id.contains('\\') && !a.name.trim().is_empty())
        .map(|a| DiscoveredApp {
            category: super::categorize(&a.name, None, None),
            name: a.name,
            source: DiscoveredSource::Packaged,
            target: ExecutableTarget {
                kind: AppKind::Packaged,
                launch_target: a.app_id,
                resolved_target: None,
                arguments: vec![],
                link_arguments: None,
                working_dir: None,
            },
            icon_path: None,
            icon_index: 0,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_cmdlet_output() {
        let json = r#"[{"Name":"Calculator","AppID":"Microsoft.WindowsCalculator_8wekyb3d8bbwe!App"},
            {"Name":"Notepad++","AppID":"{6D809377-6AF0-444B-8957-A3773F02200E}\\Notepad++\\notepad++.exe"}]"#;
        let apps = super::parse_start_apps(json).unwrap();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "Calculator");
        let single = r#"{"Name":"Photos","AppID":"Microsoft.Windows.Photos_8wekyb3d8bbwe!App"}"#;
        assert_eq!(super::parse_start_apps(single).unwrap().len(), 1);
    }
}
