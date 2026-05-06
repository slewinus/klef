//! Age-encrypted file backend for headless / CI / Docker environments.
//!
//! The entire vault lives in a single age-encrypted file. Every `get`/`set`/`remove`
//! decrypts → mutates → re-encrypts atomically (tmp + rename).
//!
//! Passphrase is sourced from `KLEF_PASSPHRASE` env var (CI) or from a masked
//! TTY prompt, and cached for the lifetime of the process.
//!
//! As of v0.4.1 the vault also stores `IndexData` internally, so metadata
//! (`env_var`, note, tags, timestamps) never touches the global plaintext index.

use crate::error::KlefError;
use crate::store::MetaStore;
use crate::store::backend::Backend;
use crate::store::index::{IndexData, KeyMeta};
use age::secrecy::SecretString;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use time::OffsetDateTime;
use zeroize::Zeroizing;

use super::age_crypto::{age_decrypt, age_encrypt};

// ---------------------------------------------------------------------------
// On-disk format
// ---------------------------------------------------------------------------

/// The full contents of an age vault file (v1).
///
/// `version` is written on every save and is defaulted to 1 on deserialization
/// so that legacy v0.4 vaults (which had only `{"secrets":{...}}`) load cleanly.
/// The missing `index` field is filled with a synthesized default during
/// [`AgeBackend::load_vault`].
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgeVault {
    #[serde(default = "vault_version")]
    version: u32,
    secrets: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "is_default_index")]
    index: IndexData,
}

const fn vault_version() -> u32 {
    1
}

fn is_default_index(d: &IndexData) -> bool {
    d.version == 1 && d.keys.is_empty()
}

