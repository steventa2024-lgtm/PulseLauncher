//! Minimal, defensive parser for Windows Shell Link (`.lnk`) files following
//! the public [MS-SHLLINK] specification. It only reads bytes; it never
//! resolves, opens or executes the target.

use std::path::Path;

const HEADER_SIZE: usize = 0x4C;
const LINK_CLSID: [u8; 16] = [
    0x01, 0x14, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
];
const MAX_LNK_SIZE: u64 = 1024 * 1024;

const HAS_ID_LIST: u32 = 1 << 0;
const HAS_LINK_INFO: u32 = 1 << 1;
const HAS_NAME: u32 = 1 << 2;
const HAS_RELATIVE_PATH: u32 = 1 << 3;
const HAS_WORKING_DIR: u32 = 1 << 4;
const HAS_ARGUMENTS: u32 = 1 << 5;
const HAS_ICON_LOCATION: u32 = 1 << 6;
const IS_UNICODE: u32 = 1 << 7;
const FORCE_NO_LINK_INFO: u32 = 1 << 8;
const HAS_DARWIN_ID: u32 = 1 << 12;

const SIG_ENVIRONMENT: u32 = 0xA000_0001;
const SIG_DARWIN: u32 = 0xA000_0006;
const SIG_ICON_ENVIRONMENT: u32 = 0xA000_0007;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedLink {
    pub local_path: Option<String>,
    pub network_path: Option<String>,
    pub description: Option<String>,
    pub relative_path: Option<String>,
    pub working_dir: Option<String>,
    pub arguments: Option<String>,
    pub icon_location: Option<String>,
    pub icon_index: i32,
    pub env_target: Option<String>,
    pub icon_env_target: Option<String>,
    pub advertised: bool,
    pub show_command: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LnkError {
    TooSmall,
    BadHeader,
    Truncated(&'static str),
    TooLarge,
    Io(String),
}

impl std::fmt::Display for LnkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LnkError::TooSmall => write!(f, "file is too small to be a shortcut"),
            LnkError::BadHeader => write!(f, "not a Windows shortcut (bad header)"),
            LnkError::Truncated(w) => write!(f, "shortcut is truncated ({w})"),
            LnkError::TooLarge => write!(f, "shortcut file is unexpectedly large"),
            LnkError::Io(e) => write!(f, "{e}"),
        }
    }
}

struct Reader<'a> {
    b: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn u16(&mut self, w: &'static str) -> Result<u16, LnkError> {
        let s = self
            .b
            .get(self.pos..self.pos + 2)
            .ok_or(LnkError::Truncated(w))?;
        self.pos += 2;
        Ok(u16::from_le_bytes([s[0], s[1]]))
    }
    fn u32(&mut self, w: &'static str) -> Result<u32, LnkError> {
        let s = self
            .b
            .get(self.pos..self.pos + 4)
            .ok_or(LnkError::Truncated(w))?;
        self.pos += 4;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn skip(&mut self, n: usize, w: &'static str) -> Result<(), LnkError> {
        if self.pos + n > self.b.len() {
            return Err(LnkError::Truncated(w));
        }
        self.pos += n;
        Ok(())
    }
}

