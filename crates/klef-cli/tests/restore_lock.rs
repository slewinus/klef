//! Regression tests for #72: concurrent restore + concurrent mutation
//! must not corrupt the store.
//!
//! `Store::restore` now acquires the inter-process flock once and holds it
//! across the full operation (backend writes + index commit). Before the
//! fix the lock was taken and released between phases, leaving a window
//! where a parallel klef invocation could interleave and corrupt the
//! index/backend invariants.
//!
//! These tests assert the post-race INVARIANT (no corruption observable
//! through `klef list` / `klef get`) rather than precise timing — which is
//! intentionally non-deterministic. They are sufficient to flag a
//! regression where the lock window shrinks back to per-phase scope.
//!
//! Unix-only: the test harness uses POSIX flock semantics. Windows uses a
//! different (mandatory) locking primitive that behaves the same from
//! klef's perspective but isn't worth duplicating CI for.

#![cfg(unix)]

use assert_cmd::Command as AssertCommand;
use assert_cmd::cargo::CommandCargoExt;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use tempfile::TempDir;

const PASSPHRASE_CONFIRM: &str = "test-passphrase-123\ntest-passphrase-123\n";
const PASSPHRASE_RESTORE: &str = "test-passphrase-123\n";

/// Build a pre-configured `assert_cmd` command for a vault directory.
fn klef(dir: &Path) -> AssertCommand {
    let secrets = dir.join("secrets.json");
    let index = dir.join("index.json");
    let mut c = AssertCommand::cargo_bin("klef").unwrap();
    c.env("KLEF_TEST_BACKEND", format!("file:{}", secrets.display()));
    c.env("KLEF_INDEX_PATH", &index);
    c
}

/// Configure a raw `std::process::Command` for spawning concurrent klef
/// processes against the given vault dir.
fn klef_raw(dir: &Path) -> Command {
    let secrets = dir.join("secrets.json");
    let index = dir.join("index.json");
    let mut c = Command::cargo_bin("klef").unwrap();
    c.env("KLEF_TEST_BACKEND", format!("file:{}", secrets.display()));
    c.env("KLEF_INDEX_PATH", &index);
    c
}

/// Populate a vault with `n` synthetic entries.
fn populate(dir: &Path, n: usize) {
    for i in 0..n {
        klef(dir)
            .args(["add", &format!("key{i}")])
            .write_stdin(format!("value-{i}"))
            .assert()
            .success();
    }
}

/// Create a backup of `vault_dir` into `out_path` using `PASSPHRASE_CONFIRM`.
fn make_backup(vault_dir: &Path, out_path: &Path) {
    klef(vault_dir)
        .arg("backup")
        .arg(out_path)
        .write_stdin(PASSPHRASE_CONFIRM)
        .assert()
        .success();
}

/// Spawn `klef restore <bundle>` against `vault_dir`, feeding the
/// passphrase on stdin. The handle must be `wait()`ed by the caller.
fn spawn_restore(vault_dir: &Path, bundle: &Path) -> std::process::Child {
    let mut cmd = klef_raw(vault_dir);
    cmd.arg("restore").arg(bundle);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = cmd.spawn().expect("spawn restore");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(PASSPHRASE_RESTORE.as_bytes())
        .unwrap();
    child
}

/// Spawn `klef add NAME` against `vault_dir`, feeding the value on stdin.
fn spawn_add(vault_dir: &Path, name: &str, value: &str) -> std::process::Child {
    let mut cmd = klef_raw(vault_dir);
    cmd.arg("add").arg(name);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = cmd.spawn().expect("spawn add");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(value.as_bytes())
        .unwrap();
    child
}

/// Read the JSON index and return the map of keys (empty map if absent).
fn read_index_keys(vault_dir: &Path) -> serde_json::Map<String, serde_json::Value> {
    let path = vault_dir.join("index.json");
    let raw = std::fs::read_to_string(&path).expect("index must exist");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("index must be valid JSON");
    v.get("keys")
        .and_then(serde_json::Value::as_object)
        .cloned()
        .unwrap_or_default()
}

