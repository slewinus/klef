//! Integration-style tests for `tools.rs`. Lives in a sibling file (loaded
//! via `#[path]`) to keep `tools.rs` under the 300-line cap.

use super::*;
use crate::commands::mcp::policy;
use std::sync::{Mutex, OnceLock};
use tempfile::TempDir;

/// Tests share process-wide env vars (`KLEF_INDEX_PATH`, `KLEF_TEST_BACKEND`).
/// Serialize via this mutex while building the store; once `Store` is
/// constructed it no longer reads env, so the guard is dropped before any
/// `.await` to satisfy clippy's `await_holding_lock`.
fn env_lock() -> &'static Mutex<()> {
    static M: OnceLock<Mutex<()>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(()))
}

fn ctx_for_tests(rules_toml: &str) -> (Ctx, TempDir) {
    let _guard = env_lock()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let tmp = TempDir::new().unwrap();
    // SAFETY: tests are single-threaded per #[tokio::test] (current_thread runtime
    // unless flavored). We set env before constructing the store and never read
    // these vars from concurrent threads. Required because workspace lints `deny`
    // unsafe and `set_var` is unsafe in Rust 2024.
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("KLEF_INDEX_PATH", tmp.path().join("index.json"));
    }
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var(
            "KLEF_TEST_BACKEND",
            format!("file:{}", tmp.path().join("vault").display()),
        );
    }
    let store = Arc::new(klef_core::build_store(None).unwrap());
    store
        .add("stripe", "sk_live_abcdefg", None, None, Vec::new(), false)
        .unwrap();
    let pol_path = tmp.path().join("p.toml");
    std::fs::write(&pol_path, rules_toml).unwrap();
    let policy = Arc::new(policy::load(&pol_path).unwrap());
    let audit = Audit::new(tmp.path().join("audit.log"));
    (
        Ctx {
            store,
            policy,
            audit,
        },
        tmp,
    )
}

#[tokio::test]
async fn klef_list_returns_metadata_and_filters() {
    let (ctx, _tmp) = ctx_for_tests("");
    let v = klef_list(&ctx, ListInput::default()).await.unwrap();
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].name, "stripe");
    let v2 = klef_list(
        &ctx,
        ListInput {
            filter: Some("nope".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(v2.is_empty());
}

#[tokio::test]
async fn klef_run_deny_no_rule_match() {
    let (ctx, _tmp) = ctx_for_tests("");
    let r = klef_run(
        &ctx,
        RunInput {
            argv: vec!["echo".into(), "hi".into()],
            env_refs: vec![],
            cwd: None,
            timeout_ms: None,
        },
    )
    .await;
    assert!(matches!(r, Err(ToolError::Policy(_))));
}

#[tokio::test]
async fn klef_run_allow_redacts_secret() {
    let toml = r#"
        [[allow]]
        argv = ["printenv", "stripe"]
        env_refs = ["stripe"]
    "#;
    let (ctx, _tmp) = ctx_for_tests(toml);
    let out = klef_run(
        &ctx,
        RunInput {
            argv: vec!["printenv".into(), "stripe".into()],
            env_refs: vec!["stripe".into()],
            cwd: None,
            timeout_ms: Some(5000),
        },
    )
    .await
    .expect("allow path must succeed");
    assert_eq!(
        out.exit_code, 0,
        "printenv should find env var named 'stripe'"
    );
    assert!(
        !out.stdout.contains("sk_live_abcdefg"),
        "raw secret value must not appear in stdout"
    );
    assert!(
        out.stdout.contains("[REDACTED:stripe]"),
        "redaction placeholder must appear; got {:?}",
        out.stdout
    );
}

#[tokio::test]
async fn klef_run_empty_workspace_roots_ignores_client_cwd() {
    let toml = r#"
        workspace_roots = []
        [[allow]]
        argv = ["pwd"]
        env_refs = []
    "#;
    let (ctx, _tmp) = ctx_for_tests(toml);
    let out = klef_run(
        &ctx,
        RunInput {
            argv: vec!["pwd".into()],
            env_refs: vec![],
            cwd: Some("/etc".into()),
            timeout_ms: Some(5000),
        },
    )
    .await
    .expect("allow path must succeed");
    // Process inherited the klef mcp cwd, NOT /etc.
    assert!(
        !out.stdout.contains("/etc"),
        "cwd must not be /etc; got {:?}",
        out.stdout
    );
}

#[tokio::test]
async fn klef_run_deny_audit_recorded() {
    let (ctx, tmp) = ctx_for_tests("");
    let _ = klef_run(
        &ctx,
        RunInput {
            argv: vec!["bash".into(), "-c".into(), "x".into()],
            env_refs: vec![],
            cwd: None,
            timeout_ms: None,
        },
    )
    .await;
    let log = std::fs::read_to_string(tmp.path().join("audit.log")).unwrap();
    let last = log.lines().last().unwrap();
    assert!(last.contains("\"decision\":\"deny\""));
    assert!(last.contains("shell_denylist:bash"));
}
