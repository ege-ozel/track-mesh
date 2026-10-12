use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
use uuid::Uuid;

const VERSION: u32 = 1;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    version: u32,
    upnp_mapping_enabled: bool,
}

pub(super) fn load_upnp_mapping(directory: &Path) -> bool {
    let path = directory.join("settings.json");
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    serde_json::from_slice::<Settings>(&bytes)
        .is_ok_and(|value| value.version == VERSION && value.upnp_mapping_enabled)
}

pub(super) fn store_upnp_mapping(directory: &Path, enabled: bool) -> Result<()> {
    let destination = directory.join("settings.json");
    let temporary = directory.join(format!("settings.json.{}.tmp", Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let bytes = serde_json::to_vec(&Settings {
            version: VERSION,
            upnp_mapping_enabled: enabled,
        })?;
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
