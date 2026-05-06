use crate::error::KlefError;

pub trait Backend: Send + Sync {
    /// Human-readable backend identifier for diagnostics (e.g. `status`).
    /// Examples: `"keychain"`, `"age:/path/to/vault.age"`, `"file:/tmp/x.json"`,
    /// `"memory"`.
    fn describe(&self) -> String;

    /// Retrieve a secret by name.
    ///
    /// # Errors
    ///
    /// Returns `KeyNotFound` if the key does not exist.
    fn get(&self, name: &str) -> Result<String, KlefError>;

    /// Store a secret by name.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend fails to store the secret.
    fn set(&self, name: &str, value: &str) -> Result<(), KlefError>;

    /// Remove a secret by name.
    ///
    /// # Errors
    ///
    /// Returns `KeyNotFound` if the key does not exist.
    fn remove(&self, name: &str) -> Result<(), KlefError>;

    /// Enumerate every key the backend currently stores, if the backend
    /// supports enumeration. Returns `None` for backends that cannot list
    /// (e.g. the macOS / Linux Secret Service via the `keyring` crate).
    ///
    /// Used by `status` to detect the reverse desync direction:
    /// secrets that exist in the backend but are missing from the index
    /// (the failure mode of a partial `add` whose index write fails).
    ///
    /// # Errors
    ///
    /// Returns an error if enumeration is supported but fails (e.g. corrupt
    /// vault file).
    fn list_names(&self) -> Result<Option<Vec<String>>, KlefError> {
        Ok(None)
    }
}

#[derive(Default)]
pub struct MemoryBackend {
    inner: std::sync::Mutex<std::collections::HashMap<String, String>>,
}

impl MemoryBackend {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl Backend for MemoryBackend {
    fn describe(&self) -> String {
        "memory".to_string()
    }

    fn get(&self, name: &str) -> Result<String, KlefError> {
        self.inner
            .lock()
            .unwrap()
            .get(name)
            .cloned()
            .ok_or_else(|| KlefError::KeyNotFound(name.to_string()))
    }

    fn set(&self, name: &str, value: &str) -> Result<(), KlefError> {
        self.inner
            .lock()
            .unwrap()
            .insert(name.to_string(), value.to_string());
        Ok(())
    }

    fn remove(&self, name: &str) -> Result<(), KlefError> {
        {
            let mut g = self.inner.lock().unwrap();
            g.remove(name)
                .ok_or_else(|| KlefError::KeyNotFound(name.to_string()))?;
        }
        Ok(())
    }

    fn list_names(&self) -> Result<Option<Vec<String>>, KlefError> {
        Ok(Some(self.inner.lock().unwrap().keys().cloned().collect()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_then_get_returns_value() {
        let b = MemoryBackend::new();
        b.set("stripe", "sk_live_xyz").unwrap();
        assert_eq!(b.get("stripe").unwrap(), "sk_live_xyz");
    }

    #[test]
    fn get_missing_returns_key_not_found() {
        let b = MemoryBackend::new();
        assert!(matches!(b.get("nope"), Err(KlefError::KeyNotFound(_))));
    }

    #[test]
    fn remove_then_get_returns_not_found() {
        let b = MemoryBackend::new();
        b.set("k", "v").unwrap();
        b.remove("k").unwrap();
        assert!(matches!(b.get("k"), Err(KlefError::KeyNotFound(_))));
    }

    #[test]
    fn remove_missing_returns_not_found() {
        let b = MemoryBackend::new();
        assert!(matches!(b.remove("nope"), Err(KlefError::KeyNotFound(_))));
    }
}
