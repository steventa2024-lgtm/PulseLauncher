use std::collections::{HashMap, HashSet};
use std::path::Path;

use rusqlite::{params, OptionalExtension, Row};
use serde::Deserialize;

use super::{now, Db};
use crate::error::{AppError, AppResult};
use crate::models::*;

const APP_COLS: &str =
    "id, name, custom_name, kind, source, launch_target, resolved_target, arguments, \
    link_arguments, working_dir, icon_path, icon_index, favorite, hidden, portable, category, \
    sort_order, preview_asset_id, created_at, updated_at, last_seen_at";

/// Stable identity used for de-duplication across discovery sources.
pub fn identity_key(target: &ExecutableTarget) -> String {
    let norm = |s: &str| s.replace('/', "\\").trim().to_lowercase();
    match target.kind {
        AppKind::Packaged => format!("aumid:{}", target.launch_target.trim().to_lowercase()),
        AppKind::Uri => format!("uri:{}", target.launch_target.trim().to_lowercase()),
        AppKind::Executable => format!(
            "exe:{}|{}",
            norm(&target.launch_target),
            target.arguments.join("\u{1f}")
        ),
        AppKind::Shortcut => match &target.resolved_target {
            Some(t) if !t.trim().is_empty() => format!(
                "exe:{}|{}",
                norm(t),
                target.link_arguments.clone().unwrap_or_default().trim()
            ),
            _ => format!("lnk:{}", norm(&target.launch_target)),
        },
    }
}

