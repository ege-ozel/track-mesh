//! Versioned, atomic local settings that do not alter the project database.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

const VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum ProjectBackend {
    #[default]
    None,
    SelfHostedMailbox,
    Supabase,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SupabaseProjectState {
    NotConfigured,
    #[default]
    Unknown,
    Claimed,
    Member,
    RouteTaken,
    WaitingForGrant,
    Unreachable,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectBackendConfig {
    pub backend: ProjectBackend,
    pub supabase_url: Option<String>,
    pub supabase_anon_key: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupabaseDefaultConfig {
    pub url: String,
    pub publishable_key: String,
}

fn compiled_default_supabase() -> Option<SupabaseDefaultConfig> {
    let url = option_env!("TRACKMESH_DEFAULT_SUPABASE_URL")?;
    let publishable_key = option_env!("TRACKMESH_DEFAULT_SUPABASE_PUBLISHABLE_KEY")?;
    Some(SupabaseDefaultConfig {
        url: url.to_owned(),
        publishable_key: publishable_key.to_owned(),
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingGrant {
    pub project_id: Uuid,
    pub target_device_id: Uuid,
    pub target_public_key: String,
    pub actor_seq: u64,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Settings {
    version: u32,
    upnp_mapping_enabled: bool,
    #[serde(default)]
    pub project_backends: HashMap<Uuid, ProjectBackendConfig>,
    #[serde(default)]
    pub actor_sequences: HashMap<Uuid, u64>,
    #[serde(default)]
    pub pending_grants: Vec<PendingGrant>,
    #[serde(default = "compiled_default_supabase")]
    pub default_supabase: Option<SupabaseDefaultConfig>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: VERSION,
            upnp_mapping_enabled: false,
            project_backends: HashMap::new(),
            actor_sequences: HashMap::new(),
            pending_grants: Vec::new(),
            default_supabase: compiled_default_supabase(),
        }
    }
}

impl Settings {
    pub fn upnp_mapping_enabled(&self) -> bool {
        self.upnp_mapping_enabled
    }
    pub fn set_upnp_mapping_enabled(&mut self, enabled: bool) {
        self.upnp_mapping_enabled = enabled;
    }
}

#[derive(Clone)]
pub(super) struct SettingsState {
    pub value: Settings,
    writable: bool,
}

impl SettingsState {
    pub fn load(directory: &Path) -> Self {
        let path = directory.join("settings.json");
        if !path.exists() {
            return Self {
                value: Settings::default(),
                writable: true,
            };
        }
        let loaded = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Settings>(&bytes).ok());
        match loaded {
            Some(mut value) if (1..=VERSION).contains(&value.version) => {
                value.version = VERSION;
                Self {
                    value,
                    writable: true,
                }
            }
            _ => Self {
                value: Settings::default(),
                writable: false,
            },
        }
    }

    fn commit(&mut self, directory: &Path, mut candidate: Settings) -> Result<()> {
        ensure!(
            self.writable,
            "settings are invalid or from a newer version; refusing to overwrite them"
        );
        candidate.version = VERSION;
        store(directory, &candidate)?;
        self.value = candidate;
        Ok(())
    }

    pub fn set_upnp_mapping(&mut self, directory: &Path, enabled: bool) -> Result<()> {
        let mut candidate = self.value.clone();
        candidate.set_upnp_mapping_enabled(enabled);
        self.commit(directory, candidate)
    }

    pub fn set_project_backend(
        &mut self,
        directory: &Path,
        project: Uuid,
        config: ProjectBackendConfig,
    ) -> Result<()> {
        let mut candidate = self.value.clone();
        candidate.project_backends.insert(project, config);
        self.commit(directory, candidate)
    }

    pub fn set_default_supabase(
        &mut self,
        directory: &Path,
        config: Option<SupabaseDefaultConfig>,
    ) -> Result<()> {
        let mut candidate = self.value.clone();
        candidate.default_supabase = config;
        self.commit(directory, candidate)
    }

    pub fn enqueue_grant(
        &mut self,
        directory: &Path,
        project_id: Uuid,
        target_device_id: Uuid,
        target_public_key: String,
    ) -> Result<PendingGrant> {
        if let Some(existing) = self.value.pending_grants.iter().find(|grant| {
            grant.project_id == project_id && grant.target_device_id == target_device_id
        }) {
            ensure!(
                existing.target_public_key == target_public_key,
                "pending grant identity changed"
            );
            return Ok(existing.clone());
        }
        let mut candidate = self.value.clone();
        let previous = candidate
            .actor_sequences
            .get(&project_id)
            .copied()
            .unwrap_or(0);
        let next = previous
            .checked_add(1)
            .context("Supabase actor sequence exhausted")?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let now = u64::try_from(now).context("Unix millisecond value exceeds u64")?;
        let actor_seq = next.max(now);
        candidate.actor_sequences.insert(project_id, actor_seq);
        let grant = PendingGrant {
            project_id,
            target_device_id,
            target_public_key,
            actor_seq,
        };
        candidate.pending_grants.push(grant.clone());
        self.commit(directory, candidate)?;
        Ok(grant)
    }

    pub fn pending_grants(&self, project_id: Option<Uuid>) -> Vec<PendingGrant> {
        self.value
            .pending_grants
            .iter()
            .filter(|grant| project_id.is_none_or(|project| grant.project_id == project))
            .cloned()
            .collect()
    }

    pub fn complete_grant(&mut self, directory: &Path, grant: &PendingGrant) -> Result<()> {
        let mut candidate = self.value.clone();
        candidate.pending_grants.retain(|item| {
            item.project_id != grant.project_id
                || item.target_device_id != grant.target_device_id
                || item.actor_seq != grant.actor_seq
        });
        self.commit(directory, candidate)
    }
}

fn store(directory: &Path, settings: &Settings) -> Result<()> {
    let destination = directory.join("settings.json");
    let temporary = directory.join(format!("settings.json.{}.tmp", Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let bytes = serde_json::to_vec(settings)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .context("could not create temporary settings file")?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        snapshot::atomic_replace(&temporary, &destination)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}