fn u32_at(b: &[u8], off: usize) -> Option<u32> {
    b.get(off..off + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Decode a single-byte string. Windows-1252 is approximated as Latin-1,
/// which is exact for ASCII and good enough for display of legacy paths.
fn ansi_cstr(b: &[u8], off: usize) -> Option<String> {
    let s = b.get(off..)?;
    let end = s.iter().position(|&c| c == 0)?;
    Some(s[..end].iter().map(|&c| c as char).collect())
}

fn utf16_cstr(b: &[u8], off: usize) -> Option<String> {
    let s = b.get(off..)?;
    let mut units = Vec::new();
    let (chunks, _) = s.as_chunks::<2>();
    for ch in chunks {
        let u = u16::from_le_bytes(*ch);
        if u == 0 {
            return Some(String::from_utf16_lossy(&units));
        }
        units.push(u);
    }
    None
}

fn fixed_utf16(b: &[u8]) -> Option<String> {
    let (chunks, _) = b.as_chunks::<2>();
    let units: Vec<u16> = chunks
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .take_while(|&u| u != 0)
        .collect();
    (!units.is_empty()).then(|| String::from_utf16_lossy(&units))
}

fn fixed_ansi(b: &[u8]) -> Option<String> {
    let s: String = b
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as char)
        .collect();
    (!s.is_empty()).then_some(s)
}

fn parse_link_info(info: &[u8], out: &mut ParsedLink) {
    let header_size = u32_at(info, 4).unwrap_or(0) as usize;
    let flags = u32_at(info, 8).unwrap_or(0);
    let local_off = u32_at(info, 16).unwrap_or(0) as usize;
    let net_off = u32_at(info, 20).unwrap_or(0) as usize;
    let suffix_off = u32_at(info, 24).unwrap_or(0) as usize;
    let (local_u, suffix_u) = if header_size >= 0x24 {
        (
            u32_at(info, 28).unwrap_or(0) as usize,
            u32_at(info, 32).unwrap_or(0) as usize,
        )
    } else {
        (0, 0)
    };
    let suffix = if suffix_u != 0 {
        utf16_cstr(info, suffix_u)
    } else if suffix_off != 0 {
        ansi_cstr(info, suffix_off)
    } else {
        None
    }
    .unwrap_or_default();

    if flags & 1 != 0 {
        let base = if local_u != 0 {
            utf16_cstr(info, local_u)
        } else if local_off != 0 {
            ansi_cstr(info, local_off)
        } else {
            None
        };
        if let Some(base) = base {
            out.local_path = Some(format!("{base}{suffix}"));
        }
    }
    if flags & 2 != 0 && net_off != 0 {
        let net = &info[net_off.min(info.len())..];
        let net_name_off = u32_at(net, 8).unwrap_or(0) as usize;
        let net_name_u = if net_name_off > 0x14 {
            u32_at(net, 20).unwrap_or(0) as usize
        } else {
            0
        };
        let name = if net_name_u != 0 {
            utf16_cstr(net, net_name_u)
        } else {
            ansi_cstr(net, net_name_off)
        };
        if let Some(name) = name {
            out.network_path = Some(if suffix.is_empty() {
                name
            } else {
                format!("{}\\{}", name.trim_end_matches('\\'), suffix)
            });
        }
    }
}

pub fn parse_bytes(b: &[u8]) -> Result<ParsedLink, LnkError> {
    if b.len() < HEADER_SIZE {
        return Err(LnkError::TooSmall);
    }
    if u32_at(b, 0) != Some(HEADER_SIZE as u32) || b[4..20] != LINK_CLSID {
        return Err(LnkError::BadHeader);
    }
    let flags = u32_at(b, 20).unwrap_or(0);
    let mut out = ParsedLink {
        icon_index: u32_at(b, 56).unwrap_or(0) as i32,
        show_command: u32_at(b, 60).unwrap_or(1),
        advertised: flags & HAS_DARWIN_ID != 0,
        ..Default::default()
    };
    let mut r = Reader {
        b,
        pos: HEADER_SIZE,
    };
    if flags & HAS_ID_LIST != 0 {
        let n = r.u16("id list size")? as usize;
        r.skip(n, "id list")?;
    }
    if flags & HAS_LINK_INFO != 0 && flags & FORCE_NO_LINK_INFO == 0 {
        let start = r.pos;
        let size = r.u32("link info size")? as usize;
        if size < 0x1C || start + size > b.len() {
            return Err(LnkError::Truncated("link info"));
        }
        parse_link_info(&b[start..start + size], &mut out);
        r.pos = start + size;
    } else if flags & HAS_LINK_INFO != 0 {
        let start = r.pos;
        let size = r.u32("link info size")? as usize;
        r.pos = start;
        r.skip(size, "link info")?;
    }
    let unicode = flags & IS_UNICODE != 0;
    let read_string = |r: &mut Reader| -> Result<String, LnkError> {
        let count = r.u16("string length")? as usize;
        if unicode {
            let bytes =
                r.b.get(r.pos..r.pos + count * 2)
                    .ok_or(LnkError::Truncated("string"))?;
            r.pos += count * 2;
            let (chunks, _) = bytes.as_chunks::<2>();
            let units: Vec<u16> = chunks.iter().map(|c| u16::from_le_bytes(*c)).collect();
            Ok(String::from_utf16_lossy(&units))
        } else {
            let bytes =
                r.b.get(r.pos..r.pos + count)
                    .ok_or(LnkError::Truncated("string"))?;
            r.pos += count;
            Ok(bytes.iter().map(|&c| c as char).collect())
        }
    };
    let non_empty = |s: String| (!s.is_empty()).then_some(s);
    if flags & HAS_NAME != 0 {
        out.description = non_empty(read_string(&mut r)?);
    }
    if flags & HAS_RELATIVE_PATH != 0 {
        out.relative_path = non_empty(read_string(&mut r)?);
    }
    if flags & HAS_WORKING_DIR != 0 {
        out.working_dir = non_empty(read_string(&mut r)?);
    }
    if flags & HAS_ARGUMENTS != 0 {
        out.arguments = non_empty(read_string(&mut r)?);
    }
    if flags & HAS_ICON_LOCATION != 0 {
        out.icon_location = non_empty(read_string(&mut r)?);
    }
    // Extra data blocks (best effort; malformed blocks end parsing).
    for _ in 0..64 {
        let Ok(size) = r.u32("extra size") else { break };
        if size < 4 {
            break;
        }
        let start = r.pos - 4;
        let Some(block) = b.get(start..start + size as usize) else {
            break;
        };
        let sig = u32_at(block, 4).unwrap_or(0);
        if (sig == SIG_ENVIRONMENT || sig == SIG_ICON_ENVIRONMENT) && block.len() >= 0x314 {
            let target = fixed_utf16(&block[268..788]).or_else(|| fixed_ansi(&block[8..268]));
            if sig == SIG_ENVIRONMENT {
                out.env_target = target;
            } else {
                out.icon_env_target = target;
            }
        } else if sig == SIG_DARWIN {
            out.advertised = true;
        }
        r.pos = start + size as usize;
    }
    Ok(out)
}

pub fn parse_file(path: &Path) -> Result<ParsedLink, LnkError> {
    let meta = std::fs::metadata(path).map_err(|e| LnkError::Io(e.to_string()))?;
    if meta.len() > MAX_LNK_SIZE {
        return Err(LnkError::TooLarge);
    }
    let bytes = std::fs::read(path).map_err(|e| LnkError::Io(e.to_string()))?;
    parse_bytes(&bytes)
}

/// Expand `%VAR%` references using the current environment. Unknown
/// variables are left untouched so the caller can detect them.
pub fn expand_env(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) if end > 0 => {
                let name = &after[..end];
                match std::env::var(name) {
                    Ok(v) => out.push_str(&v),
                    Err(_) => {
                        out.push('%');
                        out.push_str(name);
                        out.push('%');
                    }
                }
                rest = &after[end + 1..];
            }
            _ => {
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

impl ParsedLink {
    /// Best guess at the executable a shortcut points to.
    pub fn resolved_target(&self, link_path: &Path) -> Option<String> {
        if let Some(p) = &self.local_path {
            return Some(expand_env(p));
        }
        if let Some(p) = &self.network_path {
            return Some(p.clone());
        }
        if let Some(p) = &self.env_target {
            return Some(expand_env(p));
        }
        if let Some(rel) = &self.relative_path {
            let dir = link_path.parent()?;
            let joined = dir.join(rel.replace('\\', std::path::MAIN_SEPARATOR_STR));
            return Some(joined.to_string_lossy().into_owned());
        }
        None
    }

    pub fn icon_path(&self) -> Option<String> {
        self.icon_env_target
            .as_deref()
            .or(self.icon_location.as_deref())
            .map(expand_env)
    }
}

#[cfg(test)]
pub mod fixture {
    //! Builds spec-conformant `.lnk` bytes for tests.
    use super::*;

    #[derive(Default)]
    pub struct LinkSpec<'a> {
        pub local_base: Option<&'a str>,
        pub unicode_base: bool,
        pub description: Option<&'a str>,
        pub relative: Option<&'a str>,
        pub working_dir: Option<&'a str>,
        pub arguments: Option<&'a str>,
        pub icon: Option<&'a str>,
        pub icon_index: i32,
        pub env_target: Option<&'a str>,
        pub darwin: bool,
    }

    fn push_str(out: &mut Vec<u8>, s: &str) {
        let units: Vec<u16> = s.encode_utf16().collect();
        out.extend_from_slice(&(units.len() as u16).to_le_bytes());
        for u in units {
            out.extend_from_slice(&u.to_le_bytes());
        }
    }

    pub fn build(spec: &LinkSpec) -> Vec<u8> {
        let mut flags = IS_UNICODE;
        if spec.local_base.is_some() {
            flags |= HAS_LINK_INFO;
        }
        if spec.description.is_some() {
            flags |= HAS_NAME;
        }
        if spec.relative.is_some() {
            flags |= HAS_RELATIVE_PATH;
        }
        if spec.working_dir.is_some() {
            flags |= HAS_WORKING_DIR;
        }
        if spec.arguments.is_some() {
            flags |= HAS_ARGUMENTS;
        }
        if spec.icon.is_some() {
            flags |= HAS_ICON_LOCATION;
        }
        if spec.darwin {
            flags |= HAS_DARWIN_ID;
        }
        let mut out = Vec::new();
        out.extend_from_slice(&(HEADER_SIZE as u32).to_le_bytes());
        out.extend_from_slice(&LINK_CLSID);
        out.extend_from_slice(&flags.to_le_bytes());
        out.extend_from_slice(&0x20u32.to_le_bytes());
        out.extend_from_slice(&[0u8; 24]);
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&spec.icon_index.to_le_bytes());
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&[0u8; 12]);
        assert_eq!(out.len(), HEADER_SIZE);

        if let Some(base) = spec.local_base {
            let header_size: u32 = if spec.unicode_base { 0x24 } else { 0x1C };
            let mut info = vec![0u8; header_size as usize];
            let volume_off = info.len();
            info.extend_from_slice(&0x10u32.to_le_bytes());
            info.extend_from_slice(&3u32.to_le_bytes());
            info.extend_from_slice(&0u32.to_le_bytes());
            info.extend_from_slice(&0x10u32.to_le_bytes());
            let ansi_off = info.len();
            if spec.unicode_base {
                info.extend_from_slice(b"?\0");
            } else {
                info.extend_from_slice(base.as_bytes());
                info.push(0);
            }
            let suffix_off = info.len();
            info.push(0);
            let (uni_off, uni_suffix_off) = if spec.unicode_base {
                let o = info.len();
                for u in base.encode_utf16() {
                    info.extend_from_slice(&u.to_le_bytes());
                }
                info.extend_from_slice(&[0, 0]);
                let so = info.len();
                info.extend_from_slice(&[0, 0]);
                (o, so)
            } else {
                (0, 0)
            };
            let total = info.len() as u32;
            info[0..4].copy_from_slice(&total.to_le_bytes());
            info[4..8].copy_from_slice(&header_size.to_le_bytes());
            info[8..12].copy_from_slice(&1u32.to_le_bytes());
            info[12..16].copy_from_slice(&(volume_off as u32).to_le_bytes());
            info[16..20].copy_from_slice(&(ansi_off as u32).to_le_bytes());
            info[20..24].copy_from_slice(&0u32.to_le_bytes());
            info[24..28].copy_from_slice(&(suffix_off as u32).to_le_bytes());
            if spec.unicode_base {
                info[28..32].copy_from_slice(&(uni_off as u32).to_le_bytes());
                info[32..36].copy_from_slice(&(uni_suffix_off as u32).to_le_bytes());
            }
            out.extend_from_slice(&info);
        }
        for s in [
            spec.description,
            spec.relative,
            spec.working_dir,
            spec.arguments,
            spec.icon,
        ]
        .into_iter()
        .flatten()
        {
            push_str(&mut out, s);
        }
        if let Some(t) = spec.env_target {
            let mut block = vec![0u8; 0x314];
            block[0..4].copy_from_slice(&0x314u32.to_le_bytes());
            block[4..8].copy_from_slice(&SIG_ENVIRONMENT.to_le_bytes());
            let a = t.as_bytes();
            block[8..8 + a.len()].copy_from_slice(a);
            for (i, u) in t.encode_utf16().enumerate() {
                block[268 + i * 2..270 + i * 2].copy_from_slice(&u.to_le_bytes());
            }
            out.extend_from_slice(&block);
        }
        out.extend_from_slice(&0u32.to_le_bytes());
        out
    }
}

