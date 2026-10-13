use std::path::{Component, Path, PathBuf};

use regex::Regex;
use url::Url;

use crate::error::{AppError, AppResult};

const MAX_ARG: usize = 4096;
const MAX_ARGS: usize = 64;
const MAX_URL: usize = 2000;

pub fn redact(input: &str) -> String {
    let patterns = [
        r"(?i)\b(api[_-]?key|token|password|secret|authorization)\b\s*[:=]\s*\S+",
        r"(?i)\bbearer\s+\S+",
        r"(?i)\bsk-[A-Za-z0-9]{8,}\b",
    ];
    let mut out = input.to_string();
    for pattern in patterns {
        let re = Regex::new(pattern).expect("redaction pattern");
        out = re.replace_all(&out, "[redacted]").into_owned();
    }
    out
}

pub fn validate_url(raw: &str) -> AppResult<String> {
    let raw = raw.trim();
    if raw.is_empty() || raw.len() > MAX_URL {
        return Err(AppError::new("URL", "URL is empty or too long."));
    }
    if raw.chars().any(|c| c == '\0' || c == '\n' || c == '\r') {
        return Err(AppError::new("URL", "URL contains a control character."));
    }
    let lower = raw.to_ascii_lowercase();
    if lower.starts_with("ms-settings:") {
        let rest = &raw["ms-settings:".len()..];
        if rest.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | '=' | '&' | '%')
        }) {
            return Ok(raw.to_string());
        }
        return Err(AppError::new(
            "URL",
            "ms-settings URI contains unsupported characters.",
        ));
    }
    let url = Url::parse(raw).map_err(|_| AppError::new("URL", "URL is malformed."))?;
    match url.scheme() {
        "http" | "https" => {}
        other => {
            return Err(AppError::new(
                "URL",
                format!("Scheme '{other}' is not allowed. Use http or https."),
            ));
        }
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(AppError::new(
            "URL",
            "URLs that embed credentials are rejected.",
        ));
    }
    if url.host_str().is_none() {
        return Err(AppError::new("URL", "URL is missing a host."));
    }
    Ok(url.to_string())
}

pub fn validate_http_endpoint(raw: &str) -> AppResult<String> {
    let url = validate_url(raw)?;
    if url.to_ascii_lowercase().starts_with("ms-settings:") {
        return Err(AppError::new(
            "URL",
            "An HTTP endpoint is required for this integration.",
        ));
    }
    Ok(url.trim_end_matches('/').to_string())
}

pub fn quote_windows_arg(arg: &str) -> String {
    if !arg.is_empty()
        && !arg
            .chars()
            .any(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '"'))
    {
        return arg.to_string();
    }
    let mut out = String::from("\"");
    let mut backslashes = 0;
    for ch in arg.chars() {
        if ch == '\\' {
            backslashes += 1;
            continue;
        }
        if ch == '"' {
            out.push_str(&"\\".repeat(backslashes * 2 + 1));
            out.push('"');
            backslashes = 0;
            continue;
        }
        out.push_str(&"\\".repeat(backslashes));
        out.push(ch);
        backslashes = 0;
    }
    out.push_str(&"\\".repeat(backslashes * 2));
    out.push('"');
    out
}

pub fn display_command(program: &str, args: &[String]) -> String {
    let mut parts = Vec::with_capacity(args.len() + 1);
    parts.push(quote_windows_arg(program));
    for arg in args {
        parts.push(quote_windows_arg(arg));
    }
    parts.join(" ")
}

pub fn validate_arguments(args: &[String]) -> AppResult<()> {
    if args.len() > MAX_ARGS {
        return Err(AppError::new(
            "ARGS",
            format!("At most {MAX_ARGS} arguments are allowed."),
        ));
    }
    for arg in args {
        if arg.len() > MAX_ARG {
            return Err(AppError::new("ARGS", "An argument is too long."));
        }
        if arg.chars().any(|c| c == '\0' || c == '\n' || c == '\r') {
            return Err(AppError::new(
                "ARGS",
                "Arguments cannot contain NUL or newline characters.",
            ));
        }
    }
    Ok(())
}

pub fn is_uri_target(target: &str) -> bool {
    let lower = target.trim().to_ascii_lowercase();
    lower.starts_with("ms-settings:")
        || lower.starts_with("http://")
        || lower.starts_with("https://")
}

pub fn dangerous_windows_extension(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("bat" | "cmd" | "ps1" | "vbs" | "js" | "jse" | "wsf" | "msi" | "scr" | "com" | "pif")
    )
}

pub fn is_shell_link(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("lnk"))
}

