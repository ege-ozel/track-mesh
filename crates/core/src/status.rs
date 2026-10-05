//! Compact adapter status. Signed manifests and complete history stay outside polling.
use crate::Engine;
use anyhow::Result;
use protocol::DeviceCard;
use serde::Serialize;
use storage::{Project, SnapshotInfo, TransferInfo};

#[derive(Serialize)]
pub struct PeerSummary {
    pub device: DeviceCard,
    /// Last-known authored metadata, not a claim about the peer's current head.
    pub announcement: Option<SnapshotInfo>,
}

#[derive(Serialize)]
pub struct ProjectSummary {
    pub project: Project,
    pub local_snapshot: Option<SnapshotInfo>,
    pub peers: Vec<PeerSummary>,
    pub pending_publications: u64,
    pub transfer: Option<TransferInfo>,
}

#[derive(Serialize)]
pub struct SummaryPage {
    pub projects: Vec<ProjectSummary>,
    pub offset: u32,
    pub has_more: bool,
}

impl Engine {
    pub fn project_summary(&self, project: uuid::Uuid) -> Result<ProjectSummary> {
        self.summarize_project(self.store.project(project)?)
    }

    fn summarize_project(&self, project: Project) -> Result<ProjectSummary> {
        let peers = self
            .store
            .peers(Some(project.project_id))?
            .into_iter()
            .map(|device| {
                Ok(PeerSummary {
                    announcement: self
                        .store
                        .latest_author_info(project.project_id, device.device_id)?,
                    device,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(ProjectSummary {
            local_snapshot: project
                .latest_local_snapshot
                .map(|id| self.store.snapshot_info(id))
                .transpose()?
                .flatten(),
            pending_publications: self
                .store
                .pending_publication_count(project.project_id, self.identity.device_id)?,
            transfer: self.store.latest_transfer(project.project_id)?,
            project,
            peers,
        })
    }
    pub fn summary_page(&self, offset: u32) -> Result<SummaryPage> {
        let mut projects = self.store.projects_page(offset, 33)?;
        let has_more = projects.len() > 32;
        projects.truncate(32);
        let projects = projects
            .into_iter()
            .map(|project| self.summarize_project(project))
            .collect::<Result<Vec<_>>>()?;
        Ok(SummaryPage {
            projects,
            offset,
            has_more,
        })
    }
}
