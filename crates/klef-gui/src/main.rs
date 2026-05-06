// Prevents an extra console window on Windows in release. macOS-only for v0.1
// but the cfg attribute is harmless on other targets.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use klef_core::{KeyDto, build_store};
use tauri::{
    Emitter as _, Manager as _,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

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

// `get_key_value` returns secret plaintext to the webview because the
// clipboard plugin runs JS-side, not Rust-side. Surface and mitigations:
//   - The webview also has `clipboard-manager:read-text` (granted in
//     capabilities/default.json) for the auto-clear verification — it
//     reads back the clipboard before clearing so we don't wipe content
//     the user copied from elsewhere within the timeout window.
//   - The CSP `connect-src` is restricted to Tauri IPC and 'self', so
//     the secret cannot be exfiltrated to a remote host.
//   - The Svelte side does not retain the secret beyond the copy call;
//     the auto-clear timer keeps a copy of the last-written string only
//     to compare it back, then drops it.
//   - `edit_key` deliberately re-reads the value from the backend when
//     the user edits metadata only, so a metadata-only update never
//     surfaces the plaintext to JS in the first place.
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

/// Place the window centered under the tray icon, querying the icon's
/// screen rect directly from Tauri (works even before the user has clicked
/// the tray for the first time, unlike `Position::TrayCenter` from the
/// positioner plugin which depends on `on_tray_event` having fired).
///
/// Returns true when the position was successfully computed and applied.
fn place_under_tray(app: &tauri::AppHandle, window: &tauri::WebviewWindow) -> bool {
    let Some(tray) = app.tray_by_id("main") else {
        return false;
    };
    let Ok(Some(rect)) = tray.rect() else {
        return false;
    };
    let scale = window.scale_factor().unwrap_or(1.0);
    // Tauri 2's Rect carries position/size as enums (Physical or Logical
    // depending on platform). Convert to physical pixels for set_position.
    let pos = rect.position.to_physical::<f64>(scale);
    let size = rect.size.to_physical::<f64>(scale);
    let win = window.outer_size().unwrap_or_default();
    let tray_center_x = pos.x + size.width / 2.0;
    #[allow(clippy::cast_possible_truncation)]
    let x = (tray_center_x - f64::from(win.width) / 2.0).round() as i32;
    #[allow(clippy::cast_possible_truncation)]
    let y = (pos.y + size.height + 4.0).round() as i32;
    let _ = window.set_position(tauri::PhysicalPosition::new(x.max(0), y.max(0)));
    true
}

/// Last-resort fallback: top-right of the current monitor with margins.
/// Used only when `tray.rect()` is unavailable (e.g. the tray icon is
/// not yet attached to a screen by the OS).
fn place_top_right_with_margin(window: &tauri::WebviewWindow) {
    let Ok(Some(monitor)) = window.current_monitor() else {
        return;
    };
    let screen = monitor.size();
    let win = window.outer_size().unwrap_or_default();
    let x = i32::try_from(screen.width.saturating_sub(win.width)).unwrap_or(0) - 12;
    let _ = window.set_position(tauri::PhysicalPosition::new(x.max(0), 32));
}

fn toggle_window(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
    } else {
        if !place_under_tray(app, &window) {
            place_top_right_with_margin(&window);
        }
        let _ = window.show();
        let _ = window.set_focus();
        // Notify the frontend that the popover just opened so it can
        // refresh data and refocus the search bar. The DOM `focus` event
        // isn't reliable on Tauri's webview when toggling visibility — the
        // OS-level show/hide doesn't always propagate as a JS focus event.
        let _ = window.emit("popover-shown", ());
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
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
                // Don't fail the whole app if another process already owns
                // ⌘⇧K (e.g. another launcher utility). The tray icon click
                // still works as a fallback. A future Settings UI (S7) will
                // let users pick a different chord.
                match app.global_shortcut().register(shortcut) {
                    Ok(()) => eprintln!("klef-gui: ⌘⇧K registered"),
                    Err(e) => eprintln!(
                        "klef-gui: failed to register ⌘⇧K ({e}); tray icon click still works"
                    ),
                }
            }

            // Hide the Dock icon AFTER the tray is up, so we never end up in
            // a state where the app is dock-less with no menu bar entry.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            eprintln!("klef-gui: setup done — click the tray icon or ⌘⇧K");

            Ok(())
        })
        .on_window_event(|_window, _event| {
            // Auto-hide-on-blur is intentionally disabled for v0.1.
            //
            // Original intent: clicking outside the popover dismisses it
            // (standard macOS menu bar utility behavior). But opening a
            // modal (Add/Edit/Delete) shifts focus inside the webview,
            // which macOS occasionally reports as a window-level
            // Focused(false). The popover hides mid-modal-mount, leaving
            // a frozen-half-rendered black rectangle (the backdrop) in
            // a state where neither the modal nor the popover responds.
            //
            // Trade-off: dismissal is via Escape, re-click on the tray
            // icon, or ⌘⇧K. A future sprint can restore auto-hide by
            // suppressing it while a modal is open (e.g. JS emits a
            // `modal-open`/`modal-closed` event that flips a Rust flag).
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
