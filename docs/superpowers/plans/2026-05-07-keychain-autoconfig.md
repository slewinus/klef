# macOS Keychain auto-configuration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a `klef keychain configure` subcommand that disables the macOS login keychain auto-lock timeout, plus a one-time stderr banner that surfaces the command in-context when klef detects the issue. Helper code lives in `klef-core` so the GUI can reuse it.

**Architecture:** Pure helper module in `klef-core::macos_keychain` (`#[cfg(target_os = "macos")]`) wraps `/usr/bin/security`. Public API: `current_status()`, `apply_friendly_settings()`, `is_already_friendly()`, `build_revert_command()`. CLI adds the `klef keychain configure` subcommand and a banner trigger that fires before value-touching commands run. Banner suppression via JSON marker file at `~/.config/klef/keychain-configured` (TTL 7 days, re-shown when keychain state drifts).

**Tech Stack:** Rust 2024, `std::process::Command` for shell-out, `serde`/`serde_json` for the marker file (existing deps). No new crates.

**Spec:** [`docs/superpowers/specs/2026-05-07-keychain-autoconfig-design.md`](../specs/2026-05-07-keychain-autoconfig-design.md)

**Notes for the engineer before starting:**
- Repo enforces `< 300 lines/file` via `.githooks/pre-commit` (`scripts/check-lines.sh`) and `cargo clippy --workspace --all-targets --all-features -- -D warnings` (workspace lints set `pedantic` + `nursery`). All new code must pass both.
- `klef-core` has `unsafe_code = "forbid"`. Don't introduce unsafe; nothing in this work needs it.
- All new tests run on macOS only via `#[cfg(target_os = "macos")]`. On Linux the modules compile to no-ops. CI already runs both.
- The plan never touches the user's real keychain in tests — every shell-out is mocked behind a `SecurityCli` trait. Manual smoke testing is documented in Task 12.

---

## File Structure

**Created:**
- `crates/klef-core/src/macos_keychain/mod.rs` — public API (`current_status`, `apply_friendly_settings`, error type), no-op on non-macOS.
- `crates/klef-core/src/macos_keychain/parse.rs` — parsing of `/usr/bin/security` outputs.
- `crates/klef-core/src/macos_keychain/status.rs` — `KeychainStatus` struct, `is_already_friendly`, `build_revert_command`.
- `crates/klef-core/src/macos_keychain/security_cli.rs` — `SecurityCli` trait + production impl that wraps `/usr/bin/security`.
- `crates/klef-cli/src/commands/keychain.rs` — `klef keychain configure` handler.
- `crates/klef-cli/src/macos_keychain_banner.rs` — banner trigger logic + JSON marker file.
- `docs/macos-keychain.md` — user-facing docs.

**Modified:**
- `crates/klef-core/src/lib.rs` — `pub mod macos_keychain;` (gated).
- `crates/klef-cli/src/cli.rs` — add `Keychain { action: KeychainAction }` variant + `KeychainAction::Configure`.
- `crates/klef-cli/src/commands/mod.rs` — `pub mod keychain;` (gated `target_os = "macos"`).
- `crates/klef-cli/src/lib.rs` — dispatch + banner trigger call from `run()`.
- `crates/klef-cli/src/macos_keychain_banner.rs` — declared as `pub mod` in `klef-cli/src/lib.rs` (or `main.rs`).
- `README.md` — short note in macOS section.

---

## Task 1: klef-core scaffold + error type

Create the module skeleton in `klef-core`. Empty on non-macOS; on macOS, exports a `KeychainHelperError` enum and the `Path` import.

**Files:**
- Create: `crates/klef-core/src/macos_keychain/mod.rs`
- Modify: `crates/klef-core/src/lib.rs` (add `pub mod macos_keychain;`)

- [ ] **Step 1: Create `macos_keychain/mod.rs`**

```rust
//! macOS Keychain helper: wraps `/usr/bin/security` to read and modify
//! login-keychain auto-lock settings. Used by both `klef-cli` (for the
//! banner + `klef keychain configure`) and `klef-gui` (for a settings
//! button). Empty on non-macOS.

#![cfg(target_os = "macos")]

mod parse;
mod security_cli;
mod status;

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
    Parse {
        cmd: &'static str,
        reason: String,
    },
}
```

- [ ] **Step 2: Register the module in `klef-core/src/lib.rs`**

Add after the existing `pub mod store;`:

```rust
#[cfg(target_os = "macos")]
pub mod macos_keychain;
```

- [ ] **Step 3: Create empty stub files for the submodules so the crate compiles**

`crates/klef-core/src/macos_keychain/parse.rs`:
```rust
//! Parsers for `/usr/bin/security` outputs. Pure-string functions, no I/O.
```

`crates/klef-core/src/macos_keychain/security_cli.rs`:
```rust
//! `SecurityCli` trait + production impl that wraps `/usr/bin/security`.
```

`crates/klef-core/src/macos_keychain/status.rs`:
```rust
//! `KeychainStatus` data type, friendliness predicate, revert-command builder.
```

- [ ] **Step 4: Verify build**

