use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use base64::Engine;
use walkdir::WalkDir;

use crate::error::{AppError, AppResult};
use crate::models::{now_iso, IndexedFile, PreviewAsset};
use crate::security::validate_index_root;

pub fn save_preview(data_dir: &Path, app_id: &str, source: &Path) -> AppResult<PreviewAsset> {
    if !source.is_file() {
        return Err(AppError::new("PREVIEW", "Choose an image file to import."));
    }
    let ext = source
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif") {
        return Err(AppError::new(
            "PREVIEW",
            "Preview images must be PNG, JPG, WEBP, or GIF.",
        ));
    }
    let meta = fs::metadata(source)?;
    if meta.len() > 8 * 1024 * 1024 {
        return Err(AppError::new(
            "PREVIEW",
            "Preview images must be 8 MB or smaller.",
        ));
    }
    let bytes = fs::read(source)?;
    if !looks_like_image(&bytes, &ext) {
        return Err(AppError::new(
            "PREVIEW",
            "File contents do not match an image.",
        ));
    }
    let dir = data_dir.join("previews").join(sanitize_id(app_id));
    fs::create_dir_all(&dir)?;
    let id = uuid::Uuid::new_v4().to_string();
    let path = dir.join(format!("{id}.{ext}"));
    fs::write(&path, bytes)?;
    Ok(PreviewAsset {
        id,
        app_id: app_id.to_string(),
        path: path.to_string_lossy().to_string(),
        source: "user-import".into(),
        created_at: now_iso(),
    })
}

pub fn delete_preview_file(path: &Path) -> AppResult<()> {
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn icon_data_url(path: &Path) -> AppResult<Option<String>> {
    if !path.is_file() {
        return Ok(None);
    }
    let meta = fs::metadata(path)?;
    if meta.len() > 512 * 1024 {
        return Ok(None);
    }
    let bytes = fs::read(path)?;
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mime = match ext.as_str() {
        "png" if bytes.starts_with(b"\x89PNG") => "image/png",
        "jpg" | "jpeg" if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) => "image/jpeg",
        "gif" if bytes.starts_with(b"GIF8") => "image/gif",
        "svg"
            if bytes
                .windows(4)
                .any(|window| window == b"<svg" || window == b"<SVG") =>
        {
            "image/svg+xml"
        }
        "webp" if bytes.len() > 12 && &bytes[8..12] == b"WEBP" => "image/webp",
        _ => return Ok(None),
    };
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Ok(Some(format!("data:{mime};base64,{encoded}")))
}

pub fn index_roots(
    roots: &[String],
    exclusions: &[String],
    extensions: &[String],
    cancel: &AtomicBool,
) -> AppResult<Vec<IndexedFile>> {
    let mut files = Vec::new();
    let extensions: Vec<String> = extensions
        .iter()
        .map(|ext| ext.trim().trim_start_matches('.').to_ascii_lowercase())
        .filter(|ext| !ext.is_empty() && ext.chars().all(|ch| ch.is_ascii_alphanumeric()))
        .collect();
    for root in roots {
        if cancel.load(Ordering::Relaxed) {
            return Err(AppError::new("CANCELLED", "Indexing was cancelled."));
        }
        let root = validate_index_root(Path::new(root))?;
        let exclusions_owned = exclusions.to_vec();
        let root_for_filter = root.clone();
        for entry in WalkDir::new(&root)
            .max_depth(6)
            .follow_links(false)
            .into_iter()
            .filter_entry(move |entry| {
                if entry.path() == root_for_filter {
                    return true;
                }
                entry
                    .file_name()
                    .to_str()
                    .is_none_or(|name| !exclusions_owned.iter().any(|excluded| excluded == name))
            })
        {
            if cancel.load(Ordering::Relaxed) {
                return Err(AppError::new("CANCELLED", "Indexing was cancelled."));
            }
            if files.len() >= 5000 {
                return Ok(files);
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            if entry.file_type().is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    if exclusions.iter().any(|excluded| excluded == name) && entry.path() != root {
                        continue;
                    }
                }
                continue;
            }
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            if path.components().any(|component| {
                component
                    .as_os_str()
                    .to_str()
                    .is_some_and(|name| exclusions.iter().any(|excluded| excluded == name))
            }) {
                continue;
            }
            let ext = path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if !extensions.iter().any(|allowed| allowed == &ext) {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("")
                .to_string();
            let parent = path
                .parent()
                .map(|parent| parent.to_string_lossy().to_string())
                .unwrap_or_default();
            let size_bytes = entry.metadata().ok().map(|meta| meta.len());
            files.push(IndexedFile {
                path: path.to_string_lossy().to_string(),
                name,
                parent,
                size_bytes,
            });
        }
    }
    Ok(files)
}

