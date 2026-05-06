//! Age-encrypted file backend for headless / CI / Docker environments.
//!
//! The entire vault lives in a single age-encrypted file. Every `get`/`set`/`remove`
//! decrypts → mutates → re-encrypts atomically (tmp + rename).
//!
//! Passphrase is sourced from `KLEF_PASSPHRASE` env var (CI) or from a masked
//! TTY prompt, and cached for the lifetime of the process.

use crate::error::KlefError;
use crate::store::backend::Backend;
use age::secrecy::SecretString;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{IsTerminal, Read, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use zeroize::Zeroizing;

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgeData {
    secrets: BTreeMap<String, String>,
}

/// Age-encrypted file backend.
///
/// File doesn't exist on first `set`? Created with the user's passphrase
/// (prompted twice for confirmation). Passphrase cached for the process lifetime.
pub struct AgeBackend {
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
            path,
            state: Mutex::new(State::default()),
        }
    }

    /// Return the cached passphrase, or prompt / read from env.
    /// `confirm = true` asks for a second confirmation prompt (new vault creation).
    fn passphrase(&self, confirm: bool) -> Result<SecretString, KlefError> {
        // Check cache first, then drop the lock before any blocking I/O.
        {
            let state = self.state.lock().unwrap();
            if let Some(p) = &state.passphrase {
                return Ok(p.clone());
            }
        }

        let pass = if let Ok(env_pass) = std::env::var("KLEF_PASSPHRASE") {
            SecretString::from(env_pass)
        } else if std::io::stdin().is_terminal() {
            let prompt = format!("Passphrase for {}: ", self.path.display());
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

        self.state.lock().unwrap().passphrase = Some(pass.clone());
        Ok(pass)
    }

    fn load(&self) -> Result<AgeData, KlefError> {
        if !self.path.exists() {
            return Ok(AgeData::default());
        }
        let ciphertext = std::fs::read(&self.path).map_err(KlefError::Io)?;
        // File exists → no confirmation needed; we're reading an existing vault.
        let pass = self.passphrase(false)?;
        let plaintext = age_decrypt(&ciphertext, &pass)?;
        serde_json::from_slice(&plaintext).map_err(|e| KlefError::IndexCorrupt {
            path: self.path.clone(),
            reason: format!("age vault content not valid JSON: {e}"),
        })
    }

    fn save(&self, data: &AgeData) -> Result<(), KlefError> {
        let plaintext: Zeroizing<Vec<u8>> = Zeroizing::new(serde_json::to_vec(data).map_err(
            |e| KlefError::IndexCorrupt {
                path: self.path.clone(),
                reason: format!("failed to serialize age vault: {e}"),
            },
        )?);

        // First save (file doesn't exist yet) → confirm passphrase.
        let confirm_needed = !self.path.exists();
        let pass = self.passphrase(confirm_needed)?;
        let ciphertext = age_encrypt(&plaintext, &pass)?;

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(KlefError::Io)?;
        }
        let tmp = self.path.with_extension("age.tmp");
        std::fs::write(&tmp, &ciphertext).map_err(KlefError::IndexWrite)?;
        std::fs::rename(&tmp, &self.path).map_err(KlefError::IndexWrite)?;
        Ok(())
    }
}

impl Backend for AgeBackend {
    fn get(&self, name: &str) -> Result<String, KlefError> {
        let data = self.load()?;
        data.secrets
            .get(name)
            .cloned()
            .ok_or_else(|| KlefError::KeyNotFound(name.to_string()))
    }

    fn set(&self, name: &str, value: &str) -> Result<(), KlefError> {
        let mut data = self.load()?;
        data.secrets.insert(name.to_string(), value.to_string());
        self.save(&data)
    }

    fn remove(&self, name: &str) -> Result<(), KlefError> {
        let mut data = self.load()?;
        data.secrets
            .remove(name)
            .ok_or_else(|| KlefError::KeyNotFound(name.to_string()))?;
        self.save(&data)
    }
}

fn age_encrypt(plaintext: &[u8], pass: &SecretString) -> Result<Vec<u8>, KlefError> {
    let encryptor = age::Encryptor::with_user_passphrase(pass.clone());
    let mut out = Vec::new();
    let mut writer = encryptor
        .wrap_output(&mut out)
        .map_err(|e| KlefError::BackendUnavailable(format!("age encrypt init: {e}")))?;
    writer.write_all(plaintext).map_err(KlefError::Io)?;
    writer
        .finish()
        .map_err(|e| KlefError::BackendUnavailable(format!("age encrypt finish: {e}")))?;
    Ok(out)
}

fn age_decrypt(ciphertext: &[u8], pass: &SecretString) -> Result<Zeroizing<Vec<u8>>, KlefError> {
    let identity = age::scrypt::Identity::new(pass.clone());
    let decryptor = age::Decryptor::new(ciphertext)
        .map_err(|e| KlefError::BackendUnavailable(format!("age decrypt init: {e}")))?;
    let mut output = Zeroizing::new(Vec::new());
    let mut reader = decryptor
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .map_err(|e| KlefError::BackendUnavailable(format!("age decrypt: {e}")))?;
    reader.read_to_end(&mut output).map_err(KlefError::Io)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn set_passphrase(b: &AgeBackend, pass: &str) {
        b.state.lock().unwrap().passphrase = Some(SecretString::from(pass.to_string()));
    }

    #[test]
    fn round_trip_with_passphrase() {
        let d = tempdir().unwrap();
        let p = d.path().join("v.age");
        let b = AgeBackend::new(p.clone());
        set_passphrase(&b, "secret");

        b.set("k", "v").unwrap();
        assert_eq!(b.get("k").unwrap(), "v");

        // Reopen with same passphrase — same value.
        let b2 = AgeBackend::new(p);
        set_passphrase(&b2, "secret");
        assert_eq!(b2.get("k").unwrap(), "v");
    }

    #[test]
    fn wrong_passphrase_returns_error() {
        let d = tempdir().unwrap();
        let p = d.path().join("v.age");
        let b = AgeBackend::new(p.clone());
        set_passphrase(&b, "right");
        b.set("k", "v").unwrap();

        let b2 = AgeBackend::new(p);
        set_passphrase(&b2, "wrong");
        let result = b2.get("k");
        assert!(matches!(result, Err(KlefError::BackendUnavailable(_))));
    }

    #[test]
    fn missing_key_is_keynotfound() {
        let d = tempdir().unwrap();
        let p = d.path().join("v.age");
        let b = AgeBackend::new(p);
        set_passphrase(&b, "x");
        b.set("a", "1").unwrap();
        assert!(matches!(b.get("nope"), Err(KlefError::KeyNotFound(_))));
    }

    #[test]
    fn remove_then_get_fails() {
        let d = tempdir().unwrap();
        let p = d.path().join("v.age");
        let b = AgeBackend::new(p);
        set_passphrase(&b, "x");
        b.set("k", "v").unwrap();
        b.remove("k").unwrap();
        assert!(matches!(b.get("k"), Err(KlefError::KeyNotFound(_))));
    }

    #[test]
    fn ciphertext_does_not_contain_value() {
        let d = tempdir().unwrap();
        let p = d.path().join("v.age");
        let b = AgeBackend::new(p.clone());
        set_passphrase(&b, "x");
        b.set("api-key", "sk_live_super_secret_value_xyz").unwrap();
        let bytes = std::fs::read(&p).unwrap();
        assert!(
            !bytes.windows(15).any(|w| w == b"sk_live_super_s"),
            "ciphertext leaked plaintext value"
        );
    }
}
