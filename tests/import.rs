use assert_cmd::Command;

fn klef_cmd(secrets: &std::path::Path, index: &std::path::Path) -> Command {
    let mut c = Command::cargo_bin("klef").unwrap();
    c.env("KLEF_TEST_BACKEND", format!("file:{}", secrets.display()));
    c.env("KLEF_INDEX_PATH", index);
    c
}

#[test]
fn import_basic_flow() {
    let d = tempfile::TempDir::new().unwrap();
    let secrets = d.path().join("secrets.json");
    let index = d.path().join("index.json");
    let envf = d.path().join(".env");
    std::fs::write(&envf, "STRIPE_API_KEY=sk_live_xyz\nPORT=3000\n").unwrap();

    klef_cmd(&secrets, &index)
        .arg("import")
        .arg(&envf)
        .arg("--yes")
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "STRIPE_API_KEY \u{2192} klef:stripe-api-key",
        ))
        .stdout(predicates::str::contains("PORT \u{2192} klef:port"))
        .stdout(predicates::str::contains("Imported 2 key"));
}

#[test]
fn import_dry_run_writes_nothing() {
    let d = tempfile::TempDir::new().unwrap();
    let secrets = d.path().join("secrets.json");
    let index = d.path().join("index.json");
    let envf = d.path().join(".env");
    std::fs::write(&envf, "API_KEY=secret\n").unwrap();

    klef_cmd(&secrets, &index)
        .arg("import")
        .arg(&envf)
        .arg("--dry-run")
        .assert()
        .success();

    // Index should NOT have been created.
    assert!(!index.exists(), "dry-run created the index file");
}

#[test]
fn import_skips_already_existing_keys() {
    let d = tempfile::TempDir::new().unwrap();
    let secrets = d.path().join("secrets.json");
    let index = d.path().join("index.json");
    let envf = d.path().join(".env");
    std::fs::write(&envf, "STRIPE=sk_live\n").unwrap();

    // First add with the name 'stripe' directly (collision target).
    klef_cmd(&secrets, &index)
        .arg("add")
        .arg("stripe")
        .write_stdin("first")
        .assert()
        .success();

    // Now import — STRIPE → klef name 'stripe' which already exists.
    klef_cmd(&secrets, &index)
        .arg("import")
        .arg(&envf)
        .arg("--yes")
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "Skipped 1 (already existed): stripe",
        ));
}

#[test]
fn import_rewrite_replaces_values_with_references() {
    let d = tempfile::TempDir::new().unwrap();
    let secrets = d.path().join("secrets.json");
    let index = d.path().join("index.json");
    let envf = d.path().join(".env");
    std::fs::write(&envf, "# comment line\nSTRIPE=sk_live_xyz\nPORT=3000\n").unwrap();

    klef_cmd(&secrets, &index)
        .arg("import")
        .arg(&envf)
        .arg("--yes")
        .arg("--rewrite")
        .assert()
        .success()
        .stdout(predicates::str::contains("Rewrote"));

    let after = std::fs::read_to_string(&envf).unwrap();
    assert!(
        after.contains("STRIPE=klef:stripe"),
        "STRIPE line not rewritten: {after}"
    );
    assert!(
        after.contains("PORT=klef:port"),
        "PORT line not rewritten: {after}"
    );
    assert!(
        after.contains("# comment line"),
        "comment was stripped: {after}"
    );
}
