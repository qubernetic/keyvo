//! Generates the Tauri context (configuration, capabilities, icons) and the
//! `allow-<command>` permission of each app command, so the capability in
//! `capabilities/default.json` can name the commands one by one. The list
//! is the one `generate_handler!` registers in `src/main.rs`;
//! `app/src/shell.test.ts` keeps the two equal.

fn main() {
    let manifest = tauri_build::AppManifest::new().commands(&[
        "status",
        "tiles",
        "outline",
        "recipes",
        "press",
        "subscribe",
        "key",
        "edit_key",
        "profile",
        "edit_profile",
        "edit_page",
        "restore_profile",
        "control",
        "edit_control",
        "open_profile",
        "profile_list",
    ]);
    let attributes = tauri_build::Attributes::new().app_manifest(manifest);
    if let Err(error) = tauri_build::try_build(attributes) {
        panic!("tauri-build: {error:#}");
    }
}
