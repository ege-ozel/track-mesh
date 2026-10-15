//! Snapshot creation shares the existing lock and transactional commit boundary.
use crate::Engine;
use anyhow::{ensure, Result};
use protocol::SignedSnapshot;
use snapshot::{ScanOptions, Stop};
use std::path::Path;
use uuid::Uuid;

impl Engine {
    pub fn snapshot(&self, project: Uuid) -> Result<SignedSnapshot> {
        self.snapshot_impl(project, false, Stop::default())
    }

    /// A stopped scan returns `snapshot::Stopped` before anything is committed.
    pub fn snapshot_until(&self, project: Uuid, stop: Stop) -> Result<SignedSnapshot> {
        self.snapshot_impl(project, false, stop)
    }

    pub fn snapshot_after_save(&self, project: Uuid) -> Result<SignedSnapshot> {
        self.snapshot_impl(project, true, Stop::default())
    }

    fn snapshot_impl(&self, project: Uuid, settled: bool, stop: Stop) -> Result<SignedSnapshot> {
        let lease = self.chunks.lease(false)?;
        let _lock = snapshot::ProjectLock::acquire(&self.directory, project)?;
        let project = self.store.project(project)?;
        ensure!(
            !project.recovery_required,
            "receive recovery must finish before snapshotting"
        );
        let parent = self.store.head(project.project_id)?;
        let scanner = if settled {
            snapshot::scan_after_save
        } else {
            snapshot::scan
        };
        let manifest = scanner(
            Path::new(&project.root),
            &self.chunks,
            project.project_id,
            self.identity.device_id,
            parent.as_ref().map(|s| &s.manifest),
            &ScanOptions {
                stop,
                ..ScanOptions::default()
            },
        )?;
        ensure!(
            !manifest.files.is_empty(),
            "Nothing to share yet: the project folder is empty."
        );
        if let Some(parent) = &parent {
            let previous = parent
                .manifest
                .files
                .iter()
                .filter(|file| !snapshot::is_ignored_path(&file.relative_path));
            if previous.eq(manifest.files.iter()) {
                return Ok(parent.clone());
            }
        }
        let signed = self.identity.sign_snapshot(manifest)?;
        // Stored with the legacy PENDING metadata status; nothing publishes it any more.
        self.store.commit_local(
            &signed,
            &self.identity.name(),
            parent.map(|s| s.manifest.snapshot_id),
            true,
        )?;
        drop(lease);
        self.maintain_storage_if_idle();
        Ok(signed)
    }
}
