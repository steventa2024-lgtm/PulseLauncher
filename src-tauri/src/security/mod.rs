//! Validation, policy and redaction helpers. Every value that reaches an OS
//! API passes through one of these functions first.

use std::path::{Component, Path, PathBuf};

use crate::error::{AppError, AppResult};
use crate::models::{AppKind, ExecutableTarget};

pub const MAX_ARGS: usize = 64;
pub const MAX_ARG_LEN: usize = 4096;
pub const MAX_PATH_LEN: usize = 2048;

/// Schemes that must never be opened, even if a user saved them. These have
/// a history of being abused for code execution or credential theft.
const BLOCKED_SCHEMES: &[&str] = &[
    "javascript",
    "vbscript",
    "data",
    "file",
    "jar",
    "ms-msdt",
    "search-ms",
    "search",
    "ms-officecmd",
    "ms-cxh",
    "ms-cxh-full",
    "ms-appinstaller",
    "shell",
    "hcp",
    "its",
    "mk",
    "ms-its",
    "res",
    "ldap",
    "smb",
];

pub fn bounded_text(s: &str, max: usize, field: &str) -> AppResult<String> {
    let t = s.trim();
    if t.is_empty() {
        return Err(AppError::invalid(format!("{field} must not be empty")));
    }
    if t.chars().count() > max {
        return Err(AppError::invalid(format!(
            "{field} must be at most {max} characters"
        )));
    }
    if t.chars().any(|c| c.is_control()) {
        return Err(AppError::invalid(format!(
            "{field} must not contain control characters"
        )));
    }
    Ok(t.to_string())
}

/// Validate a web URL opened in the default browser: http(s) only.
pub fn validate_web_url(raw: &str) -> AppResult<url::Url> {
    let raw = raw.trim();
    if raw.len() > MAX_PATH_LEN {
        return Err(AppError::invalid("URL is too long"));
    }
    let url = url::Url::parse(raw).map_err(|e| AppError::invalid(format!("Invalid URL: {e}")))?;
    match url.scheme() {
        "http" | "https" => {}
        other => {
            return Err(AppError::policy(format!(
                "Only http:// and https:// links can be opened here (got \"{other}:\")"
            )))
        }
    }
    if url.host_str().unwrap_or("").is_empty() {
        return Err(AppError::invalid("URL must include a host"));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(AppError::policy(
            "URLs with embedded credentials are not allowed",
        ));
    }
    Ok(url)
}

/// Validate a protocol/URI registration such as `ms-settings:display`.
pub fn validate_uri(raw: &str) -> AppResult<String> {
    let raw = raw.trim();
    if raw.is_empty() || raw.len() > MAX_PATH_LEN {
        return Err(AppError::invalid("URI is empty or too long"));
    }
    if raw.chars().any(|c| c.is_control() || c == '"') {
        return Err(AppError::invalid("URI contains forbidden characters"));
    }
    let (scheme, _) = raw
        .split_once(':')
        .ok_or_else(|| AppError::invalid("URI must have a scheme, e.g. ms-settings:"))?;
    let scheme_l = scheme.to_ascii_lowercase();
    let valid = !scheme_l.is_empty()
        && scheme_l.len() > 1 // reject drive letters such as C:
        && scheme_l.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && scheme_l
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.');
    if !valid {
        return Err(AppError::invalid(format!(
            "\"{scheme}\" is not a valid URI scheme"
        )));
    }
    if BLOCKED_SCHEMES.contains(&scheme_l.as_str()) {
        return Err(AppError::policy(format!(
            "The \"{scheme_l}:\" scheme is blocked for safety"
        )));
    }
    if scheme_l == "http" || scheme_l == "https" {
        validate_web_url(raw)?;
    }
    Ok(raw.to_string())
}

pub fn validate_ms_settings_uri(raw: &str) -> AppResult<String> {
    let uri = validate_uri(raw)?;
    if !uri.to_ascii_lowercase().starts_with("ms-settings:") {
        return Err(AppError::policy(
            "Only ms-settings: URIs are allowed for this action",
        ));
    }
    Ok(uri)
}

pub fn validate_args(args: &[String]) -> AppResult<()> {
    if args.len() > MAX_ARGS {
        return Err(AppError::invalid(format!(
            "At most {MAX_ARGS} arguments are allowed"
        )));
    }
    for a in args {
        if a.len() > MAX_ARG_LEN {
            return Err(AppError::invalid("An argument is too long"));
        }
        if a.contains('\0') {
            return Err(AppError::invalid(
                "Arguments must not contain NUL characters",
            ));
        }
    }
    Ok(())
}

