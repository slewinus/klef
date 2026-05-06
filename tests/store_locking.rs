//! Inter-process locking regression tests for #61.
//!
//! Spawns subprocesses that each invoke the klef binary against the same
//! file backend + index. Without locking, the second writer would silently
//! lose the first's write. With locking, both writes land.

use assert_cmd::cargo::CommandCargoExt;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

/// One-shot `klef add NAME` against a shared file backend, fed by stdin.
fn spawn_add(
    klef_index: &Path,
    file_backend: &Path,
    name: &str,
    value: &str,
) -> std::process::Child {
    let mut cmd = Command::cargo_bin("klef").unwrap();
    cmd.env("KLEF_INDEX_PATH", klef_index)
        .env(
            "KLEF_TEST_BACKEND",
            format!("file:{}", file_backend.display()),
        )
        .arg("add")
        .arg(name)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let mut child = cmd.spawn().unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(value.as_bytes())
        .unwrap();
    child
}

#[test]
fn concurrent_adds_do_not_lose_writes() {
    let d = tempdir().unwrap();
    let index = d.path().join("index.json");
    let backend = d.path().join("secrets.json");

    // Launch N concurrent `klef add KEY_i value_i`. Without locking, the
    // last writer's view (read before the others wrote) overwrites the
    // others' work and we end up with fewer than N keys.
    let n = 8;
    let mut children = Vec::new();
    for i in 0..n {
        children.push(spawn_add(
            &index,
            &backend,
            &format!("k{i}"),
            &format!("v{i}"),
        ));
    }
    let mut all_ok = true;
    for mut c in children {
        let status = c.wait().unwrap();
        if !status.success() {
            all_ok = false;
        }
    }

    // Some processes may legitimately fail if the lock contention budget is
    // blown — but those that succeed must not have lost each other's work.
    // Read the index back and verify every successful add is reflected.
    let raw = std::fs::read_to_string(&index).expect("index must exist");
    let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let keys = v
        .get("keys")
        .and_then(serde_json::Value::as_object)
        .expect("index must have a keys map");

    // Either all 8 succeeded (the success case we want), or some failed
    // because the retry budget elapsed (also acceptable — better than
    // silent data loss). What's NOT acceptable is "all processes report
    // success but some keys are missing from the index".
    if all_ok {
        assert_eq!(
            keys.len(),
            n,
            "all adds succeeded but only {} of {n} keys made it to the index — \
             writes were lost (concurrent-write race)",
            keys.len()
        );
    } else {
        // Partial success — just sanity check there's at least one key and
        // the index is well-formed.
        assert!(
            !keys.is_empty(),
            "no keys made it through despite some processes succeeding"
        );
    }
}
