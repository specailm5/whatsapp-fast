// Register the app's custom #[tauri::command] functions with the Tauri 2 ACL so
// they can be invoked and permission-granted. This is required because:
//   - the remote https://web.whatsapp.com webview cannot call custom (non-plugin)
//     commands from a non-local origin unless they are registered here; and
//   - in some builds the ACL still requires the permission to exist for any origin.
// Listing a command here makes tauri_build autogenerate allow-<command-kebab> /
// deny-<command-kebab> permissions, referenced (bare, no prefix) from capabilities.
//
// Keep this list in sync with the commands in generate_handler! in src/lib.rs.
const APP_COMMANDS: &[&str] = &[
    "settings_get",
    "settings_set_autostart",
    "settings_set_hotkey",
    "settings_set_blur_hotkey",
    "settings_set_panic_hotkey",
    "settings_reset_view",
    "summon_window",
    "open_settings_window",
    "privacy_get",
    "privacy_set",
    "privacy_toggle_all",
    "panic_toggle",
    "badge_set_count",
    "settings_set_unread_badge",
    "settings_set_flash",
    "settings_set_close_to_tray",
    "settings_set_start_minimized",
    "quit_app",
];

fn main() {
    let app_manifest = tauri_build::AppManifest::new().commands(APP_COMMANDS);
    let attributes = tauri_build::Attributes::new().app_manifest(app_manifest);
    tauri_build::try_build(attributes).expect("tauri build script failed");
}
