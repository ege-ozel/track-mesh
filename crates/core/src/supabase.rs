//! Session-scoped Supabase membership and endpoint polling.
use crate::{Engine, ProjectBackend, SupabaseProjectState};
use anyhow::{Context, Result};
use network::Network;
use protocol::{DeviceCard, HeadRecord, SupabaseEndpointRow};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::task::JoinHandle;
use uuid::Uuid;

struct ProjectSession {
    claim_attempted: bool,
    member_proven: bool,
    cursor: Option<String>,
    list_at: Instant,
    failures: u32,
    retry_at: Instant,
    grant_retry_at: Instant,
    project_url: String,
    anon_key: String,
    head_list_at: Instant,
    head_snapshot: Option<Uuid>,
    head_published: Option<Uuid>,
    head_seq: Option<u64>,
    head_ciphertext: Option<String>,
    head_attempt_at: Instant,
}

impl Default for ProjectSession {
    fn default() -> Self {
        Self {
            claim_attempted: false,
            member_proven: false,
            cursor: None,
            list_at: Instant::now(),
            failures: 0,
            retry_at: Instant::now(),
            grant_retry_at: Instant::now(),
            project_url: String::new(),
            anon_key: String::new(),
            head_list_at: Instant::now(),
            head_snapshot: None,
            head_published: None,
            head_seq: None,
            head_ciphertext: None,
            head_attempt_at: Instant::now(),
        }
    }
}

pub struct SupabaseRuntime {
    stop: tokio::sync::oneshot::Sender<()>,
    task: JoinHandle<Result<()>>,
}