#[cfg(test)]
mod tests {
    use super::fixture::*;
    use super::*;

    #[test]
    fn parses_full_unicode_link() {
        let bytes = build(&LinkSpec {
            local_base: Some("C:\\Program Files\\Pulse Tools\\PulseCüt.exe"),
            unicode_base: true,
            description: Some("Video editor"),
            working_dir: Some("C:\\Program Files\\Pulse Tools"),
            arguments: Some("--project \"C:\\My Videos\\a b\""),
            icon: Some("%SystemRoot%\\system32\\shell32.dll"),
            icon_index: 3,
            ..Default::default()
        });
        let p = parse_bytes(&bytes).unwrap();
        assert_eq!(
            p.local_path.as_deref(),
            Some("C:\\Program Files\\Pulse Tools\\PulseCüt.exe")
        );
        assert_eq!(p.description.as_deref(), Some("Video editor"));
        assert_eq!(
            p.working_dir.as_deref(),
            Some("C:\\Program Files\\Pulse Tools")
        );
        assert_eq!(
            p.arguments.as_deref(),
            Some("--project \"C:\\My Videos\\a b\"")
        );
        assert_eq!(p.icon_index, 3);
        assert!(!p.advertised);
    }

    #[test]
    fn parses_ansi_base_and_env_block() {
        let bytes = build(&LinkSpec {
            local_base: Some("C:\\Windows\\notepad.exe"),
            env_target: Some("%windir%\\notepad.exe"),
            ..Default::default()
        });
        let p = parse_bytes(&bytes).unwrap();
        assert_eq!(p.local_path.as_deref(), Some("C:\\Windows\\notepad.exe"));
        assert_eq!(p.env_target.as_deref(), Some("%windir%\\notepad.exe"));
    }