fn looks_like_image(bytes: &[u8], ext: &str) -> bool {
    match ext {
        "png" => bytes.starts_with(b"\x89PNG"),
        "jpg" | "jpeg" => bytes.starts_with(&[0xFF, 0xD8, 0xFF]),
        "gif" => bytes.starts_with(b"GIF8"),
        "webp" => bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP",
        _ => false,
    }
}

fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

pub fn read_preview_data_url(data_dir: &Path, asset_path: &Path) -> AppResult<String> {
    let root = data_dir.join("previews");
    let root = root
        .canonicalize()
        .map_err(|_| AppError::new("PREVIEW", "No preview directory exists."))?;
    let path = asset_path
        .canonicalize()
        .map_err(|_| AppError::new("PREVIEW", "Preview file is missing."))?;
    if !path.starts_with(&root) {
        return Err(AppError::new(
            "PREVIEW",
            "Preview path is outside app data.",
        ));
    }
    let meta = fs::metadata(&path)?;
    if !meta.is_file() || meta.len() > 8 * 1024 * 1024 {
        return Err(AppError::new(
            "PREVIEW",
            "Preview file is not a usable image.",
        ));
    }
    let bytes = fs::read(&path)?;
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mime = match ext.as_str() {
        "png" if looks_like_image(&bytes, "png") => "image/png",
        "jpg" | "jpeg" if looks_like_image(&bytes, "jpg") => "image/jpeg",
        "gif" if looks_like_image(&bytes, "gif") => "image/gif",
        "webp" if looks_like_image(&bytes, "webp") => "image/webp",
        _ => {
            return Err(AppError::new(
                "PREVIEW",
                "Stored preview is not a supported image.",
            ))
        }
    };
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Ok(format!("data:{mime};base64,{encoded}"))
}

pub fn file_size(path: &Path) -> Option<u64> {
    fs::metadata(path)
        .ok()
        .filter(|meta| meta.is_file())
        .map(|meta| meta.len())
}

pub fn path_missing(path: &str) -> bool {
    let path = PathBuf::from(path);
    !(path.is_file() || path.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_import_copies_png_and_rejects_text() {
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("shot.png");
        fs::write(&png, b"\x89PNG\r\n\x1a\nfake").unwrap();
        let saved = save_preview(dir.path(), "app 1", &png).unwrap();
        assert!(Path::new(&saved.path).is_file());
        let text = dir.path().join("notes.txt");
        fs::write(&text, b"hello").unwrap();
        assert!(save_preview(dir.path(), "app", &text).is_err());
        let url = read_preview_data_url(dir.path(), Path::new(&saved.path)).unwrap();
        assert!(url.starts_with("data:image/png;base64,"));
        delete_preview_file(Path::new(&saved.path)).unwrap();
        assert!(!Path::new(&saved.path).exists());
    }

    #[test]
    fn refuses_to_index_the_home_directory() {
        let cancel = AtomicBool::new(false);
        if let Some(home) = crate::security::home_dir() {
            let error = index_roots(
                &[home.to_string_lossy().to_string()],
                &[],
                &["txt".into()],
                &cancel,
            );
            assert!(error.is_err());
        }
    }
}
