//! Narrow, main-window-only Tauri entry points.
use crate::{
    documents,
    runtime::{Action, Desktop, DesktopStatus},
};
use std::sync::Arc;
use tauri::{AppHandle, State};
use uuid::Uuid;

#[tauri::command]
pub async fn get_status(
    app: AppHandle,
    desktop: State<'_, Arc<Desktop>>,
    offset: u32,
) -> Result<DesktopStatus, String> {
    let status = desktop
        .status(offset)
        .await
        .map_err(|error| error.to_string())?;
    desktop
        .try_auto_first_receive(app, &status)
        .await
        .map_err(|error| error.to_string())?;
    Ok(status)
}

#[tauri::command]
pub async fn choose_folder(app: AppHandle) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || documents::choose_folder(&app))
        .await
        .map_err(|_| "Folder picker stopped".to_owned())?
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn paste_invite_link(
    app: AppHandle,
    desktop: State<'_, Arc<Desktop>>,
) -> Result<(), String> {
    desktop.paste_invite_link(&app).map_err(|error| {
        if error.to_string() == "An invite is already open" {
            "An invite is already open".to_owned()
        } else {
            "No TrackMesh invite link on the clipboard".to_owned()
        }
    })
}

#[tauri::command]
pub async fn submit_action(
    app: AppHandle,
    desktop: State<'_, Arc<Desktop>>,
    action: Action,
) -> Result<Uuid, String> {
    desktop
        .submit(app, action)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn set_upnp_mapping(
    desktop: State<'_, Arc<Desktop>>,
    enabled: bool,
) -> Result<(), String> {
    desktop
        .set_upnp_mapping(enabled)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn close_app(app: AppHandle, desktop: State<'_, Arc<Desktop>>) -> Result<(), String> {
    desktop.begin_close(app);
    Ok(())
}
