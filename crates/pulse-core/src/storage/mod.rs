//! SQLite persistence (WAL, parameterised queries only).

pub mod migrations;

use crate::error::{CoreError, Result};
use crate::models::*;
use crate::security::{self, target_identity};
use crate::workflow::model::{WorkflowDefinition, WorkflowRun};
use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

pub struct Storage {
    conn: Mutex<Connection>,
}

pub fn now_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[derive(Debug, Clone, Default)]
pub struct AppPatch {
    pub name: Option<String>,
    pub target: Option<ExecutableTarget>,
    pub category: Option<AppCategory>,
    pub tags: Option<Vec<String>>,
    pub favorite: Option<bool>,
    pub hidden: Option<bool>,
}

const APP_SELECT: &str = "SELECT a.id,a.name,a.user_renamed,a.source,a.kind,a.path,a.args,a.working_dir,a.aumid,a.uri,a.resolved_path,\
a.identity_key,a.provenance,a.publisher,a.version,a.category,a.tags,a.favorite,a.hidden,a.stale,a.sort_order,a.created_at,a.updated_at,\
(SELECT COUNT(*) FROM launch_events e WHERE e.app_id=a.id AND e.outcome='accepted'),\
(SELECT MAX(at) FROM launch_events e WHERE e.app_id=a.id AND e.outcome='accepted') FROM apps a";

fn kind_to_db(k: TargetKind) -> &'static str {
    match k {
        TargetKind::Executable => "executable",
        TargetKind::Shortcut => "shortcut",
        TargetKind::Packaged => "packaged",
        TargetKind::Uri => "uri",
    }
}

fn kind_from_db(s: &str) -> TargetKind {
    match s {
        "shortcut" => TargetKind::Shortcut,
        "packaged" => TargetKind::Packaged,
        "uri" => TargetKind::Uri,
        _ => TargetKind::Executable,
    }
}

fn row_to_app(r: &Row) -> rusqlite::Result<AppEntry> {
    let args: String = r.get(6)?;
    let tags: String = r.get(16)?;
    Ok(AppEntry {
        id: r.get(0)?,
        name: r.get(1)?,
        user_renamed: r.get::<_, i64>(2)? != 0,
        source: AppSource::from_db(&r.get::<_, String>(3)?),
        target: ExecutableTarget {
            kind: kind_from_db(&r.get::<_, String>(4)?),
            path: r.get(5)?,
            args: serde_json::from_str(&args).unwrap_or_default(),
            working_dir: r.get(7)?,
            aumid: r.get(8)?,
            uri: r.get(9)?,
        },
        resolved_path: r.get(10)?,
        identity_key: r.get(11)?,
        provenance: r.get(12)?,
        publisher: r.get(13)?,
        version: r.get(14)?,
        category: AppCategory::from_db(&r.get::<_, String>(15)?),
        tags: serde_json::from_str(&tags).unwrap_or_default(),
        favorite: r.get::<_, i64>(17)? != 0,
        hidden: r.get::<_, i64>(18)? != 0,
        stale: r.get::<_, i64>(19)? != 0,
        sort_order: r.get(20)?,
        created_at: r.get(21)?,
        updated_at: r.get(22)?,
        collection_ids: vec![],
        launch_count: r.get(23)?,
        last_launched_at: r.get(24)?,
    })
}

impl Storage {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::init(&mut conn)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn open_in_memory() -> Result<Self> {
        let mut conn = Connection::open_in_memory()?;
        Self::init(&mut conn)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    fn init(conn: &mut Connection) -> Result<()> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        migrations::migrate(conn)?;
        Ok(())
    }

    fn c(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn schema_version(&self) -> Result<u32> {
        Ok(self.c().query_row("PRAGMA user_version", [], |r| r.get(0))?)
    }

    // ---------------------------------------------------------------- apps

    fn attach_collections(conn: &Connection, apps: &mut [AppEntry]) -> Result<()> {
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        let mut st = conn.prepare("SELECT app_id, collection_id FROM app_collections ORDER BY position")?;
        for row in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            let (a, c) = row?;
            map.entry(a).or_default().push(c);
        }
        for a in apps {
            a.collection_ids = map.remove(&a.id).unwrap_or_default();
        }
        Ok(())
    }

