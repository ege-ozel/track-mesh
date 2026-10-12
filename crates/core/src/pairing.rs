//! Explicit, ephemeral enrollment. No project-file permission until local approval.
use crate::Engine;
use anyhow::{ensure, Context, Result};
use network::Network;
use protocol::{DeviceCard, PairingRequest, PairingResponse};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;

pub const PAIRING_WINDOW: Duration = Duration::from_secs(600);
const CHALLENGE_TTL: Duration = Duration::from_secs(60);
#[derive(Default)]
pub(crate) struct PairingState {
    window: Option<Window>,
}
struct Window {
    project: Uuid,
    until: Instant,
    challenges: HashMap<String, Challenge>,
    pending: HashMap<Uuid, Pending>,
    rejected: HashSet<String>,
}
struct Challenge {
    card: DeviceCard,
    nonce: String,
    until: Instant,
}
struct Pending {
    card: DeviceCard,
}

#[derive(Default, Serialize)]
pub struct PairingStatus {
    pub project_id: Option<Uuid>,
    pub remaining_seconds: u64,
    pub requests: Vec<PairingCandidate>,
}
#[derive(Serialize)]
pub struct PairingCandidate {
    pub request_id: Uuid,
    pub device_id: Uuid,
    pub name: String,
    pub peer_id: String,
}
impl PairingState {
    fn active(&mut self) -> Option<&mut Window> {
        if self
            .window
            .as_ref()
            .is_some_and(|w| w.until <= Instant::now())
        {
            self.window = None;
        }
        let window = self.window.as_mut()?;
        window.challenges.retain(|_, c| c.until > Instant::now());
        Some(window)
    }
}
impl Engine {
    pub fn open_pairing_window(&self, project: Uuid) -> Result<()> {
        self.store.project(project)?;
        let mut state = self
            .pairing
            .lock()
            .map_err(|_| anyhow::anyhow!("pairing state unavailable"))?;
        state.window = Some(Window {
            project,
            until: Instant::now() + PAIRING_WINDOW,
            challenges: HashMap::new(),
            pending: HashMap::new(),
            rejected: HashSet::new(),
        });
        Ok(())
    }
    pub fn close_pairing_window(&self) -> Result<()> {
        self.pairing
            .lock()
            .map_err(|_| anyhow::anyhow!("pairing state unavailable"))?
            .window = None;
        Ok(())
    }
    pub fn pairing_status(&self) -> Result<PairingStatus> {
        let mut state = self
            .pairing
            .lock()
            .map_err(|_| anyhow::anyhow!("pairing state unavailable"))?;
        let Some(window) = state.active() else {
            return Ok(PairingStatus::default());
        };
        Ok(PairingStatus {
            project_id: Some(window.project),
            remaining_seconds: window
                .until
                .saturating_duration_since(Instant::now())
                .as_secs(),
            requests: window
                .pending
                .iter()
                .map(|(id, pending)| PairingCandidate {
                    request_id: *id,
                    device_id: pending.card.device_id,
                    name: pending.card.name.clone(),
                    peer_id: pending.card.peer_id.clone(),
                })
                .collect(),
        })
    }
    pub fn approve_pairing(&self, project: Uuid, request: Uuid) -> Result<()> {
        let mut state = self
            .pairing
            .lock()
            .map_err(|_| anyhow::anyhow!("pairing state unavailable"))?;
        let window = state
            .active()
            .context("pairing window expired; reopen it and ask the peer to retry")?;
        ensure!(
            window.project == project,
            "pairing request belongs to another project"
        );
        let pending = window
            .pending
            .get(&request)
            .context("pairing request expired or removed")?;
        let card = pending.card.clone();
        self.pair(project, card.clone())?;
        if self.project_backend_config(project)?.backend == crate::ProjectBackend::Supabase {
            // Persist the grant before removing the approval request so an I/O
            // failure can be retried by approving again without losing the card.
            self.queue_supabase_grant(project, &card)?;
        }
        window.pending.remove(&request);
        Ok(())
    }
    pub fn reject_pairing(&self, project: Uuid, request: Uuid) -> Result<()> {
        let mut state = self
            .pairing
            .lock()
            .map_err(|_| anyhow::anyhow!("pairing state unavailable"))?;
        let window = state.active().context("pairing window expired")?;
        ensure!(
            window.project == project,
            "pairing request belongs to another project"
        );
        let pending = window
            .pending
            .remove(&request)
            .context("pairing request expired or removed")?;
        if window.rejected.len() < 8 {
            window.rejected.insert(pending.card.peer_id);
        } else {
            state.window = None;
        }
        Ok(())
    }
    /// Called only in a blocking responder worker; malformed requests reveal no private details.
    pub fn handle_pairing(&self, peer: &str, request: PairingRequest) -> PairingResponse {
        self.handle_pairing_inner(peer, request)
            .unwrap_or(PairingResponse::Unavailable)
    }
    fn handle_pairing_inner(&self, peer: &str, request: PairingRequest) -> Result<PairingResponse> {
        let mut state = self
            .pairing
            .lock()
            .map_err(|_| anyhow::anyhow!("pairing state unavailable"))?;
        match request {
            PairingRequest::Status { project_id } => {
                if self.store.authorized(project_id, peer)?.is_some() {
                    return Ok(PairingResponse::Approved);
                }
                let window = state.active().context("pairing closed")?;
                ensure!(
                    window.project == project_id
                        && window.pending.values().any(|p| p.card.peer_id == peer),
                    "pairing unavailable"
                );
                Ok(PairingResponse::Pending)
            }
            PairingRequest::Begin { project_id, card } => {
                network::validate_card(&card)?;
                ensure!(
                    card.peer_id == peer && card.device_id != self.identity.device_id,
                    "pairing identity mismatch"
                );
                if let Some(known) = self.store.authorized(project_id, peer)? {
                    ensure!(
                        known.device_id == card.device_id && known.public_key == card.public_key,
                        "pairing identity mismatch"
                    );
                    return Ok(PairingResponse::Approved);
                }
                let window = state.active().context("pairing closed")?;
                ensure!(window.project == project_id, "pairing closed");
                ensure!(!window.rejected.contains(peer), "pairing rejected");
                if window.pending.values().any(|p| p.card.peer_id == peer) {
                    return Ok(PairingResponse::Pending);
                }
                ensure!(
                    window.challenges.len() < 8 && window.pending.len() < 8,
                    "pairing busy"
                );
                let nonce = crypto::pairing_challenge();
                window.challenges.insert(
                    peer.to_owned(),
                    Challenge {
                        card,
                        nonce: nonce.clone(),
                        until: Instant::now() + CHALLENGE_TTL,
                    },
                );
                Ok(PairingResponse::Challenge { nonce })
            }
            PairingRequest::Prove {
                project_id,
                challenge,
                proof,
            } => {
                let window = state.active().context("pairing closed")?;
                ensure!(window.project == project_id, "pairing closed");
                ensure!(!window.rejected.contains(peer), "pairing rejected");
                // Consume before validation: even invalid proofs cannot reuse this nonce.
                let issued = window
                    .challenges
                    .remove(peer)
                    .context("pairing challenge expired")?;
                ensure!(
                    issued.until > Instant::now() && issued.nonce == challenge,
                    "pairing challenge expired"
                );
                let key = crypto::project_key(
                    &self.directory.join("secrets/projects"),
                    project_id,
                    None,
                )?;
                let inviter = network::peer_id(&self.identity)?;
                let scope = protocol::pairing_scope(project_id, &inviter, &issued.card)?;
                crypto::verify_pairing_proof(&key, &challenge, &scope, &proof)?;
                ensure!(window.pending.len() < 8, "pairing busy");
                window
                    .pending
                    .insert(Uuid::new_v4(), Pending { card: issued.card });
                Ok(PairingResponse::Pending)
            }
        }
    }
    pub async fn request_pairing(
        self: &Arc<Self>,
        network: &Network,
        project: Uuid,
        inviter_device: Uuid,
        addresses: Vec<String>,
    ) -> Result<PairingResponse> {
        let disk = self.clone();
        let (inviter, card) = tokio::task::spawn_blocking(move || -> Result<_> {
            let inviter = disk.paired_peer(project, inviter_device)?;
            Ok((inviter.peer_id, disk.device_card(addresses)?))
        })
        .await??;
        let response = network
            .pairing_request(
                &inviter,
                PairingRequest::Begin {
                    project_id: project,
                    card: card.clone(),
                },
            )
            .await?;
        let nonce = match response {
            PairingResponse::Challenge { nonce } => nonce,
            PairingResponse::Pending => {
                return self.wait_pairing_approval(network, project, &inviter).await
            }
            other => return Ok(other),
        };
        let disk = self.clone();
        let peer = inviter.clone();
        let challenge = nonce.clone();
        let proof = tokio::task::spawn_blocking(move || -> Result<_> {
            let key = crypto::project_key(&disk.directory.join("secrets/projects"), project, None)?;
            let scope = protocol::pairing_scope(project, &peer, &card)?;
            crypto::pairing_proof(&key, &challenge, &scope)
        })
        .await??;
        let response = network
            .pairing_request(
                &inviter,
                PairingRequest::Prove {
                    project_id: project,
                    challenge: nonce,
                    proof,
                },
            )
            .await?;
        if matches!(response, PairingResponse::Pending) {
            self.wait_pairing_approval(network, project, &inviter).await
        } else {
            Ok(response)
        }
    }

    async fn wait_pairing_approval(
        &self,
        network: &Network,
        project: Uuid,
        inviter: &str,
    ) -> Result<PairingResponse> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
        loop {
            tokio::time::sleep(Duration::from_secs(6)).await;
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Ok(PairingResponse::Pending);
            }
            match tokio::time::timeout(
                remaining,
                network.pairing_request(
                    inviter,
                    PairingRequest::Status {
                        project_id: project,
                    },
                ),
            )
            .await
            {
                Ok(Ok(PairingResponse::Pending)) => {}
                Ok(Ok(response)) => return Ok(response),
                _ => return Ok(PairingResponse::Unavailable),
            }
        }
    }
}
