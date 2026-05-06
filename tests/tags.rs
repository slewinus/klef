//! Tests for tag-related commands and flags.

use assert_cmd::Command;
use tempfile::TempDir;

fn klef(dir: &std::path::Path) -> Command {
    let secrets = dir.join("secrets.json");
    let index = dir.join("index.json");
    let mut c = Command::cargo_bin("klef").unwrap();
    c.env("KLEF_TEST_BACKEND", format!("file:{}", secrets.display()));
    c.env("KLEF_INDEX_PATH", &index);
    c
}

fn add_with_tags(dir: &std::path::Path, name: &str, value: &str, tags: &[&str]) {
    let mut cmd = klef(dir);
    cmd.arg("add").arg(name);
    for t in tags {
        cmd.arg("--tag").arg(t);
    }
    cmd.write_stdin(value).assert().success();
}

#[test]
fn add_with_multiple_tags_persists_them() {
    let d = TempDir::new().unwrap();
    add_with_tags(d.path(), "stripe", "v", &["billing", "prod"]);
    klef(d.path())
        .arg("show")
        .arg("stripe")
        .assert()
        .success()
        .stdout(predicates::str::contains("tags:    billing, prod"));
}

#[test]
fn list_filter_by_tag_returns_only_matching() {
    let d = TempDir::new().unwrap();
    add_with_tags(d.path(), "stripe", "v1", &["billing"]);
    add_with_tags(d.path(), "anthropic", "v2", &["ai"]);
    add_with_tags(d.path(), "openai", "v3", &["ai", "billing"]);

    klef(d.path())
        .arg("list")
        .arg("--tag")
        .arg("ai")
        .assert()
        .success()
        .stdout(predicates::str::contains("anthropic"))
        .stdout(predicates::str::contains("openai"))
        .stdout(predicates::function::function(|out: &str| {
            !out.contains("stripe")
        }));
}

#[test]
fn list_verbose_shows_tags_column() {
    let d = TempDir::new().unwrap();
    add_with_tags(d.path(), "stripe", "v", &["billing", "prod"]);
    klef(d.path())
        .arg("list")
        .arg("-v")
        .assert()
        .success()
        .stdout(predicates::str::contains("TAGS"))
        .stdout(predicates::str::contains("billing, prod"));
}

#[test]
fn tags_command_lists_all_with_counts() {
    let d = TempDir::new().unwrap();
    add_with_tags(d.path(), "a", "v", &["x", "y"]);
    add_with_tags(d.path(), "b", "v", &["y"]);
    add_with_tags(d.path(), "c", "v", &["z"]);

    let assert = klef(d.path()).arg("tags").assert().success();
    let out = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    // x → 1, y → 2, z → 1
    assert!(out.contains('y'));
    assert!(out.contains('2'));
    assert!(out.contains("KEYS"));
}

#[test]
fn tags_on_empty_vault_prints_no_tags() {
    let d = TempDir::new().unwrap();
    klef(d.path())
        .arg("tags")
        .assert()
        .success()
        .stdout(predicates::str::contains("(no tags in use)"));
}

#[test]
fn edit_replaces_tags() {
    let d = TempDir::new().unwrap();
    add_with_tags(d.path(), "stripe", "v", &["a", "b"]);
    klef(d.path())
        .arg("edit")
        .arg("stripe")
        .arg("--tag")
        .arg("c")
        .arg("--tag")
        .arg("d")
        .assert()
        .success();
    klef(d.path())
        .arg("show")
        .arg("stripe")
        .assert()
        .success()
        .stdout(predicates::str::contains("tags:    c, d"))
        .stdout(predicates::function::function(|out: &str| {
            !out.contains("a, b")
        }));
}

#[test]
fn edit_clear_tags_wipes() {
    let d = TempDir::new().unwrap();
    add_with_tags(d.path(), "stripe", "v", &["a", "b"]);
    klef(d.path())
        .arg("edit")
        .arg("stripe")
        .arg("--clear-tags")
        .assert()
        .success();
    klef(d.path())
        .arg("show")
        .arg("stripe")
        .assert()
        .success()
        .stdout(predicates::function::function(|out: &str| {
            !out.contains("tags:")
        }));
}