    pub fn list_apps(&self, include_hidden: bool) -> Result<Vec<AppEntry>> {
        let conn = self.c();
        let sql = format!(
            "{APP_SELECT} {} ORDER BY a.sort_order, a.name COLLATE NOCASE",
            if include_hidden { "" } else { "WHERE a.hidden=0" }
        );
        let mut st = conn.prepare(&sql)?;
        let mut apps: Vec<AppEntry> = st.query_map([], row_to_app)?.collect::<std::result::Result<_, _>>()?;
        Self::attach_collections(&conn, &mut apps)?;
        Ok(apps)
    }

    pub fn get_app(&self, id: &str) -> Result<AppEntry> {
        let conn = self.c();
        let mut apps: Vec<AppEntry> = {
            let mut st = conn.prepare(&format!("{APP_SELECT} WHERE a.id=?1"))?;
            let rows = st.query_map([id], row_to_app)?.collect::<std::result::Result<_, _>>()?;
            rows
        };
        if apps.is_empty() {
            return Err(CoreError::NotFound(format!("app '{id}' not found")));
        }
        Self::attach_collections(&conn, &mut apps)?;
        Ok(apps.remove(0))
    }

    fn insert_app_row(conn: &Connection, a: &AppEntry) -> Result<()> {
        conn.execute(
            "INSERT INTO apps (id,name,user_renamed,source,kind,path,args,working_dir,aumid,uri,resolved_path,identity_key,provenance,publisher,version,category,tags,favorite,hidden,stale,sort_order,created_at,updated_at)\
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23)",
            params![
                a.id, a.name, a.user_renamed as i64, a.source.as_db(), kind_to_db(a.target.kind), a.target.path,
                serde_json::to_string(&a.target.args)?, a.target.working_dir, a.target.aumid, a.target.uri,
                a.resolved_path, a.identity_key, a.provenance, a.publisher, a.version, a.category.as_db(),
                serde_json::to_string(&a.tags)?, a.favorite as i64, a.hidden as i64, a.stale as i64,
                a.sort_order, a.created_at, a.updated_at
            ],
        )?;
        Ok(())
    }

    fn next_sort_order(conn: &Connection) -> Result<i64> {
        Ok(conn.query_row("SELECT COALESCE(MAX(sort_order),0)+1 FROM apps", [], |r| r.get(0))?)
    }

    /// Register a user-chosen target. The caller is expected to have validated it
    /// (`security::validate_target`); this re-validates defensively.
    pub fn add_manual_app(&self, name: &str, target: ExecutableTarget, category: AppCategory) -> Result<AppEntry> {
        self.add_manual_app_opts(name, target, category, true)
    }

    pub fn add_manual_app_opts(&self, name: &str, target: ExecutableTarget, category: AppCategory, check_exists: bool) -> Result<AppEntry> {
        let name = security::validate_display_name(name)?;
        security::validate_target_opts(&target, check_exists)?;
        let conn = self.c();
        let resolved = match target.kind {
            TargetKind::Executable => Some(target.path.clone()),
            _ => None,
        };
        let now = now_iso();
        let entry = AppEntry {
            id: new_id(),
            name,
            user_renamed: true,
            source: AppSource::Manual,
            identity_key: target_identity(&target),
            resolved_path: resolved,
            target,
            provenance: String::new(),
            publisher: None,
            version: None,
            category,
            tags: vec![],
            favorite: false,
            hidden: false,
            stale: false,
            sort_order: Self::next_sort_order(&conn)?,
            created_at: now.clone(),
            updated_at: now,
            collection_ids: vec![],
            launch_count: 0,
            last_launched_at: None,
        };
        Self::insert_app_row(&conn, &entry)?;
        Ok(entry)
    }

    /// Other entries sharing the same canonical identity.
    pub fn find_duplicates_of(&self, id: &str) -> Result<Vec<AppEntry>> {
        let app = self.get_app(id)?;
        Ok(self
            .list_apps(true)?
            .into_iter()
            .filter(|a| a.id != app.id && a.identity_key == app.identity_key)
            .collect())
    }

