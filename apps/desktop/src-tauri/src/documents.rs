//! Capability-bearing documents never cross the WebView boundary.
use anyhow::{ensure, Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    path::Path,
};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

pub fn read_document<T: DeserializeOwned>(app: &AppHandle) -> Result<T> {
    let path = app
        .dialog()
        .file()
        .add_filter("TrackMesh JSON", &["json"])
        .blocking_pick_file()
        .context("File selection cancelled")?
        .into_path()?;
    let mut data = Vec::new();
    std::fs::File::open(path)?
        .take(64 * 1024 + 1)
        .read_to_end(&mut data)?;
    ensure!(data.len() <= 64 * 1024, "Pairing document exceeds 64 KiB");
    // A parser error may quote an input value; never forward it for invitation documents.
    serde_json::from_slice(&data).map_err(|_| anyhow::anyhow!("Invalid TrackMesh JSON document"))
}

pub fn write_document(app: &AppHandle, name: &str, document: &impl Serialize) -> Result<String> {
    let path = app
        .dialog()
        .file()
        .add_filter("TrackMesh JSON", &["json"])
        .set_file_name(name)
        .blocking_save_file()
        .context("File selection cancelled")?
        .into_path()?;
    ensure!(
        path.extension()
            .is_some_and(|v| v.eq_ignore_ascii_case("json")),
        "Choose a .json filename"
    );
    write_new(&path, document)?;
    Ok(path.display().to_string())
}

fn write_new(path: &Path, document: &impl Serialize) -> Result<()> {
    let data = serde_json::to_vec_pretty(document)?;
    ensure!(data.len() <= 64 * 1024, "Pairing document exceeds 64 KiB");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .context("Choose a new filename; existing files are preserved")?;
    file.write_all(&data)?;
    file.sync_all()?;
    Ok(())
}

pub fn choose_folder(app: &AppHandle) -> Result<Option<String>> {
    app.dialog()
        .file()
        .blocking_pick_folder()
        .map(|path| Ok(path.into_path()?.display().to_string()))
        .transpose()
}
