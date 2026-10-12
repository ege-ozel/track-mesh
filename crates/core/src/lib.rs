use anyhow::{ensure, Context, Result};
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
mod endpoints;
mod firewall_observation;
mod network_observation;
mod settings;
mod supabase;
pub use firewall_observation::{observe_firewall, FirewallObservation};
pub use network_observation::{observe_network, NetworkObservation};
mod pairing;
pub use pairing::{PairingCandidate, PairingStatus, PAIRING_WINDOW};
mod mailbox;
pub use connection::public_ipv4_addresses;
pub mod peer;
pub use mailbox::{MailboxRuntime, PublicationStatus, RefreshStatus};
pub use settings::{PendingGrant, ProjectBackend, ProjectBackendConfig, SupabaseProjectState};
pub use supabase::SupabaseRuntime;
mod workflows;
pub use workflows::{
    peer_connection_state, port_observation_status, LocalStatus, PeerCheck, PeerConnectionState,
    PeerSnapshotStatus, PortObservationStatus, ProjectStatus,
};
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
    settings: Mutex<settings::SettingsState>,
    supabase_states: Mutex<std::collections::HashMap<Uuid, SupabaseProjectState>>,
    upnp_mapping_enabled: AtomicBool,
}
impl Engine {
    pub fn open(directory: PathBuf, name: &str) -> Result<Self> {
        std::fs::create_dir_all(&directory)?;
        let directory = std::fs::canonicalize(directory)?;
        let identity = Arc::new(Identity::load_or_create(&directory.join("secrets"), name)?);
        let store = Arc::new(Store::open(&directory.join("trackmesh.db"))?);
        let chunks = ChunkStore::new(directory.join("chunks"))?;
        let settings = settings::SettingsState::load(&directory);
        let upnp_mapping_enabled = settings.value.upnp_mapping_enabled();
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
            settings: Mutex::new(settings),
            supabase_states: Mutex::new(std::collections::HashMap::new()),
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
        let mut settings = self
            .settings
            .lock()
            .map_err(|_| anyhow::anyhow!("settings lock poisoned"))?;
        settings.set_upnp_mapping(&self.directory, enabled)?;
        self.upnp_mapping_enabled.store(enabled, Ordering::SeqCst);
        Ok(())
    }

    pub fn project_backend_config(&self, project: Uuid) -> Result<ProjectBackendConfig> {
        self.settings
            .lock()
            .map_err(|_| anyhow::anyhow!("settings lock poisoned"))?
            .value
            .project_backends
            .get(&project)
            .cloned()
            .map(Ok)
            .unwrap_or_else(|| Ok(ProjectBackendConfig::default()))
    }