    pub fn update_app(&self, id: &str, patch: AppPatch) -> Result<AppEntry> {
        let current = self.get_app(id)?;
        {
            let conn = self.c();
            let now = now_iso();
            if let Some(n) = &patch.name {
                let n = security::validate_display_name(n)?;
                conn.execute("UPDATE apps SET name=?1,user_renamed=1,updated_at=?2 WHERE id=?3", params![n, now, id])?;
            }
            if let Some(t) = &patch.target {
                if current.source != AppSource::Manual {
                    return Err(CoreError::Unsupported(
                        "the target of a discovered app cannot be edited; hide it and register a custom entry instead".into(),
                    ));
                }
                security::validate_target(t)?;
                conn.execute(
                    "UPDATE apps SET kind=?1,path=?2,args=?3,working_dir=?4,aumid=?5,uri=?6,resolved_path=?7,identity_key=?8,updated_at=?9 WHERE id=?10",
                    params![
                        kind_to_db(t.kind), t.path, serde_json::to_string(&t.args)?, t.working_dir, t.aumid, t.uri,
                        if t.kind == TargetKind::Executable { Some(t.path.clone()) } else { None },
                        target_identity(t), now, id
                    ],
                )?;
            }
            if let Some(c) = patch.category {
                conn.execute("UPDATE apps SET category=?1,updated_at=?2 WHERE id=?3", params![c.as_db(), now, id])?;
            }
            if let Some(tags) = &patch.tags {
                let tags = normalize_tags(tags)?;
                conn.execute("UPDATE apps SET tags=?1,updated_at=?2 WHERE id=?3", params![serde_json::to_string(&tags)?, now, id])?;
            }
            if let Some(f) = patch.favorite {
                conn.execute("UPDATE apps SET favorite=?1,updated_at=?2 WHERE id=?3", params![f as i64, now, id])?;
            }
            if let Some(h) = patch.hidden {
                conn.execute("UPDATE apps SET hidden=?1,updated_at=?2 WHERE id=?3", params![h as i64, now, id])?;
            }
        }
        self.get_app(id)
    }

    pub fn set_resolved_path(&self, id: &str, path: Option<&str>) -> Result<()> {
        self.c().execute("UPDATE apps SET resolved_path=?1, updated_at=?2 WHERE id=?3", params![path, now_iso(), id])?;
        Ok(())
    }

    pub fn set_app_metadata(&self, id: &str, publisher: Option<&str>, version: Option<&str>) -> Result<()> {
        self.c().execute("UPDATE apps SET publisher=?1, version=?2 WHERE id=?3", params![publisher, version, id])?;
        Ok(())
    }

    /// Manual entries are deleted; discovered entries are hidden (never uninstalled).
    /// Returns `true` if the row was deleted, `false` if it was hidden.
    pub fn remove_app(&self, id: &str) -> Result<bool> {
        let app = self.get_app(id)?;
        let conn = self.c();
        if app.source == AppSource::Manual {
            conn.execute("DELETE FROM apps WHERE id=?1", [id])?;
            Ok(true)
        } else {
            conn.execute("UPDATE apps SET hidden=1, updated_at=?1 WHERE id=?2", params![now_iso(), id])?;
            Ok(false)
        }
    }

