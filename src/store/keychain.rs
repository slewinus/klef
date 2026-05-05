use crate::error::KlefError;
use crate::store::backend::Backend;

pub struct KeychainBackend {
    service: String,
}

impl KeychainBackend {
    #[must_use]
    pub fn new() -> Self {
        Self {
            service: "klef".to_string(),
        }
    }
}

impl Default for KeychainBackend {
    fn default() -> Self {
        Self::new()
    }
}

fn map_err(e: keyring::Error) -> KlefError {
    use keyring::Error::{NoEntry, NoStorageAccess, PlatformFailure};
    match e {
        NoEntry => KlefError::KeyNotFound(String::new()),
        PlatformFailure(msg) | NoStorageAccess(msg) => {
            KlefError::BackendUnavailable(msg.to_string())
        }
        _ => KlefError::BackendUnavailable(e.to_string()),
    }
}

impl Backend for KeychainBackend {
    fn get(&self, name: &str) -> Result<String, KlefError> {
        let entry = keyring::Entry::new(&self.service, name).map_err(map_err)?;
        entry.get_password().map_err(|e| match e {
            keyring::Error::NoEntry => KlefError::KeyNotFound(name.to_string()),
            other => map_err(other),
        })
    }

    fn set(&self, name: &str, value: &str) -> Result<(), KlefError> {
        let entry = keyring::Entry::new(&self.service, name).map_err(map_err)?;
        entry.set_password(value).map_err(map_err)
    }

    fn remove(&self, name: &str) -> Result<(), KlefError> {
        let entry = keyring::Entry::new(&self.service, name).map_err(map_err)?;
        entry.delete_credential().map_err(|e| match e {
            keyring::Error::NoEntry => KlefError::KeyNotFound(name.to_string()),
            other => map_err(other),
        })
    }
}