impl Default for AgeVault {
    fn default() -> Self {
        Self {
            version: 1,
            secrets: BTreeMap::new(),
            index: IndexData::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Backend struct
// ---------------------------------------------------------------------------

/// Age-encrypted file backend.
///
/// File doesn't exist on first `set`? Created with the user's passphrase
/// (prompted twice for confirmation). Passphrase cached for the process lifetime.
///
/// The backend is cheaply `Clone`-able: all clones share the same underlying
/// `Arc<AgeBackendInner>`, so passphrase and file path are shared.
/// This lets the same instance be handed to both `Box<dyn Backend>` and
/// `Box<dyn MetaStore>` in [`crate::lib::build_store`].
#[derive(Clone)]
pub struct AgeBackend {
    inner: Arc<AgeBackendInner>,
}

struct AgeBackendInner {
    path: PathBuf,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    passphrase: Option<SecretString>,
}

impl AgeBackend {
    /// Build a new age backend pointing at the given file.
    /// The file need not exist yet — it will be created on the first `set`.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self {
            inner: Arc::new(AgeBackendInner {
                path,
                state: Mutex::new(State::default()),
            }),
        }
    }

    /// Return the cached passphrase, or prompt / read from env.
    /// `confirm = true` asks for a second confirmation prompt (new vault creation).
    fn passphrase(&self, confirm: bool) -> Result<SecretString, KlefError> {
        // Check cache first, then drop the lock before any blocking I/O.
        {
            let state = self.inner.state.lock().unwrap();
            if let Some(p) = &state.passphrase {
                return Ok(p.clone());
            }
        }

        let pass = if let Ok(env_pass) = std::env::var("KLEF_PASSPHRASE") {
            SecretString::from(env_pass)
        } else if std::io::stdin().is_terminal() {
            let prompt = format!("Passphrase for {}: ", self.inner.path.display());
            let p = rpassword::prompt_password(&prompt)
                .map_err(|e| KlefError::BackendUnavailable(e.to_string()))?;
            if confirm {
                let p2 = rpassword::prompt_password("Confirm passphrase: ")
                    .map_err(|e| KlefError::BackendUnavailable(e.to_string()))?;
                if p != p2 {
                    return Err(KlefError::BackendUnavailable(
                        "passphrases do not match".to_string(),
                    ));
                }
            }
            SecretString::from(p)
        } else {
            return Err(KlefError::BackendUnavailable(
                "age backend requires a passphrase: set KLEF_PASSPHRASE or run in a TTY"
                    .to_string(),
            ));
        };

        self.inner.state.lock().unwrap().passphrase = Some(pass.clone());
        Ok(pass)
    }

    /// Load the vault from disk. If the file does not exist, returns an empty vault.
    /// Legacy vaults (no `index` field) are transparently upgraded: missing metadata
    /// entries are synthesized from the secret key names so callers always see a
    /// consistent view.
    fn load_vault(&self) -> Result<AgeVault, KlefError> {
        if !self.inner.path.exists() {
            return Ok(AgeVault::default());
        }
        let ciphertext = std::fs::read(&self.inner.path).map_err(KlefError::Io)?;
        // File exists → no confirmation needed; we're reading an existing vault.
        let pass = self.passphrase(false)?;
        let plaintext = age_decrypt(&ciphertext, &pass)?;
        let mut vault: AgeVault =
            serde_json::from_slice(&plaintext).map_err(|e| KlefError::IndexCorrupt {
                path: self.inner.path.clone(),
                reason: format!("age vault content not valid JSON: {e}"),
            })?;

        // Auto-backfill missing metadata for keys present in secrets but not in
        // index.  Happens transparently on legacy v0.4 vaults that had no embedded
        // index field.  The synthesized entries are written back on the next save,
        // so the migration is one-shot.
        let now = OffsetDateTime::now_utc();
        let secret_names: Vec<String> = vault.secrets.keys().cloned().collect();
        for name in secret_names {
            if !vault.index.keys.contains_key(&name) {
                vault.index.keys.insert(
                    name.clone(),
                    KeyMeta {
                        env_var: default_env_var(&name),
                        note: None,
                        tags: vec![],
                        added_at: now,
                        updated_at: now,
                        last_used_at: None,
                    },
                );
            }
        }

        Ok(vault)
    }

    /// Serialize the vault and atomically write it to disk (tmp + rename).
    fn save_vault(&self, vault: &AgeVault) -> Result<(), KlefError> {
        let plaintext: Zeroizing<Vec<u8>> = Zeroizing::new(serde_json::to_vec(vault).map_err(
            |e| KlefError::IndexCorrupt {
                path: self.inner.path.clone(),
                reason: format!("failed to serialize age vault: {e}"),
            },
        )?);

        // First save (file doesn't exist yet) → confirm passphrase.
        let confirm_needed = !self.inner.path.exists();
        let pass = self.passphrase(confirm_needed)?;
        let ciphertext = age_encrypt(&plaintext, &pass)?;

        if let Some(parent) = self.inner.path.parent() {
            std::fs::create_dir_all(parent).map_err(KlefError::Io)?;
        }
        let tmp = self.inner.path.with_extension("age.tmp");
        std::fs::write(&tmp, &ciphertext).map_err(KlefError::IndexWrite)?;
        std::fs::rename(&tmp, &self.inner.path).map_err(KlefError::IndexWrite)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Trait impls
// ---------------------------------------------------------------------------

impl Backend for AgeBackend {
    fn describe(&self) -> String {
        format!("age:{}", self.inner.path.display())
    }

    fn get(&self, name: &str) -> Result<String, KlefError> {
        let vault = self.load_vault()?;
        vault
            .secrets
            .get(name)
            .cloned()
            .ok_or_else(|| KlefError::KeyNotFound(name.to_string()))
    }

    fn set(&self, name: &str, value: &str) -> Result<(), KlefError> {
        let mut vault = self.load_vault()?;
        vault.secrets.insert(name.to_string(), value.to_string());
        self.save_vault(&vault)
    }

    fn remove(&self, name: &str) -> Result<(), KlefError> {
        let mut vault = self.load_vault()?;
        vault
            .secrets
            .remove(name)
            .ok_or_else(|| KlefError::KeyNotFound(name.to_string()))?;
        self.save_vault(&vault)
    }

    fn list_names(&self) -> Result<Option<Vec<String>>, KlefError> {
        // Empty vault file is treated as "nothing yet" — don't prompt for a
        // passphrase just to enumerate zero entries.
        if !self.inner.path.exists() {
            return Ok(Some(Vec::new()));
        }
        let vault = self.load_vault()?;
        Ok(Some(vault.secrets.keys().cloned().collect()))
    }
}

impl MetaStore for AgeBackend {
    fn load_index(&self) -> Result<IndexData, KlefError> {
        Ok(self.load_vault()?.index)
    }

    fn save_index(&self, data: &IndexData) -> Result<(), KlefError> {
        let mut vault = self.load_vault()?;
        vault.index = data.clone();
        self.save_vault(&vault)
    }

    fn lock_path(&self) -> PathBuf {
        self.inner.path.clone()
    }
}

// ---------------------------------------------------------------------------
// Helpers (mirror of store::mod private fn)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
// Tests are in a separate file for file-cap discipline (included here so they
// can still access private types like `State` / `AgeBackendInner`).

#[cfg(test)]
mod tests {
    use super::*;
    include!("age_backend_tests.rs");
}