    pub fn set_project_backend_config(
        &self,
        project: Uuid,
        config: ProjectBackendConfig,
    ) -> Result<()> {
        self.store.project(project)?;
        match config.backend {
            ProjectBackend::Supabase => {
                let url = config.supabase_url.as_deref().unwrap_or_default();
                let key = config.supabase_anon_key.as_deref().unwrap_or_default();
                ::mailbox::validate_supabase_project_url(url)?;
                ensure!(
                    !key.is_empty() && key.len() <= 2048 && !key.chars().any(char::is_control),
                    "invalid Supabase publishable key"
                );
            }
            ProjectBackend::None | ProjectBackend::SelfHostedMailbox => ensure!(
                config.supabase_url.is_none() && config.supabase_anon_key.is_none(),
                "Supabase credentials are only valid for the Supabase backend"
            ),
        }
        let is_supabase = matches!(config.backend, ProjectBackend::Supabase);
        {
            let mut settings = self
                .settings
                .lock()
                .map_err(|_| anyhow::anyhow!("settings lock poisoned"))?;
            settings.set_project_backend(&self.directory, project, config)?;
        }
        self.set_supabase_project_state(
            project,
            if is_supabase {
                SupabaseProjectState::Unknown
            } else {
                SupabaseProjectState::NotConfigured
            },
        )?;
        if is_supabase {
            for card in self.store.peers(Some(project))? {
                if card.device_id != self.identity.device_id {
                    self.queue_supabase_grant(project, &card)?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn queue_supabase_grant(
        &self,
        project: Uuid,
        card: &protocol::DeviceCard,
    ) -> Result<PendingGrant> {
        network::validate_card(card)?;
        let mut settings = self
            .settings
            .lock()
            .map_err(|_| anyhow::anyhow!("settings lock poisoned"))?;
        ensure!(
            settings
                .value
                .project_backends
                .get(&project)
                .is_some_and(|config| matches!(config.backend, ProjectBackend::Supabase)),
            "Supabase backend is not selected for this project"
        );
        settings.enqueue_grant(
            &self.directory,
            project,
            card.device_id,
            card.public_key.clone(),
        )
    }

    pub(crate) fn pending_supabase_grants(
        &self,
        project: Option<Uuid>,
    ) -> Result<Vec<PendingGrant>> {
        Ok(self
            .settings
            .lock()
            .map_err(|_| anyhow::anyhow!("settings lock poisoned"))?
            .pending_grants(project))
    }

    pub(crate) fn complete_supabase_grant(&self, grant: &PendingGrant) -> Result<()> {
        self.settings
            .lock()
            .map_err(|_| anyhow::anyhow!("settings lock poisoned"))?
            .complete_grant(&self.directory, grant)
    }

    pub(crate) fn set_supabase_project_state(
        &self,
        project: Uuid,
        state: SupabaseProjectState,
    ) -> Result<()> {
        self.supabase_states
            .lock()
            .map_err(|_| anyhow::anyhow!("Supabase status lock poisoned"))?
            .insert(project, state);
        Ok(())
    }

    pub fn supabase_project_state(&self, project: Uuid) -> Result<SupabaseProjectState> {
        if !matches!(
            self.project_backend_config(project)?.backend,
            ProjectBackend::Supabase
        ) {
            return Ok(SupabaseProjectState::NotConfigured);
        }
        Ok(self
            .supabase_states
            .lock()
            .map_err(|_| anyhow::anyhow!("Supabase status lock poisoned"))?
            .get(&project)
            .copied()
            .unwrap_or_default())
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
        let backend = self.project_backend_config(project)?;
        let backend = match backend.backend {
            ProjectBackend::Supabase => Some(protocol::InviteBackendConfig {
                backend: protocol::InviteBackend::Supabase,
                supabase_url: backend
                    .supabase_url
                    .context("Supabase URL is unavailable")?,
                supabase_anon_key: backend
                    .supabase_anon_key
                    .context("Supabase publishable key is unavailable")?,
            }),
            ProjectBackend::None | ProjectBackend::SelfHostedMailbox => None,
        };
        let mut invite = protocol::ProjectInvite {
            version: protocol::PROTOCOL_VERSION,
            project_id: project,
            project_name: p.name,
            project_key: base64::engine::general_purpose::STANDARD.encode(*key),
            mailbox_url: p.mailbox_url,
            relay_address: p.relay_address,
            backend,
            device: self.device_card(addresses)?,
            signature: None,
        };
        if let Err(error) = self.identity.sign_project_invite(&mut invite) {
            crypto::zeroize_project_invite(&mut invite);
            return Err(error);
        }
        Ok(invite)
    }
    pub fn import_invite(&self, mut invite: protocol::ProjectInvite, folder: &Path) -> Result<()> {
        use base64::Engine as _;
        let validation = (|| -> Result<()> {
            protocol::require_protocol(invite.version)?;
            network::validate_card(&invite.device)?;
            ensure!(!invite.project_id.is_nil(), "invalid invitation project");
            if invite.signature.is_some() || invite.backend.is_some() {
                crypto::verify_project_invite(&invite)?;
            }
            if let Some(url) = &invite.mailbox_url {
                ::mailbox::validate_mailbox_url(url)?;
            }
            if let Some(info) = &invite.backend {
                match info.backend {
                    protocol::InviteBackend::Supabase => {
                        ::mailbox::validate_supabase_project_url(&info.supabase_url)?;
                        ensure!(
                            !info.supabase_anon_key.is_empty()
                                && info.supabase_anon_key.len() <= 2048
                                && !info.supabase_anon_key.chars().any(char::is_control),
                            "invalid Supabase publishable key"
                        );
                    }
                }
            }
            ensure!(
                invite.device.device_id != self.identity.device_id,
                "invitation is from this device"
            );
            Ok(())
        })();
        if let Err(error) = validation {
            crypto::zeroize_project_invite(&mut invite);
            return Err(error);
        }
        let encoded_key = zeroize::Zeroizing::new(std::mem::take(&mut invite.project_key));
        let invite_anon_key = invite
            .backend
            .as_mut()
            .map(|info| zeroize::Zeroizing::new(std::mem::take(&mut info.supabase_anon_key)));
        let key: [u8; 32] = base64::engine::general_purpose::STANDARD
            .decode(&*encoded_key)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("invalid project key"))?;
        let key = zeroize::Zeroizing::new(key);
        let backend = match &invite.backend {
            Some(info) => Some(ProjectBackendConfig {
                backend: ProjectBackend::Supabase,
                supabase_url: Some(info.supabase_url.clone()),
                supabase_anon_key: Some(
                    invite_anon_key
                        .as_ref()
                        .context("invalid Supabase publishable key")?
                        .to_string(),
                ),
            }),
            None => None,
        };
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
        if let Some(backend) = backend {
            self.set_project_backend_config(invite.project_id, backend)?;
        }
        self.pair(invite.project_id, invite.device.clone())
    }
}

pub fn default_data_directory() -> Result<PathBuf> {
    Ok(PathBuf::from(
        std::env::var_os("LOCALAPPDATA")
            .ok_or_else(|| anyhow::anyhow!("LOCALAPPDATA is unavailable"))?,
    )
    .join("TrackMesh"))
}
