pub mod backend;
pub mod file;
pub mod index;
pub mod keychain;

pub use backend::{Backend, MemoryBackend};
pub use file::FileBackend;
pub use index::{IndexData, IndexFile, KeyMeta};
pub use keychain::KeychainBackend;

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

    /// Add or update a secret by name, optionally with environment variable and note metadata.
    ///
    /// # Errors
    ///
    /// Returns `InvalidKeyName` if the name contains invalid characters.
    /// Returns `KeyAlreadyExists` if the key exists and force is false.
    /// Returns an error if the backend or index fails.
    pub fn add(
        &self,
        name: &str,
        value: &str,
        env_var: Option<String>,
        note: Option<String>,
        force: bool,
    ) -> Result<(), KlefError> {
        validate_name(name)?;
        let mut data = self.index.load()?;
        if data.keys.contains_key(name) && !force {
            return Err(KlefError::KeyAlreadyExists(name.to_string()));
        }
        let now = OffsetDateTime::now_utc();
        let meta = KeyMeta {
            env_var: env_var.unwrap_or_else(|| default_env_var(name)),
            note,
            added_at: data.keys.get(name).map_or(now, |k| k.added_at),
            updated_at: now,
        };
        self.backend.set(name, value)?;
        data.keys.insert(name.to_string(), meta);
        self.index.save(&data)?;
        Ok(())
    }

    /// Retrieve the secret value by name.
    ///
    /// # Errors
    ///
    /// Returns `KeyNotFound` if the key does not exist.
    /// Returns an error if the index or backend fails.
    pub fn get_value(&self, name: &str) -> Result<String, KlefError> {
        let data = self.index.load()?;
        if !data.keys.contains_key(name) {
            return Err(KlefError::KeyNotFound(name.to_string()));
        }
        self.backend.get(name)
    }

    /// List all stored keys and their metadata.
    ///
    /// # Errors
    ///
    /// Returns an error if the index fails to load.
    pub fn list(&self) -> Result<Vec<(String, KeyMeta)>, KlefError> {
        let data = self.index.load()?;
        Ok(data.keys.into_iter().collect())
    }

    /// Remove a secret and its metadata.
    ///
    /// # Errors
    ///
    /// Returns `KeyNotFound` if the key does not exist.
    /// Returns an error if the index fails.
    pub fn remove(&self, name: &str) -> Result<(), KlefError> {
        let mut data = self.index.load()?;
        if !data.keys.contains_key(name) {
            return Err(KlefError::KeyNotFound(name.to_string()));
        }
        // Backend delete is best-effort (key may already be gone manually).
        let _ = self.backend.remove(name);
        data.keys.remove(name);
        self.index.save(&data)?;
        Ok(())
    }

    /// Retrieve metadata for a specific key.
    ///
    /// # Errors
    ///
    /// Returns `KeyNotFound` if the key does not exist.
    /// Returns an error if the index fails to load.
    pub fn meta(&self, name: &str) -> Result<KeyMeta, KlefError> {
        let data = self.index.load()?;
        data.keys
            .get(name)
            .cloned()
            .ok_or_else(|| KlefError::KeyNotFound(name.to_string()))
    }

    /// Update the environment variable and/or note for a key.
    ///
    /// Pass `None` for fields that should not change.
    /// For note, `Some(None)` clears the note, `Some(Some(s))` sets it to s.
    ///
    /// # Errors
    ///
    /// Returns `KeyNotFound` if the key does not exist.
    /// Returns an error if the index fails.
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
    ///
    /// # Errors
    ///
    /// Returns `KeyNotFound` if the old key does not exist.
    /// Returns `KeyAlreadyExists` if the new key already exists.
    /// Returns `InvalidKeyName` if the new name is invalid.
    /// Returns an error if the backend or index fails.
    ///
    /// # Panics
    ///
    /// Panics if the old key exists in the index but cannot be removed (internal inconsistency).
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
}

/// Generate a default environment variable name from a key name.
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

/// Validate that a key name contains only alphanumeric, dash, or underscore characters.
///
/// # Errors
///
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
    fn add_then_get_round_trip() {
        let (s, _d) = make_store();
        s.add("stripe", "sk_live", None, None, false).unwrap();
        assert_eq!(s.get_value("stripe").unwrap(), "sk_live");
    }

    #[test]
    fn add_existing_without_force_fails() {
        let (s, _d) = make_store();
        s.add("stripe", "v1", None, None, false).unwrap();
        let r = s.add("stripe", "v2", None, None, false);
        assert!(matches!(r, Err(KlefError::KeyAlreadyExists(_))));
    }

    #[test]
    fn add_existing_with_force_overwrites() {
        let (s, _d) = make_store();
        s.add("stripe", "v1", None, None, false).unwrap();
        s.add("stripe", "v2", None, None, true).unwrap();
        assert_eq!(s.get_value("stripe").unwrap(), "v2");
    }

    #[test]
    fn default_env_var_uses_api_key_suffix() {
        assert_eq!(default_env_var("stripe"), "STRIPE_API_KEY");
        assert_eq!(default_env_var("stripe-prod"), "STRIPE_PROD_API_KEY");
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
    fn invalid_name_rejected() {
        let (s, _d) = make_store();
        let r = s.add("has space", "v", None, None, false);
        assert!(matches!(r, Err(KlefError::InvalidKeyName(_))));
    }

    #[test]
    fn remove_clears_both_layers() {
        let (s, _d) = make_store();
        s.add("k", "v", None, None, false).unwrap();
        s.remove("k").unwrap();
        assert!(matches!(s.get_value("k"), Err(KlefError::KeyNotFound(_))));
        assert!(s.list().unwrap().is_empty());
    }
}
