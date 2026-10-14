//! One explicit desktop session, bounded jobs, and awaited shutdown.
use crate::documents;
use anyhow::{ensure, Context, Result};
use clap::Parser;
use network::{Network, NetworkConfig, NetworkStatus};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;
use trackmesh_core::{
    default_data_directory, observe_firewall, observe_network, peer::PeerRuntime,
    peer_connection_state, port_observation_status, Engine, FirewallObservation, MailboxRuntime,
    NetworkObservation, PairingStatus, PeerCheck, PeerConnectionState, PluginRuntime,
    PortObservationStatus, ProjectBackend, ProjectBackendConfig, RevokeOutcome, SessionGrace,
    SummaryPage, SupabaseDefaultConfig, SupabaseProjectState,
};
use uuid::Uuid;

fn receive_blocked_or_error(error: anyhow::Error) -> Result<Value> {
    if let Some(refusal) = error.downcast_ref::<transfer::ReceiveRefusal>() {
        // The core message advises snapshotting first; the card offers the explicit choice instead.
        let (code, message) = match refusal {
            transfer::ReceiveRefusal::LocalChanges => (
                "LOCAL_CHANGES",
                "This folder has changes you haven't shared. Choose which version to keep.",
            ),
            transfer::ReceiveRefusal::Diverged => (
                "DIVERGED",
                "Both versions changed. Choose which version to keep.",
            ),
        };
        Ok(json!({"receive_blocked": code, "message": message}))
    } else {
        Err(error)
    }
}

#[derive(Clone, Parser)]
#[command(
    name = "trackmesh-desktop",
    about = "Private Windows project snapshots and transfers"
)]
pub struct Options {
    /// Plugin-launched owner: exit after sessions and durable work finish plus grace.
    #[arg(long)]
    plugin_owner: bool,
    #[arg(long, env = "TRACKMESH_DATA_DIR")]
    data_dir: Option<PathBuf>,
    #[arg(long, default_value = "My studio PC")]
    device_name: String,
    #[arg(long, default_value_t = 43101)]
    port: u16,
    #[arg(long)]
    external_address: Vec<String>,
    /// Development only: permit HTTP to a literal loopback IP.
    #[arg(long)]
    allow_loopback_http: bool,
    #[arg(long, env = "TRACKMESH_IPC_SUFFIX", default_value = "")]
    ipc_suffix: String,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    DeleteBackups {
        selected: Vec<trackmesh_core::BackupSelection>,
    },
    Link {
        project_id: Uuid,
        name: String,
        folder: PathBuf,
        mailbox_url: Option<String>,
        relay_address: Option<String>,
    },
    ChangeProjectRoot {
        project_id: Uuid,
        folder: PathBuf,
    },
    SetProjectBackend {
        project_id: Uuid,
        backend: ProjectBackend,
        supabase_url: Option<String>,
        supabase_anon_key: Option<String>,
    },
    SetDefaultSupabase {
        url: Option<String>,
        publishable_key: Option<String>,
    },
    SetDeviceName {
        name: String,
    },
    SetProjectsLocation {
        location: PathBuf,
    },
    SetManualPublicEndpoint {
        public_ipv4: Option<String>,
        external_port: Option<u16>,
    },
    Join {
        folder: PathBuf,
    },
    CompleteInviteLink {
        supabase_url: Option<String>,
        supabase_anon_key: Option<String>,
    },
    Pair {
        project_id: Uuid,
    },
    Revoke {
        project_id: Uuid,
        device_id: Uuid,
    },
    ExportInvite {
        project_id: Uuid,
        public_ip: Option<String>,
        external_port: Option<u16>,
    },
    CopyInviteLink {
        project_id: Uuid,
    },
    ExportCard,
    OpenPairing {
        project_id: Uuid,
    },
    ClosePairing {
        project_id: Uuid,
    },
    ApprovePairing {
        project_id: Uuid,
        request_id: Uuid,
    },
    RejectPairing {
        project_id: Uuid,
        request_id: Uuid,
    },
    RequestPairing {
        project_id: Uuid,
        device_id: Uuid,
    },
    Send {
        project_id: Uuid,
    },
    Publish {
        project_id: Uuid,
    },
    Refresh {
        project_id: Uuid,
    },
    Check {
        project_id: Uuid,
        device_id: Uuid,
    },
    Receive {
        project_id: Uuid,
        device_id: Uuid,
        snapshot_id: Option<Uuid>,
        daw_closed: bool,
    },
    UsePeerVersion {
        project_id: Uuid,
        device_id: Uuid,
        daw_closed: bool,
    },
    Recover {
        project_id: Uuid,
        daw_closed: bool,
    },
}

impl Action {
    fn project(&self) -> Option<Uuid> {
        match self {
            Self::Join { .. }
            | Self::DeleteBackups { .. }
            | Self::CompleteInviteLink { .. }
            | Self::SetDefaultSupabase { .. }
            | Self::SetDeviceName { .. }
            | Self::SetProjectsLocation { .. }
            | Self::SetManualPublicEndpoint { .. }
            | Self::ExportCard => None,
            Self::Link { project_id, .. }
            | Self::ChangeProjectRoot { project_id, .. }
            | Self::SetProjectBackend { project_id, .. }
            | Self::Pair { project_id }
            | Self::Revoke { project_id, .. }
            | Self::ExportInvite { project_id, .. }
            | Self::CopyInviteLink { project_id }
            | Self::OpenPairing { project_id }
            | Self::ClosePairing { project_id }
            | Self::ApprovePairing { project_id, .. }
            | Self::RejectPairing { project_id, .. }
            | Self::RequestPairing { project_id, .. }
            | Self::Send { project_id }
            | Self::Publish { project_id }
            | Self::Refresh { project_id }
            | Self::Check { project_id, .. }
            | Self::Receive { project_id, .. }
            | Self::UsePeerVersion { project_id, .. }
            | Self::Recover { project_id, .. } => Some(*project_id),
        }
    }
    fn label(&self) -> &'static str {
        match self {
            Self::DeleteBackups { .. } => "Delete old backups",
            Self::Link { .. } => "Link folder",
            Self::ChangeProjectRoot { .. } => "Change project folder",
            Self::SetProjectBackend { .. } => "Save project backend",
            Self::SetDefaultSupabase { .. } => "Save default Supabase settings",
            Self::SetDeviceName { .. } => "Save device name",
            Self::SetProjectsLocation { .. } => "Save Projects location",
            Self::SetManualPublicEndpoint { .. } => "Save manual port forwarding",
            Self::Join { .. } => "Accept invitation and request pairing",
            Self::CompleteInviteLink { .. } => "Accept invite link",
            Self::Pair { .. } => "Complete pairing",
            Self::Revoke { .. } => "Revoke device",
            Self::ExportInvite { .. } => "Export invitation",
            Self::CopyInviteLink { .. } => "Copy invite link",
            Self::ExportCard => "Save pairing reply",
            Self::OpenPairing { .. } => "Open pairing window",
            Self::ClosePairing { .. } => "Close pairing window",
            Self::ApprovePairing { .. } => "Approve pairing",
            Self::RejectPairing { .. } => "Reject pairing",
            Self::RequestPairing { .. } => "Request automatic pairing",
            Self::Send { .. } => "Share current version",
            Self::Publish { .. } => "Publish metadata",
            Self::Refresh { .. } => "Refresh metadata",
            Self::Check { .. } => "Check connection",
            Self::Receive { .. } => "Get collaborator's version",
            Self::UsePeerVersion { .. } => "Use collaborator's version",
            Self::Recover { .. } => "Recover installation",
        }
    }
    fn changes_setup(&self) -> bool {
        matches!(
            self,
            Self::Link { .. }
                | Self::Join { .. }
                | Self::Pair { .. }
                | Self::Revoke { .. }
                | Self::ApprovePairing { .. }
        )
    }

    fn is_parallel_projectless_setting(&self) -> bool {
        matches!(
            self,
            Self::SetDefaultSupabase { .. }
                | Self::SetDeviceName { .. }
                | Self::SetProjectsLocation { .. }
                | Self::SetManualPublicEndpoint { .. }
        )
    }
}

#[derive(Clone, Serialize)]
pub struct Job {
    pub job_id: Uuid,
    pub project_id: Option<Uuid>,
    pub label: String,
    pub running: bool,
    pub outcome: Option<Value>,
    pub error: Option<String>,
    pub completed_at: Option<u64>,
}

