//! `klef run` must not hand klef's own secrets to the child process.
//!
//! `KLEF_PASSPHRASE` unlocks the entire age vault. Before the fix, the child
//! inherited it from klef's environment — so `klef run -- npm start` gave the
//! vault master key to npm and to every postinstall script under it.

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

/// Seed one key and a `.env` that references it. Returns the env-file path.
fn seed(dir: &Path) -> std::path::PathBuf {
    klef(dir)
        .args(["add", "stripe"])
        .write_stdin("sk_live_scrub_probe")
        .assert()
        .success();

    let env_file = dir.join(".env");
    std::fs::write(&env_file, "STRIPE_API_KEY=klef:stripe\n").unwrap();
    env_file
}

#[test]
fn run_does_not_leak_passphrase_to_child() {
    let d = TempDir::new().unwrap();
    let env_file = seed(d.path());

    klef(d.path())
        .env("KLEF_PASSPHRASE", "correct-horse-battery-staple")
        .args(["run", "--env-file"])
        .arg(&env_file)
        .args(["--", "sh", "-c", "echo pass=[$KLEF_PASSPHRASE]"])
        .assert()
        .success()
        .stdout(predicate::str::contains("pass=[]"))
        .stdout(predicate::str::contains("correct-horse-battery-staple").not());
}

#[test]
fn run_still_injects_resolved_references() {
    let d = TempDir::new().unwrap();
    let env_file = seed(d.path());

    // Scrubbing must not disturb the vars klef is there to inject, nor the
    // inherited environment the child legitimately needs (PATH found `sh`).
    klef(d.path())
        .env("KLEF_PASSPHRASE", "correct-horse-battery-staple")
        .args(["run", "--env-file"])
        .arg(&env_file)
        .args(["--", "sh", "-c", "echo key=[$STRIPE_API_KEY]"])
        .assert()
        .success()
        .stdout(predicate::str::contains("key=[sk_live_scrub_probe]"));
}