/// Absolute, normalized path without `..` traversal.
pub fn validate_absolute_path(raw: &str, what: &str) -> AppResult<PathBuf> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(AppError::invalid(format!("{what} path is empty")));
    }
    if raw.len() > MAX_PATH_LEN || raw.contains('\0') {
        return Err(AppError::invalid(format!("{what} path is invalid")));
    }
    let p = Path::new(raw);
    if !is_absolute_any(raw) {
        return Err(AppError::invalid(format!(
            "{what} must be an absolute path (got \"{raw}\")"
        )));
    }
    // Check both separators so a Windows path stored or tested on another
    // host cannot sneak a parent segment through.
    if p.components().any(|c| matches!(c, Component::ParentDir))
        || raw.split(['/', '\\']).any(|seg| seg == "..")
    {
        return Err(AppError::policy(format!(
            "{what} path must not contain \"..\""
        )));
    }
    Ok(p.to_path_buf())
}

fn is_absolute_any(raw: &str) -> bool {
    if Path::new(raw).is_absolute() {
        return true;
    }
    // Windows paths are also accepted on other hosts so stored data and tests
    // remain portable; the OS layer still checks existence.
    let b = raw.as_bytes();
    (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/'))
        || raw.starts_with("\\\\")
}

fn has_ext(p: &Path, exts: &[&str]) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| exts.iter().any(|x| e.eq_ignore_ascii_case(x)))
        .unwrap_or(false)
}

/// Rules for a user registration (Add App wizard / edit).
pub fn validate_registration(t: &ExecutableTarget) -> AppResult<()> {
    validate_args(&t.arguments)?;
    match t.kind {
        AppKind::Executable => {
            let p = validate_absolute_path(&t.launch_target, "Executable")?;
            if !has_ext(&p, &["exe", "com"]) {
                return Err(AppError::invalid(
                    "Choose a Windows program file ending in .exe",
                ));
            }
            if !p.is_file() {
                return Err(AppError::new(
                    crate::error::ErrorCode::TargetMissing,
                    format!("File not found: {}", p.display()),
                ));
            }
        }
        AppKind::Shortcut => {
            let p = validate_absolute_path(&t.launch_target, "Shortcut")?;
            if !has_ext(&p, &["lnk"]) {
                return Err(AppError::invalid(
                    "Choose a Windows shortcut ending in .lnk",
                ));
            }
            if !p.is_file() {
                return Err(AppError::new(
                    crate::error::ErrorCode::TargetMissing,
                    format!("File not found: {}", p.display()),
                ));
            }
            if !t.arguments.is_empty() {
                return Err(AppError::invalid(
                    "Shortcuts carry their own arguments; extra arguments are not supported",
                ));
            }
        }
        AppKind::Uri => {
            validate_uri(&t.launch_target)?;
            if !t.arguments.is_empty() {
                return Err(AppError::invalid("URI entries do not take arguments"));
            }
        }
        AppKind::Packaged => {
            let id = t.launch_target.trim();
            if !id.contains('!')
                || id.len() > 512
                || id
                    .chars()
                    .any(|c| c.is_whitespace() || c == '\\' || c == '/')
            {
                return Err(AppError::invalid(
                    "Not a valid packaged app identifier (AUMID)",
                ));
            }
        }
    }
    if let Some(wd) = &t.working_dir {
        let p = validate_absolute_path(wd, "Working directory")?;
        if !p.is_dir() {
            return Err(AppError::new(
                crate::error::ErrorCode::TargetMissing,
                format!("Working directory not found: {}", p.display()),
            ));
        }
    }
    Ok(())
}

/// Quote one argument following the rules used by `CommandLineToArgvW` and
/// the MSVC runtime, so the receiving program sees exactly `arg`.
pub fn quote_windows_arg(arg: &str) -> String {
    if !arg.is_empty()
        && !arg
            .chars()
            .any(|c| matches!(c, ' ' | '\t' | '\n' | '\x0b' | '"'))
    {
        return arg.to_string();
    }
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('"');
    let mut backslashes = 0usize;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                out.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            _ => {
                out.extend(std::iter::repeat_n('\\', backslashes));
                out.push(c);
                backslashes = 0;
            }
        }
    }
    out.extend(std::iter::repeat_n('\\', backslashes * 2));
    out.push('"');
    out
}

