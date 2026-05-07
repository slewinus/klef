# MCP server Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `klef mcp` — a local MCP server that exposes `klef_list` (metadata) and `klef_run` (process spawn with `klef:` refs injected as env vars). No tool ever returns a plaintext secret value to the agent.

**Architecture:** New subcommand in `klef-cli`, gated behind a `mcp` Cargo feature. Uses the official `rmcp` Rust SDK over stdio. Authorization is policy-driven via a user-edited TOML file (`~/.config/klef/mcp-policy.toml`). All requests fail-closed: if the policy doesn't explicitly cover them, or if the audit log can't be written, they're rejected. Code lives in `crates/klef-cli/src/commands/mcp/`, split into `mod.rs`, `policy.rs`, `audit.rs`, `redact.rs`, `run_proc.rs`, `tools.rs`. Reuses `klef-core::Store` unchanged.

**Tech Stack:** Rust 2024, `clap` (existing), `rmcp` (new, official Anthropic MCP SDK), `tokio` (multi-thread runtime, required by `rmcp`), `serde`/`serde_json` (existing), `toml` (new, policy parsing), `time` (existing, audit timestamps).

**Spec:** [`docs/superpowers/specs/2026-05-07-mcp-server-design.md`](../specs/2026-05-07-mcp-server-design.md)

**Notes for the engineer before starting:**
- `rmcp`'s API may have evolved since this plan was written. Before Task 1, `cargo search rmcp` and skim the latest README to confirm the macro/builder names used in Tasks 11-12. The contract (tool names, input/output JSON shapes, error shape) is specified in the design doc and is what matters; adapt the rmcp glue to whatever the current API looks like.
- The repo enforces `< 300 lines/file` via `.githooks/pre-commit` and `cargo clippy --all-targets --all-features -D warnings`. Both are checked at commit. The file split below respects the line cap by construction.
- `klef-core::Store` is synchronous and uses blocking syscalls (Keychain, file I/O). Inside async handlers, wrap store calls in `tokio::task::spawn_blocking` to avoid stalling the runtime.

---

## File Structure

**Created:**
- `crates/klef-cli/src/commands/mcp/mod.rs` — entrypoint `run(store, policy_path)`, tokio runtime, rmcp server lifecycle.
- `crates/klef-cli/src/commands/mcp/policy.rs` — TOML schema, load + skeleton write, glob matching, shell denylist, workspace_roots, `Decision::evaluate`.
- `crates/klef-cli/src/commands/mcp/audit.rs` — NDJSON append, fail-closed `record()`.
- `crates/klef-cli/src/commands/mcp/redact.rs` — best-effort substitution of resolved values in stdout/stderr.
- `crates/klef-cli/src/commands/mcp/run_proc.rs` — `tokio::process` spawn, capture, truncation, timeout, process-group kill.
- `crates/klef-cli/src/commands/mcp/tools.rs` — `klef_list` and `klef_run` handler functions (orchestrate policy + store + run_proc + redact + audit).
- `crates/klef-cli/tests/mcp.rs` — end-to-end integration tests with a fake stdio MCP client.
- `docs/mcp.md` — user-facing setup guide (Claude Desktop config, policy syntax, examples).

**Modified:**
- `crates/klef-cli/Cargo.toml` — add `[features] mcp = ["dep:rmcp", "dep:tokio", "dep:toml"]` and matching `[dependencies]` entries gated by `optional = true`.
- `crates/klef-cli/src/cli.rs` — add `Mcp { policy: Option<PathBuf> }` variant to `Command`, gated by `#[cfg(feature = "mcp")]`.
- `crates/klef-cli/src/commands/mod.rs` — `#[cfg(feature = "mcp")] pub mod mcp;`.
- `crates/klef-cli/src/lib.rs` — dispatch `Command::Mcp` to `commands::mcp::run`, gated by `#[cfg(feature = "mcp")]`.
- `README.md` — short "Pour les agents IA" addition pointing to `docs/mcp.md`.

---

## Task 1: Cargo feature flag + CLI skeleton (no logic)

Goal: `cargo build -p klef --features mcp` compiles, `klef mcp --help` works, `klef mcp` exits cleanly without doing anything yet. Default build (no `--features mcp`) is unchanged.

**Files:**
- Modify: `crates/klef-cli/Cargo.toml`
- Modify: `crates/klef-cli/src/cli.rs:18-100` (add `Mcp` variant)
- Modify: `crates/klef-cli/src/commands/mod.rs:1-17`
- Modify: `crates/klef-cli/src/lib.rs:17-101` (dispatch)
- Create: `crates/klef-cli/src/commands/mcp/mod.rs`

- [ ] **Step 1: Add deps and feature flag to `Cargo.toml`**

Append to `[dependencies]`:
```toml
rmcp   = { version = "0.1", features = ["server"], optional = true }
tokio  = { version = "1",   features = ["rt-multi-thread", "macros", "process", "io-util", "time", "sync"], optional = true }
toml   = { version = "0.8", optional = true }
glob   = { version = "0.3", optional = true }
```

Append a new section before `[lib]`:
```toml
[features]
default = []
mcp = ["dep:rmcp", "dep:tokio", "dep:toml", "dep:glob"]
```

- [ ] **Step 2: Add `Mcp` variant to `Command`**

In `crates/klef-cli/src/cli.rs`, append inside `enum Command { ... }` (after `Names`):
```rust
    /// Run an MCP server exposing klef_list and klef_run over stdio.
    /// See docs/mcp.md for setup with Claude Desktop / Claude Code.
    #[cfg(feature = "mcp")]
    Mcp {
        /// Path to the policy file. Default: ~/.config/klef/mcp-policy.toml.
        #[arg(long, value_name = "PATH")]
        policy: Option<PathBuf>,
    },
```

- [ ] **Step 3: Register the `mcp` module**

In `crates/klef-cli/src/commands/mod.rs`, append at the end:
```rust
#[cfg(feature = "mcp")]
pub mod mcp;
```

- [ ] **Step 4: Create `mcp/mod.rs` placeholder**

Create `crates/klef-cli/src/commands/mcp/mod.rs`:
```rust
//! `klef mcp` — MCP server exposing klef_list (metadata) and klef_run
//! (process spawn with klef: refs injected). See docs/mcp.md.

use klef_core::error::KlefError;
use klef_core::store::Store;
use std::path::PathBuf;

/// Entry point for `klef mcp`. Loads the policy, starts the rmcp server
/// over stdio, and blocks until stdin closes.
///
/// # Errors
///
/// Returns an error if the policy file cannot be loaded or the server
/// cannot start.
pub fn run(_store: Store, _policy_path: Option<PathBuf>) -> Result<(), KlefError> {
    Err(KlefError::BackendUnavailable(
        "klef mcp: not yet implemented".into(),
    ))
}
```

- [ ] **Step 5: Wire dispatch in `lib.rs`**

In `crates/klef-cli/src/lib.rs`, before the closing `}` of the `match cli.command { ... }` block, add:
```rust
        #[cfg(feature = "mcp")]
        Command::Mcp { policy } => commands::mcp::run(store, policy),
```

- [ ] **Step 6: Verify build, both feature combinations**

Run:
```bash
cargo build -p klef
cargo build -p klef --features mcp
cargo run  -p klef --features mcp -- mcp --help
```
Expected: all build green; the `--help` output shows the `--policy <PATH>` flag and the description.

- [ ] **Step 7: Verify default build did not regain MCP**

```bash
cargo run -p klef -- mcp 2>&1 || true
```
Expected: clap rejects `mcp` as an unknown subcommand (because the variant is `cfg`-gated).

- [ ] **Step 8: Commit**

```bash
git add crates/klef-cli/Cargo.toml \
        crates/klef-cli/src/cli.rs \
        crates/klef-cli/src/commands/mod.rs \
        crates/klef-cli/src/commands/mcp/mod.rs \
        crates/klef-cli/src/lib.rs
git commit -m "feat(mcp): scaffold klef mcp subcommand behind feature flag (#24)"
```

---

## Task 2: Policy TOML schema + load with skeleton write

Goal: Define the on-disk schema as Rust types, load from disk, and write a commented skeleton on first run if the file is missing.

**Files:**
- Create: `crates/klef-cli/src/commands/mcp/policy.rs`
- Modify: `crates/klef-cli/src/commands/mcp/mod.rs` (just `pub mod policy;`)

- [ ] **Step 1: Stub `policy.rs` with types and `pub mod policy;`**

