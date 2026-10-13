use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};

use crate::discovery::looks_like_zeropulse;
use crate::error::{AppError, AppResult};
use crate::files::{delete_preview_file, file_size, path_missing};
use crate::models::TEMPLATE_VERSION;
use crate::models::{
    now_iso, AppEntry, AppKind, Collection, Diagnostics, DiscoveredApp, ExportBundle,
    ImportPreview, LaunchEvent, PreviewAsset, RegisterRequest, ScanReport, SourceKind,
    UpdateAppRequest, UserPreferences, WorkflowRecord, WorkflowRun, SCHEMA_VERSION,
    ZEROPULSE_COLLECTION_ID,
};
use crate::security::{
    bounded_text, identity_key, is_shell_link, is_uri_target, validate_accelerator,
    validate_arguments, validate_existing_program, validate_http_endpoint, validate_index_root,
    validate_url,
};
use crate::workflows::{builtin_templates, validate_document};

const MIGRATION: &str = r#"
CREATE TABLE IF NOT EXISTS meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS preferences (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS apps (
    id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    kind TEXT NOT NULL,
    source TEXT NOT NULL,
    launch_target TEXT NOT NULL,
    resolved_target TEXT,
    arguments_json TEXT NOT NULL,
    working_directory TEXT,
    icon_path TEXT,
    publisher TEXT,
    version TEXT,
    category TEXT NOT NULL,
    hidden INTEGER NOT NULL DEFAULT 0,
    favorite INTEGER NOT NULL DEFAULT 0,
    pinned INTEGER NOT NULL DEFAULT 0,
    user_modified INTEGER NOT NULL DEFAULT 0,
    discovered INTEGER NOT NULL DEFAULT 0,
    tags_json TEXT NOT NULL,
    launch_count INTEGER NOT NULL DEFAULT 0,
    last_launched_at TEXT,
    missing INTEGER NOT NULL DEFAULT 0,
    identity_key TEXT NOT NULL UNIQUE,
    notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    last_seen_at TEXT
);
CREATE TABLE IF NOT EXISTS collections (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    built_in INTEGER NOT NULL DEFAULT 0,
    position INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS collection_items (
    collection_id TEXT NOT NULL,
    app_id TEXT NOT NULL,
    position INTEGER NOT NULL,
    PRIMARY KEY (collection_id, app_id)
);
CREATE TABLE IF NOT EXISTS launch_events (
    id TEXT PRIMARY KEY,
    app_id TEXT,
    workflow_id TEXT,
    outcome TEXT NOT NULL,
    message TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS workflows (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    template_key TEXT UNIQUE,
    template_version INTEGER,
    customized INTEGER NOT NULL DEFAULT 0,
    definition_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS workflow_runs (
    id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    status TEXT NOT NULL,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    summary TEXT NOT NULL,
    log_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS preview_assets (
    id TEXT PRIMARY KEY,
    app_id TEXT NOT NULL,
    path TEXT NOT NULL,
    source TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS indexed_files (
    path TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    parent TEXT NOT NULL,
    size_bytes INTEGER
);
"#;

pub fn open_database(path: &Path) -> AppResult<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    conn.busy_timeout(std::time::Duration::from_secs(3))?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    let _ = conn.pragma_update(None, "journal_mode", "WAL");
    migrate(&conn)?;
    seed(&conn)?;
    Ok(conn)
}

pub fn migrate(conn: &Connection) -> AppResult<()> {
    conn.execute_batch(MIGRATION)?;
    let version: Option<String> = conn
        .query_row(
            "SELECT value FROM meta WHERE key = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if version.is_none() {
        conn.execute(
            "INSERT INTO meta (key, value) VALUES ('schema_version', ?1)",
            params![SCHEMA_VERSION.to_string()],
        )?;
    }
    Ok(())
}

pub fn seed(conn: &Connection) -> AppResult<()> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM preferences", [], |row| row.get(0))?;
    if count == 0 {
        save_preferences(conn, &UserPreferences::default())?;
    }
    conn.execute(
        "INSERT OR IGNORE INTO collections (id, name, built_in, position, created_at) VALUES (?1, 'ZeroPulse', 1, 0, ?2)",
        params![ZEROPULSE_COLLECTION_ID, now_iso()],
    )?;
    for template in builtin_templates() {
        let existing =
            workflow_by_template(conn, template.template_key.as_deref().unwrap_or_default())?;
        match existing {
            Some(current) if current.customized => {}
            Some(current) if current.template_version.unwrap_or(0) >= TEMPLATE_VERSION => {}
            Some(current) => {
                let mut replacement = template;
                replacement.id = current.id;
                replacement.created_at = current.created_at;
                replacement.document.values = current.document.values;
                save_workflow(conn, &replacement)?;
            }
            None => save_workflow(conn, &template)?,
        }
    }
    Ok(())
}

pub fn save_preferences(conn: &Connection, preferences: &UserPreferences) -> AppResult<()> {
    let clean = sanitize_preferences(preferences.clone())?;
    let json = serde_json::to_string(&clean)?;
    conn.execute(
        "INSERT INTO preferences (id, json) VALUES (1, ?1)
         ON CONFLICT(id) DO UPDATE SET json = excluded.json",
        params![json],
    )?;
    Ok(())
}

pub fn sanitize_preferences(mut preferences: UserPreferences) -> AppResult<UserPreferences> {
    preferences.shortcut = validate_accelerator(&preferences.shortcut)?;
    if !preferences.active_shortcut.is_empty() {
        preferences.active_shortcut = validate_accelerator(&preferences.active_shortcut)?;
    }
    preferences.text_scale = preferences.text_scale.clamp(0.85, 1.5);
    if preferences.ollama_enabled || !preferences.ollama_endpoint.is_empty() {
        preferences.ollama_endpoint = validate_http_endpoint(&preferences.ollama_endpoint)?;
    }
    if !preferences.pulseflow_endpoint.trim().is_empty() {
        preferences.pulseflow_endpoint = validate_http_endpoint(&preferences.pulseflow_endpoint)?;
    }
    preferences.index_roots = preferences
        .index_roots
        .iter()
        .map(|root| {
            validate_index_root(Path::new(root)).map(|path| path.to_string_lossy().to_string())
        })
        .collect::<AppResult<Vec<_>>>()?;
    preferences.index_extensions.retain(|ext| {
        let ext = ext.trim().trim_start_matches('.');
        !ext.is_empty() && ext.len() <= 10 && ext.chars().all(|ch| ch.is_ascii_alphanumeric())
    });
    preferences.index_extensions.truncate(20);
    preferences.shortcut_warning = preferences.shortcut_warning.chars().take(500).collect();
    Ok(preferences)
}

pub fn load_preferences(conn: &Connection) -> AppResult<UserPreferences> {
    let json: String = conn.query_row("SELECT json FROM preferences WHERE id = 1", [], |row| {
        row.get(0)
    })?;
    Ok(serde_json::from_str(&json)?)
}

pub fn list_apps(conn: &Connection, include_hidden: bool) -> AppResult<Vec<AppEntry>> {
    let sql = if include_hidden {
        "SELECT * FROM apps ORDER BY display_name COLLATE NOCASE, id"
    } else {
        "SELECT * FROM apps WHERE hidden = 0 ORDER BY display_name COLLATE NOCASE, id"
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], map_app)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
}

pub fn get_app(conn: &Connection, id: &str) -> AppResult<AppEntry> {
    let mut app = conn
        .query_row("SELECT * FROM apps WHERE id = ?1", params![id], map_app)
        .map_err(|_| AppError::new("NOT_FOUND", "App was not found."))?;
    let path = app
        .resolved_target
        .clone()
        .unwrap_or_else(|| app.launch_target.clone());
    app.size_bytes = file_size(Path::new(&path));
    app.missing = app.missing || path_missing(&path) && !is_uri_target(&path);
    Ok(app)
}

pub fn apply_scan(
    conn: &Connection,
    apps: &[DiscoveredApp],
    errors: &[String],
) -> AppResult<ScanReport> {
    let tx = conn.unchecked_transaction()?;
    let mut added = 0;
    let mut updated = 0;
    let mut seen = HashSet::new();
    for app in apps {
        seen.insert(app.identity_key.clone());
        if let Some(existing) = find_by_identity(&tx, &app.identity_key)? {
            if existing.user_modified {
                tx.execute(
                    "UPDATE apps SET discovered = 1, last_seen_at = ?2, missing = ?3, updated_at = ?2 WHERE id = ?1",
                    params![existing.id, now_iso(), bool_int(path_missing(&existing.launch_target) && !is_uri_target(&existing.launch_target))],
                )?;
            } else {
                tx.execute(
                    "UPDATE apps SET display_name = ?2, kind = ?3, source = ?4, launch_target = ?5,
                     resolved_target = ?6, arguments_json = ?7, working_directory = ?8, icon_path = ?9,
                     publisher = ?10, version = ?11, category = ?12, missing = ?13, discovered = 1,
                     last_seen_at = ?14, updated_at = ?14 WHERE id = ?1",
                    params![
                        existing.id,
                        app.display_name,
                        kind_str(app.kind),
                        source_str(app.source),
                        app.launch_target,
                        app.resolved_target,
                        serde_json::to_string(&app.arguments)?,
                        app.working_directory,
                        app.icon_path,
                        app.publisher,
                        app.version,
                        app.category,
                        bool_int(app.missing),
                        now_iso(),
                    ],
                )?;
            }
            updated += 1;
        } else {
            let id = uuid::Uuid::new_v4().to_string();
            insert_discovered(&tx, &id, app)?;
            if looks_like_zeropulse(app) && !app.missing {
                add_to_collection(&tx, ZEROPULSE_COLLECTION_ID, &id)?;
            }
            added += 1;
        }
    }
    let mut marked_missing = 0;
    let discovered_rows = {
        let mut stmt = tx.prepare("SELECT id, identity_key FROM apps WHERE discovered = 1")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    for (id, identity) in discovered_rows {
        if !seen.contains(&identity) {
            tx.execute(
                "UPDATE apps SET missing = 1, updated_at = ?2 WHERE id = ?1",
                params![id, now_iso()],
            )?;
            marked_missing += 1;
        }
    }
    let scanned_at = now_iso();
    set_meta(&tx, "last_scan_at", &scanned_at)?;
    set_meta(
        &tx,
        "last_scan_error",
        &errors
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n"),
    )?;
    tx.commit()?;
    Ok(ScanReport {
        added,
        updated,
        marked_missing,
        errors: errors.to_vec(),
        scanned_at,
    })
}

pub fn register_app(conn: &Connection, request: RegisterRequest) -> AppResult<AppEntry> {
    let name = bounded_text(&request.display_name, 120, "Display name")?;
    validate_arguments(&request.arguments)?;
    let (launch_target, resolved, kind, missing) = normalize_registration(&request.target)?;
    let identity = identity_key(resolved.as_deref().unwrap_or(&launch_target));
    let now = now_iso();
    if let Some(existing) = find_by_identity(conn, &identity)? {
        conn.execute(
            "UPDATE apps SET display_name = ?2, launch_target = ?3, resolved_target = ?4, arguments_json = ?5,
             working_directory = ?6, tags_json = ?7, notes = ?8, user_modified = 1, missing = ?9,
             hidden = 0, updated_at = ?10 WHERE id = ?1",
            params![
                existing.id,
                name,
                launch_target,
                resolved,
                serde_json::to_string(&request.arguments)?,
                request.working_directory,
                serde_json::to_string(&request.tags)?,
                request.notes,
                bool_int(missing),
                now,
            ],
        )?;
        if let Some(collection) = request.collection_id {
            add_to_collection(conn, &collection, &existing.id)?;
        }
        return get_app(conn, &existing.id);
    }
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO apps (
            id, display_name, kind, source, launch_target, resolved_target, arguments_json,
            working_directory, icon_path, publisher, version, category, hidden, favorite, pinned,
            user_modified, discovered, tags_json, launch_count, last_launched_at, missing,
            identity_key, notes, created_at, updated_at, last_seen_at
        ) VALUES (?1,?2,?3,'manual',?4,?5,?6,?7,NULL,NULL,NULL,'Other',0,0,0,1,0,?8,0,NULL,?9,?10,?11,?12,?12,?12)",
        params![
            id,
            name,
            kind_str(kind),
            launch_target,
            resolved,
            serde_json::to_string(&request.arguments)?,
            request.working_directory,
            serde_json::to_string(&request.tags)?,
            bool_int(missing),
            identity,
            request.notes,
            now,
        ],
    )?;
    if let Some(collection) = request.collection_id {
        add_to_collection(conn, &collection, &id)?;
    }
    get_app(conn, &id)
}

pub fn update_app(conn: &Connection, request: UpdateAppRequest) -> AppResult<AppEntry> {
    let current = get_app(conn, &request.id)?;
    let name = bounded_text(&request.display_name, 120, "Display name")?;
    validate_arguments(&request.arguments)?;
    let (launch_target, resolved, _kind, missing) = normalize_registration(&request.target)?;
    let identity = identity_key(resolved.as_deref().unwrap_or(&launch_target));
    if let Some(other) = find_by_identity(conn, &identity)? {
        if other.id != current.id {
            return Err(AppError::new(
                "DUPLICATE",
                "Another entry already uses that target. Edit that entry instead.",
            ));
        }
    }
    conn.execute(
        "UPDATE apps SET display_name = ?2, launch_target = ?3, resolved_target = ?4, arguments_json = ?5,
         working_directory = ?6, tags_json = ?7, category = ?8, notes = ?9, user_modified = 1,
         missing = ?10, identity_key = ?11, updated_at = ?12 WHERE id = ?1",
        params![
            current.id,
            name,
            launch_target,
            resolved,
            serde_json::to_string(&request.arguments)?,
            request.working_directory,
            serde_json::to_string(&request.tags)?,
            bounded_text(&request.category, 40, "Category")?,
            request.notes,
            bool_int(missing),
            identity,
            now_iso(),
        ],
    )?;
    get_app(conn, &current.id)
}

pub fn set_hidden(conn: &Connection, id: &str, hidden: bool) -> AppResult<()> {
    let changed = conn.execute(
        "UPDATE apps SET hidden = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, bool_int(hidden), now_iso()],
    )?;
    if changed == 0 {
        return Err(AppError::new("NOT_FOUND", "App was not found."));
    }
    Ok(())
}

pub fn delete_manual(conn: &Connection, id: &str) -> AppResult<()> {
    let app = get_app(conn, id)?;
    if app.source != SourceKind::Manual || app.discovered {
        return Err(AppError::new(
            "HIDE_ONLY",
            "Discovered apps stay installed. Hide them from PulseLauncher instead of removing the software.",
        ));
    }
    conn.execute(
        "DELETE FROM collection_items WHERE app_id = ?1",
        params![id],
    )?;
    conn.execute("DELETE FROM preview_assets WHERE app_id = ?1", params![id])?;
    conn.execute("DELETE FROM apps WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn set_favorite(conn: &Connection, id: &str, favorite: bool) -> AppResult<()> {
    set_bool(conn, id, "favorite", favorite)
}

pub fn set_pinned(conn: &Connection, id: &str, pinned: bool) -> AppResult<()> {
    set_bool(conn, id, "pinned", pinned)
}

pub fn record_launch(
    conn: &Connection,
    app_id: Option<&str>,
    workflow_id: Option<&str>,
    outcome: &str,
    message: &str,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO launch_events (id, app_id, workflow_id, outcome, message, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![uuid::Uuid::new_v4().to_string(), app_id, workflow_id, outcome, message.chars().take(2000).collect::<String>(), now_iso()],
    )?;
    if outcome == "accepted" {
        if let Some(app_id) = app_id {
            conn.execute(
                "UPDATE apps SET launch_count = launch_count + 1, last_launched_at = ?2 WHERE id = ?1",
                params![app_id, now_iso()],
            )?;
        }
    }
    Ok(())
}

pub fn list_events(conn: &Connection, app_id: Option<&str>) -> AppResult<Vec<LaunchEvent>> {
    let mut stmt = if app_id.is_some() {
        conn.prepare("SELECT id, app_id, workflow_id, outcome, message, created_at FROM launch_events WHERE app_id = ?1 ORDER BY created_at DESC LIMIT 100")?
    } else {
        conn.prepare("SELECT id, app_id, workflow_id, outcome, message, created_at FROM launch_events ORDER BY created_at DESC LIMIT 100")?
    };
    let map = |row: &rusqlite::Row| {
        Ok(LaunchEvent {
            id: row.get(0)?,
            app_id: row.get(1)?,
            workflow_id: row.get(2)?,
            outcome: row.get(3)?,
            message: row.get(4)?,
            created_at: row.get(5)?,
        })
    };
    let rows = if let Some(app_id) = app_id {
        stmt.query_map(params![app_id], map)?
    } else {
        stmt.query_map([], map)?
    };
    rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
}

pub fn list_collections(conn: &Connection) -> AppResult<Vec<Collection>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, built_in, position, created_at FROM collections ORDER BY position, name",
    )?;
    let collections = stmt
        .query_map([], |row| {
            Ok(Collection {
                id: row.get(0)?,
                name: row.get(1)?,
                built_in: row.get::<_, i64>(2)? != 0,
                position: row.get(3)?,
                created_at: row.get(4)?,
                app_ids: Vec::new(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut full = Vec::new();
    for mut collection in collections {
        let mut items = conn.prepare("SELECT app_id FROM collection_items WHERE collection_id = ?1 ORDER BY position, app_id")?;
        collection.app_ids = items
            .query_map(params![collection.id], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        full.push(collection);
    }
    Ok(full)
}

pub fn create_collection(conn: &Connection, name: &str) -> AppResult<Collection> {
    let name = bounded_text(name, 80, "Collection name")?;
    let id = format!("col-{}", uuid::Uuid::new_v4());
    let now = now_iso();
    conn.execute(
        "INSERT INTO collections (id, name, built_in, position, created_at) VALUES (?1, ?2, 0, ?3, ?4)",
        params![id, name, now_millis(), now],
    )?;
    list_collections(conn)?
        .into_iter()
        .find(|collection| collection.id == id)
        .ok_or_else(|| AppError::new("INTERNAL", "Collection was not saved."))
}

pub fn rename_collection(conn: &Connection, id: &str, name: &str) -> AppResult<()> {
    let name = bounded_text(name, 80, "Collection name")?;
    let changed = conn.execute(
        "UPDATE collections SET name = ?2 WHERE id = ?1",
        params![id, name],
    )?;
    if changed == 0 {
        return Err(AppError::new("NOT_FOUND", "Collection was not found."));
    }
    Ok(())
}

pub fn delete_collection(conn: &Connection, id: &str) -> AppResult<()> {
    let built_in: i64 = conn
        .query_row(
            "SELECT built_in FROM collections WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .map_err(|_| AppError::new("NOT_FOUND", "Collection was not found."))?;
    if built_in != 0 {
        return Err(AppError::new(
            "BUILT_IN",
            "The ZeroPulse collection can be cleared, but it is recreated so setup stays available.",
        ));
    }
    conn.execute(
        "DELETE FROM collection_items WHERE collection_id = ?1",
        params![id],
    )?;
    conn.execute("DELETE FROM collections WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn set_collection_members(conn: &Connection, id: &str, app_ids: &[String]) -> AppResult<()> {
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM collections WHERE id = ?1",
        params![id],
        |row| row.get(0),
    )?;
    if exists == 0 {
        return Err(AppError::new("NOT_FOUND", "Collection was not found."));
    }
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "DELETE FROM collection_items WHERE collection_id = ?1",
        params![id],
    )?;
    for (position, app_id) in app_ids.iter().enumerate() {
        let found: i64 = tx.query_row(
            "SELECT COUNT(*) FROM apps WHERE id = ?1",
            params![app_id],
            |row| row.get(0),
        )?;
        if found == 1 {
            tx.execute(
                "INSERT INTO collection_items (collection_id, app_id, position) VALUES (?1, ?2, ?3)",
                params![id, app_id, position as i64],
            )?;
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn save_workflow(conn: &Connection, record: &WorkflowRecord) -> AppResult<()> {
    validate_document(&record.document)?;
    let name = bounded_text(&record.name, 120, "Workflow name")?;
    let json = serde_json::to_string(&record.document)?;
    conn.execute(
        "INSERT INTO workflows (id, name, description, template_key, template_version, customized, definition_json, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            description = excluded.description,
            template_key = excluded.template_key,
            template_version = excluded.template_version,
            customized = excluded.customized,
            definition_json = excluded.definition_json,
            updated_at = excluded.updated_at",
        params![
            record.id,
            name,
            record.description,
            record.template_key,
            record.template_version,
            bool_int(record.customized),
            json,
            record.created_at,
            record.updated_at,
        ],
    )?;
    Ok(())
}

pub fn list_workflows(conn: &Connection) -> AppResult<Vec<WorkflowRecord>> {
    let mut stmt = conn.prepare("SELECT id, name, description, template_key, template_version, customized, definition_json, created_at, updated_at FROM workflows ORDER BY name")?;
    let rows = stmt.query_map([], map_workflow)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
}

pub fn get_workflow(conn: &Connection, id: &str) -> AppResult<WorkflowRecord> {
    conn.query_row(
        "SELECT id, name, description, template_key, template_version, customized, definition_json, created_at, updated_at FROM workflows WHERE id = ?1",
        params![id],
        map_workflow,
    )
    .map_err(|_| AppError::new("NOT_FOUND", "Workflow was not found."))
}

pub fn reset_template(conn: &Connection, id: &str) -> AppResult<WorkflowRecord> {
    let current = get_workflow(conn, id)?;
    let key = current
        .template_key
        .clone()
        .ok_or_else(|| AppError::new("WORKFLOW", "Only built-in templates can be reset."))?;
    let template = builtin_templates()
        .into_iter()
        .find(|item| item.template_key.as_deref() == Some(key.as_str()))
        .ok_or_else(|| AppError::new("WORKFLOW", "Template seed was not found."))?;
    let mut replacement = template;
    replacement.id = current.id;
    replacement.created_at = current.created_at;
    replacement.updated_at = now_iso();
    replacement.customized = false;
    save_workflow(conn, &replacement)?;
    get_workflow(conn, id)
}

pub fn save_run(conn: &Connection, run: &WorkflowRun) -> AppResult<()> {
    conn.execute(
        "INSERT INTO workflow_runs (id, workflow_id, status, started_at, finished_at, summary, log_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(id) DO UPDATE SET status = excluded.status, finished_at = excluded.finished_at, summary = excluded.summary, log_json = excluded.log_json",
        params![
            run.id,
            run.workflow_id,
            run.status,
            run.started_at,
            run.finished_at,
            run.summary,
            serde_json::to_string(&run.steps)?,
        ],
    )?;
    Ok(())
}

pub fn latest_run(conn: &Connection, workflow_id: &str) -> AppResult<Option<WorkflowRun>> {
    conn.query_row(
        "SELECT id, workflow_id, status, started_at, finished_at, summary, log_json FROM workflow_runs WHERE workflow_id = ?1 ORDER BY started_at DESC LIMIT 1",
        params![workflow_id],
        map_run,
    )
    .optional()
    .map_err(AppError::from)
}

pub fn list_runs(conn: &Connection, workflow_id: &str) -> AppResult<Vec<WorkflowRun>> {
    let mut stmt = conn.prepare("SELECT id, workflow_id, status, started_at, finished_at, summary, log_json FROM workflow_runs WHERE workflow_id = ?1 ORDER BY started_at DESC LIMIT 30")?;
    let rows = stmt.query_map(params![workflow_id], map_run)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
}

pub fn replace_index(conn: &Connection, files: &[crate::models::IndexedFile]) -> AppResult<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM indexed_files", [])?;
    for file in files {
        tx.execute(
            "INSERT INTO indexed_files (path, name, parent, size_bytes) VALUES (?1, ?2, ?3, ?4)",
            params![file.path, file.name, file.parent, file.size_bytes],
        )?;
    }
    set_meta(&tx, "last_index_at", &now_iso())?;
    tx.commit()?;
    Ok(())
}

pub fn list_indexed_files(conn: &Connection) -> AppResult<Vec<crate::models::IndexedFile>> {
    let mut stmt = conn.prepare(
        "SELECT path, name, parent, size_bytes FROM indexed_files ORDER BY name LIMIT 5000",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(crate::models::IndexedFile {
            path: row.get(0)?,
            name: row.get(1)?,
            parent: row.get(2)?,
            size_bytes: row.get(3)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
}

pub fn add_preview(conn: &Connection, asset: &PreviewAsset) -> AppResult<()> {
    conn.execute(
        "INSERT INTO preview_assets (id, app_id, path, source, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![asset.id, asset.app_id, asset.path, asset.source, asset.created_at],
    )?;
    Ok(())
}

pub fn list_previews(conn: &Connection, app_id: &str) -> AppResult<Vec<PreviewAsset>> {
    let mut stmt = conn.prepare("SELECT id, app_id, path, source, created_at FROM preview_assets WHERE app_id = ?1 ORDER BY created_at DESC")?;
    let rows = stmt.query_map(params![app_id], |row| {
        Ok(PreviewAsset {
            id: row.get(0)?,
            app_id: row.get(1)?,
            path: row.get(2)?,
            source: row.get(3)?,
            created_at: row.get(4)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
}

pub fn get_preview(conn: &Connection, id: &str) -> AppResult<PreviewAsset> {
    conn.query_row(
        "SELECT id, app_id, path, source, created_at FROM preview_assets WHERE id = ?1",
        params![id],
        |row| {
            Ok(PreviewAsset {
                id: row.get(0)?,
                app_id: row.get(1)?,
                path: row.get(2)?,
                source: row.get(3)?,
                created_at: row.get(4)?,
            })
        },
    )
    .map_err(|_| AppError::new("NOT_FOUND", "Preview was not found."))
}

pub fn delete_preview(conn: &Connection, id: &str) -> AppResult<()> {
    let path: String = conn
        .query_row(
            "SELECT path FROM preview_assets WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .map_err(|_| AppError::new("NOT_FOUND", "Preview was not found."))?;
    delete_preview_file(Path::new(&path))?;
    conn.execute("DELETE FROM preview_assets WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn clear_history(conn: &Connection) -> AppResult<()> {
    conn.execute("DELETE FROM launch_events", [])?;
    conn.execute("DELETE FROM workflow_runs", [])?;
    conn.execute(
        "UPDATE apps SET launch_count = 0, last_launched_at = NULL",
        [],
    )?;
    Ok(())
}

pub fn export_bundle(conn: &Connection, include_history: bool) -> AppResult<ExportBundle> {
    let apps = list_apps(conn, true)?
        .into_iter()
        .filter(|app| {
            app.source == SourceKind::Manual
                || app.user_modified
                || app.favorite
                || app.pinned
                || app.hidden
                || !app.tags.is_empty()
        })
        .collect();
    Ok(ExportBundle {
        schema_version: SCHEMA_VERSION,
        exported_at: now_iso(),
        preferences: load_preferences(conn)?,
        apps,
        collections: list_collections(conn)?,
        workflows: list_workflows(conn)?,
        launch_events: if include_history {
            list_events(conn, None)?
        } else {
            Vec::new()
        },
    })
}

pub fn preview_import(bytes: &str) -> AppResult<ImportPreview> {
    let bundle = parse_bundle(bytes)?;
    Ok(ImportPreview {
        apps: bundle.apps.len(),
        collections: bundle.collections.len(),
        workflows: bundle.workflows.len(),
        events: bundle.launch_events.len(),
        warnings: import_warnings(&bundle),
    })
}

pub fn apply_import(conn: &Connection, bytes: &str) -> AppResult<ImportPreview> {
    let bundle = parse_bundle(bytes)?;
    let preview = ImportPreview {
        apps: bundle.apps.len(),
        collections: bundle.collections.len(),
        workflows: bundle.workflows.len(),
        events: bundle.launch_events.len(),
        warnings: import_warnings(&bundle),
    };
    let tx = conn.unchecked_transaction()?;
    save_preferences(&tx, &bundle.preferences)?;
    for app in &bundle.apps {
        let mut stored = app.clone();
        stored.missing =
            path_missing(&stored.launch_target) && !is_uri_target(&stored.launch_target);
        stored.launch_count = 0;
        stored.last_launched_at = None;
        upsert_imported_app(&tx, &stored)?;
    }
    for collection in &bundle.collections {
        tx.execute(
            "INSERT INTO collections (id, name, built_in, position, created_at) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name",
            params![collection.id, collection.name, bool_int(collection.built_in), collection.position, collection.created_at],
        )?;
        set_collection_members(&tx, &collection.id, &collection.app_ids)?;
    }
    for workflow in &bundle.workflows {
        save_workflow(&tx, workflow)?;
    }
    tx.commit()?;
    Ok(preview)
}

pub fn diagnostics(
    conn: &Connection,
    version: &str,
    data_dir: &Path,
    db_path: &Path,
) -> AppResult<Diagnostics> {
    let preferences = load_preferences(conn)?;
    let app_count: i64 = conn.query_row("SELECT COUNT(*) FROM apps", [], |row| row.get(0))?;
    let visible: i64 = conn.query_row("SELECT COUNT(*) FROM apps WHERE hidden = 0", [], |row| {
        row.get(0)
    })?;
    let index_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM indexed_files", [], |row| row.get(0))?;
    let mut notes = vec![
        format!("Running on {}.", crate::platform::platform_name()),
        "No telemetry is sent.".into(),
    ];
    if !cfg!(windows) {
        notes.push("This process is not Windows. Start Menu scanning and Task Manager launch run only in a Windows build.".into());
    }
    Ok(Diagnostics {
        version: version.into(),
        os: crate::platform::platform_name().into(),
        data_dir: data_dir.display().to_string(),
        db_path: db_path.display().to_string(),
        app_count,
        visible_app_count: visible,
        last_scan_at: get_meta(conn, "last_scan_at")?,
        last_scan_error: get_meta(conn, "last_scan_error")?,
        index_count,
        last_index_at: get_meta(conn, "last_index_at")?,
        active_shortcut: preferences.active_shortcut,
        shortcut_warning: preferences.shortcut_warning,
        developer_commands_enabled: preferences.developer_commands_enabled,
        notes,
    })
}

pub fn collection_counts(conn: &Connection) -> AppResult<HashMap<String, usize>> {
    let collections = list_collections(conn)?;
    Ok(collections
        .into_iter()
        .map(|collection| (collection.id, collection.app_ids.len()))
        .collect())
}

pub fn apps_by_id(conn: &Connection) -> AppResult<HashMap<String, AppEntry>> {
    Ok(list_apps(conn, true)?
        .into_iter()
        .map(|app| (app.id.clone(), app))
        .collect())
}

pub fn collection_members(conn: &Connection) -> AppResult<HashMap<String, Vec<AppEntry>>> {
    let apps = apps_by_id(conn)?;
    let mut map = HashMap::new();
    for collection in list_collections(conn)? {
        let members = collection
            .app_ids
            .iter()
            .filter_map(|id| apps.get(id).cloned())
            .collect();
        map.insert(collection.id, members);
    }
    Ok(map)
}

fn parse_bundle(bytes: &str) -> AppResult<ExportBundle> {
    if bytes.len() > 2_000_000 {
        return Err(AppError::new("IMPORT", "Import file is larger than 2 MB."));
    }
    let bundle: ExportBundle = serde_json::from_str(bytes).map_err(|err| {
        AppError::new(
            "IMPORT",
            format!("Import file is not a PulseLauncher backup: {err}"),
        )
    })?;
    if bundle.schema_version != SCHEMA_VERSION {
        return Err(AppError::new(
            "IMPORT",
            format!("Unsupported backup schema {}.", bundle.schema_version),
        ));
    }
    if bundle.apps.len() > 2000 || bundle.workflows.len() > 200 {
        return Err(AppError::new("IMPORT", "Import contains too many records."));
    }
    for workflow in &bundle.workflows {
        validate_document(&workflow.document)?;
    }
    let _ = sanitize_preferences(bundle.preferences.clone())?;
    Ok(bundle)
}

fn import_warnings(bundle: &ExportBundle) -> Vec<String> {
    let mut warnings = Vec::new();
    let missing = bundle
        .apps
        .iter()
        .filter(|app| path_missing(&app.launch_target) && !is_uri_target(&app.launch_target))
        .count();
    if missing > 0 {
        warnings.push(format!(
            "{missing} app target(s) are not present on this machine and will be marked missing."
        ));
    }
    if !bundle.launch_events.is_empty() {
        warnings
            .push("Launch history in the file is ignored. Import does not replay history.".into());
    }
    warnings
}

fn upsert_imported_app(conn: &Connection, app: &AppEntry) -> AppResult<()> {
    conn.execute(
        "INSERT INTO apps (
            id, display_name, kind, source, launch_target, resolved_target, arguments_json,
            working_directory, icon_path, publisher, version, category, hidden, favorite, pinned,
            user_modified, discovered, tags_json, launch_count, last_launched_at, missing,
            identity_key, notes, created_at, updated_at, last_seen_at
        ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,0,NULL,?19,?20,?21,?22,?23,?23)
        ON CONFLICT(identity_key) DO UPDATE SET
            display_name = excluded.display_name,
            launch_target = excluded.launch_target,
            resolved_target = excluded.resolved_target,
            arguments_json = excluded.arguments_json,
            working_directory = excluded.working_directory,
            tags_json = excluded.tags_json,
            category = excluded.category,
            hidden = excluded.hidden,
            favorite = excluded.favorite,
            pinned = excluded.pinned,
            notes = excluded.notes,
            missing = excluded.missing,
            user_modified = 1,
            updated_at = excluded.updated_at",
        params![
            app.id,
            app.display_name,
            kind_str(app.kind),
            source_str(app.source),
            app.launch_target,
            app.resolved_target,
            serde_json::to_string(&app.arguments)?,
            app.working_directory,
            app.icon_path,
            app.publisher,
            app.version,
            app.category,
            bool_int(app.hidden),
            bool_int(app.favorite),
            bool_int(app.pinned),
            bool_int(true),
            bool_int(app.discovered),
            serde_json::to_string(&app.tags)?,
            bool_int(app.missing),
            app.identity_key,
            app.notes,
            app.created_at,
            now_iso(),
        ],
    )?;
    Ok(())
}

fn normalize_registration(target: &str) -> AppResult<(String, Option<String>, AppKind, bool)> {
    let target = target.trim();
    if is_uri_target(target) {
        let url = validate_url(target)?;
        return Ok((url.clone(), Some(url), AppKind::Uri, false));
    }
    let path = PathBuf::from(target);
    if is_shell_link(&path) {
        if !path.is_file() {
            return Err(AppError::new(
                "MISSING_TARGET",
                "Shortcut file does not exist.",
            ));
        }
        let parsed = crate::lnk::parse_lnk_file(&path)?;
        let resolved = parsed.path.filter(|item| Path::new(item).is_file());
        let missing = resolved.is_none();
        return Ok((
            path.to_string_lossy().to_string(),
            resolved,
            AppKind::Shortcut,
            missing,
        ));
    }
    let canonical = validate_existing_program(&path)?;
    Ok((
        canonical.to_string_lossy().to_string(),
        Some(canonical.to_string_lossy().to_string()),
        if cfg!(windows) {
            AppKind::Portable
        } else {
            AppKind::Desktop
        },
        false,
    ))
}

fn insert_discovered(conn: &Connection, id: &str, app: &DiscoveredApp) -> AppResult<()> {
    let now = now_iso();
    conn.execute(
        "INSERT INTO apps (
            id, display_name, kind, source, launch_target, resolved_target, arguments_json,
            working_directory, icon_path, publisher, version, category, hidden, favorite, pinned,
            user_modified, discovered, tags_json, launch_count, last_launched_at, missing,
            identity_key, notes, created_at, updated_at, last_seen_at
        ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,0,0,0,0,1,'[]',0,NULL,?13,?14,NULL,?15,?15,?15)",
        params![
            id,
            app.display_name,
            kind_str(app.kind),
            source_str(app.source),
            app.launch_target,
            app.resolved_target,
            serde_json::to_string(&app.arguments)?,
            app.working_directory,
            app.icon_path,
            app.publisher,
            app.version,
            app.category,
            bool_int(app.missing),
            app.identity_key,
            now,
        ],
    )?;
    Ok(())
}

fn add_to_collection(conn: &Connection, collection_id: &str, app_id: &str) -> AppResult<()> {
    let position: i64 = conn.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM collection_items WHERE collection_id = ?1",
        params![collection_id],
        |row| row.get(0),
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO collection_items (collection_id, app_id, position) VALUES (?1, ?2, ?3)",
        params![collection_id, app_id, position],
    )?;
    Ok(())
}

fn find_by_identity(conn: &Connection, identity: &str) -> AppResult<Option<AppEntry>> {
    conn.query_row(
        "SELECT * FROM apps WHERE identity_key = ?1",
        params![identity],
        map_app,
    )
    .optional()
    .map_err(AppError::from)
}

fn workflow_by_template(conn: &Connection, key: &str) -> AppResult<Option<WorkflowRecord>> {
    conn.query_row(
        "SELECT id, name, description, template_key, template_version, customized, definition_json, created_at, updated_at FROM workflows WHERE template_key = ?1",
        params![key],
        map_workflow,
    )
    .optional()
    .map_err(AppError::from)
}

fn set_bool(conn: &Connection, id: &str, column: &str, value: bool) -> AppResult<()> {
    if !matches!(column, "favorite" | "pinned") {
        return Err(AppError::new("INTERNAL", "Unsupported flag."));
    }
    let sql = format!("UPDATE apps SET {column} = ?2, updated_at = ?3 WHERE id = ?1");
    let changed = conn.execute(&sql, params![id, bool_int(value), now_iso()])?;
    if changed == 0 {
        return Err(AppError::new("NOT_FOUND", "App was not found."));
    }
    Ok(())
}

fn set_meta(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

fn get_meta(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    conn.query_row(
        "SELECT value FROM meta WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
    .map_err(AppError::from)
}

fn map_app(row: &rusqlite::Row) -> rusqlite::Result<AppEntry> {
    let arguments: String = row.get(6)?;
    let tags: String = row.get(17)?;
    Ok(AppEntry {
        id: row.get(0)?,
        display_name: row.get(1)?,
        kind: parse_kind(&row.get::<_, String>(2)?),
        source: parse_source(&row.get::<_, String>(3)?),
        launch_target: row.get(4)?,
        resolved_target: row.get(5)?,
        arguments: serde_json::from_str(&arguments).unwrap_or_default(),
        working_directory: row.get(7)?,
        icon_path: row.get(8)?,
        publisher: row.get(9)?,
        version: row.get(10)?,
        category: row.get(11)?,
        hidden: row.get::<_, i64>(12)? != 0,
        favorite: row.get::<_, i64>(13)? != 0,
        pinned: row.get::<_, i64>(14)? != 0,
        user_modified: row.get::<_, i64>(15)? != 0,
        discovered: row.get::<_, i64>(16)? != 0,
        tags: serde_json::from_str(&tags).unwrap_or_default(),
        launch_count: row.get(18)?,
        last_launched_at: row.get(19)?,
        missing: row.get::<_, i64>(20)? != 0,
        identity_key: row.get(21)?,
        notes: row.get(22)?,
        created_at: row.get(23)?,
        updated_at: row.get(24)?,
        size_bytes: None,
    })
}

fn map_workflow(row: &rusqlite::Row) -> rusqlite::Result<WorkflowRecord> {
    let json: String = row.get(6)?;
    let document = serde_json::from_str(&json).unwrap_or(crate::models::WorkflowDocument {
        what_it_does: String::new(),
        prerequisites_help: String::new(),
        parameters: Vec::new(),
        values: Default::default(),
        steps: Vec::new(),
    });
    Ok(WorkflowRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        template_key: row.get(3)?,
        template_version: row.get(4)?,
        customized: row.get::<_, i64>(5)? != 0,
        document,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn map_run(row: &rusqlite::Row) -> rusqlite::Result<WorkflowRun> {
    let json: String = row.get(6)?;
    Ok(WorkflowRun {
        id: row.get(0)?,
        workflow_id: row.get(1)?,
        status: row.get(2)?,
        started_at: row.get(3)?,
        finished_at: row.get(4)?,
        summary: row.get(5)?,
        steps: serde_json::from_str(&json).unwrap_or_default(),
    })
}

fn parse_kind(value: &str) -> AppKind {
    match value {
        "win32" => AppKind::Win32,
        "portable" => AppKind::Portable,
        "packaged" => AppKind::Packaged,
        "uri" => AppKind::Uri,
        "shortcut" => AppKind::Shortcut,
        _ => AppKind::Desktop,
    }
}

fn parse_source(value: &str) -> SourceKind {
    match value {
        "startMenuUser" => SourceKind::StartMenuUser,
        "startMenuCommon" => SourceKind::StartMenuCommon,
        "desktopShortcuts" => SourceKind::DesktopShortcuts,
        "manual" => SourceKind::Manual,
        "packaged" => SourceKind::Packaged,
        "uri" => SourceKind::Uri,
        _ => SourceKind::DesktopFile,
    }
}

fn kind_str(kind: AppKind) -> &'static str {
    match kind {
        AppKind::Win32 => "win32",
        AppKind::Portable => "portable",
        AppKind::Packaged => "packaged",
        AppKind::Uri => "uri",
        AppKind::Desktop => "desktop",
        AppKind::Shortcut => "shortcut",
    }
}

fn source_str(source: SourceKind) -> &'static str {
    match source {
        SourceKind::StartMenuUser => "startMenuUser",
        SourceKind::StartMenuCommon => "startMenuCommon",
        SourceKind::DesktopShortcuts => "desktopShortcuts",
        SourceKind::Manual => "manual",
        SourceKind::DesktopFile => "desktopFile",
        SourceKind::Packaged => "packaged",
        SourceKind::Uri => "uri",
    }
}

fn bool_int(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

fn now_millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::AppKind;

    fn memory() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATION).unwrap();
        seed(&conn).unwrap();
        conn
    }

    #[test]
    fn host_desktop_entries_can_be_stored() {
        let dir = std::path::PathBuf::from("/usr/share/applications");
        if !dir.is_dir() {
            return;
        }
        let batch = crate::discovery::discover_desktop_dirs(&[dir]).unwrap();
        let conn = memory();
        let report = apply_scan(&conn, &batch.apps, &batch.errors).unwrap();
        assert!(report.added > 0, "{report:?}");
        assert!(!list_apps(&conn, false).unwrap().is_empty());
    }

    #[test]
    fn seed_is_idempotent_and_history_clears() {
        let conn = memory();
        seed(&conn).unwrap();
        let workflows = list_workflows(&conn).unwrap();
        assert_eq!(workflows.len(), 12);
        let dir = tempfile::tempdir().unwrap();
        let binary = dir.path().join("tool");
        std::fs::write(&binary, b"bin").unwrap();
        let app = register_app(
            &conn,
            RegisterRequest {
                display_name: "Tool".into(),
                target: binary.display().to_string(),
                arguments: vec!["--safe".into()],
                working_directory: None,
                tags: vec!["dev".into()],
                collection_id: Some(ZEROPULSE_COLLECTION_ID.into()),
                notes: None,
            },
        )
        .unwrap();
        set_favorite(&conn, &app.id, true).unwrap();
        record_launch(&conn, Some(&app.id), None, "accepted", "started").unwrap();
        clear_history(&conn).unwrap();
        let reloaded = get_app(&conn, &app.id).unwrap();
        assert!(reloaded.favorite);
        assert_eq!(reloaded.launch_count, 0);
        assert!(list_events(&conn, None).unwrap().is_empty());
        assert!(register_app(
            &conn,
            RegisterRequest {
                display_name: "Missing".into(),
                target: dir.path().join("nope").display().to_string(),
                arguments: Vec::new(),
                working_directory: None,
                tags: Vec::new(),
                collection_id: None,
                notes: None,
            },
        )
        .is_err());
        let _ = AppKind::Portable;
    }

    #[test]
    fn favorites_survive_rescan() {
        let conn = memory();
        let dir = tempfile::tempdir().unwrap();
        let binary = dir.path().join("app");
        std::fs::write(&binary, b"bin").unwrap();
        let discovered = DiscoveredApp {
            display_name: "Sample".into(),
            kind: AppKind::Desktop,
            source: SourceKind::DesktopFile,
            launch_target: binary.display().to_string(),
            resolved_target: Some(binary.display().to_string()),
            arguments: Vec::new(),
            working_directory: None,
            icon_path: None,
            publisher: None,
            version: None,
            category: "Other".into(),
            missing: false,
            identity_key: identity_key(&binary.display().to_string()),
        };
        apply_scan(&conn, std::slice::from_ref(&discovered), &[]).unwrap();
        let app = list_apps(&conn, true)
            .unwrap()
            .into_iter()
            .find(|item| item.display_name == "Sample")
            .unwrap();
        set_favorite(&conn, &app.id, true).unwrap();
        set_hidden(&conn, &app.id, true).unwrap();
        let mut renamed = discovered.clone();
        renamed.display_name = "Sample Renamed".into();
        apply_scan(&conn, &[renamed], &[]).unwrap();
        let app = get_app(&conn, &app.id).unwrap();
        assert!(app.favorite);
        assert!(app.hidden);
        assert_eq!(app.display_name, "Sample Renamed");
        apply_scan(&conn, &[], &[]).unwrap();
        let app = get_app(&conn, &app.id).unwrap();
        assert!(app.missing);
    }

    #[test]
    fn import_rejects_bad_workflow_urls() {
        let err = preview_import(
            r#"{"schemaVersion":1,"exportedAt":"t","preferences":{},"apps":[],"collections":[],"workflows":[],"launchEvents":[]}"#,
        );
        assert!(err.is_err());
    }
}
