pub mod age_backend;
mod age_crypto;
pub mod backend;
pub mod file;
pub mod index;
pub mod keychain;
pub(crate) mod lock;
mod ops;

pub use age_backend::AgeBackend;
pub use backend::{Backend, MemoryBackend};
pub use file::FileBackend;
pub use index::{IndexData, IndexFile, KeyMeta};
pub use keychain::KeychainBackend;
pub use ops::Store;

use crate::error::KlefError;

// ---------------------------------------------------------------------------
// MetaStore trait
// ---------------------------------------------------------------------------

/// Abstraction over wherever metadata (`env_var`, note, tags, timestamps) is stored.
///
/// - For the keychain and file backends, this is the plaintext `IndexFile`.
/// - For the age backend, this is the encrypted vault itself — no plaintext ever
///   touches the global index.
pub trait MetaStore: Send + Sync {
    /// Load the full index from the backing store.
    ///
    /// # Errors
    /// Returns an index error if the store cannot be read or is corrupt.
    fn load_index(&self) -> Result<IndexData, KlefError>;

    /// Persist the full index to the backing store.
    ///
    /// # Errors
    /// Returns an index error if the store cannot be written.
    fn save_index(&self, data: &IndexData) -> Result<(), KlefError>;

    /// Path of the resource the inter-process lock should protect.
    ///
    /// Used by [`Store`] to acquire an exclusive flock around any
    /// load → mutate → save sequence (closes #61). The lock file itself
    /// is the sibling `<lock_path>.lock` — see `crate::store::lock`.
    fn lock_path(&self) -> std::path::PathBuf;
}

/// Implement `MetaStore` for `IndexFile` so the keychain / file backends can
/// use it without any changes to call sites.
impl MetaStore for IndexFile {
    fn load_index(&self) -> Result<IndexData, KlefError> {
        self.load()
    }

    fn save_index(&self, data: &IndexData) -> Result<(), KlefError> {
        self.save(data)
    }

    fn lock_path(&self) -> std::path::PathBuf {
        self.path().to_path_buf()
    }
}

// ---------------------------------------------------------------------------
// Helpers used by store.rs (pub(crate) so store submodule can access them)
// ---------------------------------------------------------------------------

/// Generate a default env-var name from a key name.
pub(crate) fn default_env_var(name: &str) -> String {
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
pub(crate) fn validate_name(name: &str) -> Result<(), KlefError> {
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
