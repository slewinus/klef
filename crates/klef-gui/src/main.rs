// Prevents an extra console window on Windows in release. macOS-only for v0.1
// but the cfg attribute is harmless on other targets.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use klef_core::{KeyDto, build_store};
use tauri::Manager as _;

/// State held by the Tauri runtime: a single `Store` instance shared across
/// all commands. Initialized on app startup with the production Keychain
/// backend; backend selection (age) lands in S6 (post-MVP).
struct AppState {
    store: klef_core::store::Store,
}

// Tauri commands receive `State` and `String` by value per the macro contract
// (the macro generates the IPC adapter from these signatures). Clippy's
// "pass by reference" lints don't apply here.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn list_keys(state: tauri::State<'_, AppState>) -> Result<Vec<KeyDto>, String> {
    let entries = state.store.list().map_err(|e| e.to_string())?;
    Ok(entries.into_iter().map(KeyDto::from).collect())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn get_key_value(name: String, state: tauri::State<'_, AppState>) -> Result<String, String> {
    state.store.get_value(&name).map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let store = build_store(None).map_err(|e| {
                Box::<dyn std::error::Error>::from(format!("failed to build Store: {e}"))
            })?;
            app.manage(AppState { store });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![list_keys, get_key_value])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