fn prune_finished_jobs(jobs: &mut Vec<Job>) {
    while jobs.iter().filter(|job| !job.running).count() > 32 {
        let oldest = jobs
            .iter()
            .enumerate()
            .filter(|(_, job)| !job.running)
            .min_by_key(|(_, job)| job.completed_at.unwrap_or(0))
            .map(|(index, _)| index);
        if let Some(index) = oldest {
            jobs.remove(index);
        } else {
            break;
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PairingApprovalState {
    #[default]
    Unknown,
    Pending,
    NotReachable,
    Approved,
}

#[derive(Clone, Serialize)]
pub struct PairingApproval {
    pub project_id: Uuid,
    pub device_id: Uuid,
    pub state: PairingApprovalState,
}

#[derive(Clone, Serialize)]
pub struct Device {
    device_id: Uuid,
    name: String,
    peer_id: String,
}

#[derive(Serialize)]
pub struct CheckView {
    pub device_id: Uuid,
    pub result: PeerCheck,
    pub fresh: bool,
    pub current_known: bool,
    pub age_seconds: u64,
}

#[derive(Serialize, Clone)]
pub struct PluginHandoff {
    pub revision: u64,
    pub project_id: Option<Uuid>,
    pub offset: u32,
}

#[derive(Serialize)]
pub struct DesktopStatus {
    pub plugin_handoff: Option<PluginHandoff>,
    pub pairing: PairingStatus,
    pub listen_port: u16,
    pub configured_external_addresses: Vec<String>,
    pub ready: bool,
    pub closing: bool,
    pub device: Option<Device>,
    pub data_directory: Option<String>,
    pub projects_location: String,
    pub error: Option<String>,
    pub network: Option<NetworkStatus>,
    pub peer_states: HashMap<String, PeerConnectionState>,
    pub port_status: Option<PortObservationStatus>,
    pub network_observation: NetworkObservation,
    pub firewall_observation: FirewallObservation,
    pub page: Option<SummaryPage>,
    pub jobs: Vec<Job>,
    pub checks: Vec<CheckView>,
    pub pairing_approvals: Vec<PairingApproval>,
    pub project_backends: HashMap<Uuid, ProjectBackendConfig>,
    pub default_supabase: Option<SupabaseDefaultConfig>,
    pub supabase_states: HashMap<Uuid, SupabaseProjectState>,
    pub invite_pending: bool,
    pub upnp_mapping_asked: bool,
    pub manual_public_endpoint: Option<trackmesh_core::ManualPublicEndpoint>,
    pub invite_needs_service_fields: bool,
}

struct Checked {
    result: PeerCheck,
    peer_id: String,
    frontier: Vec<(Uuid, Option<Uuid>)>,
    connected_since_check: bool,
    at: Instant,
}

#[derive(Default)]
struct Session {
    plugin_handoff: Option<PluginHandoff>,
    work_epoch: u64,
    window_session: bool,
    tray_session: bool,
    close_prompt_active: bool,
    engine: Option<Arc<Engine>>,
    device: Option<Device>,
    peer: Option<PeerRuntime>,
    network: Option<Network>,
    network_observation: NetworkObservation,
    firewall_observation: FirewallObservation,
    metadata: Option<MailboxRuntime>,
    plugin: Option<PluginRuntime>,
    tasks: Vec<tauri::async_runtime::JoinHandle<()>>,
    jobs: Vec<Job>,
    checks: HashMap<(Uuid, Uuid), Checked>,
    ready: bool,
    closing: bool,
    closed: bool,
    error: Option<String>,
    pending_invite: Option<PendingInviteLink>,
    auto_receive_attempted: HashSet<(Uuid, Uuid)>,
    head_pulled_at: HashMap<(Uuid, Uuid), Instant>,
    pairing_approval_states: HashMap<(Uuid, Uuid), PairingApprovalState>,
    pairing_approval_checked_at: HashMap<(Uuid, Uuid), Instant>,
    pairing_approval_in_flight: HashSet<(Uuid, Uuid)>,
}

struct PendingInviteLink {
    id: String,
    key: zeroize::Zeroizing<[u8; 32]>,
    service: Option<PendingInviteService>,
    parent_folder: Option<PathBuf>,
}

#[derive(Clone)]
struct PendingInviteService {
    url: String,
    publishable_key: String,
}

pub struct Desktop {
    options: Options,
    session: Mutex<Session>,
    // At most one slow status read per window; operation tasks never hold this gate.
    status_gate: tokio::sync::Mutex<()>,
}

impl Desktop {
    pub async fn storage_summary(&self) -> Result<trackmesh_core::StorageSummary> {
        let engine = self.lock()?.engine.clone().context("Desktop is starting")?;
        tauri::async_runtime::spawn_blocking(move || engine.storage_summary()).await?
    }
    pub fn new(options: Options) -> Self {
        let window_session = !options.plugin_owner;
        Self {
            options,
            session: Mutex::new(Session {
                window_session,
                ..Session::default()
            }),
            status_gate: tokio::sync::Mutex::new(()),
        }
    }
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Session>> {
        self.session
            .lock()
            .map_err(|_| anyhow::anyhow!("Desktop session lock poisoned"))
    }
    fn set_pairing_approval(&self, project: Uuid, device: Uuid, approval: PairingApprovalState) {
        if let Ok(mut state) = self.lock() {
            state
                .pairing_approval_states
                .insert((project, device), approval);
        }
    }
    pub fn is_closed(&self) -> bool {
        self.session.lock().is_ok_and(|s| s.closed)
    }
    pub fn start(self: &Arc<Self>, app: AppHandle) {
        let desktop = self.clone();
        // Insert the handle while holding the same gate used by close requests.
        if let Ok(mut state) = self.lock() {
            state.tasks.push(tauri::async_runtime::spawn(async move {
                if let Err(error) = desktop.initialize().await {
                    if let Ok(mut state) = desktop.lock() {
                        state.error = Some(error.to_string());
                    }
                }
            }));
        }
        let desktop = self.clone();
        tauri::async_runtime::spawn(async move {
            let mut grace = SessionGrace::default();
            let mut work_epoch = 0;
            let mut heartbeat = tokio::time::interval(Duration::from_secs(1));
            loop {
                heartbeat.tick().await;
                let request = desktop.lock().ok().and_then(|state| {
                    state
                        .plugin
                        .as_ref()
                        .and_then(PluginRuntime::take_desktop_request)
                });
                if let Some(request) = request {
                    desktop.reopen(&app);
                    let engine = desktop.lock().ok().and_then(|state| state.engine.clone());
                    let project_id = request.project_id;
                    let offset = if let (Some(engine), Some(project)) = (engine, project_id) {
                        tauri::async_runtime::spawn_blocking(move || -> Result<u32> {
                            let mut offset = 0;
                            loop {
                                let page = engine.store.projects_page(offset, 32)?;
                                if page.iter().any(|item| item.project_id == project)
                                    || page.len() < 32
                                {
                                    return Ok(offset);
                                }
                                offset = offset.checked_add(32).context("Project page overflow")?;
                            }
                        })
                        .await
                        .ok()
                        .and_then(Result::ok)
                        .unwrap_or(0)
                    } else {
                        0
                    };
                    if let Ok(mut state) = desktop.lock() {
                        if !state.closing && !state.closed {
                            state.plugin_handoff = Some(PluginHandoff {
                                revision: state
                                    .plugin_handoff
                                    .as_ref()
                                    .map_or(1, |old| old.revision.wrapping_add(1)),
                                project_id,
                                offset,
                            });
                        }
                    }
                }
                let engine = match desktop.lock() {
                    Ok(state) if state.closing || state.closed => break,
                    Ok(state) => state.engine.clone(),
                    Err(_) => break,
                };
                let pending = match engine {
                    Some(engine) => tauri::async_runtime::spawn_blocking(move || {
                        engine.has_pending_publication()
                    })
                    .await
                    .ok()
                    .and_then(Result::ok)
                    .unwrap_or(true),
                    None => true,
                };
                let should_stop = desktop.lock().is_ok_and(|mut state| {
                    if work_epoch != state.work_epoch {
                        grace = SessionGrace::default();
                        work_epoch = state.work_epoch;
                    }
                    if state.window_session
                        || state.tray_session
                        || state.close_prompt_active
                        || state.closing
                    {
                        grace = SessionGrace::default();
                        return false;
                    }
                    if state.error.is_some() {
                        state.closing = true;
                        return true;
                    }
                    let (Some(plugin), Some(peer)) = (&state.plugin, &state.peer) else {
                        return false;
                    };
                    let failed = plugin.is_finished()
                        || peer.is_finished()
                        || state
                            .metadata
                            .as_ref()
                            .is_some_and(MailboxRuntime::is_finished);
                    let busy =
                        pending || state.tasks.iter().any(|task| !task.inner().is_finished());
                    if failed || (grace.ready(plugin, peer, busy) && plugin.close_if_idle()) {
                        // Same gate as desktop submit/reopen: no new operation can enter.
                        plugin.begin_close();
                        state.closing = true;
                        true
                    } else {
                        false
                    }
                });
                if should_stop {
                    desktop.finish_close(app.clone()).await;
                    break;
                }
            }
        });
    }
    async fn initialize(&self) -> Result<()> {
        let options = self.options.clone();
        let engine = tauri::async_runtime::spawn_blocking(move || -> Result<_> {
            let directory = options
                .data_dir
                .map(Ok)
                .unwrap_or_else(default_data_directory)?;
            let device_name = if options.device_name == "My studio PC" {
                std::env::var("COMPUTERNAME").unwrap_or(options.device_name)
            } else {
                options.device_name
            };
            Ok(Arc::new(Engine::open(directory, &device_name)?))
        })
        .await??;
        let device = Device {
            device_id: engine.identity.device_id,
            name: engine.identity.name(),
            peer_id: network::peer_id(&engine.identity)?,
        };
        {
            let mut state = self.lock()?;
            state.engine = Some(engine.clone());
            state.device = Some(device);
            if state.closing {
                return Ok(());
            }
        }
        let engine_for_network = engine.clone();
        let options = self.options.clone();
        let (peer, network_observation, firewall_observation) =
            tauri::async_runtime::spawn_blocking(move || {
                let observation = observe_network();
                let firewall = observe_firewall(options.port);
                let config = NetworkConfig {
                    port: options.port,
                    external_addresses: options
                        .external_address
                        .iter()
                        .map(|v| v.parse())
                        .collect::<std::result::Result<_, _>>()?,
                    ..Default::default()
                };
                Ok::<_, anyhow::Error>((
                    PeerRuntime::start(engine_for_network, config, observation.clone(), true)?,
                    observation,
                    firewall,
                ))
            })
            .await??;
        let metadata = MailboxRuntime::start(engine.clone(), self.options.allow_loopback_http);
        let mut state = self.lock()?;
        state.network = Some(peer.network.clone());
        state.network_observation = network_observation;
        state.firewall_observation = firewall_observation;
        state.peer = Some(peer);
        state.metadata = Some(metadata);
        state.plugin = Some(PluginRuntime::start_with_desktop(
            engine,
            state.network.clone().context("Peer runtime missing")?,
            self.options.allow_loopback_http,
            &self.options.ipc_suffix,
        )?);
        if state.closing {
            if let Some(plugin) = &state.plugin {
                plugin.begin_close();
            }
        }
        state.ready = !state.closing;
        Ok(())
    }

    pub async fn status(&self, offset: u32) -> Result<DesktopStatus> {
        let _read = self.status_gate.lock().await;
        let engine = self.lock()?.engine.clone();
        let data_directory = engine
            .as_ref()
            .map(|engine| documents::display_path(&engine.directory));
        let projects_location = if let Some(engine) = engine.as_ref() {
            let disk = engine.clone();
            tauri::async_runtime::spawn_blocking(move || {
                Ok::<_, anyhow::Error>(disk.projects_location()?.map(PathBuf::from))
            })
            .await??
            .map(Ok)
            .unwrap_or_else(documents::default_projects_location)?
        } else {
            tauri::async_runtime::spawn_blocking(documents::default_projects_location).await??
        };
        let (page, pairing, project_backends, default_supabase) = match engine {
            Some(engine) => {
                tauri::async_runtime::spawn_blocking(move || -> Result<_> {
                    let page = engine.summary_page(offset)?;
                    let project_backends = page
                        .projects
                        .iter()
                        .map(|item| {
                            Ok((
                                item.project.project_id,
                                engine.project_backend_config(item.project.project_id)?,
                            ))
                        })
                        .collect::<Result<HashMap<_, _>>>()?;
                    let default_supabase = engine.default_supabase_config()?;
                    Ok((
                        Some(page),
                        engine.pairing_status()?,
                        project_backends,
                        default_supabase,
                    ))
                })
                .await??
            }
            None => (None, PairingStatus::default(), HashMap::new(), None),
        };
        let state_engine = { self.lock()?.engine.clone() };
        let supabase_states = match state_engine.clone() {
            Some(engine) => {
                let ids = project_backends.keys().copied().collect::<Vec<_>>();
                tauri::async_runtime::spawn_blocking(move || {
                    ids.into_iter()
                        .map(|id| Ok((id, engine.supabase_project_state(id)?)))
                        .collect::<Result<HashMap<_, _>>>()
                })
                .await??
            }
            None => HashMap::new(),
        };
        let upnp_mapping_asked = match state_engine.clone() {
            Some(engine) => {
                tauri::async_runtime::spawn_blocking(move || engine.upnp_mapping_asked()).await??
            }
            None => false,
        };
        let manual_public_endpoint = match state_engine {
            Some(engine) => {
                tauri::async_runtime::spawn_blocking(move || engine.manual_public_endpoint())
                    .await??
            }
            None => None,
        };
        let mut state = self.lock()?;
        let invite_needs_service_fields = state
            .pending_invite
            .as_ref()
            .is_some_and(|pending| pending.service.is_none())
            && default_supabase.is_none();
        let mut network = state.network.as_ref().map(Network::status);
        if state.peer.as_ref().is_some_and(PeerRuntime::is_finished) {
            if let Some(network) = &mut network {
                network.peers.clear();
                network.addresses.clear();
                network.error = Some("Peer runtime stopped".to_owned());
            }
        }
        // Keep known lineage visible. A disconnect invalidates equality even after reconnect.
        for checked in state.checks.values_mut() {
            checked.connected_since_check &= network
                .as_ref()
                .is_some_and(|net| net.peers.get(&checked.peer_id).is_some_and(|p| p.online));
        }
        let checks = state
            .checks
            .iter()
            .filter(|((project, _), _)| {
                page.as_ref().is_some_and(|page| {
                    page.projects
                        .iter()
                        .any(|p| p.project.project_id == *project)
                })
            })
            .map(|((_, device_id), checked)| {
                let head_matches = page.as_ref().is_some_and(|page| {
                    page.projects.iter().any(|p| {
                        p.project.project_id == checked.result.project_id
                            && p.project.latest_local_snapshot == checked.result.local_snapshot
                            && metadata_frontier(p) == checked.frontier
                    })
                });
                CheckView {
                    device_id: *device_id,
                    result: checked.result.clone(),
                    fresh: !state.closing
                        && head_matches
                        && checked.connected_since_check
                        && checked.at.elapsed() < Duration::from_secs(30),
                    current_known: head_matches,
                    age_seconds: checked.at.elapsed().as_secs(),
                }
            })
            .collect();
        if let Some(network) = &mut network {
            network.peers.retain(|id, _| {
                page.as_ref().is_some_and(|page| {
                    page.projects
                        .iter()
                        .any(|project| project.peers.iter().any(|peer| peer.device.peer_id == *id))
                })
            });
            network.addresses.truncate(32);
            network.received_snapshots.retain(|receipt| {
                page.as_ref().is_some_and(|page| {
                    page.projects.iter().any(|project| {
                        project.project.project_id == receipt.project_id
                            && project
                                .peers
                                .iter()
                                .any(|peer| peer.device.peer_id == receipt.peer_id)
                    })
                })
            });
        }
        let runtime_error = state
            .peer
            .as_ref()
            .filter(|peer| peer.is_finished())
            .map(|_| "Peer runtime stopped; reopen the desktop to reconnect".to_owned())
            .or_else(|| {
                state
                    .metadata
                    .as_ref()
                    .filter(|worker| worker.is_finished())
                    .map(|_| "Metadata worker stopped; reopen the desktop to retry".to_owned())
            });
        let peer_states = page
            .as_ref()
            .zip(network.as_ref())
            .map(|(page, network)| {
                page.projects
                    .iter()
                    .flat_map(|project| project.peers.iter())
                    .map(|peer| {
                        (
                            peer.device.peer_id.clone(),
                            peer_connection_state(network, &peer.device.peer_id),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let port_status = network.as_ref().map(port_observation_status);
        let device = state.device.clone().map(|mut device| {
            if let Some(engine) = &state.engine {
                device.name = engine.identity.name();
            }
            device
        });
        let pairing_approvals = page
            .as_ref()
            .into_iter()
            .flat_map(|page| page.projects.iter())
            .flat_map(|project| {
                project.peers.iter().map(|peer| PairingApproval {
                    project_id: project.project.project_id,
                    device_id: peer.device.device_id,
                    state: state
                        .pairing_approval_states
                        .get(&(project.project.project_id, peer.device.device_id))
                        .copied()
                        .unwrap_or_default(),
                })
            })
            .collect();
        Ok(DesktopStatus {
            plugin_handoff: state.plugin_handoff.clone(),
            pairing,
            listen_port: self.options.port,
            configured_external_addresses: self.options.external_address.clone(),
            ready: state.ready,
            closing: state.closing,
            device,
            data_directory,
            projects_location: documents::display_path(&projects_location),
            error: state.error.clone().or(runtime_error),
            network,
            peer_states,
            port_status,
            network_observation: state
                .peer
                .as_ref()
                .map(PeerRuntime::network_observation)
                .unwrap_or_else(|| state.network_observation.clone()),
            firewall_observation: state.firewall_observation.clone(),
            page,
            jobs: state.jobs.clone(),
            checks,
            pairing_approvals,
            project_backends,
            default_supabase,
            supabase_states,
            invite_pending: state.pending_invite.is_some(),
            upnp_mapping_asked,
            invite_needs_service_fields,
            manual_public_endpoint,
        })
    }

    pub async fn try_auto_first_receive(
        self: &Arc<Self>,
        app: AppHandle,
        status: &DesktopStatus,
    ) -> Result<()> {
        let Some(page) = status.page.as_ref() else {
            return Ok(());
        };
        let (network, engine) = {
            let state = self.lock()?;
            let (Some(network), Some(engine)) = (&state.network, &state.engine) else {
                return Ok(());
            };
            (network.clone(), engine.clone())
        };
        for item in &page.projects {
            if item.local_snapshot.is_some()
                || item.project.latest_local_snapshot.is_some()
                || item.project.recovery_required
                || !item.root_available
                || !item.root_empty
                || item.active_transfer
            {
                continue;
            }
            if self.lock()?.jobs.iter().any(|job| {
                job.running
                    && (job.project_id.is_none() || job.project_id == Some(item.project.project_id))
            }) {
                continue;
            }
            for peer in &item.peers {
                if !network.online(&peer.device.peer_id) {
                    continue;
                }
                let Some(snapshot) = peer.announcement.as_ref() else {
                    // Without a mailbox nothing pushes the collaborator's head; pull it with
                    // the existing Head request, at most every 10 s per project and peer.
                    let key = (item.project.project_id, peer.device.device_id);
                    {
                        let mut state = self.lock()?;
                        if state
                            .head_pulled_at
                            .get(&key)
                            .is_some_and(|at| at.elapsed() < Duration::from_secs(10))
                        {
                            continue;
                        }
                        state.head_pulled_at.insert(key, Instant::now());
                    }
                    let (engine, network) = (engine.clone(), network.clone());
                    let (project, source) = (item.project.project_id, peer.device.peer_id.clone());
                    let desktop = self.clone();
                    let device = peer.device.device_id;
                    tauri::async_runtime::spawn(async move {
                        if engine
                            .fetch_snapshot_chain(&network, project, &source, None)
                            .await
                            .is_ok()
                        {
                            desktop.set_pairing_approval(
                                project,
                                device,
                                PairingApprovalState::Approved,
                            );
                        }
                    });
                    continue;
                };
                let key = (item.project.project_id, snapshot.snapshot_id);
                {
                    let mut state = self.lock()?;
                    if state.auto_receive_attempted.contains(&key)
                        || state.jobs.iter().any(|job| {
                            job.running
                                && (job.project_id.is_none()
                                    || job.project_id == Some(item.project.project_id))
                        })
                    {
                        continue;
                    }
                    state.auto_receive_attempted.insert(key);
                }
                // Uses the same bounded receive job path as the manual Get action.
                let _ = self.submit(
                    app.clone(),
                    Action::Receive {
                        project_id: item.project.project_id,
                        device_id: peer.device.device_id,
                        snapshot_id: Some(snapshot.snapshot_id),
                        daw_closed: true,
                    },
                );
                break;
            }
        }
        Ok(())
    }

    pub async fn try_pairing_approval_status(
        self: &Arc<Self>,
        status: &DesktopStatus,
    ) -> Result<()> {
        let Some(page) = status.page.as_ref() else {
            return Ok(());
        };
        let (network, engine) = {
            let state = self.lock()?;
            let (Some(network), Some(engine)) = (&state.network, &state.engine) else {
                return Ok(());
            };
            (network.clone(), engine.clone())
        };
        for item in &page.projects {
            if item.local_snapshot.is_some()
                || item.project.latest_local_snapshot.is_some()
                || item.active_transfer
            {
                continue;
            }
            for peer in &item.peers {
                if peer.head_fetched {
                    continue;
                }
                let key = (item.project.project_id, peer.device.device_id);
                {
                    let mut state = self.lock()?;
                    // An expired or closed window drops the connection, so a pending
                    // receiver must keep asking to learn that approval is no longer coming.
                    let approval = state
                        .pairing_approval_states
                        .get(&key)
                        .copied()
                        .unwrap_or_default();
                    if approval == PairingApprovalState::Approved
                        || (approval != PairingApprovalState::Pending
                            && !network.online(&peer.device.peer_id))
                        || state.pairing_approval_in_flight.contains(&key)
                        || state.jobs.iter().any(|job| {
                            job.running && job.project_id == Some(item.project.project_id)
                        })
                        || state
                            .pairing_approval_checked_at
                            .get(&key)
                            .is_some_and(|at| at.elapsed() < Duration::from_secs(10))
                    {
                        continue;
                    }
                    state.pairing_approval_in_flight.insert(key);
                }
                let desktop = self.clone();
                let engine = engine.clone();
                let network = network.clone();
                let (project, device) = key;
                tauri::async_runtime::spawn(async move {
                    let approval = match engine.pairing_approval(&network, project, device).await {
                        Ok(protocol::PairingResponse::Approved) => PairingApprovalState::Approved,
                        Ok(protocol::PairingResponse::Pending) => PairingApprovalState::Pending,
                        _ => PairingApprovalState::NotReachable,
                    };
                    if let Ok(mut state) = desktop.lock() {
                        // A Check, head pull or receive may have proven approval meanwhile.
                        let current = state
                            .pairing_approval_states
                            .entry((project, device))
                            .or_default();
                        if *current != PairingApprovalState::Approved {
                            *current = approval;
                        }
                        state.pairing_approval_in_flight.remove(&(project, device));
                        state
                            .pairing_approval_checked_at
                            .insert((project, device), Instant::now());
                    }
                });
            }
        }
        Ok(())
    }

    pub fn submit(self: &Arc<Self>, app: AppHandle, action: Action) -> Result<Uuid> {
        let mut state = self.lock()?;
        ensure!(
            state.ready && !state.closing && !state.close_prompt_active,
            "Desktop is starting or closing"
        );
        let project = action.project();
        let parallel_setting = action.is_parallel_projectless_setting();
        let conflicting = state.jobs.iter().any(|job| {
            job.running
                && if parallel_setting {
                    job.project_id.is_none()
                } else {
                    job.project_id.is_none() || project.is_none() || job.project_id == project
                }
        });
        if conflicting {
            let projectless_conflict = project.is_none()
                || state
                    .jobs
                    .iter()
                    .any(|job| job.running && job.project_id.is_none());
            anyhow::ensure!(
                !conflicting,
                if projectless_conflict {
                    "Wait for the current operation to finish"
                } else {
                    "This project already has an active operation"
                }
            );
        }
        ensure!(
            state.jobs.iter().filter(|job| job.running).count() < 8,
            "Too many active operations"
        );
        state.work_epoch = state.work_epoch.wrapping_add(1);
        state
            .jobs
            .retain(|job| job.running || job.project_id != project);
        prune_finished_jobs(&mut state.jobs);
        state.tasks.retain(|task| !task.inner().is_finished());
        let job_id = Uuid::new_v4();
        state.jobs.push(Job {
            job_id,
            project_id: project,
            label: action.label().to_owned(),
            running: true,
            outcome: None,
            error: None,
            completed_at: None,
        });
        let desktop = self.clone();
        let engine = state.engine.clone().context("Engine is unavailable")?;
        let network = state.network.clone().context("Network is unavailable")?;
        state.tasks.push(tauri::async_runtime::spawn(async move {
            let worker_desktop = desktop.clone();
            // Join the inner task so a panic becomes a visible completed job, never a stuck gate.
            let result =
                tokio::spawn(
                    async move { worker_desktop.execute(app, engine, network, action).await },
                )
                .await
                .unwrap_or_else(|_| Err(anyhow::anyhow!("Operation worker stopped unexpectedly")));
            if let Ok(mut state) = desktop.lock() {
                if let Some(job) = state.jobs.iter_mut().find(|job| job.job_id == job_id) {
                    job.running = false;
                    job.completed_at = Some(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    );
                    match result {
                        Ok(outcome) => job.outcome = Some(outcome),
                        Err(error) => job.error = Some(error.to_string()),
                    }
                }
                prune_finished_jobs(&mut state.jobs);
            }
        }));
        Ok(job_id)
    }

    pub async fn set_upnp_mapping(&self, enabled: bool) -> Result<()> {
        let (engine, network) = {
            let state = self.lock()?;
            ensure!(
                state.ready && !state.closing,
                "Desktop is starting or closing"
            );
            (
                state.engine.clone().context("Engine is unavailable")?,
                state.network.clone().context("Network is unavailable")?,
            )
        };
        let disk = engine.clone();
        tauri::async_runtime::spawn_blocking(move || disk.set_upnp_mapping_enabled(enabled))
            .await??;
        network.set_mapping_consent(enabled);
        Ok(())
    }

    pub fn cancel_invite_link(&self) -> Result<()> {
        let mut state = self.lock()?;
        ensure!(
            !state
                .jobs
                .iter()
                .any(|job| job.running && job.label == "Accept invite link"),
            "An invite is being accepted; wait for it to finish"
        );
        ensure!(
            state.pending_invite.take().is_some(),
            "No invite link is open"
        );
        Ok(())
    }

    pub async fn open_project_folder(&self, project_id: Uuid) -> Result<()> {
        let engine = self
            .lock()?
            .engine
            .clone()
            .context("Engine is unavailable")?;
        let path = tauri::async_runtime::spawn_blocking(move || {
            Ok::<_, anyhow::Error>(PathBuf::from(engine.store.project(project_id)?.root))
        })
        .await??;
        tauri::async_runtime::spawn_blocking(move || documents::open_in_explorer(&path)).await??;
        Ok(())
    }

    pub async fn open_previous_version(&self, project_id: Uuid, transfer_id: Uuid) -> Result<()> {
        let engine = self
            .lock()?
            .engine
            .clone()
            .context("Engine is unavailable")?;
        let path = tauri::async_runtime::spawn_blocking(move || -> Result<PathBuf> {
            // Same location the apply journal uses: a sibling of the project folder.
            let root = PathBuf::from(engine.store.project(project_id)?.root);
            let stage = transfer::stage_path(&root, project_id, transfer_id)?;
            let backup = stage.join("backup");
            ensure!(backup.is_dir(), "Previous version backup is not available");
            Ok(backup)
        })
        .await??;
        tauri::async_runtime::spawn_blocking(move || documents::open_in_explorer(&path)).await??;
        Ok(())
    }

    pub async fn open_projects_location(&self) -> Result<()> {
        let engine = self
            .lock()?
            .engine
            .clone()
            .context("Engine is unavailable")?;
        let path =
            tauri::async_runtime::spawn_blocking(move || match engine.projects_location()? {
                Some(path) => Ok(PathBuf::from(path)),
                None => documents::default_projects_location(),
            })
            .await??;
        tauri::async_runtime::spawn_blocking(move || documents::open_in_explorer(&path)).await??;
        Ok(())
    }

    pub async fn choose_projects_location(&self, app: AppHandle) -> Result<Option<String>> {
        let engine = self
            .lock()?
            .engine
            .clone()
            .context("Engine is unavailable")?;
        tauri::async_runtime::spawn_blocking(move || {
            let Some(path) = documents::choose_folder(&app)? else {
                return Ok(None);
            };
            engine.set_projects_location(std::path::Path::new(&path))?;
            Ok(Some(path))
        })
        .await?
    }

    pub fn set_invite_parent_folder(&self, folder: PathBuf) -> Result<String> {
        let folder =
            std::fs::canonicalize(folder).context("Selected folder could not be resolved")?;
        ensure!(folder.is_dir(), "Choose a folder for the project");
        let mut state = self.lock()?;
        let pending = state
            .pending_invite
            .as_mut()
            .context("No invitation link is waiting")?;
        pending.parent_folder = Some(folder.clone());
        Ok(documents::display_path(&folder))
    }

    pub async fn open_invite_parent_folder(&self) -> Result<()> {
        let (engine, selected) = {
            let state = self.lock()?;
            (
                state.engine.clone().context("Engine is unavailable")?,
                state
                    .pending_invite
                    .as_ref()
                    .and_then(|pending| pending.parent_folder.clone()),
            )
        };
        let path = if let Some(path) = selected {
            path
        } else {
            tauri::async_runtime::spawn_blocking(move || match engine.projects_location()? {
                Some(path) => Ok(PathBuf::from(path)),
                None => documents::default_projects_location(),
            })
            .await??
        };
        tauri::async_runtime::spawn_blocking(move || documents::open_in_explorer(&path)).await??;
        Ok(())
    }

    async fn execute(
        &self,
        app: AppHandle,
        engine: Arc<Engine>,
        network: Network,
        action: Action,
    ) -> Result<Value> {
        let reload = action.changes_setup();
        let allow_http = self.options.allow_loopback_http;
        let outcome = match action {
            Action::Join { folder } => {
                let disk = engine.clone();
                let (project, device) =
                    tauri::async_runtime::spawn_blocking(move || -> Result<_> {
                        let invite: protocol::ProjectInvite = documents::read_document(&app)?;
                        let project = invite.project_id;
                        let device = invite.device.device_id;
                        disk.import_invite(invite, &folder)?;
                        Ok((project, device))
                    })
                    .await??;
                let disk = engine.clone();
                let namespaces =
                    tauri::async_runtime::spawn_blocking(move || disk.namespaces()).await??;
                network
                    .reload(namespaces)
                    .await
                    .context("Invitation accepted, but network reload failed; reopen desktop")?;
                // Import is committed even when the inviter is offline or their window is closed.
                // Keep a retry action instead of asking the user to reimport the project.
                self.automatic_pairing(&engine, &network, project, device, true)
                    .await
            }
            Action::CompleteInviteLink {
                supabase_url,
                supabase_anon_key,
            } => {
                let (id, key, pending_parent, linked_service) = {
                    let state = self.lock()?;
                    let pending = state
                        .pending_invite
                        .as_ref()
                        .context("No invitation link is waiting")?;
                    (
                        pending.id.clone(),
                        pending.key.clone(),
                        pending.parent_folder.clone(),
                        pending.service.clone(),
                    )
                };
                let disk = engine.clone();
                let configured =
                    tauri::async_runtime::spawn_blocking(move || disk.default_supabase_config())
                        .await??;
                let save_default_after_validation = configured.is_none();
                let config = match (linked_service, configured) {
                    (Some(service), _) => SupabaseDefaultConfig {
                        url: service.url,
                        publishable_key: service.publishable_key,
                    },
                    (None, Some(config)) => config,
                    // Saved as the default only after the signed invite confirms this service.
                    (None, None) => SupabaseDefaultConfig {
                        url: supabase_url.context("Enter the Supabase Project URL")?,
                        publishable_key: supabase_anon_key
                            .context("Enter the Supabase publishable key")?,
                    },
                };
                let parent = if let Some(parent) = pending_parent {
                    parent
                } else if let Some(location) = engine.projects_location()? {
                    PathBuf::from(location)
                } else {
                    tauri::async_runtime::spawn_blocking(documents::default_projects_location)
                        .await??
                };
                let parent = tauri::async_runtime::spawn_blocking(move || -> Result<PathBuf> {
                    std::fs::create_dir_all(&parent)
                        .context("The selected Projects location could not be created")?;
                    let parent = std::fs::canonicalize(parent)
                        .context("The selected Projects location could not be resolved")?;
                    ensure!(parent.is_dir(), "Choose a folder for the project");
                    Ok(parent)
                })
                .await??;
                let downloaded = mailbox::SupabaseClient::consume_invite_drop(
                    &config.url,
                    &config.publishable_key,
                    &id,
                )
                .await
                .map_err(|_| anyhow::anyhow!("Invite link is invalid, expired, or unavailable"))?;
                let ciphertext = zeroize::Zeroizing::new(downloaded.ciphertext);
                let mut invite =
                    crypto::decrypt_invite_drop(&id, &downloaded.expires_at, &ciphertext, &key)
                        .map_err(|_| {
                            anyhow::anyhow!("Invite link is invalid, expired, or unavailable")
                        })?;
                if crypto::verify_project_invite(&invite).is_err() {
                    crypto::zeroize_project_invite(&mut invite);
                    anyhow::bail!("Invite link is invalid, expired, or unavailable");
                }
                let signed_backend_matches = invite.backend.as_ref().is_some_and(|backend| {
                    backend.backend == protocol::InviteBackend::Supabase
                        && backend.supabase_anon_key.as_str() == config.publishable_key.as_str()
                        && mailbox::supabase_project_host(&backend.supabase_url).ok()
                            == mailbox::supabase_project_host(&config.url).ok()
                });
                if !signed_backend_matches {
                    crypto::zeroize_project_invite(&mut invite);
                    anyhow::bail!("Invite link is invalid, expired, or unavailable");
                }
                if save_default_after_validation {
                    let disk = engine.clone();
                    let saved = config.clone();
                    tauri::async_runtime::spawn_blocking(move || {
                        disk.set_default_supabase_config(Some(saved))
                    })
                    .await??;
                }
                if let Ok(mut state) = self.lock() {
                    state.pending_invite = None;
                }
                let name = sanitize_project_name(&invite.project_name);
                let folder = tauri::async_runtime::spawn_blocking(move || {
                    create_unique_invite_folder(&parent, &name)
                })
                .await??;
                let project = invite.project_id;
                let device = invite.device.device_id;
                let disk = engine.clone();
                tauri::async_runtime::spawn_blocking(move || disk.import_invite(invite, &folder))
                    .await??;
                let disk = engine.clone();
                let namespaces =
                    tauri::async_runtime::spawn_blocking(move || disk.namespaces()).await??;
                network
                    .reload(namespaces)
                    .await
                    .context("Invite accepted, but network reload failed; reopen desktop")?;
                self.automatic_pairing(&engine, &network, project, device, true)
                    .await
            }
            Action::RequestPairing {
                project_id,
                device_id,
            } => {
                self.automatic_pairing(&engine, &network, project_id, device_id, false)
                    .await
            }
            Action::Send { project_id } => serde_json::to_value(
                engine
                    .send_snapshot(&network, project_id, allow_http)
                    .await?,
            )?,
            Action::ApprovePairing {
                project_id,
                request_id,
            } => {
                let disk = engine.clone();
                let (had_snapshot, root_empty) = tauri::async_runtime::spawn_blocking(move || {
                    let summary = disk.project_summary(project_id)?;
                    Ok::<_, anyhow::Error>((summary.local_snapshot.is_some(), summary.root_empty))
                })
                .await??;
                let disk = engine.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    disk.approve_pairing(project_id, request_id)
                })
                .await??;
                if !had_snapshot && root_empty {
                    json!({"message": "Collaborator approved. Add your project files, then Share."})
                } else if !had_snapshot {
                    let shared = engine
                        .send_snapshot(&network, project_id, allow_http)
                        .await?;
                    json!({"message": "Collaborator approved. First version shared and ready to receive.", "snapshot_id": shared.snapshot_id})
                } else {
                    json!({"message": "Collaborator approved for this project."})
                }
            }
            Action::CopyInviteLink { project_id } => {
                let disk = engine.clone();
                let config = tauri::async_runtime::spawn_blocking(move || {
                    disk.project_backend_config(project_id)
                })
                .await??;
                ensure!(
                    config.backend == ProjectBackend::Supabase,
                    "Select and save Supabase for this project before creating an invite link"
                );
                let url = config.supabase_url.context("Supabase is not configured")?;
                let anon_key = config
                    .supabase_anon_key
                    .context("Supabase is not configured")?;
                mailbox::validate_supabase_project_url(&url)?;
                mailbox::validate_supabase_publishable_key(&anon_key)?;
                let ref_name = invite_service_ref(&url)?;
                wait_for_mapping_if_requested(&network).await?;
                let addresses = invite_addresses(
                    &engine,
                    &network,
                    &self.options.external_address,
                    Vec::new(),
                )
                .await?;
                let disk = engine.clone();
                let mut invite = tauri::async_runtime::spawn_blocking(move || {
                    disk.export_invite(project_id, addresses)
                })
                .await??;
                let sealed_result = crypto::encrypt_invite_drop(&invite);
                crypto::zeroize_project_invite(&mut invite);
                let sealed = sealed_result?;
                let client = mailbox::SupabaseClient::new(&url, &anon_key, project_id)?;
                let accepted = client
                    .create_invite_drop(
                        &engine.identity,
                        &sealed.id,
                        &sealed.expires_at,
                        &sealed.ciphertext,
                    )
                    .await
                    .map_err(|_| anyhow::anyhow!("Invite link could not be created"))?;
                ensure!(accepted, "Invite link could not be created");
                let encoded_key = zeroize::Zeroizing::new(base64::Engine::encode(
                    &base64::engine::general_purpose::URL_SAFE_NO_PAD,
                    sealed.key.as_slice(),
                ));
                let link = zeroize::Zeroizing::new(format!(
                    "trackmesh://invite/v2/{}/{}/{}#{}",
                    ref_name,
                    anon_key,
                    sealed.id,
                    encoded_key.as_str(),
                ));
                let disk = engine.clone();
                let replaced = tauri::async_runtime::spawn_blocking(move || {
                    disk.open_pairing_window(project_id)
                })
                .await??;
                let cleared_pending = match replaced {
                    Some(other) => Some(engine.store.project(other)?.name),
                    None => None,
                };
                network
                    .pairing_window(trackmesh_core::PAIRING_WINDOW)
                    .await?;
                let clipboard_value = zeroize::Zeroizing::new(link.to_string());
                let sequence = tauri::async_runtime::spawn_blocking(move || {
                    crate::clipboard::copy_invite_link(&clipboard_value)
                })
                .await??;
                let cleanup_value = zeroize::Zeroizing::new(link.to_string());
                let cleanup = tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(60)).await;
                    let _ = tauri::async_runtime::spawn_blocking(move || {
                        crate::clipboard::clear_if_matches(&cleanup_value, sequence)
                    })
                    .await;
                });
                if let Ok(mut state) = self.lock() {
                    state.tasks.push(cleanup);
                }
                json!({"message": "Invite link copied — paste it within 60 s", "invite_copied": true, "cleared_pending": cleared_pending})
            }
            Action::Publish { project_id } => {
                serde_json::to_value(engine.publish_metadata(project_id, allow_http).await?)?
            }
            Action::Refresh { project_id } => {
                serde_json::to_value(engine.refresh_metadata(project_id, allow_http).await?)?
            }
            Action::Check {
                project_id,
                device_id,
            } => {
                let disk = engine.clone();
                let peer = tauri::async_runtime::spawn_blocking(move || {
                    disk.paired_peer(project_id, device_id)
                })
                .await??;
                wait_for_peer(self, &network, &peer.peer_id).await?;
                let result = engine
                    .check_peer(&network, project_id, &peer.peer_id)
                    .await?;
                self.set_pairing_approval(project_id, device_id, PairingApprovalState::Approved);
                let outcome = serde_json::to_value(&result)?;
                let disk = engine.clone();
                let summary =
                    tauri::async_runtime::spawn_blocking(move || disk.project_summary(project_id))
                        .await??;
                let mut state = self.lock()?;
                if state.checks.len() >= 64 {
                    if let Some(oldest) = state
                        .checks
                        .iter()
                        .min_by_key(|(_, check)| check.at)
                        .map(|(key, _)| *key)
                    {
                        state.checks.remove(&oldest);
                    }
                }
                state.checks.insert(
                    (project_id, device_id),
                    Checked {
                        result,
                        peer_id: peer.peer_id,
                        frontier: metadata_frontier(&summary),
                        connected_since_check: true,
                        at: Instant::now(),
                    },
                );
                outcome
            }
            Action::Receive {
                project_id,
                device_id,
                snapshot_id,
                daw_closed,
            } => {
                ensure!(daw_closed, "Close the DAW project before receiving");
                let disk = engine.clone();
                let peer = tauri::async_runtime::spawn_blocking(move || {
                    disk.paired_peer(project_id, device_id)
                })
                .await??;
                wait_for_peer(self, &network, &peer.peer_id).await?;
                let peer_device = peer.device_id;
                match self
                    .receive_with_confirmation(
                        &app,
                        &engine,
                        &network,
                        project_id,
                        peer,
                        snapshot_id,
                        false,
                    )
                    .await
                {
                    Ok(transfer_id) => {
                        self.set_pairing_approval(
                            project_id,
                            peer_device,
                            PairingApprovalState::Approved,
                        );
                        json!({"transfer_id": transfer_id})
                    }
                    Err(error) => receive_blocked_or_error(error)?,
                }
            }
            Action::UsePeerVersion {
                project_id,
                device_id,
                daw_closed,
            } => {
                ensure!(daw_closed, "Close the DAW project before receiving");
                let disk = engine.clone();
                let peer = tauri::async_runtime::spawn_blocking(move || {
                    disk.paired_peer(project_id, device_id)
                })
                .await??;
                wait_for_peer(self, &network, &peer.peer_id).await?;
                match self
                    .receive_with_confirmation(
                        &app, &engine, &network, project_id, peer, None, true,
                    )
                    .await
                {
                    Ok(transfer_id) => json!({"transfer_id": transfer_id}),
                    Err(error) => receive_blocked_or_error(error)?,
                }
            }
            Action::Recover {
                project_id,
                daw_closed,
            } => {
                ensure!(daw_closed, "Close the DAW project before recovery");
                let disk = engine.clone();
                tauri::async_runtime::spawn_blocking(move || disk.recover(project_id)).await??;
                json!({"message": "Local recovery completed"})
            }
            action => {
                let disk = engine.clone();
                let dialog_app = app.clone();
                let invite_export = matches!(&action, Action::ExportInvite { .. });
                if invite_export {
                    wait_for_mapping_if_requested(&network).await?;
                }
                let explicit = match &action {
                    Action::ExportInvite {
                        public_ip: Some(ip),
                        external_port: Some(port),
                        ..
                    } => trackmesh_core::public_ipv4_addresses(ip, *port)?,
                    Action::ExportInvite {
                        public_ip: None,
                        external_port: None,
                        ..
                    }
                    | Action::ExportCard => Vec::new(),
                    Action::ExportInvite { .. } => {
                        anyhow::bail!("Public IPv4 and external port must be supplied together")
                    }
                    _ => Vec::new(),
                };
                let addresses =
                    if matches!(&action, Action::ExportInvite { .. } | Action::ExportCard) {
                        invite_addresses(
                            &engine,
                            &network,
                            &self.options.external_address,
                            explicit,
                        )
                        .await?
                    } else {
                        Vec::new()
                    };
                tauri::async_runtime::spawn_blocking(move || -> Result<Value> {
                    match action {
                        Action::DeleteBackups { selected } => {
                            let total = selected.iter().try_fold(0u64, |sum, item| sum.checked_add(item.bytes)).context("backup size overflow")?;
                            let accepted = dialog_app.dialog().message(format!("Delete {} old backups ({:.2} GB)? Each project's newest backup will be kept.", selected.len(), total as f64 / 1_000_000_000.0))
                                .title("Delete old backups?").buttons(tauri_plugin_dialog::MessageDialogButtons::OkCancel).blocking_show();
                            ensure!(accepted, "Backup deletion cancelled");
                            let bytes = disk.delete_old_backups(&selected)?;
                            Ok(json!({"message": format!("Deleted {bytes} bytes of old backups")}))
                        }
                        Action::Link { project_id, name, folder, mailbox_url, relay_address } => {
                            ensure!(!project_id.is_nil(), "Project UUID cannot be nil");
                            if let Some(relay) = &relay_address { let _: network::NetworkConfig = NetworkConfig { rendezvous: vec![(relay.parse()?, String::new())], ..Default::default() }; }
                            let is_existing = disk.store.projects()?.iter().any(|project| project.project_id == project_id);
                            disk.link(project_id, &name, &folder, mailbox_url.as_deref(), relay_address.as_deref())?;
                            if !is_existing {
                                if let Some(default) = disk.default_supabase_config()? {
                                    disk.set_project_backend_config(project_id, ProjectBackendConfig {
                                        backend: ProjectBackend::Supabase,
                                        supabase_url: Some(default.url),
                                        supabase_anon_key: Some(default.publishable_key),
                                    })?;
                                }
                            }
                            Ok(json!({"message": "Folder linked"}))
                        }
                        Action::ChangeProjectRoot { project_id, folder } => {
                            let differs = disk.project_root_differs(project_id, &folder)?;
                            if differs {
                                let accepted = dialog_app.dialog().message("This folder differs from your last shared version").title("Change project folder?").kind(tauri_plugin_dialog::MessageDialogKind::Warning).blocking_show();
                                ensure!(accepted, "Folder change cancelled");
                            }
                            disk.change_project_root(project_id, &folder, differs)?;
                            Ok(json!({"message": "Project folder updated"}))
                        }
                        Action::SetProjectBackend { project_id, backend, supabase_url, supabase_anon_key } => {
                            disk.set_project_backend_config(project_id, ProjectBackendConfig { backend, supabase_url, supabase_anon_key })?;
                            Ok(json!({"message": "Project backend settings saved"}))
                        }
                        Action::SetDefaultSupabase { url, publishable_key } => {
                            let config = match (url, publishable_key) {
                                (None, None) => None,
                                (Some(url), Some(publishable_key)) => Some(SupabaseDefaultConfig { url, publishable_key }),
                                _ => anyhow::bail!("Enter both the Supabase Project URL and publishable key"),
                            };
                            disk.set_default_supabase_config(config)?;
                            Ok(json!({"message": "Default Supabase settings saved"}))
                        }
                        Action::SetDeviceName { name } => {
                            disk.set_device_name(&name)?;
                            Ok(json!({"message": "Device name saved"}))
                        }
                        Action::SetProjectsLocation { location } => {
                            disk.set_projects_location(&location)?;
                            Ok(json!({"message": "Projects location saved"}))
                        }
                        Action::SetManualPublicEndpoint { public_ipv4, external_port } => {
                            disk.set_manual_public_endpoint(public_ipv4, external_port)?;
                            Ok(json!({"message": "Manual port forwarding settings saved"}))
                        }
                        Action::OpenPairing { project_id } => {
                            let replaced = disk.open_pairing_window(project_id)?;
                            let message = if let Some(other) = replaced {
                                format!("Pending approvals for {} were cleared; only one project can accept new devices at a time.", disk.store.project(other)?.name)
                            } else {
                                "Approval window open for 15 minutes. Ask your collaborator to press Try again, then verify the arriving device before approving.".to_owned()
                            };
                            Ok(json!({"message": message}))
                        }
                        Action::ClosePairing { project_id } => {
                            ensure!(disk.pairing_status()?.project_id == Some(project_id), "No pairing window for this project");
                            disk.close_pairing_window()?;
                            Ok(json!({"message": "Pairing window closed; unapproved requests removed"}))
                        }
                        Action::RejectPairing { project_id, request_id } => {
                            disk.reject_pairing(project_id, request_id)?;
                            Ok(json!({"message": "Pairing request rejected; no project access was granted"}))
                        }
                        Action::Pair { project_id } => {
                            disk.pair(project_id, documents::read_document(&app)?)?;
                            Ok(json!({"message": "Pairing reply accepted. This device is authorized for the selected project. Tell your collaborator pairing is complete, create a snapshot, and keep TrackMesh open."}))
                        }
                        Action::Revoke { project_id, device_id } => {
                            let name = disk.store.peers(Some(project_id))?.into_iter()
                                .find(|peer| peer.device_id == device_id)
                                .map(|peer| peer.name)
                                .unwrap_or_else(|| "device".to_owned());
                            let outcome = disk.revoke(project_id, device_id)?;
                            let message = match outcome {
                                RevokeOutcome::Queued => format!("Removed {name}'s access on this PC. TrackMesh will also remove {name} from the invite service."),
                                RevokeOutcome::ImportedSupabase => format!("Removed {name}'s access on this PC. Only the device that created this project can remove {name} from the invite service."),
                                RevokeOutcome::LocalOnly => format!("Removed {name}'s access on this PC."),
                            };
                            Ok(json!({"message": message}))
                        }
                        Action::ExportInvite { project_id, .. } => {
                            let invite = disk.export_invite(project_id, addresses)?;
                            let path = documents::write_document(&app, "trackmesh-invite.json", &invite)?;
                            let replaced = disk.open_pairing_window(project_id)?;
                            let message = if let Some(other) = replaced {
                                format!("Private invitation saved. Pending approvals for {} were cleared; only one project can accept new devices at a time.", disk.store.project(other)?.name)
                            } else {
                                "Private invitation saved. Automatic pairing window open for 15 minutes; share the invitation privately, then verify and approve your collaborator here.".to_owned()
                            };
                            Ok(json!({"message": message, "path": path}))
                        }
                        Action::ExportCard => {
                            let card = disk.device_card(addresses)?;
                            let path = documents::write_document(&app, "trackmesh-pairing-reply.json", &card)?;
                            Ok(json!({"message": "Pairing reply saved. Send this file privately to the inviter to complete pairing.", "path": path}))
                        }
                        _ => anyhow::bail!("Unsupported document action"),
                    }
                }).await??
            }
        };
        let disk = engine.clone();
        let pairing = tauri::async_runtime::spawn_blocking(move || disk.pairing_status()).await??;
        network
            .pairing_window(Duration::from_secs(pairing.remaining_seconds))
            .await?;
        if reload {
            let disk = engine;
            let namespaces =
                tauri::async_runtime::spawn_blocking(move || disk.namespaces()).await??;
            network
                .reload(namespaces)
                .await
                .context("Project setup saved, but network reload failed; reopen desktop")?;
        }
        Ok(outcome)
    }

    async fn automatic_pairing(
        &self,
        engine: &Arc<Engine>,
        network: &Network,
        project: Uuid,
        device: Uuid,
        imported: bool,
    ) -> Value {
        let result = async {
            let disk = engine.clone();
            let peer =
                tauri::async_runtime::spawn_blocking(move || disk.paired_peer(project, device))
                    .await??;
            wait_for_peer(self, network, &peer.peer_id).await?;
            let addresses =
                invite_addresses(engine, network, &self.options.external_address, Vec::new())
                    .await?;
            engine
                .request_pairing(network, project, device, addresses)
                .await
        }
        .await;
        let (state, approval, message) = match result {
            Ok(protocol::PairingResponse::Pending) => ("pending", PairingApprovalState::Pending, "Pairing request sent automatically. The inviter must verify and approve your device. No card file needs to be sent; wait for their confirmation."),
            Ok(protocol::PairingResponse::Approved) => {
                (
                    "approved",
                    PairingApprovalState::Approved,
                    "The inviter approved this device for the project.",
                )
            },
            _ => ("retry", PairingApprovalState::NotReachable, "Automatic pairing could not complete. Your project remains imported. Ask the inviter to open a pairing window and keep TrackMesh online, then use Try again. Do not reimport or create a new UUID. Both devices need a build supporting automatic pairing."),
        };
        self.set_pairing_approval(project, device, approval);
        json!({"project_id": project, "invitation_imported": imported, "pairing_state": state, "message": message})
    }

    pub fn hide_to_tray(self: &Arc<Self>, app: AppHandle) {
        let engine = {
            let Ok(mut state) = self.lock() else {
                return;
            };
            if state.closing || state.closed || state.close_prompt_active {
                return;
            }
            state.close_prompt_active = true;
            state.engine.clone()
        };
        let desktop = self.clone();
        tauri::async_runtime::spawn(async move {
            let dialog_app = app.clone();
            let result = tauri::async_runtime::spawn_blocking(move || -> Result<()> {
                let shown = engine.as_ref().map(|engine| engine.tray_notice_shown()).transpose()?.unwrap_or(false);
                if !shown {
                    dialog_app.dialog().message("Closing this window keeps TRACKMESH running in the system tray so collaborators can get your versions. Click the tray icon to reopen. Use Close safely or the tray's Quit to exit.").title("TRACKMESH stays available").blocking_show();
                    if let Some(engine) = engine { engine.mark_tray_notice_shown()?; }
                }
                Ok(())
            }).await;
            if let Ok(mut state) = desktop.lock() {
                state.close_prompt_active = false;
                match result {
                    Ok(Ok(())) if !state.closing && !state.closed => {
                        if let Some(window) = app.get_webview_window("main") {
                            match window.hide() {
                                Ok(()) => {
                                    state.window_session = false;
                                    state.tray_session = true;
                                    state.work_epoch = state.work_epoch.wrapping_add(1);
                                }
                                Err(error) => state.error = Some(error.to_string()),
                            }
                        }
                    }
                    Ok(Err(error)) => state.error = Some(error.to_string()),
                    Err(_) => {
                        state.error =
                            Some("Could not show the tray notice; window kept open".to_owned())
                    }
                    _ => {}
                }
            }
        });
    }

    pub fn request_quit(self: &Arc<Self>, app: AppHandle) {
        let engine = {
            let Ok(mut state) = self.lock() else {
                return;
            };
            if state.closing || state.closed || state.close_prompt_active {
                return;
            }
            state.close_prompt_active = true;
            state.engine.clone()
        };
        let desktop = self.clone();
        tauri::async_runtime::spawn(async move {
            let dialog_app = app.clone();
            let result = tauri::async_runtime::spawn_blocking(move || -> Result<bool> {
                let recipients = engine.as_ref().map(|engine| engine.shutdown_recipients()).transpose()?.unwrap_or_default();
                // Nobody loses access without collaborators, so quit needs no confirmation.
                if recipients.is_empty() {
                    return Ok(true);
                }
                let message = format!("{} can't get your versions while TRACKMESH is closed. Quit? Accepted work will finish before closing.", recipients.join(", "));
                Ok(dialog_app.dialog().message(message).title("Close safely?").buttons(tauri_plugin_dialog::MessageDialogButtons::OkCancel).blocking_show())
            }).await;
            let close = desktop.lock().is_ok_and(|mut state| {
                state.close_prompt_active = false;
                match result {
                    Ok(Ok(true)) => {
                        state.closing = true;
                        state.window_session = false;
                        state.tray_session = false;
                        if let Some(plugin) = &state.plugin {
                            plugin.begin_close();
                        }
                        true
                    }
                    Ok(Err(error)) => {
                        state.error = Some(error.to_string());
                        false
                    }
                    Err(_) => {
                        state.error =
                            Some("Could not confirm shutdown; TRACKMESH kept running".to_owned());
                        false
                    }
                    _ => false,
                }
            });
            if close {
                desktop.finish_close(app).await;
            }
        });
    }
    pub fn reopen(&self, app: &AppHandle) {
        if let Ok(mut state) = self.lock() {
            if state.closing || state.closed {
                return;
            }
            state.window_session = true;
            state.work_epoch = state.work_epoch.wrapping_add(1);
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }
    }
    pub fn handle_invite_link(self: &Arc<Self>, app: AppHandle, value: String) {
        self.reopen(&app);
        let value = zeroize::Zeroizing::new(value);
        let parsed = crate::uri::parse_invite_link(&value);
        if let Ok(mut state) = self.lock() {
            state.work_epoch = state.work_epoch.wrapping_add(1);
            let accepting = state
                .jobs
                .iter()
                .any(|job| job.running && job.label == "Accept invite link");
            if let Ok(parsed) = parsed {
                if !accepting {
                    let service = parsed.service.map(|service| PendingInviteService {
                        url: format!("https://{}.supabase.co", service.ref_name),
                        publishable_key: service.publishable_key,
                    });
                    // A new project-less invite replaces only project-less notices.
                    state
                        .jobs
                        .retain(|job| job.running || job.project_id.is_some());
                    prune_finished_jobs(&mut state.jobs);
                    state.pending_invite = Some(PendingInviteLink {
                        id: parsed.id,
                        key: parsed.key,
                        service,
                        parent_folder: None,
                    });
                    return;
                }
            }
            state
                .jobs
                .retain(|job| job.running || job.project_id.is_some());
            prune_finished_jobs(&mut state.jobs);
            state.jobs.push(Job {
                job_id: Uuid::new_v4(),
                project_id: None,
                label: "Open invite link".to_owned(),
                running: false,
                outcome: None,
                error: Some(
                    if accepting {
                        "An invite is being accepted. Wait for it to finish before opening another link.".to_owned()
                    } else {
                        "This invite link could not be opened. Check the link and try again.".to_owned()
                    }
                ),
                completed_at: None,
            });
            prune_finished_jobs(&mut state.jobs);
        }
    }
    pub fn paste_invite_link(&self, app: &AppHandle) -> Result<()> {
        self.reopen(app);
        let mut state = self.lock()?;
        ensure!(
            !state
                .jobs
                .iter()
                .any(|job| job.running && job.label == "Accept invite link"),
            "An invite is being accepted; wait for it to finish"
        );
        let parsed = crate::clipboard::take_invite_link()?;
        state.work_epoch = state.work_epoch.wrapping_add(1);
        state
            .jobs
            .retain(|job| job.running || job.label != "Accept invite link");
        state.pending_invite = Some(PendingInviteLink {
            id: parsed.id,
            key: parsed.key,
            service: parsed.service.map(|service| PendingInviteService {
                url: format!("https://{}.supabase.co", service.ref_name),
                publishable_key: service.publishable_key,
            }),
            parent_folder: None,
        });
        Ok(())
    }
    async fn finish_close(&self, app: AppHandle) {
        let result = self.shutdown().await;
        if let Err(error) = &result {
            tracing::error!(%error, "Desktop shutdown finished with an error");
        }
        if let Ok(mut state) = self.lock() {
            state.closed = true;
        }
        app.exit(if result.is_ok() { 0 } else { 1 });
    }
    async fn shutdown(&self) -> Result<()> {
        let tasks = {
            let mut state = self.lock()?;
            std::mem::take(&mut state.tasks)
        };
        let mut errors = Vec::new();
        for task in tasks {
            if let Err(error) = task.await {
                errors.push(error.to_string());
            }
        }
        // Initialization may have installed the runtime while close was waiting for it.
        let (metadata, peer, plugin) = {
            let mut state = self.lock()?;
            (
                state.metadata.take(),
                state.peer.take(),
                state.plugin.take(),
            )
        };
        if let Some(plugin) = plugin {
            if let Err(error) = plugin.stop().await {
                errors.push(error.to_string());
            }
        }
        if let Some(metadata) = metadata {
            if let Err(error) = metadata.stop().await {
                errors.push(error.to_string());
            }
        }
        if let Some(peer) = peer {
            if let Err(error) = peer.stop().await {
                errors.push(error.to_string());
            }
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
}

fn create_unique_invite_folder(parent: &std::path::Path, name: &str) -> Result<PathBuf> {
    ensure!(!name.is_empty(), "Project folder name is empty");
    let mut suffix = 1u64;
    loop {
        let folder_name = if suffix == 1 {
            name.to_owned()
        } else {
            format!("{name} ({suffix})")
        };
        let path = parent.join(folder_name);
        match std::fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                suffix = suffix
                    .checked_add(1)
                    .context("No available project folder name")?;
            }
            Err(error) => return Err(error.into()),
        }
    }
}

fn sanitize_project_name(name: &str) -> String {
    let mut safe = name
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
            {
                '_'
            } else {
                ch
            }
        })
        .take(120)
        .collect::<String>();
    safe = safe.trim().trim_end_matches([' ', '.']).to_owned();
    if safe.is_empty() {
        safe = "Shared Project".to_owned();
    }
    let stem = safe
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end_matches([' ', '.'])
        .to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || matches!(stem.as_str(), "CONIN$" | "CONOUT$")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        || matches!(
            stem.as_str(),
            "COM¹" | "COM²" | "COM³" | "LPT¹" | "LPT²" | "LPT³"
        )
    {
        safe.insert(0, '_');
    }
    safe
}

fn metadata_frontier(summary: &trackmesh_core::ProjectSummary) -> Vec<(Uuid, Option<Uuid>)> {
    let mut frontier = summary
        .peers
        .iter()
        .map(|peer| {
            (
                peer.device.device_id,
                peer.announcement.as_ref().map(|s| s.snapshot_id),
            )
        })
        .collect::<Vec<_>>();
    frontier.sort();
    frontier
}

async fn wait_for_peer(desktop: &Desktop, network: &Network, peer: &str) -> Result<()> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    while !network.online(peer) {
        ensure!(!desktop.lock()?.closing, "Desktop session is closing");
        ensure!(
            tokio::time::Instant::now() < deadline && network.status().error.is_none(),
            "Source peer is offline or unreachable; retry when online"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Ok(())
}

fn invite_service_ref(url: &str) -> Result<String> {
    mailbox::validate_supabase_project_url(url)?;
    let raw_host = url
        .strip_prefix("https://")
        .context("Invite links require a Supabase project URL of https://<ref>.supabase.co")?;
    ensure!(
        raw_host
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'.'),
        "Invite links require a Supabase project URL of https://<ref>.supabase.co"
    );
    let raw_reference = raw_host
        .strip_suffix(".supabase.co")
        .context("Invite links require a Supabase project URL of https://<ref>.supabase.co")?;
    let host = mailbox::supabase_project_host(url)?;
    let reference = host
        .strip_suffix(".supabase.co")
        .context("Invite links require a Supabase project URL of https://<ref>.supabase.co")?;
    ensure!(
        (1..=63).contains(&reference.len())
            && reference
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit()),
        "Invite links require a lowercase single-label Supabase project host"
    );
    ensure!(
        raw_reference == reference
            && raw_host == format!("{reference}.supabase.co")
            && host == format!("{reference}.supabase.co"),
        "Invite links require a single-label Supabase project host"
    );
    Ok(reference.to_owned())
}

