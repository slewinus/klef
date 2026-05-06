pub mod backend;
pub mod file;
pub mod index;
pub mod keychain;

pub use backend::{Backend, MemoryBackend};
pub use file::FileBackend;
pub use index::{IndexData, IndexFile, KeyMeta};
pub use keychain::KeychainBackend;

use crate::commands::backup::BundleEntry;
use crate::error::KlefError;
use std::path::PathBuf;
use time::OffsetDateTime;

/// Coordinates access to secret values (via `Backend`) and metadata (via `IndexFile`).
pub struct Store {
    backend: Box<dyn Backend>,
    index: IndexFile,
}

impl Store {
    /// Create a new Store backed by the given backend and index file path.
    #[must_use]
    pub fn new(backend: Box<dyn Backend>, index_path: PathBuf) -> Self {
        Self {
            backend,
            index: IndexFile::new(index_path),
        }
    }

    /// Add or update a secret by name, optionally with env-var, note, and tags metadata.
    /// # Errors
    /// Returns `InvalidKeyName`, `KeyAlreadyExists`, or a backend/index error.
    pub fn add(
        &self,
        name: &str,
        value: &str,
        env_var: Option<String>,
        note: Option<String>,
        tags: Vec<String>,
        force: bool,
    ) -> Result<(), KlefError> {
        validate_name(name)?;
        let mut data = self.index.load()?;
        if data.keys.contains_key(name) && !force {
            return Err(KlefError::KeyAlreadyExists(name.to_string()));
        }
        let now = OffsetDateTime::now_utc();
        let mut sorted_tags = tags;
        sorted_tags.sort();
        sorted_tags.dedup();
        let meta = KeyMeta {
            env_var: env_var.unwrap_or_else(|| default_env_var(name)),
            note,
            tags: sorted_tags,
            added_at: data.keys.get(name).map_or(now, |k| k.added_at),
            updated_at: now,
        };
        self.backend.set(name, value)?;
        data.keys.insert(name.to_string(), meta);
        self.index.save(&data)?;
        Ok(())
    }

    /// Replace the tag set on an existing key.
    ///
    /// # Errors
    /// `KeyNotFound` if name doesn't exist; index errors propagated.
    pub fn set_tags(&self, name: &str, tags: Vec<String>) -> Result<(), KlefError> {
        let mut data = self.index.load()?;
        let meta = data
            .keys
            .get_mut(name)
            .ok_or_else(|| KlefError::KeyNotFound(name.to_string()))?;
        let mut sorted = tags;
        sorted.sort();
        sorted.dedup();
        meta.tags = sorted;
        meta.updated_at = OffsetDateTime::now_utc();
        self.index.save(&data)?;
        Ok(())
    }

    /// Return a map of tag → number of keys carrying it.
    ///
    /// # Errors
    /// Index load error.
    pub fn tags_with_counts(&self) -> Result<std::collections::BTreeMap<String, usize>, KlefError> {
        let data = self.index.load()?;
        let mut counts = std::collections::BTreeMap::new();
        for meta in data.keys.values() {
            for t in &meta.tags {
                *counts.entry(t.clone()).or_insert(0) += 1;
            }
        }
        Ok(counts)
    }

    /// Retrieve the secret value by name.
    /// # Errors
    /// Returns `KeyNotFound` if the key does not exist, or a backend error.
    pub fn get_value(&self, name: &str) -> Result<String, KlefError> {
        let data = self.index.load()?;
        if !data.keys.contains_key(name) {
            return Err(KlefError::KeyNotFound(name.to_string()));
        }
        self.backend.get(name)
    }

    /// List all stored keys and their metadata.
    /// # Errors
    /// Returns an error if the index fails to load.
    pub fn list(&self) -> Result<Vec<(String, KeyMeta)>, KlefError> {
        let data = self.index.load()?;
        Ok(data.keys.into_iter().collect())
    }

    /// Remove a secret and its metadata.
    /// # Errors
    /// Returns `KeyNotFound` if the key does not exist, or an index error.
    pub fn remove(&self, name: &str) -> Result<(), KlefError> {
        let mut data = self.index.load()?;
        if !data.keys.contains_key(name) {
            return Err(KlefError::KeyNotFound(name.to_string()));
        }
        // Tolerate `KeyNotFound` (the secret may already be gone — manual
        // deletion, concurrent rm, etc.) but propagate any other backend error
        // so callers don't believe the secret is gone when it isn't.
        match self.backend.remove(name) {
            Ok(()) | Err(KlefError::KeyNotFound(_)) => {}
            Err(e) => return Err(e),
        }
        data.keys.remove(name);
        self.index.save(&data)?;
        Ok(())
    }

