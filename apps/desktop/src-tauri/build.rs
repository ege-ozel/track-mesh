fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_status",
            "choose_folder",
            "open_project_folder",
            "open_projects_location",
            "choose_projects_location",
            "choose_invite_parent_folder",
            "open_invite_parent_folder",
            "paste_invite_link",
            "submit_action",
            "set_upnp_mapping",
            "close_app",
        ]),
    ))
    .expect("TrackMesh desktop build configuration is invalid");
}
