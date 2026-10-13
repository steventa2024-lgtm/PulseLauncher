//! Pure-Rust parser for the Windows Shell Link (`.lnk`) binary format ([MS-SHLLINK]).
//!
//! Parsing only reads bytes; it never resolves or executes the link target. Links that carry
//! only a shell item ID list (no filesystem path), e.g. many packaged-app shortcuts, yield
//! `target: None` rather than a guessed path.

use crate::security::{expand_env, split_windows_args};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShellLink {
    pub target: Option<String>,
    pub relative_path: Option<String>,
    pub args: Vec<String>,
    pub raw_args: Option<String>,
    pub working_dir: Option<String>,
    pub icon_location: Option<String>,
    pub icon_index: i32,
    pub description: Option<String>,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum LnkError {
    #[error("file is too small to be a shortcut")]
    TooShort,
    #[error("not a shell link (bad header)")]
    BadHeader,
    #[error("shortcut is truncated or corrupt")]
    Corrupt,
}

const HAS_ID_LIST: u32 = 1 << 0;
const HAS_LINK_INFO: u32 = 1 << 1;
const HAS_NAME: u32 = 1 << 2;
const HAS_REL_PATH: u32 = 1 << 3;
const HAS_WORKING_DIR: u32 = 1 << 4;
const HAS_ARGS: u32 = 1 << 5;
const HAS_ICON: u32 = 1 << 6;
const IS_UNICODE: u32 = 1 << 7;
const MAX_LNK_BYTES: usize = 1024 * 1024;

struct Reader<'a> {
    d: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn u16(&mut self) -> Result<u16, LnkError> {
        let b = self.d.get(self.pos..self.pos + 2).ok_or(LnkError::Corrupt)?;
        self.pos += 2;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Result<u32, LnkError> {
        let b = self.d.get(self.pos..self.pos + 4).ok_or(LnkError::Corrupt)?;
        self.pos += 4;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn skip(&mut self, n: usize) -> Result<(), LnkError> {
        if self.pos + n > self.d.len() {
            return Err(LnkError::Corrupt);
        }
        self.pos += n;
        Ok(())
    }
}

fn cstr_ansi(d: &[u8], off: usize) -> Option<String> {
    let s = d.get(off..)?;
    let end = s.iter().position(|&b| b == 0)?;
    // ANSI code page is not knowable here; ASCII is exact, other bytes are decoded lossily.
    Some(String::from_utf8_lossy(&s[..end]).into_owned())
}

fn cstr_utf16(d: &[u8], off: usize) -> Option<String> {
    let s = d.get(off..)?;
    let mut units = Vec::new();
    for ch in s.chunks_exact(2) {
        let u = u16::from_le_bytes([ch[0], ch[1]]);
        if u == 0 {
            return Some(String::from_utf16_lossy(&units));
        }
        units.push(u);
    }
    None
}

fn read_string_data(r: &mut Reader, unicode: bool) -> Result<String, LnkError> {
    let count = r.u16()? as usize;
    if unicode {
        let bytes = r.d.get(r.pos..r.pos + count * 2).ok_or(LnkError::Corrupt)?;
        r.pos += count * 2;
        let units: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        Ok(String::from_utf16_lossy(&units))
    } else {
        let bytes = r.d.get(r.pos..r.pos + count).ok_or(LnkError::Corrupt)?;
        r.pos += count;
        Ok(String::from_utf8_lossy(bytes).into_owned())
    }
}

pub fn parse(data: &[u8]) -> Result<ShellLink, LnkError> {
    if data.len() < 0x4C {
        return Err(LnkError::TooShort);
    }
    if data.len() > MAX_LNK_BYTES {
        return Err(LnkError::Corrupt);
    }
    let mut r = Reader { d: data, pos: 0 };
    if r.u32()? != 0x4C {
        return Err(LnkError::BadHeader);
    }
    const CLSID: [u8; 16] = [
        0x01, 0x14, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
    ];
    if data[4..20] != CLSID {
        return Err(LnkError::BadHeader);
    }
    r.pos = 20;
    let flags = r.u32()?;
    r.pos = 0x38;
    let icon_index = r.u32()? as i32;
    r.pos = 0x4C;

    let unicode = flags & IS_UNICODE != 0;
    let mut link = ShellLink { icon_index, ..Default::default() };

    if flags & HAS_ID_LIST != 0 {
        let sz = r.u16()? as usize;
        r.skip(sz)?;
    }

    if flags & HAS_LINK_INFO != 0 {
        let start = r.pos;
        let info_size = r.u32()? as usize;
        let info = data.get(start..start + info_size).ok_or(LnkError::Corrupt)?;
        if info.len() < 0x1C {
            return Err(LnkError::Corrupt);
        }
        let hdr = u32::from_le_bytes(info[4..8].try_into().unwrap()) as usize;
        let li_flags = u32::from_le_bytes(info[8..12].try_into().unwrap());
        let base_off = u32::from_le_bytes(info[16..20].try_into().unwrap()) as usize;
        let suffix_off = u32::from_le_bytes(info[24..28].try_into().unwrap()) as usize;
        if li_flags & 1 != 0 {
            let mut base = None;
            let mut suffix = None;
            if hdr >= 0x24 && info.len() >= 0x24 {
                let ub = u32::from_le_bytes(info[28..32].try_into().unwrap()) as usize;
                let us = u32::from_le_bytes(info[32..36].try_into().unwrap()) as usize;
                if ub != 0 {
                    base = cstr_utf16(info, ub);
                }
                if us != 0 {
                    suffix = cstr_utf16(info, us);
                }
            }
            if base.is_none() {
                base = cstr_ansi(info, base_off);
            }
            if suffix.is_none() && suffix_off != 0 {
                suffix = cstr_ansi(info, suffix_off);
            }
            if let Some(mut b) = base {
                if let Some(s) = suffix.filter(|s| !s.is_empty()) {
                    if !b.ends_with('\\') {
                        b.push('\\');
                    }
                    b.push_str(&s);
                }
                if !b.is_empty() {
                    link.target = Some(b);
                }
            }
        }
        r.pos = start + info_size;
    }

    if flags & HAS_NAME != 0 {
        link.description = Some(read_string_data(&mut r, unicode)?).filter(|s| !s.is_empty());
    }
    if flags & HAS_REL_PATH != 0 {
        link.relative_path = Some(read_string_data(&mut r, unicode)?).filter(|s| !s.is_empty());
    }
    if flags & HAS_WORKING_DIR != 0 {
        link.working_dir = Some(read_string_data(&mut r, unicode)?).filter(|s| !s.is_empty());
    }
    if flags & HAS_ARGS != 0 {
        let raw = read_string_data(&mut r, unicode)?;
        if !raw.is_empty() {
            link.args = split_windows_args(&raw);
            link.raw_args = Some(raw);
        }
    }
    if flags & HAS_ICON != 0 {
        link.icon_location = Some(read_string_data(&mut r, unicode)?).filter(|s| !s.is_empty());
    }

    // ExtraData: look for the environment-variable block when no base path was found.
    while r.pos + 8 <= data.len() {
        let size = r.u32()? as usize;
        if size < 4 {
            break;
        }
        let sig = r.u32()?;
        let body = size.saturating_sub(8);
        if sig == 0xA000_0001 && link.target.is_none() && body >= 780 {
            let blk = data.get(r.pos..r.pos + 780).ok_or(LnkError::Corrupt)?;
            if let Some(t) = cstr_utf16(&blk[260..], 0).filter(|s| !s.is_empty()) {
                link.target = Some(t);
            } else if let Some(t) = cstr_ansi(&blk[..260], 0).filter(|s| !s.is_empty()) {
                link.target = Some(t);
            }
        }
        if r.skip(body).is_err() {
            break;
        }
    }

    for f in [&mut link.target, &mut link.working_dir, &mut link.icon_location] {
        if let Some(v) = f.as_mut() {
            *v = expand_env(v);
        }
    }
    Ok(link)
}

/// Resolve the effective target, falling back to the relative path next to the link.
pub fn resolve_target(link: &ShellLink, lnk_path: &std::path::Path) -> Option<String> {
    if let Some(t) = &link.target {
        return Some(t.clone());
    }
    let rel = link.relative_path.as_ref()?;
    let dir = lnk_path.parent()?;
    Some(crate::security::normalize_path(&dir.join(rel.replace('\\', "/")).to_string_lossy()))
}

/// Fixture writer used by tests and the test-fixture generator. Produces a valid, minimal link.
#[doc(hidden)]
pub mod fixtures {
    pub struct LnkSpec<'a> {
        pub target: &'a str,
        pub args: &'a str,
        pub working_dir: &'a str,
        pub description: &'a str,
        pub unicode: bool,
        pub with_link_info: bool,
    }

    impl<'a> Default for LnkSpec<'a> {
        fn default() -> Self {
            Self { target: "", args: "", working_dir: "", description: "", unicode: true, with_link_info: true }
        }
    }

    fn string_data(out: &mut Vec<u8>, s: &str, unicode: bool) {
        if unicode {
            let u: Vec<u16> = s.encode_utf16().collect();
            out.extend((u.len() as u16).to_le_bytes());
            for c in u {
                out.extend(c.to_le_bytes());
            }
        } else {
            out.extend((s.len() as u16).to_le_bytes());
            out.extend(s.as_bytes());
        }
    }

    pub fn build_lnk(spec: &LnkSpec) -> Vec<u8> {
        let mut flags: u32 = 0;
        if spec.with_link_info {
            flags |= 1 << 1;
        }
        if !spec.description.is_empty() {
            flags |= 1 << 2;
        }
        if !spec.working_dir.is_empty() {
            flags |= 1 << 4;
        }
        if !spec.args.is_empty() {
            flags |= 1 << 5;
        }
        if spec.unicode {
            flags |= 1 << 7;
        }
        let mut v = Vec::new();
        v.extend(0x4Cu32.to_le_bytes());
        v.extend([0x01, 0x14, 0x02, 0, 0, 0, 0, 0, 0xC0, 0, 0, 0, 0, 0, 0, 0x46]);
        v.extend(flags.to_le_bytes());
        v.extend([0u8; 0x4C - 24]);
        if spec.with_link_info {
            let base = spec.target.as_bytes();
            let hdr = 0x1Cusize;
            let vol_id_size = 0x11usize;
            let vol_off = hdr;
            let base_off = vol_off + vol_id_size;
            let suffix_off = base_off + base.len() + 1;
            let total = suffix_off + 1;
            let mut li = Vec::new();
            li.extend((total as u32).to_le_bytes());
            li.extend((hdr as u32).to_le_bytes());
            li.extend(1u32.to_le_bytes());
            li.extend((vol_off as u32).to_le_bytes());
            li.extend((base_off as u32).to_le_bytes());
            li.extend(0u32.to_le_bytes());
            li.extend((suffix_off as u32).to_le_bytes());
            li.extend((vol_id_size as u32).to_le_bytes());
            li.extend([0u8; 0x0D]);
            li.extend(base);
            li.push(0);
            li.push(0);
            v.extend(li);
        }
        if !spec.description.is_empty() {
            string_data(&mut v, spec.description, spec.unicode);
        }
        if !spec.working_dir.is_empty() {
            string_data(&mut v, spec.working_dir, spec.unicode);
        }
        if !spec.args.is_empty() {
            string_data(&mut v, spec.args, spec.unicode);
        }
        v.extend(0u32.to_le_bytes());
        v
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;

    #[test]
    fn parses_basic_link() {
        let b = build_lnk(&LnkSpec {
            target: "C:\\Program Files\\App\\app.exe",
            args: "--profile \"My Profile\"",
            working_dir: "C:\\Program Files\\App",
            description: "My app",
            ..Default::default()
        });
        let l = parse(&b).unwrap();
        assert_eq!(l.target.as_deref(), Some("C:\\Program Files\\App\\app.exe"));
        assert_eq!(l.args, vec!["--profile", "My Profile"]);
        assert_eq!(l.working_dir.as_deref(), Some("C:\\Program Files\\App"));
        assert_eq!(l.description.as_deref(), Some("My app"));
    }

    #[test]
    fn parses_ansi_strings() {
        let b = build_lnk(&LnkSpec { target: "C:\\a\\b.exe", args: "-x", unicode: false, ..Default::default() });
        let l = parse(&b).unwrap();
        assert_eq!(l.target.as_deref(), Some("C:\\a\\b.exe"));
        assert_eq!(l.args, vec!["-x"]);
    }

    #[test]
    fn unicode_working_dir_and_description() {
        let b = build_lnk(&LnkSpec {
            target: "C:\\x\\y.exe",
            working_dir: "C:\\Üser\\日本語",
            description: "Ünïcödé",
            ..Default::default()
        });
        let l = parse(&b).unwrap();
        assert_eq!(l.working_dir.as_deref(), Some("C:\\Üser\\日本語"));
        assert_eq!(l.description.as_deref(), Some("Ünïcödé"));
    }

    #[test]
    fn no_link_info_means_no_target() {
        let b = build_lnk(&LnkSpec { with_link_info: false, description: "id-list only", ..Default::default() });
        let l = parse(&b).unwrap();
        assert!(l.target.is_none());
    }

    #[test]
    fn rejects_garbage_and_truncation() {
        assert_eq!(parse(&[]), Err(LnkError::TooShort));
        assert_eq!(parse(&[0u8; 200]), Err(LnkError::BadHeader));
        let b = build_lnk(&LnkSpec { target: "C:\\a.exe", args: "x y z", ..Default::default() });
        for cut in [0x4C, 0x60, b.len() - 6] {
            // Must never panic; either parses what is there or reports an error.
            let _ = parse(&b[..cut]);
        }
        let mut huge = b.clone();
        huge.resize(2 * 1024 * 1024, 0);
        assert!(parse(&huge).is_err());
    }

    #[test]
    fn fuzz_never_panics() {
        let base = build_lnk(&LnkSpec { target: "C:\\a.exe", args: "x", working_dir: "C:\\w", description: "d", ..Default::default() });
        let mut seed = 0x1234_5678u32;
        for _ in 0..2000 {
            let mut b = base.clone();
            for _ in 0..4 {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let i = (seed as usize >> 8) % b.len();
                b[i] = (seed >> 24) as u8;
            }
            let _ = parse(&b);
        }
    }

    #[test]
    fn env_vars_in_target_are_expanded() {
        std::env::set_var("PULSE_LNK_ROOT", "C:\\Tools");
        let b = build_lnk(&LnkSpec { target: "%PULSE_LNK_ROOT%\\t.exe", ..Default::default() });
        assert_eq!(parse(&b).unwrap().target.as_deref(), Some("C:\\Tools\\t.exe"));
    }
}