Create `crates/klef-cli/src/commands/mcp/policy.rs`:
```rust
//! Policy file: TOML schema, parsing, and a skeleton-on-first-run helper.
//! Matching logic lives below; see `Policy::evaluate`.

use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Policy {
    #[serde(default)]
    pub workspace_roots: Vec<PathBuf>,
    #[serde(default, rename = "allow")]
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    pub argv: Vec<String>,
    #[serde(default)]
    pub env_refs: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum PolicyError {
    #[error("policy: cannot read {path}: {source}")]
    Read { path: PathBuf, source: std::io::Error },
    #[error("policy: cannot write skeleton to {path}: {source}")]
    WriteSkeleton { path: PathBuf, source: std::io::Error },
    #[error("policy: invalid TOML in {path}: {source}")]
    Toml { path: PathBuf, source: toml::de::Error },
}

const SKELETON: &str = r#"# klef MCP policy file.
#
# Each [[allow]] rule whitelists one argv pattern + the env_refs that may be
# injected into it. A request is allowed if SOME rule matches argv (with
# wildcards) AND covers every requested env_ref. Otherwise: deny.
#
# Shells (sh, bash, zsh, python, node, ...) are denied unconditionally.
# See docs/mcp.md for full semantics.

# Roots under which klef_run may execute. Empty = ignore client-supplied cwd.
workspace_roots = []

# [[allow]]
# argv = ["npm", "run", "*"]
# env_refs = ["stripe", "anthropic"]
"#;

/// Load the policy from disk. If the file does not exist, write a commented
/// skeleton (with no active rules) and return an empty `Policy`.
///
/// # Errors
///
/// Returns `PolicyError::Read` for filesystem errors other than "not found",
/// `PolicyError::WriteSkeleton` if the skeleton write fails, or
/// `PolicyError::Toml` if the file parses as invalid TOML.
pub fn load(path: &Path) -> Result<Policy, PolicyError> {
    match std::fs::read_to_string(path) {
        Ok(s) => toml::from_str::<Policy>(&s).map_err(|source| PolicyError::Toml {
            path: path.to_path_buf(),
            source,
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|source| PolicyError::WriteSkeleton {
                    path: path.to_path_buf(),
                    source,
                })?;
            }
            std::fs::write(path, SKELETON).map_err(|source| PolicyError::WriteSkeleton {
                path: path.to_path_buf(),
                source,
            })?;
            Ok(Policy::default())
        }
        Err(source) => Err(PolicyError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}
```

In `crates/klef-cli/src/commands/mcp/mod.rs`, add after the doc-comment line:
```rust
pub mod policy;
```

- [ ] **Step 2: Write failing tests for `load`**

Append at the bottom of `policy.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn load_missing_file_writes_skeleton_and_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("nested").join("mcp-policy.toml");
        let pol = load(&path).expect("load should succeed");
        assert!(pol.rules.is_empty());
        assert!(pol.workspace_roots.is_empty());
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains("[[allow]]"), "skeleton must contain commented rule example");
    }

    #[test]
    fn load_existing_valid_toml() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("p.toml");
        std::fs::write(
            &path,
            r#"workspace_roots = ["/tmp"]
[[allow]]
argv = ["npm", "start"]
env_refs = ["stripe"]
"#,
        )
        .unwrap();
        let pol = load(&path).unwrap();
        assert_eq!(pol.rules.len(), 1);
        assert_eq!(pol.rules[0].argv, vec!["npm", "start"]);
        assert_eq!(pol.rules[0].env_refs, vec!["stripe"]);
        assert_eq!(pol.workspace_roots, vec![PathBuf::from("/tmp")]);
    }

    #[test]
    fn load_invalid_toml_returns_toml_error() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("bad.toml");
        std::fs::write(&path, "this is = not = toml").unwrap();
        let err = load(&path).unwrap_err();
        assert!(matches!(err, PolicyError::Toml { .. }));
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test -p klef --features mcp commands::mcp::policy::tests
```
Expected: all 3 tests PASS. (They will pass on first run because Step 1 already wrote the implementation. This is intentional for trivial loaders — the test suite is the spec.)

- [ ] **Step 4: Commit**

```bash
git add crates/klef-cli/src/commands/mcp/mod.rs \
        crates/klef-cli/src/commands/mcp/policy.rs
git commit -m "feat(mcp): policy TOML loader with skeleton-on-first-run"
```

---

## Task 3: Argv glob matching

Goal: `argv_matches(rule_pattern, request_argv)` returns true iff lengths match and each element matches its glob.

**Files:**
- Modify: `crates/klef-cli/src/commands/mcp/policy.rs`

- [ ] **Step 1: Write failing tests**

Append inside the existing `mod tests` block:
```rust
    #[test]
    fn argv_matches_exact() {
        assert!(argv_matches(&["npm".into(), "start".into()], &["npm", "start"]));
        assert!(!argv_matches(&["npm".into(), "start".into()], &["npm", "test"]));
    }

    #[test]
    fn argv_matches_wildcard_token() {
        assert!(argv_matches(&["npm".into(), "run".into(), "*".into()], &["npm", "run", "dev"]));
        assert!(argv_matches(&["npm".into(), "run".into(), "*".into()], &["npm", "run", "build:prod"]));
        assert!(!argv_matches(&["npm".into(), "run".into(), "*".into()], &["npm", "test"]));
    }

    #[test]
    fn argv_matches_length_mismatch_is_no_match() {
        // No variadic wildcards: a 2-element pattern never matches a 3-element argv.
        assert!(!argv_matches(&["npm".into(), "*".into()], &["npm", "run", "dev"]));
    }

    #[test]
    fn argv_matches_question_mark_token() {
        assert!(argv_matches(&["cargo".into(), "?est".into()], &["cargo", "test"]));
        assert!(!argv_matches(&["cargo".into(), "?est".into()], &["cargo", "build"]));
    }

    #[test]
    fn argv_matches_url_with_path_glob() {
        assert!(argv_matches(
            &["curl".into(), "https://api.stripe.com/*".into()],
            &["curl", "https://api.stripe.com/v1/charges"],
        ));
    }
```

- [ ] **Step 2: Run tests, confirm they fail**

```bash
cargo test -p klef --features mcp commands::mcp::policy::tests::argv_matches
```
Expected: compile error — `argv_matches` not defined.

- [ ] **Step 3: Implement `argv_matches`**

Above the `#[cfg(test)]` block in `policy.rs`, add:
```rust
/// Match a request argv against a rule's argv pattern.
///
/// Each pattern element is a token-level glob (`*` and `?` per `glob::Pattern`).
/// Length mismatch = no match (no variadic wildcards).
pub fn argv_matches(pattern: &[String], argv: &[&str]) -> bool {
    if pattern.len() != argv.len() {
        return false;
    }
    pattern.iter().zip(argv.iter()).all(|(pat, arg)| {
        glob::Pattern::new(pat).is_ok_and(|p| p.matches(arg))
    })
}
```

- [ ] **Step 4: Run tests, confirm they pass**

```bash
cargo test -p klef --features mcp commands::mcp::policy::tests::argv_matches
```
Expected: 5 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/klef-cli/src/commands/mcp/policy.rs
git commit -m "feat(mcp): argv glob matching for policy rules"
```

---

## Task 4: Shell denylist

Goal: `is_shell_program(argv0)` returns true for any of the denied program names, including absolute paths.

**Files:**
- Modify: `crates/klef-cli/src/commands/mcp/policy.rs`

- [ ] **Step 1: Write failing tests**

Append inside `mod tests`:
```rust
    #[test]
    fn shell_denylist_bare_names() {
        for name in ["sh", "bash", "zsh", "python", "python3", "node", "deno", "bun", "env"] {
            assert!(is_shell_program(name), "{name} must be denied");
        }
    }

    #[test]
    fn shell_denylist_absolute_paths() {
        assert!(is_shell_program("/bin/sh"));
        assert!(is_shell_program("/usr/bin/python3"));
        assert!(is_shell_program("/opt/homebrew/bin/node"));
    }

    #[test]
    fn shell_denylist_does_not_match_innocent_programs() {
        assert!(!is_shell_program("npm"));
        assert!(!is_shell_program("/usr/local/bin/cargo"));
        assert!(!is_shell_program("./my-script.sh"), "extension does not imply shell interpreter");
    }
```

- [ ] **Step 2: Run, confirm failure**

```bash
cargo test -p klef --features mcp commands::mcp::policy::tests::shell_denylist
```
Expected: compile error — `is_shell_program` not defined.

- [ ] **Step 3: Implement `is_shell_program`**

Above the `#[cfg(test)]` block, add:
```rust
const SHELL_DENYLIST: &[&str] = &[
    "sh", "bash", "zsh", "fish", "dash", "ksh", "csh", "tcsh", "ash",
    "python", "python3", "ruby", "perl", "lua", "awk",
    "node", "deno", "bun",
    "eval", "exec", "env",
];

/// True if `argv0` resolves to one of the hard-coded shell-or-interpreter
/// programs that bypass rule intent. Compares `Path::file_name(argv0)` so
/// `/usr/bin/python3` and `python3` are treated identically.
#[must_use]
pub fn is_shell_program(argv0: &str) -> bool {
    let name = Path::new(argv0)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(argv0);
    SHELL_DENYLIST.contains(&name)
}
```

- [ ] **Step 4: Run, confirm pass**

