//! Banner trigger for the macOS keychain timeout issue. Decides whether
//! to print a one-time stderr banner pointing the user at
//! `klef keychain configure`. Suppressed via marker file at
//! `~/.config/klef/keychain-configured` with TTL + state-drift re-show.

#![cfg(target_os = "macos")]
#![allow(dead_code)] // Wired into the run() entrypoint in a later task.
#![allow(clippy::redundant_pub_crate)]

use klef_core::macos_keychain::KeychainStatus;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const MARKER_FILE: &str = "keychain-configured";
const TTL: Duration = Duration::from_hours(7 * 24);

#[derive(Debug, Serialize, Deserialize, Default)]
pub(crate) struct Marker {
    pub(crate) applied: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) configured_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) banner_shown_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) banner_state: Option<BannerState>,
    // Revert info — present when applied=true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) keychain_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) prev_timeout_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) prev_lock_on_sleep: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct BannerState {
    pub(crate) keychain_path: PathBuf,
    pub(crate) timeout_seconds: Option<u64>,
    pub(crate) lock_on_sleep: bool,
}

impl BannerState {
    pub(crate) fn from_status(s: &KeychainStatus) -> Self {
        Self {
            keychain_path: s.path.clone(),
            timeout_seconds: s.timeout_seconds,
            lock_on_sleep: s.lock_on_sleep,
        }
    }
    pub(crate) fn matches_status(&self, s: &KeychainStatus) -> bool {
        let path_eq = self.keychain_path == s.path;
        let timeout_eq = self.timeout_seconds == s.timeout_seconds;
        let lock_eq = self.lock_on_sleep == s.lock_on_sleep;
        path_eq && timeout_eq && lock_eq
    }
}

pub(crate) fn marker_path() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("KLEF_KEYCHAIN_MARKER_DIR") {
        return Some(PathBuf::from(dir).join(MARKER_FILE));
    }
    Some(dirs::config_dir()?.join("klef").join(MARKER_FILE))
}

pub(crate) fn load_marker(path: &Path) -> Option<Marker> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub(crate) fn marker_age(path: &Path) -> Option<Duration> {
    let mtime = std::fs::metadata(path).ok()?.modified().ok()?;
    SystemTime::now().duration_since(mtime).ok()
}

/// Decide whether the banner should be re-shown given the current state.
pub(crate) fn should_show_banner(path: &Path, current: &KeychainStatus) -> bool {
    let Some(marker) = load_marker(path) else {
        return true;
    };
    if marker.applied {
        return false;
    }
    // applied=false: re-show when state drifted OR marker is stale.
    let drifted = marker
        .banner_state
        .as_ref()
        .is_none_or(|bs| !bs.matches_status(current));
    if drifted {
        return true;
    }
    matches!(marker_age(path), Some(age) if age > TTL)
}

pub(crate) fn write_banner_shown(
    path: &Path,
    current: &KeychainStatus,
) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let now = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string());
    let payload = serde_json::json!({
        "applied": false,
        "banner_shown_at": now,
        "banner_state": BannerState::from_status(current),
    });
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&payload).unwrap_or_default(),
    )?;
    Ok(())
}

use crate::cli::Command;
use klef_core::macos_keychain::{KeychainHelperError, current_status, is_already_friendly};
use klef_core::store::Store;
use std::io::Write;

const OPT_OUT_ENV: &str = "KLEF_NO_KEYCHAIN_AUTOCONFIG";

/// Whether the given command will read or write a keychain value.
/// Returns `false` for read-only/metadata commands like list/status/completions.
#[must_use]
pub(crate) const fn command_touches_values(cmd: &Command) -> bool {
    matches!(
        cmd,
        Command::Get { .. }
            | Command::Show { .. }
            | Command::Run { .. }
            | Command::Add { .. }
            | Command::Edit { .. }
            | Command::Rm { .. }
            | Command::Rename { .. }
            | Command::Import { .. }
            | Command::Export { .. }
    )
}

/// Whether the resolved store backend is the OS keychain.
pub(crate) fn backend_is_keychain(store: &Store) -> bool {
    store.backend_description() == "keychain"
}

/// Try to emit the banner. All-or-nothing best-effort: returns silently
/// on any error. Caller does not propagate errors.
pub(crate) fn maybe_emit_banner<W: Write>(stderr: &mut W) {
    maybe_emit_with(stderr, current_status);
}

fn maybe_emit_with<W: Write>(
    stderr: &mut W,
    read_status: impl FnOnce() -> Result<KeychainStatus, KeychainHelperError>,
) {
    if std::env::var_os(OPT_OUT_ENV).is_some() {
        return;
    }
    let Some(path) = marker_path() else { return };

    let Ok(status) = read_status() else { return };

    if is_already_friendly(&status) {
        // Nothing to warn about; persist as "applied" so we never check again.
        let _ = write_already_friendly(&path, &status);
        return;
    }

    if !should_show_banner(&path, &status) {
        return;
    }

    let timeout_disp = status
        .timeout_seconds
        .map_or_else(|| "off".to_string(), |s| format!("{s}s"));
    let lock_disp = if status.lock_on_sleep { "yes" } else { "no" };

    let _ = writeln!(
        stderr,
        "klef: heads up — your macOS keychain auto-locks (timeout: {timeout_disp}, lock-on-sleep: {lock_disp})."
    );
    let _ = writeln!(
        stderr,
        "      You'll be prompted for your password on every klef call until this is fixed."
    );
    let _ = writeln!(
        stderr,
        "      One-shot fix (modifies macOS Keychain settings, not your klef data):"
    );
    let _ = writeln!(stderr, "          klef keychain configure");
    let _ = writeln!(stderr, "      To suppress this notice without fixing:");
    let _ = writeln!(stderr, "          export {OPT_OUT_ENV}=1");

    let _ = write_banner_shown(&path, &status);
}

fn write_already_friendly(path: &Path, status: &KeychainStatus) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let now = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string());
    let body = serde_json::json!({
        "applied": true,
        "configured_at": now,
        "keychain_path": status.path,
        "prev_timeout_seconds": status.timeout_seconds,
        "prev_lock_on_sleep": status.lock_on_sleep,
    });
    std::fs::write(path, serde_json::to_vec_pretty(&body).unwrap_or_default())?;
    Ok(())
}

#[cfg(test)]
#[path = "macos_keychain_banner_tests.rs"]
mod tests;
