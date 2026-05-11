//! The `Store` type — coordinates `Backend` (secret values) and `MetaStore` (metadata).

use crate::backup::BundleEntry;
use crate::error::KlefError;
use crate::store::lock::FileLock;
use crate::store::{Backend, KeyMeta, MetaStore};
use time::OffsetDateTime;

/// Coordinates access to secret values (via `Backend`) and metadata (via `MetaStore`).
pub struct Store {
    pub(super) backend: Box<dyn Backend>,
    pub(super) meta: Box<dyn MetaStore>,
}

impl Store {
    /// Create a new Store backed by the given backend and metadata store.
    #[must_use]
    pub fn new(backend: Box<dyn Backend>, meta: Box<dyn MetaStore>) -> Self {
        Self { backend, meta }
    }

    /// Human-readable backend identifier for diagnostics (e.g. `status`).
    #[must_use]
    pub fn backend_description(&self) -> String {
        self.backend.describe()
    }

    /// Inter-process lock held across load → mutate → save (closes #61).
    fn lock(&self) -> Result<FileLock, KlefError> {
        FileLock::acquire(&self.meta.lock_path())
    }

    /// Add or update a secret by name, optionally with metadata.
    /// # Errors
    /// `InvalidKeyName`, `InvalidEnvVar`, `KeyAlreadyExists`, or backend/index.
    pub fn add(
        &self,
        name: &str,
        value: &str,
        env_var: Option<String>,
        note: Option<String>,
        tags: Vec<String>,
        force: bool,
    ) -> Result<(), KlefError> {
        super::validate_name(name)?;
        let resolved_env_var = env_var.unwrap_or_else(|| super::default_env_var(name));
        super::validate_env_var(&resolved_env_var)?;
        let _lock = self.lock()?;
        let mut data = self.meta.load_index()?;
        if data.keys.contains_key(name) && !force {
            return Err(KlefError::KeyAlreadyExists(name.to_string()));
        }
        let now = OffsetDateTime::now_utc();
        let mut sorted_tags = tags;
        sorted_tags.sort();
        sorted_tags.dedup();
        // Preserve last_used_at across re-adds (metadata edits shouldn't reset it).
        let meta = KeyMeta {
            env_var: resolved_env_var,
            note,
            tags: sorted_tags,
            added_at: data.keys.get(name).map_or(now, |k| k.added_at),
            updated_at: now,
            last_used_at: data.keys.get(name).and_then(|k| k.last_used_at),
        };
        // Atomicity (#48): snapshot → write → save; restore on save failure.
        let prior = self.backend.get(name).ok();
        self.backend.set(name, value)?;
        data.keys.insert(name.to_string(), meta);
        if let Err(e) = self.meta.save_index(&data) {
            if let Some(old) = prior {
                let _ = self.backend.set(name, &old);
            } else {
                let _ = self.backend.remove(name);
            }
            return Err(e);
        }
        Ok(())
    }

    /// Replace the tag set on an existing key.
    /// # Errors
    /// `KeyNotFound` if name doesn't exist; index errors propagated.
    pub fn set_tags(&self, name: &str, tags: Vec<String>) -> Result<(), KlefError> {
        let _lock = self.lock()?;
        let mut data = self.meta.load_index()?;
        let meta = data
            .keys
            .get_mut(name)
            .ok_or_else(|| KlefError::KeyNotFound(name.to_string()))?;
        let mut sorted = tags;
        sorted.sort();
        sorted.dedup();
        meta.tags = sorted;
        meta.updated_at = OffsetDateTime::now_utc();
        self.meta.save_index(&data)?;
        Ok(())
    }

    /// Return a map of tag → number of keys carrying it.
    /// # Errors
    /// Index load error.
    pub fn tags_with_counts(&self) -> Result<std::collections::BTreeMap<String, usize>, KlefError> {
        let data = self.meta.load_index()?;
        let mut counts = std::collections::BTreeMap::new();
        for meta in data.keys.values() {
            for t in &meta.tags {
                *counts.entry(t.clone()).or_insert(0) += 1;
            }
        }
        Ok(counts)
    }

    /// Retrieve the secret value.
    /// # Errors
    /// `KeyNotFound` or backend error.
    pub fn get_value(&self, name: &str) -> Result<String, KlefError> {
        let data = self.meta.load_index()?;
        if !data.keys.contains_key(name) {
            return Err(KlefError::KeyNotFound(name.to_string()));
        }
        self.backend.get(name)
    }

    /// List all stored keys and their metadata.
    /// # Errors
    /// Index load failure.
    pub fn list(&self) -> Result<Vec<(String, KeyMeta)>, KlefError> {
        let data = self.meta.load_index()?;
        Ok(data.keys.into_iter().collect())
    }

    /// Remove a secret and its metadata.
    /// # Errors
    /// `KeyNotFound` or an index error.
    pub fn remove(&self, name: &str) -> Result<(), KlefError> {
        let _lock = self.lock()?;
        let mut data = self.meta.load_index()?;
        if !data.keys.contains_key(name) {
            return Err(KlefError::KeyNotFound(name.to_string()));
        }
        // Tolerate KeyNotFound (concurrent rm), propagate other backend errors.
        match self.backend.remove(name) {
            Ok(()) | Err(KlefError::KeyNotFound(_)) => {}
            Err(e) => return Err(e),
        }
        data.keys.remove(name);
        self.meta.save_index(&data)?;
        Ok(())
    }