/// Two concurrent `klef restore` of the same bundle MUST NOT corrupt the
/// store. Either both succeed (lock serializes them) or one fails on the
/// lock-retry budget — never silent half-state.
#[test]
fn concurrent_restores_do_not_corrupt_index() {
    // Larger N widens the race window for phase-1 vs phase-2 interleaving.
    const N: usize = 12;

    let source = TempDir::new().unwrap();
    let bundle_dir = TempDir::new().unwrap();
    let bundle = bundle_dir.path().join("vault.age");

    populate(source.path(), N);
    make_backup(source.path(), &bundle);

    // Fresh empty vault — restore into it from two processes at once.
    let target = TempDir::new().unwrap();

    let h1 = spawn_restore(target.path(), &bundle);
    let h2 = spawn_restore(target.path(), &bundle);

    let s1 = h1.wait_with_output().unwrap();
    let s2 = h2.wait_with_output().unwrap();

    // At least one must have succeeded (the bundle has no conflict on an
    // empty target, so the only failure mode is lock contention).
    assert!(
        s1.status.success() || s2.status.success(),
        "at least one of the two restores must succeed"
    );

    // Invariant: the index has EXACTLY the bundle's keys — no fewer (lost
    // writes from phase-1/phase-2 interleaving), no extras (corruption).
    let keys = read_index_keys(target.path());
    assert_eq!(
        keys.len(),
        N,
        "index must hold exactly {N} keys after concurrent restores, got {}",
        keys.len()
    );
    for i in 0..N {
        assert!(
            keys.contains_key(&format!("key{i}")),
            "key{i} missing from index — interleaving corrupted the restore"
        );
    }

    // Backend must also resolve every key (no orphan in index without a
    // backend value).
    for i in 0..N {
        klef(target.path())
            .args(["get", &format!("key{i}")])
            .assert()
            .success();
    }
}

/// A concurrent `klef add` racing a `klef restore` MUST NOT leave the
/// index in a state where some bundle keys are missing because they were
/// stomped by the add's index rewrite (the bug #72 specifically calls
/// out).
#[test]
fn restore_racing_with_add_keeps_bundle_intact() {
    const N: usize = 10;

    let source = TempDir::new().unwrap();
    let bundle_dir = TempDir::new().unwrap();
    let bundle = bundle_dir.path().join("vault.age");

    populate(source.path(), N);
    make_backup(source.path(), &bundle);

    let target = TempDir::new().unwrap();

    // Race: restore the N-entry bundle while another klef tries to insert
    // its own key. Pre-fix, the add could land its index save BETWEEN
    // restore's phase 1 (backend writes done) and phase 2 (index commit)
    // — silently overwriting the restore's index write window.
    let restore_h = spawn_restore(target.path(), &bundle);
    let add_h = spawn_add(target.path(), "concurrent_add", "concurrent-val");

    let r_out = restore_h.wait_with_output().unwrap();
    let a_out = add_h.wait_with_output().unwrap();

    // Restore must have succeeded against an empty target (no conflict).
    // Add may or may not have succeeded depending on lock-retry budget;
    // either outcome is acceptable as long as the invariant holds below.
    assert!(
        r_out.status.success(),
        "restore must succeed against an empty target"
    );

    let keys = read_index_keys(target.path());

    // Invariant: every bundle key is present. If `add` won the race AFTER
    // restore, the index has N+1 keys (N bundle + concurrent_add). If
    // `add` failed on lock contention, the index has exactly N. What's
    // forbidden: any bundle key missing.
    for i in 0..N {
        assert!(
            keys.contains_key(&format!("key{i}")),
            "key{i} missing — restore/add interleaving lost a bundle key"
        );
    }

    // If `add` reports success its key must actually be in the index
    // (otherwise success was a lie).
    if a_out.status.success() {
        assert!(
            keys.contains_key("concurrent_add"),
            "klef add reported success but its key is missing from the index"
        );
    }
}
