//! Tests for `klef edit --note-edit` (open $EDITOR / $VISUAL for the note).

use assert_cmd::Command;
use predicates::prelude::*;
use std::os::unix::fs::PermissionsExt;
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

fn write_editor_script(dir: &Path, name: &str, body: &str) -> std::path::PathBuf {
    let path = dir.join(format!("{name}.sh"));
    let script = format!("#!/bin/sh\n{body}\n");
    std::fs::write(&path, script).unwrap();
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&path, perms).unwrap();
    path
}

#[test]
fn note_edit_with_editor_writes_trimmed_note() {
    let d = TempDir::new().unwrap();

    klef(d.path())
        .arg("add")
        .arg("stripe")
        .arg("--note")
        .arg("old note")
        .write_stdin("v")
        .assert()
        .success();

    // Editor script overwrites the file with whitespace-padded content;
    // klef should trim it.
    let editor = write_editor_script(d.path(), "fresh", r#"printf '  fresh note  \n' > "$1""#);

    klef(d.path())
        .env("EDITOR", &editor)
        .arg("edit")
        .arg("stripe")
        .arg("--note-edit")
        .assert()
        .success()
        .stdout(predicate::str::contains("✓ 'stripe' note updated"));

    klef(d.path())
        .arg("show")
        .arg("stripe")
        .assert()
        .success()
        .stdout(predicate::str::contains("note:    fresh note"));
}

#[test]
fn note_edit_pre_fills_editor_with_current_note() {
    let d = TempDir::new().unwrap();

    klef(d.path())
        .arg("add")
        .arg("foo")
        .arg("--note")
        .arg("billing prod")
        .write_stdin("v")
        .assert()
        .success();

    // Editor script copies the pre-filled content back out so we can inspect it.
    let captured = d.path().join("captured.txt");
    let editor = write_editor_script(
        d.path(),
        "capture",
        &format!(r#"cp "$1" "{}""#, captured.display()),
    );

    klef(d.path())
        .env("EDITOR", &editor)
        .arg("edit")
        .arg("foo")
        .arg("--note-edit")
        .assert()
        .success();

    let pre_filled = std::fs::read_to_string(&captured).unwrap();
    assert_eq!(pre_filled, "billing prod");
}

#[test]
fn note_edit_visual_takes_precedence_over_editor() {
    let d = TempDir::new().unwrap();

    klef(d.path())
        .arg("add")
        .arg("api")
        .write_stdin("v")
        .assert()
        .success();

    let visual = write_editor_script(d.path(), "visual", r#"echo "from VISUAL" > "$1""#);
    let editor = write_editor_script(d.path(), "editor", r#"echo "from EDITOR" > "$1""#);

    klef(d.path())
        .env("VISUAL", &visual)
        .env("EDITOR", &editor)
        .arg("edit")
        .arg("api")
        .arg("--note-edit")
        .assert()
        .success();

    klef(d.path())
        .arg("show")
        .arg("api")
        .assert()
        .success()
        .stdout(predicate::str::contains("note:    from VISUAL"));
}

#[test]
fn note_edit_falls_back_to_stdin_when_editor_unset() {
    let d = TempDir::new().unwrap();

    klef(d.path())
        .arg("add")
        .arg("foo")
        .write_stdin("v")
        .assert()
        .success();

    klef(d.path())
        .arg("edit")
        .arg("foo")
        .arg("--note-edit")
        .write_stdin("typed-from-stdin\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("✓ 'foo' note updated"));

    klef(d.path())
        .arg("show")
        .arg("foo")
        .assert()
        .success()
        .stdout(predicate::str::contains("note:    typed-from-stdin"));
}

#[test]
fn note_edit_empty_result_clears_note() {
    let d = TempDir::new().unwrap();

    klef(d.path())
        .arg("add")
        .arg("foo")
        .arg("--note")
        .arg("to be cleared")
        .write_stdin("v")
        .assert()
        .success();

    let editor = write_editor_script(d.path(), "blank", r#"printf '   \n' > "$1""#);

    klef(d.path())
        .env("EDITOR", &editor)
        .arg("edit")
        .arg("foo")
        .arg("--note-edit")
        .assert()
        .success();

    // `klef show` omits the `note:` line entirely when there is no note.
    klef(d.path())
        .arg("show")
        .arg("foo")
        .assert()
        .success()
        .stdout(predicate::str::contains("note:").not());
}

#[test]
fn note_edit_aborts_when_editor_fails() {
    let d = TempDir::new().unwrap();

    klef(d.path())
        .arg("add")
        .arg("foo")
        .arg("--note")
        .arg("kept")
        .write_stdin("v")
        .assert()
        .success();

    let editor = write_editor_script(d.path(), "fail", "exit 1");

    klef(d.path())
        .env("EDITOR", &editor)
        .arg("edit")
        .arg("foo")
        .arg("--note-edit")
        .assert()
        .failure();

    klef(d.path())
        .arg("show")
        .arg("foo")
        .assert()
        .success()
        .stdout(predicate::str::contains("note:    kept"));
}

#[test]
fn note_edit_conflicts_with_note_flag() {
    let d = TempDir::new().unwrap();

    klef(d.path())
        .arg("edit")
        .arg("anything")
        .arg("--note-edit")
        .arg("--note")
        .arg("x")
        .assert()
        .failure();
}
