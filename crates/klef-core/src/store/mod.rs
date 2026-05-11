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

/// Validate that an env-var name is a POSIX shell-safe identifier
/// (`^[A-Za-z_][A-Za-z0-9_]*$`).
///
/// klef renders `export VAR=value` lines in `klef export` and the GUI may
/// import names from `.env` files; an unvalidated `VAR` can become a shell
/// injection vector if a downstream consumer pipes the output into `eval`.
/// Defense-in-depth: refuse to *store* names that would render unsafely
/// instead of relying on every consumer to escape them.
///
/// # Errors
/// Returns `InvalidEnvVar` if empty, starting with a digit, or containing
/// any character outside `[A-Za-z0-9_]`.
pub(crate) fn validate_env_var(var: &str) -> Result<(), KlefError> {
    let mut chars = var.chars();
    let first_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    let rest_ok = chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !(first_ok && rest_ok) {
        return Err(KlefError::InvalidEnvVar(var.to_string()));
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

    #[test]
    fn validate_env_var_accepts_posix_identifier() {
        assert!(validate_env_var("FOO").is_ok());
        assert!(validate_env_var("_HIDDEN").is_ok());
        assert!(validate_env_var("STRIPE_API_KEY").is_ok());
        assert!(validate_env_var("a1_b2").is_ok());
    }

    #[test]
    fn validate_env_var_rejects_shell_injection_payloads() {
        // Empty
        assert!(validate_env_var("").is_err());
        // Leading digit
        assert!(validate_env_var("1FOO").is_err());
        // Shell metachars — these are the actual exploits
        assert!(validate_env_var("FOO; rm -rf $HOME").is_err());
        assert!(validate_env_var("FOO`id`").is_err());
        assert!(validate_env_var("FOO$(id)").is_err());
        assert!(validate_env_var("FOO=bar # ").is_err());
        // Whitespace
        assert!(validate_env_var("FOO BAR").is_err());
        assert!(validate_env_var("FOO\nBAR").is_err());
        // Unicode is rejected (env vars are ASCII)
        assert!(validate_env_var("FOOé").is_err());
    }
    // Other store tests (add, remove, rename, orphan, tags) live in
    // tests/store_remove.rs (file-cap discipline).
}
