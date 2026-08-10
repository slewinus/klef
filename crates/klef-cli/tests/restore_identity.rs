//! `klef restore --identity` — reading back a backup made with `--recipient`.
//!
//! Before this, `klef backup --recipient age1...` printed a success line and
//! wrote a file that klef could never read again: `restore` bailed out on any
//! non-scrypt age file. A user following the flag's own help text ended up with
//! a backup they could not restore, and no warning until the day they needed it.

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

/// Write a fresh age keypair to `<dir>/<name>` and return (path, recipient).
fn keypair(dir: &Path, name: &str) -> (std::path::PathBuf, String) {
    use age::secrecy::ExposeSecret as _;
    let id = age::x25519::Identity::generate();
    let recipient = id.to_public().to_string();
    let path = dir.join(name);
    std::fs::write(&path, format!("{}\n", id.to_string().expose_secret())).unwrap();
    (path, recipient)
}

/// Seed one key so there is something to back up.
fn seed(dir: &Path) {
    klef(dir)
        .args(["add", "stripe", "--as", "STRIPE_KEY"])
        .write_stdin("sk_live_roundtrip")
        .assert()
        .success();
}

#[test]
fn recipient_backup_round_trips_through_identity_restore() {
    let d = TempDir::new().unwrap();
    seed(d.path());
    let (id_path, recipient) = keypair(d.path(), "id.txt");
    let backup = d.path().join("out.age");

    klef(d.path())
        .args(["backup"])
        .arg(&backup)
        .args(["--recipient", &recipient])
        .assert()
        .success();

    klef(d.path())
        .args(["rm", "stripe", "--yes"])
        .assert()
        .success();

    klef(d.path())
        .args(["restore"])
        .arg(&backup)
        .arg("--identity")
        .arg(&id_path)
        .assert()
        .success();

    klef(d.path())
        .args(["get", "stripe"])
        .assert()
        .success()
        .stdout(predicate::str::contains("sk_live_roundtrip"));

    // Metadata has to survive too, not just the secret.
    klef(d.path())
        .args(["export", "stripe", "--format", "dotenv"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("STRIPE_KEY="));
}

#[test]
fn recipient_backup_without_identity_says_what_to_pass() {
    let d = TempDir::new().unwrap();
    seed(d.path());
    let (_, recipient) = keypair(d.path(), "id.txt");
    let backup = d.path().join("out.age");

    klef(d.path())
        .args(["backup"])
        .arg(&backup)
        .args(["--recipient", &recipient])
        .assert()
        .success();

    klef(d.path())
        .args(["restore"])
        .arg(&backup)
        .arg("--force")
        .assert()
        .failure()
        .stderr(predicate::str::contains("--identity"));
}

#[test]
fn a_non_matching_identity_is_rejected() {
    let d = TempDir::new().unwrap();
    seed(d.path());
    let (_, recipient) = keypair(d.path(), "id.txt");
    let (other_path, _) = keypair(d.path(), "other.txt");
    let backup = d.path().join("out.age");

    klef(d.path())
        .args(["backup"])
        .arg(&backup)
        .args(["--recipient", &recipient])
        .assert()
        .success();

    klef(d.path())
        .args(["restore"])
        .arg(&backup)
        .arg("--force")
        .arg("--identity")
        .arg(&other_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("None of the supplied identities"));
}

#[test]
fn identity_on_a_passphrase_backup_is_refused() {
    let d = TempDir::new().unwrap();
    seed(d.path());
    let (id_path, _) = keypair(d.path(), "id.txt");
    let backup = d.path().join("pass.age");

    // No --recipient → passphrase mode, prompted twice for confirmation.
    klef(d.path())
        .args(["backup"])
        .arg(&backup)
        .write_stdin("pw\npw\n")
        .assert()
        .success();

    // Refused up front rather than silently ignoring the flag and then
    // prompting for a passphrase the user didn't expect to need.
    klef(d.path())
        .args(["restore"])
        .arg(&backup)
        .arg("--force")
        .arg("--identity")
        .arg(&id_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("passphrase-encrypted"));
}

#[test]
fn an_unreadable_identity_file_is_reported_by_path() {
    let d = TempDir::new().unwrap();
    seed(d.path());
    let (_, recipient) = keypair(d.path(), "id.txt");
    let backup = d.path().join("out.age");
    let missing = d.path().join("does-not-exist.txt");

    klef(d.path())
        .args(["backup"])
        .arg(&backup)
        .args(["--recipient", &recipient])
        .assert()
        .success();

    klef(d.path())
        .args(["restore"])
        .arg(&backup)
        .arg("--force")
        .arg("--identity")
        .arg(&missing)
        .assert()
        .failure()
        .stderr(predicate::str::contains("does-not-exist.txt"));
}
