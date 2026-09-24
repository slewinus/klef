//! Tauri commands. Every store call goes through [`blocking`] so a keychain
//! password prompt or an age decrypt never freezes the popover.

use crate::state::{AppState, blocking};
use klef_core::KeyDto;
use klef_core::error::KlefError;
use klef_core::store::{AgeBackend, Store};
use std::path::PathBuf;
use std::time::Duration;
use tauri::State;

#[tauri::command]
pub async fn list_keys(state: State<'_, AppState>) -> Result<Vec<KeyDto>, String> {
    let st = state.inner().clone();
    blocking(move || {
        st.store()
            .list()
            .map(|e| e.into_iter().map(KeyDto::from).collect())
    })
    .await
}

/// Copy a secret to the clipboard without it ever entering the webview.
/// `clear_after_secs`: `None` or `0` disables the auto-clear.
#[tauri::command]
pub async fn copy_key(
    name: String,
    clear_after_secs: Option<u64>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let st = state.inner().clone();
    blocking(move || {
        let value = zeroize::Zeroizing::new(st.store().get_value(&name)?);
        copy_to_clipboard(
            &value,
            clear_after_secs.filter(|s| *s > 0).map(Duration::from_secs),
        )?;
        // GUI-only — `klef get` stays a pure read so scripts can't pollute the
        // field. Non-fatal: the user already has the value.
        let recorded = st.store().record_access(&name);
        if let Err(e) = recorded {
            eprintln!("klef-gui: record_access({name}): {e}");
        }
        Ok::<_, KlefError>(())
    })
    .await
}

#[cfg(target_os = "macos")]
#[allow(clippy::unnecessary_wraps)] // mirrors the non-macOS signature
fn copy_to_clipboard(value: &str, clear_after: Option<Duration>) -> Result<(), KlefError> {
    crate::clipboard::copy_secret(value, clear_after);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn copy_to_clipboard(_: &str, _: Option<Duration>) -> Result<(), KlefError> {
    Err(KlefError::BackendUnavailable(
        "clipboard copy is macOS-only".into(),
    ))
}

#[tauri::command]
pub async fn add_key(
    name: String,
    value: String,
    env_var: Option<String>,
    note: Option<String>,
    tags: Vec<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let st = state.inner().clone();
    // force=false → "Add" not "Update". env_var=None lets Store::add derive
    // the CLI default.
    blocking(move || st.store().add(&name, &value, env_var, note, tags, false)).await
}

#[tauri::command]
pub async fn delete_key(name: String, state: State<'_, AppState>) -> Result<(), String> {
    let st = state.inner().clone();
    blocking(move || st.store().remove(&name)).await
}

/// Update a key, renaming it first when `new_name` differs.
///
/// `value = None` keeps the current secret: it is re-read from the backend
/// rather than round-tripped through the webview, so a metadata-only edit
/// never exposes the plaintext to JS.
#[tauri::command]
#[allow(clippy::too_many_arguments)] // flat IPC payload
pub async fn edit_key(
    name: String,
    new_name: Option<String>,
    value: Option<String>,
    env_var: Option<String>,
    note: Option<String>,
    tags: Vec<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let st = state.inner().clone();
    blocking(move || {
        let store = st.store();
        let name = match new_name.filter(|n| *n != name) {
            Some(new) => {
                store.rename(&name, &new)?;
                new
            }
            None => name,
        };
        let value = match value {
            Some(v) => v,
            None => store.get_value(&name)?,
        };
        store.add(&name, &value, env_var, note, tags, true)
    })
    .await
}

/// Switch to an existing age vault. The passphrase is checked by decrypting
/// before the swap, so a typo leaves the current vault in place.
#[tauri::command]
pub async fn open_age_vault(
    path: String,
    passphrase: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let st = state.inner().clone();
    blocking(move || {
        let path = PathBuf::from(path);
        if !path.is_file() {
            return Err(format!("no vault file at {}", path.display()));
        }
        let age = AgeBackend::with_passphrase(path, passphrase);
        let store = Store::new(Box::new(age.clone()), Box::new(age));
        store.list().map_err(|e| match e {
            KlefError::BackendUnavailable(m) if m.starts_with("age decrypt") => {
                "wrong passphrase, or not an age vault".to_string()
            }
            other => other.to_string(),
        })?;
        st.swap_store(store);
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn use_keychain(state: State<'_, AppState>) -> Result<(), String> {
    let st = state.inner().clone();
    blocking(move || klef_core::build_store(None).map(|s| st.swap_store(s))).await
}

#[tauri::command]
#[allow(clippy::needless_pass_by_value)] // Tauri commands take owned args
pub fn set_shortcut(accelerator: String, app: tauri::AppHandle) -> Result<(), String> {
    crate::shortcut::set(&app, &accelerator)
}

#[tauri::command]
pub fn open_keychain_access() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return std::process::Command::new("open")
        .args(["-a", "Keychain Access"])
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string());
    #[cfg(not(target_os = "macos"))]
    Err("only available on macOS".to_string())
}

/// Quit klef-gui. `#[allow]`: Tauri commands require owned `AppHandle`.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}