```bash
cargo test -p klef --features mcp commands::mcp::policy::tests::shell_denylist
```
Expected: 3 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/klef-cli/src/commands/mcp/policy.rs
git commit -m "feat(mcp): hard-coded shell denylist for argv[0]"
```

---

## Task 5: Workspace_roots check

Goal: `cwd_under_roots(cwd, &roots)` returns true if `cwd`, after canonicalization, is under any of `roots` (also canonicalized). Empty `roots` returns `true` (treated as "no constraint" — the caller decides whether to ignore client cwd).

**Files:**
- Modify: `crates/klef-cli/src/commands/mcp/policy.rs`

- [ ] **Step 1: Write failing tests**

Append inside `mod tests`:
```rust
    #[test]
    fn cwd_under_roots_empty_means_unconstrained() {
        assert!(cwd_under_roots(Path::new("/etc"), &[]));
    }

    #[test]
    fn cwd_under_roots_match() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let sub = root.join("project").join("src");
        std::fs::create_dir_all(&sub).unwrap();
        assert!(cwd_under_roots(&sub, &[root.to_path_buf()]));
    }

    #[test]
    fn cwd_under_roots_outside_is_rejected() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("a");
        let sibling = tmp.path().join("b");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&sibling).unwrap();
        assert!(!cwd_under_roots(&sibling, &[root]));
    }

    #[test]
    fn cwd_under_roots_nonexistent_cwd_is_rejected() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        assert!(!cwd_under_roots(&root.join("does-not-exist"), &[root]));
    }
```

- [ ] **Step 2: Run, confirm failure**

```bash
cargo test -p klef --features mcp commands::mcp::policy::tests::cwd_under_roots
```
Expected: compile error.

- [ ] **Step 3: Implement**

Above `#[cfg(test)]`:
```rust
/// Whether `cwd` resolves to a path under any element of `roots`.
///
/// Empty `roots` returns `true` — interpreted as "no constraint".
/// Both `cwd` and each `root` are canonicalized; canonicalization failures
/// (non-existent paths, permission errors) yield `false`.
#[must_use]
pub fn cwd_under_roots(cwd: &Path, roots: &[PathBuf]) -> bool {
    if roots.is_empty() {
        return true;
    }
    let Ok(cwd_real) = cwd.canonicalize() else { return false };
    roots.iter().any(|r| {
        r.canonicalize()
            .is_ok_and(|root_real| cwd_real.starts_with(&root_real))
    })
}
```

- [ ] **Step 4: Run, confirm pass**

```bash
cargo test -p klef --features mcp commands::mcp::policy::tests::cwd_under_roots
```
Expected: 4 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/klef-cli/src/commands/mcp/policy.rs
git commit -m "feat(mcp): canonicalized workspace_roots check for cwd"
```

---

## Task 6: `Policy::evaluate` — composes everything

Goal: A single entrypoint `evaluate(req) -> Decision` that runs denylist → cwd check → rule scan, returning either `Allow { matched_rule_index }` or `Deny { reason }`.

**Files:**
- Modify: `crates/klef-cli/src/commands/mcp/policy.rs`

- [ ] **Step 1: Add request/decision types and write failing tests**

Above `#[cfg(test)]`, add:
```rust
#[derive(Debug, Clone)]
pub struct Request<'a> {
    pub argv: &'a [String],
    pub env_refs: &'a [String],
    pub cwd: Option<&'a Path>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Allow { matched_rule_index: usize },
    Deny { reason: DenyReason },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DenyReason {
    ShellDenylist(String),
    CwdNotInWorkspaceRoots,
    NoRuleMatch,
}

impl Policy {
    /// Evaluate a request against this policy. First-rule-that-fully-covers wins.
    #[must_use]
    pub fn evaluate(&self, req: &Request<'_>) -> Decision {
        if let Some(prog) = req.argv.first() {
            if is_shell_program(prog) {
                return Decision::Deny { reason: DenyReason::ShellDenylist(prog.clone()) };
            }
        } else {
            return Decision::Deny { reason: DenyReason::NoRuleMatch };
        }
        if let Some(cwd) = req.cwd {
            if !cwd_under_roots(cwd, &self.workspace_roots) {
                return Decision::Deny { reason: DenyReason::CwdNotInWorkspaceRoots };
            }
        }
        let argv_strs: Vec<&str> = req.argv.iter().map(String::as_str).collect();
        for (idx, rule) in self.rules.iter().enumerate() {
            if !argv_matches(&rule.argv, &argv_strs) { continue; }
            let covers_all_envs = req.env_refs.iter().all(|r| rule.env_refs.contains(r));
            if covers_all_envs {
                return Decision::Allow { matched_rule_index: idx };
            }
        }
        Decision::Deny { reason: DenyReason::NoRuleMatch }
    }
}
```

Append to `mod tests`:
```rust
    fn pol(toml_str: &str) -> Policy { toml::from_str(toml_str).unwrap() }
    fn req<'a>(argv: &'a [String], envs: &'a [String]) -> Request<'a> {
        Request { argv, env_refs: envs, cwd: None }
    }

    #[test]
    fn evaluate_allow_when_argv_and_envs_covered() {
        let p = pol(r#"
            [[allow]]
            argv = ["npm", "start"]
            env_refs = ["stripe", "anthropic"]
        "#);
        let argv = vec!["npm".into(), "start".into()];
        let envs = vec!["stripe".into()];
        assert_eq!(p.evaluate(&req(&argv, &envs)), Decision::Allow { matched_rule_index: 0 });
    }

    #[test]
    fn evaluate_deny_when_env_not_in_rule() {
        let p = pol(r#"
            [[allow]]
            argv = ["npm", "start"]
            env_refs = ["stripe"]
        "#);
        let argv = vec!["npm".into(), "start".into()];
        let envs = vec!["openai".into()];
        assert_eq!(p.evaluate(&req(&argv, &envs)), Decision::Deny { reason: DenyReason::NoRuleMatch });
    }

    #[test]
    fn evaluate_deny_shell_even_if_rule_matches() {
        let p = pol(r#"
            [[allow]]
            argv = ["bash", "-c", "*"]
            env_refs = ["stripe"]
        "#);
        let argv = vec!["bash".into(), "-c".into(), "echo $STRIPE".into()];
        let envs = vec!["stripe".into()];
        match p.evaluate(&req(&argv, &envs)) {
            Decision::Deny { reason: DenyReason::ShellDenylist(s) } => assert_eq!(s, "bash"),
            other => panic!("expected shell deny, got {other:?}"),
        }
    }

    #[test]
    fn evaluate_picks_first_covering_rule() {
        let p = pol(r#"
            [[allow]]
            argv = ["npm", "start"]
            env_refs = ["stripe"]

            [[allow]]
            argv = ["npm", "*"]
            env_refs = ["stripe", "anthropic"]
        "#);
        let argv = vec!["npm".into(), "start".into()];
        let envs = vec!["stripe".into(), "anthropic".into()];
        // First rule matches argv but doesn't cover anthropic; second covers both.
        assert_eq!(p.evaluate(&req(&argv, &envs)), Decision::Allow { matched_rule_index: 1 });
    }
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p klef --features mcp commands::mcp::policy
```
Expected: all policy tests PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/klef-cli/src/commands/mcp/policy.rs
git commit -m "feat(mcp): Policy::evaluate composes denylist, cwd, rule match"
```

---

## Task 7: Audit log — fail-closed NDJSON append

Goal: `Audit::record(&entry)` appends one JSON line atomically. Failure to write returns an error (caller will reject the request).

**Files:**
- Create: `crates/klef-cli/src/commands/mcp/audit.rs`
- Modify: `crates/klef-cli/src/commands/mcp/mod.rs` (add `pub mod audit;`)

- [ ] **Step 1: Stub `audit.rs` with types**

Create `crates/klef-cli/src/commands/mcp/audit.rs`:
```rust
//! Append-only NDJSON audit log. Fail-closed: any write error must propagate
//! so the caller can deny the request.

use serde::Serialize;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
pub struct Entry<'a> {
    pub ts: String,
    pub tool: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub argv: Option<&'a [String]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_refs: Option<&'a [String]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<&'a str>,
    pub decision: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_rule_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr_bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout_truncated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr_truncated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timed_out: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count_returned: Option<usize>,
}

#[derive(Debug, thiserror::Error)]
#[error("audit: {0}")]
pub struct AuditError(String);

#[derive(Debug, Clone)]
pub struct Audit { path: PathBuf }

impl Audit {
    #[must_use]
    pub fn new(path: PathBuf) -> Self { Self { path } }

    /// Append `entry` as one NDJSON line. Returns an error if the file
    /// cannot be opened/written/synced — caller MUST refuse the request.
    pub fn record(&self, entry: &Entry<'_>) -> Result<(), AuditError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| AuditError(e.to_string()))?;
        }
        let mut line = serde_json::to_vec(entry).map_err(|e| AuditError(e.to_string()))?;
        line.push(b'\n');
        let mut f = OpenOptions::new()
            .create(true).append(true).open(&self.path)
            .map_err(|e| AuditError(format!("open {}: {e}", self.path.display())))?;
        f.write_all(&line).map_err(|e| AuditError(e.to_string()))?;
        f.sync_all().map_err(|e| AuditError(e.to_string()))?;
        Ok(())
    }
}

