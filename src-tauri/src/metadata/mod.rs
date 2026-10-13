//! File metadata and icon extraction by *reading* PE resources with the
//! pure-Rust `pelite` parser. Executables are memory-mapped read-only and are
//! never loaded or run.

use std::path::Path;

use pelite::resources::Name;
use pelite::{FileMap, PeFile};

use crate::models::AppMetadata;

const MAX_PE_SIZE: u64 = 1024 * 1024 * 1024;
const MAX_ICON_BYTES: usize = 2 * 1024 * 1024;

fn clean(s: Option<String>) -> Option<String> {
    s.map(|v| {
        v.trim_matches(|c: char| c.is_whitespace() || c == '\0')
            .to_string()
    })
    .filter(|v| !v.is_empty() && v.len() <= 512)
}

pub fn is_pe_candidate(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            matches!(
                e.to_ascii_lowercase().as_str(),
                "exe" | "dll" | "com" | "cpl" | "ocx"
            )
        })
        .unwrap_or(false)
}

pub fn read_metadata(path: &Path) -> AppMetadata {
    let mut meta = AppMetadata::default();
    let Ok(fs_meta) = std::fs::metadata(path) else {
        return meta;
    };
    if fs_meta.is_file() {
        meta.file_size = Some(fs_meta.len());
    }
    meta.modified_at = fs_meta.modified().ok().map(|t| {
        chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
    });
    if !fs_meta.is_file() || !is_pe_candidate(path) || fs_meta.len() > MAX_PE_SIZE {
        return meta;
    }
    let Ok(map) = FileMap::open(path) else {
        return meta;
    };
    let Ok(pe) = PeFile::from_bytes(&map) else {
        return meta;
    };
    let Ok(res) = pe.resources() else {
        return meta;
    };
    let Ok(vi) = res.version_info() else {
        return meta;
    };
    let lang = vi.translation().first().copied();
    if let Some(lang) = lang {
        meta.publisher = clean(vi.value(lang, "CompanyName"));
        meta.description = clean(vi.value(lang, "FileDescription"));
        meta.product_name = clean(vi.value(lang, "ProductName"));
        meta.version = clean(vi.value(lang, "ProductVersion"))
            .or_else(|| clean(vi.value(lang, "FileVersion")));
    }
    if meta.version.is_none() {
        if let Some(f) = vi.fixed() {
            meta.version = Some(f.dwProductVersion.to_string());
        }
    }
    meta
}

/// Extract an icon group as `.ico` bytes. `index >= 0` selects the n-th
/// group; a negative index selects the resource id `-index` (the same
/// convention Windows uses for icon locations).
pub fn extract_icon(path: &Path, index: i32) -> Option<Vec<u8>> {
    let lower = path.to_string_lossy().to_lowercase();
    if lower.ends_with(".ico") {
        let meta = std::fs::metadata(path).ok()?;
        if meta.len() as usize > MAX_ICON_BYTES {
            return None;
        }
        return std::fs::read(path).ok();
    }
    if !is_pe_candidate(path) {
        return None;
    }
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_PE_SIZE {
        return None;
    }
    let map = FileMap::open(path).ok()?;
    let pe = PeFile::from_bytes(&map).ok()?;
    let res = pe.resources().ok()?;
    let groups: Vec<_> = res.icons().filter_map(Result::ok).collect();
    let group = if index < 0 {
        let id = index.unsigned_abs();
        groups
            .iter()
            .find(|(n, _)| matches!(n, Name::Id(i) if *i == id))
            .or_else(|| groups.first())
    } else {
        groups.get(index as usize).or_else(|| groups.first())
    }?;
    let mut out = Vec::new();
    group.1.write(&mut out).ok()?;
    (out.len() <= MAX_ICON_BYTES && !out.is_empty()).then_some(out)
}

