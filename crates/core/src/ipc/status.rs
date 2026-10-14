use crate::{Engine, ProjectSummary};
use anyhow::Result;
use network::Network;
use protocol::{PluginProject, PluginStatus};
use storage::Lineage;
use uuid::Uuid;

pub(super) const GET_GUIDANCE: &str =
    "Save and close this project in your DAW, then press Get collaborator's version.";

#[derive(Clone)]
pub(super) enum Notice {
    Queued,
    Saving,
    Committed,
    Failed,
    Receive,
}
impl Notice {
    fn message(&self) -> (bool, String) {
        match self {
            Self::Queued => (true, "Share current version queued; waiting for project files to settle.".into()),
            Self::Saving => (true, "Save observed; waiting for project files to settle.".into()),
            Self::Committed => (true, "Current version shared. Your collaborator can get it while TrackMesh is running.".into()),
            Self::Failed => (false, "Sharing failed or files did not settle. Check locks/recovery in TrackMesh, then retry Share current version.".into()),
            Self::Receive => (true, GET_GUIDANCE.into()),
        }
    }
}
// Only local names and last folder components leave Rust; never full paths.
pub(super) fn projects(engine: &Engine) -> Result<(Vec<PluginProject>, bool)> {
    let mut records = engine.store.projects_page(0, 33)?;
    records.extend(engine.store.projects_page(33, 32)?);
    let more = records.len() > 64;
    records.truncate(64);
    Ok((
        records
            .into_iter()
            .map(|project| PluginProject {
                project_id: project.project_id,
                name: project
                    .name
                    .chars()
                    .take(96)
                    .map(|ch| if ch.is_control() { ' ' } else { ch })
                    .collect(),
                folder_name: std::path::Path::new(&project.root)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("")
                    .chars()
                    .take(48)
                    .map(|ch| if ch.is_control() { ' ' } else { ch })
                    .collect(),
            })
            .collect(),
        more,
    ))
}
pub fn unavailable(project_id: Uuid, message: &str) -> PluginStatus {
    PluginStatus {
        ok: false,
        project_id,
        project_name: String::new(),
        local_snapshot: None,
        remote_snapshot: None,
        author: None,
        created_at: None,
        peer_online: false,
        connection: "OFFLINE".into(),
        state: "UNLINKED".into(),
        message: bounded(message),
        projects: Vec::new(),
        projects_more: false,
        picker_supported: true,
        desktop_available: false,
        status_line: String::new(),
        share_enabled: false,
        get_enabled: false,
        pending_publications: 0,
    }
}
pub fn summarize(
    summary: ProjectSummary,
    network: &Network,
    notice: Option<Notice>,
) -> PluginStatus {
    let selected = summary.peers.iter().max_by_key(|p| {
        p.offline_head
            .as_ref()
            .map(|s| s.created_at.as_str())
            .or_else(|| p.announcement.as_ref().map(|s| s.created_at.as_str()))
    });
    let remote = selected.and_then(|p| p.announcement.as_ref());
    let hint = selected.and_then(|p| p.offline_head.as_ref());
    let connection = network.status();
    let route = selected
        .and_then(|p| connection.peers.get(&p.device.peer_id))
        .filter(|p| p.online && connection.error.is_none());
    let lineage = selected.and_then(|p| p.known_lineage.as_ref());
    let name = selected
        .map(|p| bounded(&p.device.name))
        .unwrap_or_default();
    let transfer = summary
        .transfer
        .as_ref()
        .filter(|t| t.state != storage::TransferState::Complete);
    let (state, line) = if summary.project.recovery_required {
        (
            storage::TransferState::RecoveryRequired.as_str(),
            "Close the DAW and recover this project before receiving another version.".into(),
        )
    } else if let Some(transfer) = transfer {
        let message = match transfer.state {
            storage::TransferState::Receiving | storage::TransferState::Applying => format!(
                "Receiving project… {} %",
                transfer
                    .completed_bytes
                    .saturating_mul(100)
                    .checked_div(transfer.total_bytes)
                    .unwrap_or(0)
                    .min(100)
            ),
            storage::TransferState::Paused => "Receive paused. Resume receive in TrackMesh.".into(),
            _ => format!("Transfer {}", transfer.state.as_str().replace('_', " ")),
        };
        (transfer.state.as_str(), message)
    } else if lineage == Some(&Lineage::Diverged) {
        (
            "DIVERGED",
            format!("You and {name} both changed the project"),
        )
    } else if selected.is_none() {
        ("LAST_KNOWN", "Invite a collaborator in TrackMesh.".into())
    } else if route.is_none() {
        (
            "OFFLINE",
            format!("{name} is offline; their version can be fetched when they are online"),
        )
    } else if hint.is_some() || lineage == Some(&Lineage::RemoteAhead) {
        ("REMOTE_AHEAD", format!("{name} shared a new version"))
    } else if lineage == Some(&Lineage::LocalAhead) {
        (
            "LOCAL_AHEAD",
            format!("Your version is newer than {name}'s"),
        )
    } else if lineage == Some(&Lineage::Equal) {
        ("UP_TO_DATE", format!("Up to date with {name}"))
    } else {
        (
            "LAST_KNOWN",
            format!("Waiting for {name} to share a version"),
        )
    };
    let first_share_blocked = summary.local_snapshot.is_none()
        && (summary.root_empty
            || summary.peers.iter().any(|peer| {
                peer.announcement.is_some()
                    || peer.offline_head.is_some()
                    || peer.known_lineage.is_some()
            }));
    let share_enabled = !first_share_blocked
        && summary.root_available
        && !summary.root_empty
        && !summary.project.recovery_required
        && !summary.active_transfer;
    let get_enabled = route.is_some()
        && (remote.is_some() || hint.is_some())
        && !summary.project.recovery_required
        && !transfer.is_some_and(|item| {
            matches!(
                item.state,
                storage::TransferState::Receiving | storage::TransferState::Applying
            )
        })
        && lineage != Some(&Lineage::Diverged);
    let (ok, message) = notice
        .map(|notice| notice.message())
        .unwrap_or((true, String::new()));
    PluginStatus {
        ok,
        project_id: summary.project.project_id,
        project_name: bounded(&summary.project.name),
        local_snapshot: summary
            .local_snapshot
            .as_ref()
            .map(|s| s.snapshot_id.to_string()),
        remote_snapshot: hint
            .map(|s| s.snapshot_id.to_string())
            .or_else(|| remote.map(|s| s.snapshot_id.to_string())),
        author: selected.map(|_| name),
        created_at: hint
            .map(|s| s.created_at.clone())
            .or_else(|| remote.map(|s| s.created_at.clone())),
        peer_online: route.is_some(),
        connection: route.map_or("OFFLINE", |p| p.route.as_str()).into(),
        state: state.into(),
        message: bounded(&message),
        projects: Vec::new(),
        projects_more: false,
        picker_supported: true,
        desktop_available: false,
        status_line: bounded(&line),
        share_enabled,
        get_enabled,
        pending_publications: if summary.mailbox_enabled {
            summary.pending_publications
        } else {
            0
        },
    }
}
pub fn bounded(text: &str) -> String {
    text.chars()
        .take(512)
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect()
}
