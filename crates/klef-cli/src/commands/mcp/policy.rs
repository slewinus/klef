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
        assert!(
            written.contains("[[allow]]"),
            "skeleton must contain commented rule example"
        );
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

    #[test]
    fn argv_matches_exact() {
        assert!(argv_matches(
            &["npm".into(), "start".into()],
            &["npm", "start"]
        ));
        assert!(!argv_matches(
            &["npm".into(), "start".into()],
            &["npm", "test"]
        ));
    }

    #[test]
    fn argv_matches_wildcard_token() {
        assert!(argv_matches(
            &["npm".into(), "run".into(), "*".into()],
            &["npm", "run", "dev"]
        ));
        assert!(argv_matches(
            &["npm".into(), "run".into(), "*".into()],
            &["npm", "run", "build:prod"]
        ));
        assert!(!argv_matches(
            &["npm".into(), "run".into(), "*".into()],
            &["npm", "test"]
        ));
    }

    #[test]
    fn argv_matches_length_mismatch_is_no_match() {
        assert!(!argv_matches(
            &["npm".into(), "*".into()],
            &["npm", "run", "dev"]
        ));
    }

    #[test]
    fn argv_matches_question_mark_token() {
        assert!(argv_matches(
            &["cargo".into(), "?est".into()],
            &["cargo", "test"]
        ));
        assert!(!argv_matches(
            &["cargo".into(), "?est".into()],
            &["cargo", "build"]
        ));
    }

    #[test]
    fn argv_matches_url_with_path_glob() {
        assert!(argv_matches(
            &["curl".into(), "https://api.stripe.com/*".into()],
            &["curl", "https://api.stripe.com/v1/charges"],
        ));
    }

    #[test]
    fn shell_denylist_bare_names() {
        for name in [
            "sh", "bash", "zsh", "python", "python3", "node", "deno", "bun", "env",
        ] {
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
        assert!(
            !is_shell_program("./my-script.sh"),
            "extension does not imply shell interpreter"
        );
    }

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
}
