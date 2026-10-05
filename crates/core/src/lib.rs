use anyhow::{ensure, Result};
use crypto::Identity;
use protocol::SignedSnapshot;
use snapshot::{ChunkStore, ScanOptions};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use storage::Store;
use uuid::Uuid;
pub mod peer;
mod workflows;
pub use workflows::{LocalStatus, PeerCheck, ProjectStatus};

pub struct Engine {
    pub directory: PathBuf,
    pub store: Arc<Store>,
    pub chunks: ChunkStore,
    pub identity: Arc<Identity>,
}
impl Engine {
    pub fn open(directory: PathBuf, name: &str) -> Result<Self> {
        std::fs::create_dir_all(&directory)?;
        let directory = std::fs::canonicalize(directory)?;
        let identity = Arc::new(Identity::load_or_create(&directory.join("secrets"), name)?);
        let store = Arc::new(Store::open(&directory.join("trackmesh.db"))?);
        let chunks = ChunkStore::new(directory.join("chunks"))?;
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
        let root = snapshot::canonical_root(root)?;
        ensure!(
            !self.directory.starts_with(&root) && !root.starts_with(&self.directory),
            "project folder and agent data directory must be separate"
        );
        crypto::project_key(&self.directory.join("secrets/projects"), project, None)?;
        self.store.link(project, name, &root, mailbox, relay)
    }
    pub fn snapshot(&self, project: Uuid) -> Result<SignedSnapshot> {
        let _lock = snapshot::ProjectLock::acquire(&self.directory, project)?;
        let project = self.store.project(project)?;
        ensure!(
            !project.recovery_required,
            "receive recovery must finish before snapshotting"
        );
        let parent = self.store.head(project.project_id)?;
        let manifest = snapshot::scan(
            Path::new(&project.root),
            &self.chunks,
            project.project_id,
            self.identity.device_id,
            parent.as_ref().map(|s| &s.manifest),
            &ScanOptions::default(),
        )?;
        if let Some(parent) = &parent {
            if parent.manifest.files == manifest.files {
                return Ok(parent.clone());
            }
        }
        let signed = self.identity.sign_snapshot(manifest)?;
        // Always pending until a mailbox ACK; local-only setup remains visibly pending.
        self.store.commit_local(
            &signed,
            &self.identity.name,
            parent.map(|s| s.manifest.snapshot_id),
            true,
        )?;
        Ok(signed)
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
