//! Integration tests for `klef backup` and `klef restore`.

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;
use tempfile::TempDir;

/// Build a `klef` command pre-configured with isolated index + secrets paths.
fn klef(dir: &Path) -> Command {
    let secrets = dir.join("secrets.json");
    let index = dir.join("index.json");
    let mut c = Command::cargo_bin("klef").unwrap();
    c.env("KLEF_TEST_BACKEND", format!("file:{}", secrets.display()));
    c.env("KLEF_INDEX_PATH", &index);
    c
}

/// Add N entries to a fresh vault.
fn populate(dir: &Path, entries: &[(&str, &str, &str)]) {
    for (name, value, note) in entries {
        klef(dir)
            .args(["add", name, "--note", note])
            .write_stdin(*value)
            .assert()
            .success();
    }
}

const PASSPHRASE_CONFIRM: &str = "test-passphrase-123\ntest-passphrase-123\n";
const PASSPHRASE_RESTORE: &str = "test-passphrase-123\n";

#[test]
fn backup_then_restore_round_trip() {
    let vault_dir = TempDir::new().unwrap();
    let out_dir = TempDir::new().unwrap();
    let backup_path = out_dir.path().join("vault.age");

    // 1. Populate vault with 3 entries.
    populate(
        vault_dir.path(),
        &[
            ("stripe-prod", "sk_live_xxxxx", "stripe account"),
            ("anthropic", "sk-ant-xxx", "llm key"),
            ("github-token", "ghp_xxxx", "ci token"),
        ],
    );

    // 2. Backup with passphrase.
    klef(vault_dir.path())
        .arg("backup")
        .arg(&backup_path)
        .write_stdin(PASSPHRASE_CONFIRM)
        .assert()
        .success()
        .stdout(predicate::str::contains("3 entries"));

    assert!(backup_path.exists(), "backup file should exist");

    // 3. Wipe the vault (delete index + secrets).
    let _ = std::fs::remove_file(vault_dir.path().join("index.json"));
    let _ = std::fs::remove_file(vault_dir.path().join("secrets.json"));

    // 4. Restore.
    klef(vault_dir.path())
        .arg("restore")
        .arg(&backup_path)
        .write_stdin(PASSPHRASE_RESTORE)
        .assert()
        .success()
        .stdout(predicate::str::contains("3 entries written"));

    // 5. Verify all entries are back.
    klef(vault_dir.path())
        .arg("get")
        .arg("stripe-prod")
        .assert()
        .success()
        .stdout(predicate::str::contains("sk_live_xxxxx"));

    klef(vault_dir.path())
        .arg("get")
        .arg("anthropic")
        .assert()
        .success()
        .stdout(predicate::str::contains("sk-ant-xxx"));

    klef(vault_dir.path())
        .arg("get")
        .arg("github-token")
        .assert()
        .success()
        .stdout(predicate::str::contains("ghp_xxxx"));

    // 6. Verify metadata (note) is preserved.
    klef(vault_dir.path())
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("stripe account"));
}

#[test]
fn backup_empty_vault_is_valid() {
    let vault_dir = TempDir::new().unwrap();
    let out_dir = TempDir::new().unwrap();
    let backup_path = out_dir.path().join("empty.age");

    // Backup empty vault.
    klef(vault_dir.path())
        .arg("backup")
        .arg(&backup_path)
        .write_stdin(PASSPHRASE_CONFIRM)
        .assert()
        .success()
        .stdout(predicate::str::contains("0 entries"));

    // Fresh vault dir for restore.
    let restore_dir = TempDir::new().unwrap();

    klef(restore_dir.path())
        .arg("restore")
        .arg(&backup_path)
        .write_stdin(PASSPHRASE_RESTORE)
        .assert()
        .success()
        .stdout(predicate::str::contains("0 entries written"));

    klef(restore_dir.path()).arg("list").assert().success();
}

