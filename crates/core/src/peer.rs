use crate::Engine;
use anyhow::{ensure, Context, Result};
use network::{Network, NetworkConfig};
use network::{PortMappingRuntime, UpnpTarget};
use protocol::{PeerRequest, PeerResponse, SignedSnapshot};
use std::{
    collections::HashSet,
    net::{IpAddr, Ipv4Addr},
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::task::{JoinHandle, JoinSet};
use uuid::Uuid;

pub struct PeerRuntime {
    pub network: Network,
    actor: JoinHandle<Result<()>>,
    responder: JoinHandle<Result<()>>,
    // Windows closes this exclusive handle only after this runtime is drained.
    _lease: Arc<std::fs::File>,
    activity: Arc<ResponderActivity>,
    mapping: PortMappingRuntime,
}
#[derive(Default)]
struct ResponderActivity {
    active: AtomicUsize,
    epoch: AtomicU64,
}
struct ResponderWork(Arc<ResponderActivity>);
impl Drop for ResponderWork {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
    }
}
impl PeerRuntime {
    pub fn start(
        engine: Arc<Engine>,
        mut config: NetworkConfig,
        observation: crate::NetworkObservation,
    ) -> Result<Self> {
        let lease = Arc::new(runtime_lease(&engine.directory)?);
        config.rendezvous = engine.namespaces()?;
        let (network, mut incoming, actor) =
            network::start(engine.identity.clone(), engine.store.clone(), config)?;
        network.set_mapping_consent(engine.upnp_mapping_enabled());
        let mapping = PortMappingRuntime::start(
            network.clone(),
            upnp_target(&observation),
            engine.identity.device_id.to_string(),
        );
        // The lease outlives an accidentally dropped adapter handle as well.
        let actor_lease = lease.clone();
        let actor = tokio::spawn(async move {
            let _lease = actor_lease;
            actor.await.context("network actor panicked")?
        });
        let network_copy = network.clone();
        let responder_lease = lease.clone();
        let activity = Arc::new(ResponderActivity::default());
        let responder_activity = activity.clone();
        let responder = tokio::spawn(async move {
            let _lease = responder_lease;
            let mut workers = JoinSet::new();
            let mut closed = false;
            loop {
                if closed && workers.is_empty() {
                    break;
                }
                tokio::select! {
                    message=incoming.recv(),if !closed && workers.len()<8=>{
                        let Some(message)=message else {closed=true;continue;};
                        responder_activity.active.fetch_add(1, Ordering::SeqCst);
                        responder_activity.epoch.fetch_add(1, Ordering::SeqCst);
                        let work = ResponderWork(responder_activity.clone());
                        let engine=engine.clone();let network=network_copy.clone();
                        workers.spawn(async move {
                            let _work = work;
                            let peer=message.peer.to_string();let request=message.request.clone();
                            match request {
                                network::IncomingRequest::Transfer(request) => {
                                    let response=tokio::task::spawn_blocking(move||transfer::serve(&engine.store,&engine.chunks,&peer,&request)).await;
                                    let response=match response {Ok(Ok(r))=>r,Ok(Err(e))=>PeerResponse::Error {message:e.to_string()},Err(_)=>PeerResponse::Error {message:"peer disk operation failed".into()}};
                                    let _=network.respond(message,response).await;
                                }
                                network::IncomingRequest::Pairing(request) => {
                                    let response = tokio::task::spawn_blocking(move || engine.handle_pairing(&peer, request)).await.unwrap_or(protocol::PairingResponse::Unavailable);
                                    let _ = network.respond_pairing(message, response).await;
                                }
                            }
                        });
                    },
                    result=workers.join_next(),if !workers.is_empty()=>{if let Some(Err(error))=result {tracing::warn!(%error,"peer responder worker stopped");}},
                }
            }
            Ok(())
        });
        Ok(Self {
            network,
            actor,
            responder,
            _lease: lease,
            activity,
            mapping,
        })
    }
    pub fn is_finished(&self) -> bool {
        self.actor.is_finished()
    }
    pub fn idle_epoch(&self) -> Option<u64> {
        let epoch = self.activity.epoch.load(Ordering::SeqCst);
        (self.activity.active.load(Ordering::SeqCst) == 0).then_some(epoch)
    }
    pub async fn wait_for_peer(&self, peer: &str, timeout: Duration) -> Result<()> {
        let deadline = tokio::time::Instant::now() + timeout;
        while !self.network.online(peer) {
            ensure!(
                !self.actor.is_finished(),
                "network actor stopped before connecting"
            );
            ensure!(
                tokio::time::Instant::now() < deadline,
                "source peer is offline or unreachable; no files can be downloaded"
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Ok(())
    }
    pub async fn stop(self) -> Result<()> {
        self.mapping.stop(&self.network).await;
        self.network.shutdown().await;
        let actor = self.actor.await.context("network actor panicked")?;
        self.responder.await.context("peer responder panicked")??;
        actor
    }
}

fn upnp_target(observation: &crate::NetworkObservation) -> Option<UpnpTarget> {
    let mut routes = observation.default_gateways.iter().collect::<Vec<_>>();
    routes.sort_by_key(|route| (route.interface_index, route.address.clone()));
    routes.dedup_by_key(|route| (route.interface_index, route.address.clone()));
    let [route] = routes.as_slice() else {
        return None;
    };
    let gateway = route.address.parse::<IpAddr>().ok()?;
    let interface = observation
        .interfaces
        .iter()
        .find(|item| item.interface_index == route.interface_index)?;
    let mut addresses = interface
        .ipv4
        .iter()
        .filter_map(|value| value.parse::<Ipv4Addr>().ok())
        .filter(|ip| {
            !ip.is_unspecified() && !ip.is_loopback() && !ip.is_link_local() && !ip.is_multicast()
        });
    let local = addresses.next()?;
    addresses
        .next()
        .is_none()
        .then_some(UpnpTarget { gateway, local })
}

fn runtime_lease(directory: &std::path::Path) -> Result<std::fs::File> {
    let locks = directory.join("locks");
    std::fs::create_dir_all(&locks)?;
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
    }
    #[cfg(not(windows))]
    anyhow::bail!("peer runtime ownership requires Windows");
    options
        .open(locks.join("peer-runtime.lock"))
        .context("another TrackMesh peer runtime already owns this data directory")
}

impl Engine {
    pub async fn fetch_snapshot_chain(
        &self,
        network: &Network,
        project: Uuid,
        source: &str,
        target: Option<Uuid>,
    ) -> Result<Option<SignedSnapshot>> {
        ensure!(
            self.store.authorized(project, source)?.is_some(),
            "peer is not paired with this project"
        );
        let response = network
            .request(
                source,
                if let Some(snapshot_id) = target {
                    PeerRequest::Manifest {
                        project_id: project,
                        snapshot_id,
                    }
                } else {
                    PeerRequest::Head {
                        project_id: project,
                    }
                },
            )
            .await?;
        let snapshot = match response {
            PeerResponse::Head { snapshot: None } if target.is_none() => return Ok(None),
            PeerResponse::Head { snapshot: Some(s) } if target.is_none() => s,
            PeerResponse::Manifest { snapshot: s } if target.is_some() => s,
            PeerResponse::Error { message } => anyhow::bail!(message),
            _ => anyhow::bail!("unexpected peer manifest response"),
        };
        transfer::validate_snapshot(&self.store, &self.identity, &snapshot, project, target)?;
        self.remember_author_manifest(&snapshot)?;
        let local = self.store.project(project)?.latest_local_snapshot;
        let mut seen = HashSet::from([snapshot.manifest.snapshot_id]);
        let mut parent = snapshot.manifest.parent_snapshot_id;
        for _ in 0..1024 {
            let Some(id) = parent else {
                return Ok(Some(snapshot));
            };
            ensure!(seen.insert(id), "cyclic snapshot ancestry");
            if Some(id) == local {
                return Ok(Some(snapshot));
            }
            let ancestor = if let Some(cached) = self.store.snapshot(id)? {
                cached
            } else {
                match network
                    .request(
                        source,
                        PeerRequest::Manifest {
                            project_id: project,
                            snapshot_id: id,
                        },
                    )
                    .await?
                {
                    PeerResponse::Manifest { snapshot } => snapshot,
                    PeerResponse::Error { message } => {
                        anyhow::bail!("snapshot ancestry is incomplete: {message}")
                    }
                    _ => anyhow::bail!("unexpected ancestor response"),
                }
            };
            transfer::validate_snapshot(&self.store, &self.identity, &ancestor, project, Some(id))?;
            parent = ancestor.manifest.parent_snapshot_id;
            self.remember_author_manifest(&ancestor)?;
        }
        anyhow::bail!("snapshot ancestry exceeds the MVP history limit; receive blocked")
    }
    fn remember_author_manifest(&self, snapshot: &SignedSnapshot) -> Result<()> {
        let name = if snapshot.manifest.author_device_id == self.identity.device_id {
            self.identity.name.clone()
        } else {
            self.store
                .peers(Some(snapshot.manifest.project_id))?
                .into_iter()
                .find(|p| p.device_id == snapshot.manifest.author_device_id)
                .context("snapshot author is not paired")?
                .name
        };
        self.store.remember_manifest(snapshot, &name)
    }
    pub fn receiver(&self, network: Network) -> transfer::Receiver {
        transfer::Receiver {
            store: self.store.clone(),
            chunks: self.chunks.clone(),
            network,
            identity: self.identity.clone(),
            directory: self.directory.clone(),
        }
    }
}
