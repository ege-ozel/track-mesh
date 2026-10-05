//! One explicit desktop session, bounded jobs, and awaited shutdown.
use crate::documents;
use anyhow::{ensure, Context, Result};
use clap::Parser;
use network::{Network, NetworkConfig, NetworkStatus};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::AppHandle;
use trackmesh_core::{
    default_data_directory, peer::PeerRuntime, Engine, MailboxRuntime, PeerCheck, PluginRuntime,
    SummaryPage,
};
use uuid::Uuid;

#[derive(Clone, Parser)]
#[command(
    name = "trackmesh-desktop",
    about = "Private Windows project snapshots and transfers"
)]
pub struct Options {
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
    Link {
        project_id: Uuid,
        name: String,
        folder: PathBuf,
        mailbox_url: Option<String>,
        relay_address: Option<String>,
    },
    Join {
        folder: PathBuf,
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
    },
    ExportCard,
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
    Recover {
        project_id: Uuid,
        daw_closed: bool,
    },
}

impl Action {
    fn project(&self) -> Option<Uuid> {
        match self {
            Self::Join { .. } | Self::ExportCard => None,
            Self::Link { project_id, .. }
            | Self::Pair { project_id }
            | Self::Revoke { project_id, .. }
            | Self::ExportInvite { project_id }
            | Self::Send { project_id }
            | Self::Publish { project_id }
            | Self::Refresh { project_id }
            | Self::Check { project_id, .. }
            | Self::Receive { project_id, .. }
            | Self::Recover { project_id, .. } => Some(*project_id),
        }
    }
    fn label(&self) -> &'static str {
        match self {
            Self::Link { .. } => "Link folder",
            Self::Join { .. } => "Import invitation",
            Self::Pair { .. } => "Pair device",
            Self::Revoke { .. } => "Revoke device",
            Self::ExportInvite { .. } => "Export invitation",
            Self::ExportCard => "Export device card",
            Self::Send { .. } => "Send Snapshot",
            Self::Publish { .. } => "Publish metadata",
            Self::Refresh { .. } => "Refresh metadata",
            Self::Check { .. } => "Check peer",
            Self::Receive { .. } => "Receive snapshot",
            Self::Recover { .. } => "Recover installation",
        }
    }
    fn changes_setup(&self) -> bool {
        matches!(
            self,
            Self::Link { .. } | Self::Join { .. } | Self::Pair { .. } | Self::Revoke { .. }
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

#[derive(Serialize)]
pub struct DesktopStatus {
    pub ready: bool,
    pub closing: bool,
    pub device: Option<Device>,
    pub data_directory: Option<String>,
    pub error: Option<String>,
    pub network: Option<NetworkStatus>,
    pub page: Option<SummaryPage>,
    pub jobs: Vec<Job>,
    pub checks: Vec<CheckView>,
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
    engine: Option<Arc<Engine>>,
    device: Option<Device>,
    peer: Option<PeerRuntime>,
    network: Option<Network>,
    metadata: Option<MailboxRuntime>,
    plugin: Option<PluginRuntime>,
    tasks: Vec<tauri::async_runtime::JoinHandle<()>>,
    jobs: Vec<Job>,
    checks: HashMap<(Uuid, Uuid), Checked>,
    ready: bool,
    closing: bool,
    closed: bool,
    error: Option<String>,
}

pub struct Desktop {
    options: Options,
    session: Mutex<Session>,
    // At most one slow status read per window; operation tasks never hold this gate.
    status_gate: tokio::sync::Mutex<()>,
}

impl Desktop {
    pub fn new(options: Options) -> Self {
        Self {
            options,
            session: Mutex::new(Session::default()),
            status_gate: tokio::sync::Mutex::new(()),
        }
    }
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Session>> {
        self.session
            .lock()
            .map_err(|_| anyhow::anyhow!("Desktop session lock poisoned"))
    }
    pub fn is_closed(&self) -> bool {
        self.session.lock().is_ok_and(|s| s.closed)
    }
    pub fn start(self: &Arc<Self>) {
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
    }
    async fn initialize(&self) -> Result<()> {
        let options = self.options.clone();
        let engine = tauri::async_runtime::spawn_blocking(move || -> Result<_> {
            let directory = options
                .data_dir
                .map(Ok)
                .unwrap_or_else(default_data_directory)?;
            Ok(Arc::new(Engine::open(directory, &options.device_name)?))
        })
        .await??;
        let device = Device {
            device_id: engine.identity.device_id,
            name: engine.identity.name.clone(),
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
        let peer = tauri::async_runtime::spawn_blocking(move || {
            let config = NetworkConfig {
                port: options.port,
                external_addresses: options
                    .external_address
                    .iter()
                    .map(|v| v.parse())
                    .collect::<std::result::Result<_, _>>()?,
                ..Default::default()
            };
            PeerRuntime::start(engine_for_network, config)
        })
        .await??;
        let metadata = MailboxRuntime::start(engine.clone(), self.options.allow_loopback_http);
        let mut state = self.lock()?;
        state.network = Some(peer.network.clone());
        state.peer = Some(peer);
        state.metadata = Some(metadata);
        state.plugin = Some(PluginRuntime::start(
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
            .map(|engine| engine.directory.display().to_string());
        let page = match engine {
            Some(engine) => Some(
                tauri::async_runtime::spawn_blocking(move || engine.summary_page(offset)).await??,
            ),
            None => None,
        };
        let mut state = self.lock()?;
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
        Ok(DesktopStatus {
            ready: state.ready,
            closing: state.closing,
            device: state.device.clone(),
            data_directory,
            error: state.error.clone().or(runtime_error),
            network,
            page,
            jobs: state.jobs.clone(),
            checks,
        })
    }

    pub fn submit(self: &Arc<Self>, app: AppHandle, action: Action) -> Result<Uuid> {
        let mut state = self.lock()?;
        ensure!(
            state.ready && !state.closing,
            "Desktop is starting or closing"
        );
        let project = action.project();
        ensure!(
            !state.jobs.iter().any(|job| job.running
                && (job.project_id.is_none() || project.is_none() || job.project_id == project)),
            "This project already has an active operation"
        );
        ensure!(
            state.jobs.iter().filter(|job| job.running).count() < 8,
            "Too many active operations"
        );
        state.jobs.retain(|job| job.running);
        state.tasks.retain(|task| !task.inner().is_finished());
        let job_id = Uuid::new_v4();
        state.jobs.push(Job {
            job_id,
            project_id: project,
            label: action.label().to_owned(),
            running: true,
            outcome: None,
            error: None,
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
                    match result {
                        Ok(outcome) => job.outcome = Some(outcome),
                        Err(error) => job.error = Some(error.to_string()),
                    }
                }
            }
        }));
        Ok(job_id)
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
            Action::Send { project_id } => {
                serde_json::to_value(engine.send_snapshot(project_id, allow_http).await?)?
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
                json!({"transfer_id": engine.receive_from_peer(&network, project_id, peer, snapshot_id).await?})
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
                let mut addresses = advertised_addresses(&network);
                addresses.extend(self.options.external_address.iter().cloned());
                addresses.sort();
                addresses.dedup();
                addresses.truncate(16);
                tauri::async_runtime::spawn_blocking(move || -> Result<Value> {
                    match action {
                        Action::Link { project_id, name, folder, mailbox_url, relay_address } => {
                            ensure!(!project_id.is_nil(), "Project UUID cannot be nil");
                            if let Some(relay) = &relay_address { let _: network::NetworkConfig = NetworkConfig { rendezvous: vec![(relay.parse()?, String::new())], ..Default::default() }; }
                            disk.link(project_id, &name, &folder, mailbox_url.as_deref(), relay_address.as_deref())?;
                            Ok(json!({"message": "Folder linked"}))
                        }
                        Action::Join { folder } => {
                            let invite = documents::read_document(&app)?;
                            disk.import_invite(invite, &folder)?;
                            Ok(json!({"message": "Invitation imported. Export this device card for the inviter to pair."}))
                        }
                        Action::Pair { project_id } => {
                            disk.pair(project_id, documents::read_document(&app)?)?;
                            Ok(json!({"message": "Device paired"}))
                        }
                        Action::Revoke { project_id, device_id } => {
                            disk.revoke(project_id, device_id)?;
                            Ok(json!({"message": "Project access revoked locally. The service operator must also update infrastructure ACLs."}))
                        }
                        Action::ExportInvite { project_id } => {
                            let invite = disk.export_invite(project_id, addresses)?;
                            let path = documents::write_document(&app, "trackmesh-invite.json", &invite)?;
                            Ok(json!({"message": "Private invitation saved", "path": path}))
                        }
                        Action::ExportCard => {
                            let card = disk.device_card(addresses)?;
                            let path = documents::write_document(&app, "trackmesh-card.json", &card)?;
                            Ok(json!({"message": "Device card saved", "path": path}))
                        }
                        _ => anyhow::bail!("Unsupported document action"),
                    }
                }).await??
            }
        };
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

    pub fn begin_close(self: &Arc<Self>, app: AppHandle) {
        let should_close = self
            .lock()
            .map(|mut state| {
                if state.closing || state.closed {
                    false
                } else {
                    state.closing = true;
                    if let Some(plugin) = &state.plugin {
                        plugin.begin_close();
                    }
                    true
                }
            })
            .unwrap_or(false);
        if !should_close {
            return;
        }
        let desktop = self.clone();
        tauri::async_runtime::spawn(async move {
            let result = desktop.shutdown().await;
            if let Err(error) = &result {
                tracing::error!(%error, "Desktop shutdown finished with an error");
            }
            if let Ok(mut state) = desktop.lock() {
                state.closed = true;
            }
            app.exit(if result.is_ok() { 0 } else { 1 });
        });
    }
    async fn shutdown(&self) -> Result<()> {
        let (metadata, network, tasks) = {
            let mut state = self.lock()?;
            (
                state.metadata.take(),
                state.network.clone(),
                std::mem::take(&mut state.tasks),
            )
        };
        if let Some(network) = network {
            network.shutdown().await;
        }
        let mut errors = Vec::new();
        if let Some(metadata) = metadata {
            if let Err(error) = metadata.stop().await {
                errors.push(error.to_string());
            }
        }
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

fn advertised_addresses(network: &Network) -> Vec<String> {
    network
        .status()
        .addresses
        .into_iter()
        .filter(|address| !address.contains("/ip4/0.0.0.0/") && !address.contains("/ip6/::/"))
        .take(16)
        .collect()
}
