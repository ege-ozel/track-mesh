//! Explicit manual send: local commit survives metadata transport failure.
use crate::{Engine, PublicationStatus};
use anyhow::Result;
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Serialize)]
pub struct SendStatus {
    pub snapshot_id: Uuid,
    pub publication: Option<PublicationStatus>,
    pub publication_error: Option<String>,
}

impl Engine {
    pub async fn send_snapshot(
        self: &Arc<Self>,
        project: Uuid,
        allow_loopback_http: bool,
    ) -> Result<SendStatus> {
        let engine = self.clone();
        let snapshot_id = tokio::task::spawn_blocking(move || {
            engine.snapshot(project).map(|s| s.manifest.snapshot_id)
        })
        .await??;
        let (publication, publication_error) =
            match self.publish_metadata(project, allow_loopback_http).await {
                Ok(status) => (Some(status), None),
                Err(error) => (None, Some(error.to_string())),
            };
        Ok(SendStatus {
            snapshot_id,
            publication,
            publication_error,
        })
    }
}
