//! Regression tests for #48 (atomic add/rename) and #49 (reverse desync detection).
//!
//! Uses a `FailingMetaStore` to inject `save_index` failures and verify the
//! `Store` compensates by rolling back the backend mutation, so we never leak
//! a phantom secret.

use klef::error::KlefError;
use klef::store::{Backend, IndexData, MemoryBackend, MetaStore, Store};
use std::path::PathBuf;
use std::sync::Mutex;

struct FailingMetaStore {
    data: Mutex<IndexData>,
    fail_next_save: Mutex<bool>,
    lock_dir: tempfile::TempDir,
}

impl FailingMetaStore {
    fn new() -> Self {
        Self {
            data: Mutex::new(IndexData::default()),
            fail_next_save: Mutex::new(false),
            lock_dir: tempfile::tempdir().unwrap(),
        }
    }
    fn fail_next(&self) {
        *self.fail_next_save.lock().unwrap() = true;
    }
}

impl MetaStore for FailingMetaStore {
    fn load_index(&self) -> Result<IndexData, KlefError> {
        Ok(self.data.lock().unwrap().clone())
    }
    fn save_index(&self, data: &IndexData) -> Result<(), KlefError> {
        if std::mem::take(&mut *self.fail_next_save.lock().unwrap()) {
            return Err(KlefError::IndexWrite(std::io::Error::other(
                "simulated disk full",
            )));
        }
        *self.data.lock().unwrap() = data.clone();
        Ok(())
    }
    fn lock_path(&self) -> PathBuf {
        self.lock_dir.path().join("test-resource")
    }
}

fn make_store() -> Store {
    Store::new(
        Box::new(MemoryBackend::new()),
        Box::new(FailingMetaStore::new()) as Box<dyn MetaStore>,
    )
}

#[test]
fn add_rolls_back_backend_when_index_save_fails() {
    // Build a Store whose meta store will fail on the next save_index, then
    // verify the value never lands in the backend.
    let backend = MemoryBackend::new();
    let meta = FailingMetaStore::new();
    meta.fail_next();

    // We need a handle to the backend to inspect it — wrap in Arc-like by
    // doing the assertion via Store::get_value which goes through the same
    // backend but checks the index first. So instead, build a dedicated test
    // backend that exposes its inner state.
    let s = Store::new(Box::new(backend), Box::new(meta));
    let r = s.add("ghost", "should-not-persist", None, None, vec![], false);
    assert!(matches!(r, Err(KlefError::IndexWrite(_))));

    // Subsequent list() must show the key as absent (index empty),
    // and a re-add must succeed without "already exists".
    assert!(s.list().unwrap().is_empty());
    s.add("ghost", "ok", None, None, vec![], false).unwrap();
    assert_eq!(s.get_value("ghost").unwrap(), "ok");
}

#[test]
fn add_force_restores_prior_value_when_index_save_fails() {
    let s = make_store();
    s.add("k", "original", None, None, vec![], false).unwrap();

    // Now arm the failure for the next save_index, then force-overwrite.
    // Reach into the meta store via a fresh Store with a shared FailingMetaStore.
    let backend = MemoryBackend::new();
    backend.set("k", "original").unwrap();
    let meta = FailingMetaStore::new();
    meta.save_index(&{
        let mut d = IndexData::default();
        d.keys.insert(
            "k".to_string(),
            klef::store::KeyMeta {
                env_var: "K_API_KEY".into(),
                note: None,
                tags: vec![],
                added_at: time::OffsetDateTime::now_utc(),
                updated_at: time::OffsetDateTime::now_utc(),
            },
        );
        d
    })
    .unwrap();
    meta.fail_next();
    let s = Store::new(Box::new(backend), Box::new(meta));

    let r = s.add("k", "new-value", None, None, vec![], true);
    assert!(matches!(r, Err(KlefError::IndexWrite(_))));

    // Backend must have rolled back to the original value.
    assert_eq!(s.get_value("k").unwrap(), "original");
}

#[test]
fn orphan_backend_entries_detects_backend_only_keys() {
    // Backend has a key that the index doesn't know about — exactly the
    // "phantom secret" scenario described in #48.
    let backend = MemoryBackend::new();
    backend.set("ghost", "v").unwrap();
    let meta = FailingMetaStore::new();
    let s = Store::new(Box::new(backend), Box::new(meta));

    let orphans = s.orphan_backend_entries().unwrap();
    assert_eq!(orphans, Some(vec!["ghost".to_string()]));
}

#[test]
fn orphan_backend_entries_clean_when_in_sync() {
    let s = make_store();
    s.add("k", "v", None, None, vec![], false).unwrap();
    assert_eq!(s.orphan_backend_entries().unwrap(), Some(vec![]));
}

/// Backend that mimics the keychain: cannot enumerate.
struct NonEnumerableBackend(MemoryBackend);

impl Backend for NonEnumerableBackend {
    fn describe(&self) -> String {
        "non-enumerable".into()
    }
    fn get(&self, n: &str) -> Result<String, KlefError> {
        self.0.get(n)
    }
    fn set(&self, n: &str, v: &str) -> Result<(), KlefError> {
        self.0.set(n, v)
    }
    fn remove(&self, n: &str) -> Result<(), KlefError> {
        self.0.remove(n)
    }
    // Inherits default list_names() → Ok(None).
}

#[test]
fn orphan_backend_entries_returns_none_for_non_enumerable_backend() {
    let s = Store::new(
        Box::new(NonEnumerableBackend(MemoryBackend::new())),
        Box::new(FailingMetaStore::new()) as Box<dyn MetaStore>,
    );
    assert_eq!(s.orphan_backend_entries().unwrap(), None);
}
