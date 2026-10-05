//! Bounded Windows IPC adapter. Shares the owner's Engine and network handle.
mod security;
mod status;
use crate::Engine;
use anyhow::{ensure, Context, Result};
use network::Network;
use protocol::{PluginEvent, PluginStatus, MAX_IPC_BYTES};
pub use security::plugin_pipe_name;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::windows::named_pipe::NamedPipeServer,
    sync::watch,
    task::{JoinHandle, JoinSet},
    time::timeout,
};
use uuid::Uuid;

#[derive(Default)]
struct State {
    sessions: HashMap<Uuid, Uuid>,
    messages: HashMap<Uuid, status::Notice>,
    jobs: HashMap<Uuid, JoinHandle<()>>,
}
struct Shared {
    engine: Arc<Engine>,
    network: Network,
    allow_http: bool,
    closing: AtomicBool,
    state: Mutex<State>,
}
impl Shared {
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, State>> {
        self.state
            .lock()
            .map_err(|_| anyhow::anyhow!("Plugin session lock poisoned"))
    }
    async fn status(&self, project: Uuid) -> Result<PluginStatus> {
        let engine = self.engine.clone();
        let summary = tokio::task::spawn_blocking(move || engine.project_summary(project)).await?;
        match summary {
            Ok(summary) => Ok(status::summarize(
                summary,
                &self.network,
                self.lock()?.messages.get(&project).cloned(),
            )),
            Err(_) => Ok(status::unavailable(
                project,
                "Link this project UUID to a local folder in TRACKMESH desktop.",
            )),
        }
    }
    fn send(self: &Arc<Self>, project: Uuid) -> Result<()> {
        let mut state = self.lock()?;
        ensure!(
            !self.closing.load(Ordering::Acquire),
            "Plugin bridge is closing"
        );
        state.jobs.retain(|_, task| !task.is_finished());
        ensure!(
            state.jobs.len() < 8 && !state.jobs.contains_key(&project),
            "Snapshot work is already active; retry later"
        );
        ensure!(
            state.messages.contains_key(&project) || state.messages.len() < 64,
            "Plugin project limit reached; restart the session"
        );
        state.messages.insert(project, status::Notice::Queued);
        let shared = self.clone();
        let task = tokio::spawn(async move {
            let result = shared
                .engine
                .send_snapshot(project, shared.allow_http)
                .await;
            let message = match result {
                Ok(sent) => status::Notice::Committed(sent.snapshot_id),
                Err(_) => status::Notice::Failed,
            };
            if let Ok(mut state) = shared.lock() {
                state.messages.insert(project, message);
            }
        });
        state.jobs.insert(project, task);
        Ok(())
    }
}

pub struct PluginRuntime {
    stop: watch::Sender<bool>,
    task: JoinHandle<Result<()>>,
    shared: Arc<Shared>,
}
impl PluginRuntime {
    pub fn start(
        engine: Arc<Engine>,
        network: Network,
        allow_http: bool,
        suffix: &str,
    ) -> Result<Self> {
        let name = plugin_pipe_name(suffix)?;
        let pipe = security::create_pipe(&name, true)
            .context("Cannot own the per-user plugin pipe; another bridge may be active")?;
        let (stop, receiver) = watch::channel(false);
        let shared = Arc::new(Shared {
            engine,
            network,
            allow_http,
            closing: AtomicBool::new(false),
            state: Mutex::new(State::default()),
        });
        let task = tokio::spawn(serve(pipe, name, shared.clone(), receiver));
        Ok(Self { stop, task, shared })
    }
    pub fn is_finished(&self) -> bool {
        self.task.is_finished()
    }
    pub fn is_idle(&self) -> bool {
        self.shared.lock().is_ok_and(|state| {
            state.sessions.is_empty() && state.jobs.values().all(JoinHandle::is_finished)
        })
    }
    /// Stop accepting commands immediately; `stop` still awaits started local work.
    pub fn begin_close(&self) {
        self.shared.closing.store(true, Ordering::Release);
        let _ = self.stop.send(true);
    }
    pub async fn stop(self) -> Result<()> {
        self.begin_close();
        let server = self.task.await;
        let tasks = std::mem::take(&mut self.shared.lock()?.jobs);
        let mut errors = Vec::new();
        for task in tasks.into_values() {
            if let Err(e) = task.await {
                errors.push(e.to_string());
            }
        }
        server??;
        ensure!(
            errors.is_empty(),
            "Plugin snapshot worker failed: {}",
            errors.join("; ")
        );
        Ok(())
    }
}
impl Drop for Shared {
    fn drop(&mut self) {
        self.closing.store(true, Ordering::Release);
    }
}

