//! macOS Keychain helper.
//!
//! Wraps `/usr/bin/security` to read and modify login-keychain auto-lock
//! settings. Used by both `klef-cli` (for the banner + `klef keychain
//! configure`) and `klef-gui` (for a settings button). Empty on non-macOS.

#![cfg(target_os = "macos")]

mod parse;
mod security_cli;
mod status;

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
