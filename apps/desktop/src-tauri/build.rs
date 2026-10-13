fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_status",
            "choose_folder",
            "paste_invite_link",
            "submit_action",
            "set_upnp_mapping",
            "close_app",
        ]),
    ))
    .expect("TrackMesh desktop build configuration is invalid");
}