```bash
cargo build -p klef-core
cargo build -p klef-core --target x86_64-unknown-linux-gnu  # if available; else skip
```
Expected: green. (On Linux/CI the module is gated out and the empty submodules don't compile.)

- [ ] **Step 5: Commit**

```bash
git add crates/klef-core/src/macos_keychain/ crates/klef-core/src/lib.rs
git commit -m "feat(keychain): scaffold macos_keychain module in klef-core"
```

---

## Task 2: Parse `/usr/bin/security` outputs

TDD-implement two parsers: one for `default-keychain` (outputs a quoted path), one for `show-keychain-info` (outputs timeout + lock-on-sleep info on stdout or stderr depending on macOS version).

**Files:**
- Modify: `crates/klef-core/src/macos_keychain/parse.rs`

- [ ] **Step 1: Write failing tests**

Replace the contents of `crates/klef-core/src/macos_keychain/parse.rs` with:

```rust
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

/// Parse `show-keychain-info` output into (timeout_seconds, lock_on_sleep).
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

    // Match `timeout=NUMs` (NUM is 1+ digits).
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
        assert_eq!(p, PathBuf::from("/Users/alice/Library/Keychains/login.keychain-db"));
    }

    #[test]
    fn default_keychain_unquoted_returns_parse_error() {
        let err = default_keychain_path("not a quoted path").unwrap_err();
        assert!(matches!(err, KeychainHelperError::Parse { cmd: "default-keychain", .. }));
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
        assert!(matches!(err, KeychainHelperError::Parse { cmd: "show-keychain-info", .. }));
    }
}
```

- [ ] **Step 2: Run tests, confirm pass**

```bash
cargo test -p klef-core macos_keychain::parse
```
Expected: 7 tests PASS (the implementation is included alongside the tests, so they pass on first run — this is OK for parsers where the test cases are essentially the spec).

- [ ] **Step 3: Verify hooks**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
scripts/check-lines.sh
```
All green.

- [ ] **Step 4: Commit**

```bash
git add crates/klef-core/src/macos_keychain/parse.rs
git commit -m "feat(keychain): parsers for /usr/bin/security default-keychain and show-keychain-info"
```

---

## Task 3: `KeychainStatus` + `is_already_friendly` + `build_revert_command`

Pure data + logic — no I/O. TDD.

**Files:**
- Modify: `crates/klef-core/src/macos_keychain/status.rs`

- [ ] **Step 1: Write failing tests**

Replace `crates/klef-core/src/macos_keychain/status.rs`:

```rust
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
/// If `prev` is already friendly, returns a `# nothing to revert ...` comment
/// instead of a callable command (caller can print it; user sees that no
/// revert is needed).
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
    parts.push(format!("{}", shell_quote(&prev.path)));
    parts.join(" ")
}

