use crate::Engine;
use anyhow::{ensure, Context, Result};
use network::{Network, NetworkConfig};
use protocol::{PeerRequest, PeerResponse, SignedSnapshot};
use std::{collections::HashSet, sync::Arc, time::Duration};
use tokio::task::{JoinHandle, JoinSet};
use uuid::Uuid;

pub struct PeerRuntime {
    pub network: Network,
    actor: JoinHandle<Result<()>>,
    responder: JoinHandle<Result<()>>,
    // Windows closes this exclusive handle only after this runtime is drained.
    _lease: Arc<std::fs::File>,
}
impl PeerRuntime {
    pub fn start(engine: Arc<Engine>, mut config: NetworkConfig) -> Result<Self> {
        let lease = Arc::new(runtime_lease(&engine.directory)?);
        config.rendezvous = engine.namespaces()?;
        let (network, mut incoming, actor) =
            network::start(engine.identity.clone(), engine.store.clone(), config)?;
        // The lease outlives an accidentally dropped adapter handle as well.
        let actor_lease = lease.clone();
        let actor = tokio::spawn(async move {
            let _lease = actor_lease;
            actor.await.context("network actor panicked")?
        });
        let network_copy = network.clone();
        let responder_lease = lease.clone();
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
                        let engine=engine.clone();let network=network_copy.clone();
                        workers.spawn(async move {
                            let peer=message.peer.to_string();let request=message.request.clone();
                            let response=tokio::task::spawn_blocking(move||transfer::serve(&engine.store,&engine.chunks,&peer,&request)).await;
                            let response=match response {Ok(Ok(r))=>r,Ok(Err(e))=>PeerResponse::Error {message:e.to_string()},Err(_)=>PeerResponse::Error {message:"peer disk operation failed".into()}};
                            let _=network.respond(message,response).await;
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
        })
    }
    pub fn is_finished(&self) -> bool {
        self.actor.is_finished()
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
        self.network.shutdown().await;
        let actor = self.actor.await.context("network actor panicked")?;
        self.responder.await.context("peer responder panicked")??;
        actor
    }
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