pub fn validate_existing_program(path: &Path) -> AppResult<PathBuf> {
    if path.as_os_str().is_empty() {
        return Err(AppError::new("PATH", "Program path is empty."));
    }
    let text = path.to_string_lossy();
    if text.contains('\0') || text.contains('\n') {
        return Err(AppError::new(
            "PATH",
            "Program path contains a control character.",
        ));
    }
    if !path.is_absolute() {
        return Err(AppError::new(
            "PATH",
            "Program path must be absolute. PulseLauncher does not resolve commands through a shell.",
        ));
    }
    if !path.exists() {
        return Err(AppError::new(
            "MISSING_TARGET",
            format!("Target does not exist: {}", path.display()),
        ));
    }
    if !path.is_file() {
        return Err(AppError::new(
            "PATH",
            "Target is not a file. Choose an executable.",
        ));
    }
    if dangerous_windows_extension(path) {
        return Err(AppError::new(
            "PATH",
            "Script and installer extensions are blocked. Register an .exe or a shortcut instead.",
        ));
    }
    #[cfg(windows)]
    {
        let ext_ok = is_shell_link(path)
            || path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("exe"));
        if !ext_ok {
            return Err(AppError::new(
                "PATH",
                "On Windows, registered programs must be .exe or .lnk files.",
            ));
        }
    }
    let canonical = path.canonicalize().map_err(|err| {
        AppError::new(
            "PATH",
            format!("Could not resolve {}: {err}", path.display()),
        )
    })?;
    if let Ok(current) = std::env::current_exe() {
        if let Ok(current) = current.canonicalize() {
            if current == canonical {
                return Err(AppError::new(
                    "PATH",
                    "Refusing to launch PulseLauncher from inside PulseLauncher.",
                ));
            }
        }
    }
    Ok(canonical)
}

pub fn validate_existing_directory(path: &Path) -> AppResult<PathBuf> {
    if !path.is_absolute() {
        return Err(AppError::new("PATH", "Folder path must be absolute."));
    }
    if !path.exists() {
        return Err(AppError::new(
            "MISSING_TARGET",
            format!("Folder does not exist: {}", path.display()),
        ));
    }
    if !path.is_dir() {
        return Err(AppError::new("PATH", "Path is not a folder."));
    }
    path.canonicalize()
        .map_err(|err| AppError::new("PATH", err.to_string()))
}

pub fn validate_index_root(path: &Path) -> AppResult<PathBuf> {
    let canon = validate_existing_directory(path)?;
    if is_filesystem_root(&canon) {
        return Err(AppError::new(
            "INDEX",
            "Refusing to index a drive root. Choose a specific folder.",
        ));
    }
    if let Some(home) = home_dir() {
        if canon == home {
            return Err(AppError::new(
                "INDEX",
                "Refusing to index the entire home directory. Choose a project or documents folder.",
            ));
        }
    }
    Ok(canon)
}

fn is_filesystem_root(path: &Path) -> bool {
    let mut components = path.components();
    match (components.next(), components.next()) {
        (Some(Component::RootDir), None) => true,
        (Some(Component::Prefix(_)), Some(Component::RootDir)) => components.next().is_none(),
        _ => false,
    }
}

pub fn home_dir() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("HOME") {
        let path = PathBuf::from(home);
        if path.is_dir() {
            return path.canonicalize().ok().or(Some(path));
        }
    }
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        let path = PathBuf::from(profile);
        if path.is_dir() {
            return path.canonicalize().ok().or(Some(path));
        }
    }
    None
}

pub fn identity_key(target: &str) -> String {
    let trimmed = target.trim();
    if is_uri_target(trimmed) {
        return format!("uri:{}", trimmed.to_ascii_lowercase());
    }
    let path = Path::new(trimmed);
    let canon = path
        .canonicalize()
        .unwrap_or_else(|_| normalize_logical(path));
    let mut text = canon.to_string_lossy().to_string();
    if cfg!(windows) {
        text = text.to_lowercase();
    }
    format!("path:{text}")
}

pub fn normalize_logical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

