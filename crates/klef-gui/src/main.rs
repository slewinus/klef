// Prevents an extra console window on Windows in release. macOS-only for v0.1
// but the cfg attribute is harmless on other targets.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "macos")]
mod clipboard;
mod commands;
mod dotenv_import;
mod shortcut;
mod state;
mod tray;

pub use state::AppState;

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    // Only one shortcut is ever registered (see `shortcut`).
                    // Fire on release: macOS repeats presses for held keys.
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Released {
                        tray::toggle_window(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            // Start on the keychain; the frontend switches to an age vault
            // (open_age_vault) once the user unlocks it.
            let store = klef_core::build_store(None).map_err(|e| {
                Box::<dyn std::error::Error>::from(format!("failed to build Store: {e}"))
            })?;
            tauri::Manager::manage(app, AppState::new(store));

            // Build the tray BEFORE flipping the activation policy — if it
            // fails we want a clear error rather than a silent dock-less exit.
            tray::build(app)?;

            // The global shortcut is registered by the frontend on mount
            // (`set_shortcut`), from the user's saved setting.

            // Hide the Dock icon AFTER the tray is up, so we never end up in
            // a state where the app is dock-less with no menu bar entry.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_keys,
            commands::copy_key,
            commands::add_key,
            commands::delete_key,
            commands::edit_key,
            commands::open_age_vault,
            commands::use_keychain,
            commands::set_shortcut,
            commands::open_keychain_access,
            commands::quit_app,
            dotenv_import::preview_dotenv_import,
            dotenv_import::apply_dotenv_import,
            dotenv_import::cancel_dotenv_import,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app, event| {
        // Quitting inside the auto-clear window must not leave a secret behind.
        #[cfg(target_os = "macos")]
        if matches!(event, tauri::RunEvent::Exit) {
            clipboard::clear_pending();
        }
        #[cfg(not(target_os = "macos"))]
        let _ = event;
    });
}