pub fn target_status(target: &ExecutableTarget) -> TargetStatus {
    match target.kind {
        AppKind::Executable => {
            if Path::new(&target.launch_target).is_file() {
                TargetStatus::Ok
            } else {
                TargetStatus::Missing
            }
        }
        AppKind::Shortcut => {
            if !Path::new(&target.launch_target).is_file() {
                return TargetStatus::Missing;
            }
            match &target.resolved_target {
                Some(t) if !t.contains('%') && Path::new(t).is_absolute() => {
                    if Path::new(t).exists() {
                        TargetStatus::Ok
                    } else {
                        TargetStatus::Missing
                    }
                }
                _ => TargetStatus::Unknown,
            }
        }
        AppKind::Packaged | AppKind::Uri => TargetStatus::Unknown,
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppUpdate {
    pub name: Option<String>,
    pub launch_target: Option<String>,
    pub arguments: Option<Vec<String>>,
    pub working_dir: Option<Option<String>>,
    pub icon_path: Option<Option<String>>,
    pub category: Option<Option<String>>,
    pub portable: Option<bool>,
    pub favorite: Option<bool>,
    pub hidden: Option<bool>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewManualApp {
    pub name: String,
    pub kind: AppKind,
    pub launch_target: String,
    #[serde(default)]
    pub arguments: Vec<String>,
    pub working_dir: Option<String>,
    pub icon_path: Option<String>,
    pub category: Option<String>,
    #[serde(default = "default_true")]
    pub portable: bool,
    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_true() -> bool {
    true
}

pub struct ReconcileStats {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    pub marked_missing: usize,
}

impl Db {
    fn map_app(r: &Row) -> rusqlite::Result<AppEntry> {
        let kind: String = r.get(3)?;
        let source: String = r.get(4)?;
        let args: String = r.get(7)?;
        let name: String = r.get(1)?;
        let custom: Option<String> = r.get(2)?;
        let target = ExecutableTarget {
            kind: AppKind::parse(&kind).unwrap_or(AppKind::Executable),
            launch_target: r.get(5)?,
            resolved_target: r.get(6)?,
            arguments: serde_json::from_str(&args).unwrap_or_default(),
            link_arguments: r.get(8)?,
            working_dir: r.get(9)?,
        };
        Ok(AppEntry {
            id: r.get(0)?,
            name: custom.unwrap_or_else(|| name.clone()),
            original_name: name,
            source: DiscoveredSource::parse(&source).unwrap_or(DiscoveredSource::Manual),
            status: target_status(&target),
            target,
            icon_path: r.get(10)?,
            icon_index: r.get(11)?,
            favorite: r.get(12)?,
            hidden: r.get(13)?,
            portable: r.get(14)?,
            category: r.get(15)?,
            sort_order: r.get(16)?,
            preview_asset_id: r.get(17)?,
            created_at: r.get(18)?,
            updated_at: r.get(19)?,
            last_seen_at: r.get(20)?,
            tags: vec![],
            collection_ids: vec![],
            metadata: AppMetadata::default(),
            launch_count: 0,
            last_launched_at: None,
        })
    }

    fn decorate(&self, apps: &mut [AppEntry]) -> AppResult<()> {
        let conn = self.conn();
        let mut tags: HashMap<String, Vec<String>> = HashMap::new();
        let mut s = conn.prepare("SELECT app_id, tag FROM app_tags ORDER BY tag")?;
        for row in s.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            let (a, t) = row?;
            tags.entry(a).or_default().push(t);
        }
        let mut cols: HashMap<String, Vec<String>> = HashMap::new();
        let mut s = conn.prepare("SELECT app_id, collection_id FROM collection_items")?;
        for row in s.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            let (a, c) = row?;
            cols.entry(a).or_default().push(c);
        }
        let mut usage: HashMap<String, (i64, Option<String>)> = HashMap::new();
        let mut s = conn.prepare(
            "SELECT app_id, COUNT(*), MAX(at) FROM launch_events WHERE app_id IS NOT NULL AND success = 1 GROUP BY app_id",
        )?;
        for row in s.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })? {
            let (a, c, at) = row?;
            usage.insert(a, (c, at));
        }
        for app in apps.iter_mut() {
            app.tags = tags.remove(&app.id).unwrap_or_default();
            app.collection_ids = cols.remove(&app.id).unwrap_or_default();
            if let Some((c, at)) = usage.remove(&app.id) {
                app.launch_count = c;
                app.last_launched_at = at;
            }
        }
        Ok(())
    }

    pub fn list_apps(&self, include_hidden: bool) -> AppResult<Vec<AppEntry>> {
        let mut apps = {
            let conn = self.conn();
            let sql = format!(
                "SELECT {APP_COLS} FROM apps {} ORDER BY sort_order, COALESCE(custom_name, name) COLLATE NOCASE",
                if include_hidden { "" } else { "WHERE hidden = 0" }
            );
            let mut s = conn.prepare(&sql)?;
            let v = s
                .query_map([], Self::map_app)?
                .collect::<Result<Vec<_>, _>>()?;
            v
        };
        self.decorate(&mut apps)?;
        Ok(apps)
    }

    pub fn get_app(&self, id: &str) -> AppResult<AppEntry> {
        let app = {
            let conn = self.conn();
            conn.query_row(
                &format!("SELECT {APP_COLS} FROM apps WHERE id = ?1"),
                params![id],
                Self::map_app,
            )
            .optional()?
        };
        let mut apps = vec![app.ok_or_else(|| AppError::not_found("App not found"))?];
        self.decorate(&mut apps)?;
        Ok(apps.remove(0))
    }

    pub fn find_app_by_name(&self, name: &str) -> AppResult<Option<AppEntry>> {
        let lower = name.trim().to_lowercase();
        Ok(self
            .list_apps(false)?
            .into_iter()
            .find(|a| a.name.to_lowercase() == lower))
    }

    pub fn add_manual_app(&self, new: NewManualApp) -> AppResult<AppEntry> {
        let name = crate::security::bounded_text(&new.name, 120, "Display name")?;
        let target = ExecutableTarget {
            kind: new.kind,
            launch_target: new.launch_target.trim().to_string(),
            resolved_target: None,
            arguments: new.arguments,
            link_arguments: None,
            working_dir: new.working_dir.filter(|s| !s.trim().is_empty()),
        };
        crate::security::validate_registration(&target)?;
        let key = identity_key(&target);
        let id = uuid::Uuid::new_v4().to_string();
        let ts = now();
        {
            let conn = self.conn();
            let dup: Option<String> = conn
                .query_row(
                    "SELECT COALESCE(custom_name, name) FROM apps WHERE identity_key = ?1",
                    params![key],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(existing) = dup {
                return Err(AppError::new(
                    crate::error::ErrorCode::Conflict,
                    format!("This target is already registered as \"{existing}\"."),
                ));
            }
            conn.execute(
                "INSERT INTO apps (id, identity_key, name, kind, source, launch_target, arguments, working_dir,
                    icon_path, portable, category, status, created_at, updated_at, last_seen_at)
                 VALUES (?1, ?2, ?3, ?4, 'manual', ?5, ?6, ?7, ?8, ?9, ?10, 'unknown', ?11, ?11, ?11)",
                params![
                    id,
                    key,
                    name,
                    target.kind.as_str(),
                    target.launch_target,
                    serde_json::to_string(&target.arguments)?,
                    target.working_dir,
                    new.icon_path,
                    new.portable,
                    new.category,
                    ts
                ],
            )?;
        }
        self.set_tags(&id, &new.tags)?;
        self.get_app(&id)
    }

    pub fn update_app(&self, id: &str, upd: AppUpdate) -> AppResult<AppEntry> {
        let current = self.get_app(id)?;
        let manual = current.source == DiscoveredSource::Manual;
        let mut target = current.target.clone();
        if manual {
            if let Some(t) = &upd.launch_target {
                target.launch_target = t.trim().to_string();
            }
            if let Some(a) = &upd.arguments {
                target.arguments = a.clone();
            }
            if let Some(w) = &upd.working_dir {
                target.working_dir = w.clone().filter(|s| !s.trim().is_empty());
            }
            crate::security::validate_registration(&target)?;
        } else if upd.launch_target.is_some()
            || upd.arguments.is_some()
            || upd.working_dir.is_some()
        {
            return Err(AppError::policy(
                "Targets of OS-discovered apps cannot be edited. Register a custom entry instead.",
            ));
        }
        let custom_name = match &upd.name {
            Some(n) => {
                let n = crate::security::bounded_text(n, 120, "Display name")?;
                if n == current.original_name && !manual {
                    None
                } else {
                    Some(n)
                }
            }
            None => (current.name != current.original_name).then(|| current.name.clone()),
        };
        {
            let conn = self.conn();
            let key = identity_key(&target);
            if manual {
                let clash: Option<String> = conn
                    .query_row(
                        "SELECT id FROM apps WHERE identity_key = ?1 AND id != ?2",
                        params![key, id],
                        |r| r.get(0),
                    )
                    .optional()?;
                if clash.is_some() {
                    return Err(AppError::new(
                        crate::error::ErrorCode::Conflict,
                        "Another entry already uses this target and arguments.",
                    ));
                }
            }
            let (name_col, custom_col) = if manual {
                (
                    custom_name.clone().unwrap_or(current.original_name.clone()),
                    None,
                )
            } else {
                (current.original_name.clone(), custom_name)
            };
            conn.execute(
                "UPDATE apps SET name = ?2, custom_name = ?3, launch_target = ?4, arguments = ?5, working_dir = ?6,
                    icon_path = ?7, category = ?8, portable = ?9, favorite = ?10, hidden = ?11,
                    identity_key = ?12, updated_at = ?13 WHERE id = ?1",
                params![
                    id,
                    name_col,
                    custom_col,
                    target.launch_target,
                    serde_json::to_string(&target.arguments)?,
                    target.working_dir,
                    upd.icon_path.clone().unwrap_or(current.icon_path.clone()),
                    upd.category.clone().unwrap_or(current.category.clone()),
                    upd.portable.unwrap_or(current.portable),
                    upd.favorite.unwrap_or(current.favorite),
                    upd.hidden.unwrap_or(current.hidden),
                    key,
                    now()
                ],
            )?;
        }
        if let Some(tags) = &upd.tags {
            self.set_tags(id, tags)?;
        }
        self.get_app(id)
    }

    pub fn set_tags(&self, id: &str, tags: &[String]) -> AppResult<()> {
        let mut clean: Vec<String> = Vec::new();
        for t in tags {
            let t = t.trim().to_lowercase();
            if t.is_empty() {
                continue;
            }
            let t = crate::security::bounded_text(&t, 40, "Tag")?;
            if !clean.contains(&t) {
                clean.push(t);
            }
        }
        if clean.len() > 32 {
            return Err(AppError::invalid("At most 32 tags per app"));
        }
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM app_tags WHERE app_id = ?1", params![id])?;
        for t in clean {
            tx.execute(
                "INSERT INTO app_tags (app_id, tag) VALUES (?1, ?2)",
                params![id, t],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Manual entries are deleted; OS-discovered entries are hidden (never
    /// uninstalled - PulseLauncher does not touch installed software).
    pub fn remove_app(&self, id: &str) -> AppResult<bool> {
        let app = self.get_app(id)?;
        let conn = self.conn();
        if app.source == DiscoveredSource::Manual {
            conn.execute("DELETE FROM apps WHERE id = ?1", params![id])?;
            Ok(true)
        } else {
            conn.execute(
                "UPDATE apps SET hidden = 1, updated_at = ?2 WHERE id = ?1",
                params![id, now()],
            )?;
            Ok(false)
        }
    }

    pub fn reorder_apps(&self, ids: &[String]) -> AppResult<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        for (i, id) in ids.iter().enumerate() {
            tx.execute(
                "UPDATE apps SET sort_order = ?2 WHERE id = ?1",
                params![id, i as i64],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Merge a discovery pass into the database. User data (favorites, tags,
    /// custom names, collections, hidden) is preserved. Entries from scanned
    /// sources that disappeared are deleted unless the user customised them,
    /// in which case they stay and show as missing.
    pub fn reconcile_discovered(
        &self,
        found: &[DiscoveredApp],
        scanned_sources: &[DiscoveredSource],
    ) -> AppResult<ReconcileStats> {
        let mut stats = ReconcileStats {
            added: 0,
            updated: 0,
            removed: 0,
            marked_missing: 0,
        };
        // De-duplicate by identity, keeping the highest priority source.
        let mut best: HashMap<String, &DiscoveredApp> = HashMap::new();
        for app in found {
            let key = identity_key(&app.target);
            match best.get(&key) {
                Some(existing) if existing.source.priority() <= app.source.priority() => {}
                _ => {
                    best.insert(key, app);
                }
            }
        }
        let ts = now();
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        let mut seen: HashSet<String> = HashSet::new();
        for (key, app) in &best {
            let existing: Option<(String, String)> = tx
                .query_row(
                    "SELECT id, source FROM apps WHERE identity_key = ?1",
                    params![key],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            match existing {
                Some((id, source)) => {
                    seen.insert(id.clone());
                    if source == "manual" {
                        continue;
                    }
                    tx.execute(
                        "UPDATE apps SET name = ?2, kind = ?3, source = ?4, launch_target = ?5, resolved_target = ?6,
                            link_arguments = ?7, working_dir = ?8, icon_path = ?9, icon_index = ?10,
                            category = COALESCE(category, ?11), last_seen_at = ?12 WHERE id = ?1",
                        params![
                            id,
                            app.name,
                            app.target.kind.as_str(),
                            app.source.as_str(),
                            app.target.launch_target,
                            app.target.resolved_target,
                            app.target.link_arguments,
                            app.target.working_dir,
                            app.icon_path,
                            app.icon_index,
                            app.category,
                            ts
                        ],
                    )?;
                    stats.updated += 1;
                }
                None => {
                    let id = uuid::Uuid::new_v4().to_string();
                    tx.execute(
                        "INSERT INTO apps (id, identity_key, name, kind, source, launch_target, resolved_target, arguments,
                            link_arguments, working_dir, icon_path, icon_index, category, created_at, updated_at, last_seen_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14, ?14)",
                        params![
                            id,
                            key,
                            app.name,
                            app.target.kind.as_str(),
                            app.source.as_str(),
                            app.target.launch_target,
                            app.target.resolved_target,
                            serde_json::to_string(&app.target.arguments)?,
                            app.target.link_arguments,
                            app.target.working_dir,
                            app.icon_path,
                            app.icon_index,
                            app.category,
                            ts
                        ],
                    )?;
                    seen.insert(id);
                    stats.added += 1;
                }
            }
        }
        // Handle vanished entries from the sources that were scanned.
        let sources: Vec<&str> = scanned_sources.iter().map(|s| s.as_str()).collect();
        let mut stale: Vec<(String, bool)> = Vec::new();
        {
            let mut s = tx.prepare(
                "SELECT a.id,
                    (a.favorite = 1 OR a.custom_name IS NOT NULL OR a.hidden = 1
                     OR EXISTS (SELECT 1 FROM app_tags t WHERE t.app_id = a.id)
                     OR EXISTS (SELECT 1 FROM collection_items c WHERE c.app_id = a.id)) AS customised,
                    a.source
                 FROM apps a WHERE a.source != 'manual'",
            )?;
            let rows = s.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, bool>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?;
            for row in rows {
                let (id, customised, source) = row?;
                if !seen.contains(&id) && sources.contains(&source.as_str()) {
                    stale.push((id, customised));
                }
            }
        }
        for (id, customised) in stale {
            if customised {
                stats.marked_missing += 1;
            } else {
                tx.execute("DELETE FROM apps WHERE id = ?1", params![id])?;
                stats.removed += 1;
            }
        }
        tx.commit()?;
        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lnk(name: &str, lnk_path: &str, target: &str, source: DiscoveredSource) -> DiscoveredApp {
        DiscoveredApp {
            name: name.into(),
            source,
            target: ExecutableTarget {
                kind: AppKind::Shortcut,
                launch_target: lnk_path.into(),
                resolved_target: Some(target.into()),
                arguments: vec![],
                link_arguments: None,
                working_dir: None,
            },
            icon_path: None,
            icon_index: 0,
            category: None,
        }
    }

    #[test]
    fn reconcile_dedupes_and_preserves_user_data() {
        let db = Db::open_in_memory().unwrap();
        let scanned = [
            DiscoveredSource::StartMenuUser,
            DiscoveredSource::StartMenuCommon,
        ];
        let found = vec![
            lnk(
                "Code",
                "C:\\u\\Code.lnk",
                "C:\\Apps\\Code.exe",
                DiscoveredSource::StartMenuCommon,
            ),
            lnk(
                "Code",
                "C:\\c\\Code.lnk",
                "c:/apps/code.exe",
                DiscoveredSource::StartMenuUser,
            ),
            lnk(
                "Paint",
                "C:\\c\\Paint.lnk",
                "C:\\Win\\mspaint.exe",
                DiscoveredSource::StartMenuCommon,
            ),
        ];
        let s = db.reconcile_discovered(&found, &scanned).unwrap();
        assert_eq!(s.added, 2);
        let apps = db.list_apps(true).unwrap();
        assert_eq!(apps.len(), 2);
        let code = apps.iter().find(|a| a.name == "Code").unwrap();
        assert_eq!(code.source, DiscoveredSource::StartMenuUser);

        db.update_app(
            &code.id,
            AppUpdate {
                favorite: Some(true),
                name: Some("VS Code".into()),
                tags: Some(vec!["Dev".into(), "dev".into()]),
                ..Default::default()
            },
        )
        .unwrap();

        // Second scan: Paint removed, Code still present.
        let s = db.reconcile_discovered(&found[..2], &scanned).unwrap();
        assert_eq!(s.removed, 1);
        let code = db.get_app(&code.id).unwrap();
        assert!(code.favorite);
        assert_eq!(code.name, "VS Code");
        assert_eq!(code.original_name, "Code");
        assert_eq!(code.tags, vec!["dev".to_string()]);

        // Third scan: Code gone, but customised so it is kept.
        let s = db.reconcile_discovered(&[], &scanned).unwrap();
        assert_eq!(s.marked_missing, 1);
        assert!(db.get_app(&code.id).is_ok());
    }

    #[test]
    fn discovered_targets_are_not_editable_and_removal_hides() {
        let db = Db::open_in_memory().unwrap();
        db.reconcile_discovered(
            &[lnk(
                "A",
                "C:\\a.lnk",
                "C:\\a.exe",
                DiscoveredSource::StartMenuUser,
            )],
            &[DiscoveredSource::StartMenuUser],
        )
        .unwrap();
        let a = db.list_apps(false).unwrap().remove(0);
        let err = db
            .update_app(
                &a.id,
                AppUpdate {
                    launch_target: Some("C:\\evil.exe".into()),
                    ..Default::default()
                },
            )
            .unwrap_err();
        assert_eq!(err.code, crate::error::ErrorCode::PolicyBlocked);
        assert!(!db.remove_app(&a.id).unwrap());
        assert!(db.list_apps(false).unwrap().is_empty());
        assert_eq!(db.list_apps(true).unwrap().len(), 1);
    }

    #[test]
    fn manual_app_with_spaces_and_unicode_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("Pulse Tools ünïcødé");
        std::fs::create_dir_all(&folder).unwrap();
        let exe = folder.join("Pulse Cut.exe");
        std::fs::write(&exe, b"MZ").unwrap();
        let db = Db::open_in_memory().unwrap();
        let app = db
            .add_manual_app(NewManualApp {
                name: "PulseCut".into(),
                kind: AppKind::Executable,
                launch_target: exe.to_string_lossy().into(),
                arguments: vec!["--project".into(), "C:\\My Projects\\a b".into()],
                working_dir: Some(folder.to_string_lossy().into()),
                icon_path: None,
                category: Some("ZeroPulse".into()),
                portable: true,
                tags: vec!["zeropulse".into()],
            })
            .unwrap();
        assert_eq!(app.status, TargetStatus::Ok);
        assert_eq!(app.target.arguments.len(), 2);
        // Duplicate registration is rejected.
        assert!(db
            .add_manual_app(NewManualApp {
                name: "Again".into(),
                kind: AppKind::Executable,
                launch_target: exe.to_string_lossy().into(),
                arguments: vec!["--project".into(), "C:\\My Projects\\a b".into()],
                working_dir: None,
                icon_path: None,
                category: None,
                portable: true,
                tags: vec![],
            })
            .is_err());
        std::fs::remove_file(&exe).unwrap();
        assert_eq!(db.get_app(&app.id).unwrap().status, TargetStatus::Missing);
        assert!(db.remove_app(&app.id).unwrap());
        assert!(db.get_app(&app.id).is_err());
    }

    #[test]
    fn rejects_relative_or_non_executable_registration() {
        let db = Db::open_in_memory().unwrap();
        let r = db.add_manual_app(NewManualApp {
            name: "x".into(),
            kind: AppKind::Executable,
            launch_target: "notepad.exe".into(),
            arguments: vec![],
            working_dir: None,
            icon_path: None,
            category: None,
            portable: true,
            tags: vec![],
        });
        assert!(r.is_err());
    }
}
