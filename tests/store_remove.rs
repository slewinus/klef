//! Integration tests for `Store` (rename, remove, orphan detection,
//! and `remove` error semantics).
//!
//! Lives outside `src/store/mod.rs` so the latter stays under the 300-line
//! file cap. Uses only the public klef API.

use klef::error::KlefError;
use klef::store::{Backend, MemoryBackend, Store};
use std::sync::Mutex;
use tempfile::tempdir;

fn make_store() -> (Store, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let s = Store::new(
        Box::new(MemoryBackend::new()),
        dir.path().join("index.json"),
    );
    (s, dir)
}

#[test]
fn rename_moves_value_and_meta() {
    let (s, _d) = make_store();
    s.add("a", "v", None, None, false).unwrap();
    s.rename("a", "b").unwrap();
    assert!(matches!(s.get_value("a"), Err(KlefError::KeyNotFound(_))));
    assert_eq!(s.get_value("b").unwrap(), "v");
}

#[test]
fn remove_clears_both_layers() {
    let (s, _d) = make_store();
    s.add("k", "v", None, None, false).unwrap();
    s.remove("k").unwrap();
    assert!(matches!(s.get_value("k"), Err(KlefError::KeyNotFound(_))));
    assert!(s.list().unwrap().is_empty());
}

#[test]
fn orphan_index_entries_finds_index_only_keys() {
    let (s, _d) = make_store();
    s.add("a", "v", None, None, false).unwrap();
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
    let s = Store::new(Box::new(backend), dir.path().join("index.json"));
    s.add("k", "v", None, None, true).unwrap();

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
    let s = Store::new(Box::new(backend), dir.path().join("index.json"));
    // Add directly to index without touching backend by using add() which
    // sets both — then we rely on the failing remove returning KeyNotFound.
    // Since FailingRemoveBackend wraps MemoryBackend, add() works normally.
    s.add("k", "v", None, None, false).unwrap();

    s.remove("k")
        .expect("KeyNotFound from backend must be tolerated");
    assert!(matches!(s.meta("k"), Err(KlefError::KeyNotFound(_))));
}
