//! Tests for `--value-from-file` on add/edit and the `klef set-note` shortcut.

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
fn add_with_value_from_file() {
    let d = TempDir::new().unwrap();
    let secret_path = d.path().join("cert.pem");
    std::fs::write(
        &secret_path,
        "-----BEGIN PRIVATE KEY-----\nabc123def\n-----END PRIVATE KEY-----\n",
    )
    .unwrap();

    klef(d.path())
        .arg("add")
        .arg("ssl-key")
        .arg("--value-from-file")
        .arg(&secret_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("✓ 'ssl-key' saved"));

    klef(d.path())
        .arg("get")
        .arg("ssl-key")
        .assert()
        .success()
        .stdout(predicate::str::contains("BEGIN PRIVATE KEY"))
        .stdout(predicate::str::contains("abc123def"));
}

#[test]
fn edit_with_value_from_file_replaces_value() {
    let d = TempDir::new().unwrap();

    klef(d.path())
        .arg("add")
        .arg("foo")
        .write_stdin("v1")
        .assert()
        .success();

    let new_value_path = d.path().join("new.txt");
    std::fs::write(&new_value_path, "v2-from-file").unwrap();

    klef(d.path())
        .arg("edit")
        .arg("foo")
        .arg("--value-from-file")
        .arg(&new_value_path)
        .assert()
        .success();

    klef(d.path())
        .arg("get")
        .arg("foo")
        .assert()
        .success()
        .stdout(predicate::str::contains("v2-from-file"));
}

#[test]
fn set_note_updates_only_the_note() {
    let d = TempDir::new().unwrap();

    klef(d.path())
        .arg("add")
        .arg("stripe")
        .write_stdin("v")
        .assert()
        .success();

    klef(d.path())
        .arg("set-note")
        .arg("stripe")
        .arg("billing prod")
        .assert()
        .success()
        .stdout(predicate::str::contains("✓ 'stripe' note updated"));

    klef(d.path())
        .arg("show")
        .arg("stripe")
        .assert()
        .success()
        .stdout(predicate::str::contains("note:    billing prod"))
        // value should be untouched
        .stdout(predicate::str::contains("value:   v"));
}

#[test]
fn set_note_on_missing_key_fails() {
    let d = TempDir::new().unwrap();

    klef(d.path())
        .arg("set-note")
        .arg("nope")
        .arg("hi")
        .assert()
        .failure()
        .code(2);
}
