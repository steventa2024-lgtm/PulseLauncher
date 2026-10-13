//! Start Menu shortcut discovery. Reads `.lnk` bytes only; never launches or resolves via the shell.

use crate::lnk;
use crate::models::*;
use crate::security::{normalize_path, target_identity};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const MAX_DEPTH: usize = 8;
const MAX_FILES: usize = 20_000;

#[derive(Debug, Default)]
pub struct ScanOutput {
    pub apps: Vec<DiscoveredApp>,
    pub issues: Vec<ScanIssue>,
    pub skipped_uninstallers: usize,
    pub skipped_no_target: usize,
    pub duplicates_merged: usize,
    pub scanned_roots: Vec<String>,
}

pub fn is_uninstaller(name: &str, exe: &str) -> bool {
    let n = name.to_lowercase();
    let file = Path::new(&exe.replace('\\', "/"))
        .file_name()
        .map(|f| f.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    n.starts_with("uninstall")
        || n.contains(" uninstall")
        || n.starts_with("remove ")
        || file.starts_with("unins")
        || file.contains("uninstall")
}

pub fn scan_start_menu(roots: &[(PathBuf, AppSource)]) -> ScanOutput {
    let mut out = ScanOutput::default();
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut files_seen = 0usize;
    for (root, source) in roots {
        out.scanned_roots.push(root.to_string_lossy().into_owned());
        if !root.exists() {
            out.issues.push(ScanIssue { path: root.to_string_lossy().into_owned(), reason: "folder does not exist".into() });
            continue;
        }
        walk(root, *source, 0, &mut files_seen, &mut out, &mut seen);
    }
    out
}

fn walk(
    dir: &Path,
    source: AppSource,
    depth: usize,
    files_seen: &mut usize,
    out: &mut ScanOutput,
    seen: &mut HashMap<String, usize>,
) {
    if depth > MAX_DEPTH || *files_seen > MAX_FILES {
        return;
    }
    let rd = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(e) => {
            out.issues.push(ScanIssue { path: dir.to_string_lossy().into_owned(), reason: format!("cannot read folder: {e}") });
            return;
        }
    };
    let mut entries: Vec<_> = rd.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let path = e.path();
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            walk(&path, source, depth + 1, files_seen, out, seen);
        } else if path.extension().map(|x| x.eq_ignore_ascii_case("lnk")).unwrap_or(false) {
            *files_seen += 1;
            handle_lnk(&path, source, out, seen);
        }
    }
}

fn handle_lnk(path: &Path, source: AppSource, out: &mut ScanOutput, seen: &mut HashMap<String, usize>) {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            out.issues.push(ScanIssue { path: path.to_string_lossy().into_owned(), reason: format!("cannot read: {e}") });
            return;
        }
    };
    let link = match lnk::parse(&bytes) {
        Ok(l) => l,
        Err(e) => {
            out.issues.push(ScanIssue { path: path.to_string_lossy().into_owned(), reason: format!("invalid shortcut: {e}") });
            return;
        }
    };
    let Some(resolved) = lnk::resolve_target(&link, path) else {
        out.skipped_no_target += 1;
        return;
    };
    let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    if name.is_empty() {
        return;
    }
    if is_uninstaller(&name, &resolved) {
        out.skipped_uninstallers += 1;
        return;
    }
    let lower = resolved.to_lowercase();
    if !(lower.ends_with(".exe") || lower.ends_with(".com") || lower.ends_with(".bat") || lower.ends_with(".cmd")) {
        // Documents, folders, URLs and help links are not applications.
        out.skipped_no_target += 1;
        return;
    }
    // Identity is based on the resolved executable + shortcut arguments so that the same app
    // reachable from user and all-users Start Menus is merged.
    let id_probe = ExecutableTarget {
        kind: TargetKind::Executable,
        path: normalize_path(&resolved),
        args: link.args.clone(),
        working_dir: None,
        aumid: None,
        uri: None,
    };
    let key = target_identity(&id_probe);
    if seen.contains_key(&key) {
        out.duplicates_merged += 1;
        return;
    }
    seen.insert(key.clone(), out.apps.len());
    out.apps.push(DiscoveredApp {
        name,
        source,
        target: ExecutableTarget {
            kind: TargetKind::Shortcut,
            path: path.to_string_lossy().into_owned(),
            args: vec![],
            working_dir: None,
            aumid: None,
            uri: None,
        },
        resolved_path: Some(normalize_path(&resolved)),
        identity_key: key,
        provenance: format!(
            "{} (target: {}{})",
            path.to_string_lossy(),
            resolved,
            if link.args.is_empty() { String::new() } else { format!(" {}", crate::security::join_windows_args(&link.args)) }
        ),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lnk::fixtures::*;

    fn write_lnk(dir: &Path, name: &str, spec: &LnkSpec) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join(name), build_lnk(spec)).unwrap();
    }

    #[test]
    fn scans_nested_unicode_dedupes_and_filters() {
        let t = tempfile::tempdir().unwrap();
        let user = t.path().join("user");
        let common = t.path().join("common");
        write_lnk(&user, "Editor.lnk", &LnkSpec { target: "C:\\Tools\\editor.exe", ..Default::default() });
        write_lnk(&user.join("Dev Tools"), "Ünïcode 日本.lnk", &LnkSpec { target: "C:\\Tools\\u.exe", args: "--x \"a b\"", ..Default::default() });
        write_lnk(&common, "Editor (shared).lnk", &LnkSpec { target: "C:\\Tools\\editor.exe", ..Default::default() });
        write_lnk(&common, "Uninstall Editor.lnk", &LnkSpec { target: "C:\\Tools\\unins000.exe", ..Default::default() });
        write_lnk(&common, "Readme.lnk", &LnkSpec { target: "C:\\Tools\\readme.txt", ..Default::default() });
        write_lnk(&common, "IdListOnly.lnk", &LnkSpec { with_link_info: false, ..Default::default() });
        fs::write(common.join("broken.lnk"), b"not a link at all, definitely not").unwrap();
        fs::write(common.join("ignore.txt"), b"x").unwrap();

        let out = scan_start_menu(&[
            (user.clone(), AppSource::StartMenuUser),
            (common.clone(), AppSource::StartMenuCommon),
            (t.path().join("missing"), AppSource::StartMenuCommon),
        ]);
        let mut names: Vec<_> = out.apps.iter().map(|a| a.name.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["Editor", "Ünïcode 日本"], "{names:?}");
        let editor = out.apps.iter().find(|a| a.name == "Editor").unwrap();
        assert_eq!(editor.source, AppSource::StartMenuUser, "user Start Menu wins over all-users duplicate");
        assert_eq!(out.duplicates_merged, 1);
        assert_eq!(out.skipped_uninstallers, 1);
        assert_eq!(out.skipped_no_target, 2);
        assert_eq!(out.issues.len(), 2, "{:?}", out.issues);
        assert!(out.apps.iter().any(|a| a.provenance.contains("--x \"a b\"")));
    }

    #[test]
    fn uninstaller_heuristics() {
        assert!(is_uninstaller("Uninstall Foo", "C:\\x\\a.exe"));
        assert!(is_uninstaller("Foo", "C:\\x\\unins000.exe"));
        assert!(!is_uninstaller("Unity Hub", "C:\\x\\Unity Hub.exe"));
    }
}
