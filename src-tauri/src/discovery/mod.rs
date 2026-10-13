//! App discovery. Adapters only *read* shortcut files and system listings;
//! nothing found here is executed.

use std::path::Path;

use crate::models::*;

pub mod lnk;
#[cfg(windows)]
mod packaged;

const MAX_DEPTH: usize = 8;
const MAX_LINKS_PER_ROOT: usize = 5000;

pub struct DiscoveryResult {
    pub apps: Vec<DiscoveredApp>,
    pub scanned_sources: Vec<DiscoveredSource>,
    pub roots: Vec<String>,
    pub errors: Vec<String>,
}

pub struct ScanOptions {
    pub include_desktop: bool,
    pub include_packaged: bool,
}

pub fn scan(opts: &ScanOptions) -> DiscoveryResult {
    let mut result = DiscoveryResult {
        apps: vec![],
        scanned_sources: vec![],
        roots: vec![],
        errors: vec![],
    };
    let roots = crate::platform::start_menu_roots(opts.include_desktop);
    if roots.is_empty() {
        result.errors.push(
            "Start Menu discovery is only available on Windows. Use Add App to register programs manually."
                .into(),
        );
    }
    for (source, root) in roots {
        result.roots.push(root.to_string_lossy().into_owned());
        match scan_shortcut_root(&root, source, source != DiscoveredSource::Desktop) {
            Ok(mut found) => {
                result.apps.append(&mut found);
                if !result.scanned_sources.contains(&source) {
                    result.scanned_sources.push(source);
                }
            }
            Err(e) => result.errors.push(format!("{}: {e}", root.display())),
        }
    }
    #[cfg(windows)]
    if opts.include_packaged {
        match packaged::list_packaged_apps() {
            Ok(mut apps) => {
                result.apps.append(&mut apps);
                result.scanned_sources.push(DiscoveredSource::Packaged);
            }
            Err(e) => result.errors.push(format!("Packaged apps: {e}")),
        }
    }
    #[cfg(not(windows))]
    let _ = opts.include_packaged;
    result
}

/// Walk a shortcut folder (Start Menu or Desktop) and parse every `.lnk`.
pub fn scan_shortcut_root(
    root: &Path,
    source: DiscoveredSource,
    recursive: bool,
) -> Result<Vec<DiscoveredApp>, String> {
    if !root.is_dir() {
        return Err("folder not found".into());
    }
    let mut out = Vec::new();
    let walker = walkdir::WalkDir::new(root)
        .max_depth(if recursive { MAX_DEPTH } else { 1 })
        .follow_links(false);
    for entry in walker.into_iter().filter_map(Result::ok) {
        if out.len() >= MAX_LINKS_PER_ROOT {
            break;
        }
        let path = entry.path();
        if !entry.file_type().is_file() || !has_ext(path, "lnk") {
            continue;
        }
        if let Some(app) = app_from_link(root, path, source) {
            out.push(app);
        }
    }
    Ok(out)
}

fn has_ext(p: &Path, ext: &str) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(ext))
}

fn is_noise(name: &str) -> bool {
    let n = name.to_lowercase();
    [
        "uninstall",
        "remove ",
        "readme",
        "release notes",
        "license",
        "help and support",
        "website",
    ]
    .iter()
    .any(|k| n.contains(k))
}

pub fn app_from_link(root: &Path, path: &Path, source: DiscoveredSource) -> Option<DiscoveredApp> {
    let name = path.file_stem()?.to_string_lossy().into_owned();
    if is_noise(&name) {
        return None;
    }
    let parsed = lnk::parse_file(path).ok()?;
    let resolved = parsed.resolved_target(path);
    if let Some(t) = &resolved {
        let ext = Path::new(t)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase());
        match ext.as_deref() {
            Some("exe" | "com" | "msc" | "cpl" | "bat" | "cmd") | None => {}
            // Documents, web pages and help files are not applications.
            Some(_) => return None,
        }
        if t.to_lowercase().contains("unins") {
            return None;
        }
    }
    let folder = path
        .parent()
        .and_then(|p| p.strip_prefix(root).ok())
        .map(|p| p.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty());
    let icon = parsed
        .icon_path()
        .filter(|p| !p.trim().is_empty())
        .or_else(|| resolved.clone());
    Some(DiscoveredApp {
        category: categorize(&name, resolved.as_deref(), folder.as_deref()),
        name,
        source,
        target: ExecutableTarget {
            kind: AppKind::Shortcut,
            launch_target: path.to_string_lossy().into_owned(),
            resolved_target: resolved,
            arguments: vec![],
            link_arguments: parsed.arguments.clone(),
            working_dir: parsed.working_dir.as_deref().map(lnk::expand_env),
        },
        icon_path: icon,
        icon_index: parsed.icon_index,
    })
}

