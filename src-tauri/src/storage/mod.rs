//! SQLite persistence. A single per-user database file holds every view's
//! data. Migrations are embedded, versioned via `PRAGMA user_version` and
//! applied atomically.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::error::{AppError, AppResult};
use crate::models::*;

pub mod apps;
pub mod workflows;

const MIGRATIONS: &[(i64, &str)] = &[(1, include_str!("../../migrations/0001_init.sql"))];

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: &Path) -> AppResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> AppResult<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> AppResult<Self> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.migrate()?;
        Ok(db)
    }

    pub fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn schema_version(&self) -> AppResult<i64> {
        Ok(self
            .conn()
            .query_row("PRAGMA user_version", [], |r| r.get(0))?)
    }

    fn migrate(&self) -> AppResult<()> {
        let mut conn = self.conn();
        let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        for (version, sql) in MIGRATIONS {
            if *version <= current {
                continue;
            }
            let tx = conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", version)?;
            tx.commit()?;
        }
        Ok(())
    }

    // ---------------------------------------------------------------- prefs

    pub fn preferences(&self) -> AppResult<UserPreferences> {
        let raw: Option<String> = self
            .conn()
            .query_row("SELECT value FROM preferences WHERE id = 1", [], |r| {
                r.get(0)
            })
            .optional()?;
        Ok(match raw {
            Some(s) => serde_json::from_str(&s).unwrap_or_default(),
            None => UserPreferences::default(),
        })
    }

    pub fn save_preferences(&self, prefs: &UserPreferences) -> AppResult<()> {
        let json = serde_json::to_string(prefs)?;
        self.conn().execute(
            "INSERT INTO preferences (id, value) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET value = excluded.value",
            params![json],
        )?;
        Ok(())
    }

    // --------------------------------------------------------- collections

    pub fn list_collections(&self) -> AppResult<Vec<Collection>> {
        let conn = self.conn();
        let mut items: HashMap<String, Vec<String>> = HashMap::new();
        let mut stmt = conn.prepare(
            "SELECT collection_id, app_id FROM collection_items ORDER BY sort_order, rowid",
        )?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            let (c, a) = row?;
            items.entry(c).or_default().push(a);
        }
        let mut stmt = conn.prepare(
            "SELECT id, name, system_key, sort_order FROM collections ORDER BY sort_order, name",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Collection {
                id: r.get(0)?,
                name: r.get(1)?,
                system_key: r.get(2)?,
                sort_order: r.get(3)?,
                app_ids: vec![],
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            let mut c = row?;
            c.app_ids = items.remove(&c.id).unwrap_or_default();
            out.push(c);
        }
        Ok(out)
    }

    pub fn create_collection(&self, name: &str, system_key: Option<&str>) -> AppResult<Collection> {
        let name = crate::security::bounded_text(name, 80, "Collection name")?;
        let id = uuid::Uuid::new_v4().to_string();
        let conn = self.conn();
        let order: i64 = conn.query_row(
            "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM collections",
            [],
            |r| r.get(0),
        )?;
        conn.execute(
            "INSERT INTO collections (id, name, system_key, sort_order, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, name, system_key, order, now()],
        )?;
        Ok(Collection {
            id,
            name,
            system_key: system_key.map(str::to_owned),
            sort_order: order,
            app_ids: vec![],
        })
    }

    pub fn ensure_system_collection(&self, key: &str, name: &str) -> AppResult<()> {
        let exists: Option<String> = self
            .conn()
            .query_row(
                "SELECT id FROM collections WHERE system_key = ?1",
                params![key],
                |r| r.get(0),
            )
            .optional()?;
        if exists.is_none() {
            self.create_collection(name, Some(key))?;
        }
        Ok(())
    }

    pub fn rename_collection(&self, id: &str, name: &str) -> AppResult<()> {
        let name = crate::security::bounded_text(name, 80, "Collection name")?;
        let n = self.conn().execute(
            "UPDATE collections SET name = ?2 WHERE id = ?1",
            params![id, name],
        )?;
        if n == 0 {
            return Err(AppError::not_found("Collection not found"));
        }
        Ok(())
    }

    pub fn delete_collection(&self, id: &str) -> AppResult<()> {
        let n = self
            .conn()
            .execute("DELETE FROM collections WHERE id = ?1", params![id])?;
        if n == 0 {
            return Err(AppError::not_found("Collection not found"));
        }
        Ok(())
    }

    pub fn set_collection_membership(
        &self,
        collection_id: &str,
        app_id: &str,
        member: bool,
    ) -> AppResult<()> {
        let conn = self.conn();
        if member {
            let order: i64 = conn.query_row(
                "SELECT COALESCE(MAX(sort_order), 0) + 1 FROM collection_items WHERE collection_id = ?1",
                params![collection_id],
                |r| r.get(0),
            )?;
            conn.execute(
                "INSERT OR IGNORE INTO collection_items (collection_id, app_id, sort_order) VALUES (?1, ?2, ?3)",
                params![collection_id, app_id, order],
            )?;
        } else {
            conn.execute(
                "DELETE FROM collection_items WHERE collection_id = ?1 AND app_id = ?2",
                params![collection_id, app_id],
            )?;
        }
        Ok(())
    }

    pub fn reorder_collection(&self, collection_id: &str, app_ids: &[String]) -> AppResult<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        for (i, app_id) in app_ids.iter().enumerate() {
            tx.execute(
                "UPDATE collection_items SET sort_order = ?3 WHERE collection_id = ?1 AND app_id = ?2",
                params![collection_id, app_id, i as i64],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    // -------------------------------------------------------------- history

    pub fn record_launch(
        &self,
        app_id: Option<&str>,
        workflow_id: Option<&str>,
        label: &str,
        success: bool,
        process_id: Option<u32>,
        message: &str,
    ) -> AppResult<()> {
        self.conn().execute(
            "INSERT INTO launch_events (app_id, workflow_id, label, at, success, process_id, message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                app_id,
                workflow_id,
                label,
                now(),
                success,
                process_id,
                crate::security::redact(message)
            ],
        )?;
        Ok(())
    }

    pub fn launch_events(&self, app_id: Option<&str>, limit: usize) -> AppResult<Vec<LaunchEvent>> {
        let conn = self.conn();
        let map = |r: &Row| {
            Ok(LaunchEvent {
                id: r.get(0)?,
                app_id: r.get(1)?,
                workflow_id: r.get(2)?,
                label: r.get(3)?,
                at: r.get(4)?,
                success: r.get(5)?,
                process_id: r.get(6)?,
                message: r.get(7)?,
            })
        };
        let cols = "id, app_id, workflow_id, label, at, success, process_id, message";
        let limit = limit.min(1000) as i64;
        let rows = if let Some(app) = app_id {
            let mut s = conn.prepare(&format!(
                "SELECT {cols} FROM launch_events WHERE app_id = ?1 ORDER BY id DESC LIMIT ?2"
            ))?;
            let v = s
                .query_map(params![app, limit], map)?
                .collect::<Result<Vec<_>, _>>()?;
            v
        } else {
            let mut s = conn.prepare(&format!(
                "SELECT {cols} FROM launch_events ORDER BY id DESC LIMIT ?1"
            ))?;
            let v = s
                .query_map(params![limit], map)?
                .collect::<Result<Vec<_>, _>>()?;
            v
        };
        Ok(rows)
    }

    /// Clears launch history, workflow run logs and the file index.
    pub fn clear_history(&self) -> AppResult<()> {
        let conn = self.conn();
        conn.execute_batch(
            "DELETE FROM launch_events; DELETE FROM workflow_runs; DELETE FROM file_index;",
        )?;
        Ok(())
    }

    // ----------------------------------------------------------- file index

    pub fn replace_file_index(&self, entries: &[(String, String, bool, String)]) -> AppResult<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM file_index", [])?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR IGNORE INTO file_index (path, name, is_dir, root) VALUES (?1, ?2, ?3, ?4)",
            )?;
            for (path, name, is_dir, root) in entries {
                stmt.execute(params![path, name, is_dir, root])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn file_index_entries(&self) -> AppResult<Vec<(String, String, bool)>> {
        let conn = self.conn();
        let mut s = conn.prepare("SELECT path, name, is_dir FROM file_index")?;
        let v = s
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(v)
    }

    pub fn file_index_count(&self) -> AppResult<i64> {
        Ok(self
            .conn()
            .query_row("SELECT COUNT(*) FROM file_index", [], |r| r.get(0))?)
    }

    pub fn is_indexed_path(&self, path: &str) -> AppResult<bool> {
        Ok(self
            .conn()
            .query_row(
                "SELECT 1 FROM file_index WHERE path = ?1",
                params![path],
                |_| Ok(()),
            )
            .optional()?
            .is_some())
    }

    // ---------------------------------------------------------------- scans

    pub fn save_scan(&self, report: &ScanReport) -> AppResult<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO scans (started_at, finished_at, report) VALUES (?1, ?2, ?3)",
            params![
                report.started_at,
                report.finished_at,
                serde_json::to_string(report)?
            ],
        )?;
        conn.execute(
            "DELETE FROM scans WHERE id NOT IN (SELECT id FROM scans ORDER BY id DESC LIMIT 20)",
            [],
        )?;
        Ok(())
    }

    pub fn last_scan(&self) -> AppResult<Option<ScanReport>> {
        let raw: Option<String> = self
            .conn()
            .query_row(
                "SELECT report FROM scans ORDER BY id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?;
        Ok(raw.and_then(|s| serde_json::from_str(&s).ok()))
    }

    // ------------------------------------------------------- preview assets

    pub fn add_preview_asset(
        &self,
        app_id: &str,
        file_path: &str,
        source: &str,
    ) -> AppResult<PreviewAsset> {
        let asset = PreviewAsset {
            id: uuid::Uuid::new_v4().to_string(),
            app_id: app_id.into(),
            file_path: file_path.into(),
            source: source.into(),
            created_at: now(),
        };
        let conn = self.conn();
        conn.execute(
            "INSERT INTO preview_assets (id, app_id, file_path, source, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![asset.id, asset.app_id, asset.file_path, asset.source, asset.created_at],
        )?;
        conn.execute(
            "UPDATE apps SET preview_asset_id = ?2, updated_at = ?3 WHERE id = ?1",
            params![app_id, asset.id, now()],
        )?;
        Ok(asset)
    }

    pub fn preview_assets(&self, app_id: &str) -> AppResult<Vec<PreviewAsset>> {
        let conn = self.conn();
        let mut s = conn.prepare(
            "SELECT id, app_id, file_path, source, created_at FROM preview_assets WHERE app_id = ?1 ORDER BY created_at DESC",
        )?;
        let v = s
            .query_map(params![app_id], |r| {
                Ok(PreviewAsset {
                    id: r.get(0)?,
                    app_id: r.get(1)?,
                    file_path: r.get(2)?,
                    source: r.get(3)?,
                    created_at: r.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(v)
    }

    /// Removes the DB rows; returns file paths so the caller can delete the
    /// copies stored under the app data directory.
    pub fn remove_preview_assets(&self, app_id: &str) -> AppResult<Vec<String>> {
        let paths: Vec<String> = self
            .preview_assets(app_id)?
            .into_iter()
            .map(|a| a.file_path)
            .collect();
        let conn = self.conn();
        conn.execute(
            "DELETE FROM preview_assets WHERE app_id = ?1",
            params![app_id],
        )?;
        conn.execute(
            "UPDATE apps SET preview_asset_id = NULL WHERE id = ?1",
            params![app_id],
        )?;
        Ok(paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_to_latest_version_and_is_idempotent() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.schema_version().unwrap(), 1);
        db.migrate().unwrap();
        assert_eq!(db.schema_version().unwrap(), 1);
    }

    #[test]
    fn file_database_survives_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested dir").join("pulse.db");
        {
            let db = Db::open(&path).unwrap();
            let mut p = db.preferences().unwrap();
            p.theme = "light".into();
            db.save_preferences(&p).unwrap();
        }
        let db = Db::open(&path).unwrap();
        assert_eq!(db.preferences().unwrap().theme, "light");
    }

    #[test]
    fn display_prefs_survive_older_json() {
        let parsed: crate::models::UserPreferences =
            serde_json::from_str(r#"{"theme":"light"}"#).unwrap();
        assert_eq!(parsed.theme, "light");
        assert!(parsed.show_app_icons);
        assert!(parsed.show_shortcuts);
    }

    #[test]
    fn preferences_default_when_missing_or_corrupt() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.preferences().unwrap(), UserPreferences::default());
        db.conn()
            .execute(
                "INSERT INTO preferences (id, value) VALUES (1, 'not json')",
                [],
            )
            .unwrap();
        assert_eq!(db.preferences().unwrap(), UserPreferences::default());
    }

    #[test]
    fn collections_crud() {
        let db = Db::open_in_memory().unwrap();
        db.ensure_system_collection("zeropulse", "ZeroPulse")
            .unwrap();
        db.ensure_system_collection("zeropulse", "ZeroPulse")
            .unwrap();
        let c = db.create_collection("Dev", None).unwrap();
        assert_eq!(db.list_collections().unwrap().len(), 2);
        db.rename_collection(&c.id, "Dev Tools").unwrap();
        assert!(db
            .list_collections()
            .unwrap()
            .iter()
            .any(|x| x.name == "Dev Tools"));
        db.delete_collection(&c.id).unwrap();
        assert_eq!(db.list_collections().unwrap().len(), 1);
        assert!(db.create_collection("   ", None).is_err());
    }

    #[test]
    fn clear_history_removes_events() {
        let db = Db::open_in_memory().unwrap();
        db.record_launch(None, None, "x", true, Some(1), "ok")
            .unwrap();
        assert_eq!(db.launch_events(None, 10).unwrap().len(), 1);
        db.clear_history().unwrap();
        assert!(db.launch_events(None, 10).unwrap().is_empty());
    }
}
