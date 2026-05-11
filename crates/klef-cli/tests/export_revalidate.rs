//! Regression test for the export → eval shell-injection vector.
//!
//! `klef add` validates `env_var` at write time, but legacy indexes from
//! pre-validation klef installs (or a tampered file on disk) could feed
//! a shell-unsafe payload into `klef export`. Render must re-validate.

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;
use tempfile::TempDir;

fn klef(dir: &Path) -> Command {
    let secrets = dir.join("secrets.json");
    let index = dir.join("index.json");
    let mut c = Command::cargo_bin("klef").unwrap();
    c.env("KLEF_TEST_BACKEND", format!("file:{}", secrets.display()));
    c.env("KLEF_INDEX_PATH", &index);
    c
}

#[test]
fn export_rejects_legacy_index_with_shell_metachars_in_env_var() {
    let d = TempDir::new().unwrap();
    klef(d.path())
        .arg("add")
        .arg("stripe")
        .write_stdin("v")
        .assert()
        .success();
    // Simulate a legacy / tampered file: hand-edit the index to inject a
    // shell-unsafe env-var name. `klef add` would have refused this, but
    // an on-disk file could still feed it into the render path.
    let index_path = d.path().join("index.json");
    let raw = std::fs::read_to_string(&index_path).unwrap();
    let tampered = raw.replace("STRIPE_API_KEY", "FOO; rm -rf $HOME #");
    assert_ne!(raw, tampered, "expected to replace something");
    std::fs::write(&index_path, tampered).unwrap();

    klef(d.path())
        .arg("export")
        .arg("stripe")
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid env-var name"));
}