fn shell_quote(p: &std::path::Path) -> String {
    // Single-quote the path, escaping any embedded `'` as `'\''`.
    let s = p.display().to_string();
    if s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-')) {
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
```

- [ ] **Step 2: Run tests, confirm pass**

```bash
cargo test -p klef-core macos_keychain::status
```
Expected: 6 tests PASS.

- [ ] **Step 3: Hooks**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
scripts/check-lines.sh
```

- [ ] **Step 4: Commit**

```bash
git add crates/klef-core/src/macos_keychain/status.rs
git commit -m "feat(keychain): KeychainStatus, friendliness predicate, revert command builder"
```

---

## Task 4: `SecurityCli` trait + production impl

The trait abstracts the three `/usr/bin/security` invocations. Production impl wraps `Command::new("/usr/bin/security")`. No new tests for the prod impl (it's a thin wrapper); tests come in Task 5 via mocking.

**Files:**
- Modify: `crates/klef-core/src/macos_keychain/security_cli.rs`

- [ ] **Step 1: Implement trait + production impl**

Replace `crates/klef-core/src/macos_keychain/security_cli.rs`:

```rust
//! `SecurityCli` trait + production impl that wraps `/usr/bin/security`.
//!
//! Tests inject mock impls of this trait. Production code uses
//! `RealSecurityCli` which spawns the real binary at the absolute path
//! `/usr/bin/security` (not via `$PATH`).

use crate::macos_keychain::KeychainHelperError;
use std::path::Path;
use std::process::Command;

const SECURITY: &str = "/usr/bin/security";

pub(crate) trait SecurityCli {
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

pub(crate) struct RealSecurityCli;

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
        // macOS versions vary on stdout vs stderr; concatenate both.
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
```

- [ ] **Step 2: Verify build**

```bash
cargo build -p klef-core
```
Expected: green.

- [ ] **Step 3: Hooks**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
scripts/check-lines.sh
```

- [ ] **Step 4: Commit**

```bash
git add crates/klef-core/src/macos_keychain/security_cli.rs
git commit -m "feat(keychain): SecurityCli trait + production impl wrapping /usr/bin/security"
```

---

## Task 5: Public API: `current_status` + `apply_friendly_settings`

Add the public API to `mod.rs`. Both functions have a `pub` no-arg version (uses `RealSecurityCli`) and a `pub(crate) *_with_cli` variant for tests. Mock-based tests for the `_with_cli` variants.

**Files:**
- Modify: `crates/klef-core/src/macos_keychain/mod.rs`

- [ ] **Step 1: Add public API + tests**

Replace `crates/klef-core/src/macos_keychain/mod.rs`:

```rust
//! macOS Keychain helper: wraps `/usr/bin/security` to read and modify
//! login-keychain auto-lock settings. Used by both `klef-cli` (for the
//! banner + `klef keychain configure`) and `klef-gui` (for a settings
//! button). Empty on non-macOS.

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
    Parse {
        cmd: &'static str,
        reason: String,
    },
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
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p klef-core macos_keychain
```
Expected: 15 tests PASS (parse: 7, status: 6, mod: 2).

- [ ] **Step 3: Hooks**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
scripts/check-lines.sh
```

- [ ] **Step 4: Commit**

```bash
git add crates/klef-core/src/macos_keychain/mod.rs
git commit -m "feat(keychain): public API current_status + apply_friendly_settings with mock-tested logic"
```

---

## Task 6: CLI subcommand declaration

Add `Keychain { action: KeychainAction }` to `cli.rs` with one variant `Configure`. macOS-only via `#[cfg(target_os = "macos")]`.

**Files:**
- Modify: `crates/klef-cli/src/cli.rs`

- [ ] **Step 1: Add `KeychainAction` enum and `Keychain` variant**

In `crates/klef-cli/src/cli.rs`, near the top after the existing `use clap::...` imports, add:

```rust
#[cfg(target_os = "macos")]
#[derive(clap::Subcommand)]
pub enum KeychainAction {
    /// Disable macOS keychain auto-lock so klef stops prompting for your
    /// password every time the keychain re-locks.
    Configure,
}
```

Then in the `enum Command { ... }` block, add (near the end, just before the closing `}`):

```rust
    /// macOS keychain helpers (avoid frequent password prompts).
    #[cfg(target_os = "macos")]
    Keychain {
        #[command(subcommand)]
        action: KeychainAction,
    },
```

- [ ] **Step 2: Verify build**

```bash
cargo build -p klef
cargo run -p klef -- keychain --help
```
Expected on macOS: `klef keychain --help` shows the `configure` subcommand. On Linux: clap rejects `keychain` as unknown.

- [ ] **Step 3: Hooks**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

- [ ] **Step 4: Commit**

```bash
git add crates/klef-cli/src/cli.rs
git commit -m "feat(keychain): klef keychain configure subcommand declaration (macOS only)"
```

---

## Task 7: `klef keychain configure` handler

Implement the `commands::keychain::configure()` handler. It uses the `klef-core` helper, prints the result + revert command, and writes the marker. TDD against a mock helper API: we extract a small inner function that takes `current_status` + `apply` callbacks so we can test without invoking real `security`.

**Files:**
- Create: `crates/klef-cli/src/commands/keychain.rs`
- Modify: `crates/klef-cli/src/commands/mod.rs` (add gated `pub mod keychain;`)
- Modify: `crates/klef-cli/src/lib.rs` (dispatch the new variant)

- [ ] **Step 1: Create `commands/keychain.rs` with handler + tests**

Create `crates/klef-cli/src/commands/keychain.rs`:

```rust
//! `klef keychain configure` — explicit user-action that disables macOS
//! keychain auto-lock (no timeout, no lock-on-sleep) and writes a marker
//! recording the prior state for revert.

#![cfg(target_os = "macos")]

use klef_core::error::KlefError;
use klef_core::macos_keychain::{
    KeychainStatus, apply_friendly_settings, build_revert_command, current_status,
    is_already_friendly,
};
use std::io::Write;

const MARKER_FILE: &str = "keychain-configured";

/// Top-level handler for `klef keychain configure`.
///
/// # Errors
///
/// Returns `KlefError::BackendUnavailable` if the underlying
/// `KeychainHelperError` cannot be recovered from. Marker write failures
/// are warnings, not errors (the underlying setting was already applied).
pub fn run() -> Result<(), KlefError> {
    let mut stderr = std::io::stderr();
    let mut stdout = std::io::stdout();
    run_with(&mut stdout, &mut stderr, current_status, apply_friendly_settings)
}

fn run_with(
    stdout: &mut impl Write,
    stderr: &mut impl Write,
    read: impl FnOnce() -> Result<KeychainStatus, klef_core::macos_keychain::KeychainHelperError>,
    apply: impl FnOnce(&KeychainStatus) -> Result<(), klef_core::macos_keychain::KeychainHelperError>,
) -> Result<(), KlefError> {
    let prev = read().map_err(|e| KlefError::BackendUnavailable(e.to_string()))?;

    if is_already_friendly(&prev) {
        writeln!(
            stdout,
            "macOS keychain is already configured for klef (no timeout, no lock-on-sleep). Nothing to do."
        )
        .ok();
        write_marker_applied(&prev).ok();
        return Ok(());
    }

    apply(&prev).map_err(|e| KlefError::BackendUnavailable(e.to_string()))?;

    writeln!(
        stdout,
        "macOS keychain configured: no timeout, no lock-on-sleep. You should no longer be prompted for your password during this login session."
    )
    .ok();
    writeln!(stdout, "To revert, run:").ok();
    writeln!(stdout, "    {}", build_revert_command(&prev)).ok();

    if let Err(e) = write_marker_applied(&prev) {
        writeln!(
            stderr,
            "warning: keychain settings updated but marker write failed: {e}"
        )
        .ok();
    }

    Ok(())
}

fn write_marker_applied(prev: &KeychainStatus) -> Result<(), std::io::Error> {
    let path = marker_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let now = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string());
    let body = serde_json::json!({
        "applied": true,
        "configured_at": now,
        "keychain_path": prev.path,
        "prev_timeout_seconds": prev.timeout_seconds,
        "prev_lock_on_sleep": prev.lock_on_sleep,
    });
    std::fs::write(&path, serde_json::to_vec_pretty(&body).unwrap_or_default())?;
    Ok(())
}

fn marker_path() -> Result<std::path::PathBuf, std::io::Error> {
    let base = dirs::config_dir().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "config_dir unavailable")
    })?;
    Ok(base.join("klef").join(MARKER_FILE))
}

#[cfg(test)]
mod tests {
    use super::*;
    use klef_core::macos_keychain::KeychainHelperError;
    use std::cell::RefCell;
    use std::path::PathBuf;

    fn st(timeout_seconds: Option<u64>, lock_on_sleep: bool) -> KeychainStatus {
        KeychainStatus {
            path: PathBuf::from("/Users/alice/Library/Keychains/login.keychain-db"),
            timeout_seconds,
            lock_on_sleep,
        }
    }

    #[test]
    fn idempotent_when_already_friendly_does_not_call_apply() {
        let read_called = RefCell::new(false);
        let apply_called = RefCell::new(false);
        let mut out = Vec::new();
        let mut err = Vec::new();
        run_with(
            &mut out,
            &mut err,
            || {
                *read_called.borrow_mut() = true;
                Ok(st(None, false))
            },
            |_s| {
                *apply_called.borrow_mut() = true;
                Ok(())
            },
        )
        .unwrap();
        assert!(*read_called.borrow());
        assert!(!*apply_called.borrow(), "apply must not be called when already friendly");
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("already configured"));
    }

    #[test]
    fn applies_and_prints_revert_command_when_not_friendly() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        run_with(
            &mut out,
            &mut err,
            || Ok(st(Some(600), true)),
            |_s| Ok(()),
        )
        .unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("configured"));
        assert!(s.contains("To revert"));
        assert!(s.contains("-u -t 600"));
        assert!(s.contains(" -l "));
    }

    #[test]
    fn read_failure_propagates_as_backend_unavailable() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let result: Result<(), KlefError> = run_with(
            &mut out,
            &mut err,
            || {
                Err(KeychainHelperError::Parse {
                    cmd: "default-keychain",
                    reason: "boom".into(),
                })
            },
            |_s| Ok(()),
        );
        let e = result.unwrap_err();
        let msg = e.to_string();
        assert!(msg.contains("default-keychain"), "got: {msg}");
    }

    #[test]
    fn apply_failure_propagates_as_backend_unavailable() {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let result: Result<(), KlefError> = run_with(
            &mut out,
            &mut err,
            || Ok(st(Some(600), true)),
            |_s| {
                Err(KeychainHelperError::NonZeroExit {
                    cmd: "set-keychain-settings",
                    code: 1,
                    stderr: "denied".into(),
                })
            },
        );
        let e = result.unwrap_err();
        assert!(e.to_string().contains("set-keychain-settings"));
    }
}
```

- [ ] **Step 2: Register the module**

In `crates/klef-cli/src/commands/mod.rs`, add:

```rust
#[cfg(target_os = "macos")]
pub mod keychain;
```

(Place it alphabetically among the other `pub mod` lines.)

- [ ] **Step 3: Wire dispatch in `lib.rs`**

In `crates/klef-cli/src/lib.rs`, before the closing `}` of the dispatch `match`, add:

```rust
        #[cfg(target_os = "macos")]
        Command::Keychain { action } => match action {
            cli::KeychainAction::Configure => commands::keychain::run(),
        },
```

(The `cli::KeychainAction` path matches the `pub enum KeychainAction` in `cli.rs`. If `cli` is already brought into scope via `use cli::{Cli, Command};`, you may write `KeychainAction::Configure` directly — match the existing style.)

- [ ] **Step 4: Run tests**

```bash
cargo test -p klef commands::keychain
cargo build -p klef
```
Expected: 4 tests PASS, build green.

- [ ] **Step 5: Hooks**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
scripts/check-lines.sh
```

- [ ] **Step 6: Commit**

```bash
git add crates/klef-cli/src/commands/keychain.rs \
        crates/klef-cli/src/commands/mod.rs \
        crates/klef-cli/src/lib.rs
git commit -m "feat(keychain): klef keychain configure handler with marker write"
```

---

## Task 8: Banner marker model — load, write, TTL/state-drift checks

The marker file at `~/.config/klef/keychain-configured` has two shapes (banner-shown, applied). Logic for: load marker, decide whether to re-show banner, write banner-shown marker. TDD with TempDir as fake config dir (via `KLEF_KEYCHAIN_MARKER_DIR` env var that the tests set).

**Files:**
- Create: `crates/klef-cli/src/macos_keychain_banner.rs`
- Modify: `crates/klef-cli/src/lib.rs` (declare the module)

- [ ] **Step 1: Create the module + load/write logic + TTL/drift tests**

Create `crates/klef-cli/src/macos_keychain_banner.rs`:

```rust
//! Banner trigger for the macOS keychain timeout issue. Decides whether
//! to print a one-time stderr banner pointing the user at
//! `klef keychain configure`. Suppressed via marker file at
//! `~/.config/klef/keychain-configured` with TTL + state-drift re-show.

#![cfg(target_os = "macos")]

use klef_core::macos_keychain::KeychainStatus;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const MARKER_FILE: &str = "keychain-configured";
const TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);

#[derive(Debug, Serialize, Deserialize, Default)]
pub(crate) struct Marker {
    pub(crate) applied: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) configured_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) banner_shown_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) banner_state: Option<BannerState>,
    // Revert info — present when applied=true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) keychain_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) prev_timeout_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) prev_lock_on_sleep: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct BannerState {
    pub(crate) keychain_path: PathBuf,
    pub(crate) timeout_seconds: Option<u64>,
    pub(crate) lock_on_sleep: bool,
}