pub fn expand_percent(input: &str, lookup: impl Fn(&str) -> Option<String>) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '%' {
            if let Some(rel) = chars[i + 1..].iter().position(|c| *c == '%') {
                let name: String = chars[i + 1..i + 1 + rel].iter().collect();
                if !name.is_empty() {
                    if let Some(value) = lookup(&name) {
                        out.push_str(&value);
                        i += rel + 2;
                        continue;
                    }
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

pub fn expand_env(input: &str) -> String {
    expand_percent(input, |name| std::env::var(name).ok())
}

pub fn split_desktop_exec(input: &str) -> AppResult<Vec<String>> {
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    let mut tokens = Vec::new();
    while i < chars.len() {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }
        let mut token = String::new();
        let mut quote: Option<char> = None;
        while i < chars.len() {
            let ch = chars[i];
            if quote.is_none() && ch.is_whitespace() {
                break;
            }
            if ch == '\\' && i + 1 < chars.len() {
                let next = chars[i + 1];
                match next {
                    's' => token.push(' '),
                    'n' => token.push('\n'),
                    't' => token.push('\t'),
                    'r' => token.push('\r'),
                    '\\' | '"' | '\'' => token.push(next),
                    other => {
                        token.push('\\');
                        token.push(other);
                    }
                }
                i += 2;
                continue;
            }
            if (ch == '"' || ch == '\'') && quote.is_none() {
                quote = Some(ch);
                i += 1;
                continue;
            }
            if Some(ch) == quote {
                quote = None;
                i += 1;
                continue;
            }
            token.push(ch);
            i += 1;
        }
        if quote.is_some() {
            return Err(AppError::new(
                "DESKTOP",
                "Desktop Exec value has an unterminated quote.",
            ));
        }
        if !token.is_empty() {
            tokens.push(token);
        }
    }
    Ok(tokens)
}

pub fn strip_field_codes(tokens: Vec<String>) -> Vec<String> {
    let codes = [
        "f", "F", "u", "U", "d", "D", "n", "N", "i", "c", "k", "v", "m",
    ];
    tokens
        .into_iter()
        .filter_map(|token| {
            if token.len() == 2 && token.starts_with('%') {
                let code = &token[1..];
                if codes.contains(&code) {
                    return None;
                }
            }
            Some(token.replace("%%", "%"))
        })
        .collect()
}

pub fn validate_accelerator(input: &str) -> AppResult<String> {
    let trimmed = input.trim();
    if trimmed.len() > 64 {
        return Err(AppError::new("SHORTCUT", "Shortcut is too long."));
    }
    let parts: Vec<&str> = trimmed
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    if parts.len() < 2 {
        return Err(AppError::new(
            "SHORTCUT",
            "Use at least one modifier and a key, for example Alt+Space.",
        ));
    }
    let key = parts[parts.len() - 1];
    let mods = &parts[..parts.len() - 1];
    let mut saw_primary = false;
    for modifier in mods {
        match modifier.to_ascii_lowercase().as_str() {
            "ctrl" | "control" | "alt" | "option" | "shift" | "super" | "meta" | "command"
            | "cmd" | "commandorcontrol" | "cmdorctrl" => {
                if !matches!(modifier.to_ascii_lowercase().as_str(), "shift") {
                    saw_primary = true;
                }
            }
            _ => {
                return Err(AppError::new(
                    "SHORTCUT",
                    format!("Unknown modifier '{modifier}'. Put the key last."),
                ));
            }
        }
    }
    if !saw_primary {
        return Err(AppError::new(
            "SHORTCUT",
            "Include Ctrl, Alt, Super, or CommandOrControl so typing is not captured.",
        ));
    }
    let key_ok = matches!(
        key.to_ascii_lowercase().as_str(),
        "space"
            | "enter"
            | "tab"
            | "escape"
            | "esc"
            | "minus"
            | "equal"
            | "comma"
            | "period"
            | "slash"
            | "backslash"
            | "backquote"
    ) || (key.len() == 1
        && key
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric()))
        || (key.len() >= 2
            && key.len() <= 3
            && key.to_ascii_lowercase().starts_with('f')
            && key[1..].parse::<u8>().is_ok_and(|n| (1..=12).contains(&n)));
    if !key_ok {
        return Err(AppError::new(
            "SHORTCUT",
            format!("Key '{key}' is not supported."),
        ));
    }
    let canonical_mods: Vec<String> = mods
        .iter()
        .map(|modifier| match modifier.to_ascii_lowercase().as_str() {
            "control" | "ctrl" => "Ctrl".into(),
            "option" | "alt" => "Alt".into(),
            "super" | "meta" | "command" | "cmd" => "Super".into(),
            "shift" => "Shift".into(),
            "commandorcontrol" | "cmdorctrl" => "CommandOrControl".into(),
            other => other.to_string(),
        })
        .collect();
    let canonical_key = if key.eq_ignore_ascii_case("space") {
        "Space".into()
    } else if key.eq_ignore_ascii_case("esc") || key.eq_ignore_ascii_case("escape") {
        "Escape".into()
    } else if key.len() == 1 {
        key.to_ascii_uppercase()
    } else if key.to_ascii_lowercase().starts_with('f') && key[1..].parse::<u8>().is_ok() {
        format!("F{}", &key[1..])
    } else {
        let mut chars = key.chars();
        let first = chars.next().unwrap_or_default().to_ascii_uppercase();
        format!("{first}{}", chars.as_str().to_ascii_lowercase())
    };
    Ok(format!("{}+{canonical_key}", canonical_mods.join("+")))
}

pub fn find_on_path(name: &str) -> Option<PathBuf> {
    if name.contains('/') || name.contains('\\') || name.contains('\0') {
        return None;
    }
    let paths = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&paths) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        {
            let exe = dir.join(format!("{name}.exe"));
            if exe.is_file() {
                return Some(exe);
            }
        }
    }
    None
}

