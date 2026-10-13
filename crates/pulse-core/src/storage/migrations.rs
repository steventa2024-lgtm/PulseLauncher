//! Versioned, atomic schema migrations driven by `PRAGMA user_version`.

use rusqlite::Connection;

pub const MIGRATIONS: &[&str] = &[
    // v1
    r#"
    CREATE TABLE apps (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        user_renamed INTEGER NOT NULL DEFAULT 0,
        source TEXT NOT NULL,
        kind TEXT NOT NULL,
        path TEXT NOT NULL DEFAULT '',
        args TEXT NOT NULL DEFAULT '[]',
        working_dir TEXT,
        aumid TEXT,
        uri TEXT,
        resolved_path TEXT,
        identity_key TEXT NOT NULL,
        provenance TEXT NOT NULL DEFAULT '',
        publisher TEXT,
        version TEXT,
        category TEXT NOT NULL DEFAULT 'none',
        tags TEXT NOT NULL DEFAULT '[]',
        favorite INTEGER NOT NULL DEFAULT 0,
        hidden INTEGER NOT NULL DEFAULT 0,
        stale INTEGER NOT NULL DEFAULT 0,
        sort_order INTEGER NOT NULL DEFAULT 0,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );
    CREATE INDEX idx_apps_identity ON apps(identity_key);

    CREATE TABLE collections (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        system_key TEXT UNIQUE,
        sort_order INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE app_collections (
        app_id TEXT NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
        collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
        position INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (app_id, collection_id)
    );

    CREATE TABLE launch_events (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        app_id TEXT,
        app_name TEXT NOT NULL,
        at TEXT NOT NULL,
        outcome TEXT NOT NULL,
        pid INTEGER,
        detail TEXT,
        args_display TEXT
    );
    CREATE INDEX idx_launch_app ON launch_events(app_id, at);

    CREATE TABLE workflows (
        id TEXT PRIMARY KEY,
        kind TEXT NOT NULL,
        name TEXT NOT NULL,
        description TEXT NOT NULL DEFAULT '',
        template_id TEXT UNIQUE,
        template_version INTEGER,
        definition TEXT NOT NULL,
        enabled INTEGER NOT NULL DEFAULT 1,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );
    CREATE TABLE workflow_runs (
        id TEXT PRIMARY KEY,
        workflow_id TEXT NOT NULL,
        workflow_name TEXT NOT NULL,
        started_at TEXT NOT NULL,
        finished_at TEXT,
        status TEXT NOT NULL,
        results TEXT NOT NULL DEFAULT '[]'
    );
    CREATE INDEX idx_runs_wf ON workflow_runs(workflow_id, started_at);

    CREATE TABLE preview_assets (
        id TEXT PRIMARY KEY,
        app_id TEXT NOT NULL REFERENCES apps(id) ON DELETE CASCADE,
        file_name TEXT NOT NULL,
        caption TEXT,
        source TEXT NOT NULL,
        created_at TEXT NOT NULL
    );

    CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
    CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
    "#,
    // v2: opt-in file index
    r#"
    CREATE TABLE indexed_files (
        path TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        is_dir INTEGER NOT NULL,
        root TEXT NOT NULL
    );
    CREATE INDEX idx_files_root ON indexed_files(root);
    "#,
];

pub fn migrate(conn: &mut Connection) -> rusqlite::Result<u32> {
    migrate_with(conn, MIGRATIONS)
}

pub fn migrate_with(conn: &mut Connection, migrations: &[&str]) -> rusqlite::Result<u32> {
    let current: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in migrations.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.execute_batch(&format!("PRAGMA user_version = {}", i + 1))?;
        tx.commit()?;
    }
    conn.query_row("PRAGMA user_version", [], |r| r.get(0))
}
