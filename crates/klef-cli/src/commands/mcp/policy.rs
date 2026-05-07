//! Policy file: TOML schema, parsing, and a skeleton-on-first-run helper.
//! Matching and evaluation logic is added in subsequent tasks.

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
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("policy: cannot write skeleton to {path}: {source}")]
    WriteSkeleton {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("policy: invalid TOML in {path}: {source}")]
    Toml {
        path: PathBuf,
        source: toml::de::Error,
    },
}

const SKELETON: &str = r#"# klef MCP policy file. Each [[allow]] rule whitelists one argv pattern +
# env_refs that may be injected. A request is allowed iff SOME rule matches
# argv (with wildcards) AND covers every requested env_ref. Shells (sh, bash,
# python, node, ...) are denied unconditionally. See docs/mcp.md.

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

/// Match a request argv against a rule's argv pattern.
///
/// Each pattern element is a token-level glob (`*` and `?` per `glob::Pattern`).
/// Length mismatch = no match (no variadic wildcards).
#[must_use]
pub fn argv_matches(pattern: &[String], argv: &[&str]) -> bool {
    if pattern.len() != argv.len() {
        return false;
    }
    pattern
        .iter()
        .zip(argv.iter())
        .all(|(pat, arg)| glob::Pattern::new(pat).is_ok_and(|p| p.matches(arg)))
}

const SHELL_DENYLIST: &[&str] = &[
    "sh", "bash", "zsh", "fish", "dash", "ksh", "csh", "tcsh", "ash", "python", "python3", "ruby",
    "perl", "lua", "awk", "node", "deno", "bun", "eval", "exec", "env",
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
    let Ok(cwd_real) = cwd.canonicalize() else {
        return false;
    };
    roots.iter().any(|r| {
        r.canonicalize()
            .is_ok_and(|root_real| cwd_real.starts_with(&root_real))
    })
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
