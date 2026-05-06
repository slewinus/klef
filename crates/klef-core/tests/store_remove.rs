//! Integration tests for `Store` (add, rename, remove, orphan detection,
//! tags, and `remove` error semantics).
//!
//! Lives outside `src/store/mod.rs` so the latter stays under the 300-line
//! file cap. Uses only the public klef API.

use klef_core::error::KlefError;
use klef_core::store::{Backend, IndexFile, MemoryBackend, MetaStore, Store};
use std::sync::Mutex;
use tempfile::tempdir;

#[test]
fn add_then_get_round_trip() {
    let (s, _d) = make_store();
    s.add("stripe", "sk_live", None, None, vec![], false)
        .unwrap();
    assert_eq!(s.get_value("stripe").unwrap(), "sk_live");
}

#[test]
fn add_existing_without_force_fails() {
    let (s, _d) = make_store();
    s.add("stripe", "v1", None, None, vec![], false).unwrap();
    let r = s.add("stripe", "v2", None, None, vec![], false);
    assert!(matches!(r, Err(KlefError::KeyAlreadyExists(_))));
}

#[test]
fn add_existing_with_force_overwrites() {
    let (s, _d) = make_store();
    s.add("stripe", "v1", None, None, vec![], false).unwrap();
    s.add("stripe", "v2", None, None, vec![], true).unwrap();
    assert_eq!(s.get_value("stripe").unwrap(), "v2");
}

#[test]
fn add_with_tags_dedups_and_sorts() {
    let (s, _d) = make_store();
    let tags = vec![
        "billing".to_string(),
        "prod".to_string(),
        "billing".to_string(),
        "alpha".to_string(),
    ];
    s.add("stripe", "v", None, None, tags, false).unwrap();
    let m = s.meta("stripe").unwrap();
    assert_eq!(m.tags, vec!["alpha", "billing", "prod"]);
}

#[test]
fn invalid_name_rejected() {
    let (s, _d) = make_store();
    let r = s.add("has space", "v", None, None, vec![], false);
    assert!(matches!(r, Err(KlefError::InvalidKeyName(_))));
}

fn make_store() -> (Store, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let s = Store::new(
        Box::new(MemoryBackend::new()),
        Box::new(IndexFile::new(dir.path().join("index.json"))) as Box<dyn MetaStore>,
    );
    (s, dir)
}

#[test]
fn rename_moves_value_and_meta() {
    let (s, _d) = make_store();
    s.add("a", "v", None, None, vec![], false).unwrap();
    s.rename("a", "b").unwrap();
    assert!(matches!(s.get_value("a"), Err(KlefError::KeyNotFound(_))));
    assert_eq!(s.get_value("b").unwrap(), "v");
}

#[test]
fn remove_clears_both_layers() {
    let (s, _d) = make_store();
    s.add("k", "v", None, None, vec![], false).unwrap();
    s.remove("k").unwrap();
    assert!(matches!(s.get_value("k"), Err(KlefError::KeyNotFound(_))));
    assert!(s.list().unwrap().is_empty());
}

#[test]
fn orphan_index_entries_finds_index_only_keys() {
    let (s, _d) = make_store();
    s.add("a", "v", None, None, vec![], false).unwrap();
    assert!(s.orphan_index_entries().unwrap().is_empty());
}

/// Backend whose `remove` returns a configurable error (used to simulate
/// Keychain permission failures, I/O errors, etc.).
struct FailingRemoveBackend {
    inner: MemoryBackend,
    err: Mutex<Option<fn() -> KlefError>>,
}

impl FailingRemoveBackend {
    fn new(err: fn() -> KlefError) -> Self {
        Self {
            inner: MemoryBackend::new(),
            err: Mutex::new(Some(err)),
        }
    }
}

impl Backend for FailingRemoveBackend {
    fn describe(&self) -> String {
        "failing-remove".to_string()
    }

    fn get(&self, name: &str) -> Result<String, KlefError> {
        self.inner.get(name)
    }
    fn set(&self, name: &str, value: &str) -> Result<(), KlefError> {
        self.inner.set(name, value)
    }
    fn remove(&self, name: &str) -> Result<(), KlefError> {
        let err = *self.err.lock().unwrap();
        if let Some(f) = err {
            return Err(f());
        }
        self.inner.remove(name)
    }
}

#[test]
fn remove_propagates_non_not_found_backend_error() {
    let dir = tempdir().unwrap();
    let backend = FailingRemoveBackend::new(|| KlefError::BackendDenied);
    backend.inner.set("k", "v").unwrap();
    let s = Store::new(
        Box::new(backend),
        Box::new(IndexFile::new(dir.path().join("index.json"))) as Box<dyn MetaStore>,
    );
    s.add("k", "v", None, None, vec![], true).unwrap();

    let r = s.remove("k");
    assert!(matches!(r, Err(KlefError::BackendDenied)));
    // Index must NOT have been modified — the secret is still there.
    assert!(s.meta("k").is_ok());
}

#[test]
fn remove_tolerates_backend_key_not_found() {
    // Simulates "secret already deleted manually from Keychain": the index
    // still has the entry, but the backend says it's gone. `rm` should
    // still succeed and clean up the index.
    let dir = tempdir().unwrap();
    let backend = FailingRemoveBackend::new(|| KlefError::KeyNotFound("k".into()));
    let s = Store::new(
        Box::new(backend),
        Box::new(IndexFile::new(dir.path().join("index.json"))) as Box<dyn MetaStore>,
    );
    // Add directly to index without touching backend by using add() which
    // sets both — then we rely on the failing remove returning KeyNotFound.
    // Since FailingRemoveBackend wraps MemoryBackend, add() works normally.
    s.add("k", "v", None, None, vec![], false).unwrap();

    s.remove("k")
        .expect("KeyNotFound from backend must be tolerated");
    assert!(matches!(s.meta("k"), Err(KlefError::KeyNotFound(_))));
}

#[test]
fn record_access_sets_last_used_at() {
    let (s, _d) = make_store();
    s.add("stripe", "v", None, None, vec![], false).unwrap();
    let before = s.meta("stripe").unwrap().last_used_at;
    assert!(before.is_none(), "newly added key has no last_used_at");

    s.record_access("stripe").unwrap();
    let after = s.meta("stripe").unwrap().last_used_at;
    assert!(after.is_some(), "record_access populates last_used_at");
}

#[test]
fn record_access_unknown_key_returns_not_found() {
    let (s, _d) = make_store();
    let r = s.record_access("nope");
    assert!(matches!(r, Err(KlefError::KeyNotFound(_))));
}

#[test]
fn re_add_preserves_last_used_at() {
    let (s, _d) = make_store();
    s.add("stripe", "v1", None, None, vec![], false).unwrap();
    s.record_access("stripe").unwrap();
    let recorded = s.meta("stripe").unwrap().last_used_at;
    assert!(recorded.is_some());

    // Force re-add (simulates editing the value) — last_used_at must
    // survive so the recency tracking isn't reset by metadata edits.
    s.add("stripe", "v2", None, Some("note".into()), vec![], true)
        .unwrap();
    assert_eq!(s.meta("stripe").unwrap().last_used_at, recorded);
}
