//! Validation, argument handling and redaction. Nothing here executes anything.

use crate::error::{CoreError, Result};
use crate::models::{ExecutableTarget, TargetKind};
use once_cell::sync::Lazy;
use regex::Regex;
use std::path::{Path, PathBuf};
use url::Url;

pub const MAX_NAME_LEN: usize = 120;
pub const MAX_ARG_LEN: usize = 8_192;
pub const MAX_ARGS: usize = 64;
pub const MAX_URL_LEN: usize = 2_048;
pub const MAX_IMPORT_BYTES: usize = 4 * 1024 * 1024;

/// Schemes that may be opened via the OS. Everything else (`file:`, `javascript:`, `data:`, `vbscript:` ...) is rejected.
const WEB_SCHEMES: &[&str] = &["http", "https"];
/// Documented Windows settings URI scheme, allowed only for explicit settings actions.
const SETTINGS_SCHEMES: &[&str] = &["ms-settings"];

pub fn validate_web_url(input: &str) -> Result<Url> {
    let input = input.trim();
    if input.is_empty() || input.len() > MAX_URL_LEN {
        return Err(CoreError::Security("URL is empty or too long".into()));
    }
    if input.chars().any(|c| c.is_control()) {
        return Err(CoreError::Security("URL contains control characters".into()));
    }
    let url = Url::parse(input).map_err(|e| CoreError::Security(format!("malformed URL: {e}")))?;
    if !WEB_SCHEMES.contains(&url.scheme()) {
        return Err(CoreError::Security(format!(
            "scheme '{}' is not allowed; only http and https can be opened",
            url.scheme()
        )));
    }
    if url.host_str().map(str::is_empty).unwrap_or(true) {
        return Err(CoreError::Security("URL has no host".into()));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(CoreError::Security(
            "URLs with embedded credentials are not allowed".into(),
        ));
    }
    Ok(url)
}

/// HTTP(S) URL that must point at this machine. Used for optional local integrations.
pub fn validate_loopback_url(input: &str) -> Result<Url> {
    let url = validate_web_url(input)?;
    let host = url.host_str().unwrap_or("");
    if !matches!(host, "127.0.0.1" | "localhost" | "::1") {
        return Err(CoreError::Security(
            "only a loopback address on this computer is allowed".into(),
        ));
    }
    Ok(url)
}

pub fn validate_settings_uri(input: &str) -> Result<Url> {
    let url = Url::parse(input.trim()).map_err(|e| CoreError::Security(format!("malformed URI: {e}")))?;
    if !SETTINGS_SCHEMES.contains(&url.scheme()) {
        return Err(CoreError::Security(format!(
            "scheme '{}' is not an approved settings URI",
            url.scheme()
        )));
    }
    Ok(url)
}

pub fn validate_display_name(name: &str) -> Result<String> {
    let n = name.trim();
    if n.is_empty() {
        return Err(CoreError::invalid("name must not be empty"));
    }
    if n.chars().count() > MAX_NAME_LEN {
        return Err(CoreError::invalid(format!("name exceeds {MAX_NAME_LEN} characters")));
    }
    if n.chars().any(|c| c.is_control()) {
        return Err(CoreError::invalid("name contains control characters"));
    }
    Ok(n.to_string())
}

pub fn validate_args(args: &[String]) -> Result<()> {
    if args.len() > MAX_ARGS {
        return Err(CoreError::invalid(format!("more than {MAX_ARGS} arguments")));
    }
    for a in args {
        if a.len() > MAX_ARG_LEN {
            return Err(CoreError::invalid("an argument is too long"));
        }
        if a.contains('\0') {
            return Err(CoreError::invalid("arguments must not contain NUL"));
        }
    }
    Ok(())
}

