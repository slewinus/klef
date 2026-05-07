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
