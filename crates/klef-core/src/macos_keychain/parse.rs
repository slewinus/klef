//! Parsers for `/usr/bin/security` outputs. Pure-string functions, no I/O.

use crate::macos_keychain::KeychainHelperError;
use std::path::PathBuf;

/// Parse `default-keychain` output. The output is a single quoted path on stdout,
/// e.g. `    "/Users/alice/Library/Keychains/login.keychain-db"\n`.
///
/// # Errors
///
/// Returns `KeychainHelperError::Parse` if the output doesn't contain a quoted path.
pub fn default_keychain_path(s: &str) -> Result<PathBuf, KeychainHelperError> {
    let trimmed = s.trim();
    let inner = trimmed
        .strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .ok_or_else(|| KeychainHelperError::Parse {
            cmd: "default-keychain",
            reason: format!("expected quoted path, got {trimmed:?}"),
        })?;
    Ok(PathBuf::from(inner))
}

/// Parse `show-keychain-info` output into (`timeout_seconds`, `lock_on_sleep`).
///
/// Possible outputs:
/// - `Keychain "<path>" no-timeout`
/// - `Keychain "<path>" timeout=600s`
/// - `Keychain "<path>" no-timeout, lock-on-sleep`
/// - `Keychain "<path>" timeout=1800s, lock-on-sleep`
///
/// macOS versions differ on stdout vs stderr; the caller should pass the
/// concatenated combined output of stdout+stderr.
///
/// # Errors
///
/// Returns `KeychainHelperError::Parse` if neither `no-timeout` nor `timeout=Ns`
/// is present in the input.
pub fn show_keychain_info(s: &str) -> Result<(Option<u64>, bool), KeychainHelperError> {
    let lock_on_sleep = s.contains("lock-on-sleep");

    if s.contains("no-timeout") {
        return Ok((None, lock_on_sleep));
    }

    let timeout = extract_timeout_seconds(s).ok_or_else(|| KeychainHelperError::Parse {
        cmd: "show-keychain-info",
        reason: format!("found neither no-timeout nor timeout=Ns in {s:?}"),
    })?;
    Ok((Some(timeout), lock_on_sleep))
}

fn extract_timeout_seconds(s: &str) -> Option<u64> {
    let needle = "timeout=";
    let start = s.find(needle)? + needle.len();
    let rest = &s[start..];
    let end = rest.find(|c: char| !c.is_ascii_digit())?;
    rest[..end].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_keychain_strips_quotes_and_whitespace() {
        let s = "    \"/Users/alice/Library/Keychains/login.keychain-db\"\n";
        let p = default_keychain_path(s).unwrap();
        assert_eq!(
            p,
            PathBuf::from("/Users/alice/Library/Keychains/login.keychain-db")
        );
    }

    #[test]
    fn default_keychain_unquoted_returns_parse_error() {
        let err = default_keychain_path("not a quoted path").unwrap_err();
        assert!(matches!(
            err,
            KeychainHelperError::Parse {
                cmd: "default-keychain",
                ..
            }
        ));
    }

    #[test]
    fn show_no_timeout_no_lock() {
        let s = "Keychain \"/path/login.keychain-db\" no-timeout\n";
        assert_eq!(show_keychain_info(s).unwrap(), (None, false));
    }

    #[test]
    fn show_no_timeout_with_lock() {
        let s = "Keychain \"/path\" no-timeout, lock-on-sleep\n";
        assert_eq!(show_keychain_info(s).unwrap(), (None, true));
    }

    #[test]
    fn show_timeout_with_lock() {
        let s = "Keychain \"/path\" timeout=600s, lock-on-sleep\n";
        assert_eq!(show_keychain_info(s).unwrap(), (Some(600), true));
    }

    #[test]
    fn show_timeout_only() {
        let s = "Keychain \"/path\" timeout=1800s\n";
        assert_eq!(show_keychain_info(s).unwrap(), (Some(1800), false));
    }

    #[test]
    fn show_unparseable_returns_parse_error() {
        let err = show_keychain_info("garbage output").unwrap_err();
        assert!(matches!(
            err,
            KeychainHelperError::Parse {
                cmd: "show-keychain-info",
                ..
            }
        ));
    }
}
