//! Integration tests for `klef discover`.

use assert_cmd::Command;
use predicates::str as pstr;
use std::fs;
use tempfile::TempDir;

fn klef(dir: &std::path::Path) -> Command {
    let secrets = dir.join("secrets.json");
    let index = dir.join("index.json");
    let mut c = Command::cargo_bin("klef").unwrap();
    c.env("KLEF_TEST_BACKEND", format!("file:{}", secrets.display()));
    c.env("KLEF_INDEX_PATH", &index);
    c
}

fn make_proj(root: &std::path::Path, name: &str, env_content: &str) {
    let proj = root.join(name);
    fs::create_dir_all(&proj).unwrap();
    fs::write(proj.join(".env"), env_content).unwrap();
}

#[test]
fn discover_finds_env_files_in_subdirs() {
    let d = TempDir::new().unwrap();
    let scan_root = d.path().join("projects");
    fs::create_dir(&scan_root).unwrap();
    make_proj(&scan_root, "alpha", "STRIPE_KEY=sk_live\n");
    make_proj(&scan_root, "beta", "OPENAI_KEY=sk-test\n");

    klef(d.path())
        .arg("discover")
        .arg("--root")
        .arg(&scan_root)
        .arg("--yes")
        .assert()
        .success()
        .stdout(pstr::contains("STRIPE_KEY"))
        .stdout(pstr::contains("stripe-key"))
        .stdout(pstr::contains("OPENAI_KEY"))
        .stdout(pstr::contains("openai-key"))
        .stdout(pstr::contains("Imported 2 key"));
}

#[test]
fn discover_dry_run_writes_nothing() {
    let d = TempDir::new().unwrap();
    let scan_root = d.path().join("projects");
    fs::create_dir(&scan_root).unwrap();
    make_proj(&scan_root, "alpha", "X=secret\n");

    klef(d.path())
        .arg("discover")
        .arg("--root")
        .arg(&scan_root)
        .arg("--dry-run")
        .assert()
        .success();

    assert!(
        !d.path().join("index.json").exists(),
        "dry-run should not create index"
    );
}

#[test]
fn discover_skips_node_modules() {
    let d = TempDir::new().unwrap();
    let scan_root = d.path().join("projects/myapp");
    fs::create_dir_all(scan_root.join("node_modules/foo")).unwrap();
    fs::write(scan_root.join("node_modules/foo/.env"), "BAD=secret\n").unwrap();
    fs::write(scan_root.join(".env"), "GOOD=secret\n").unwrap();

    klef(d.path())
        .arg("discover")
        .arg("--root")
        .arg(&scan_root)
        .arg("--yes")
        .assert()
        .success()
        .stdout(pstr::contains("GOOD"))
        .stdout(pstr::contains("good"))
        .stdout(predicates::function::function(|out: &str| {
            !out.contains("BAD")
        }));
}

#[test]
fn discover_dedups_across_files() {
    let d = TempDir::new().unwrap();
    let scan_root = d.path().join("projects");
    fs::create_dir(&scan_root).unwrap();
    make_proj(&scan_root, "alpha", "STRIPE=first\n");
    make_proj(&scan_root, "beta", "STRIPE=second\n");

    klef(d.path())
        .arg("discover")
        .arg("--root")
        .arg(&scan_root)
        .arg("--yes")
        .assert()
        .success()
        .stdout(pstr::contains("1 conflict(s) resolved"));

    // The default first-found means alpha's value won (alpha < beta alphabetically).
    klef(d.path())
        .arg("get")
        .arg("stripe")
        .assert()
        .success()
        .stdout(pstr::contains("first"));
}

#[test]
fn discover_respects_last_found_conflict_mode() {
    let d = TempDir::new().unwrap();
    let scan_root = d.path().join("projects");
    fs::create_dir(&scan_root).unwrap();
    make_proj(&scan_root, "alpha", "STRIPE=first\n");
    make_proj(&scan_root, "beta", "STRIPE=second\n");

    klef(d.path())
        .arg("discover")
        .arg("--root")
        .arg(&scan_root)
        .arg("--on-conflict")
        .arg("last-found")
        .arg("--yes")
        .assert()
        .success();

    klef(d.path())
        .arg("get")
        .arg("stripe")
        .assert()
        .success()
        .stdout(pstr::contains("second"));
}

#[test]
fn discover_prints_no_files_message_on_empty_root() {
    let d = TempDir::new().unwrap();
    let scan_root = d.path().join("empty");
    fs::create_dir(&scan_root).unwrap();

    klef(d.path())
        .arg("discover")
        .arg("--root")
        .arg(&scan_root)
        .arg("--yes")
        .assert()
        .success()
        .stdout(pstr::contains("no .env files found"));
}

#[test]
fn discover_skip_pattern_excludes_matches() {
    let d = TempDir::new().unwrap();
    let scan_root = d.path().join("projects");
    fs::create_dir(&scan_root).unwrap();
    let proj = scan_root.join("alpha");
    fs::create_dir(&proj).unwrap();
    fs::write(
        proj.join(".env"),
        "STRIPE_API_KEY=secret\nPORT=3000\nDB_NAME=app\n",
    )
    .unwrap();

    klef(d.path())
        .arg("discover")
        .arg("--root")
        .arg(&scan_root)
        .arg("--skip-pattern")
        .arg(r"^(PORT|DB_NAME)$")
        .arg("--yes")
        .assert()
        .success()
        .stdout(pstr::contains("STRIPE_API_KEY → klef:stripe-api-key"))
        .stdout(pstr::contains("2 skipped by pattern"))
        .stdout(predicates::function::function(|out: &str| {
            !out.contains("klef:port") && !out.contains("klef:db-name")
        }));
}

#[test]
fn discover_skip_defaults_excludes_common_config() {
    let d = TempDir::new().unwrap();
    let scan_root = d.path().join("projects");
    fs::create_dir(&scan_root).unwrap();
    let proj = scan_root.join("alpha");
    fs::create_dir(&proj).unwrap();
    fs::write(
        proj.join(".env"),
        "STRIPE_API_KEY=secret\nPORT=3000\nDB_PORT=5432\nNODE_ENV=production\nDEBUG=true\n",
    )
    .unwrap();

    klef(d.path())
        .arg("discover")
        .arg("--root")
        .arg(&scan_root)
        .arg("--skip-defaults")
        .arg("--yes")
        .assert()
        .success()
        .stdout(pstr::contains("STRIPE_API_KEY → klef:stripe-api-key"))
        .stdout(pstr::contains("4 skipped by pattern"))
        .stdout(predicates::function::function(|out: &str| {
            !out.contains("klef:port") && !out.contains("klef:db-port")
        }));
}

#[test]
fn discover_invalid_skip_pattern_returns_error() {
    let d = TempDir::new().unwrap();
    let scan_root = d.path().join("projects");
    fs::create_dir(&scan_root).unwrap();

    klef(d.path())
        .arg("discover")
        .arg("--root")
        .arg(&scan_root)
        .arg("--skip-pattern")
        .arg("[unbalanced")
        .arg("--yes")
        .assert()
        .failure()
        .stderr(pstr::contains("invalid skip pattern"));
}
