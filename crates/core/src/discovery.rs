use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};
use crate::lnk::parse_lnk_file;
use crate::models::{AppKind, DiscoveredApp, DiscoveryBatch, SourceKind};
use crate::security::{
    find_on_path, identity_key, is_shell_link, split_desktop_exec, strip_field_codes,
};

#[derive(Debug, Clone)]
pub struct DiscoverOptions {
    pub desktop_dirs: Vec<PathBuf>,
    pub shortcut_dirs: Vec<(PathBuf, SourceKind)>,
    pub include_desktop_shortcuts: bool,
}

impl Default for DiscoverOptions {
    fn default() -> Self {
        Self {
            desktop_dirs: default_desktop_dirs(),
            shortcut_dirs: default_shortcut_dirs(),
            include_desktop_shortcuts: false,
        }
    }
}

pub fn discover_installed(options: &DiscoverOptions) -> DiscoveryBatch {
    let mut batch = DiscoveryBatch {
        apps: Vec::new(),
        errors: Vec::new(),
    };
    if !options.desktop_dirs.is_empty() {
        match discover_desktop_dirs(&options.desktop_dirs) {
            Ok(found) => {
                batch.apps.extend(found.apps);
                batch.errors.extend(found.errors);
            }
            Err(err) => batch.errors.push(err.to_string()),
        }
    }
    for (dir, source) in &options.shortcut_dirs {
        match discover_shortcuts(dir, *source) {
            Ok(found) => {
                batch.apps.extend(found.apps);
                batch.errors.extend(found.errors);
            }
            Err(err) => batch.errors.push(format!("{}: {err}", dir.display())),
        }
    }
    if options.include_desktop_shortcuts {
        if let Some(desktop) = crate::platform::user_desktop_dir() {
            match discover_shortcuts(&desktop, SourceKind::DesktopShortcuts) {
                Ok(found) => {
                    batch.apps.extend(found.apps);
                    batch.errors.extend(found.errors);
                }
                Err(err) => batch.errors.push(err.to_string()),
            }
        }
    }
    batch
        .apps
        .sort_by(|left, right| left.identity_key.cmp(&right.identity_key));
    batch
        .apps
        .dedup_by(|right, left| right.identity_key == left.identity_key);
    batch
}

pub fn discover_desktop_dirs(dirs: &[PathBuf]) -> AppResult<DiscoveryBatch> {
    let mut batch = DiscoveryBatch {
        apps: Vec::new(),
        errors: Vec::new(),
    };
    for dir in dirs {
        if !dir.is_dir() {
            continue;
        }
        let entries = fs::read_dir(dir).map_err(|err| {
            AppError::new("DISCOVERY", format!("Cannot read {}: {err}", dir.display()))
        })?;
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    batch.errors.push(err.to_string());
                    continue;
                }
            };
            let path = entry.path();
            if path.is_dir() {
                if let Ok(mut nested) = discover_desktop_dirs(&[path]) {
                    batch.apps.append(&mut nested.apps);
                    batch.errors.append(&mut nested.errors);
                }
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("desktop") {
                continue;
            }
            match parse_desktop_file(&path) {
                Ok(Some(app)) => batch.apps.push(app),
                Ok(None) => {}
                Err(err) => batch.errors.push(format!("{}: {err}", path.display())),
            }
        }
    }
    Ok(batch)
}

