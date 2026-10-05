//! Announcement persistence, pending publication, acknowledgements, and cursors.
use crate::Store;
use anyhow::{ensure, Result};
use protocol::{Announcement, Envelope, SignedSnapshot};
use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

impl Store {
    pub fn ingest_remote(&self, envelope: &Envelope, announcement: &Announcement) -> Result<()> {
        let db = self.lock()?;
        let existing: Option<(String,String,String)> = db.query_row("SELECT manifest_hash,author_device_id,project_id FROM snapshots WHERE snapshot_id=?1", [envelope.snapshot_id.to_string()], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
        if let Some((hash, author, project)) = existing {
            ensure!(
                hash == announcement.manifest_hash
                    && author == envelope.author_device_id.to_string()
                    && project == envelope.project_id.to_string(),
                "remote immutable snapshot collision"
            );
            return Ok(());
        }
        db.execute("INSERT INTO snapshots(snapshot_id,project_id,parent_snapshot_id,author_device_id,created_at,manifest_hash,announcement_json,envelope_json,location,metadata_status,total_size,changed_bytes,file_count,author_name) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'REMOTE','REMOTE',?9,?10,?11,?12)", params![envelope.snapshot_id.to_string(),envelope.project_id.to_string(),announcement.parent_snapshot_id.map(|v|v.to_string()),envelope.author_device_id.to_string(),envelope.created_at.to_rfc3339(),announcement.manifest_hash,serde_json::to_string(announcement)?,serde_json::to_string(envelope)?,announcement.total_size,announcement.changed_bytes,announcement.file_count,announcement.author_name])?;
        Ok(())
    }
    pub fn pending_publications(&self) -> Result<Vec<SignedSnapshot>> {
        let db = self.lock()?;
        let mut stmt = db.prepare("SELECT snapshot_json FROM snapshots WHERE metadata_status='PENDING' AND location='LOCAL' ORDER BY created_at")?;
        let values = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        values
            .into_iter()
            .map(|s| Ok(serde_json::from_str(&s)?))
            .collect()
    }
    pub fn publication_envelope(&self, id: Uuid) -> Result<Option<Envelope>> {
        let json: Option<Option<String>> = self
            .lock()?
            .query_row(
                "SELECT envelope_json FROM snapshots WHERE snapshot_id=?1",
                [id.to_string()],
                |r| r.get(0),
            )
            .optional()?;
        json.flatten()
            .map(|s| Ok(serde_json::from_str(&s)?))
            .transpose()
    }
    pub fn save_envelope(&self, envelope: &Envelope) -> Result<()> {
        self.lock()?.execute(
            "UPDATE snapshots SET envelope_json=COALESCE(envelope_json,?2) WHERE snapshot_id=?1",
            params![
                envelope.snapshot_id.to_string(),
                serde_json::to_string(envelope)?
            ],
        )?;
        Ok(())
    }
    pub fn published(&self, id: Uuid) -> Result<()> {
        self.lock()?.execute("UPDATE snapshots SET metadata_status='PUBLISHED' WHERE snapshot_id=?1 AND metadata_status='PENDING'", [id.to_string()])?;
        Ok(())
    }
    pub fn cursor(&self, project: Uuid) -> Result<i64> {
        Ok(self.lock()?.query_row(
            "SELECT cursor FROM mailbox_state WHERE project_id=?1",
            [project.to_string()],
            |r| r.get(0),
        )?)
    }
    pub fn set_cursor(&self, project: Uuid, cursor: i64) -> Result<()> {
        self.lock()?.execute(
            "UPDATE mailbox_state SET cursor=?2 WHERE project_id=?1 AND cursor<=?2",
            params![project.to_string(), cursor],
        )?;
        Ok(())
    }
}
