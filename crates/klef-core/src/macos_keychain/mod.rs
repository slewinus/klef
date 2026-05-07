//! macOS Keychain helper: wraps `/usr/bin/security` to read and modify
//! login-keychain auto-lock settings.
//!
//! Used by both `klef-cli` (for the banner + `klef keychain configure`)
//! and `klef-gui` (for a settings button). Empty on non-macOS.

#![cfg(target_os = "macos")]

mod parse;
mod security_cli;
mod status;

use security_cli::{RealSecurityCli, SecurityCli};
pub use status::{KeychainStatus, build_revert_command, is_already_friendly};

#[derive(Debug, thiserror::Error)]
pub enum KeychainHelperError {
    #[error("running /usr/bin/security: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("/usr/bin/security {cmd} exited {code}: {stderr}")]
    NonZeroExit {
        cmd: &'static str,
        code: i32,
        stderr: String,
    },
    #[error("could not parse output of /usr/bin/security {cmd}: {reason}")]
    Parse { cmd: &'static str, reason: String },
}

/// Read the current login-keychain settings.
///
/// # Errors
///
/// Any I/O, parsing, or non-zero exit from `/usr/bin/security` is wrapped
/// in `KeychainHelperError`.
pub fn current_status() -> Result<KeychainStatus, KeychainHelperError> {
    current_status_with_cli(&RealSecurityCli)
}

/// Apply the friendly settings (no timeout, no lock-on-sleep) to the
/// keychain at `status.path`.
///
/// # Errors
///
/// Any I/O or non-zero exit from `/usr/bin/security` is wrapped in
/// `KeychainHelperError`.
pub fn apply_friendly_settings(status: &KeychainStatus) -> Result<(), KeychainHelperError> {
    apply_friendly_settings_with_cli(&RealSecurityCli, status)
}

pub(crate) fn current_status_with_cli<C: SecurityCli>(
    cli: &C,
) -> Result<KeychainStatus, KeychainHelperError> {
    let path_raw = cli.default_keychain()?;
    let path = parse::default_keychain_path(&path_raw)?;
    let info = cli.show_keychain_info(&path)?;
    let (timeout_seconds, lock_on_sleep) = parse::show_keychain_info(&info)?;
    Ok(KeychainStatus {
        path,
        timeout_seconds,
        lock_on_sleep,
    })
}

pub(crate) fn apply_friendly_settings_with_cli<C: SecurityCli>(
    cli: &C,
    status: &KeychainStatus,
) -> Result<(), KeychainHelperError> {
    cli.set_keychain_settings(&status.path, None, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use security_cli::SecurityCli;
    use std::cell::RefCell;
    use std::path::{Path, PathBuf};

    struct MockCli {
        default_keychain_output: String,
        show_output: String,
        set_calls: RefCell<Vec<(PathBuf, Option<u64>, bool)>>,
    }

    impl MockCli {
        fn new(default: &str, show: &str) -> Self {
            Self {
                default_keychain_output: default.to_string(),
                show_output: show.to_string(),
                set_calls: RefCell::new(Vec::new()),
            }
        }
    }

    impl SecurityCli for MockCli {
        fn default_keychain(&self) -> Result<String, KeychainHelperError> {
            Ok(self.default_keychain_output.clone())
        }
        fn show_keychain_info(&self, _path: &Path) -> Result<String, KeychainHelperError> {
            Ok(self.show_output.clone())
        }
        fn set_keychain_settings(
            &self,
            path: &Path,
            timeout_seconds: Option<u64>,
            lock_on_sleep: bool,
        ) -> Result<(), KeychainHelperError> {
            self.set_calls
                .borrow_mut()
                .push((path.to_path_buf(), timeout_seconds, lock_on_sleep));
            Ok(())
        }
    }

    #[test]
    fn current_status_resolves_path_then_reads_settings() {
        let cli = MockCli::new(
            "    \"/Users/alice/Library/Keychains/login.keychain-db\"\n",
            "Keychain \"/Users/alice/Library/Keychains/login.keychain-db\" timeout=600s, lock-on-sleep\n",
        );
        let s = current_status_with_cli(&cli).unwrap();
        assert_eq!(
            s.path,
            PathBuf::from("/Users/alice/Library/Keychains/login.keychain-db")
        );
        assert_eq!(s.timeout_seconds, Some(600));
        assert!(s.lock_on_sleep);
    }

    #[test]
    fn apply_friendly_settings_invokes_cli_with_no_flags() {
        let cli = MockCli::new("\"/p\"\n", "Keychain \"/p\" no-timeout\n");
        let st = KeychainStatus {
            path: PathBuf::from("/p"),
            timeout_seconds: Some(30),
            lock_on_sleep: true,
        };
        apply_friendly_settings_with_cli(&cli, &st).unwrap();
        let calls = cli.set_calls.borrow();
        assert_eq!(calls.len(), 1);
        let (path, timeout, lock) = &calls[0];
        assert_eq!(path, &PathBuf::from("/p"));
        assert_eq!(*timeout, None);
        assert!(!*lock);
    }
}