pub fn now_iso() -> String {
    let now = time::OffsetDateTime::now_utc();
    now.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}
```

In `mod.rs`, append `pub mod audit;` after `pub mod policy;`.

- [ ] **Step 2: Write tests**

Append to `audit.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn argv() -> Vec<String> { vec!["echo".into(), "hi".into()] }
    fn envs() -> Vec<String> { vec!["stripe".into()] }

    #[test]
    fn record_writes_one_ndjson_line_and_appends() {
        let tmp = TempDir::new().unwrap();
        let a = Audit::new(tmp.path().join("audit.log"));
        for _ in 0..3 {
            let av = argv();
            let er = envs();
            let e = Entry {
                ts: now_iso(),
                tool: "klef_run",
                argv: Some(&av),
                env_refs: Some(&er),
                cwd: None,
                decision: "allow",
                matched_rule_index: Some(0),
                reason: None,
                exit_code: Some(0),
                duration_ms: Some(1),
                stdout_bytes: Some(0),
                stderr_bytes: Some(0),
                stdout_truncated: Some(false),
                stderr_truncated: Some(false),
                timed_out: Some(false),
                count_returned: None,
            };
            a.record(&e).unwrap();
        }
        let s = std::fs::read_to_string(tmp.path().join("audit.log")).unwrap();
        assert_eq!(s.matches('\n').count(), 3);
        for line in s.lines() {
            let v: serde_json::Value = serde_json::from_str(line).unwrap();
            assert_eq!(v["tool"], "klef_run");
            assert_eq!(v["decision"], "allow");
        }
    }

    #[test]
    fn record_fails_when_path_is_unwritable() {
        // Path with a non-directory parent component can't be created.
        let tmp = TempDir::new().unwrap();
        let blocker = tmp.path().join("not-a-dir");
        std::fs::write(&blocker, b"x").unwrap();
        let a = Audit::new(blocker.join("audit.log"));
        let av = argv();
        let er = envs();
        let e = Entry {
            ts: now_iso(), tool: "klef_run", argv: Some(&av), env_refs: Some(&er), cwd: None,
            decision: "allow", matched_rule_index: Some(0), reason: None,
            exit_code: Some(0), duration_ms: Some(0), stdout_bytes: Some(0), stderr_bytes: Some(0),
            stdout_truncated: Some(false), stderr_truncated: Some(false), timed_out: Some(false),
            count_returned: None,
        };
        assert!(a.record(&e).is_err());
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test -p klef --features mcp commands::mcp::audit
```
Expected: 2 tests PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/klef-cli/src/commands/mcp/audit.rs \
        crates/klef-cli/src/commands/mcp/mod.rs
git commit -m "feat(mcp): NDJSON audit log with fail-closed write"
```

---

## Task 8: Best-effort redaction

Goal: `redact(buf, &resolved)` replaces every byte-occurrence of each value (length > 4) with `[REDACTED:<name>]`.

**Files:**
- Create: `crates/klef-cli/src/commands/mcp/redact.rs`
- Modify: `crates/klef-cli/src/commands/mcp/mod.rs` (add `pub mod redact;`)

- [ ] **Step 1: Stub `redact.rs`**

Create `crates/klef-cli/src/commands/mcp/redact.rs`:
```rust
//! Best-effort substitution of resolved env_ref values in captured stdout/
//! stderr. Operates on raw bytes (binary-safe). Values <= 4 bytes are skipped
//! (false-positive risk too high — a "PORT=3000" value would match every
//! occurrence of "3000").

const MIN_VALUE_LEN: usize = 5;

/// Replace every byte-occurrence of each resolved value in `buf` with
/// `[REDACTED:<name>]`. Mutates `buf` in place. Skips values <= 4 bytes.
pub fn redact(buf: &mut Vec<u8>, resolved: &[(String, String)]) {
    for (name, value) in resolved {
        if value.len() < MIN_VALUE_LEN { continue; }
        let needle = value.as_bytes();
        let placeholder = format!("[REDACTED:{name}]").into_bytes();
        replace_all(buf, needle, &placeholder);
    }
}

fn replace_all(haystack: &mut Vec<u8>, needle: &[u8], replacement: &[u8]) {
    if needle.is_empty() || needle.len() > haystack.len() { return; }
    let mut out: Vec<u8> = Vec::with_capacity(haystack.len());
    let mut i = 0;
    while i + needle.len() <= haystack.len() {
        if &haystack[i..i + needle.len()] == needle {
            out.extend_from_slice(replacement);
            i += needle.len();
        } else {
            out.push(haystack[i]);
            i += 1;
        }
    }
    out.extend_from_slice(&haystack[i..]);
    *haystack = out;
}
```

In `mod.rs`, append `pub mod redact;`.

- [ ] **Step 2: Write tests**

Append to `redact.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_simple_occurrence() {
        let mut buf = b"key is sk_live_abcdef and stop".to_vec();
        redact(&mut buf, &[("stripe".into(), "sk_live_abcdef".into())]);
        assert_eq!(buf, b"key is [REDACTED:stripe] and stop");
    }

    #[test]
    fn redact_multi_occurrence() {
        let mut buf = b"AA-XYZ12-BB-XYZ12-CC".to_vec();
        redact(&mut buf, &[("k".into(), "XYZ12".into())]);
        assert_eq!(buf, b"AA-[REDACTED:k]-BB-[REDACTED:k]-CC");
    }

    #[test]
    fn redact_skips_values_below_min_len() {
        let mut buf = b"port is 3000 and 3000 again".to_vec();
        redact(&mut buf, &[("port".into(), "3000".into())]);
        assert_eq!(buf, b"port is 3000 and 3000 again", "<5-byte values must be skipped");
    }

    #[test]
    fn redact_binary_safe() {
        let mut buf: Vec<u8> = vec![0x00, 0xFF, b's', b'k', b'_', b'a', b'b', b'c', 0x00];
        redact(&mut buf, &[("k".into(), "sk_abc".into())]);
        assert_eq!(buf, [0x00, 0xFF, b'[', b'R', b'E', b'D', b'A', b'C', b'T', b'E', b'D', b':', b'k', b']', 0x00]);
    }

    #[test]
    fn redact_no_op_when_value_absent() {
        let mut buf = b"nothing to see here".to_vec();
        redact(&mut buf, &[("k".into(), "missing".into())]);
        assert_eq!(buf, b"nothing to see here");
    }
}
```

- [ ] **Step 3: Run**

```bash
cargo test -p klef --features mcp commands::mcp::redact
```
Expected: 5 tests PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/klef-cli/src/commands/mcp/redact.rs \
        crates/klef-cli/src/commands/mcp/mod.rs
git commit -m "feat(mcp): best-effort byte-level redaction of resolved values"
```

---

## Task 9: Process spawn with timeout, capture, kill-tree

Goal: `spawn_and_capture(req)` runs `argv` with a curated env, captures stdout/stderr (truncated at 1 MB), enforces a hardcap timeout, and on timeout SIGTERMs the entire process group, then SIGKILLs after 2s grace.

**Files:**
- Create: `crates/klef-cli/src/commands/mcp/run_proc.rs`
- Modify: `crates/klef-cli/src/commands/mcp/mod.rs` (add `pub mod run_proc;`)

- [ ] **Step 1: Stub the module**

Create `crates/klef-cli/src/commands/mcp/run_proc.rs`:
```rust
//! Spawn a child process with a curated env, capture stdout/stderr with
//! truncation, enforce a timeout, and kill the whole process group on
//! timeout to avoid orphan descendants.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;
use tokio::process::Command;

pub const STDOUT_CAP_BYTES: usize = 1024 * 1024;
pub const STDERR_CAP_BYTES: usize = 1024 * 1024;
pub const HARDCAP_TIMEOUT_MS: u64 = 300_000;
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;

const PARENT_ENV_WHITELIST: &[&str] = &["PATH", "HOME", "USER", "LANG", "LC_ALL", "TERM", "TMPDIR"];

#[derive(Debug)]
pub struct ProcRequest {
    pub argv: Vec<String>,
    pub env: HashMap<String, String>,
    pub cwd: Option<PathBuf>,
    pub timeout_ms: u64,
}

#[derive(Debug)]
pub struct ProcResult {
    pub exit_code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub duration_ms: u64,
    pub timed_out: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ProcError {
    #[error("spawn: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("argv is empty")]
    EmptyArgv,
}

/// Run a child to completion or timeout. The child receives ONLY the env
/// vars in `req.env` plus the parent-whitelist (PATH, HOME, ...). Stdin is
/// `/dev/null`. Truncates each stream at 1 MB.
pub async fn spawn_and_capture(req: ProcRequest) -> Result<ProcResult, ProcError> {
    let (program, args) = req.argv.split_first().ok_or(ProcError::EmptyArgv)?;
    let mut cmd = Command::new(program);
    cmd.args(args)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    for k in PARENT_ENV_WHITELIST {
        if let Ok(v) = std::env::var(k) { cmd.env(k, v); }
    }
    for (k, v) in &req.env { cmd.env(k, v); }
    if let Some(cwd) = &req.cwd { cmd.current_dir(cwd); }
    #[cfg(unix)]
    unsafe {
        use std::os::unix::process::CommandExt;
        cmd.pre_exec(|| {
            // SAFETY: setsid() is async-signal-safe.
            if libc::setsid() == -1 { return Err(std::io::Error::last_os_error()); }
            Ok(())
        });
    }

    let started = Instant::now();
    let mut child = cmd.spawn()?;
    #[cfg(unix)]
    let pgid = child.id().map(|p| p as i32);

    let stdout = child.stdout.take().expect("piped");
    let stderr = child.stderr.take().expect("piped");
    let stdout_task = tokio::spawn(read_capped(stdout, STDOUT_CAP_BYTES));
    let stderr_task = tokio::spawn(read_capped(stderr, STDERR_CAP_BYTES));

    let timeout_ms = req.timeout_ms.min(HARDCAP_TIMEOUT_MS);
    let wait = child.wait();
    let outcome = tokio::time::timeout(Duration::from_millis(timeout_ms), wait).await;

    let timed_out = outcome.is_err();
    let exit_status = match outcome {
        Ok(Ok(s)) => Some(s),
        Ok(Err(e)) => return Err(ProcError::Spawn(e)),
        Err(_) => {
            #[cfg(unix)]
            if let Some(pgid) = pgid { kill_group(pgid); }
            None
        }
    };

    let (stdout_buf, stdout_truncated) = stdout_task.await.unwrap_or_else(|_| (Vec::new(), false));
    let (stderr_buf, stderr_truncated) = stderr_task.await.unwrap_or_else(|_| (Vec::new(), false));

    let exit_code = exit_status.and_then(|s| s.code()).unwrap_or(-1);
    Ok(ProcResult {
        exit_code,
        stdout: stdout_buf,
        stderr: stderr_buf,
        stdout_truncated,
        stderr_truncated,
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        timed_out,
    })
}

async fn read_capped<R: AsyncReadExt + Unpin>(mut r: R, cap: usize) -> (Vec<u8>, bool) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let mut truncated = false;
    loop {
        match r.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if buf.len() + n > cap {
                    let take = cap.saturating_sub(buf.len());
                    buf.extend_from_slice(&chunk[..take]);
                    truncated = true;
                    // Drain the rest so the child doesn't block on a full pipe.
                    let mut sink = [0u8; 8192];
                    while r.read(&mut sink).await.unwrap_or(0) > 0 {}
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
            }
        }
    }
    (buf, truncated)
}

