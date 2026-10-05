//! Receive orchestration. A project lock survives any spawned blocking apply.
use crate::{
    apply::apply,
    download::download,
    verification::{validate_snapshot, verify_working_folder},
};
use anyhow::{ensure, Context, Result};
use crypto::Identity;
use network::Network;
use protocol::{DeviceCard, PeerRequest, PeerResponse, SignedSnapshot};
use snapshot::{ChunkStore, ProjectLock};
use std::{path::PathBuf, sync::Arc};
use storage::{Lineage, Store, TransferState};
use uuid::Uuid;

pub struct Receiver {
    pub store: Arc<Store>,
    pub chunks: ChunkStore,
    pub network: Network,
    pub identity: Arc<Identity>,
    pub directory: PathBuf,
}
impl Receiver {
    pub async fn receive(&self, snapshot: SignedSnapshot, source: DeviceCard) -> Result<Uuid> {
        let project = snapshot.manifest.project_id;
        let target = snapshot.manifest.snapshot_id;
        let result = self.receive_inner(snapshot, source).await;
        if let Err(error) = &result {
            if let Some(session) = self.store.transfers()?.into_iter().find(|t| {
                t.project_id == project
                    && t.snapshot_id == target
                    && t.state != TransferState::Complete
            }) {
                let state = if self.store.project(project)?.recovery_required {
                    TransferState::RecoveryRequired
                } else {
                    TransferState::Paused
                };
                self.store
                    .transfer_state(session.transfer_id, state, Some(&error.to_string()))?;
            }
        }
        result
    }
    async fn receive_inner(&self, snapshot: SignedSnapshot, source: DeviceCard) -> Result<Uuid> {
        let project = snapshot.manifest.project_id;
        validate_snapshot(&self.store, &self.identity, &snapshot, project, None)?;
        let trusted = self
            .store
            .authorized(project, &source.peer_id)?
            .context("source peer is not paired with this project")?;
        ensure!(
            trusted.device_id == source.device_id && trusted.public_key == source.public_key,
            "source device identity mismatch"
        );
        let lock = Arc::new(ProjectLock::acquire(&self.directory, project)?);
        let current = self.store.project(project)?;
        if current.recovery_required {
            let pending: Vec<_> = self
                .store
                .transfers()?
                .into_iter()
                .filter(|t| t.project_id == project && t.state.requires_recovery())
                .collect();
            ensure!(
                pending.len() == 1 && pending[0].snapshot_id == snapshot.manifest.snapshot_id,
                "recover the existing interrupted apply before receiving a different snapshot"
            );
        }
        let lineage = self.store.lineage(
            project,
            current.latest_local_snapshot,
            snapshot.manifest.snapshot_id,
        )?;
        ensure!(
            matches!(lineage, Lineage::RemoteAhead | Lineage::Equal),
            "history is {lineage:?}; receive is blocked"
        );
        let id = self.store.begin_transfer(&snapshot, source.device_id)?;
        let session = self
            .store
            .transfers()?
            .into_iter()
            .find(|t| t.transfer_id == id)
            .context("transfer record missing")?;
        if session.state == TransferState::Complete {
            ensure!(
                current.latest_local_snapshot == Some(snapshot.manifest.snapshot_id),
                "completed transfer belongs to an older local history"
            );
            let store = self.store.clone();
            let chunks = self.chunks.clone();
            let s = snapshot.clone();
            let guard = lock.clone();
            tokio::task::spawn_blocking(move || {
                let _guard = guard;
                verify_working_folder(&store, &chunks, project, Some(&s))
            })
            .await??;
        } else if current.recovery_required || session.state.requires_recovery() {
            // Local recovery is completed without downloading or regenerating already installed files.
            self.apply_locked(snapshot.clone(), id, lock.clone())
                .await?;
        } else {
            let store = self.store.clone();
            let chunks = self.chunks.clone();
            let guard = lock.clone();
            tokio::task::spawn_blocking(move || {
                let _guard = guard;
                let parent = store.head(project)?;
                verify_working_folder(&store, &chunks, project, parent.as_ref())
            })
            .await??;
            ensure!(
                self.store.project(project)?.latest_local_snapshot == session.base_snapshot_id,
                "local head advanced since this transfer started"
            );
            self.store
                .transfer_state(id, TransferState::Receiving, None)?;
            if let Err(error) = download(
                &self.store,
                &self.chunks,
                &self.network,
                &snapshot,
                id,
                &source.peer_id,
            )
            .await
            {
                self.store
                    .transfer_state(id, TransferState::Paused, Some(&error.to_string()))?;
                return Err(error);
            }
            self.apply_locked(snapshot.clone(), id, lock.clone())
                .await?;
        }
        let confirmed = matches!(
            self.network
                .request(
                    &source.peer_id,
                    PeerRequest::Verified {
                        project_id: project,
                        snapshot_id: snapshot.manifest.snapshot_id,
                        manifest_hash: snapshot.manifest_hash.clone(),
                    }
                )
                .await,
            Ok(PeerResponse::Verified { equal: true })
        );
        if confirmed {
            self.store
                .mark_verified(project, snapshot.manifest.snapshot_id, &source.peer_id)?;
        }
        tracing::info!(transfer_id=%id, snapshot_id=%snapshot.manifest.snapshot_id, confirmed, "snapshot installed and verified locally");
        Ok(id)
    }
    async fn apply_locked(
        &self,
        snapshot: SignedSnapshot,
        id: Uuid,
        lock: Arc<ProjectLock>,
    ) -> Result<()> {
        let store = self.store.clone();
        let chunks = self.chunks.clone();
        let result = tokio::task::spawn_blocking(move || {
            let _guard = lock;
            apply(&store, &chunks, &snapshot, id)
        })
        .await?;
        if let Err(error) = result {
            let session = self
                .store
                .transfers()?
                .into_iter()
                .find(|t| t.transfer_id == id)
                .context("transfer missing")?;
            let recovery = self.store.project(session.project_id)?.recovery_required;
            self.store.transfer_state(
                id,
                if recovery {
                    TransferState::RecoveryRequired
                } else {
                    TransferState::Paused
                },
                Some(&error.to_string()),
            )?;
            return Err(error);
        }
        Ok(())
    }
}