pub fn validate_user_json_path(path: &Path, must_exist: bool) -> AppResult<PathBuf> {
    if path.as_os_str().is_empty() || !path.is_absolute() {
        return Err(AppError::new("PATH", "Choose an absolute .json file path."));
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(AppError::new("PATH", "The path must not contain '..'."));
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if !name.to_ascii_lowercase().ends_with(".json") || name.len() > 180 || name.contains('\0') {
        return Err(AppError::new("PATH", "Choose a .json file name."));
    }
    if path.is_dir() {
        return Err(AppError::new("PATH", "Choose a file, not a folder."));
    }
    let parent = path
        .parent()
        .ok_or_else(|| AppError::new("PATH", "Choose a file inside an existing folder."))?;
    if !parent.is_dir() {
        return Err(AppError::new(
            "PATH",
            "The destination folder does not exist.",
        ));
    }
    if must_exist && !path.is_file() {
        return Err(AppError::new("PATH", "That file does not exist."));
    }
    Ok(path.to_path_buf())
}

pub fn bounded_text(value: &str, max: usize, label: &str) -> AppResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::new("INPUT", format!("{label} is required.")));
    }
    if trimmed.len() > max {
        return Err(AppError::new(
            "INPUT",
            format!("{label} is longer than {max} characters."),
        ));
    }
    if trimmed.contains('\0') {
        return Err(AppError::new("INPUT", format!("{label} contains NUL.")));
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_dangerous_urls() {
        assert!(validate_url("javascript:alert(1)").is_err());
        assert!(validate_url("file:///etc/passwd").is_err());
        assert!(validate_url("http://user:pass@example.com").is_err());
        assert!(validate_url("https://example.com/docs").is_ok());
        assert_eq!(
            validate_url("ms-settings:display").unwrap(),
            "ms-settings:display"
        );
    }

    #[test]
    fn quotes_windows_arguments_without_shell_splitting() {
        assert_eq!(quote_windows_arg("plain"), "plain");
        assert_eq!(quote_windows_arg("a b"), "\"a b\"");
        assert_eq!(quote_windows_arg(r#"say "hi""#), r#""say \"hi\"""#);
        let shown = display_command(
            "C:\\Program Files\\app.exe",
            &["--name".into(), "a b".into()],
        );
        assert!(
            shown.contains("\"C:\\\\Program Files\\\\app.exe\"") || shown.contains("Program Files")
        );
        assert!(shown.contains("\"a b\""));
    }

    #[test]
    fn redacts_secrets() {
        let text = "token=abcd1234 and Bearer supersecret and sk-abcdef123456";
        let red = redact(text);
        assert!(!red.contains("abcd1234"));
        assert!(!red.contains("supersecret"));
        assert!(!red.contains("sk-abcdef123456"));
        assert!(red.contains("[redacted]"));
    }

    #[test]
    fn parses_desktop_exec_and_strips_codes() {
        let tokens = split_desktop_exec(r#""/opt/My App/bin" --flag %u"#).unwrap();
        let tokens = strip_field_codes(tokens);
        assert_eq!(tokens, vec!["/opt/My App/bin".to_string(), "--flag".into()]);
    }

    #[test]
    fn accelerator_requires_a_primary_modifier() {
        assert!(validate_accelerator("Space").is_err());
        assert!(validate_accelerator("Shift+A").is_err());
        assert_eq!(validate_accelerator("alt+space").unwrap(), "Alt+Space");
        assert_eq!(
            validate_accelerator("ctrl+alt+Space").unwrap(),
            "Ctrl+Alt+Space"
        );
    }

    #[test]
    fn expands_percent_variables_from_lookup() {
        let value = expand_percent("%WINDIR%\\notepad.exe", |name| {
            (name == "WINDIR").then(|| "C:\\Windows".into())
        });
        assert_eq!(value, "C:\\Windows\\notepad.exe");
    }

    #[test]
    fn rejects_relative_json_export_paths() {
        assert!(validate_user_json_path(Path::new("backup.json"), false).is_err());
        assert!(validate_user_json_path(Path::new("/tmp/../etc/passwd.json"), false).is_err());
    }

    #[test]
    fn rejects_home_and_root_index_targets() {
        assert!(validate_index_root(Path::new("/")).is_err());
        if let Some(home) = home_dir() {
            assert!(validate_index_root(&home).is_err());
        }
    }
}