impl BannerState {
    pub(crate) fn from_status(s: &KeychainStatus) -> Self {
        Self {
            keychain_path: s.path.clone(),
            timeout_seconds: s.timeout_seconds,
            lock_on_sleep: s.lock_on_sleep,
        }
    }
    pub(crate) fn matches_status(&self, s: &KeychainStatus) -> bool {
        self.keychain_path == s.path
            && self.timeout_seconds == s.timeout_seconds
            && self.lock_on_sleep == s.lock_on_sleep
    }
}

pub(crate) fn marker_path() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("KLEF_KEYCHAIN_MARKER_DIR") {
        return Some(PathBuf::from(dir).join(MARKER_FILE));
    }
    Some(dirs::config_dir()?.join("klef").join(MARKER_FILE))
}

pub(crate) fn load_marker(path: &Path) -> Option<Marker> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub(crate) fn marker_age(path: &Path) -> Option<Duration> {
    let mtime = std::fs::metadata(path).ok()?.modified().ok()?;
    SystemTime::now().duration_since(mtime).ok()
}

/// Decide whether the banner should be re-shown given the current state.
pub(crate) fn should_show_banner(path: &Path, current: &KeychainStatus) -> bool {
    let Some(marker) = load_marker(path) else { return true };
    if marker.applied {
        return false;
    }
    // applied=false: re-show when state drifted OR marker is stale.
    let drifted = marker
        .banner_state
        .as_ref()
        .map_or(true, |bs| !bs.matches_status(current));
    if drifted {
        return true;
    }
    matches!(marker_age(path), Some(age) if age > TTL)
}

