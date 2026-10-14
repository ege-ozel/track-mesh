//! The native tray is owned by the existing desktop process and network session.
use crate::runtime::Desktop;
use anyhow::{Context, Result};
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

pub fn install(app: &AppHandle) -> Result<()> {
    let open = MenuItem::with_id(app, "trackmesh-open", "Open TRACKMESH", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "trackmesh-quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let icon = app
        .default_window_icon()
        .cloned()
        .context("TRACKMESH tray icon is unavailable")?;
    TrayIconBuilder::with_id("trackmesh-tray")
        .icon(icon)
        .tooltip("TRACKMESH — running")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let desktop = app.state::<Arc<Desktop>>();
            match event.id.as_ref() {
                "trackmesh-open" => desktop.reopen(app),
                "trackmesh-quit" => desktop.request_quit(app.clone()),
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                let app = tray.app_handle();
                app.state::<Arc<Desktop>>().reopen(app);
            }
        })
        .build(app)?;
    Ok(())
}
