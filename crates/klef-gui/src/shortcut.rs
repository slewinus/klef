//! The user-configurable global shortcut that toggles the popover.
//!
//! Rust owns registration (the webview holds no global-shortcut permission);
//! the chosen accelerator lives in the frontend settings and is pushed here
//! on launch and on save via the `set_shortcut` command.

use std::sync::{Mutex, PoisonError};
use tauri_plugin_global_shortcut::{GlobalShortcutExt as _, Shortcut};

static CURRENT: Mutex<Option<Shortcut>> = Mutex::new(None);

/// Swap the registered shortcut for `accelerator` (e.g. `"Cmd+Shift+K"`).
/// On failure the previous shortcut is restored, so a taken chord never
/// leaves the user with no hotkey at all.
///
/// # Errors
/// Unparseable accelerator, or registration refused (typically: another app
/// already owns that chord).
pub fn set(app: &tauri::AppHandle, accelerator: &str) -> Result<(), String> {
    let new: Shortcut = accelerator
        .parse()
        .map_err(|e| format!("invalid shortcut “{accelerator}”: {e}"))?;
    let gs = app.global_shortcut();
    let mut current = CURRENT.lock().unwrap_or_else(PoisonError::into_inner);
    if *current == Some(new) {
        return Ok(());
    }
    if let Some(old) = *current {
        let _ = gs.unregister(old);
    }
    let result = gs.register(new);
    *current = if result.is_ok() {
        Some(new)
    } else {
        current.filter(|old| gs.register(*old).is_ok())
    };
    drop(current);
    result.map_err(|e| {
        format!("couldn't register “{accelerator}” ({e}) — another app may already use it")
    })
}

#[cfg(test)]
mod tests {
    use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut};

    /// Contract with `ui/lib/shortcut.ts`: the recorder emits
    /// `KeyboardEvent.code` tokens, and the default must be ⌘⇧K.
    #[test]
    fn frontend_accelerators_parse() {
        let default: Shortcut = "Cmd+Shift+KeyK".parse().unwrap();
        assert_eq!(
            default,
            Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyK)
        );
        for accel in [
            "Ctrl+Alt+Digit1",
            "Cmd+Space",
            "Alt+Shift+ArrowUp",
            "Cmd+Comma",
        ] {
            assert!(accel.parse::<Shortcut>().is_ok(), "{accel}");
        }
    }
}
