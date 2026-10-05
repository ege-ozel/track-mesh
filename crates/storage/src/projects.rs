//! Project-folder links and local project status.
use crate::{
    models::{project_row, PROJECT_COLUMNS},
    Project, Store,
};
use anyhow::{ensure, Context, Result};
use rusqlite::{params, OptionalExtension};
use std::path::Path;
use uuid::Uuid;

impl Store {
    pub fn projects_page(&self, offset: u32, limit: u32) -> Result<Vec<Project>> {
        ensure!(
            (1..=33).contains(&limit),
            "project page must contain at most 33 records"
        );
        let db = self.lock()?;
        let mut statement = db.prepare(&format!(
            "SELECT {PROJECT_COLUMNS} FROM projects ORDER BY name,project_id LIMIT ?1 OFFSET ?2"
        ))?;
        let rows = statement
            .query_map(params![limit, offset], project_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
    pub fn projects(&self) -> Result<Vec<Project>> {
        let db = self.lock()?;
        let mut statement = db.prepare(&format!(
            "SELECT {PROJECT_COLUMNS} FROM projects ORDER BY name"
        ))?;
        let rows = statement
            .query_map([], project_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
    pub fn project(&self, id: Uuid) -> Result<Project> {
        self.lock()?
            .query_row(
                &format!("SELECT {PROJECT_COLUMNS} FROM projects WHERE project_id=?1"),
                [id.to_string()],
                project_row,
            )
            .optional()?
            .context("project UUID is not linked to a folder")
    }
    pub fn link(
        &self,
        id: Uuid,
        name: &str,
        root: &Path,
        mailbox: Option<&str>,
        relay: Option<&str>,
    ) -> Result<()> {
        ensure!(
            !name.trim().is_empty() && name.len() <= 320,
            "invalid project name"
        );
        let root = root.to_str().context("project folder must be Unicode")?;
        let db = self.lock()?;
        let existing: Option<String> = db
            .query_row(
                "SELECT root FROM projects WHERE project_id=?1",
                [id.to_string()],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(existing) = existing {
            ensure!(existing == root, "UUID already linked to another folder");
        }
        db.execute("INSERT INTO projects(project_id,name,root,mailbox_url,relay_address) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(project_id) DO UPDATE SET name=excluded.name,mailbox_url=excluded.mailbox_url,relay_address=excluded.relay_address", params![id.to_string(),name,root,mailbox,relay])?;
        db.execute(
            "INSERT OR IGNORE INTO mailbox_state(project_id) VALUES(?1)",
            [id.to_string()],
        )?;
        Ok(())
    }
    pub fn set_project_error(&self, id: Uuid, error: Option<&str>) -> Result<()> {
        self.lock()?.execute(
            "UPDATE projects SET error=?2 WHERE project_id=?1",
            params![id.to_string(), error],
        )?;
        Ok(())
    }
    pub fn mark_recovery(&self, id: Uuid, required: bool) -> Result<()> {
        self.lock()?.execute(
            "UPDATE projects SET recovery_required=?2 WHERE project_id=?1",
            params![id.to_string(), required],
        )?;
        Ok(())
    }
}
