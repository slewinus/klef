//! Tests split from `macos_keychain_banner.rs` to keep that file under the
//! 300-line cap. Imported via `#[cfg(test)] #[path = "..."] mod tests;`.

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

#[test]
fn opt_out_env_var_suppresses_banner() {
    let tmp = TempDir::new().unwrap();
    // SAFETY: tests in this file are run with --test-threads=1.
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var(OPT_OUT_ENV, "1");
        std::env::set_var("KLEF_KEYCHAIN_MARKER_DIR", tmp.path());
    }
    let mut buf: Vec<u8> = Vec::new();
    maybe_emit_with(&mut buf, || Ok(st()));
    // SAFETY: tests in this file are run with --test-threads=1.
    #[allow(unsafe_code)]
    unsafe {
        std::env::remove_var(OPT_OUT_ENV);
        std::env::remove_var("KLEF_KEYCHAIN_MARKER_DIR");
    }
    assert!(
        buf.is_empty(),
        "expected no output, got: {:?}",
        String::from_utf8_lossy(&buf)
    );
}

#[test]
fn already_friendly_state_writes_applied_marker_no_banner() {
    let tmp = TempDir::new().unwrap();
    // SAFETY: tests in this file are run with --test-threads=1.
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("KLEF_KEYCHAIN_MARKER_DIR", tmp.path());
    }
    let mut buf: Vec<u8> = Vec::new();
    maybe_emit_with(&mut buf, || {
        Ok(KeychainStatus {
            path: PathBuf::from("/p"),
            timeout_seconds: None,
            lock_on_sleep: false,
        })
    });
    // SAFETY: tests in this file are run with --test-threads=1.
    #[allow(unsafe_code)]
    unsafe {
        std::env::remove_var("KLEF_KEYCHAIN_MARKER_DIR");
    }
    assert!(buf.is_empty());
    let marker_file = tmp.path().join(MARKER_FILE);
    let body: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&marker_file).unwrap()).unwrap();
    assert_eq!(body["applied"], true);
}

#[test]
fn unfriendly_state_emits_banner_and_writes_shown_marker() {
    let tmp = TempDir::new().unwrap();
    // SAFETY: tests in this file are run with --test-threads=1.
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("KLEF_KEYCHAIN_MARKER_DIR", tmp.path());
    }
    let mut buf: Vec<u8> = Vec::new();
    maybe_emit_with(&mut buf, || Ok(st()));
    // SAFETY: tests in this file are run with --test-threads=1.
    #[allow(unsafe_code)]
    unsafe {
        std::env::remove_var("KLEF_KEYCHAIN_MARKER_DIR");
    }
    let s = String::from_utf8(buf).unwrap();
    assert!(s.contains("klef keychain configure"));
    assert!(s.contains("KLEF_NO_KEYCHAIN_AUTOCONFIG"));
    let marker_file = tmp.path().join(MARKER_FILE);
    let body: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&marker_file).unwrap()).unwrap();
    assert_eq!(body["applied"], false);
    assert!(body["banner_state"].is_object());
}
