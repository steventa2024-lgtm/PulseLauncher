//! Opt-in file/folder index. Only folders the user explicitly lists are
//! walked, with depth, entry-count and exclusion limits.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

const MAX_DEPTH: usize = 6;

pub struct IndexOutcome {
    pub entries: Vec<(String, String, bool, String)>,
    pub errors: Vec<String>,
    pub truncated: bool,
}

pub fn build_index(
    roots: &[String],
    exclusions: &[String],
    max_entries: usize,
    cancel: &AtomicBool,
) -> IndexOutcome {
    let excl: Vec<String> = exclusions
        .iter()
        .map(|e| e.trim().to_lowercase())
        .filter(|e| !e.is_empty())
        .collect();
    let mut out = IndexOutcome {
        entries: vec![],
        errors: vec![],
        truncated: false,
    };
    let max_entries = max_entries.clamp(100, 200_000);
    for root in roots {
        let rp = Path::new(root);
        if crate::security::validate_absolute_path(root, "Index root").is_err() || !rp.is_dir() {
            out.errors
                .push(format!("Skipped {root}: not an existing absolute folder"));
            continue;
        }
        let walker = walkdir::WalkDir::new(rp)
            .max_depth(MAX_DEPTH)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy().to_lowercase();
                e.depth() == 0
                    || !(excl.contains(&name) || name.starts_with('.') && e.file_type().is_dir())
            });
        for entry in walker {
            if cancel.load(Ordering::Relaxed) {
                out.errors.push("Indexing cancelled".into());
                return out;
            }
            match entry {
                Ok(e) if e.depth() > 0 => {
                    if out.entries.len() >= max_entries {
                        out.truncated = true;
                        break;
                    }
                    out.entries.push((
                        e.path().to_string_lossy().into_owned(),
                        e.file_name().to_string_lossy().into_owned(),
                        e.file_type().is_dir(),
                        root.clone(),
                    ));
                }
                Ok(_) => {}
                Err(err) => {
                    if out.errors.len() < 50 {
                        out.errors.push(err.to_string());
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_only_roots_with_exclusions_and_limits() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Projects");
        std::fs::create_dir_all(root.join("app").join("node_modules").join("deep")).unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join("app").join("readme.md"), b"x").unwrap();
        std::fs::write(root.join("app").join("node_modules").join("pkg.js"), b"x").unwrap();
        let cancel = AtomicBool::new(false);
        let r = build_index(
            &[root.to_string_lossy().into(), "relative".into()],
            &["node_modules".into()],
            1000,
            &cancel,
        );
        let names: Vec<_> = r.entries.iter().map(|e| e.1.as_str()).collect();
        assert!(names.contains(&"app") && names.contains(&"readme.md"));
        assert!(!names.contains(&"pkg.js") && !names.contains(&".git"));
        assert_eq!(r.errors.len(), 1);

        for i in 0..150 {
            std::fs::write(root.join(format!("f{i}.txt")), b"").unwrap();
        }
        let r = build_index(&[root.to_string_lossy().into()], &[], 100, &cancel);
        assert!(r.truncated && r.entries.len() == 100);

        cancel.store(true, Ordering::Relaxed);
        let r = build_index(&[root.to_string_lossy().into()], &[], 1000, &cancel);
        assert!(r.entries.is_empty());
    }
}
