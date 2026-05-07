//! `SecurityCli` trait + production impl that wraps `/usr/bin/security`.
//!
//! Tests inject mock impls of this trait. Production code uses
//! `RealSecurityCli` which spawns the real binary at the absolute path
//! `/usr/bin/security` (not via `$PATH`).

use crate::macos_keychain::KeychainHelperError;
use std::path::Path;
use std::process::Command;

const SECURITY: &str = "/usr/bin/security";

pub trait SecurityCli {
    fn default_keychain(&self) -> Result<String, KeychainHelperError>;
    fn show_keychain_info(&self, path: &Path) -> Result<String, KeychainHelperError>;
    /// Set keychain settings.
    /// `timeout_seconds = None` and `lock_on_sleep = false` clears both.
    fn set_keychain_settings(
        &self,
        path: &Path,
        timeout_seconds: Option<u64>,
        lock_on_sleep: bool,
    ) -> Result<(), KeychainHelperError>;
}

pub struct RealSecurityCli;

impl SecurityCli for RealSecurityCli {
    fn default_keychain(&self) -> Result<String, KeychainHelperError> {
        let out = Command::new(SECURITY).arg("default-keychain").output()?;
        require_success("default-keychain", &out)?;
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    fn show_keychain_info(&self, path: &Path) -> Result<String, KeychainHelperError> {
        let out = Command::new(SECURITY)
            .arg("show-keychain-info")
            .arg(path)
            .output()?;
        require_success("show-keychain-info", &out)?;
        let mut combined = String::from_utf8_lossy(&out.stdout).into_owned();
        combined.push_str(&String::from_utf8_lossy(&out.stderr));
        Ok(combined)
    }

    fn set_keychain_settings(
        &self,
        path: &Path,
        timeout_seconds: Option<u64>,
        lock_on_sleep: bool,
    ) -> Result<(), KeychainHelperError> {
        let mut cmd = Command::new(SECURITY);
        cmd.arg("set-keychain-settings");
        if let Some(t) = timeout_seconds {
            cmd.arg("-u").arg("-t").arg(t.to_string());
        }
        if lock_on_sleep {
            cmd.arg("-l");
        }
        cmd.arg(path);
        let out = cmd.output()?;
        require_success("set-keychain-settings", &out)?;
        Ok(())
    }
}

fn require_success(
    cmd: &'static str,
    out: &std::process::Output,
) -> Result<(), KeychainHelperError> {
    if out.status.success() {
        return Ok(());
    }
    Err(KeychainHelperError::NonZeroExit {
        cmd,
        code: out.status.code().unwrap_or(-1),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}
