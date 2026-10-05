//! Rehash cached chunks, fetch only missing chunks, then persist verified progress.
use anyhow::{ensure, Result};
use futures::{stream, StreamExt, TryStreamExt};
use network::Network;
use protocol::{PeerRequest, PeerResponse, SignedSnapshot};
use snapshot::ChunkStore;
use std::{collections::BTreeMap, sync::Arc};
use storage::Store;
use uuid::Uuid;

pub(crate) async fn download(
    store: &Arc<Store>,
    chunks: &ChunkStore,
    network: &Network,
    s: &SignedSnapshot,
    id: Uuid,
    peer: &str,
) -> Result<()> {
    let mut unique = BTreeMap::new();
    for file in &s.manifest.files {
        for chunk in &file.chunks {
            unique.insert(chunk.blake3.clone(), chunk.size);
        }
    }
    let total: u64 = unique.values().map(|s| *s as u64).sum();
    store.reset_transfer_progress(id, total)?;
    let mut completed = 0u64;
    let mut missing = Vec::new();
    for (hash, size) in unique {
        let cache = chunks.clone();
        let h = hash.clone();
        if tokio::task::spawn_blocking(move || cache.has_verified(&h, size)).await? {
            completed += size as u64;
            store.transfer_chunk(id, &hash, size, completed)?;
        } else {
            missing.push((hash, size));
        }
    }
    tracing::info!(transfer_id=%id, reused_bytes=completed, missing_bytes=total-completed, missing_chunks=missing.len(), "verified resume cache; requesting only missing chunks");
    let project = s.manifest.project_id;
    let snapshot = s.manifest.snapshot_id;
    let mut requests = stream::iter(missing.into_iter().map(|(hash, size)| {
        let network = network.clone();
        let chunks = chunks.clone();
        let peer = peer.to_owned();
        async move {
            let mut last_error = None;
            for attempt in 0..3 {
                match network
                    .request(
                        &peer,
                        PeerRequest::Chunk {
                            project_id: project,
                            snapshot_id: snapshot,
                            hash: hash.clone(),
                        },
                    )
                    .await
                {
                    Ok(PeerResponse::Chunk {
                        hash: returned,
                        data,
                    }) => {
                        ensure!(
                            returned == hash
                                && data.len() == size as usize
                                && protocol::hash(&data) == hash,
                            "received chunk failed BLAKE3 verification"
                        );
                        let h = hash.clone();
                        let cache = chunks.clone();
                        tokio::task::spawn_blocking(move || cache.put(&h, &data)).await??;
                        return Ok((hash, size));
                    }
                    Ok(PeerResponse::Error { message }) => {
                        last_error = Some(anyhow::anyhow!(message))
                    }
                    Ok(_) => anyhow::bail!("unexpected chunk response"),
                    Err(error) => last_error = Some(error),
                }
                if !network.online(&peer) {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(250 * (attempt + 1))).await;
            }
            Err(last_error.unwrap_or_else(|| anyhow::anyhow!("chunk transfer failed")))
        }
    }))
    .buffer_unordered(8);
    while let Some((hash, size)) = requests.try_next().await? {
        completed += size as u64;
        store.record_chunk(&hash, size)?;
        store.transfer_chunk(id, &hash, size, completed)?;
        tracing::debug!(transfer_id=%id, completed_bytes=completed, total_bytes=total, "chunk committed");
    }
    Ok(())
}