    #[test]
    fn env_target_used_without_link_info_and_darwin_detected() {
        let bytes = build(&LinkSpec {
            env_target: Some("%PL_TEST_ROOT%\\app.exe"),
            darwin: true,
            ..Default::default()
        });
        let p = parse_bytes(&bytes).unwrap();
        assert!(p.advertised);
        std::env::set_var("PL_TEST_ROOT", "D:\\Tools");
        assert_eq!(
            p.resolved_target(Path::new("C:\\x\\a.lnk")).as_deref(),
            Some("D:\\Tools\\app.exe")
        );
    }

    #[test]
    fn relative_path_fallback() {
        let bytes = build(&LinkSpec {
            relative: Some(".\\bin\\tool.exe"),
            ..Default::default()
        });
        let p = parse_bytes(&bytes).unwrap();
        let t = p.resolved_target(Path::new("/tmp/links/tool.lnk")).unwrap();
        assert!(t.contains("bin") && t.ends_with("tool.exe"));
    }

    #[test]
    fn rejects_garbage_and_truncation() {
        assert_eq!(parse_bytes(b"hello"), Err(LnkError::TooSmall));
        assert_eq!(parse_bytes(&[0u8; 100]), Err(LnkError::BadHeader));
        let bytes = build(&LinkSpec {
            local_base: Some("C:\\a.exe"),
            arguments: Some("--long-argument"),
            ..Default::default()
        });
        for cut in [HEADER_SIZE + 2, bytes.len() - 10] {
            assert!(parse_bytes(&bytes[..cut]).is_err(), "cut at {cut}");
        }
    }

    #[test]
    fn fuzz_like_random_inputs_never_panic() {
        let base = build(&LinkSpec {
            local_base: Some("C:\\a.exe"),
            arguments: Some("x"),
            env_target: Some("C:\\b.exe"),
            ..Default::default()
        });
        let mut seed: u32 = 12345;
        for _ in 0..2000 {
            let mut b = base.clone();
            for _ in 0..8 {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
                let i = (seed as usize) % b.len();
                b[i] = (seed >> 16) as u8;
            }
            let _ = parse_bytes(&b);
        }
    }

    #[test]
    fn expands_known_and_keeps_unknown_vars() {
        std::env::set_var("PL_X", "C:\\X");
        assert_eq!(expand_env("%PL_X%\\a"), "C:\\X\\a");
        assert_eq!(expand_env("%PL_NOPE_123%\\a"), "%PL_NOPE_123%\\a");
        assert_eq!(expand_env("100% sure"), "100% sure");
    }
}