/// Split a Windows icon location such as `C:\x\app.exe,-101`.
pub fn split_icon_location(loc: &str, default_index: i32) -> (String, i32) {
    if let Some((p, i)) = loc.rsplit_once(',') {
        if let Ok(n) = i.trim().parse::<i32>() {
            return (p.trim().trim_matches('"').to_string(), n);
        }
    }
    (loc.trim().trim_matches('"').to_string(), default_index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_pe_files_degrade_gracefully() {
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("fake.exe");
        std::fs::write(&fake, b"MZ this is not a real program").unwrap();
        let m = read_metadata(&fake);
        assert_eq!(m.file_size, Some(29));
        assert!(m.version.is_none() && m.publisher.is_none());
        assert!(extract_icon(&fake, 0).is_none());
        assert_eq!(
            read_metadata(&dir.path().join("missing.exe")),
            AppMetadata::default()
        );
    }

    #[test]
    fn icon_location_split() {
        assert_eq!(
            split_icon_location("C:\\a b\\x.exe,-101", 0),
            ("C:\\a b\\x.exe".into(), -101)
        );
        assert_eq!(
            split_icon_location("\"C:\\x.ico\"", 2),
            ("C:\\x.ico".into(), 2)
        );
    }

    #[cfg(windows)]
    #[test]
    fn reads_real_windows_binary_metadata_and_icon() {
        let sys = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
        let notepad = std::path::PathBuf::from(format!("{sys}\\System32\\notepad.exe"));
        if !notepad.exists() {
            eprintln!("notepad.exe not present; skipping");
            return;
        }
        let m = read_metadata(&notepad);
        assert!(m.publisher.unwrap_or_default().contains("Microsoft"));
        assert!(m.version.is_some());
        let ico = extract_icon(&notepad, 0).expect("notepad has an icon");
        assert_eq!(&ico[0..4], &[0, 0, 1, 0]);
    }

    /// Builds a real PE with version info and an icon using mingw-w64 when it
    /// is installed (Linux CI / dev). Skips otherwise.
    #[cfg(not(windows))]
    #[test]
    fn reads_mingw_built_pe_when_toolchain_available() {
        use std::process::Command;
        let have = |b: &str| Command::new(b).arg("--version").output().is_ok();
        if !have("x86_64-w64-mingw32-gcc") || !have("x86_64-w64-mingw32-windres") {
            eprintln!("mingw-w64 not installed; skipping PE fixture test");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        // 1x1 32-bit icon.
        let mut ico = vec![0u8, 0, 1, 0, 1, 0, 1, 1, 0, 0, 1, 0, 32, 0];
        let bmp_len: u32 = 40 + 4 + 4;
        ico.extend_from_slice(&bmp_len.to_le_bytes());
        ico.extend_from_slice(&22u32.to_le_bytes());
        let mut bmp = vec![0u8; 40];
        bmp[0..4].copy_from_slice(&40u32.to_le_bytes());
        bmp[4..8].copy_from_slice(&1i32.to_le_bytes());
        bmp[8..12].copy_from_slice(&2i32.to_le_bytes());
        bmp[12..14].copy_from_slice(&1u16.to_le_bytes());
        bmp[14..16].copy_from_slice(&32u16.to_le_bytes());
        ico.extend_from_slice(&bmp);
        ico.extend_from_slice(&[0x43, 0x88, 0xFF, 0xFF, 0, 0, 0, 0]);
        std::fs::write(d.join("app.ico"), &ico).unwrap();
        std::fs::write(
            d.join("app.rc"),
            "1 ICON \"app.ico\"\n1 VERSIONINFO\nFILEVERSION 1,2,3,4\nPRODUCTVERSION 1,2,3,4\nBEGIN\n BLOCK \"StringFileInfo\"\n BEGIN\n  BLOCK \"040904B0\"\n  BEGIN\n   VALUE \"CompanyName\", \"ZeroPulse Test\"\n   VALUE \"FileDescription\", \"Fixture\"\n   VALUE \"ProductVersion\", \"1.2.3\"\n  END\n END\n BLOCK \"VarFileInfo\"\n BEGIN\n  VALUE \"Translation\", 0x409, 1200\n END\nEND\n",
        )
        .unwrap();
        std::fs::write(d.join("main.c"), "int main(void){return 0;}\n").unwrap();
        let ok = Command::new("x86_64-w64-mingw32-windres")
            .current_dir(d)
            .args(["app.rc", "-O", "coff", "-o", "app.res"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
            && Command::new("x86_64-w64-mingw32-gcc")
                .current_dir(d)
                .args(["main.c", "app.res", "-o", "Fixture App.exe"])
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
        assert!(ok, "mingw build failed");
        let exe = d.join("Fixture App.exe");
        let m = read_metadata(&exe);
        assert_eq!(m.publisher.as_deref(), Some("ZeroPulse Test"));
        assert_eq!(m.version.as_deref(), Some("1.2.3"));
        assert_eq!(m.description.as_deref(), Some("Fixture"));
        let icon = extract_icon(&exe, 0).expect("icon");
        assert_eq!(&icon[0..4], &[0, 0, 1, 0]);
    }
}
