use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum KlefError {
    #[error("backend unavailable: {0}")]
    BackendUnavailable(String),
    #[error("backend access denied. Check Keychain permissions and retry")]
    BackendDenied,
    #[error("index file corrupt at {path}: {reason}")]
    IndexCorrupt { path: PathBuf, reason: String },
    // Note: IndexWrite and Io both wrap io::Error and so neither uses #[from] —
    // call sites must explicitly choose. IndexWrite is reserved for the atomic
    // index write/rename in store::index; every other I/O failure (stdin, stdout,
    // .env reads, etc.) goes through Io. See docs/design §7.1.
    #[error("failed to write index: {0}")]
    IndexWrite(std::io::Error),
    #[error("i/o error: {0}")]
    Io(std::io::Error),
    #[error("key '{0}' not found. List available keys: klef list")]
    KeyNotFound(String),
    #[error("key '{0}' already exists (use --force to overwrite)")]
    KeyAlreadyExists(String),
    #[error("invalid key name '{0}': use only alphanumeric, dash, or underscore")]
    InvalidKeyName(String),
    #[error(
        "invalid env-var name '{0}': must match [A-Za-z_][A-Za-z0-9_]* (POSIX shell-safe identifier)"
    )]
    InvalidEnvVar(String),
    #[error("env file not found: {0}. Check the path or pass --env-file <FILE>")]
    EnvFileNotFound(PathBuf),
    #[error(
        "broken reference: {var}=klef:{key} - key '{key}' not found. Add it with: klef add {key}"
    )]
    BrokenReference { var: String, key: String },
    #[error("invalid skip pattern '{0}': must be a valid regex")]
    InvalidSkipPattern(String),
}

impl KlefError {
    #[must_use]
    pub const fn exit_code(&self) -> i32 {
        match self {
            Self::BackendUnavailable(_) | Self::BackendDenied => 4,
            Self::KeyNotFound(_) => 2,
            Self::BrokenReference { .. } => 3,
            _ => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_code_for_key_not_found_is_2() {
        let e = KlefError::KeyNotFound("stripe".into());
        assert_eq!(e.exit_code(), 2);
    }

    #[test]
    fn exit_code_for_broken_ref_is_3() {
        let e = KlefError::BrokenReference {
            var: "STRIPE_KEY".into(),
            key: "stripe".into(),
        };
        assert_eq!(e.exit_code(), 3);
    }

    #[test]
    fn exit_code_for_backend_is_4() {
        let e = KlefError::BackendDenied;
        assert_eq!(e.exit_code(), 4);
    }
}
