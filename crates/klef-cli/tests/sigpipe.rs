//! Regression tests: multi-line stdout commands must exit cleanly
//! when the downstream pipe closes early (closes #73).

#![cfg(unix)]

use assert_cmd::Command as AssertCommand;
use assert_cmd::cargo::CommandCargoExt;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use tempfile::TempDir;

fn klef_env(cmd: &mut Command, dir: &Path) {
    let secrets = dir.join("secrets.json");
    let index = dir.join("index.json");
    cmd.env("KLEF_TEST_BACKEND", format!("file:{}", secrets.display()));
    cmd.env("KLEF_INDEX_PATH", &index);
    cmd.env_remove("EDITOR");
    cmd.env_remove("VISUAL");
}

fn klef_assert(dir: &Path) -> AssertCommand {
    let secrets = dir.join("secrets.json");
    let index = dir.join("index.json");
    let mut c = AssertCommand::cargo_bin("klef").unwrap();
    c.env("KLEF_TEST_BACKEND", format!("file:{}", secrets.display()));
    c.env("KLEF_INDEX_PATH", &index);
    c.env_remove("EDITOR");
    c.env_remove("VISUAL");
    c
}

fn seed_50_keys(dir: &Path) {
    for i in 0..50 {
        klef_assert(dir)
            .arg("add")
            .arg(format!("key_{i:03}"))
            .arg("--as")
            .arg(format!("KEY_{i:03}"))
            .write_stdin(format!("v{i}"))
            .assert()
            .success();
    }
}

fn seed_50_keys_with_tags(dir: &Path) {
    for i in 0..50 {
        klef_assert(dir)
            .arg("add")
            .arg(format!("key_{i:03}"))
            .arg("--as")
            .arg(format!("KEY_{i:03}"))
            .arg("--tag")
            .arg(format!("tag{}", i % 10))
            .write_stdin(format!("v{i}"))
            .assert()
            .success();
    }
}

fn assert_clean_exit_on_pipe_close(args: &[&str], dir: &Path) {
    let mut cmd = Command::cargo_bin("klef").unwrap();
    klef_env(&mut cmd, dir);
    cmd.args(args).stdout(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    let _ = reader.read_line(&mut line);
    drop(reader);
    let status = child.wait().unwrap();
    let code = status.code();
    assert!(
        matches!(code, Some(0 | 141)),
        "expected clean exit (0 or 141), got status={status:?}, code={code:?}"
    );
}

#[test]
fn list_pipe_to_head_exits_cleanly() {
    let d = TempDir::new().unwrap();
    seed_50_keys(d.path());
    assert_clean_exit_on_pipe_close(&["list"], d.path());
}

#[test]
fn tags_pipe_to_head_exits_cleanly() {
    let d = TempDir::new().unwrap();
    seed_50_keys_with_tags(d.path());
    assert_clean_exit_on_pipe_close(&["tags"], d.path());
}

#[test]
fn names_pipe_to_head_exits_cleanly() {
    let d = TempDir::new().unwrap();
    seed_50_keys(d.path());
    assert_clean_exit_on_pipe_close(&["_names"], d.path());
}