    /// Record a copy (updates `last_used_at`). GUI-only.
    /// # Errors
    /// `KeyNotFound` or index error.
    pub fn record_access(&self, name: &str) -> Result<(), KlefError> {
        let _lock = self.lock()?;
        let mut data = self.meta.load_index()?;
        let meta = data
            .keys
            .get_mut(name)
            .ok_or_else(|| KlefError::KeyNotFound(name.to_string()))?;
        meta.last_used_at = Some(OffsetDateTime::now_utc());
        self.meta.save_index(&data)?;
        Ok(())
    }

    /// Retrieve metadata for a key.
    /// # Errors
    /// `KeyNotFound` or index error.
    pub fn meta(&self, name: &str) -> Result<KeyMeta, KlefError> {
        let data = self.meta.load_index()?;
        data.keys
            .get(name)
            .cloned()
            .ok_or_else(|| KlefError::KeyNotFound(name.to_string()))
    }

    /// Update the env-var and/or note for a key.
    /// `None` keeps the existing value; `Some(None)` clears the note.
    /// # Errors
    /// `KeyNotFound` or an index error.
    pub fn update_meta(
        &self,
        name: &str,
        env_var: Option<String>,
        note: Option<Option<String>>,
    ) -> Result<(), KlefError> {
        let _lock = self.lock()?;
        let mut data = self.meta.load_index()?;
        let meta = data
            .keys
            .get_mut(name)
            .ok_or_else(|| KlefError::KeyNotFound(name.to_string()))?;
        if let Some(v) = env_var {
            super::validate_env_var(&v)?;
            meta.env_var = v;
        }
        if let Some(n) = note {
            meta.note = n;
        }
        meta.updated_at = OffsetDateTime::now_utc();
        self.meta.save_index(&data)?;
        Ok(())
    }

    /// Rename a secret.
    /// # Errors
    /// `KeyNotFound`, `KeyAlreadyExists`, `InvalidKeyName`, or backend/index.
    /// # Panics
    /// On internal inconsistency (old key in index but not removable).
    pub fn rename(&self, old: &str, new: &str) -> Result<(), KlefError> {
        super::validate_name(new)?;
        let _lock = self.lock()?;
        let mut data = self.meta.load_index()?;
        if !data.keys.contains_key(old) {
            return Err(KlefError::KeyNotFound(old.to_string()));
        }
        if data.keys.contains_key(new) {
            return Err(KlefError::KeyAlreadyExists(new.to_string()));
        }
        // Atomicity (#48): set new → save → remove old; on save fail undo new.
        let value = self.backend.get(old)?;
        self.backend.set(new, &value)?;
        let mut meta = data.keys.remove(old).expect("checked above");
        meta.updated_at = OffsetDateTime::now_utc();
        data.keys.insert(new.to_string(), meta);
        if let Err(e) = self.meta.save_index(&data) {
            let _ = self.backend.remove(new);
            return Err(e);
        }
        let _ = self.backend.remove(old);
        Ok(())
    }

    /// Restore Phase 1: backend write only (no index touch).
    /// # Errors
    /// Backend write failure.
    pub fn restore_phase_1(&self, entry: &BundleEntry) -> Result<(), KlefError> {
        let _lock = self.lock()?;
        self.backend.set(&entry.name, &entry.value)
    }

    /// Restore Phase 2: rewrite index from bundle entries.
    /// # Errors
    /// `InvalidKeyName`, `InvalidEnvVar`, or an index save failure.
    pub fn restore_phase_2(&self, entries: &[BundleEntry]) -> Result<(), KlefError> {
        // Reject malformed names/env-vars before index write (#-injection).
        for entry in entries {
            super::validate_name(&entry.name)?;
            super::validate_env_var(&entry.env_var)?;
        }
        let _lock = self.lock()?;
        let mut data = self.meta.load_index()?;
        for entry in entries {
            data.keys.insert(
                entry.name.clone(),
                KeyMeta {
                    env_var: entry.env_var.clone(),
                    note: entry.note.clone(),
                    tags: entry.tags.clone(),
                    added_at: entry.added_at,
                    updated_at: entry.updated_at,
                    last_used_at: None,
                },
            );
        }
        self.meta.save_index(&data)
    }

    /// Return key names in index but missing from backend.
    /// # Errors
    /// Returns an error if the index fails to load.
    pub fn orphan_index_entries(&self) -> Result<Vec<String>, KlefError> {
        let data = self.meta.load_index()?;
        let mut orphans = Vec::new();
        for name in data.keys.keys() {
            if matches!(self.backend.get(name), Err(KlefError::KeyNotFound(_))) {
                orphans.push(name.clone());
            }
        }
        Ok(orphans)
    }

    /// Backend keys missing from the index. `Ok(None)` = backend can't
    /// enumerate (keychain) — closes #49.
    /// # Errors
    /// Enumeration or index-load failure.
    pub fn orphan_backend_entries(&self) -> Result<Option<Vec<String>>, KlefError> {
        let Some(backend_keys) = self.backend.list_names()? else {
            return Ok(None);
        };
        let data = self.meta.load_index()?;
        Ok(Some(
            backend_keys
                .into_iter()
                .filter(|n| !data.keys.contains_key(n))
                .collect(),
        ))
    }
}
