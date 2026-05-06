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

fn format_unavailable_msg(inner: &str) -> String {
    #[cfg(target_os = "linux")]
    {
        format!(
            "{inner}\n\n\
             hint: klef needs a running Secret Service implementation on Linux.\n\
             - desktop session: install gnome-keyring or KWallet and ensure the daemon is running\n\
             - server / CI / Docker: use the age backend instead — `klef --backend age:/path/to/vault.age ...`\n\
               (passphrase via KLEF_PASSPHRASE for non-interactive use)"
        )
    }
    #[cfg(target_os = "macos")]
    {
        format!(
            "{inner}\n\n\
             hint: this typically means the macOS Keychain is locked or not accessible.\n\
             try opening Keychain Access.app and unlocking your login keychain."
        )
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        inner.to_string()
    }
}

fn map_err(e: keyring::Error) -> KlefError {
    use keyring::Error::{NoEntry, NoStorageAccess, PlatformFailure};
    match e {
        NoEntry => KlefError::KeyNotFound(String::new()),
        PlatformFailure(msg) | NoStorageAccess(msg) => {
            KlefError::BackendUnavailable(format_unavailable_msg(&msg.to_string()))
        }
        other => KlefError::BackendUnavailable(format_unavailable_msg(&other.to_string())),
    }
}

impl Backend for KeychainBackend {
    fn describe(&self) -> String {
        "keychain".to_string()
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_unavailable_msg_preserves_inner() {
        let msg = format_unavailable_msg("dbus error: Connection refused");
        assert!(msg.contains("dbus error: Connection refused"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_hint_mentions_secret_service() {
        let msg = format_unavailable_msg("anything");
        assert!(msg.contains("Secret Service"));
        assert!(msg.contains("age:/path/to/vault.age"));
        assert!(msg.contains("KLEF_PASSPHRASE"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_hint_mentions_keychain_access() {
        let msg = format_unavailable_msg("locked");
        assert!(msg.contains("Keychain Access"));
    }
}
