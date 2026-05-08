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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn st() -> KeychainStatus {
        KeychainStatus {
            path: PathBuf::from("/Users/alice/Library/Keychains/login.keychain-db"),
            timeout_seconds: Some(600),
            lock_on_sleep: true,
        }
    }

    #[test]
    fn show_banner_when_marker_missing() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(MARKER_FILE);
        assert!(should_show_banner(&path, &st()));
    }

    #[test]
    fn no_banner_when_applied_marker_present() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(MARKER_FILE);
        let body = serde_json::json!({
            "applied": true,
            "configured_at": "2026-05-07T14:23:45Z",
            "keychain_path": "/p",
            "prev_timeout_seconds": 600,
            "prev_lock_on_sleep": true,
        });
        std::fs::write(&path, serde_json::to_vec(&body).unwrap()).unwrap();
        assert!(!should_show_banner(&path, &st()));
    }

    #[test]
    fn no_banner_when_shown_marker_state_matches_and_recent() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(MARKER_FILE);
        write_banner_shown(&path, &st()).unwrap();
        assert!(!should_show_banner(&path, &st()));
    }

    #[test]
    fn banner_re_shown_when_state_drifts() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(MARKER_FILE);
        write_banner_shown(&path, &st()).unwrap();
        let mut drifted = st();
        drifted.timeout_seconds = Some(30); // changed
        assert!(should_show_banner(&path, &drifted));
    }

    #[test]
    fn banner_re_shown_when_marker_older_than_ttl() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(MARKER_FILE);
        write_banner_shown(&path, &st()).unwrap();
        // Backdate mtime to 8 days ago.
        let old = SystemTime::now() - Duration::from_hours(8 * 24);
        let f = std::fs::File::open(&path).unwrap();
        f.set_modified(old).unwrap();
        assert!(should_show_banner(&path, &st()));
    }
}
