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
    pub fn tray_notice_shown(&self) -> Result<bool> {
        Ok(self
            .settings
            .lock()
            .map_err(|_| anyhow::anyhow!("settings lock poisoned"))?
            .value
            .tray_notice_shown)
    }

    pub fn mark_tray_notice_shown(&self) -> Result<()> {
        self.settings
            .lock()
            .map_err(|_| anyhow::anyhow!("settings lock poisoned"))?
            .mark_tray_notice_shown(&self.directory)
    }

    /// Collaborators lose access to locally served versions even when currently offline.
    pub fn shutdown_recipients(&self) -> Result<Vec<String>> {
        let mut recipients = std::collections::BTreeSet::new();
        for project in self.store.projects()? {
            for peer in self.store.peers(Some(project.project_id))? {
                recipients.insert(peer.name);
            }
        }
        Ok(recipients.into_iter().collect())
    }
}