/// Lexically normalise a path (no filesystem access): collapses `.`/`..`, unifies separators on Windows.
pub fn normalize_path(p: &str) -> String {
    let p = p.trim().trim_matches('"');
    let unified: String = if cfg!(windows) { p.replace('/', "\\") } else { p.to_string() };
    let sep = if cfg!(windows) { '\\' } else { '/' };
    let (prefix, rest) = split_prefix(&unified);
    let mut out: Vec<&str> = Vec::new();
    for comp in rest.split(sep) {
        match comp {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    let mut s = String::from(prefix);
    if !prefix.is_empty() || unified.starts_with(sep) {
        if !prefix.ends_with(sep) {
            s.push(sep);
        }
    }
    s.push_str(&out.join(&sep.to_string()));
    s
}

fn split_prefix(p: &str) -> (&str, &str) {
    let b = p.as_bytes();
    if cfg!(windows) {
        if p.starts_with("\\\\?\\") {
            return ("\\\\?\\", &p[4..]);
        }
        if p.starts_with("\\\\") {
            return ("\\\\", &p[2..]);
        }
        if b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic() {
            return (&p[..2], &p[2..]);
        }
    }
    ("", p)
}

/// Case-normalised key for identity comparison (Windows paths are case-insensitive).
pub fn path_identity(p: &str) -> String {
    let n = normalize_path(p);
    if cfg!(windows) {
        n.to_lowercase()
    } else {
        n
    }
}

pub fn target_identity(t: &ExecutableTarget) -> String {
    match t.kind {
        TargetKind::Packaged => format!("pkg:{}", t.aumid.clone().unwrap_or_default().to_lowercase()),
        TargetKind::Uri => format!("uri:{}", t.uri.clone().unwrap_or_default()),
        TargetKind::Executable | TargetKind::Shortcut => {
            let mut k = format!("exe:{}", path_identity(&t.path));
            if !t.args.is_empty() {
                k.push('|');
                k.push_str(&join_windows_args(&t.args));
            }
            k
        }
    }
}

/// Validate a user-supplied launch target. Requires an absolute, existing file for exe/lnk.
pub fn validate_target(t: &ExecutableTarget) -> Result<()> {
    validate_target_opts(t, true)
}

/// `check_exists = false` validates shape only (used for imported registrations that may point at
/// files on another machine; they then show a missing-target status).
pub fn validate_target_opts(t: &ExecutableTarget, check_exists: bool) -> Result<()> {
    validate_args(&t.args)?;
    match t.kind {
        TargetKind::Executable | TargetKind::Shortcut => {
            if t.path.contains('\0') || t.path.trim().is_empty() {
                return Err(CoreError::invalid("path is empty"));
            }
            let p = Path::new(&t.path);
            if !is_absolute_any(&t.path) {
                return Err(CoreError::invalid("path must be absolute"));
            }
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase())
                .unwrap_or_default();
            let allowed: &[&str] = if t.kind == TargetKind::Shortcut {
                &["lnk"]
            } else if cfg!(windows) {
                &["exe", "com", "bat", "cmd"]
            } else {
                &["exe", "com", "bat", "cmd", "sh", ""]
            };
            if !allowed.contains(&ext.as_str()) {
                return Err(CoreError::invalid(format!(
                    "unsupported file type '.{ext}' (expected {})",
                    if t.kind == TargetKind::Shortcut { ".lnk" } else { ".exe" }
                )));
            }
            if check_exists && !fs_is_file(&t.path) {
                return Err(CoreError::NotFound(format!("file not found: {}", t.path)));
            }
        }
        TargetKind::Packaged => {
            let a = t.aumid.as_deref().unwrap_or("");
            if a.is_empty() || !AUMID_RE.is_match(a) {
                return Err(CoreError::invalid("invalid AppUserModelID"));
            }
        }
        TargetKind::Uri => {
            let u = t.uri.as_deref().unwrap_or("");
            if validate_web_url(u).is_err() && validate_settings_uri(u).is_err() {
                return Err(CoreError::Security("URI scheme is not approved".into()));
            }
        }
    }
    if let Some(wd) = &t.working_dir {
        if !wd.is_empty() {
            if !is_absolute_any(wd) {
                return Err(CoreError::invalid("working directory must be absolute"));
            }
            if check_exists && !Path::new(wd).is_dir() {
                return Err(CoreError::NotFound(format!("working directory not found: {wd}")));
            }
        }
    }
    Ok(())
}

static AUMID_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[A-Za-z0-9._\-]+(![A-Za-z0-9._\-]+)?$").unwrap());

/// True if something exists at `p`. Uses link metadata as a fallback so Windows app-execution
/// aliases (e.g. `wt.exe` in WindowsApps, which are reparse points) are detected.
pub fn fs_exists(p: &str) -> bool {
    let path = Path::new(p);
    path.exists() || (cfg!(windows) && std::fs::symlink_metadata(path).is_ok())
}

pub fn fs_is_file(p: &str) -> bool {
    let path = Path::new(p);
    match std::fs::metadata(path) {
        Ok(m) => m.is_file(),
        Err(_) => cfg!(windows) && std::fs::symlink_metadata(path).map(|m| !m.is_dir()).unwrap_or(false),
    }
}

pub fn is_absolute_any(p: &str) -> bool {
    if Path::new(p).is_absolute() {
        return true;
    }
    let b = p.as_bytes();
    // Accept Windows absolute forms even when running tests on another OS.
    (b.len() >= 3 && b[1] == b':' && b[0].is_ascii_alphabetic() && (b[2] == b'\\' || b[2] == b'/'))
        || p.starts_with("\\\\")
}

/// Quote one argument following the `CommandLineToArgvW` / MSVCRT rules.
pub fn quote_windows_arg(arg: &str) -> String {
    if !arg.is_empty() && !arg.chars().any(|c| matches!(c, ' ' | '\t' | '\n' | '\x0b' | '"')) {
        return arg.to_string();
    }
    let mut out = String::from("\"");
    let mut backslashes = 0usize;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                out.extend(std::iter::repeat('\\').take(backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            _ => {
                out.extend(std::iter::repeat('\\').take(backslashes));
                backslashes = 0;
                out.push(c);
            }
        }
    }
    out.extend(std::iter::repeat('\\').take(backslashes * 2));
    out.push('"');
    out
}

pub fn join_windows_args(args: &[String]) -> String {
    args.iter().map(|a| quote_windows_arg(a)).collect::<Vec<_>>().join(" ")
}

/// Split a Windows command-line tail (not including the program name) into arguments.
pub fn split_windows_args(s: &str) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < chars.len() {
        while i < chars.len() && (chars[i] == ' ' || chars[i] == '\t') {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }
        let mut cur = String::new();
        let mut in_quotes = false;
        loop {
            let mut bs = 0;
            while i < chars.len() && chars[i] == '\\' {
                bs += 1;
                i += 1;
            }
            if i < chars.len() && chars[i] == '"' {
                cur.extend(std::iter::repeat('\\').take(bs / 2));
                if bs % 2 == 1 {
                    cur.push('"');
                } else if in_quotes && i + 1 < chars.len() && chars[i + 1] == '"' {
                    cur.push('"');
                    i += 1;
                } else {
                    in_quotes = !in_quotes;
                }
                i += 1;
            } else {
                cur.extend(std::iter::repeat('\\').take(bs));
                if i >= chars.len() || (!in_quotes && (chars[i] == ' ' || chars[i] == '\t')) {
                    break;
                }
                cur.push(chars[i]);
                i += 1;
            }
        }
        out.push(cur);
    }
    out
}

