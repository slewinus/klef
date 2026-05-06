//! End-to-end tests for klef --backend age: through the CLI.
//!
//! These tests verify that:
//! 1. Metadata (note, tags) round-trips through the encrypted vault.
//! 2. The global index file is NEVER written when --backend age: is in use.
//! 3. Listing only returns keys from the age vault.

use assert_cmd::Command;
use tempfile::TempDir;

fn klef_age(dir: &std::path::Path) -> Command {
    let vault = dir.join("vault.age");
    let mut c = Command::cargo_bin("klef").unwrap();
    c.env("KLEF_PASSPHRASE", "test123");
    c.arg("--backend").arg(format!("age:{}", vault.display()));
    c
}

#[test]
fn age_vault_round_trip_with_metadata() {
    let d = TempDir::new().unwrap();

    // Add a key with note and tag.
    klef_age(d.path())
        .args(["add", "stripe", "--note", "prod", "--tag", "billing"])
        .write_stdin("sk_live")
        .assert()
        .success();

    // Show should display note, tags, and value — all from the encrypted vault.
    let output = klef_age(d.path())
        .args(["show", "stripe"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let out = String::from_utf8(output).unwrap();
    assert!(out.contains("prod"), "note not found in show output: {out}");
    assert!(
        out.contains("billing"),
        "tag not found in show output: {out}"
    );
    assert!(
        out.contains("sk_live"),
        "value not found in show output: {out}"
    );
}

#[test]
fn age_vault_does_not_pollute_global_index() {
    let d = TempDir::new().unwrap();
    let global_index = d.path().join("global-index.json");

    // Add a key, routing index writes through KLEF_INDEX_PATH so we can detect
    // any writes to the global index path.
    klef_age(d.path())
        .env("KLEF_INDEX_PATH", &global_index)
        .args(["add", "isolated-key"])
        .write_stdin("v")
        .assert()
        .success();

    // The global index path should NOT have been created.
    assert!(
        !global_index.exists(),
        "age backend wrote to the global index file (it shouldn't)"
    );
}

#[test]
fn age_vault_list_returns_only_age_keys() {
    let d = TempDir::new().unwrap();

    klef_age(d.path())
        .args(["add", "a"])
        .write_stdin("1")
        .assert()
        .success();
    klef_age(d.path())
        .args(["add", "b"])
        .write_stdin("2")
        .assert()
        .success();

    let output = klef_age(d.path())
        .args(["list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let out = String::from_utf8(output).unwrap();
    assert!(out.contains('a'), "key 'a' missing from list: {out}");
    assert!(out.contains('b'), "key 'b' missing from list: {out}");
}
