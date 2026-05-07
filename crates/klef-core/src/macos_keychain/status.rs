//! `KeychainStatus` data type, friendliness predicate, revert-command builder.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeychainStatus {
    pub path: PathBuf,
    /// `None` = no auto-lock timeout configured.
    pub timeout_seconds: Option<u64>,
    /// True if the keychain locks when the system sleeps.
    pub lock_on_sleep: bool,
}

/// True iff the status indicates settings that won't trigger password
/// re-prompts during a session: no timeout AND not lock-on-sleep.
#[must_use]
pub const fn is_already_friendly(s: &KeychainStatus) -> bool {
    s.timeout_seconds.is_none() && !s.lock_on_sleep
}

/// Build a shell command string that reverts to the given prior state.
///
/// `man security` for `set-keychain-settings`:
/// - `-u`: enable "lock after timeout". Without this, `-t` has no effect.
/// - `-t N`: timeout in seconds (only honored with `-u`).
/// - `-l`: lock when system sleeps.
/// - no flags: clear both.
///
/// If `prev` is already friendly, returns a `# nothing to revert ...`
/// comment instead of a callable command.
#[must_use]
pub fn build_revert_command(prev: &KeychainStatus) -> String {
    if is_already_friendly(prev) {
        return format!(
            "# nothing to revert; previous state was already no-timeout, no-lock-on-sleep ({})",
            prev.path.display()
        );
    }
    let mut parts = vec!["security set-keychain-settings".to_string()];
    if let Some(t) = prev.timeout_seconds {
        parts.push(format!("-u -t {t}"));
    }
    if prev.lock_on_sleep {
        parts.push("-l".to_string());
    }
    parts.push(shell_quote(&prev.path));
    parts.join(" ")
}

fn shell_quote(p: &std::path::Path) -> String {
    let s = p.display().to_string();
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-'))
    {
        s
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(timeout_seconds: Option<u64>, lock_on_sleep: bool) -> KeychainStatus {
        KeychainStatus {
            path: PathBuf::from("/Users/alice/Library/Keychains/login.keychain-db"),
            timeout_seconds,
            lock_on_sleep,
        }
    }

    #[test]
    fn friendly_only_when_both_clear() {
        assert!(is_already_friendly(&s(None, false)));
        assert!(!is_already_friendly(&s(None, true)));
        assert!(!is_already_friendly(&s(Some(600), false)));
        assert!(!is_already_friendly(&s(Some(600), true)));
    }

    #[test]
    fn revert_includes_u_t_and_l_for_full_state() {
        let cmd = build_revert_command(&s(Some(600), true));
        assert!(cmd.contains("-u -t 600"), "missing -u -t in {cmd}");
        assert!(cmd.contains(" -l "), "missing -l in {cmd}");
        assert!(cmd.contains("login.keychain-db"));
    }

    #[test]
    fn revert_includes_u_t_only_when_no_lock() {
        let cmd = build_revert_command(&s(Some(30), false));
        assert!(cmd.contains("-u -t 30"));
        assert!(!cmd.contains(" -l "));
    }

    #[test]
    fn revert_includes_l_only_when_no_timeout() {
        let cmd = build_revert_command(&s(None, true));
        assert!(!cmd.contains("-u"));
        assert!(!cmd.contains("-t"));
        assert!(cmd.contains(" -l "));
    }

    #[test]
    fn revert_friendly_state_returns_nothing_to_revert_comment() {
        let cmd = build_revert_command(&s(None, false));
        assert!(cmd.starts_with("# nothing to revert"), "got: {cmd}");
        assert!(!cmd.contains("set-keychain-settings"));
    }

    #[test]
    fn revert_quotes_path_with_spaces() {
        let mut st = s(Some(60), false);
        st.path = PathBuf::from("/Users/alice with space/login.keychain-db");
        let cmd = build_revert_command(&st);
        assert!(
            cmd.contains("'/Users/alice with space/login.keychain-db'"),
            "got: {cmd}"
        );
    }
}
