//! Tests for the hidden `klef _names` helper used by shell completion.

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
fn names_prints_one_per_line() {
    let d = TempDir::new().unwrap();

    klef(d.path())
        .arg("add")
        .arg("alpha")
        .write_stdin("v")
        .assert()
        .success();
    klef(d.path())
        .arg("add")
        .arg("beta")
        .write_stdin("v")
        .assert()
        .success();
    klef(d.path())
        .arg("add")
        .arg("gamma")
        .write_stdin("v")
        .assert()
        .success();

    let assert = klef(d.path()).arg("_names").assert().success();
    let out = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines, vec!["alpha", "beta", "gamma"]);
}

#[test]
fn names_on_empty_index_prints_nothing() {
    let d = TempDir::new().unwrap();
    let assert = klef(d.path()).arg("_names").assert().success();
    let out = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(out.is_empty(), "expected empty output, got: {out:?}");
}

#[test]
fn names_is_hidden_from_help() {
    klef(TempDir::new().unwrap().path())
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::function::function(|out: &str| {
            !out.contains("_names")
        }));
}

#[test]
fn completions_zsh_includes_klef_names_function() {
    klef(TempDir::new().unwrap().path())
        .arg("completions")
        .arg("zsh")
        .assert()
        .success()
        .stdout(predicates::str::contains("_klef_names()"));
}

#[test]
fn completions_bash_includes_klef_names_function() {
    klef(TempDir::new().unwrap().path())
        .arg("completions")
        .arg("bash")
        .assert()
        .success()
        .stdout(predicates::str::contains("_klef_names()"));
}

#[test]
fn completions_bash_uses_klef_names_for_positionals() {
    klef(TempDir::new().unwrap().path())
        .arg("completions")
        .arg("bash")
        .assert()
        .success()
        .stdout(predicates::str::contains(
            r#"COMPREPLY=( $(compgen -W "$(_klef_names)" -- "${cur}") )"#,
        ));
}

#[test]
fn completions_fish_uses_klef_names_command() {
    klef(TempDir::new().unwrap().path())
        .arg("completions")
        .arg("fish")
        .assert()
        .success()
        .stdout(predicates::str::contains("(klef _names)"));
}
