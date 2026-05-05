use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;
use tempfile::TempDir;

/// Build a `klef` command pre-configured with isolated index + secrets paths.
/// Each test owns a tempdir that's cleaned up on drop.
fn klef(dir: &Path) -> Command {
    let secrets = dir.join("secrets.json");
    let index = dir.join("index.json");
    let mut c = Command::cargo_bin("klef").unwrap();
    c.env("KLEF_TEST_BACKEND", format!("file:{}", secrets.display()));
    c.env("KLEF_INDEX_PATH", &index);
    c
}

#[test]
fn add_get_list_rm_round_trip() {
    let d = TempDir::new().unwrap();

    klef(d.path())
        .arg("add")
        .arg("stripe")
        .write_stdin("sk_live")
        .assert()
        .success();

    klef(d.path())
        .arg("get")
        .arg("stripe")
        .assert()
        .success()
        .stdout(predicate::str::contains("sk_live"));

    klef(d.path())
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("stripe"));

    klef(d.path())
        .arg("rm")
        .arg("stripe")
        .arg("--yes")
        .assert()
        .success();

    klef(d.path())
        .arg("get")
        .arg("stripe")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn export_emits_shell_export() {
    let d = TempDir::new().unwrap();
    klef(d.path())
        .arg("add")
        .arg("stripe")
        .write_stdin("v")
        .assert()
        .success();
    klef(d.path())
        .arg("export")
        .arg("stripe")
        .assert()
        .success()
        .stdout("export STRIPE_API_KEY=v\n");
}

#[test]
fn run_resolves_references() {
    let d = TempDir::new().unwrap();
    let envf = d.path().join(".env");
    std::fs::write(&envf, "STRIPE_KEY=klef:stripe\nPORT=3000\n").unwrap();

    klef(d.path())
        .arg("add")
        .arg("stripe")
        .write_stdin("sk_live")
        .assert()
        .success();

    klef(d.path())
        .arg("run")
        .arg("--env-file")
        .arg(&envf)
        .arg("--")
        .arg("/bin/sh")
        .arg("-c")
        .arg("printf '%s|%s' \"$STRIPE_KEY\" \"$PORT\"")
        .assert()
        .success()
        .stdout("sk_live|3000");
}

#[test]
fn run_with_broken_reference_exits_3() {
    let d = TempDir::new().unwrap();
    let envf = d.path().join(".env");
    std::fs::write(&envf, "X=klef:missing\n").unwrap();

    klef(d.path())
        .arg("run")
        .arg("--env-file")
        .arg(&envf)
        .arg("--")
        .arg("/bin/echo")
        .arg("hi")
        .assert()
        .failure()
        .code(3);
}

#[test]
fn rename_moves_key() {
    let d = TempDir::new().unwrap();
    klef(d.path())
        .arg("add")
        .arg("foo")
        .write_stdin("v")
        .assert()
        .success();
    klef(d.path())
        .arg("rename")
        .arg("foo")
        .arg("bar")
        .assert()
        .success();
    klef(d.path())
        .arg("get")
        .arg("bar")
        .assert()
        .success()
        .stdout(predicate::str::contains("v"));
    klef(d.path())
        .arg("get")
        .arg("foo")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn version_flag_prints_crate_version() {
    use assert_cmd::Command;
    Command::cargo_bin("klef")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicates::str::starts_with("klef "))
        .stdout(predicates::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn version_short_flag_prints_crate_version() {
    use assert_cmd::Command;
    Command::cargo_bin("klef")
        .unwrap()
        .arg("-V")
        .assert()
        .success()
        .stdout(predicates::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn completions_zsh_emits_compdef() {
    use assert_cmd::Command;
    Command::cargo_bin("klef")
        .unwrap()
        .arg("completions")
        .arg("zsh")
        .assert()
        .success()
        .stdout(predicates::str::starts_with("#compdef klef"));
}

#[test]
fn completions_bash_emits_function() {
    use assert_cmd::Command;
    Command::cargo_bin("klef")
        .unwrap()
        .arg("completions")
        .arg("bash")
        .assert()
        .success()
        .stdout(predicates::str::contains("_klef()"));
}

#[test]
fn add_with_positional_value_gives_helpful_error() {
    let d = TempDir::new().unwrap();
    let secrets = d.path().join("secrets.json");
    let index = d.path().join("index.json");

    Command::cargo_bin("klef")
        .unwrap()
        .env("KLEF_TEST_BACKEND", format!("file:{}", secrets.display()))
        .env("KLEF_INDEX_PATH", &index)
        .arg("add")
        .arg("stripe")
        .arg("sk_live_xyz")
        .assert()
        .failure()
        .code(64)
        .stderr(predicates::str::contains(
            "reads the secret value from a TTY prompt or stdin",
        ))
        .stderr(predicates::str::contains("echo -n value | klef add"));
}

#[test]
fn edit_with_positional_value_gives_helpful_error() {
    let d = TempDir::new().unwrap();
    let secrets = d.path().join("secrets.json");
    let index = d.path().join("index.json");

    Command::cargo_bin("klef")
        .unwrap()
        .env("KLEF_TEST_BACKEND", format!("file:{}", secrets.display()))
        .env("KLEF_INDEX_PATH", &index)
        .arg("edit")
        .arg("stripe")
        .arg("newvalue")
        .assert()
        .failure()
        .code(64)
        .stderr(predicates::str::contains("klef edit"));
}

#[test]
fn status_text_output_healthy() {
    let d = TempDir::new().unwrap();

    klef(d.path())
        .arg("add")
        .arg("stripe")
        .write_stdin("v")
        .assert()
        .success();

    klef(d.path())
        .arg("status")
        .assert()
        .success()
        .stdout(predicates::str::contains("keys         1"))
        .stdout(predicates::str::contains("desync       none"));
}

#[test]
fn status_json_output() {
    let d = TempDir::new().unwrap();

    klef(d.path())
        .arg("status")
        .arg("--format")
        .arg("json")
        .assert()
        .success()
        .stdout(predicates::str::contains("\"klef_version\":"))
        .stdout(predicates::str::contains("\"keys\": 0"));
}

#[test]
fn status_detects_desync_and_exits_1() {
    use std::fs;
    let d = TempDir::new().unwrap();
    let secrets = d.path().join("secrets.json");

    klef(d.path())
        .arg("add")
        .arg("orphan")
        .write_stdin("v")
        .assert()
        .success();

    fs::write(&secrets, "{\"secrets\":{}}").unwrap();

    klef(d.path())
        .arg("status")
        .assert()
        .failure()
        .code(1)
        .stdout(predicates::str::contains("orphan(s) in index: orphan"));
}
