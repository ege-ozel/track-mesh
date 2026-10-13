//! Shared application workflows for the CLI and future desktop/IPC adapters.
//! Callers own network/session lifetime; this module owns project decisions.
use crate::Engine;
use anyhow::{Context, Result};
use network::{ConnectionStatus, Network, NetworkStatus};
use protocol::{DeviceCard, PeerRequest, PeerResponse};
use serde::Serialize;
use storage::{Lineage, Project, SnapshotInfo, TransferInfo};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PeerConnectionState {
    Connected,
    Offline,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PortObservationStatus {
    Unverified,
    VerifiedInbound,
}

pub fn peer_connection_state(network: &NetworkStatus, peer: &str) -> PeerConnectionState {
    if network
        .peers
        .get(peer)
        .is_some_and(|connection| connection.online)
    {
        PeerConnectionState::Connected
    } else {
        PeerConnectionState::Offline
    }
}

pub fn port_observation_status(network: &NetworkStatus) -> PortObservationStatus {
    if network.reachability.state == "VERIFIED_INBOUND" {
        PortObservationStatus::VerifiedInbound
    } else {
        PortObservationStatus::Unverified
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PeerSnapshotStatus {
    RemoteAhead,
    UpToDate,
    Diverged,
    LocalAhead,
    Unknown,
}

#[derive(Serialize)]
pub struct ProjectStatus {
    pub project: Project,
    pub trusted_peers: Vec<DeviceCard>,
    pub snapshots: Vec<SnapshotInfo>,
}

#[derive(Serialize)]
pub struct LocalStatus {
    pub device: DeviceCard,
    pub projects: Vec<ProjectStatus>,
    pub transfers: Vec<TransferInfo>,
    pub verification: &'static str,
}

#[derive(Clone, Serialize)]
pub struct PeerCheck {
    pub project_id: Uuid,
    pub local_snapshot: Option<Uuid>,
    pub remote_snapshot: Option<Uuid>,
    pub lineage: Option<Lineage>,
    pub verified_equal: bool,
    pub connection: Option<ConnectionStatus>,
    /// Based only on the current network actor connection table.
    pub connection_state: PeerConnectionState,
    /// Based on this connected peer's fetched head; equality additionally requires its ACK.
    pub snapshot_status: PeerSnapshotStatus,
    pub last_verified_at: Option<String>,
}

impl Engine {
    pub fn local_status(&self) -> Result<LocalStatus> {
        let projects = self
            .store
            .projects()?
            .into_iter()
            .map(|project| {
                Ok(ProjectStatus {
                    trusted_peers: self.store.peers(Some(project.project_id))?,
                    snapshots: self.store.history(project.project_id)?,
                    project,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(LocalStatus {
            device: self.device_card(vec![])?,
            projects,
            transfers: self.store.transfers()?,
            verification: "LAST_KNOWN; use check to contact a peer",
        })
    }

    pub fn paired_peer(&self, project: Uuid, device: Uuid) -> Result<DeviceCard> {
        self.store
            .peers(Some(project))?
            .into_iter()
            .find(|peer| peer.device_id == device)
            .context("device is not paired with this project")
    }

    pub async fn check_peer(
        &self,
        network: &Network,
        project: Uuid,
        peer: &str,
    ) -> Result<PeerCheck> {
        let remote = self
            .fetch_snapshot_chain(network, project, peer, None)
            .await?;
        let local = self.store.project(project)?.latest_local_snapshot;
        let lineage = remote
            .as_ref()
            .map(|snapshot| {
                self.store
                    .lineage(project, local, snapshot.manifest.snapshot_id)
            })
            .transpose()?;
        let mut verified = false;
        if lineage == Some(Lineage::Equal) {
            let snapshot = remote.as_ref().context("remote snapshot missing")?;
            verified = matches!(
                network
                    .request(
                        peer,
                        PeerRequest::Verified {
                            project_id: project,
                            snapshot_id: snapshot.manifest.snapshot_id,
                            manifest_hash: snapshot.manifest_hash.clone(),
                        }
                    )
                    .await?,
                PeerResponse::Verified { equal: true }
            );
            if verified {
                self.store
                    .mark_verified(project, snapshot.manifest.snapshot_id, peer)?;
            }
        }
        let connection = network.status().peers.get(peer).cloned();
        let connected = connection
            .as_ref()
            .is_some_and(|connection| connection.online);
        let connection_state = if connected {
            PeerConnectionState::Connected
        } else {
            PeerConnectionState::Offline
        };
        let snapshot_status = if !connected {
            PeerSnapshotStatus::Unknown
        } else {
            match (&lineage, verified) {
                (Some(Lineage::RemoteAhead), _) => PeerSnapshotStatus::RemoteAhead,
                (Some(Lineage::Diverged), _) => PeerSnapshotStatus::Diverged,
                (Some(Lineage::LocalAhead), _) => PeerSnapshotStatus::LocalAhead,
                (Some(Lineage::Equal), true) => PeerSnapshotStatus::UpToDate,
                _ => PeerSnapshotStatus::Unknown,
            }
        };
        Ok(PeerCheck {
            project_id: project,
            local_snapshot: local,
            remote_snapshot: remote
                .as_ref()
                .map(|snapshot| snapshot.manifest.snapshot_id),
            lineage,
            verified_equal: verified,
            connection,
            connection_state,
            snapshot_status,
            last_verified_at: self.store.project(project)?.last_verified_at,
        })
    }

    pub async fn receive_from_peer(
        &self,
        network: &Network,
        project: Uuid,
        peer: DeviceCard,
        target: Option<Uuid>,
    ) -> Result<Uuid> {
        self.receive_from_peer_with_limit(network, project, peer, target, u64::MAX)
            .await
    }

    pub async fn receive_from_peer_with_limit(
        &self,
        network: &Network,
        project: Uuid,
        peer: DeviceCard,
        target: Option<Uuid>,
        download_limit: u64,
    ) -> Result<Uuid> {
        let snapshot = self
            .fetch_snapshot_chain(network, project, &peer.peer_id, target)
            .await?
            .context("source has no local snapshot")?;
        let mut receiver = self.receiver(network.clone());
        receiver.download_limit = download_limit;
        let result = receiver.receive(snapshot, peer).await;
        self.maintain_storage_after_transfer().await;
        result
    }

    /// Preserve the current folder as a local snapshot before explicitly replacing a divergent
    /// history with the collaborator's version.
    pub async fn receive_replacing_local(
        self: &std::sync::Arc<Self>,
        network: &Network,
        project: Uuid,
        peer: DeviceCard,
        target: Option<Uuid>,
    ) -> Result<Uuid> {
        self.receive_replacing_local_with_limit(network, project, peer, target, u64::MAX)
            .await
    }

    pub async fn receive_replacing_local_with_limit(
        self: &std::sync::Arc<Self>,
        network: &Network,
        project: Uuid,
        peer: DeviceCard,
        target: Option<Uuid>,
        download_limit: u64,
    ) -> Result<Uuid> {
        // Scanning and hashing are blocking disk work. `snapshot` reuses the head when nothing
        // changed, so the folder is hashed once; an empty folder has nothing to preserve.
        let engine = self.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let project_record = engine.store.project(project)?;
            anyhow::ensure!(
                !project_record.recovery_required,
                "receive recovery must finish before replacing the local version"
            );
            if !snapshot::is_empty(std::path::Path::new(&project_record.root))? {
                engine.snapshot(project)?;
            }
            Ok(())
        })
        .await??;

        let snapshot = self
            .fetch_snapshot_chain(network, project, &peer.peer_id, target)
            .await?
            .context("source has no local snapshot")?;
        let mut receiver = self.receiver(network.clone());
        receiver.download_limit = download_limit;
        let result = receiver.receive_diverged(snapshot, peer).await;
        self.maintain_storage_after_transfer().await;
        result
    }

    pub fn recover(&self, project: Uuid) -> Result<()> {
        transfer::recover_project(
            &self.store,
            &self.chunks,
            &self.identity,
            &self.directory,
            project,
        )
    }
}
