// Prevents an extra console window on Windows in release. macOS-only for v0.1
// but the cfg attribute is harmless on other targets.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use klef_core::{KeyDto, build_store};
use tauri::{
    Manager as _, WindowEvent,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_positioner::{Position, WindowExt as _};

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

// `get_key_value` is the only command that returns a secret to the webview.
// Plaintext is necessary for clipboard copy (Tauri's clipboard plugin runs
// JS-side, not Rust-side). Mitigations:
//   - The CSP forbids exfiltration via connect-src (only Tauri IPC + 'self').
//   - The Svelte `App.svelte` does not retain the value beyond the copy call.
//   - The capability list explicitly grants only clipboard-manager:write-text.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn get_key_value(name: String, state: tauri::State<'_, AppState>) -> Result<String, String> {
    state.store.get_value(&name).map_err(|e| e.to_string())
}

fn toggle_window(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
    } else {
        let _ = window.move_window(Position::TrayCenter);
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_positioner::init())
        .setup(|app| {
            // Build the Store once and share it via Tauri-managed state.
            let store = build_store(None).map_err(|e| {
                Box::<dyn std::error::Error>::from(format!("failed to build Store: {e}"))
            })?;
            app.manage(AppState { store });

            // Hide the Dock icon — this is a menu bar utility, not a windowed
            // app. Equivalent to setting `LSUIElement` in Info.plist for a
            // shipped bundle, but `set_activation_policy` works at runtime
            // for `cargo run` too.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Tray icon: clicking it toggles the popover, anchored under the
            // icon via tauri-plugin-positioner.
            let _tray = TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().unwrap().clone())
                // Template-mode requires a monochrome PNG with alpha so
                // macOS can tint it with the menu bar color. Our S2.2c
                // placeholder is a solid blue square — it renders as
                // invisible under template mode. Disable until we ship a
                // proper alpha-channel logo (S7 polish sprint).
                .icon_as_template(false)
                .on_tray_icon_event(|tray, event| {
                    // The positioner plugin tracks tray geometry from this
                    // hook; without this call, `Position::TrayCenter` falls
                    // back to screen center.
                    tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);

                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_window(tray.app_handle());
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            // Auto-hide the popover when it loses focus — the standard
            // macOS menu bar utility behavior. Users dismiss by clicking
            // away.
            if matches!(event, WindowEvent::Focused(false)) {
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![list_keys, get_key_value])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
