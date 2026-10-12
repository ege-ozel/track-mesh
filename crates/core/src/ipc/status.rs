use crate::ProjectSummary;
use network::Network;
use protocol::PluginStatus;
use uuid::Uuid;

#[derive(Clone)]
pub(super) enum Notice {
    Queued,
    Saving,
    Committed(Uuid),
    Failed,
    Receive,
}

impl Notice {
    fn message(&self, pending: u64) -> (bool, String) {
        match self {
            Self::Queued => (true, "Snapshot queued in the agent.".into()),
            Self::Saving => (true, "Save observed; waiting for project files to settle.".into()),
            Self::Committed(id) if pending > 0 => (true, format!("Snapshot {id} committed locally; {pending} metadata announcement(s) PENDING. Retry in desktop.")),
            Self::Committed(id) => (true, format!("Snapshot {id} committed locally; no pending authored metadata.")),
            Self::Failed => (false, "Snapshot failed or files did not settle. Check locks/recovery in desktop, then retry Send Snapshot.".into()),
            Self::Receive => (true, "Close this DAW project, then use Receive in TRACKMESH desktop.".into()),
        }
    }
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
    }
}

pub fn summarize(
    summary: ProjectSummary,
    network: &Network,
    notice: Option<Notice>,
) -> PluginStatus {
    let selected = summary
        .peers
        .iter()
        .filter(|p| p.announcement.is_some())
        .max_by_key(|p| {
            p.announcement
                .as_ref()
                .map(|s| (&s.created_at, s.snapshot_id))
        })
        .or_else(|| summary.peers.first());
    let remote = selected.and_then(|p| p.announcement.as_ref());
    let connection = network.status();
    let route = selected
        .and_then(|p| connection.peers.get(&p.device.peer_id))
        .filter(|p| p.online && connection.error.is_none());
    let state = if summary.project.recovery_required {
        storage::TransferState::RecoveryRequired.as_str().into()
    } else if let Some(transfer) = summary
        .transfer
        .as_ref()
        .filter(|t| t.state != storage::TransferState::Complete)
    {
        transfer.state.as_str().into()
    } else if summary.pending_publications > 0 {
        "PENDING".into()
    } else {
        "LAST_KNOWN".into()
    };
    let history_message = match (remote, route) {
        (None, _) => "No remote history. Refresh metadata in TRACKMESH desktop.".into(),
        (Some(_), Some(_)) => {
            "Last-known remote metadata is available; use desktop Check for a live comparison."
                .into()
        }
        (Some(_), None) => {
            "Last-known remote metadata; project files are unavailable while its source is offline."
                .into()
        }
    };
    let (ok, message) = notice
        .map(|notice| notice.message(summary.pending_publications))
        .unwrap_or((true, history_message));
    PluginStatus {
        ok,
        project_id: summary.project.project_id,
        project_name: bounded(&summary.project.name),
        local_snapshot: summary
            .local_snapshot
            .as_ref()
            .map(|s| s.snapshot_id.to_string()),
        remote_snapshot: remote.map(|s| s.snapshot_id.to_string()),
        author: remote.map(|s| bounded(&s.author_name)),
        created_at: remote.map(|s| s.created_at.clone()),
        peer_online: route.is_some(),
        connection: route.map_or("OFFLINE", |p| p.route.as_str()).into(),
        state,
        message: bounded(&message),
    }
}

pub fn bounded(text: &str) -> String {
    text.chars().take(2048).collect()
}