    pub fn reorder_apps(&self, ordered_ids: &[String]) -> Result<()> {
        let mut conn = self.c();
        let tx = conn.transaction()?;
        for (i, id) in ordered_ids.iter().enumerate() {
            tx.execute("UPDATE apps SET sort_order=?1 WHERE id=?2", params![i as i64 + 1, id])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Merge scan results, preserving favourites/tags/collections/renames. Entries of the scanned
    /// sources that are no longer present are flagged `stale`, not deleted.
    pub fn reconcile_scan(&self, found: &[DiscoveredApp], scanned: &[AppSource]) -> Result<(usize, usize, usize)> {
        let mut conn = self.c();
        let tx = conn.transaction()?;
        let now = now_iso();
        let (mut added, mut updated) = (0, 0);
        let mut seen_ids: Vec<String> = Vec::new();
        for d in found {
            let existing: Option<(String, bool)> = tx
                .query_row(
                    "SELECT id, user_renamed FROM apps WHERE identity_key=?1 AND source<>'manual'",
                    [&d.identity_key],
                    |r| Ok((r.get(0)?, r.get::<_, i64>(1)? != 0)),
                )
                .optional()?;
            if let Some((id, renamed)) = existing {
                tx.execute(
                    "UPDATE apps SET source=?1,kind=?2,path=?3,args=?4,working_dir=?5,aumid=?6,uri=?7,resolved_path=?8,provenance=?9,stale=0,updated_at=?10 WHERE id=?11",
                    params![
                        d.source.as_db(), kind_to_db(d.target.kind), d.target.path, serde_json::to_string(&d.target.args)?,
                        d.target.working_dir, d.target.aumid, d.target.uri, d.resolved_path, d.provenance, now, id
                    ],
                )?;
                if !renamed {
                    tx.execute("UPDATE apps SET name=?1 WHERE id=?2", params![d.name, id])?;
                }
                seen_ids.push(id);
                updated += 1;
            } else {
                let entry = AppEntry {
                    id: new_id(),
                    name: d.name.clone(),
                    user_renamed: false,
                    source: d.source,
                    target: d.target.clone(),
                    resolved_path: d.resolved_path.clone(),
                    identity_key: d.identity_key.clone(),
                    provenance: d.provenance.clone(),
                    publisher: None,
                    version: None,
                    category: AppCategory::None,
                    tags: vec![],
                    favorite: false,
                    hidden: false,
                    stale: false,
                    sort_order: Self::next_sort_order(&tx)?,
                    created_at: now.clone(),
                    updated_at: now.clone(),
                    collection_ids: vec![],
                    launch_count: 0,
                    last_launched_at: None,
                };
                Self::insert_app_row(&tx, &entry)?;
                seen_ids.push(entry.id);
                added += 1;
            }
        }
        let mut stale = 0;
        {
            let mut st = tx.prepare("SELECT id, source FROM apps WHERE source<>'manual' AND stale=0")?;
            let rows: Vec<(String, String)> = st
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<std::result::Result<_, _>>()?;
            drop(st);
            for (id, src) in rows {
                let src = AppSource::from_db(&src);
                let covered = scanned.iter().any(|s| match (s, src) {
                    // Start Menu user/common may move between each other on re-scan, so treat them as one group.
                    (AppSource::StartMenuUser | AppSource::StartMenuCommon, AppSource::StartMenuUser | AppSource::StartMenuCommon) => true,
                    (a, b) => *a == b,
                });
                if covered && !seen_ids.contains(&id) {
                    tx.execute("UPDATE apps SET stale=1 WHERE id=?1", [id])?;
                    stale += 1;
                }
            }
        }
        tx.commit()?;
        Ok((added, updated, stale))
    }

    // --------------------------------------------------------- collections

    pub fn ensure_system_collection(&self, key: &str, name: &str) -> Result<Collection> {
        let conn = self.c();
        let existing: Option<String> = conn
            .query_row("SELECT id FROM collections WHERE system_key=?1", [key], |r| r.get(0))
            .optional()?;
        let id = match existing {
            Some(id) => id,
            None => {
                let id = new_id();
                conn.execute(
                    "INSERT INTO collections (id,name,system_key,sort_order) VALUES (?1,?2,?3,0)",
                    params![id, name, key],
                )?;
                id
            }
        };
        drop(conn);
        self.get_collection(&id)
    }

    pub fn list_collections(&self) -> Result<Vec<Collection>> {
        let conn = self.c();
        let mut st = conn.prepare(
            "SELECT c.id,c.name,c.system_key,c.sort_order,(SELECT COUNT(*) FROM app_collections ac JOIN apps a ON a.id=ac.app_id WHERE ac.collection_id=c.id AND a.hidden=0) \
             FROM collections c ORDER BY c.system_key IS NULL, c.sort_order, c.name COLLATE NOCASE",
        )?;
        let rows = st
            .query_map([], |r| {
                Ok(Collection { id: r.get(0)?, name: r.get(1)?, system_key: r.get(2)?, sort_order: r.get(3)?, app_count: r.get(4)? })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    pub fn get_collection(&self, id: &str) -> Result<Collection> {
        self.list_collections()?
            .into_iter()
            .find(|c| c.id == id)
            .ok_or_else(|| CoreError::NotFound("collection not found".into()))
    }

    pub fn create_collection(&self, name: &str) -> Result<Collection> {
        let name = security::validate_display_name(name)?;
        let id = new_id();
        {
            let conn = self.c();
            let order: i64 = conn.query_row("SELECT COALESCE(MAX(sort_order),0)+1 FROM collections", [], |r| r.get(0))?;
            conn.execute("INSERT INTO collections (id,name,system_key,sort_order) VALUES (?1,?2,NULL,?3)", params![id, name, order])?;
        }
        self.get_collection(&id)
    }

    pub fn rename_collection(&self, id: &str, name: &str) -> Result<Collection> {
        let name = security::validate_display_name(name)?;
        self.c().execute("UPDATE collections SET name=?1 WHERE id=?2", params![name, id])?;
        self.get_collection(id)
    }

    pub fn delete_collection(&self, id: &str) -> Result<()> {
        let sys: Option<String> = self.c().query_row("SELECT system_key FROM collections WHERE id=?1", [id], |r| r.get(0)).optional()?.flatten();
        if sys.is_some() {
            return Err(CoreError::Unsupported("built-in collections cannot be deleted (you can empty them)".into()));
        }
        self.c().execute("DELETE FROM collections WHERE id=?1", [id])?;
        Ok(())
    }

    pub fn add_to_collection(&self, app_id: &str, collection_id: &str) -> Result<()> {
        self.get_app(app_id)?;
        let conn = self.c();
        let pos: i64 = conn.query_row(
            "SELECT COALESCE(MAX(position),0)+1 FROM app_collections WHERE collection_id=?1",
            [collection_id],
            |r| r.get(0),
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO app_collections (app_id,collection_id,position) VALUES (?1,?2,?3)",
            params![app_id, collection_id, pos],
        )?;
        Ok(())
    }

    pub fn remove_from_collection(&self, app_id: &str, collection_id: &str) -> Result<()> {
        self.c().execute("DELETE FROM app_collections WHERE app_id=?1 AND collection_id=?2", params![app_id, collection_id])?;
        Ok(())
    }

    // -------------------------------------------------------------- launch

    pub fn record_launch(
        &self,
        app_id: Option<&str>,
        app_name: &str,
        outcome: LaunchOutcomeKind,
        pid: Option<u32>,
        detail: Option<&str>,
        args_display: Option<&str>,
        record_history: bool,
    ) -> Result<()> {
        // With history recording off, failures are still counted for diagnostics but only successful
        // launches affect usage ranking; nothing is stored at all when the toggle is off.
        if !record_history {
            return Ok(());
        }
        let outcome_s = match outcome {
            LaunchOutcomeKind::Accepted => "accepted",
            LaunchOutcomeKind::Failed => "failed",
            LaunchOutcomeKind::MissingTarget => "missing_target",
        };
        let detail = detail.map(security::redact);
        let args = args_display.map(security::redact);
        self.c().execute(
            "INSERT INTO launch_events (app_id,app_name,at,outcome,pid,detail,args_display) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![app_id, app_name, now_iso(), outcome_s, pid, detail, args],
        )?;
        Ok(())
    }

    pub fn launch_events(&self, app_id: Option<&str>, limit: u32) -> Result<Vec<LaunchEvent>> {
        let conn = self.c();
        let map = |r: &Row| -> rusqlite::Result<LaunchEvent> {
            let o: String = r.get(4)?;
            Ok(LaunchEvent {
                id: r.get(0)?,
                app_id: r.get(1)?,
                app_name: r.get(2)?,
                at: r.get(3)?,
                outcome: match o.as_str() {
                    "accepted" => LaunchOutcomeKind::Accepted,
                    "missing_target" => LaunchOutcomeKind::MissingTarget,
                    _ => LaunchOutcomeKind::Failed,
                },
                pid: r.get(5)?,
                detail: r.get(6)?,
                args_display: r.get(7)?,
            })
        };
        let base = "SELECT id,app_id,app_name,at,outcome,pid,detail,args_display FROM launch_events";
        let rows = match app_id {
            Some(id) => {
                let mut st = conn.prepare(&format!("{base} WHERE app_id=?1 ORDER BY id DESC LIMIT ?2"))?;
                let v = st.query_map(params![id, limit], map)?.collect::<std::result::Result<Vec<_>, _>>()?;
                v
            }
            None => {
                let mut st = conn.prepare(&format!("{base} ORDER BY id DESC LIMIT ?1"))?;
                let v = st.query_map(params![limit], map)?.collect::<std::result::Result<Vec<_>, _>>()?;
                v
            }
        };
        Ok(rows)
    }

    /// Deletes launch events and workflow run history. Returns rows removed.
    pub fn clear_history(&self) -> Result<usize> {
        let conn = self.c();
        let a = conn.execute("DELETE FROM launch_events", [])?;
        let b = conn.execute("DELETE FROM workflow_runs", [])?;
        Ok(a + b)
    }

    // ----------------------------------------------------------- workflows

    pub fn upsert_workflow(&self, w: &WorkflowDefinition) -> Result<()> {
        let json = serde_json::to_string(w)?;
        let now = now_iso();
        self.c().execute(
            "INSERT INTO workflows (id,kind,name,description,template_id,template_version,definition,enabled,created_at,updated_at)\
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?9)\
             ON CONFLICT(id) DO UPDATE SET kind=?2,name=?3,description=?4,template_version=?6,definition=?7,enabled=?8,updated_at=?9",
            params![w.id, w.kind.as_db(), w.name, w.description, w.template_id, w.template_version, json, w.enabled as i64, now],
        )?;
        Ok(())
    }

    pub fn list_workflows(&self) -> Result<Vec<WorkflowDefinition>> {
        let conn = self.c();
        let mut st = conn.prepare("SELECT definition FROM workflows ORDER BY kind DESC, name COLLATE NOCASE")?;
        let rows: Vec<String> = st.query_map([], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        rows.iter().map(|j| Ok(serde_json::from_str(j)?)).collect()
    }

    pub fn get_workflow(&self, id: &str) -> Result<WorkflowDefinition> {
        let j: Option<String> = self.c().query_row("SELECT definition FROM workflows WHERE id=?1", [id], |r| r.get(0)).optional()?;
        match j {
            Some(j) => Ok(serde_json::from_str(&j)?),
            None => Err(CoreError::NotFound(format!("workflow '{id}' not found"))),
        }
    }

    pub fn get_workflow_by_template(&self, template_id: &str) -> Result<Option<WorkflowDefinition>> {
        let j: Option<String> = self
            .c()
            .query_row("SELECT definition FROM workflows WHERE template_id=?1", [template_id], |r| r.get(0))
            .optional()?;
        Ok(match j {
            Some(j) => Some(serde_json::from_str(&j)?),
            None => None,
        })
    }

    pub fn delete_workflow(&self, id: &str) -> Result<()> {
        self.c().execute("DELETE FROM workflows WHERE id=?1", [id])?;
        Ok(())
    }

    pub fn save_run(&self, run: &WorkflowRun) -> Result<()> {
        self.c().execute(
            "INSERT OR REPLACE INTO workflow_runs (id,workflow_id,workflow_name,started_at,finished_at,status,results) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![
                run.id, run.workflow_id, run.workflow_name, run.started_at, run.finished_at,
                run.status.as_db(), serde_json::to_string(&run.steps)?
            ],
        )?;
        self.c().execute(
            "DELETE FROM workflow_runs WHERE id NOT IN (SELECT id FROM workflow_runs ORDER BY started_at DESC LIMIT 500)",
            [],
        )?;
        Ok(())
    }

    pub fn list_runs(&self, workflow_id: Option<&str>, limit: u32) -> Result<Vec<WorkflowRun>> {
        let conn = self.c();
        let map = |r: &Row| -> rusqlite::Result<(String, String, String, String, Option<String>, String, String)> {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?))
        };
        let base = "SELECT id,workflow_id,workflow_name,started_at,finished_at,status,results FROM workflow_runs";
        let rows: Vec<_> = match workflow_id {
            Some(w) => {
                let mut st = conn.prepare(&format!("{base} WHERE workflow_id=?1 ORDER BY started_at DESC LIMIT ?2"))?;
                let v = st.query_map(params![w, limit], map)?.collect::<std::result::Result<Vec<_>, _>>()?;
                v
            }
            None => {
                let mut st = conn.prepare(&format!("{base} ORDER BY started_at DESC LIMIT ?1"))?;
                let v = st.query_map(params![limit], map)?.collect::<std::result::Result<Vec<_>, _>>()?;
                v
            }
        };
        rows.into_iter()
            .map(|(id, wid, wn, s, f, st, res)| {
                Ok(WorkflowRun {
                    id,
                    workflow_id: wid,
                    workflow_name: wn,
                    started_at: s,
                    finished_at: f,
                    status: crate::workflow::model::RunStatus::from_db(&st),
                    steps: serde_json::from_str(&res)?,
                })
            })
            .collect()
    }

    // ------------------------------------------------------------ settings

    pub fn get_preferences(&self) -> Result<UserPreferences> {
        let v: Option<String> = self.c().query_row("SELECT value FROM settings WHERE key='preferences'", [], |r| r.get(0)).optional()?;
        Ok(match v {
            Some(j) => serde_json::from_str(&j).unwrap_or_default(),
            None => UserPreferences::default(),
        })
    }

    pub fn set_preferences(&self, p: &UserPreferences) -> Result<()> {
        self.c().execute(
            "INSERT INTO settings (key,value) VALUES ('preferences',?1) ON CONFLICT(key) DO UPDATE SET value=?1",
            [serde_json::to_string(p)?],
        )?;
        Ok(())
    }

    pub fn get_meta(&self, key: &str) -> Result<Option<String>> {
        Ok(self.c().query_row("SELECT value FROM meta WHERE key=?1", [key], |r| r.get(0)).optional()?)
    }

    pub fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.c().execute("INSERT INTO meta (key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=?2", params![key, value])?;
        Ok(())
    }

    // ------------------------------------------------------------- preview

    pub fn add_preview_asset(&self, app_id: &str, file_name: &str, caption: Option<&str>, source: &str) -> Result<PreviewAsset> {
        self.get_app(app_id)?;
        let a = PreviewAsset {
            id: new_id(),
            app_id: app_id.into(),
            file_name: file_name.into(),
            caption: caption.map(String::from),
            source: source.into(),
            created_at: now_iso(),
        };
        self.c().execute(
            "INSERT INTO preview_assets (id,app_id,file_name,caption,source,created_at) VALUES (?1,?2,?3,?4,?5,?6)",
            params![a.id, a.app_id, a.file_name, a.caption, a.source, a.created_at],
        )?;
        Ok(a)
    }

    pub fn list_preview_assets(&self, app_id: &str) -> Result<Vec<PreviewAsset>> {
        let conn = self.c();
        let mut st = conn.prepare("SELECT id,app_id,file_name,caption,source,created_at FROM preview_assets WHERE app_id=?1 ORDER BY created_at")?;
        let rows = st
            .query_map([app_id], |r| {
                Ok(PreviewAsset { id: r.get(0)?, app_id: r.get(1)?, file_name: r.get(2)?, caption: r.get(3)?, source: r.get(4)?, created_at: r.get(5)? })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    pub fn delete_preview_asset(&self, id: &str) -> Result<Option<PreviewAsset>> {
        let a: Option<PreviewAsset> = {
            let conn = self.c();
            conn.query_row(
                "SELECT id,app_id,file_name,caption,source,created_at FROM preview_assets WHERE id=?1",
                [id],
                |r| Ok(PreviewAsset { id: r.get(0)?, app_id: r.get(1)?, file_name: r.get(2)?, caption: r.get(3)?, source: r.get(4)?, created_at: r.get(5)? }),
            )
            .optional()?
        };
        self.c().execute("DELETE FROM preview_assets WHERE id=?1", [id])?;
        Ok(a)
    }

    // --------------------------------------------------------- file index

    pub fn replace_indexed_files(&self, root: &str, files: &[(String, String, bool)]) -> Result<()> {
        let mut conn = self.c();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM indexed_files WHERE root=?1", [root])?;
        {
            let mut st = tx.prepare("INSERT OR REPLACE INTO indexed_files (path,name,is_dir,root) VALUES (?1,?2,?3,?4)")?;
            for (p, n, d) in files {
                st.execute(params![p, n, *d as i64, root])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn list_indexed_files(&self) -> Result<Vec<(String, String, bool)>> {
        let conn = self.c();
        let mut st = conn.prepare("SELECT path,name,is_dir FROM indexed_files")?;
        let rows = st
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? != 0)))?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    pub fn clear_file_index(&self) -> Result<()> {
        self.c().execute("DELETE FROM indexed_files", [])?;
        Ok(())
    }

    pub fn count_indexed_files(&self) -> Result<i64> {
        Ok(self.c().query_row("SELECT COUNT(*) FROM indexed_files", [], |r| r.get(0))?)
    }

    /// Replace the database contents of the given sort order etc. for backup restore tests.
    pub fn integrity_check(&self) -> Result<bool> {
        let r: String = self.c().query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        Ok(r == "ok")
    }
}

pub fn normalize_tags(tags: &[String]) -> Result<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for t in tags {
        let t = t.trim();
        if t.is_empty() {
            continue;
        }
        if t.chars().count() > 40 || t.chars().any(|c| c.is_control()) {
            return Err(CoreError::invalid("invalid tag"));
        }
        if !out.iter().any(|e| e.eq_ignore_ascii_case(t)) {
            out.push(t.to_string());
        }
    }
    if out.len() > 30 {
        return Err(CoreError::invalid("too many tags"));
    }
    Ok(out)
}
