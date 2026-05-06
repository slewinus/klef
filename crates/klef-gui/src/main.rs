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

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn add_key(
    name: String,
    value: String,
    env_var: Option<String>,
    note: Option<String>,
    tags: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    // `force = false`: this command is for "Add", not "Update". The GUI
    // surfaces a separate Edit form (S4.2) that calls add with force=true.
    // env_var: None lets Store::add derive the default (`UPPERCASE_API_KEY`)
    // exactly like the CLI does.
    state
        .store
        .add(&name, &value, env_var, note, tags, false)
        .map_err(|e| e.to_string())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn delete_key(name: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.store.remove(&name).map_err(|e| e.to_string())
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn edit_key(
    name: String,
    value: Option<String>,
    env_var: Option<String>,
    note: Option<String>,
    tags: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    // value=None means "keep the current value" — preserves the secret
    // when the user only edits metadata (the common case). We re-read it
    // from the backend rather than asking the webview to round-trip the
    // plaintext, so a metadata-only edit never exposes the value to JS.
    let value_to_use = match value {
        Some(v) => v,
        None => state.store.get_value(&name).map_err(|e| e.to_string())?,
    };
    state
        .store
        .add(&name, &value_to_use, env_var, note, tags, true)
        .map_err(|e| e.to_string())
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
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    use tauri_plugin_global_shortcut::ShortcutState;
                    // Fire on key release so we don't double-fire on the
                    // user holding ⌘⇧K. macOS sends repeat events for held
                    // keys; release-only filters those out.
                    if event.state == ShortcutState::Released
                        && shortcut.matches(
                            tauri_plugin_global_shortcut::Modifiers::SUPER
                                | tauri_plugin_global_shortcut::Modifiers::SHIFT,
                            tauri_plugin_global_shortcut::Code::KeyK,
                        )
                    {
                        toggle_window(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            eprintln!("klef-gui: setup start");

            // Build the Store once and share it via Tauri-managed state.
            let store = build_store(None).map_err(|e| {
                Box::<dyn std::error::Error>::from(format!("failed to build Store: {e}"))
            })?;
            app.manage(AppState { store });
            eprintln!("klef-gui: store ready");

            // Tray icon: clicking it toggles the popover, anchored under the
            // icon via tauri-plugin-positioner. Build this BEFORE flipping the
            // activation policy — if it fails we want a clear panic rather
            // than a silent dock-less exit.
            let icon = app
                .default_window_icon()
                .ok_or("default_window_icon returned None — check tauri.conf.json bundle.icon")?
                .clone();
            let _tray = TrayIconBuilder::with_id("main")
                .icon(icon)
                // Template-mode requires a monochrome PNG with alpha so
                // macOS can tint it with the menu bar color. Our S2.2c
                // placeholder is a solid blue square — it renders as
                // invisible under template mode. Disable until we ship a
                // proper alpha-channel logo (S7 polish sprint).
                .icon_as_template(false)
                .on_tray_icon_event(|tray, event| {
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
            eprintln!("klef-gui: tray ready");

            // Register the global hotkey ⌘⇧K. The handler is wired in the
            // plugin builder above; here we just declare what to listen for.
            // Using `Modifiers::SUPER` for Cmd to keep the same code path on
            // Linux/Windows when we eventually port (`SUPER` is Cmd on macOS,
            // Win/Meta elsewhere).
            {
                use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt as _, Modifiers};
                let shortcut = tauri_plugin_global_shortcut::Shortcut::new(
                    Some(Modifiers::SUPER | Modifiers::SHIFT),
                    Code::KeyK,
                );
                app.global_shortcut().register(shortcut)?;
                eprintln!("klef-gui: ⌘⇧K registered");
            }

            // Hide the Dock icon AFTER the tray is up, so we never end up in
            // a state where the app is dock-less with no menu bar entry.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            eprintln!("klef-gui: setup done — click the tray icon or ⌘⇧K");

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
        .invoke_handler(tauri::generate_handler![
            list_keys,
            get_key_value,
            add_key,
            delete_key,
            edit_key
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