async fn serve(
    mut pipe: NamedPipeServer,
    name: String,
    shared: Arc<Shared>,
    mut stop: watch::Receiver<bool>,
) -> Result<()> {
    let mut clients = JoinSet::new();
    let outcome = loop {
        tokio::select! {
            biased;
            _ = stop.changed() => break Ok(()),
            Some(result) = clients.join_next(), if !clients.is_empty() => {
                if let Err(error) = result { tracing::warn!(%error, "Plugin IPC worker ended"); }
            },
            connected = pipe.connect(), if clients.len() < 16 => {
                if let Err(error) = connected { break Err(error.into()); }
                let next = match security::create_pipe(&name, false) { Ok(next) => next, Err(error) => break Err(error) };
                let connected = std::mem::replace(&mut pipe, next);
                let shared = shared.clone();
                let mut stop = stop.clone();
                clients.spawn(async move {
                    let mut binding = None;
                    tokio::select! {
                        _ = stop.changed() => {},
                        _ = connection(connected, &shared, &mut binding) => {},
                    }
                    if let Some((session, _)) = binding { if let Ok(mut state) = shared.lock() { state.sessions.remove(&session); } }
                });
            },
        }
    };
    shared.closing.store(true, Ordering::Release);
    // Connection tasks also listen to this channel. On an internal listener error,
    // abort only pipe tasks; snapshot workers are owned separately and awaited by stop.
    clients.abort_all();
    while clients.join_next().await.is_some() {}
    shared.lock()?.sessions.clear();
    drop(pipe);
    outcome
}

async fn connection(
    mut pipe: NamedPipeServer,
    shared: &Arc<Shared>,
    binding: &mut Option<(Uuid, Uuid)>,
) -> Result<()> {
    loop {
        ensure!(
            !shared.closing.load(Ordering::Acquire),
            "Plugin bridge is closing"
        );
        let event = timeout(Duration::from_secs(15), async {
            let length = pipe.read_u32_le().await? as usize;
            ensure!(
                length > 0 && length <= MAX_IPC_BYTES,
                "Invalid plugin frame size"
            );
            let mut bytes = vec![0; length];
            pipe.read_exact(&mut bytes).await?;
            Ok::<PluginEvent, anyhow::Error>(serde_json::from_slice(&bytes)?)
        })
        .await??;
        let (session, project) = event.ids();
        ensure!(
            !session.is_nil() && !project.is_nil(),
            "Nil plugin identifiers"
        );
        match &event {
            PluginEvent::PluginConnected { .. } => {
                ensure!(binding.is_none(), "Plugin connection already bound");
                let mut state = shared.lock()?;
                ensure!(
                    !state.sessions.contains_key(&session),
                    "Plugin session already connected"
                );
                state.sessions.insert(session, project);
                *binding = Some((session, project));
            }
            PluginEvent::ProjectId { .. } => {
                ensure!(
                    binding.is_some_and(|(id, _)| id == session),
                    "Plugin session is not bound"
                );
                shared.lock()?.sessions.insert(session, project);
                *binding = Some((session, project));
            }
            _ => ensure!(
                *binding == Some((session, project)),
                "Plugin session/project mismatch"
            ),
        }
        let mut reply = shared.status(project).await?;
        match event {
            PluginEvent::SendSnapshot { .. } if !reply.project_name.is_empty() => {
                match shared.send(project) {
                    Ok(()) => reply.message = "Snapshot queued in the agent.".into(),
                    Err(_) => {
                        reply.ok = false;
                        reply.message = "Agent busy or closing; retry later.".into();
                    }
                }
            }
            PluginEvent::ReceiveSnapshot { .. } => {
                reply.message =
                    "Close this DAW project, then use Receive in TRACKMESH desktop.".into();
                let mut state = shared.lock()?;
                if state.messages.contains_key(&project) || state.messages.len() < 64 {
                    state.messages.insert(project, status::Notice::Receive);
                }
            }
            PluginEvent::SaveObserved { .. } => {
                reply.message = "Save observed. Use Send Snapshot to commit and publish.".into();
            }
            PluginEvent::SessionEnding { .. } => {
                write_reply(&mut pipe, &reply).await?;
                return Ok(());
            }
            _ => {}
        }
        write_reply(&mut pipe, &reply).await?;
        // A client cannot turn compact polling into an unbounded SQLite job stream.
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
async fn write_reply(pipe: &mut NamedPipeServer, reply: &PluginStatus) -> Result<()> {
    let bytes = serde_json::to_vec(reply)?;
    ensure!(bytes.len() <= MAX_IPC_BYTES, "Plugin reply is too large");
    timeout(Duration::from_secs(5), async {
        pipe.write_u32_le(bytes.len() as u32).await?;
        pipe.write_all(&bytes).await
    })
    .await??;
    Ok(())
}
