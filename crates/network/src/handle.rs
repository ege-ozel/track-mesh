//! Bounded commands and responses between application tasks and the single swarm owner.
use crate::NetworkStatus;
use anyhow::{ensure, Context, Result};
use libp2p::{request_response, Multiaddr, PeerId};
use protocol::{PairingRequest, PairingResponse, PeerRequest, PeerResponse};
use std::{
    sync::{Arc, RwLock},
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};
pub struct Incoming {
    pub peer: PeerId,
    pub request: IncomingRequest,
    pub(super) channel: IncomingChannel,
}
#[derive(Clone)]
pub enum IncomingRequest {
    Transfer(PeerRequest),
    Pairing(PairingRequest),
}
pub(super) enum IncomingChannel {
    Transfer(request_response::ResponseChannel<PeerResponse>),
    Pairing(request_response::ResponseChannel<PairingResponse>),
}
pub(super) enum Command {
    Request(PeerId, PeerRequest, oneshot::Sender<Result<PeerResponse>>),
    Respond(Incoming, Box<PeerResponse>),
    PairingRequest(
        PeerId,
        PairingRequest,
        oneshot::Sender<Result<PairingResponse>>,
    ),
    PairingRespond(Incoming, PairingResponse),
    PairingWindow(Duration),
    Reload(Vec<(Multiaddr, String)>),
    Shutdown,
}
#[derive(Clone)]
pub struct Network {
    pub(super) command: mpsc::Sender<Command>,
    pub(super) status: Arc<RwLock<NetworkStatus>>,
}
impl Network {
    pub async fn pairing_window(&self, duration: Duration) -> Result<()> {
        self.command
            .send(Command::PairingWindow(duration))
            .await
            .context("network stopped")
    }
    pub async fn pairing_request(
        &self,
        peer: &str,
        request: PairingRequest,
    ) -> Result<PairingResponse> {
        let peer: PeerId = peer.parse()?;
        ensure!(
            self.online(&peer.to_string()),
            "inviter is offline or unreachable"
        );
        let (tx, rx) = oneshot::channel();
        self.command
            .send(Command::PairingRequest(peer, request, tx))
            .await
            .context("network stopped")?;
        tokio::time::timeout(Duration::from_secs(25), rx)
            .await
            .context("pairing request timed out")??
    }
    pub async fn respond_pairing(
        &self,
        incoming: Incoming,
        response: PairingResponse,
    ) -> Result<()> {
        self.command
            .send(Command::PairingRespond(incoming, response))
            .await
            .context("network stopped")
    }
    pub fn status(&self) -> NetworkStatus {
        self.status.read().map(|s| s.clone()).unwrap_or_default()
    }
    pub fn online(&self, peer: &str) -> bool {
        self.status().peers.get(peer).is_some_and(|p| p.online)
    }
    pub async fn request(&self, peer: &str, request: PeerRequest) -> Result<PeerResponse> {
        let peer: PeerId = peer.parse()?;
        ensure!(
            self.online(&peer.to_string()),
            "source peer is offline; retry when it is online"
        );
        let (tx, rx) = oneshot::channel();
        self.command
            .send(Command::Request(peer, request, tx))
            .await
            .context("network stopped")?;
        tokio::time::timeout(Duration::from_secs(90), rx)
            .await
            .context("peer request timed out")??
    }
    pub async fn respond(&self, incoming: Incoming, response: PeerResponse) -> Result<()> {
        self.command
            .send(Command::Respond(incoming, Box::new(response)))
            .await
            .context("network stopped")
    }
    pub async fn reload(&self, namespaces: Vec<(Multiaddr, String)>) -> Result<()> {
        self.command
            .send(Command::Reload(namespaces))
            .await
            .context("network stopped")
    }
    pub async fn shutdown(&self) {
        let _ = self.command.send(Command::Shutdown).await;
    }
}