async fn wait_for_mapping_if_requested(network: &Network) -> Result<()> {
    let status = network.status();
    if !status.port_mapping.consent_enabled
        || !matches!(
            status.port_mapping.state.as_str(),
            "DISCOVERING" | "NOT_ATTEMPTED"
        )
    {
        return Ok(());
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    let before = tokio::time::timeout_at(deadline, network.endpoint_addresses())
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or_default();
    let mut state = status.port_mapping.state;
    while tokio::time::Instant::now() < deadline {
        if !matches!(state.as_str(), "DISCOVERING" | "NOT_ATTEMPTED") {
            break;
        }
        tokio::time::sleep_until(std::cmp::min(
            tokio::time::Instant::now() + Duration::from_millis(150),
            deadline,
        ))
        .await;
        state = network.status().port_mapping.state;
    }
    if state == "MAPPED" {
        let initial = before
            .iter()
            .any(|address| network::is_public_endpoint_address(address));
        if !initial {
            let address_deadline = tokio::time::Instant::now() + Duration::from_secs(2);
            while tokio::time::Instant::now() < address_deadline {
                let current =
                    tokio::time::timeout_at(address_deadline, network.endpoint_addresses())
                        .await
                        .ok()
                        .and_then(Result::ok)
                        .unwrap_or_default();
                if current.iter().any(|address| {
                    network::is_public_endpoint_address(address) && !before.contains(address)
                }) {
                    break;
                }
                tokio::time::sleep_until(std::cmp::min(
                    tokio::time::Instant::now() + Duration::from_millis(100),
                    address_deadline,
                ))
                .await;
            }
        }
    }
    Ok(())
}

async fn invite_addresses(
    engine: &Arc<Engine>,
    network: &Network,
    startup_external: &[String],
    explicit: Vec<String>,
) -> Result<Vec<String>> {
    let disk = engine.clone();
    let manual =
        tauri::async_runtime::spawn_blocking(move || disk.manual_public_endpoint()).await??;
    let endpoint_addresses = network.endpoint_addresses().await?;
    let mut output: Vec<String> = Vec::new();
    let mut add = |address: String, validate: bool| {
        if (!validate || protocol::validate_endpoint_address(&address).is_ok())
            && !output.contains(&address)
        {
            output.push(address);
        }
    };
    for address in explicit {
        add(address, true);
    }
    if let Some(manual) = manual {
        for address in
            trackmesh_core::public_ipv4_addresses(&manual.public_ipv4, manual.external_port)?
        {
            add(address, true);
        }
    }
    // Operator-supplied startup addresses (possibly DNS) are kept as before this helper.
    for address in startup_external {
        add(address.clone(), false);
    }
    let mut dynamic = endpoint_addresses;
    dynamic.extend(network.status().addresses);
    dynamic.retain(|address| protocol::validate_endpoint_address(address).is_ok());
    dynamic.sort_by_key(|address| !network::is_public_endpoint_address(address));
    for address in dynamic {
        add(address, true);
    }
    output.truncate(16);
    output.sort();
    Ok(output)
}