pub fn join_windows_args(args: &[String]) -> String {
    args.iter()
        .map(|a| quote_windows_arg(a))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Human-readable preview of exactly what will run.
pub fn preview_command(program: &str, args: &[String]) -> String {
    let mut s = quote_windows_arg(program);
    if !args.is_empty() {
        s.push(' ');
        s.push_str(&join_windows_args(args));
    }
    s
}

/// Best-effort secret redaction for anything written to logs or history.
pub fn redact(input: &str) -> String {
    const KEYS: &[&str] = &[
        "password",
        "passwd",
        "pwd",
        "secret",
        "token",
        "apikey",
        "api_key",
        "api-key",
        "authorization",
        "bearer",
        "access_key",
        "private_key",
        "client_secret",
    ];
    let mut out = String::with_capacity(input.len());
    for line in input.split_inclusive('\n') {
        let lower = line.to_ascii_lowercase();
        let mut redacted = line.to_string();
        for key in KEYS {
            if let Some(pos) = lower.find(key) {
                let after = pos + key.len();
                let rest = &lower[after..];
                let sep = rest
                    .char_indices()
                    .find(|(_, c)| *c == '=' || *c == ':' || *c == ' ');
                if let Some((i, _)) = sep {
                    let value_start = after + i + 1;
                    if value_start < line.len() {
                        let tail_end = line[value_start..]
                            .find(['\n', '&', ';', ',', '"', '\''])
                            .map(|e| value_start + e)
                            .unwrap_or(line.len());
                        if !line[value_start..tail_end].trim().is_empty() {
                            redacted =
                                format!("{}[REDACTED]{}", &line[..value_start], &line[tail_end..]);
                        }
                    }
                }
                break;
            }
        }
        out.push_str(&redacted);
    }
    // Common token shapes.
    let mut words: Vec<String> = Vec::new();
    for w in out.split(' ') {
        let looks_secret = (w.starts_with("ghp_")
            || w.starts_with("github_pat_")
            || w.starts_with("sk-")
            || w.starts_with("xox"))
            && w.len() > 12;
        words.push(if looks_secret {
            "[REDACTED]".into()
        } else {
            w.to_string()
        });
    }
    words.join(" ")
}

/// Ensure a resolved file path stays inside `base` (for note creation etc.).
pub fn ensure_within(base: &Path, candidate: &Path) -> AppResult<()> {
    let b = base.canonicalize()?;
    let parent = candidate
        .parent()
        .ok_or_else(|| AppError::invalid("Invalid file path"))?
        .canonicalize()?;
    if !parent.starts_with(&b) {
        return Err(AppError::policy("Path escapes the selected folder"));
    }
    Ok(())
}

/// Reduce a user-supplied file name to safe characters.
pub fn sanitize_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim().trim_end_matches(['.', ' ']).to_string();
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "LPT1", "LPT2", "LPT3",
    ];
    let stem = trimmed.split('.').next().unwrap_or("").to_ascii_uppercase();
    if trimmed.is_empty() || reserved.contains(&stem.as_str()) {
        return format!("note_{trimmed}");
    }
    trimmed.chars().take(120).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_urls() {
        assert!(validate_web_url("https://example.com/a?b=c").is_ok());
        assert!(validate_web_url("http://localhost:3000").is_ok());
        for bad in [
            "javascript:alert(1)",
            "file:///C:/Windows/System32/calc.exe",
            "ftp://x",
            "https://user:pw@example.com",
            "not a url",
            "data:text/html,hi",
        ] {
            assert!(validate_web_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn uris() {
        assert!(validate_uri("ms-settings:display").is_ok());
        assert!(validate_uri("mailto:a@b.c").is_ok());
        assert!(validate_uri("C:\\Windows\\notepad.exe").is_err());
        assert!(validate_uri("ms-msdt:/id PCWDiagnostic").is_err());
        assert!(validate_uri("search-ms:query=x").is_err());
        assert!(validate_uri("FILE:///c:/x").is_err());
        assert!(validate_uri("ms-settings:\"&calc").is_err());
        assert!(validate_ms_settings_uri("https://x.com").is_err());
        assert!(validate_ms_settings_uri("ms-settings:").is_ok());
    }

    #[test]
    fn windows_quoting_matches_commandlinetoargvw_rules() {
        assert_eq!(quote_windows_arg("simple"), "simple");
        assert_eq!(quote_windows_arg(""), "\"\"");
        assert_eq!(quote_windows_arg("a b"), "\"a b\"");
        assert_eq!(quote_windows_arg("C:\\My Dir\\"), "\"C:\\My Dir\\\\\"");
        assert_eq!(quote_windows_arg("say \"hi\""), "\"say \\\"hi\\\"\"");
        assert_eq!(quote_windows_arg("a\\\\\"b"), "\"a\\\\\\\\\\\"b\"");
        assert_eq!(quote_windows_arg("x&y|z"), "x&y|z");
        assert_eq!(
            join_windows_args(&["--path".into(), "C:\\Program Files\\x".into()]),
            "--path \"C:\\Program Files\\x\""
        );
    }

    #[test]
    fn rejects_traversal_and_relative_paths() {
        assert!(validate_absolute_path("relative\\x.exe", "x").is_err());
        assert!(validate_absolute_path("C:\\a\\..\\b.exe", "x").is_err());
        assert!(validate_absolute_path("C:\\a\\b.exe", "x").is_ok());
        assert!(validate_absolute_path("\\\\server\\share\\b.exe", "x").is_ok());
        assert!(validate_args(&["a\0b".into()]).is_err());
    }

    #[test]
    fn redacts_secrets() {
        let r = redact("login password=hunter2 ok\nTOKEN: abc123\nnothing here");
        assert!(!r.contains("hunter2"));
        assert!(!r.contains("abc123"));
        assert!(r.contains("nothing here"));
        assert!(!redact("use ghp_abcdefghijklmnop now").contains("ghp_abcdefghijklmnop"));
    }

    #[test]
    fn sanitizes_file_names() {
        assert_eq!(sanitize_file_name("a/b:c*.txt"), "a_b_c_.txt");
        assert_eq!(sanitize_file_name("CON.txt"), "note_CON.txt");
        assert_eq!(sanitize_file_name("..."), "note_");
    }
}
