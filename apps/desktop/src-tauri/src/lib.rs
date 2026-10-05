//! Windows desktop adapter. Core owns project decisions; this adapter owns the window session.
#[cfg(not(target_os = "windows"))]
compile_error!("TrackMesh desktop MVP supports Windows only");
mod commands;
mod documents;
mod runtime;

use clap::Parser;
use runtime::{Desktop, Options};
use std::sync::Arc;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

pub fn run() {
    let options = Options::parse();
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "trackmesh_core=info,trackmesh_transfer=info,trackmesh_network=info".into()
            }),
        )
        .init();
    let desktop = Arc::new(Desktop::new(options));
    let setup_desktop = desktop.clone();
    let application = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(desktop.clone())
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::choose_folder,
            commands::submit_action,
            commands::close_app
        ])
        .setup(move |app| {
            WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .title("TRACKMESH")
                .inner_size(1180.0, 820.0)
                .min_inner_size(820.0, 620.0)
                .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
                .on_navigation(|url| {
                    (url.scheme() == "tauri" && url.host_str() == Some("localhost"))
                        || (matches!(url.scheme(), "http" | "https")
                            && url.host_str() == Some("tauri.localhost"))
                })
                .build()?;
            setup_desktop.start();
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let desktop = window.state::<Arc<Desktop>>();
                if !desktop.is_closed() {
                    api.prevent_close();
                    desktop.begin_close(window.app_handle().clone());
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("could not start TRACKMESH desktop");
    application.run(move |app, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event {
            if !desktop.is_closed() {
                api.prevent_exit();
                desktop.begin_close(app.clone());
            }
        }
    });
}