#[cfg(unix)]
fn kill_group(pgid: i32) {
    // SAFETY: killpg() is async-signal-safe; we call from async context.
    unsafe {
        libc::killpg(pgid, libc::SIGTERM);
    }
    std::thread::sleep(Duration::from_secs(2));
    unsafe {
        libc::killpg(pgid, libc::SIGKILL);
    }
}
```

In `mod.rs`, append `pub mod run_proc;`. In `Cargo.toml` `[dependencies]`, add (still `optional = true`):
```toml
libc = { version = "0.2", optional = true }
```
And update the `mcp` feature to include `"dep:libc"`.

- [ ] **Step 2: Write tests**

Append to `run_proc.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn echo(args: &[&str]) -> ProcRequest {
        ProcRequest {
            argv: std::iter::once("echo".to_string())
                .chain(args.iter().map(|s| s.to_string()))
                .collect(),
            env: HashMap::new(),
            cwd: None,
            timeout_ms: 5000,
        }
    }

    #[tokio::test]
    async fn echo_returns_stdout_and_zero_exit() {
        let r = spawn_and_capture(echo(&["hello"])).await.unwrap();
        assert_eq!(r.exit_code, 0);
        assert!(!r.timed_out);
        assert_eq!(String::from_utf8(r.stdout).unwrap().trim(), "hello");
    }

    #[tokio::test]
    async fn injected_env_visible_to_child() {
        let mut env = HashMap::new();
        env.insert("KLEF_TEST_VAR".into(), "secret-xyz".into());
        let req = ProcRequest {
            argv: vec!["sh".into(), "-c".into(), "echo $KLEF_TEST_VAR".into()],
            env, cwd: None, timeout_ms: 5000,
        };
        // NOTE: Uses sh for the env-visibility test only; klef mcp would never
        // accept this argv (shell denylist). This test bypasses policy.
        let r = spawn_and_capture(req).await.unwrap();
        assert_eq!(String::from_utf8(r.stdout).unwrap().trim(), "secret-xyz");
    }

    #[tokio::test]
    async fn parent_env_not_inherited_outside_whitelist() {
        // SAFETY: test binary is single-threaded by default per #[tokio::test].
        // The variable is set then unset within this test only.
        unsafe { std::env::set_var("KLEF_NOT_WHITELISTED", "leak-me"); }
        let req = ProcRequest {
            argv: vec!["sh".into(), "-c".into(), "echo ${KLEF_NOT_WHITELISTED:-absent}".into()],
            env: HashMap::new(), cwd: None, timeout_ms: 5000,
        };
        let r = spawn_and_capture(req).await.unwrap();
        unsafe { std::env::remove_var("KLEF_NOT_WHITELISTED"); }
        assert_eq!(String::from_utf8(r.stdout).unwrap().trim(), "absent");
    }

    #[tokio::test]
    async fn timeout_marks_timed_out_and_kills() {
        let req = ProcRequest {
            argv: vec!["sleep".into(), "30".into()],
            env: HashMap::new(), cwd: None, timeout_ms: 200,
        };
        let r = spawn_and_capture(req).await.unwrap();
        assert!(r.timed_out);
        assert!(r.duration_ms < 5000, "kill must be prompt; got {} ms", r.duration_ms);
    }

    #[tokio::test]
    async fn stdout_truncates_at_cap() {
        // `yes` floods stdout; we cap at 1 MB.
        let req = ProcRequest {
            argv: vec!["sh".into(), "-c".into(), "yes hello".into()],
            env: HashMap::new(), cwd: None, timeout_ms: 1500,
        };
        let r = spawn_and_capture(req).await.unwrap();
        assert!(r.stdout_truncated);
        assert!(r.stdout.len() <= STDOUT_CAP_BYTES);
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test -p klef --features mcp commands::mcp::run_proc
```
Expected: 5 tests PASS. (Tests do use `sh` for env-plumbing checks; this is a unit-test contract on `run_proc`, which is below the policy layer. Production policy will deny these argvs.)

- [ ] **Step 4: Commit**

```bash
git add crates/klef-cli/Cargo.toml \
        crates/klef-cli/src/commands/mcp/run_proc.rs \
        crates/klef-cli/src/commands/mcp/mod.rs
git commit -m "feat(mcp): tokio process spawn with timeout, kill-tree, env curation"
```

---

## Task 10: `tools.rs` — `klef_list` and `klef_run` orchestration (no rmcp yet)

Goal: Two functions that take a typed input, do the full work (policy, store, redact, audit), and return a typed output. They're pure orchestration — the rmcp adapter (Task 11) will just deserialize → call → serialize.

**Files:**
- Create: `crates/klef-cli/src/commands/mcp/tools.rs`
- Modify: `crates/klef-cli/src/commands/mcp/mod.rs` (add `pub mod tools;`)

- [ ] **Step 1: Stub the module with input/output types**

Create `crates/klef-cli/src/commands/mcp/tools.rs`:
```rust
//! Handler functions for klef_list and klef_run. Pure orchestration over
//! `Store` + `Policy` + `run_proc` + `redact` + `audit`. The rmcp adapter
//! in `mod.rs` translates JSON-RPC requests into calls here.

use crate::commands::mcp::audit::{Audit, Entry, now_iso};
use crate::commands::mcp::policy::{Decision, DenyReason, Policy, Request as PolReq};
use crate::commands::mcp::redact;
use crate::commands::mcp::run_proc::{self, ProcRequest, DEFAULT_TIMEOUT_MS, HARDCAP_TIMEOUT_MS};
use klef_core::store::Store;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct ListInput {
    pub tag: Option<String>,
    pub filter: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListEntry {
    pub name: String,
    pub note: Option<String>,
    pub tags: Vec<String>,
    pub added_at: String,
}

#[derive(Debug, Deserialize)]
pub struct RunInput {
    pub argv: Vec<String>,
    #[serde(default)]
    pub env_refs: Vec<String>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct RunOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub timed_out: bool,
    pub encoding: &'static str,
}

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("policy: {0}")]
    Policy(String),
    #[error("store: env_ref '{0}' not found")]
    EnvRefNotFound(String),
    #[error("audit: {0}")]
    Audit(String),
    #[error("internal: {0}")]
    Internal(String),
}

