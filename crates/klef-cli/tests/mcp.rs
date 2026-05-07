//! End-to-end MCP integration tests.
//!
//! These spawn the real `klef` binary with `--features mcp` and drive it as
//! an MCP client over stdio using the `rmcp` client. They cover:
//!
//! 1. `tools/list` exposes exactly `klef_list` and `klef_run`, no more.
//! 2. `klef_run` with `argv = ["sh", "-c", ...]` is denied by the shell
//!    denylist (independent of policy contents).

#![cfg(feature = "mcp")]

use assert_cmd::Command as TestCmd;
use rmcp::ServiceExt;
use rmcp::model::CallToolRequestParam;
use rmcp::transport::TokioChildProcess;
use serde_json::json;
use std::path::Path;
use tempfile::TempDir;
use tokio::process::Command as TokioCommand;

/// Path to the `klef` binary built by cargo for this crate.
fn klef_bin() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin("klef")
}

/// Pre-populate the file-backed store with one entry named `stripe` whose
/// value is `sk_live_abc`. Uses a synchronous `klef add` invocation so the
/// store on disk is fully written before the MCP server is spawned.
fn pre_add_stripe(index_path: &Path, vault_path: &Path) {
    TestCmd::cargo_bin("klef")
        .unwrap()
        .env("KLEF_INDEX_PATH", index_path)
        .env(
            "KLEF_TEST_BACKEND",
            format!("file:{}", vault_path.display()),
        )
        .args(["add", "stripe"])
        .write_stdin("sk_live_abc")
        .assert()
        .success();
}

/// Spawn `klef mcp` as a subprocess and return a connected rmcp client plus
/// the `TempDir` that owns its on-disk state. The `TempDir` must outlive the
/// client; both are returned to the caller for that reason.
async fn spawn_server() -> (
    rmcp::service::RunningService<rmcp::service::RoleClient, ()>,
    TempDir,
) {
    let tmp = TempDir::new().unwrap();
    let index_path = tmp.path().join("index.json");
    let vault_path = tmp.path().join("secrets.json");
    let policy_path = tmp.path().join("p.toml");
    let xdg = tmp.path().join("xdg");
    std::fs::create_dir_all(&xdg).unwrap();

    // Empty policy: no allow rules. Shell deny still fires regardless.
    std::fs::write(&policy_path, "workspace_roots = []\n").unwrap();

    pre_add_stripe(&index_path, &vault_path);

    let mut cmd = TokioCommand::new(klef_bin());
    cmd.arg("mcp")
        .arg("--policy")
        .arg(&policy_path)
        .env("KLEF_INDEX_PATH", &index_path)
        .env(
            "KLEF_TEST_BACKEND",
            format!("file:{}", vault_path.display()),
        )
        // Isolate from any real ~/.config/klef on the dev box.
        .env("XDG_CONFIG_HOME", &xdg)
        .env("HOME", tmp.path())
        // Inherit stderr so we can see klef's startup banner if a test
        // breaks; rmcp owns stdin/stdout via TokioChildProcess.
        .stderr(std::process::Stdio::inherit());

    let transport = TokioChildProcess::new(&mut cmd).expect("spawn klef mcp");
    let client = ().serve(transport).await.expect("rmcp handshake");
    (client, tmp)
}

#[tokio::test]
async fn tools_list_exposes_only_list_and_run() {
    let (client, _tmp) = spawn_server().await;

    let tools = client.peer().list_tools(None).await.expect("list_tools");

    let names: Vec<String> = tools
        .tools
        .iter()
        .map(|t| t.name.as_ref().to_string())
        .collect();

    assert_eq!(names.len(), 2, "expected exactly 2 tools, got {names:?}");
    assert!(
        names.contains(&"klef_list".to_string()),
        "missing klef_list: {names:?}"
    );
    assert!(
        names.contains(&"klef_run".to_string()),
        "missing klef_run: {names:?}"
    );

    client.cancel().await.ok();
}

#[tokio::test]
async fn klef_run_deny_shell_returns_error() {
    let (client, _tmp) = spawn_server().await;

    let args = json!({
        "argv": ["sh", "-c", "echo hi"],
        "env_refs": [],
    });
    let serde_json::Value::Object(args_obj) = args else {
        unreachable!()
    };

    let result = client
        .peer()
        .call_tool(CallToolRequestParam {
            name: "klef_run".into(),
            arguments: Some(args_obj),
        })
        .await
        .expect("call_tool");

    assert_eq!(
        result.is_error,
        Some(true),
        "expected is_error=true for shell-denied request, got {result:?}"
    );

    // Pull text payloads out of result.content. We accept either the
    // structured json content or text content; the human-readable deny
    // message must mention the shell denylist.
    let blob = serde_json::to_string(&result.content).unwrap();
    assert!(
        blob.contains("shell denylist") || blob.contains("denylist"),
        "deny message did not mention shell denylist: {blob}"
    );

    client.cancel().await.ok();
}