impl SupabaseRuntime {
    pub fn start(engine: Arc<Engine>, network: Network) -> Self {
        let (stop, mut stopped) = tokio::sync::oneshot::channel();
        let monitor_engine = engine.clone();
        let worker: JoinHandle<Result<()>> = tokio::spawn(async move {
            let mut sessions = HashMap::<Uuid, ProjectSession>::new();
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    _ = &mut stopped => return Ok(()),
                    _ = interval.tick() => {}
                }
                if stopped.try_recv().is_ok() {
                    return Ok(());
                }
                let engine_for_config = engine.clone();
                let configs = match tokio::task::spawn_blocking(move || -> Result<_> {
                    let mut result = Vec::new();
                    for project in engine_for_config.store.projects()? {
                        let config =
                            engine_for_config.project_backend_config(project.project_id)?;
                        if matches!(config.backend, ProjectBackend::Supabase) {
                            result.push((project.project_id, config));
                        }
                    }
                    Ok(result)
                })
                .await
                {
                    Ok(Ok(configs)) => configs,
                    _ => continue,
                };
                let active = configs
                    .iter()
                    .map(|(project, _)| *project)
                    .collect::<std::collections::HashSet<_>>();
                sessions.retain(|project, _| active.contains(project));
                for (project, config) in configs {
                    let session = sessions.entry(project).or_default();
                    let url = config.supabase_url.as_deref().unwrap_or_default();
                    let key = config.supabase_anon_key.as_deref().unwrap_or_default();
                    if session.project_url != url || session.anon_key != key {
                        *session = ProjectSession {
                            project_url: url.to_owned(),
                            anon_key: key.to_owned(),
                            ..ProjectSession::default()
                        };
                    }
                    if session.retry_at > Instant::now() {
                        continue;
                    }
                    let client = match ::mailbox::SupabaseClient::new(url, key, project) {
                        Ok(client) => client,
                        Err(_) => {
                            mark_unavailable(&engine, project, session);
                            continue;
                        }
                    };

                    if !session.claim_attempted {
                        match client.claim(&engine.identity).await {
                            Ok(true) => {
                                session.claim_attempted = true;
                                session.member_proven = true;
                                session.failures = 0;
                                let _ = engine.set_supabase_project_state(
                                    project,
                                    SupabaseProjectState::Claimed,
                                );
                            }
                            Ok(false) => {
                                session.claim_attempted = true;
                                match client.endpoint_page(&engine.identity, None).await {
                                    Ok(rows) => {
                                        session.member_proven = true;
                                        session.cursor = last_cursor(&rows);
                                        let _ = engine.set_supabase_project_state(
                                            project,
                                            SupabaseProjectState::Member,
                                        );
                                        apply_rows(engine.clone(), network.clone(), project, rows)
                                            .await;
                                    }
                                    Err(::mailbox::SupabaseError::Rejected) => {
                                        session.list_at = Instant::now() + Duration::from_secs(30);
                                        let _ = engine.set_supabase_project_state(
                                            project,
                                            SupabaseProjectState::WaitingForGrant,
                                        );
                                        continue;
                                    }
                                    Err(::mailbox::SupabaseError::Unavailable) => {
                                        mark_unavailable(&engine, project, session);
                                        continue;
                                    }
                                }
                            }
                            Err(::mailbox::SupabaseError::Rejected) => {
                                session.claim_attempted = true;
                                match client.endpoint_page(&engine.identity, None).await {
                                    Ok(rows) => {
                                        session.member_proven = true;
                                        session.cursor = last_cursor(&rows);
                                        session.list_at = Instant::now() + Duration::from_secs(30);
                                        let _ = engine.set_supabase_project_state(
                                            project,
                                            SupabaseProjectState::Member,
                                        );
                                        apply_rows(engine.clone(), network.clone(), project, rows)
                                            .await;
                                    }
                                    Err(::mailbox::SupabaseError::Rejected) => {
                                        session.list_at = Instant::now() + Duration::from_secs(30);
                                        let _ = engine.set_supabase_project_state(
                                            project,
                                            SupabaseProjectState::WaitingForGrant,
                                        );
                                        continue;
                                    }
                                    Err(::mailbox::SupabaseError::Unavailable) => {
                                        mark_unavailable(&engine, project, session);
                                        continue;
                                    }
                                }
                            }
                            Err(::mailbox::SupabaseError::Unavailable) => {
                                mark_unavailable(&engine, project, session);
                                continue;
                            }
                        }
                    }

                    if session.claim_attempted && !session.member_proven {
                        if session.list_at > Instant::now() {
                            continue;
                        }
                        match client.endpoint_page(&engine.identity, None).await {
                            Ok(rows) => {
                                session.member_proven = true;
                                session.cursor = last_cursor(&rows);
                                session.list_at = Instant::now() + Duration::from_secs(30);
                                session.failures = 0;
                                let _ = engine.set_supabase_project_state(
                                    project,
                                    SupabaseProjectState::Member,
                                );
                                apply_rows(engine.clone(), network.clone(), project, rows).await;
                            }
                            Err(::mailbox::SupabaseError::Rejected) => {
                                session.list_at = Instant::now() + Duration::from_secs(30);
                                let _ = engine.set_supabase_project_state(
                                    project,
                                    SupabaseProjectState::WaitingForGrant,
                                );
                                continue;
                            }
                            Err(::mailbox::SupabaseError::Unavailable) => {
                                mark_unavailable(&engine, project, session);
                                continue;
                            }
                        }
                    }
                    if !session.member_proven {
                        session.retry_at = Instant::now() + backoff(session.failures);
                        continue;
                    }
                    if session.head_list_at <= Instant::now() {
                        session.head_list_at = Instant::now() + Duration::from_secs(30);
                        if let Ok(rows) = client.head_list(&engine.identity).await {
                            apply_head_rows(engine.clone(), project, rows).await;
                        }
                    }
                    if let Some(snapshot) = latest_local_head(engine.clone(), project).await {
                        if session.head_snapshot != Some(snapshot.manifest.snapshot_id) {
                            let head = head_record(&snapshot);
                            let engine_for_seq = engine.clone();
                            let seq = tokio::task::spawn_blocking(move || {
                                engine_for_seq.next_supabase_sequence(project)
                            })
                            .await;
                            if let Ok(Ok(seq)) = seq {
                                let key_engine = engine.clone();
                                let identity = engine.identity.clone();
                                let key_record = head.clone();
                                if let Ok(Ok((ciphertext, _))) =
                                    tokio::task::spawn_blocking(move || -> Result<_> {
                                        let key = crypto::project_key(
                                            &key_engine.directory.join("secrets/projects"),
                                            project,
                                            None,
                                        )?;
                                        let ciphertext = crypto::wrap_head_record(
                                            project,
                                            &identity,
                                            &key_record,
                                            seq,
                                            &key,
                                        )?;
                                        Ok((ciphertext, seq))
                                    })
                                    .await
                                {
                                    session.head_snapshot = Some(snapshot.manifest.snapshot_id);
                                    session.head_seq = Some(seq);
                                    session.head_ciphertext = Some(ciphertext);
                                    session.head_attempt_at = Instant::now();
                                }
                            }
                        }
                    }
                    if session.head_published != session.head_snapshot
                        && session.head_attempt_at.elapsed() >= Duration::from_secs(5)
                    {
                        if let (Some(seq), Some(ciphertext)) =
                            (session.head_seq, session.head_ciphertext.as_deref())
                        {
                            session.head_attempt_at = Instant::now();
                            match client.publish_head(&engine.identity, seq, ciphertext).await {
                                Ok(true) => session.head_published = session.head_snapshot,
                                Ok(false) => {
                                    // The service may have accepted an earlier request whose
                                    // response was lost. Reissue with a fresh monotonic seq.
                                    session.head_snapshot = None;
                                    session.head_seq = None;
                                    session.head_ciphertext = None;
                                }
                                Err(_) => {}
                            }
                        }
                    }
                    if session.grant_retry_at <= Instant::now()
                        && !flush_grants(&engine, &client, project).await
                    {
                        session.grant_retry_at = Instant::now() + backoff(session.failures);
                        session.failures = session.failures.saturating_add(1);
                        let _ = engine
                            .set_supabase_project_state(project, SupabaseProjectState::Unreachable);
                    }
                    if session.list_at <= Instant::now() {
                        match client
                            .endpoint_page(&engine.identity, session.cursor.as_deref())
                            .await
                        {
                            Ok(rows) => {
                                session.failures = 0;
                                session.cursor =
                                    last_cursor_after(session.cursor.as_deref(), &rows);
                                session.list_at = Instant::now() + Duration::from_secs(30);
                                let _ = engine.set_supabase_project_state(
                                    project,
                                    SupabaseProjectState::Member,
                                );
                                apply_rows(engine.clone(), network.clone(), project, rows).await;
                            }
                            Err(_) => {
                                mark_unavailable(&engine, project, session);
                                session.member_proven = false;
                                session.list_at = Instant::now() + backoff(session.failures);
                            }
                        }
                    }
                }
            }
        });
        let task = tokio::spawn(async move {
            let worker_panicked = match worker.await {
                Ok(Ok(())) => return Ok(()),
                Ok(Err(_)) => false,
                Err(error) => error.is_panic(),
            };
            let engine = monitor_engine.clone();
            if let Ok(Ok(projects)) =
                tokio::task::spawn_blocking(move || engine.store.projects()).await
            {
                for project in projects {
                    if matches!(monitor_engine.project_backend_config(project.project_id), Ok(config) if config.backend == ProjectBackend::Supabase)
                    {
                        let _ = monitor_engine.set_supabase_project_state(
                            project.project_id,
                            SupabaseProjectState::Unreachable,
                        );
                    }
                }
            }
            tracing::error!(
                panicked = worker_panicked,
                "Supabase background task stopped unexpectedly"
            );
            Ok(())
        });
        Self { stop, task }
    }

    pub fn is_finished(&self) -> bool {
        self.task.is_finished()
    }

    pub async fn stop(self) -> Result<()> {
        let _ = self.stop.send(());
        let mut task = self.task;
        match tokio::time::timeout(Duration::from_secs(20), &mut task).await {
            Ok(result) => result.context("Supabase runtime task failed")??,
            Err(_) => {
                task.abort();
                let _ = tokio::time::timeout(Duration::from_secs(1), &mut task).await;
                anyhow::bail!("Supabase runtime did not stop within 20 seconds");
            }
        }
        Ok(())
    }
}