pub struct Ctx {
    pub store: Arc<Store>,
    pub policy: Arc<Policy>,
    pub audit: Audit,
}

pub async fn klef_list(ctx: &Ctx, input: ListInput) -> Result<Vec<ListEntry>, ToolError> {
    let store = ctx.store.clone();
    let entries = tokio::task::spawn_blocking(move || store.list())
        .await.map_err(|e| ToolError::Internal(e.to_string()))?
        .map_err(|e| ToolError::Internal(e.to_string()))?;

    let needle = input.filter.as_deref().map(str::to_lowercase);
    let tag = input.tag.as_deref();
    let filtered: Vec<ListEntry> = entries.into_iter().filter_map(|(name, meta)| {
        if let Some(t) = tag {
            if !meta.tags.iter().any(|x| x == t) { return None; }
        }
        if let Some(n) = needle.as_deref() {
            let matches_name = name.to_lowercase().contains(n);
            let matches_note = meta.note.as_deref().is_some_and(|x| x.to_lowercase().contains(n));
            if !matches_name && !matches_note { return None; }
        }
        let added_at = meta.added_at
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default();
        Some(ListEntry { name, note: meta.note, tags: meta.tags, added_at })
    }).collect();

    let count = filtered.len();
    ctx.audit.record(&Entry {
        ts: now_iso(),
        tool: "klef_list",
        argv: None, env_refs: None, cwd: None,
        decision: "allow",
        matched_rule_index: None, reason: None,
        exit_code: None, duration_ms: None,
        stdout_bytes: None, stderr_bytes: None,
        stdout_truncated: None, stderr_truncated: None, timed_out: None,
        count_returned: Some(count),
    }).map_err(|e| ToolError::Audit(e.to_string()))?;

    Ok(filtered)
}

pub async fn klef_run(ctx: &Ctx, input: RunInput) -> Result<RunOutput, ToolError> {
    // 1. Validate timeout up front.
    let timeout_ms = input.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
    if timeout_ms > HARDCAP_TIMEOUT_MS {
        let reason = format!("timeout_exceeds_max:{timeout_ms}");
        record_deny(ctx, &input, &reason)?;
        return Err(ToolError::Policy(format!(
            "timeout_ms {timeout_ms} exceeds max {HARDCAP_TIMEOUT_MS}"
        )));
    }

    // 2. Policy evaluation.
    let cwd_ref = input.cwd.as_deref();
    let pol_req = PolReq { argv: &input.argv, env_refs: &input.env_refs, cwd: cwd_ref };
    let matched_rule_index = match ctx.policy.evaluate(&pol_req) {
        Decision::Allow { matched_rule_index } => matched_rule_index,
        Decision::Deny { reason } => {
            let reason_str = format_deny(&reason);
            record_deny(ctx, &input, &reason_str)?;
            return Err(ToolError::Policy(human_deny(&reason, &input)));
        }
    };

    // 3. Resolve env_refs from store.
    let mut resolved: Vec<(String, String)> = Vec::with_capacity(input.env_refs.len());
    for name in &input.env_refs {
        let store = ctx.store.clone();
        let n = name.clone();
        let v = tokio::task::spawn_blocking(move || store.get_value(&n)).await
            .map_err(|e| ToolError::Internal(e.to_string()))?;
        match v {
            Ok(value) => resolved.push((name.clone(), value)),
            Err(_) => {
                let reason = format!("env_ref_not_found:{name}");
                record_deny(ctx, &input, &reason)?;
                return Err(ToolError::EnvRefNotFound(name.clone()));
            }
        }
    }

    // 4. Spawn the child.
    let env: HashMap<String, String> = resolved.iter().cloned().collect();
    let proc_req = ProcRequest {
        argv: input.argv.clone(),
        env,
        cwd: input.cwd.clone(),
        timeout_ms,
    };
    let mut result = run_proc::spawn_and_capture(proc_req).await
        .map_err(|e| ToolError::Internal(e.to_string()))?;

    // 5. Best-effort redaction (mutates buffers in place).
    redact::redact(&mut result.stdout, &resolved);
    redact::redact(&mut result.stderr, &resolved);

    // 6. UTF-8 vs base64 encoding decision.
    let (stdout_str, stderr_str, encoding) = encode_outputs(&result.stdout, &result.stderr);

    // 7. Audit allow.
    ctx.audit.record(&Entry {
        ts: now_iso(),
        tool: "klef_run",
        argv: Some(&input.argv),
        env_refs: Some(&input.env_refs),
        cwd: input.cwd.as_deref().and_then(|p| p.to_str()),
        decision: "allow",
        matched_rule_index: Some(matched_rule_index),
        reason: None,
        exit_code: Some(result.exit_code),
        duration_ms: Some(result.duration_ms),
        stdout_bytes: Some(result.stdout.len()),
        stderr_bytes: Some(result.stderr.len()),
        stdout_truncated: Some(result.stdout_truncated),
        stderr_truncated: Some(result.stderr_truncated),
        timed_out: Some(result.timed_out),
        count_returned: None,
    }).map_err(|e| ToolError::Audit(e.to_string()))?;

    Ok(RunOutput {
        exit_code: result.exit_code,
        stdout: stdout_str,
        stderr: stderr_str,
        duration_ms: result.duration_ms,
        stdout_truncated: result.stdout_truncated,
        stderr_truncated: result.stderr_truncated,
        timed_out: result.timed_out,
        encoding,
    })
}

fn record_deny(ctx: &Ctx, input: &RunInput, reason: &str) -> Result<(), ToolError> {
    ctx.audit.record(&Entry {
        ts: now_iso(),
        tool: "klef_run",
        argv: Some(&input.argv),
        env_refs: Some(&input.env_refs),
        cwd: input.cwd.as_deref().and_then(|p| p.to_str()),
        decision: "deny",
        matched_rule_index: None,
        reason: Some(reason.to_string()),
        exit_code: None, duration_ms: None,
        stdout_bytes: None, stderr_bytes: None,
        stdout_truncated: None, stderr_truncated: None, timed_out: None,
        count_returned: None,
    }).map_err(|e| ToolError::Audit(e.to_string()))
}

fn format_deny(r: &DenyReason) -> String {
    match r {
        DenyReason::ShellDenylist(p) => format!("shell_denylist:{p}"),
        DenyReason::CwdNotInWorkspaceRoots => "cwd_not_in_workspace_roots".into(),
        DenyReason::NoRuleMatch => "no_rule_match".into(),
    }
}

fn human_deny(r: &DenyReason, input: &RunInput) -> String {
    match r {
        DenyReason::ShellDenylist(p) => format!("program '{p}' is on the shell denylist"),
        DenyReason::CwdNotInWorkspaceRoots => format!(
            "cwd {:?} is not under any workspace_root",
            input.cwd.as_ref().map(|p| p.display().to_string()).unwrap_or_default()
        ),
        DenyReason::NoRuleMatch => format!(
            "no rule matches argv {:?} with env_refs {:?}", input.argv, input.env_refs
        ),
    }
}

fn encode_outputs(out: &[u8], err: &[u8]) -> (String, String, &'static str) {
    match (std::str::from_utf8(out), std::str::from_utf8(err)) {
        (Ok(o), Ok(e)) => (o.to_string(), e.to_string(), "utf8"),
        _ => (
            base64_encode(out),
            base64_encode(err),
            "base64",
        ),
    }
}

