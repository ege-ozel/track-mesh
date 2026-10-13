//! Work-based owner retention; no filesystem observation or permanent service.
use crate::{peer::PeerRuntime, Engine, PluginRuntime};
use anyhow::Result;
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct SessionGrace {
    idle_since: Option<Instant>,
    epochs: Option<(u64, u64)>,
}

impl SessionGrace {
    /// Brief sessions/requests between polls reset grace just like long-running work.
    pub fn ready(&mut self, plugin: &PluginRuntime, peer: &PeerRuntime, busy: bool) -> bool {
        let epochs = plugin.idle_epoch().zip(peer.idle_epoch());
        if busy || epochs.is_none() || self.epochs != epochs {
            self.idle_since = None;
        }
        self.epochs = epochs;
        if busy || epochs.is_none() {
            return false;
        }
        self.idle_since.get_or_insert_with(Instant::now).elapsed() >= Duration::from_secs(60)
    }
}

impl Engine {
    /// A narrow query; adapters schedule this outside their UI/network threads.
    pub fn has_pending_publication(&self) -> Result<bool> {
        for project in self.store.projects()? {
            if self.mailbox_enabled(project.project_id)?
                && self
                    .store
                    .pending_publication_count(project.project_id, self.identity.device_id)?
                    > 0
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