async fn latest_local_head(engine: Arc<Engine>, project: Uuid) -> Option<protocol::SignedSnapshot> {
    tokio::task::spawn_blocking(move || engine.store.head(project).ok().flatten())
        .await
        .ok()
        .flatten()
}

fn head_record(snapshot: &protocol::SignedSnapshot) -> HeadRecord {
    let manifest = &snapshot.manifest;
    HeadRecord {
        snapshot_id: manifest.snapshot_id,
        parent_id: manifest.parent_snapshot_id,
        created_at: manifest.created_at,
        author_device_id: manifest.author_device_id,
        total_size: manifest.total_size,
        file_count: manifest.files.len() as u32,
        manifest_hash: snapshot.manifest_hash.clone(),
        signature: String::new(),
    }
}

async fn apply_head_rows(engine: Arc<Engine>, project: Uuid, rows: Vec<protocol::SupabaseHeadRow>) {
    let peers = engine.clone();
    let cards = tokio::task::spawn_blocking(move || peers.store.peers(Some(project))).await;
    let Ok(Ok(cards)) = cards else {
        return;
    };
    for row in rows {
        let Ok(writer) = Uuid::parse_str(&row.writer_device_id) else {
            continue;
        };
        if writer.to_string() != row.writer_device_id || writer == engine.identity.device_id {
            continue;
        }
        let Some(card) = cards.iter().find(|card| card.device_id == writer) else {
            continue;
        };
        let encrypted = row.ciphertext;
        let seq = row.seq;
        let Ok(seq_number) = seq.parse::<u64>() else {
            continue;
        };
        if seq_number == 0 {
            continue;
        }
        let seq_for_decode = seq.clone();
        let public_key = card.public_key.clone();
        let key_engine = engine.clone();
        let decoded = tokio::task::spawn_blocking(move || -> Result<_> {
            let key = crypto::project_key(
                &key_engine.directory.join("secrets/projects"),
                project,
                None,
            )?;
            let record =
                crypto::unwrap_head_record(&encrypted, project, writer, &seq_for_decode, &key)?;
            crypto::verify_head_record(&record, &public_key)?;
            Ok(record)
        })
        .await;
        if let Ok(Ok(record)) = decoded {
            let _ = engine.remember_offline_head(project, writer, seq_number, record);
        }
    }
}