pub fn parse_desktop_file(path: &Path) -> AppResult<Option<DiscoveredApp>> {
    let text = fs::read_to_string(path).map_err(|err| {
        AppError::new(
            "DESKTOP",
            format!("Could not read {}: {err}", path.display()),
        )
    })?;
    let mut in_entry = false;
    let mut name = None;
    let mut exec = None;
    let mut icon = None;
    let mut try_exec = None;
    let mut hidden = false;
    let mut no_display = false;
    let mut app_type = None;
    let mut categories = String::new();
    let mut version = None;
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            in_entry = line.eq_ignore_ascii_case("[Desktop Entry]");
            continue;
        }
        if !in_entry {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if key == "Name" {
            name = Some(value.to_string());
        } else if key == "Exec" {
            exec = Some(value.to_string());
        } else if key == "Icon" {
            icon = Some(value.to_string());
        } else if key == "TryExec" {
            try_exec = Some(value.to_string());
        } else if key == "Type" {
            app_type = Some(value.to_string());
        } else if key.eq_ignore_ascii_case("NoDisplay") && truthy(value) {
            no_display = true;
        } else if key.eq_ignore_ascii_case("Hidden") && truthy(value) {
            hidden = true;
        } else if key == "Categories" {
            categories = value.to_string();
        } else if key == "X-App-Version" {
            // `Version` is the desktop-entry spec revision (usually 1.0), not the application.
            version = Some(value.to_string());
        }
    }
    if hidden || no_display {
        return Ok(None);
    }
    if app_type
        .as_deref()
        .is_some_and(|value| !value.eq_ignore_ascii_case("Application"))
    {
        return Ok(None);
    }
    let Some(exec) = exec else {
        return Ok(None);
    };
    let mut tokens = strip_field_codes(split_desktop_exec(&exec)?);
    if tokens.is_empty() {
        return Err(AppError::new(
            "DESKTOP",
            "Exec value did not contain a program.",
        ));
    }
    let program = tokens.remove(0);
    let resolved = resolve_program(&program);
    let missing = match &try_exec {
        Some(candidate) => resolve_program(candidate).is_none(),
        None => resolved.is_none(),
    };
    let launch_target = resolved.clone().unwrap_or(program);
    let identity = identity_key(&launch_target);
    let display_name = name.unwrap_or_else(|| {
        path.file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("Application")
            .to_string()
    });
    Ok(Some(DiscoveredApp {
        display_name,
        kind: AppKind::Desktop,
        source: SourceKind::DesktopFile,
        launch_target,
        resolved_target: resolved,
        arguments: tokens,
        working_directory: None,
        icon_path: icon.and_then(|icon| resolve_icon(&icon)),
        publisher: None,
        version,
        category: category_from_desktop(&categories),
        missing,
        identity_key: identity,
    }))
}

fn truthy(value: &str) -> bool {
    matches!(value.to_ascii_lowercase().as_str(), "true" | "1" | "yes")
}

fn resolve_program(program: &str) -> Option<String> {
    let path = PathBuf::from(program);
    if path.is_absolute() {
        return path.is_file().then(|| path.to_string_lossy().to_string());
    }
    find_on_path(program).map(|path| path.to_string_lossy().to_string())
}

fn category_from_desktop(categories: &str) -> String {
    let lower = categories.to_ascii_lowercase();
    if lower.contains("development") || lower.contains("ide") {
        "Developer Tools".into()
    } else if lower.contains("office")
        || lower.contains("productivity")
        || lower.contains("utility")
    {
        "Productivity".into()
    } else {
        "Other".into()
    }
}

fn resolve_icon(icon: &str) -> Option<String> {
    let direct = PathBuf::from(icon);
    if direct.is_absolute() && direct.is_file() {
        return Some(direct.to_string_lossy().to_string());
    }
    if icon.contains('/') || icon.contains('\\') || icon.contains('\0') {
        return None;
    }
    let roots = [
        "/usr/share/icons/hicolor",
        "/usr/share/pixmaps",
        "/usr/share/icons",
    ];
    let folders = [
        "scalable/apps",
        "128x128/apps",
        "64x64/apps",
        "48x48/apps",
        "32x32/apps",
        "",
    ];
    let extensions = ["svg", "png", "xpm"];
    for root in roots {
        for folder in folders {
            for extension in extensions {
                let path = if folder.is_empty() {
                    Path::new(root).join(format!("{icon}.{extension}"))
                } else {
                    Path::new(root)
                        .join(folder)
                        .join(format!("{icon}.{extension}"))
                };
                if path.is_file() {
                    return Some(path.to_string_lossy().to_string());
                }
            }
        }
    }
    None
}

pub fn discover_shortcuts(dir: &Path, source: SourceKind) -> AppResult<DiscoveryBatch> {
    let mut batch = DiscoveryBatch {
        apps: Vec::new(),
        errors: Vec::new(),
    };
    if !dir.exists() {
        batch.errors.push(format!(
            "Shortcut directory is not accessible: {}",
            dir.display()
        ));
        return Ok(batch);
    }
    walk_shortcuts(dir, source, &mut batch);
    Ok(batch)
}

