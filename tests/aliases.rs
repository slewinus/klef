//! Tests for command aliases (e.g. `klef remove` ↔ `klef rm`).
//! When new aliases are added, append a test here rather than growing tests/cli.rs.

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

#[test]
fn remove_is_alias_for_rm() {
    let d = TempDir::new().unwrap();

    klef(d.path())
        .arg("add")
        .arg("foo")
        .write_stdin("v")
        .assert()
        .success();

    // Use the `remove` alias instead of `rm`.
    klef(d.path())
        .arg("remove")
        .arg("foo")
        .arg("--yes")
        .assert()
        .success();

    // Confirm it's actually gone.
    klef(d.path())
        .arg("get")
        .arg("foo")
        .assert()
        .failure()
        .code(2);
}