fn mark_unavailable(engine: &Engine, project: Uuid, session: &mut ProjectSession) {
    session.failures = session.failures.saturating_add(1);
    session.retry_at = Instant::now() + backoff(session.failures);
    let _ = engine.set_supabase_project_state(project, SupabaseProjectState::Unreachable);
}

fn backoff(failures: u32) -> Duration {
    let seconds = 2u64.saturating_pow(failures.min(8)).min(300);
    let jitter = u64::from(Uuid::new_v4().as_bytes()[0]) % (seconds / 5 + 1);
    Duration::from_secs(seconds.saturating_sub(seconds / 10).saturating_add(jitter))
}

fn last_cursor(rows: &[SupabaseEndpointRow]) -> Option<String> {
    rows.last().map(|row| row.cursor.clone())
}

fn last_cursor_after(previous: Option<&str>, rows: &[SupabaseEndpointRow]) -> Option<String> {
    last_cursor(rows).or_else(|| previous.map(str::to_owned))
}

async fn flush_grants(
    engine: &Arc<Engine>,
    client: &::mailbox::SupabaseClient,
    project: Uuid,
) -> bool {
    let grants = match engine.pending_supabase_grants(Some(project)) {
        Ok(grants) => grants,
        Err(_) => return false,
    };
    for grant in grants {
        match client
            .grant(
                &engine.identity,
                grant.actor_seq,
                grant.target_device_id,
                &grant.target_public_key,
            )
            .await
        {
            Ok(true) => {
                if engine.complete_supabase_grant(&grant).is_err() {
                    return false;
                }
            }
            Ok(false) | Err(_) => return false,
        }
    }
    true
}

async fn apply_rows(
    engine: Arc<Engine>,
    network: Network,
    project: Uuid,
    rows: Vec<SupabaseEndpointRow>,
) {
    for row in rows {
        let Ok(writer) = Uuid::parse_str(&row.writer_device_id) else {
            continue;
        };
        if writer.to_string() != row.writer_device_id || writer == engine.identity.device_id {
            continue;
        }
        let cards = engine.clone();
        let trusted = tokio::task::spawn_blocking(move || -> Result<Option<DeviceCard>> {
            Ok(cards
                .store
                .peers(Some(project))?
                .into_iter()
                .find(|card| card.device_id == writer))
        })
        .await;
        let Ok(Ok(Some(card))) = trusted else {
            continue;
        };
        let key_engine = engine.clone();
        let encrypted = row.ciphertext;
        let expires_at = row.expires_at;
        let seq = row.seq;
        let decoded = tokio::task::spawn_blocking(move || -> Result<_> {
            let key = crypto::project_key(
                &key_engine.directory.join("secrets/projects"),
                project,
                None,
            )?;
            crypto::unwrap_endpoint_record(&encrypted, project, writer, &seq, &expires_at, &key)
        })
        .await;
        let Ok(Ok(record)) = decoded else {
            continue;
        };
        let _ = engine
            .apply_endpoint_record(&network, project, card.peer_id, record)
            .await;
    }
}