pub(crate) fn write_banner_shown(
    path: &Path,
    current: &KeychainStatus,
) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let now = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string());
    let payload = serde_json::json!({
        "applied": false,
        "banner_shown_at": now,
        "banner_state": BannerState::from_status(current),
    });
    std::fs::write(path, serde_json::to_vec_pretty(&payload).unwrap_or_default())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn st() -> KeychainStatus {
        KeychainStatus {
            path: PathBuf::from("/Users/alice/Library/Keychains/login.keychain-db"),
            timeout_seconds: Some(600),
            lock_on_sleep: true,
        }
    }

    #[test]
    fn show_banner_when_marker_missing() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(MARKER_FILE);
        assert!(should_show_banner(&path, &st()));
    }

    #[test]
    fn no_banner_when_applied_marker_present() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(MARKER_FILE);
        let body = serde_json::json!({
            "applied": true,
            "configured_at": "2026-05-07T14:23:45Z",
            "keychain_path": "/p",
            "prev_timeout_seconds": 600,
            "prev_lock_on_sleep": true,
        });
        std::fs::write(&path, serde_json::to_vec(&body).unwrap()).unwrap();
        assert!(!should_show_banner(&path, &st()));
    }

    #[test]
    fn no_banner_when_shown_marker_state_matches_and_recent() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(MARKER_FILE);
        write_banner_shown(&path, &st()).unwrap();
        assert!(!should_show_banner(&path, &st()));
    }

    #[test]
    fn banner_re_shown_when_state_drifts() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(MARKER_FILE);
        write_banner_shown(&path, &st()).unwrap();
        let mut drifted = st();
        drifted.timeout_seconds = Some(30); // changed
        assert!(should_show_banner(&path, &drifted));
    }

    #[test]
    fn banner_re_shown_when_marker_older_than_ttl() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(MARKER_FILE);
        write_banner_shown(&path, &st()).unwrap();
        // Backdate mtime to 8 days ago.
        let old = SystemTime::now() - Duration::from_secs(8 * 24 * 60 * 60);
        let f = std::fs::File::open(&path).unwrap();
        f.set_modified(old).unwrap();
        assert!(should_show_banner(&path, &st()));
    }
}
```

- [ ] **Step 2: Declare the module in `klef-cli/src/lib.rs`**

Near the top of `crates/klef-cli/src/lib.rs`:

```rust
#[cfg(target_os = "macos")]
mod macos_keychain_banner;
```

- [ ] **Step 3: Run tests**

```bash
cargo test -p klef macos_keychain_banner
```
Expected: 5 tests PASS.

- [ ] **Step 4: Hooks**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
scripts/check-lines.sh
```