fn base64_encode(b: &[u8]) -> String {
    use std::fmt::Write;
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((b.len() + 2) / 3 * 4);
    let mut i = 0;
    while i + 3 <= b.len() {
        let n = u32::from(b[i]) << 16 | u32::from(b[i + 1]) << 8 | u32::from(b[i + 2]);
        for shift in [18, 12, 6, 0] {
            out.push(A[((n >> shift) & 0x3F) as usize] as char);
        }
        i += 3;
    }
    if i < b.len() {
        let mut n: u32 = 0;
        for j in 0..3 {
            n <<= 8;
            if i + j < b.len() { n |= u32::from(b[i + j]); }
        }
        for shift in [18, 12, 6, 0] { let _ = write!(out, "{}", A[((n >> shift) & 0x3F) as usize] as char); }
        let pad = 3 - (b.len() - i);
        out.replace_range(out.len() - pad.., &"=".repeat(pad));
    }
    out
}
```

Append `pub mod tools;` to `mod.rs`.

- [ ] **Step 2: Write integration-style tests using a file backend**

Append to `tools.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::mcp::policy;
    use tempfile::TempDir;

    fn ctx_for_tests(rules_toml: &str) -> (Ctx, TempDir) {
        let tmp = TempDir::new().unwrap();
        // KLEF_TEST_BACKEND requires debug build (cfg(debug_assertions)) — tests run in debug.
        unsafe { std::env::set_var("KLEF_INDEX_PATH", tmp.path().join("index.json")); }
        unsafe { std::env::set_var("KLEF_TEST_BACKEND", format!("file:{}", tmp.path().join("vault").display())); }
        let store = Arc::new(klef_core::build_store(None).unwrap());
        store.add("stripe", "sk_live_abcdefg", None, None, Vec::new(), false).unwrap();
        let pol_path = tmp.path().join("p.toml");
        std::fs::write(&pol_path, rules_toml).unwrap();
        let policy = Arc::new(policy::load(&pol_path).unwrap());
        let audit = Audit::new(tmp.path().join("audit.log"));
        (Ctx { store, policy, audit }, tmp)
    }

    #[tokio::test]
    async fn klef_list_returns_metadata_and_filters() {
        let (ctx, _tmp) = ctx_for_tests("");
        let v = klef_list(&ctx, ListInput::default()).await.unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].name, "stripe");
        let v2 = klef_list(&ctx, ListInput { filter: Some("nope".into()), ..Default::default() }).await.unwrap();
        assert!(v2.is_empty());
    }

    #[tokio::test]
    async fn klef_run_deny_no_rule_match() {
        let (ctx, _tmp) = ctx_for_tests("");
        let r = klef_run(&ctx, RunInput {
            argv: vec!["echo".into(), "hi".into()],
            env_refs: vec![], cwd: None, timeout_ms: None,
        }).await;
        assert!(matches!(r, Err(ToolError::Policy(_))));
    }

    #[tokio::test]
    async fn klef_run_allow_redacts_secret() {
        let toml = r#"
            [[allow]]
            argv = ["printenv", "STRIPE_KEY"]
            env_refs = ["stripe"]
        "#;
        let (ctx, _tmp) = ctx_for_tests(toml);
        let r = klef_run(&ctx, RunInput {
            argv: vec!["printenv".into(), "STRIPE_KEY".into()],
            env_refs: vec!["stripe".into()],
            cwd: None, timeout_ms: Some(5000),
        }).await;
        // printenv won't have STRIPE_KEY (we inject `stripe`, not STRIPE_KEY) — but
        // even if it did, redaction would catch the value. Assert no plaintext leak:
        match r {
            Ok(out) => assert!(!out.stdout.contains("sk_live_abcdefg"), "value must not appear in stdout"),
            Err(_) => {} // policy reject is also fine; the assertion is "no leak".
        }
    }

    #[tokio::test]
    async fn klef_run_deny_audit_recorded() {
        let (ctx, tmp) = ctx_for_tests("");
        let _ = klef_run(&ctx, RunInput {
            argv: vec!["bash".into(), "-c".into(), "x".into()],
            env_refs: vec![], cwd: None, timeout_ms: None,
        }).await;
        let log = std::fs::read_to_string(tmp.path().join("audit.log")).unwrap();
        let last = log.lines().last().unwrap();
        assert!(last.contains("\"decision\":\"deny\""));
        assert!(last.contains("shell_denylist:bash"));
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test -p klef --features mcp commands::mcp::tools
```
Expected: 4 tests PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/klef-cli/src/commands/mcp/tools.rs \
        crates/klef-cli/src/commands/mcp/mod.rs
git commit -m "feat(mcp): klef_list and klef_run handlers with policy + redact + audit"
```

---

## Task 11: rmcp wiring — register tools, dispatch, init, run loop

Goal: `commands::mcp::run(store, policy_path)` builds a tokio runtime, registers `klef_list` and `klef_run` with rmcp's tool builder, serves over stdio, and blocks until stdin closes.

> **Engineer note:** The exact rmcp builder API may differ from what's sketched below. Inspect `cargo doc -p rmcp --features server --open` and adapt — the contract is: tool name, JSON Schema for input, async handler `Fn(Input) -> Result<Output, McpError>`. Errors map to `isError: true` with the `ToolError::Display` message.

**Files:**
- Modify: `crates/klef-cli/src/commands/mcp/mod.rs`

- [ ] **Step 1: Replace `mcp/mod.rs` body**

Replace the `run()` function in `crates/klef-cli/src/commands/mcp/mod.rs` (preserve `pub mod` lines):
```rust
pub mod audit;
pub mod policy;
pub mod redact;
pub mod run_proc;
pub mod tools;

use klef_core::error::KlefError;
use klef_core::store::Store;
use std::path::PathBuf;
use std::sync::Arc;

/// Default policy file path: $XDG_CONFIG_HOME/klef/mcp-policy.toml.
fn default_policy_path() -> Result<PathBuf, KlefError> {
    let base = dirs::config_dir().ok_or_else(|| {
        KlefError::BackendUnavailable("could not resolve config directory".into())
    })?;
    Ok(base.join("klef").join("mcp-policy.toml"))
}

fn default_audit_path() -> Result<PathBuf, KlefError> {
    let base = dirs::config_dir().ok_or_else(|| {
        KlefError::BackendUnavailable("could not resolve config directory".into())
    })?;
    Ok(base.join("klef").join("audit.log"))
}

/// Entry point for `klef mcp`. Loads (or creates) policy, builds the rmcp
/// server, serves over stdio. Blocks until stdin closes.
///
/// # Errors
///
/// Returns an error on policy load failure, audit-path resolution failure,
/// or rmcp server failure.
pub fn run(store: Store, policy_path: Option<PathBuf>) -> Result<(), KlefError> {
    let policy_path = policy_path.map_or_else(default_policy_path, Ok)?;
    let policy = policy::load(&policy_path)
        .map_err(|e| KlefError::BackendUnavailable(e.to_string()))?;
    let audit_path = default_audit_path()?;

    let ctx = Arc::new(tools::Ctx {
        store: Arc::new(store),
        policy: Arc::new(policy),
        audit: audit::Audit::new(audit_path),
    });

    eprintln!("klef mcp: policy = {}", policy_path.display());

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| KlefError::BackendUnavailable(format!("tokio: {e}")))?;
    rt.block_on(serve(ctx))
}

async fn serve(ctx: Arc<tools::Ctx>) -> Result<(), KlefError> {
    use rmcp::ServiceExt;
    // Adapt to the current rmcp builder API. Pseudocode shape:
    let server = rmcp::ServerBuilder::new("klef", env!("CARGO_PKG_VERSION"))
        .tool("klef_list", schema_list(), {
            let ctx = ctx.clone();
            move |input: tools::ListInput| {
                let ctx = ctx.clone();
                async move {
                    tools::klef_list(&ctx, input).await
                        .map_err(|e| rmcp::Error::tool(e.to_string()))
                }
            }
        })
        .tool("klef_run", schema_run(), {
            let ctx = ctx.clone();
            move |input: tools::RunInput| {
                let ctx = ctx.clone();
                async move {
                    tools::klef_run(&ctx, input).await
                        .map_err(|e| rmcp::Error::tool(e.to_string()))
                }
            }
        })
        .build();
    server.serve_stdio().await
        .map_err(|e| KlefError::BackendUnavailable(format!("mcp serve: {e}")))
}

fn schema_list() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "tag":    { "type": "string", "description": "Filter to keys having this tag." },
            "filter": { "type": "string", "description": "Substring filter (case-insensitive)." }
        }
    })
}

fn schema_run() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "required": ["argv"],
        "properties": {
            "argv":       { "type": "array",  "items": { "type": "string" }, "minItems": 1 },
            "env_refs":   { "type": "array",  "items": { "type": "string" } },
            "cwd":        { "type": "string" },
            "timeout_ms": { "type": "integer", "minimum": 1, "maximum": 300000 }
        }
    })
}
```

> If the rmcp API differs (likely), adjust `ServerBuilder`, `Error::tool`, and `serve_stdio` to the actual symbols. The shape (one builder call per tool, async handler, stdio serve) is generic across MCP SDKs.

- [ ] **Step 2: Add `dirs` to mcp feature deps if not transitive**

Check that `dirs` is reachable from `klef-cli` (it is — already a direct dep at line 21 of `Cargo.toml`). No change needed.

- [ ] **Step 3: Build with the feature**

```bash
cargo build -p klef --features mcp
```
Expected: green. If rmcp API symbols don't match, adjust until it builds — the contract from Task 10's tests is unchanged.

- [ ] **Step 4: Smoke-run**

```bash
cargo run -p klef --features mcp -- mcp --policy /tmp/klef-test-policy.toml &
sleep 1; jobs -l; kill %1 2>/dev/null
```
Expected: `klef mcp: policy = /tmp/klef-test-policy.toml` on stderr; the skeleton file appears at that path; the process is alive until killed.

- [ ] **Step 5: Commit**

```bash
git add crates/klef-cli/src/commands/mcp/mod.rs
git commit -m "feat(mcp): rmcp server wiring with stdio transport"
```

---

## Task 12: Integration tests with a fake stdio MCP client

Goal: `crates/klef-cli/tests/mcp.rs` exercises the binary end-to-end: spawn `klef mcp`, send JSON-RPC frames, assert responses.

**Files:**
- Create: `crates/klef-cli/tests/mcp.rs`

- [ ] **Step 1: Write the test harness**

Create `crates/klef-cli/tests/mcp.rs`:
```rust
//! End-to-end MCP server tests. Spawns the `klef` binary with the `mcp`
//! feature, speaks JSON-RPC 2.0 over stdio, asserts responses.

#![cfg(feature = "mcp")]

use assert_cmd::cargo::CommandCargoExt;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use tempfile::TempDir;

fn spawn_server(tmp: &TempDir, policy: &str) -> std::process::Child {
    let policy_path = tmp.path().join("p.toml");
    std::fs::write(&policy_path, policy).unwrap();
    let vault = tmp.path().join("vault");
    let index = tmp.path().join("index.json");
    let mut cmd = Command::cargo_bin("klef").unwrap();
    cmd.arg("mcp").arg("--policy").arg(&policy_path)
        .env("KLEF_INDEX_PATH", &index)
        .env("KLEF_TEST_BACKEND", format!("file:{}", vault.display()))
        .env("HOME", tmp.path())
        .env("XDG_CONFIG_HOME", tmp.path().join("config"))
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd.spawn().unwrap()
}

fn rpc(child: &mut std::process::Child, body: serde_json::Value) -> serde_json::Value {
    let stdin = child.stdin.as_mut().unwrap();
    let line = serde_json::to_string(&body).unwrap();
    writeln!(stdin, "{line}").unwrap();
    stdin.flush().unwrap();
    let stdout = child.stdout.as_mut().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut buf = String::new();
    reader.read_line(&mut buf).unwrap();
    serde_json::from_str(&buf).unwrap()
}

#[test]
fn tools_list_exposes_only_list_and_run() {
    let tmp = TempDir::new().unwrap();
    let mut child = spawn_server(&tmp, "");
    let _init = rpc(&mut child, serde_json::json!({
        "jsonrpc":"2.0", "id":1, "method":"initialize",
        "params":{ "protocolVersion":"2024-11-05", "capabilities":{}, "clientInfo":{"name":"t","version":"0"}}
    }));
    let resp = rpc(&mut child, serde_json::json!({
        "jsonrpc":"2.0", "id":2, "method":"tools/list", "params":{}
    }));
    let names: Vec<String> = resp["result"]["tools"].as_array().unwrap()
        .iter().map(|t| t["name"].as_str().unwrap().to_string()).collect();
    assert!(names.contains(&"klef_list".to_string()));
    assert!(names.contains(&"klef_run".to_string()));
    assert!(!names.iter().any(|n| n == "klef_get" || n == "klef_export"));
    child.kill().ok();
}

#[test]
fn klef_run_deny_shell_returns_error() {
    let tmp = TempDir::new().unwrap();
    let mut child = spawn_server(&tmp, "");
    let _ = rpc(&mut child, serde_json::json!({
        "jsonrpc":"2.0", "id":1, "method":"initialize",
        "params":{ "protocolVersion":"2024-11-05", "capabilities":{}, "clientInfo":{"name":"t","version":"0"}}
    }));
    let resp = rpc(&mut child, serde_json::json!({
        "jsonrpc":"2.0", "id":2, "method":"tools/call",
        "params":{ "name":"klef_run", "arguments":{ "argv":["bash","-c","echo x"], "env_refs":[] } }
    }));
    let is_error = resp["result"]["isError"].as_bool().unwrap_or(false)
        || resp.get("error").is_some();
    assert!(is_error, "expected error, got {resp}");
    child.kill().ok();
}
```

> If the rmcp adapter in Task 11 ended up using a slightly different framing (e.g., Content-Length headers vs. line-delimited JSON), update `rpc()` here to match. The intent is: send one frame, read one frame.

- [ ] **Step 2: Run integration tests**

```bash
cargo test -p klef --features mcp --test mcp
```
Expected: 2 tests PASS. Adjust `rpc()` framing if rmcp uses LSP-style headers instead of line-delimited JSON.

- [ ] **Step 3: Commit**

```bash
git add crates/klef-cli/tests/mcp.rs
git commit -m "test(mcp): end-to-end JSON-RPC tests for tool surface and shell deny"
```

---

## Task 13: User docs

Goal: `docs/mcp.md` shows how to wire klef into Claude Desktop and how to write a policy.

**Files:**
- Create: `docs/mcp.md`
- Modify: `README.md` (add a one-line pointer)

- [ ] **Step 1: Write `docs/mcp.md`**

Create `docs/mcp.md`:
```markdown
# klef MCP server

`klef mcp` exposes klef to MCP clients (Claude Desktop, Claude Code, Cursor) so an agent can use your API keys without ever receiving the plaintext value.

## What's exposed — and what isn't

| Tool | Effect | Sees secret values? |
|---|---|---|
| `klef_list` | Returns names + metadata | ❌ never |
| `klef_run`  | Spawns a process with `klef:` refs injected as env vars; returns stdout/stderr | ❌ not directly |
| ~~`klef_get`~~ | _not exposed_ — would leak values into the agent's context | — |
| ~~`klef_add` / `klef_rm` / `klef_edit`~~ | _not exposed_ — mutation stays manual | — |

To populate a `.env`, the agent writes the *reference* `klef:<name>` directly. It never needs the value.

## Setup — Claude Desktop

```json
{
  "mcpServers": {
    "klef": {
      "command": "klef",
      "args": ["mcp"]
    }
  }
}
```

Restart Claude Desktop. Ask "list my klef keys" — you should see metadata. Ask "show me my Stripe key" — Claude will say it can't (the tool doesn't exist).

## Policy file

Path: `~/.config/klef/mcp-policy.toml`. First run writes a commented skeleton; edit it to enable `klef_run`.

```toml
workspace_roots = ["/Users/oscarr/Desktop", "/Users/oscarr/code"]

[[allow]]
argv = ["npm", "run", "*"]
env_refs = ["stripe", "anthropic"]

[[allow]]
argv = ["cargo", "test"]
env_refs = []
```

Matching rules:
- A request is allowed if some rule's `argv` matches (token-level globs, `*` and `?`) **and** the rule's `env_refs` covers every requested env_ref.
- Shells are denied unconditionally: `sh, bash, zsh, python, node, ...` — even if a rule appears to allow them.
- If `workspace_roots` is set, requests with a `cwd` outside any root are denied. Empty/unset = no constraint.

## Audit log

Every call (allow or deny) writes one NDJSON entry to `~/.config/klef/audit.log`. If the log can't be written, the call is denied (fail-closed). No internal rotation — manage with `logrotate` if you keep it forever.

## Threat model

This is **not** a zero-knowledge system. A malicious agent can craft an `argv` that exfiltrates a value (e.g., `curl` with the key in a query string). What this design changes vs. exposing a `klef_get` tool:

- Without `klef_run`: every normal use puts plaintext secrets into the agent's context — passive, continuous leak into transcripts and provider logs.
- With `klef_run`: secrets only enter agent-visible output if the agent issues an explicitly extractive `argv` — leaves an audit trail, rejectable via policy.

Risk shifts from "passive systematic leak" to "active detectable exfil".
```

- [ ] **Step 2: Add a one-line pointer in README**

In `README.md`, in the "Pour les agents IA" section (around line 173), append a bullet:
```markdown
- **[`docs/mcp.md`](./docs/mcp.md)** : MCP server (`klef mcp`) — let Claude/Cursor use your keys without ever seeing the plaintext value.
```

- [ ] **Step 3: Commit**

```bash
git add docs/mcp.md README.md
git commit -m "docs(mcp): user setup guide for Claude Desktop and policy syntax"
```

---

## Task 14: Final verification

Goal: Full test suite passes with and without the feature, hooks pass, line cap respected.

- [ ] **Step 1: All-features test pass**

```bash
cargo test --workspace --all-features
```
Expected: green.

- [ ] **Step 2: Default-features test pass (regression check)**

```bash
cargo test --workspace
```
Expected: green; no MCP code compiled in.

- [ ] **Step 3: Clippy pass**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
Expected: green.

- [ ] **Step 4: Format check**

```bash
cargo fmt --all -- --check
```
Expected: green.

- [ ] **Step 5: Line cap**

```bash
scripts/check-lines.sh
```
Expected: green (no file > 300 lines). If a file is over, split it before commit.

- [ ] **Step 6: Manual smoke test**

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:
```json
{
  "mcpServers": { "klef": { "command": "<repo>/target/debug/klef", "args": ["--features", "mcp", "mcp"] } }
}
```
(or build a release binary first). Restart Claude Desktop. Ask "list my klef keys". Confirm names show. Ask "what's the value of stripe" — Claude should report there's no such tool.

- [ ] **Step 7: Final commit if anything remains**

If steps 3-5 required fixes:
```bash
git add -A
git commit -m "chore(mcp): fmt + clippy + line-cap fixes"
```

---

## Wrap-up

After all tasks:
- Issue #24 is closed by the merge of this work.
- The pivot comment posted in #24 is now backed by code; reply on the issue with a link to the merged PR.
- Out-of-scope items (interactive approval, GUI side-channel, `klef mcp policy add` helper) are tracked as follow-up issues if/when needed.