#[test]
fn restore_with_conflict_without_force_aborts() {
    let vault_a = TempDir::new().unwrap();
    let vault_b = TempDir::new().unwrap();
    let out_dir = TempDir::new().unwrap();
    let backup_path = out_dir.path().join("vault_a.age");

    // Vault A: stripe-prod
    populate(vault_a.path(), &[("stripe-prod", "sk_live_a", "")]);

    // Backup vault A.
    klef(vault_a.path())
        .arg("backup")
        .arg(&backup_path)
        .write_stdin(PASSPHRASE_CONFIRM)
        .assert()
        .success();

    // Vault B: also has stripe-prod (conflict).
    populate(vault_b.path(), &[("stripe-prod", "sk_live_b", "")]);

    // Restore A into B — should fail with conflict error.
    klef(vault_b.path())
        .arg("restore")
        .arg(&backup_path)
        .write_stdin(PASSPHRASE_RESTORE)
        .assert()
        .failure()
        .stderr(predicate::str::contains("conflict"));

    // B's stripe-prod should still have the original value.
    klef(vault_b.path())
        .arg("get")
        .arg("stripe-prod")
        .assert()
        .success()
        .stdout(predicate::str::contains("sk_live_b"));
}

#[test]
fn restore_with_conflict_and_force_overwrites() {
    let vault_a = TempDir::new().unwrap();
    let vault_b = TempDir::new().unwrap();
    let out_dir = TempDir::new().unwrap();
    let backup_path = out_dir.path().join("vault_a.age");

    populate(vault_a.path(), &[("stripe-prod", "sk_live_a", "")]);

    klef(vault_a.path())
        .arg("backup")
        .arg(&backup_path)
        .write_stdin(PASSPHRASE_CONFIRM)
        .assert()
        .success();

    populate(vault_b.path(), &[("stripe-prod", "sk_live_b", "")]);

    // Restore with --force: should overwrite.
    klef(vault_b.path())
        .arg("restore")
        .arg(&backup_path)
        .arg("--force")
        .write_stdin(PASSPHRASE_RESTORE)
        .assert()
        .success()
        .stdout(predicate::str::contains("1 entries written"));

    // B's stripe-prod should now have A's value.
    klef(vault_b.path())
        .arg("get")
        .arg("stripe-prod")
        .assert()
        .success()
        .stdout(predicate::str::contains("sk_live_a"));
}

/// Encrypt a JSON blob directly with age (passphrase), writing to `path`.
fn write_age_bundle(json: &[u8], path: &std::path::Path) {
    use age::secrecy::SecretString;
    use std::io::Write as _;

    let encryptor =
        age::Encryptor::with_user_passphrase(SecretString::from("test-passphrase-123".to_owned()));
    let mut ct = Vec::new();
    let mut writer = encryptor.wrap_output(&mut ct).unwrap();
    writer.write_all(json).unwrap();
    writer.finish().unwrap();
    std::fs::write(path, &ct).unwrap();
}

#[test]
fn restore_rejects_unsupported_format_version() {
    use klef::commands::backup::{Bundle, BundleSource};
    use time::macros::datetime;

    let out_dir = TempDir::new().unwrap();
    let backup_path = out_dir.path().join("future.age");

    let bundle = Bundle {
        format_version: 999,
        tool: "klef".to_string(),
        klef_version: "99.0.0".to_string(),
        created_at: datetime!(2026-05-06 12:00:00 UTC),
        source: BundleSource {
            hostname: "test".to_string(),
            platform: "macos".to_string(),
        },
        entries: vec![],
    };
    let json = serde_json::to_vec(&bundle).unwrap();
    write_age_bundle(&json, &backup_path);

    let restore_dir = TempDir::new().unwrap();
    klef(restore_dir.path())
        .arg("restore")
        .arg(&backup_path)
        .write_stdin(PASSPHRASE_RESTORE)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsupported format_version"));
}

#[test]
fn restore_rejects_non_klef_tool() {
    use klef::commands::backup::{Bundle, BundleSource};
    use time::macros::datetime;

    let out_dir = TempDir::new().unwrap();
    let backup_path = out_dir.path().join("not_klef.age");

    let bundle = Bundle {
        format_version: 1,
        tool: "not-klef".to_string(),
        klef_version: "0.2.0".to_string(),
        created_at: datetime!(2026-05-06 12:00:00 UTC),
        source: BundleSource {
            hostname: "test".to_string(),
            platform: "macos".to_string(),
        },
        entries: vec![],
    };
    let json = serde_json::to_vec(&bundle).unwrap();
    write_age_bundle(&json, &backup_path);

    let restore_dir = TempDir::new().unwrap();
    klef(restore_dir.path())
        .arg("restore")
        .arg(&backup_path)
        .write_stdin(PASSPHRASE_RESTORE)
        .assert()
        .failure()
        .stderr(predicate::str::contains("not a klef backup"));
}