fn walk_shortcuts(dir: &Path, source: SourceKind, batch: &mut DiscoveryBatch) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            batch.errors.push(format!("{}: {err}", dir.display()));
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                batch.errors.push(err.to_string());
                continue;
            }
        };
        let path = entry.path();
        if path.is_dir() {
            walk_shortcuts(&path, source, batch);
            continue;
        }
        if !is_shell_link(&path) {
            continue;
        }
        match shortcut_to_app(&path, source) {
            Ok(app) => batch.apps.push(app),
            Err(err) => batch.errors.push(format!("{}: {err}", path.display())),
        }
    }
}

fn shortcut_to_app(path: &Path, source: SourceKind) -> AppResult<DiscoveredApp> {
    let parsed = parse_lnk_file(path)?;
    let raw_target = parsed.path.clone().unwrap_or_default();
    let resolved = if raw_target.is_empty() {
        None
    } else if Path::new(&raw_target).is_file() {
        Some(raw_target.clone())
    } else {
        None
    };
    let missing = resolved.is_none();
    let launch_target = if missing {
        path.to_string_lossy().to_string()
    } else {
        raw_target.clone()
    };
    let identity = identity_key(resolved.as_deref().unwrap_or(&launch_target));
    let display_name = parsed.name.clone().unwrap_or_else(|| {
        path.file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("Shortcut")
            .to_string()
    });
    let arguments = parsed
        .arguments
        .as_deref()
        .map(split_shortcut_args)
        .unwrap_or_default();
    Ok(DiscoveredApp {
        display_name,
        kind: if missing {
            AppKind::Shortcut
        } else {
            AppKind::Win32
        },
        source,
        launch_target,
        resolved_target: resolved,
        arguments,
        working_directory: parsed.working_directory,
        icon_path: parsed.icon_location.and_then(|icon| {
            let icon_path = PathBuf::from(icon);
            icon_path
                .is_file()
                .then(|| icon_path.to_string_lossy().to_string())
        }),
        publisher: None,
        version: None,
        category: "Other".into(),
        missing,
        identity_key: identity,
    })
}

fn split_shortcut_args(input: &str) -> Vec<String> {
    split_desktop_exec(input).unwrap_or_else(|_| vec![input.to_string()])
}

fn default_desktop_dirs() -> Vec<PathBuf> {
    if cfg!(windows) {
        return Vec::new();
    }
    let mut dirs = Vec::new();
    if let Some(home) = crate::security::home_dir() {
        dirs.push(home.join(".local/share/applications"));
    }
    if let Some(data_dirs) = std::env::var_os("XDG_DATA_DIRS") {
        for dir in std::env::split_paths(&data_dirs) {
            dirs.push(dir.join("applications"));
        }
    } else {
        dirs.push(PathBuf::from("/usr/share/applications"));
        dirs.push(PathBuf::from("/usr/local/share/applications"));
    }
    dirs
}