    /// Retrieve metadata for a specific key.
    /// # Errors
    /// Returns `KeyNotFound` if the key does not exist, or an index error.
    pub fn meta(&self, name: &str) -> Result<KeyMeta, KlefError> {
        let data = self.index.load()?;
        data.keys
            .get(name)
            .cloned()
            .ok_or_else(|| KlefError::KeyNotFound(name.to_string()))
    }

    /// Update the env-var and/or note for a key.
    /// `None` fields are unchanged; `Some(None)` clears the note.
    /// # Errors
    /// Returns `KeyNotFound` if the key does not exist, or an index error.
    pub fn update_meta(
        &self,
        name: &str,
        env_var: Option<String>,
        note: Option<Option<String>>,
    ) -> Result<(), KlefError> {
        let mut data = self.index.load()?;
        let meta = data
            .keys
            .get_mut(name)
            .ok_or_else(|| KlefError::KeyNotFound(name.to_string()))?;
        if let Some(v) = env_var {
            meta.env_var = v;
        }
        if let Some(n) = note {
            meta.note = n;
        }
        meta.updated_at = OffsetDateTime::now_utc();
        self.index.save(&data)?;
        Ok(())
    }

    /// Rename a secret.
    /// # Errors
    /// Returns `KeyNotFound`, `KeyAlreadyExists`, `InvalidKeyName`, or a backend/index error.
    /// # Panics
    /// Panics on internal inconsistency (old key in index but not removable).
    pub fn rename(&self, old: &str, new: &str) -> Result<(), KlefError> {
        validate_name(new)?;
        let mut data = self.index.load()?;
        if !data.keys.contains_key(old) {
            return Err(KlefError::KeyNotFound(old.to_string()));
        }
        if data.keys.contains_key(new) {
            return Err(KlefError::KeyAlreadyExists(new.to_string()));
        }
        let value = self.backend.get(old)?;
        self.backend.set(new, &value)?;
        let _ = self.backend.remove(old);
        let mut meta = data.keys.remove(old).expect("checked above");
        meta.updated_at = OffsetDateTime::now_utc();
        data.keys.insert(new.to_string(), meta);
        self.index.save(&data)?;
        Ok(())
    }
    /// Restore Phase 1: write entry value to the backend only (no index write).
    /// # Errors
    /// Returns an error if the backend write fails.
    pub fn restore_phase_1(&self, entry: &BundleEntry) -> Result<(), KlefError> {
        self.backend.set(&entry.name, &entry.value)
    }

    /// Restore Phase 2: rewrite the index from a list of bundle entries.
    /// # Errors
    /// Returns an error if the index save fails.
    pub fn restore_phase_2(&self, entries: &[BundleEntry]) -> Result<(), KlefError> {
        let mut data = self.index.load()?;
        for entry in entries {
            data.keys.insert(
                entry.name.clone(),
                KeyMeta {
                    env_var: entry.env_var.clone(),
                    note: entry.note.clone(),
                    tags: entry.tags.clone(),
                    added_at: entry.added_at,
                    updated_at: entry.updated_at,
                },
            );
        }
        self.index.save(&data)
    }

    /// Return key names in index but missing from backend.
    /// # Errors
    /// Returns an error if the index fails to load.
    pub fn orphan_index_entries(&self) -> Result<Vec<String>, KlefError> {
        let data = self.index.load()?;
        let mut orphans = Vec::new();
        for name in data.keys.keys() {
            if matches!(self.backend.get(name), Err(KlefError::KeyNotFound(_))) {
                orphans.push(name.clone());
            }
        }
        Ok(orphans)
    }
}

/// Generate a default env-var name from a key name.
fn default_env_var(name: &str) -> String {
    let upper: String = name
        .chars()
        .map(|c| {
            if c == '-' {
                '_'
            } else {
                c.to_ascii_uppercase()
            }
        })
        .collect();
    format!("{upper}_API_KEY")
}

/// Validate that a key name is alphanumeric with dashes or underscores.
/// # Errors
/// Returns `InvalidKeyName` if the name is empty or contains invalid characters.
fn validate_name(name: &str) -> Result<(), KlefError> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(KlefError::InvalidKeyName(name.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_env_var_uses_api_key_suffix() {
        assert_eq!(default_env_var("stripe"), "STRIPE_API_KEY");
        assert_eq!(default_env_var("stripe-prod"), "STRIPE_PROD_API_KEY");
    }
    // Other store tests (add, remove, rename, orphan, tags) live in
    // tests/store_remove.rs (file-cap discipline).
}
