//! Opt-in file/folder name index. Only explicitly configured roots are walked; never the whole drive.

use crate::error::{CoreError, Result};
use std::path::Path;

pub const MAX_ENTRIES_PER_ROOT: usize = 50_000;
pub const MAX_DEPTH: usize = 6;

#[derive(Debug, Default, Clone)]
pub struct IndexReport {
    pub indexed: usize,
    pub truncated: bool,
    pub errors: usize,
}

/// Walk `root` collecting (path, name, is_dir). Honors name-based exclusions and does not follow symlinks.
pub fn index_root(root: &Path, exclusions: &[String]) -> Result<(Vec<(String, String, bool)>, IndexReport)> {
    if !root.is_dir() {
        return Err(CoreError::NotFound(format!("index root is not a folder: {}", root.display())));
    }
    let mut out = Vec::new();
    let mut rep = IndexReport::default();
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let rd = match std::fs::read_dir(&dir) {
            Ok(r) => r,
            Err(_) => {
                rep.errors += 1;
                continue;
            }
        };
        for e in rd.filter_map(|e| e.ok()) {
            let name = e.file_name().to_string_lossy().into_owned();
            if exclusions.iter().any(|x| x.eq_ignore_ascii_case(&name)) {
                continue;
            }
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            let path = e.path().to_string_lossy().into_owned();
            if out.len() >= MAX_ENTRIES_PER_ROOT {
                rep.truncated = true;
                break;
            }
            if ft.is_dir() {
                out.push((path, name, true));
                if depth + 1 < MAX_DEPTH {
                    stack.push((e.path(), depth + 1));
                }
            } else {
                out.push((path, name, false));
            }
        }
        if rep.truncated {
            break;
        }
    }
    rep.indexed = out.len();
    Ok((out, rep))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_only_root_and_honors_exclusions() {
        let t = tempfile::tempdir().unwrap();
        let r = t.path().join("proj");
        std::fs::create_dir_all(r.join("src")).unwrap();
        std::fs::create_dir_all(r.join("node_modules/x")).unwrap();
        std::fs::write(r.join("src/main.rs"), "").unwrap();
        std::fs::write(r.join("node_modules/x/skip.js"), "").unwrap();
        std::fs::write(t.path().join("outside.txt"), "").unwrap();
        let (files, rep) = index_root(&r, &["node_modules".into()]).unwrap();
        let names: Vec<_> = files.iter().map(|f| f.1.as_str()).collect();
        assert!(names.contains(&"main.rs") && names.contains(&"src"));
        assert!(!names.contains(&"skip.js") && !names.contains(&"outside.txt"));
        assert_eq!(rep.indexed, files.len());
        assert!(index_root(&t.path().join("nope"), &[]).is_err());
    }
}