/// Exact, human-readable preview of what will be executed.
pub fn command_preview(t: &ExecutableTarget) -> String {
    match t.kind {
        TargetKind::Packaged => format!("shell:AppsFolder\\{}", t.aumid.clone().unwrap_or_default()),
        TargetKind::Uri => t.uri.clone().unwrap_or_default(),
        _ => {
            let mut s = quote_windows_arg(&t.path);
            if !t.args.is_empty() {
                s.push(' ');
                s.push_str(&join_windows_args(&t.args));
            }
            s
        }
    }
}

/// Expand `%NAME%` environment references (Windows style). Unknown names are left intact.
pub fn expand_env(s: &str) -> String {
    static RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"%([A-Za-z0-9_()]+)%").unwrap());
    RE.replace_all(s, |c: &regex::Captures| {
        let name = &c[1];
        std::env::var(name)
            .or_else(|_| {
                std::env::vars()
                    .find(|(k, _)| k.eq_ignore_ascii_case(name))
                    .map(|(_, v)| v)
                    .ok_or(std::env::VarError::NotPresent)
            })
            .unwrap_or_else(|_| c[0].to_string())
    })
    .into_owned()
}

static SECRET_KV: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(password|passwd|pwd|secret|token|api[_-]?key|apikey|access[_-]?key|authorization|client[_-]?secret)\b(\s*[=:]\s*)(\x22[^\x22]*\x22|'[^']*'|\S+)").unwrap()
});
static BEARER: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\b(bearer|basic)\s+[A-Za-z0-9._~+/=\-]{8,}").unwrap());
static LONG_TOKEN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:gh[pousr]_[A-Za-z0-9]{20,}|sk-[A-Za-z0-9\-_]{20,}|AKIA[0-9A-Z]{16}|xox[baprs]-[A-Za-z0-9\-]{10,})\b").unwrap()
});
static URL_CRED: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)([a-z][a-z0-9+.\-]*://)[^/\s:@]+:[^/\s@]+@").unwrap());

pub const REDACTED: &str = "[REDACTED]";

/// Best-effort secret redaction for anything written to logs or history.
pub fn redact(text: &str) -> String {
    let t = URL_CRED.replace_all(text, format!("$1{REDACTED}@").as_str());
    let t = BEARER.replace_all(&t, format!("$1 {REDACTED}").as_str());
    let t = SECRET_KV.replace_all(&t, format!("$1$2{REDACTED}").as_str());
    let t = LONG_TOKEN.replace_all(&t, REDACTED);
    t.into_owned()
}

/// Join a user-provided file name onto a directory, rejecting traversal.
pub fn safe_join(dir: &Path, file_name: &str) -> Result<PathBuf> {
    let name = file_name.trim();
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains(['/', '\\', ':', '\0', '*', '?', '"', '<', '>', '|'])
        || name.ends_with('.')
        || name.ends_with(' ')
    {
        return Err(CoreError::Security(format!("unsafe file name '{name}'")));
    }
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    if ["CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "LPT1", "LPT2", "LPT3"].contains(&stem.as_str()) {
        return Err(CoreError::Security("reserved device name".into()));
    }
    Ok(dir.join(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_policy() {
        assert!(validate_web_url("https://example.com/a?b=1").is_ok());
        assert!(validate_web_url("http://localhost:3000").is_ok());
        for bad in [
            "javascript:alert(1)",
            "file:///C:/Windows/System32/cmd.exe",
            "data:text/html,hi",
            "vbscript:x",
            "ftp://example.com",
            "https://",
            "not a url",
            "https://user:pw@example.com",
            "https://exa\nmple.com",
            "",
        ] {
            assert!(validate_web_url(bad).is_err(), "should reject {bad:?}");
        }
        assert!(validate_web_url(&format!("https://a.com/{}", "x".repeat(3000))).is_err());
    }

    #[test]
    fn settings_uri_policy() {
        assert!(validate_loopback_url("http://127.0.0.1:11434").is_ok());
        assert!(validate_loopback_url("http://localhost:11434/api/tags").is_ok());
        assert!(validate_loopback_url("https://example.com").is_err());
        assert!(validate_loopback_url("http://10.0.0.5:11434").is_err());
        assert!(validate_settings_uri("ms-settings:").is_ok());
        assert!(validate_settings_uri("ms-settings:network").is_ok());
        assert!(validate_settings_uri("https://x.com").is_err());
        assert!(validate_settings_uri("calc:").is_err());
    }

    #[test]
    fn quote_roundtrip() {
        let cases: Vec<Vec<&str>> = vec![
            vec!["simple"],
            vec!["with space", "x"],
            vec!["quote\"inside"],
            vec!["trailing\\"],
            vec!["trailing space\\"],
            vec!["C:\\Program Files\\App\\a.txt", "--flag=1"],
            vec![""],
            vec!["a&b|c>d", "%PATH%", "$(whoami)", "`id`"],
            vec!["back\\\\slashes\\\"quote"],
            vec!["ünï cödé 日本語"],
        ];
        for c in cases {
            let args: Vec<String> = c.iter().map(|s| s.to_string()).collect();
            let joined = join_windows_args(&args);
            assert_eq!(split_windows_args(&joined), args, "roundtrip for {joined}");
        }
    }

    #[test]
    fn quote_known_values() {
        assert_eq!(quote_windows_arg("a b"), "\"a b\"");
        assert_eq!(quote_windows_arg("a\"b"), "\"a\\\"b\"");
        assert_eq!(quote_windows_arg("c:\\dir\\"), "c:\\dir\\");
        assert_eq!(quote_windows_arg("c:\\my dir\\"), "\"c:\\my dir\\\\\"");
        assert_eq!(quote_windows_arg(""), "\"\"");
    }

    #[test]
    fn split_handles_quotes_and_spaces() {
        assert_eq!(split_windows_args("  a   \"b c\" d\\e  "), vec!["a", "b c", "d\\e"]);
        assert_eq!(split_windows_args("\"\""), vec![""]);
        assert!(split_windows_args("   ").is_empty());
    }

    #[test]
    fn redaction() {
        assert!(!redact("password=hunter2 done").contains("hunter2"));
        assert!(!redact("API_KEY: sk-abcdefghijklmnopqrstuvwxyz123456").contains("sk-abc"));
        assert!(!redact("Authorization: Bearer abcdefghijklmnop").contains("abcdefghijklmnop"));
        assert!(!redact("ghp_abcdefghijklmnopqrstuvwxyz0123456789").contains("ghp_abc"));
        assert!(!redact("https://bob:s3cret@example.com/x").contains("s3cret"));
        assert_eq!(redact("nothing secret here"), "nothing secret here");
        assert!(redact("secret=\"two words\" ok").contains("ok"));
        assert!(!redact("secret=\"two words\" ok").contains("two words"));
    }

    #[test]
    fn safe_join_blocks_traversal() {
        let d = Path::new("/tmp/x");
        assert!(safe_join(d, "note.txt").is_ok());
        for bad in ["../evil.txt", "a/b.txt", "a\\b.txt", "..", "", "C:evil", "CON.txt", "name.", "a*b"] {
            assert!(safe_join(d, bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn normalize_collapses_dots() {
        if !cfg!(windows) {
            assert_eq!(normalize_path("/a/b/../c/./d"), "/a/c/d");
            assert_eq!(normalize_path("/a//b/"), "/a/b");
        }
    }

    #[test]
    fn env_expansion() {
        std::env::set_var("PULSE_TEST_VAR", "/opt/x");
        assert_eq!(expand_env("%PULSE_TEST_VAR%/bin"), "/opt/x/bin");
        assert_eq!(expand_env("%NOPE_NOT_SET%/bin"), "%NOPE_NOT_SET%/bin");
    }

    #[test]
    fn validate_target_rules() {
        let t = ExecutableTarget::executable("relative.exe");
        assert!(validate_target(&t).is_err());
        let t = ExecutableTarget::executable("/definitely/missing/app.exe");
        assert!(matches!(validate_target(&t), Err(CoreError::NotFound(_))));
        let t = ExecutableTarget::executable("/tmp/readme.txt");
        assert!(matches!(validate_target(&t), Err(CoreError::Invalid(_))));
        let uri = ExecutableTarget {
            kind: TargetKind::Uri,
            path: String::new(),
            args: vec![],
            working_dir: None,
            aumid: None,
            uri: Some("javascript:alert(1)".into()),
        };
        assert!(validate_target(&uri).is_err());
    }
}