/// Keyword-based default category. Users can override it per app.
pub fn categorize(name: &str, target: Option<&str>, folder: Option<&str>) -> Option<String> {
    let hay = format!(
        "{} {} {}",
        name.to_lowercase(),
        target.unwrap_or("").to_lowercase(),
        folder.unwrap_or("").to_lowercase()
    );
    let lname = name.to_lowercase();
    if [
        "pulsecut",
        "pulseflow",
        "openpulse",
        "pulseapply",
        "zeropulse",
        "pulselauncher",
    ]
    .iter()
    .any(|k| lname.replace([' ', '-', '_'], "").contains(k))
    {
        return Some("ZeroPulse".into());
    }
    const DEV: &[&str] = &[
        "visual studio",
        "vs code",
        "code.exe",
        "cursor",
        "terminal",
        "powershell",
        "git",
        "python",
        "node",
        "docker",
        "postman",
        "jetbrains",
        "intellij",
        "pycharm",
        "webstorm",
        "rider",
        "clion",
        "goland",
        "sublime",
        "notepad++",
        "wsl",
        "cmake",
        "android studio",
        "putty",
        "sql",
        "dbeaver",
        "wireshark",
        "insomnia",
        "github",
        "developer",
        "debugger",
        "windsurf",
    ];
    const PROD: &[&str] = &[
        "word",
        "excel",
        "powerpoint",
        "outlook",
        "onenote",
        "office",
        "notion",
        "obsidian",
        "slack",
        "teams",
        "zoom",
        "calendar",
        "mail",
        "to do",
        "todo",
        "notepad",
        "acrobat",
        "pdf",
        "libreoffice",
        "thunderbird",
        "evernote",
        "calculator",
        "snipping",
    ];
    if DEV.iter().any(|k| hay.contains(k)) {
        return Some("Developer Tools".into());
    }
    if PROD.iter().any(|k| hay.contains(k)) {
        return Some("Productivity".into());
    }
    None
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::lnk::fixture::{build, LinkSpec};
    use super::*;

    #[test]
    fn scans_nested_start_menu_and_filters_noise() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Programs");
        let nested = root.join("Pulse Tools ü");
        std::fs::create_dir_all(&nested).unwrap();
        let write = |p: PathBuf, target: &str| {
            std::fs::write(
                p,
                build(&LinkSpec {
                    local_base: Some(target),
                    unicode_base: true,
                    ..Default::default()
                }),
            )
            .unwrap()
        };
        write(nested.join("PulseCut.lnk"), "C:\\Tools\\PulseCut.exe");
        write(root.join("Visual Studio Code.lnk"), "C:\\VS\\Code.exe");
        write(
            nested.join("Uninstall PulseCut.lnk"),
            "C:\\Tools\\unins000.exe",
        );
        write(root.join("Manual.lnk"), "C:\\Tools\\manual.pdf");
        std::fs::write(root.join("broken.lnk"), b"garbage").unwrap();
        std::fs::write(root.join("notes.txt"), b"x").unwrap();

        let mut apps = scan_shortcut_root(&root, DiscoveredSource::StartMenuUser, true).unwrap();
        apps.sort_by(|a, b| a.name.cmp(&b.name));
        let names: Vec<_> = apps.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, vec!["PulseCut", "Visual Studio Code"]);
        assert_eq!(apps[0].category.as_deref(), Some("ZeroPulse"));
        assert_eq!(apps[1].category.as_deref(), Some("Developer Tools"));
        assert_eq!(apps[0].target.kind, AppKind::Shortcut);
        assert_eq!(
            apps[0].target.resolved_target.as_deref(),
            Some("C:\\Tools\\PulseCut.exe")
        );
    }

    #[test]
    fn missing_root_is_reported_not_panicking() {
        assert!(scan_shortcut_root(
            Path::new("/definitely/not/here"),
            DiscoveredSource::StartMenuUser,
            true
        )
        .is_err());
    }
}