fn default_shortcut_dirs() -> Vec<(PathBuf, SourceKind)> {
    #[cfg(windows)]
    {
        crate::platform::windows_start_menu_dirs()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

pub fn looks_like_zeropulse(app: &DiscoveredApp) -> bool {
    let haystack = format!(
        "{} {} {} {}",
        app.display_name,
        app.publisher.clone().unwrap_or_default(),
        app.launch_target,
        app.resolved_target.clone().unwrap_or_default()
    )
    .to_ascii_lowercase();
    [
        "zeropulse",
        "pulsecut",
        "pulseflow",
        "openpulse",
        "pulseapply",
    ]
    .iter()
    .any(|token| haystack.contains(token))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn host_application_dir_is_scanned_when_present() {
        let dir = PathBuf::from("/usr/share/applications");
        if !dir.is_dir() {
            return;
        }
        let batch = discover_desktop_dirs(&[dir]).unwrap();
        assert!(
            !batch.apps.is_empty(),
            "expected desktop entries, errors: {:?}",
            batch.errors
        );
    }

    #[test]
    fn desktop_discovery_does_not_execute_the_program() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("executed.txt");
        let script = dir.path().join("marker.sh");
        {
            let mut file = fs::File::create(&script).unwrap();
            writeln!(file, "#!/bin/sh").unwrap();
            writeln!(file, "echo pwned > \"{}\"", marker.display()).unwrap();
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&script).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&script, permissions).unwrap();
        }
        let desktop = dir.path().join("marker.desktop");
        fs::write(
            &desktop,
            format!(
                "[Desktop Entry]\nType=Application\nName=Marker App\nExec=\"{}\" --flag\nCategories=Development;\n",
                script.display()
            ),
        )
        .unwrap();
        let batch = discover_desktop_dirs(&[dir.path().to_path_buf()]).unwrap();
        assert!(!marker.exists(), "discovery must not launch executables");
        assert_eq!(batch.apps.len(), 1);
        assert_eq!(batch.apps[0].display_name, "Marker App");
        assert_eq!(batch.apps[0].category, "Developer Tools");
        assert_eq!(batch.apps[0].arguments, vec!["--flag".to_string()]);
        assert!(!batch.apps[0].missing);
        assert_eq!(batch.apps[0].version, None);
    }

    #[test]
    fn desktop_spec_version_is_not_reported_as_the_app_version() {
        let dir = tempfile::tempdir().unwrap();
        let binary = dir.path().join("tool");
        fs::write(&binary, b"#!/bin/sh\n").unwrap();
        fs::write(
            dir.path().join("spec.desktop"),
            format!(
                "[Desktop Entry]\nVersion=1.0\nType=Application\nName=Spec Only\nExec={}\n",
                binary.display()
            ),
        )
        .unwrap();
        fs::write(
            dir.path().join("app.desktop"),
            format!(
                "[Desktop Entry]\nVersion=1.0\nX-App-Version=9.9.1\nType=Application\nName=With App Version\nExec={}\n",
                binary.display()
            ),
        )
        .unwrap();
        let batch = discover_desktop_dirs(&[dir.path().to_path_buf()]).unwrap();
        let spec = batch
            .apps
            .iter()
            .find(|app| app.display_name == "Spec Only")
            .unwrap();
        let app = batch
            .apps
            .iter()
            .find(|app| app.display_name == "With App Version")
            .unwrap();
        assert_eq!(spec.version, None);
        assert_eq!(app.version.as_deref(), Some("9.9.1"));
    }

    #[test]
    fn dedupes_identical_targets() {
        let dir = tempfile::tempdir().unwrap();
        let binary = dir.path().join("tool");
        fs::write(&binary, b"#!/bin/sh\n").unwrap();
        for name in ["a.desktop", "b.desktop"] {
            fs::write(
                dir.path().join(name),
                format!(
                    "[Desktop Entry]\nType=Application\nName={name}\nExec={}\n",
                    binary.display()
                ),
            )
            .unwrap();
        }
        let mut batch = discover_desktop_dirs(&[dir.path().to_path_buf()]).unwrap();
        batch
            .apps
            .sort_by(|a, b| a.identity_key.cmp(&b.identity_key));
        batch.apps.dedup_by(|a, b| a.identity_key == b.identity_key);
        assert_eq!(batch.apps.len(), 1);
    }

    #[test]
    fn zeropulse_suggestion_requires_a_real_name() {
        let app = DiscoveredApp {
            display_name: "PulseCut".into(),
            kind: AppKind::Portable,
            source: SourceKind::Manual,
            launch_target: "/opt/PulseCut.exe".into(),
            resolved_target: None,
            arguments: Vec::new(),
            working_directory: None,
            icon_path: None,
            publisher: Some("ZeroPulse".into()),
            version: None,
            category: "Other".into(),
            missing: false,
            identity_key: "path:/opt/pulsecut.exe".into(),
        };
        assert!(looks_like_zeropulse(&app));
        let other = DiscoveredApp {
            display_name: "Calculator".into(),
            launch_target: "/usr/bin/gnome-calculator".into(),
            publisher: Some("GNOME".into()),
            identity_key: "path:/usr/bin/gnome-calculator".into(),
            ..app
        };
        assert!(!looks_like_zeropulse(&other));
    }
}
