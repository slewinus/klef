//! Regression tests for `klef edit` `env_var` preservation (closes #71).
//!
//! Before the fix, `klef edit <name>` without `--as` would silently
//! overwrite a custom `env_var` (set at `add` time) with the default
//! name derived from the key. These tests pin the desired behavior:
//! editing only the value must NOT touch metadata that was not
//! explicitly targeted.

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
    c.env_remove("EDITOR");
    c.env_remove("VISUAL");
    c
}

#[test]
fn edit_value_without_as_preserves_custom_env_var() {
    let d = TempDir::new().unwrap();

    klef(d.path())
        .arg("add")
        .arg("stripe")
        .arg("--as")
        .arg("STRIPE_KEY")
        .write_stdin("v1")
        .assert()
        .success();

    klef(d.path())
        .arg("edit")
        .arg("stripe")
        .write_stdin("v2")
        .assert()
        .success();

    klef(d.path())
        .arg("show")
        .arg("stripe")
        .assert()
        .success()
        .stdout(predicate::str::contains("env var: STRIPE_KEY"))
        .stdout(predicate::str::contains("value:   v2"));
}

#[test]
fn edit_value_with_as_changes_env_var() {
    let d = TempDir::new().unwrap();

    klef(d.path())
        .arg("add")
        .arg("stripe")
        .arg("--as")
        .arg("STRIPE_KEY")
        .write_stdin("v1")
        .assert()
        .success();

    // `--as` without --value-from-file is a meta-only update: env_var is
    // changed, the value is left untouched.
    klef(d.path())
        .arg("edit")
        .arg("stripe")
        .arg("--as")
        .arg("STRIPE_NEW")
        .assert()
        .success();

    klef(d.path())
        .arg("show")
        .arg("stripe")
        .assert()
        .success()
        .stdout(predicate::str::contains("env var: STRIPE_NEW"))
        .stdout(predicate::str::contains("value:   v1"));
}

#[test]
fn edit_value_preserves_default_env_var_when_unchanged() {
    // Sanity check: when no custom env_var was ever set, the default
    // (derived from the key name) is still in place after an edit.
    let d = TempDir::new().unwrap();

    klef(d.path())
        .arg("add")
        .arg("stripe")
        .write_stdin("v1")
        .assert()
        .success();

    klef(d.path())
        .arg("edit")
        .arg("stripe")
        .write_stdin("v2")
        .assert()
        .success();

    klef(d.path())
        .arg("show")
        .arg("stripe")
        .assert()
        .success()
        .stdout(predicate::str::contains("env var: STRIPE_API_KEY"))
        .stdout(predicate::str::contains("value:   v2"));
}
