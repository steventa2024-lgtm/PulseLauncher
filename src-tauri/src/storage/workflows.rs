use std::collections::BTreeMap;

use rusqlite::{params, OptionalExtension};

use super::{now, Db};
use crate::error::{AppError, AppResult};
use crate::workflows::{
    contains_run_command, validate_definition, validate_values, WorkflowDefinition, WorkflowRun,
};

fn parse_def(json: String) -> AppResult<WorkflowDefinition> {
    let def: WorkflowDefinition = serde_json::from_str(&json)?;
    validate_definition(&def)?;
    Ok(def)
}

impl Db {
    pub fn list_workflows(&self) -> AppResult<Vec<WorkflowDefinition>> {
        let conn = self.conn();
        let mut stmt =
            conn.prepare("SELECT definition FROM workflows ORDER BY name COLLATE NOCASE")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(parse_def(row?)?);
        }
        Ok(out)
    }

    pub fn get_workflow(&self, id: &str) -> AppResult<WorkflowDefinition> {
        let json: Option<String> = self
            .conn()
            .query_row(
                "SELECT definition FROM workflows WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .optional()?;
        parse_def(json.ok_or_else(|| AppError::not_found("Workflow not found"))?)
    }

    pub fn save_workflow(&self, def: &WorkflowDefinition) -> AppResult<()> {
        validate_definition(def)?;
        let json = serde_json::to_string(def)?;
        let ts = now();
        self.conn().execute(
            "INSERT INTO workflows (id, kind, name, definition, template_key, template_version, trusted, hotkey, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)
             ON CONFLICT(id) DO UPDATE SET kind = excluded.kind, name = excluded.name, definition = excluded.definition,
                template_key = excluded.template_key, template_version = excluded.template_version,
                trusted = excluded.trusted, hotkey = excluded.hotkey, updated_at = excluded.updated_at",
            params![
                def.id,
                match def.kind {
                    crate::workflows::WorkflowKind::Command => "command",
                    crate::workflows::WorkflowKind::Workflow => "workflow",
                },
                def.name,
                json,
                def.template_key,
                def.template_version,
                def.trusted,
                def.hotkey,
                ts
            ],
        )?;
        Ok(())
    }

    /// Inserts any built-in template whose key is not already stored.
    /// Existing rows, including edited ones, are left untouched.
    pub fn seed_templates(&self) -> AppResult<usize> {
        let mut added = 0;
        for def in crate::workflows::templates::all() {
            let key = def.template_key.clone().unwrap_or_default();
            let exists: Option<String> = self
                .conn()
                .query_row(
                    "SELECT id FROM workflows WHERE template_key = ?1",
                    params![key],
                    |r| r.get(0),
                )
                .optional()?;
            if exists.is_none() {
                self.save_workflow(&def)?;
                added += 1;
            }
        }
        Ok(added)
    }

    pub fn reset_template(&self, id: &str) -> AppResult<WorkflowDefinition> {
        let current = self.get_workflow(id)?;
        let key = current
            .template_key
            .clone()
            .ok_or_else(|| AppError::invalid("Only a built-in template can be reset"))?;
        let mut fresh = crate::workflows::templates::by_key(&key).ok_or_else(|| {
            AppError::not_found("This template is no longer part of PulseLauncher")
        })?;
        fresh.id = current.id;
        self.save_workflow(&fresh)?;
        Ok(fresh)
    }

    pub fn update_workflow_values(
        &self,
        id: &str,
        values: BTreeMap<String, String>,
        known_apps: &std::collections::HashSet<String>,
    ) -> AppResult<WorkflowDefinition> {
        let mut def = self.get_workflow(id)?;
        validate_values(&def, &values, known_apps)?;
        def.values = values;
        def.trusted = !contains_run_command(&def.steps);
        self.save_workflow(&def)?;
        Ok(def)
    }

    pub fn update_workflow_steps(
        &self,
        id: &str,
        name: String,
        steps: Vec<crate::workflows::Step>,
    ) -> AppResult<WorkflowDefinition> {
        let mut def = self.get_workflow(id)?;
        def.name = name;
        def.steps = steps;
        def.trusted = !contains_run_command(&def.steps) && def.template_key.is_some();
        if contains_run_command(&def.steps) {
            def.trusted = false;
        }
        self.save_workflow(&def)?;
        Ok(def)
    }

    pub fn delete_workflow(&self, id: &str) -> AppResult<()> {
        let def = self.get_workflow(id)?;
        if def.template_key.is_some() {
            return Err(AppError::policy(
                "Built-in templates can be reset, not deleted.",
            ));
        }
        self.conn()
            .execute("DELETE FROM workflows WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn trust_workflow(
        &self,
        id: &str,
        developer_enabled: bool,
    ) -> AppResult<WorkflowDefinition> {
        if !developer_enabled {
            return Err(AppError::policy(
                "Turn on developer command execution in Settings before trusting a command.",
            ));
        }
        let mut def = self.get_workflow(id)?;
        if !contains_run_command(&def.steps) {
            return Err(AppError::invalid(
                "This workflow has no developer command to trust.",
            ));
        }
        def.trusted = true;
        self.save_workflow(&def)?;
        Ok(def)
    }

    pub fn save_run(&self, run: &WorkflowRun) -> AppResult<()> {
        self.conn().execute(
            "INSERT INTO workflow_runs (id, workflow_id, name, status, started_at, finished_at, steps, error)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                run.id,
                run.workflow_id,
                run.name,
                serde_json::to_value(run.status)?.as_str().unwrap_or("failed"),
                run.started_at,
                run.finished_at,
                serde_json::to_string(&run.steps)?,
                run.error
            ],
        )?;
        self.conn().execute(
            "DELETE FROM workflow_runs WHERE workflow_id = ?1 AND id NOT IN (
                SELECT id FROM workflow_runs WHERE workflow_id = ?1 ORDER BY started_at DESC LIMIT 30
            )",
            params![run.workflow_id],
        )?;
        Ok(())
    }

    pub fn workflow_runs(&self, workflow_id: &str, limit: usize) -> AppResult<Vec<WorkflowRun>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, workflow_id, name, status, started_at, finished_at, steps, error
             FROM workflow_runs WHERE workflow_id = ?1 ORDER BY started_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![workflow_id, limit.min(100) as i64], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, Option<String>>(7)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, workflow_id, name, status, started_at, finished_at, steps, error) = row?;
            out.push(WorkflowRun {
                id,
                workflow_id,
                name,
                status: serde_json::from_value(serde_json::Value::String(status))
                    .unwrap_or(crate::workflows::RunStatus::Failed),
                started_at,
                finished_at,
                steps: serde_json::from_str(&steps).unwrap_or_default(),
                error,
            });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_seed_once_and_edits_survive() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.seed_templates().unwrap(), 12);
        assert_eq!(db.seed_templates().unwrap(), 0);
        assert_eq!(db.list_workflows().unwrap().len(), 12);
        let mut values = BTreeMap::new();
        values.insert("url".into(), "https://example.com/pulse".into());
        let updated = db
            .update_workflow_values("template:open-website", values, &Default::default())
            .unwrap();
        assert_eq!(
            updated.values.get("url").map(String::as_str),
            Some("https://example.com/pulse")
        );
        db.seed_templates().unwrap();
        assert_eq!(
            db.get_workflow("template:open-website")
                .unwrap()
                .values
                .get("url")
                .map(String::as_str),
            Some("https://example.com/pulse")
        );
        let reset = db.reset_template("template:open-website").unwrap();
        assert!(!reset.values.contains_key("url"));
        assert!(db.delete_workflow("template:copy-date").is_err());
    }

    #[test]
    fn rejects_bad_configuration() {
        let db = Db::open_in_memory().unwrap();
        db.seed_templates().unwrap();
        let mut values = BTreeMap::new();
        values.insert("url".into(), "javascript:alert(1)".into());
        assert!(db
            .update_workflow_values("template:open-website", values, &Default::default())
            .is_err());
    }
}
