//! Create a unique text note. Existing files are never overwritten.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult, ErrorCode};
use crate::security;

pub fn write_note(folder: &str, prefix: &str, content: &str) -> AppResult<PathBuf> {
    let dir = security::validate_absolute_path(folder, "Note folder")?;
    if !dir.is_dir() {
        return Err(AppError::new(
            ErrorCode::TargetMissing,
            format!("Folder not found: {}", dir.display()),
        ));
    }
    if content.len() > 100_000 {
        return Err(AppError::invalid(
            "Note text is limited to 100000 characters",
        ));
    }
    let prefix = {
        let raw = prefix.trim();
        security::sanitize_file_name(if raw.is_empty() { "note" } else { raw })
    };
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    for n in 1..=1000 {
        let name = if n == 1 {
            format!("{prefix}-{stamp}.txt")
        } else {
            format!("{prefix}-{stamp}-{n}.txt")
        };
        let path = dir.join(&name);
        security::ensure_within(&dir, &path)?;
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                file.write_all(content.as_bytes())?;
                return Ok(path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(AppError::new(
        ErrorCode::Conflict,
        "Could not find a free file name for the note",
    ))
}

pub fn path_exists(path: &str) -> AppResult<bool> {
    let p = security::validate_absolute_path(path, "Path")?;
    Ok(Path::new(&p).exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_unique_notes_and_refuses_overwrite_and_escape() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().to_str().unwrap();
        let a = write_note(folder, "a/b", "hello").unwrap();
        assert!(a.exists());
        assert!(a.file_name().unwrap().to_string_lossy().starts_with("a_b-"));
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "hello");
        // A second note in the same second gets a numeric suffix rather than
        // replacing the first one.
        let b = write_note(folder, "a/b", "second").unwrap();
        assert_ne!(a, b);
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "hello");
        assert!(write_note("relative", "n", "").is_err());
        let outside = dir.path().join("..").join("escape");
        assert!(write_note(outside.to_str().unwrap(), "n", "").is_err());
    }
}
