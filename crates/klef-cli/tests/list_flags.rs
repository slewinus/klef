//! Tests for `klef list --verbose` and `klef list --filter <pattern>`.

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

fn add(d: &Path, name: &str, value: &str) {
    klef(d)
        .arg("add")
        .arg(name)
        .write_stdin(value)
        .assert()
        .success();
}

#[test]
fn verbose_adds_added_column() {
    let d = TempDir::new().unwrap();
    add(d.path(), "stripe", "v");

    klef(d.path())
        .arg("list")
        .arg("--verbose")
        .assert()
        .success()
        .stdout(predicate::str::contains("ADDED"))
        .stdout(predicate::str::contains("stripe"));
}

#[test]
fn default_list_does_not_have_added_column() {
    let d = TempDir::new().unwrap();
    add(d.path(), "stripe", "v");

    klef(d.path())
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("NAME"))
        .stdout(predicate::str::contains("stripe"))
        .stdout(predicate::str::contains("ADDED").not());
}

#[test]
fn filter_matches_name_substring() {
    let d = TempDir::new().unwrap();
    add(d.path(), "stripe-prod", "v1");
    add(d.path(), "stripe-test", "v2");
    add(d.path(), "anthropic", "v3");

    klef(d.path())
        .arg("list")
        .arg("--filter")
        .arg("stripe")
        .assert()
        .success()
        .stdout(predicate::str::contains("stripe-prod"))
        .stdout(predicate::str::contains("stripe-test"))
        .stdout(predicate::str::contains("anthropic").not());
}

#[test]
fn filter_is_case_insensitive() {
    let d = TempDir::new().unwrap();
    add(d.path(), "stripe", "v");

    klef(d.path())
        .arg("list")
        .arg("--filter")
        .arg("STRIPE")
        .assert()
        .success()
        .stdout(predicate::str::contains("stripe"));
}

#[test]
fn filter_matches_note_substring() {
    let d = TempDir::new().unwrap();

    // Add then set note via edit so the index has a note field set.
    add(d.path(), "alpha", "v");
    klef(d.path())
        .arg("edit")
        .arg("alpha")
        .arg("--note")
        .arg("billing-prod")
        .assert()
        .success();

    add(d.path(), "beta", "v");

    klef(d.path())
        .arg("list")
        .arg("--filter")
        .arg("billing")
        .assert()
        .success()
        .stdout(predicate::str::contains("alpha"))
        .stdout(predicate::str::contains("beta").not());
}

#[test]
fn filter_with_no_match_prints_empty_marker() {
    let d = TempDir::new().unwrap();
    add(d.path(), "stripe", "v");

    klef(d.path())
        .arg("list")
        .arg("--filter")
        .arg("nope")
        .assert()
        .success()
        .stdout(predicate::str::contains("(no keys stored)"));
}

#[test]
fn verbose_and_filter_compose() {
    let d = TempDir::new().unwrap();
    add(d.path(), "stripe-prod", "v");
    add(d.path(), "anthropic", "v");

    klef(d.path())
        .arg("list")
        .arg("--verbose")
        .arg("--filter")
        .arg("stripe")
        .assert()
        .success()
        .stdout(predicate::str::contains("ADDED"))
        .stdout(predicate::str::contains("stripe-prod"))
        .stdout(predicate::str::contains("anthropic").not());
}
