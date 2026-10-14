//! Bounded Windows IPC adapter. Shares the owner's Engine and network handle.
mod jobs;
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
    epoch: u64,
    sessions: HashMap<Uuid, Uuid>,
    messages: HashMap<Uuid, status::Notice>,
    jobs: HashMap<Uuid, jobs::Job>,
    desktop_request: Option<PluginDesktopRequest>,
}
#[derive(Clone, Copy)]
pub struct PluginDesktopRequest {
    pub project_id: Option<Uuid>,
}
struct Shared {
    engine: Arc<Engine>,
    network: Network,
    allow_http: bool,
    closing: AtomicBool,
    state: Mutex<State>,
    desktop_available: bool,
}
impl Shared {
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, State>> {
        self.state
            .lock()
            .map_err(|_| anyhow::anyhow!("Plugin session lock poisoned"))
    }
    async fn status(&self, project: Uuid) -> Result<PluginStatus> {
        let engine = self.engine.clone();
        let (summary, projects, more) = tokio::task::spawn_blocking(move || -> Result<_> {
            let (projects, more) = status::projects(&engine)?;
            Ok((engine.project_summary(project), projects, more))
        })
        .await??;
        let mut reply = match summary {
            Ok(summary) => status::summarize(
                summary,
                &self.network,
                self.lock()?.messages.get(&project).cloned(),
            ),
            Err(_) => status::unavailable(project, "Link this project's folder in TrackMesh first"),
        };
        reply.projects = projects;
        reply.projects_more = more;
        reply.desktop_available = self.desktop_available;
        reply.get_enabled &= self.desktop_available;
        Ok(reply)
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
        Self::start_impl(engine, network, allow_http, suffix, false)
    }
    pub fn start_with_desktop(
        engine: Arc<Engine>,
        network: Network,
        allow_http: bool,
        suffix: &str,
    ) -> Result<Self> {
        Self::start_impl(engine, network, allow_http, suffix, true)
    }
    pub fn take_desktop_request(&self) -> Option<PluginDesktopRequest> {
        self.shared.lock().ok()?.desktop_request.take()
    }
    fn start_impl(
        engine: Arc<Engine>,
        network: Network,
        allow_http: bool,
        suffix: &str,
        desktop_available: bool,
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
            desktop_available,
        });
        let task = tokio::spawn(serve(pipe, name, shared.clone(), receiver));
        Ok(Self { stop, task, shared })
    }
    pub fn is_finished(&self) -> bool {
        self.task.is_finished()
    }
    pub fn is_idle(&self) -> bool {
        self.idle_epoch().is_some()
    }
    pub fn idle_epoch(&self) -> Option<u64> {
        let state = self.shared.lock().ok()?;
        (state.sessions.is_empty()
            && state.desktop_request.is_none()
            && state.jobs.values().all(|job| job.task.is_finished()))
        .then_some(state.epoch)
    }
    /// Claim idle shutdown under the same gate used to accept connections/jobs.
    pub fn close_if_idle(&self) -> bool {
        let Ok(state) = self.shared.lock() else {
            return false;
        };
        if !state.sessions.is_empty()
            || state.desktop_request.is_some()
            || state.jobs.values().any(|job| !job.task.is_finished())
        {
            return false;
        }
        self.begin_close();
        true
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
        for job in tasks.into_values() {
            if let Err(e) = job.task.await {
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
                    !shared.closing.load(Ordering::Acquire),
                    "Plugin bridge is closing"
                );
                ensure!(
                    !state.sessions.contains_key(&session),
                    "Plugin session already connected"
                );
                state.sessions.insert(session, project);
                state.epoch = state.epoch.wrapping_add(1);
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
            PluginEvent::SendSnapshot { .. } if reply.share_enabled => {
                match shared.schedule(project, false) {
                    Ok(()) => {
                        reply.message =
                            "Share current version queued; waiting for project files to settle."
                                .into()
                    }
                    Err(_) => {
                        reply.ok = false;
                        reply.message = "Agent busy or closing; retry later.".into();
                    }
                }
            }
            PluginEvent::SendSnapshot { .. } => {
                reply.ok = false;
                reply.message = "Share current version is unavailable. Open TrackMesh to finish setup or recovery first.".into();
            }
            PluginEvent::ReceiveSnapshot { .. } => {
                if !shared.desktop_available {
                    reply.ok = false;
                    reply.message = "Open TrackMesh to get your collaborator's version.".into();
                } else {
                    let mut state = shared.lock()?;
                    ensure!(
                        !shared.closing.load(Ordering::Acquire),
                        "Plugin bridge is closing"
                    );
                    state.desktop_request = Some(PluginDesktopRequest {
                        project_id: (!reply.project_name.is_empty()).then_some(project),
                    });
                    state.epoch = state.epoch.wrapping_add(1);
                    reply.message = status::GET_GUIDANCE.into();
                    if state.messages.contains_key(&project) || state.messages.len() < 64 {
                        state.messages.insert(project, status::Notice::Receive);
                    }
                }
            }
            PluginEvent::SaveObserved { .. } if !reply.project_name.is_empty() => {
                match shared.schedule(project, true) {
                    Ok(()) => {
                        reply.message = "Save observed; waiting for project files to settle.".into()
                    }
                    Err(_) => {
                        reply.ok = false;
                        reply.message =
                            "Save not queued; agent busy or closing. Retry Share current version."
                                .into();
                    }
                }
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