- [ ] **Step 5: Commit**

```bash
git add crates/klef-cli/src/macos_keychain_banner.rs crates/klef-cli/src/lib.rs
git commit -m "feat(keychain): banner marker model with TTL and state-drift re-show"
```

---

## Task 9: Banner trigger predicates + emit logic

Add the predicates `command_touches_values()` and `backend_is_keychain()`, plus a `maybe_emit_banner()` function that combines marker check + env-var opt-out + status read + print.

**Files:**
- Modify: `crates/klef-cli/src/macos_keychain_banner.rs`

- [ ] **Step 1: Append predicates + `maybe_emit_banner` + tests**

Append to `crates/klef-cli/src/macos_keychain_banner.rs` (above the existing `#[cfg(test)] mod tests`):

```rust
use crate::cli::Command;
use klef_core::macos_keychain::{KeychainHelperError, current_status, is_already_friendly};
use klef_core::store::Store;
use std::io::Write;

const OPT_OUT_ENV: &str = "KLEF_NO_KEYCHAIN_AUTOCONFIG";

/// Whether the given command will read or write a keychain value.
/// Returns `false` for read-only/metadata commands like list/status/completions.
#[must_use]
pub(crate) const fn command_touches_values(cmd: &Command) -> bool {
    matches!(
        cmd,
        Command::Get { .. }
            | Command::Show { .. }
            | Command::Run { .. }
            | Command::Add { .. }
            | Command::Edit { .. }
            | Command::Rm { .. }
            | Command::Rename { .. }
            | Command::Import { .. }
            | Command::Export { .. }
    )
}

/// Whether the resolved store backend is the OS keychain.
pub(crate) fn backend_is_keychain(store: &Store) -> bool {
    store.backend_description() == "keychain"
}

/// Try to emit the banner. All-or-nothing best-effort: returns silently
/// on any error. Caller does not propagate errors.
pub(crate) fn maybe_emit_banner<W: Write>(stderr: &mut W) {
    maybe_emit_with(stderr, current_status)
}

fn maybe_emit_with<W: Write>(
    stderr: &mut W,
    read_status: impl FnOnce() -> Result<KeychainStatus, KeychainHelperError>,
) {
    if std::env::var_os(OPT_OUT_ENV).is_some() {
        return;
    }
    let Some(path) = marker_path() else { return };

    let Ok(status) = read_status() else { return };

    if is_already_friendly(&status) {
        // Nothing to warn about; persist the friendly state as an "applied"
        // marker so we never check again.
        let _ = write_already_friendly(&path, &status);
        return;
    }

    if !should_show_banner(&path, &status) {
        return;
    }

    let timeout_disp = status
        .timeout_seconds
        .map_or_else(|| "off".to_string(), |s| format!("{s}s"));
    let lock_disp = if status.lock_on_sleep { "yes" } else { "no" };

    let _ = writeln!(
        stderr,
        "klef: heads up — your macOS keychain auto-locks (timeout: {timeout_disp}, lock-on-sleep: {lock_disp})."
    );
    let _ = writeln!(
        stderr,
        "      You'll be prompted for your password on every klef call until this is fixed."
    );
    let _ = writeln!(
        stderr,
        "      One-shot fix (modifies macOS Keychain settings, not your klef data):"
    );
    let _ = writeln!(stderr, "          klef keychain configure");
    let _ = writeln!(stderr, "      To suppress this notice without fixing:");
    let _ = writeln!(stderr, "          export {OPT_OUT_ENV}=1");

    let _ = write_banner_shown(&path, &status);
}

fn write_already_friendly(
    path: &Path,
    status: &KeychainStatus,
) -> Result<(), std::io::Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let now = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string());
    let body = serde_json::json!({
        "applied": true,
        "configured_at": now,
        "keychain_path": status.path,
        "prev_timeout_seconds": status.timeout_seconds,
        "prev_lock_on_sleep": status.lock_on_sleep,
    });
    std::fs::write(path, serde_json::to_vec_pretty(&body).unwrap_or_default())?;
    Ok(())
}
```

- [ ] **Step 2: Add tests for the new functions**

In the `#[cfg(test)] mod tests { ... }` block at the bottom of the file, append:

```rust
    #[test]
    fn opt_out_env_var_suppresses_banner() {
        let tmp = TempDir::new().unwrap();
        // SAFETY: this test sets a process-global env var. Other tests in the
        // module don't read this var, so the race is bounded to this scope.
        unsafe {
            std::env::set_var(OPT_OUT_ENV, "1");
            std::env::set_var("KLEF_KEYCHAIN_MARKER_DIR", tmp.path());
        }
        let mut buf: Vec<u8> = Vec::new();
        maybe_emit_with(&mut buf, || Ok(st()));
        unsafe {
            std::env::remove_var(OPT_OUT_ENV);
            std::env::remove_var("KLEF_KEYCHAIN_MARKER_DIR");
        }
        assert!(buf.is_empty(), "expected no output, got: {:?}", String::from_utf8_lossy(&buf));
    }

    #[test]
    fn already_friendly_state_writes_applied_marker_no_banner() {
        let tmp = TempDir::new().unwrap();
        unsafe {
            std::env::set_var("KLEF_KEYCHAIN_MARKER_DIR", tmp.path());
        }
        let mut buf: Vec<u8> = Vec::new();
        maybe_emit_with(&mut buf, || {
            Ok(KeychainStatus {
                path: PathBuf::from("/p"),
                timeout_seconds: None,
                lock_on_sleep: false,
            })
        });
        unsafe {
            std::env::remove_var("KLEF_KEYCHAIN_MARKER_DIR");
        }
        assert!(buf.is_empty());
        let marker_file = tmp.path().join(MARKER_FILE);
        let body: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&marker_file).unwrap()).unwrap();
        assert_eq!(body["applied"], true);
    }

    #[test]
    fn unfriendly_state_emits_banner_and_writes_shown_marker() {
        let tmp = TempDir::new().unwrap();
        unsafe {
            std::env::set_var("KLEF_KEYCHAIN_MARKER_DIR", tmp.path());
        }
        let mut buf: Vec<u8> = Vec::new();
        maybe_emit_with(&mut buf, || Ok(st()));
        unsafe {
            std::env::remove_var("KLEF_KEYCHAIN_MARKER_DIR");
        }
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("klef keychain configure"));
        assert!(s.contains("KLEF_NO_KEYCHAIN_AUTOCONFIG"));
        let marker_file = tmp.path().join(MARKER_FILE);
        let body: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&marker_file).unwrap()).unwrap();
        assert_eq!(body["applied"], false);
        assert!(body["banner_state"].is_object());
    }
```

- [ ] **Step 3: Run tests**

Tests in this file mutate process env. Run with `--test-threads=1` to avoid races with other tests in the same binary:

```bash
cargo test -p klef macos_keychain_banner -- --test-threads=1
```
Expected: 8 tests PASS (5 from Task 8 + 3 new).

- [ ] **Step 4: Hooks**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
scripts/check-lines.sh
```

- [ ] **Step 5: Commit**

```bash
git add crates/klef-cli/src/macos_keychain_banner.rs
git commit -m "feat(keychain): banner trigger predicates and emit logic with opt-out env var"
```

---

## Task 10: Wire the banner into `klef-cli::run()`

Call `maybe_emit_banner()` exactly once per process, only when both predicates hold (backend = keychain AND command touches values).

**Files:**
- Modify: `crates/klef-cli/src/lib.rs`

- [ ] **Step 1: Add the trigger call in `run()`**

In `crates/klef-cli/src/lib.rs`, modify the `pub fn run(cli: Cli) -> Result<(), KlefError>` body. Find the line:

```rust
    let store = klef_core::build_store(cli.backend.as_deref())?;
```

Insert immediately after it:

```rust
    #[cfg(target_os = "macos")]
    if macos_keychain_banner::backend_is_keychain(&store)
        && macos_keychain_banner::command_touches_values(&cli.command)
    {
        macos_keychain_banner::maybe_emit_banner(&mut std::io::stderr());
    }
```

- [ ] **Step 2: Verify build + smoke**

```bash
cargo build -p klef
# A no-op smoke test (won't trigger banner because we use the file backend).
KLEF_TEST_BACKEND=file:/tmp/klef-banner-smoke cargo run -p klef -- list 2>&1 | head -5
```
Expected: `klef list` runs normally without printing the banner (file backend → predicate false).

- [ ] **Step 3: Test full suite**

```bash
cargo test --workspace --all-features -- --test-threads=1
```
Expected: green.

- [ ] **Step 4: Hooks**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
scripts/check-lines.sh
```

- [ ] **Step 5: Commit**

```bash
git add crates/klef-cli/src/lib.rs
git commit -m "feat(keychain): trigger banner before value-touching keychain commands"
```

---

## Task 11: User-facing docs

Create `docs/macos-keychain.md` and add a single line in `README.md` pointing to it.

**Files:**
- Create: `docs/macos-keychain.md`
- Modify: `README.md`

- [ ] **Step 1: Create `docs/macos-keychain.md`**

Create the file with:

