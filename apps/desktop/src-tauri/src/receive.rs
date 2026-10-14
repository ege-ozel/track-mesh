//! Native confirmation based on verified missing chunks, pinned to the requested snapshot.
use crate::runtime::Desktop;
use anyhow::{ensure, Result};
use network::Network;
use protocol::DeviceCard;
use std::sync::Arc;
use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
use trackmesh_core::Engine;
use uuid::Uuid;

impl Desktop {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn receive_with_confirmation(
        &self,
        app: &AppHandle,
        engine: &Arc<Engine>,
        network: &Network,
        project: Uuid,
        peer: DeviceCard,
        target: Option<Uuid>,
        replace: bool,
        stop: &transfer::Stop,
    ) -> Result<Uuid> {
        let mut target = target;
        let mut limit = 1_000_000_000;
        loop {
            let result = if replace {
                engine
                    .receive_replacing_local_with_limit(
                        network,
                        project,
                        peer.clone(),
                        target,
                        limit,
                        stop.clone(),
                    )
                    .await
            } else {
                engine
                    .receive_from_peer_with_limit(
                        network,
                        project,
                        peer.clone(),
                        target,
                        limit,
                        stop.clone(),
                    )
                    .await
            };
            match result {
                Err(error) if error.downcast_ref::<transfer::DownloadApproval>().is_some() => {
                    let approval = error.downcast_ref::<transfer::DownloadApproval>().unwrap();
                    target = Some(approval.snapshot_id);
                    limit = approval.new_bytes;
                    let app = app.clone();
                    let accepted = tauri::async_runtime::spawn_blocking(move || {
                        app.dialog().message(format!("This version needs about {:.2} GB of new data. TrackMesh also keeps a backup of the files it replaces. Continue?", limit as f64 / 1_000_000_000.0))
                            .title("Confirm download").buttons(MessageDialogButtons::OkCancel).blocking_show()
                    }).await?;
                    ensure!(
                        accepted,
                        "Download cancelled; verified progress is kept for Resume"
                    );
                }
                result => return result,
            }
        }
    }
}
