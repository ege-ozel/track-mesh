use anyhow::{ensure, Result};
use crypto::Identity;
use snapshot::ChunkStore;
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use storage::Store;
use uuid::Uuid;
mod connection;
mod network_observation;
mod settings;
pub use network_observation::{observe_network, NetworkObservation};
mod pairing;
pub use pairing::{PairingCandidate, PairingStatus, PAIRING_WINDOW};
mod mailbox;
pub use connection::public_ipv4_addresses;
pub mod peer;
pub use mailbox::{MailboxRuntime, PublicationStatus, RefreshStatus};
mod workflows;
pub use workflows::{LocalStatus, PeerCheck, ProjectStatus};
mod status;
pub use status::{PeerSummary, ProjectSummary, SummaryPage};
mod sending;
pub use sending::SendStatus;
#[cfg(windows)]
mod ipc;
mod snapshots;
#[cfg(windows)]
pub use ipc::{plugin_pipe_name, PluginRuntime};
#[cfg(windows)]
mod lifetime;
#[cfg(windows)]
pub use lifetime::SessionGrace;

pub struct Engine {
    pub directory: PathBuf,
    pub store: Arc<Store>,
    pub chunks: ChunkStore,
    pub identity: Arc<Identity>,
    metadata_jobs: Mutex<std::collections::HashMap<Uuid, Arc<tokio::sync::Mutex<()>>>>,
    pairing: Mutex<pairing::PairingState>,
    upnp_mapping_enabled: AtomicBool,
}
impl Engine {
    pub fn open(directory: PathBuf, name: &str) -> Result<Self> {
        std::fs::create_dir_all(&directory)?;
        let directory = std::fs::canonicalize(directory)?;
        let identity = Arc::new(Identity::load_or_create(&directory.join("secrets"), name)?);
        let store = Arc::new(Store::open(&directory.join("trackmesh.db"))?);
        let chunks = ChunkStore::new(directory.join("chunks"))?;
        let upnp_mapping_enabled = settings::load_upnp_mapping(&directory);
        let peer_id = network::peer_id(&identity)?;
        store.add_device(
            identity.device_id,
            &identity.name,
            &peer_id,
            &identity.public_key(),
        )?;
        Ok(Self {
            directory,
            store,
            chunks,
            identity,
            metadata_jobs: Mutex::new(std::collections::HashMap::new()),
            pairing: Mutex::new(pairing::PairingState::default()),
            upnp_mapping_enabled: AtomicBool::new(upnp_mapping_enabled),
        })
    }
    pub fn link(
        &self,
        project: Uuid,
        name: &str,
        root: &Path,
        mailbox: Option<&str>,
        relay: Option<&str>,
    ) -> Result<()> {
        if let Some(url) = mailbox {
            ::mailbox::validate_mailbox_url(url)?;
        }
        let root = snapshot::canonical_root(root)?;
        ensure!(
            !self.directory.starts_with(&root) && !root.starts_with(&self.directory),
            "project folder and agent data directory must be separate"
        );
        crypto::project_key(&self.directory.join("secrets/projects"), project, None)?;
        self.store.link(project, name, &root, mailbox, relay)
    }
}

impl Engine {
    pub fn upnp_mapping_enabled(&self) -> bool {
        self.upnp_mapping_enabled.load(Ordering::SeqCst)
    }
    pub fn set_upnp_mapping_enabled(&self, enabled: bool) -> Result<()> {
        settings::store_upnp_mapping(&self.directory, enabled)?;
        self.upnp_mapping_enabled.store(enabled, Ordering::SeqCst);
        Ok(())
    }
}

impl Engine {
    pub fn device_card(&self, addresses: Vec<String>) -> Result<protocol::DeviceCard> {
        let card = self
            .identity
            .sign_card(network::peer_id(&self.identity)?, addresses)?;
        network::validate_card(&card)?;
        Ok(card)
    }
    pub fn pair(&self, project: Uuid, card: protocol::DeviceCard) -> Result<()> {
        network::validate_card(&card)?;
        ensure!(
            card.device_id != self.identity.device_id,
            "cannot pair this device with itself"
        );
        self.store.trust(project, &card)
    }
    pub fn revoke(&self, project: Uuid, device: Uuid) -> Result<()> {
        self.store.project(project)?;
        self.store.revoke(project, device)
    }
    pub fn namespaces(&self) -> Result<Vec<(libp2p::Multiaddr, String)>> {
        let mut result = Vec::new();
        for project in self.store.projects()? {
            if let Some(relay) = project.relay_address {
                let key = crypto::project_key(
                    &self.directory.join("secrets/projects"),
                    project.project_id,
                    None,
                )?;
                let mut material = b"trackmesh-private-rendezvous-v1".to_vec();
                material.extend_from_slice(&*key);
                result.push((relay.parse()?, protocol::hash(&material)));
            }
        }
        Ok(result)
    }
    pub fn export_invite(
        &self,
        project: Uuid,
        addresses: Vec<String>,
    ) -> Result<protocol::ProjectInvite> {
        use base64::Engine as _;
        let p = self.store.project(project)?;
        let key = crypto::project_key(&self.directory.join("secrets/projects"), project, None)?;
        Ok(protocol::ProjectInvite {
            version: protocol::PROTOCOL_VERSION,
            project_id: project,
            project_name: p.name,
            project_key: base64::engine::general_purpose::STANDARD.encode(*key),
            mailbox_url: p.mailbox_url,
            relay_address: p.relay_address,
            device: self.device_card(addresses)?,
        })
    }
    pub fn import_invite(&self, invite: protocol::ProjectInvite, folder: &Path) -> Result<()> {
        use base64::Engine as _;
        protocol::require_protocol(invite.version)?;
        network::validate_card(&invite.device)?;
        if let Some(url) = &invite.mailbox_url {
            ::mailbox::validate_mailbox_url(url)?;
        }
        ensure!(
            invite.device.device_id != self.identity.device_id,
            "invitation is from this device"
        );
        let key: [u8; 32] = base64::engine::general_purpose::STANDARD
            .decode(&invite.project_key)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("invalid project key"))?;
        crypto::project_key(
            &self.directory.join("secrets/projects"),
            invite.project_id,
            Some(&key),
        )?;
        self.link(
            invite.project_id,
            &invite.project_name,
            folder,
            invite.mailbox_url.as_deref(),
            invite.relay_address.as_deref(),
        )?;
        self.pair(invite.project_id, invite.device)
    }
}

pub fn default_data_directory() -> Result<PathBuf> {
    Ok(PathBuf::from(
        std::env::var_os("LOCALAPPDATA")
            .ok_or_else(|| anyhow::anyhow!("LOCALAPPDATA is unavailable"))?,
    )
    .join("TrackMesh"))
}