```markdown
# macOS keychain — frequent password prompts

If klef keeps prompting you for your login password every 10–30 minutes
on macOS, the cause is the login keychain's auto-lock timeout, not klef
itself. macOS re-locks the keychain after the timeout and every klef
call to read a value (`get`, `show`, `run`, MCP `klef_run`) triggers a
re-unlock prompt.

## One-shot fix

```bash
klef keychain configure
```

This runs `security set-keychain-settings` against your default keychain
(no flags = no timeout, no lock-on-sleep). klef writes a marker at
`~/.config/klef/keychain-configured` recording your prior settings so
the post-run output shows the exact revert command.

After running, you should see no further password prompts during the
current login session. The keychain still locks at logout/reboot — your
data is no less secure at rest, only the auto-lock-during-session
behavior changes.

## Tradeoff

Disabling auto-lock means an attacker with physical access to your
unlocked Mac no longer faces a re-prompt for keychain items. They
already have your browser sessions, ssh-agent keys, etc. — so the
marginal increase in attack surface is small but non-zero. If your
threat model is "someone briefly walks up to my unlocked screen", keep
the timeout and accept the prompts.

## Opt out

To suppress the banner without applying the fix:

```bash
export KLEF_NO_KEYCHAIN_AUTOCONFIG=1
```

This only suppresses the in-context banner; running `klef keychain
configure` still works (it's an explicit user action).

## Reverting

The post-run output of `klef keychain configure` prints the precise
revert command using your prior state, e.g.:

```
security set-keychain-settings -u -t 600 -l /Users/you/Library/Keychains/login.keychain-db
```

Or you can adjust the timeout via Keychain Access.app: open it,
right-click the `login` keychain, "Change settings for keychain login…",
configure as you wish.

## Corporate Mac / MDM

If your machine is managed by an MDM (Jamf, Intune, etc.) that enforces
a non-zero keychain timeout for compliance reasons, klef's fix will get
reverted at the next sync. For these setups: do not run `klef keychain
configure`. Either accept the prompts or use the `--backend age:...`
file backend with `KLEF_PASSPHRASE` for non-interactive flows.

## What klef detects automatically

When klef is about to read or write a keychain value AND your default
keychain has auto-lock enabled, klef prints a one-time banner pointing
you at `klef keychain configure`. The banner suppresses itself after one
showing (marker file). It re-shows if your keychain state changes or if
the marker is older than 7 days.

The banner does NOT print from `klef mcp` because Claude Desktop captures
that process's stderr to log files you don't read. Pure-MCP-only users
will discover the fix via this doc or the GUI's settings panel.
```

- [ ] **Step 2: Add the README pointer**

In `README.md`, in the "Statut" section near the bottom (or wherever the existing "Plateformes supportées" bullet lives), add a new bullet:

```markdown
- **macOS users**: if you see frequent password prompts, run `klef keychain configure` once — see [`docs/macos-keychain.md`](./docs/macos-keychain.md).
```

Alternatively, place it under "Documentation" if that section exists in your current README. Match the style of nearby bullets.

- [ ] **Step 3: Commit**

```bash
git add docs/macos-keychain.md README.md
git commit -m "docs(keychain): user guide for klef keychain configure and the timeout issue"
```

---

## Task 12: Final verification

Run the full gate suite + manual smoke on a Mac with a non-zero timeout.

**Gates:**

- [ ] **Step 1: All-features test pass**

```bash
cargo test --workspace --all-features -- --test-threads=1
```
Expected: green. (`--test-threads=1` because banner tests mutate process env.)

- [ ] **Step 2: Default-features test pass**

```bash
cargo test --workspace -- --test-threads=1
```
Expected: green.

- [ ] **Step 3: Clippy + fmt + line cap**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
scripts/check-lines.sh
```
All green.

- [ ] **Step 4: Manual smoke on a real Mac with the issue**

```bash
# Backup current keychain settings
security show-keychain-info ~/Library/Keychains/login.keychain-db
# Force the issue: 10-min timeout + lock-on-sleep (you'll see prompts otherwise)
security set-keychain-settings -u -t 600 -l ~/Library/Keychains/login.keychain-db

# Build release
cargo build -p klef --release

# Trigger the banner via a value-touching command
./target/release/klef get some-existing-key  # if you have one; or `klef list` won't trigger
# Expected: banner appears on stderr exactly once.

# Run again — banner should be silent (marker present)
./target/release/klef get some-existing-key
# Expected: no banner.

# Apply the fix
./target/release/klef keychain configure
# Expected: prints "configured" + revert command.

# Verify
security show-keychain-info ~/Library/Keychains/login.keychain-db
# Expected: `no-timeout`, no `lock-on-sleep`.

# Re-run a value-touching command
./target/release/klef get some-existing-key
# Expected: no prompt, no banner.
```

- [ ] **Step 5: Final commit if anything came up**

If steps 1-4 required fixes:

```bash
git add -A
git commit -m "chore(keychain): fmt + clippy + line-cap + smoke fixes"
```

If nothing needed fixing, do NOT create an empty commit — just report.

---

## Wrap-up

After all tasks:
- The branch implements the spec end-to-end.
- macOS-only behavior gated behind `cfg`; Linux is unaffected.
- A future GUI follow-up (out of scope here) reuses `klef_core::macos_keychain` directly and adds its own UI button + marker handling.
- Open questions to revisit later: should we expose a meta field on `klef_list` MCP responses so MCP-only users get warned in chat? Tracked as a follow-up issue.
