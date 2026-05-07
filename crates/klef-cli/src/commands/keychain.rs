//! `klef keychain configure` — explicit user-action that disables macOS
//! keychain auto-lock (no timeout, no lock-on-sleep) and writes a marker
//! recording the prior state for revert.

#![cfg(target_os = "macos")]

use klef_core::error::KlefError;
use klef_core::macos_keychain::{
    KeychainStatus, apply_friendly_settings, build_revert_command, current_status,
    is_already_friendly,
};
use std::io::Write;

const MARKER_FILE: &str = "keychain-configured";

/// Top-level handler for `klef keychain configure`.
///
/// # Errors
///
/// Returns `KlefError::BackendUnavailable` if the underlying
/// `KeychainHelperError` cannot be recovered from. Marker write failures
/// are warnings, not errors (the underlying setting was already applied).
pub fn run() -> Result<(), KlefError> {
    let mut stderr = std::io::stderr();
    let mut stdout = std::io::stdout();
    run_with(
        &mut stdout,
        &mut stderr,
        current_status,
        apply_friendly_settings,
    )
}

fn run_with(
    stdout: &mut impl Write,
    stderr: &mut impl Write,
    read: impl FnOnce() -> Result<KeychainStatus, klef_core::macos_keychain::KeychainHelperError>,
    apply: impl FnOnce(&KeychainStatus) -> Result<(), klef_core::macos_keychain::KeychainHelperError>,
) -> Result<(), KlefError> {
    let prev = read().map_err(|e| KlefError::BackendUnavailable(e.to_string()))?;

    if is_already_friendly(&prev) {
        writeln!(
            stdout,
            "macOS keychain is already configured for klef (no timeout, no lock-on-sleep). Nothing to do."
        )
        .ok();
        write_marker_applied(&prev).ok();
        return Ok(());
    }

    apply(&prev).map_err(|e| KlefError::BackendUnavailable(e.to_string()))?;

    writeln!(
        stdout,
        "macOS keychain configured: no timeout, no lock-on-sleep. You should no longer be prompted for your password during this login session."
    )
    .ok();
    writeln!(stdout, "To revert, run:").ok();
    writeln!(stdout, "    {}", build_revert_command(&prev)).ok();

    if let Err(e) = write_marker_applied(&prev) {
        writeln!(
            stderr,
            "warning: keychain settings updated but marker write failed: {e}"
        )
        .ok();
    }

    Ok(())
}

fn write_marker_applied(prev: &KeychainStatus) -> Result<(), std::io::Error> {
    let path = marker_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let now = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string());
    let body = serde_json::json!({
        "applied": true,
        "configured_at": now,
        "keychain_path": prev.path,
        "prev_timeout_seconds": prev.timeout_seconds,
        "prev_lock_on_sleep": prev.lock_on_sleep,
    });
    std::fs::write(&path, serde_json::to_vec_pretty(&body).unwrap_or_default())?;
    Ok(())
}

fn marker_path() -> Result<std::path::PathBuf, std::io::Error> {
    let base = dirs::config_dir().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "config_dir unavailable")
    })?;
    Ok(base.join("klef").join(MARKER_FILE))
}

#[cfg(test)]
mod tests {
    use super::*;
    use klef_core::macos_keychain::KeychainHelperError;
    use std::cell::RefCell;
    use std::path::PathBuf;

    fn st(timeout_seconds: Option<u64>, lock_on_sleep: bool) -> KeychainStatus {
        KeychainStatus {
            path: PathBuf::from("/Users/alice/Library/Keychains/login.keychain-db"),
            timeout_seconds,
            lock_on_sleep,
        }
    }

    #[test]
    fn idempotent_when_already_friendly_does_not_call_apply() {
        let read_called = RefCell::new(false);
        let apply_called = RefCell::new(false);
        let mut out = Vec::new();
        let mut err = Vec::new();
        run_with(
            &mut out,
            &mut err,
            || {
                *read_called.borrow_mut() = true;
                Ok(st(None, false))
            },
            |_s| {
                *apply_called.borrow_mut() = true;
                Ok(())
            },
        )
        .unwrap();
        assert!(*read_called.borrow());
        assert!(
            !*apply_called.borrow(),
            "apply must not be called when already friendly"
        );
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("already configured"));
    }

    #[test]
    fn applies_and_prints_revert_command_when_not_friendly() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        run_with(&mut out, &mut err, || Ok(st(Some(600), true)), |_s| Ok(())).unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("configured"));
        assert!(s.contains("To revert"));
        assert!(s.contains("-u -t 600"));
        assert!(s.contains(" -l "));
    }

    #[test]
    fn read_failure_propagates_as_backend_unavailable() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let result: Result<(), KlefError> = run_with(
            &mut out,
            &mut err,
            || {
                Err(KeychainHelperError::Parse {
                    cmd: "default-keychain",
                    reason: "boom".into(),
                })
            },
            |_s| Ok(()),
        );
        let e = result.unwrap_err();
        let msg = e.to_string();
        assert!(msg.contains("default-keychain"), "got: {msg}");
    }

    #[test]
    fn apply_failure_propagates_as_backend_unavailable() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let result: Result<(), KlefError> = run_with(
            &mut out,
            &mut err,
            || Ok(st(Some(600), true)),
            |_s| {
                Err(KeychainHelperError::NonZeroExit {
                    cmd: "set-keychain-settings",
                    code: 1,
                    stderr: "denied".into(),
                })
            },
        );
        let e = result.unwrap_err();
        assert!(e.to_string().contains("set-keychain-settings"));
    }
}
