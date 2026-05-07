//! `klef mcp` — MCP server exposing `klef_list` (metadata) and `klef_run`
//! (process spawn with klef: refs injected). See `docs/mcp.md`.

pub mod audit;
pub mod policy;
pub mod redact;
pub mod run_proc;
pub mod tools;
pub mod tools_audit;

use crate::commands::mcp::audit::Audit;
use crate::commands::mcp::tools::{Ctx, ListInput, RunInput};
use klef_core::error::KlefError;
use klef_core::store::Store;
use rmcp::ServerHandler;
use rmcp::ServiceExt;
use rmcp::model::{
    CallToolRequestParam, CallToolResult, Content, Implementation, JsonObject, ListToolsResult,
    PaginatedRequestParam, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

/// Entry point for `klef mcp`. Loads the policy, starts the rmcp server
/// over stdio, and blocks until stdin closes.
///
/// # Errors
///
/// Returns an error if the policy file cannot be loaded or the server
/// cannot start.
pub fn run(store: Store, policy_path: Option<PathBuf>) -> Result<(), KlefError> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| KlefError::BackendUnavailable("config dir unavailable".into()))?
        .join("klef");

    let policy_path = policy_path.unwrap_or_else(|| config_dir.join("mcp-policy.toml"));
    let audit_path = config_dir.join("audit.log");

    let policy = policy::load(&policy_path)
        .map_err(|e| KlefError::BackendUnavailable(format!("policy load: {e}")))?;

    eprintln!("klef mcp: policy = {}", policy_path.display());

    let ctx = Ctx {
        store: Arc::new(store),
        policy: Arc::new(policy),
        audit: Audit::new(audit_path),
    };

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| KlefError::BackendUnavailable(format!("tokio runtime: {e}")))?;

    rt.block_on(serve(ctx))
        .map_err(|e| KlefError::BackendUnavailable(format!("mcp serve: {e}")))?;
    Ok(())
}

async fn serve(ctx: Ctx) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let handler = KlefHandler { ctx: Arc::new(ctx) };
    let transport = (tokio::io::stdin(), tokio::io::stdout());
    let running = handler.serve(transport).await?;
    running.waiting().await?;
    Ok(())
}

#[derive(Clone)]
struct KlefHandler {
    ctx: Arc<Ctx>,
}

impl ServerHandler for KlefHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: rmcp::model::ProtocolVersion::default(),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "klef".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            instructions: Some(
                "klef MCP server: klef_list returns metadata only; klef_run spawns \
                 a child process with klef-resolved env vars under policy."
                    .into(),
            ),
        }
    }

    async fn list_tools(
        &self,
        _req: PaginatedRequestParam,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::Error> {
        Ok(ListToolsResult {
            next_cursor: None,
            tools: vec![
                Tool::new(
                    "klef_list",
                    "List klef entries (metadata only — no values). Optional tag/filter.",
                    schema::list_input(),
                ),
                Tool::new(
                    "klef_run",
                    "Spawn a child process with `klef:`-resolved env vars. Subject to policy \
                     rules; output is best-effort redacted of resolved values.",
                    schema::run_input(),
                ),
            ],
        })
    }

    async fn call_tool(
        &self,
        req: CallToolRequestParam,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, rmcp::Error> {
        let args = req.arguments.unwrap_or_default();
        match req.name.as_ref() {
            "klef_list" => {
                let input: ListInput = serde_json::from_value(serde_json::Value::Object(args))
                    .map_err(|e| {
                        rmcp::Error::invalid_params(format!("klef_list args: {e}"), None)
                    })?;
                match tools::klef_list(&self.ctx, input).await {
                    Ok(entries) => {
                        let v = serde_json::to_value(&entries).map_err(|e| {
                            rmcp::Error::internal_error(format!("serialize: {e}"), None)
                        })?;
                        Ok(CallToolResult::success(vec![Content::json(v).map_err(
                            |e| rmcp::Error::internal_error(format!("content: {e}"), None),
                        )?]))
                    }
                    Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
                }
            }
            "klef_run" => {
                let input: RunInput = serde_json::from_value(serde_json::Value::Object(args))
                    .map_err(|e| {
                        rmcp::Error::invalid_params(format!("klef_run args: {e}"), None)
                    })?;
                match tools::klef_run(&self.ctx, input).await {
                    Ok(out) => {
                        let v = serde_json::to_value(&out).map_err(|e| {
                            rmcp::Error::internal_error(format!("serialize: {e}"), None)
                        })?;
                        Ok(CallToolResult::success(vec![Content::json(v).map_err(
                            |e| rmcp::Error::internal_error(format!("content: {e}"), None),
                        )?]))
                    }
                    Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
                }
            }
            other => Err(rmcp::Error::invalid_params(
                format!("unknown tool: {other}"),
                None,
            )),
        }
    }
}

mod schema {
    use super::{JsonObject, json};

    fn obj(v: serde_json::Value) -> JsonObject {
        match v {
            serde_json::Value::Object(m) => m,
            _ => JsonObject::default(),
        }
    }

    pub(super) fn list_input() -> JsonObject {
        obj(json!({
            "type": "object",
            "properties": {
                "tag": {
                    "type": "string",
                    "description": "Filter to entries tagged with this string."
                },
                "filter": {
                    "type": "string",
                    "description": "Case-insensitive substring match on name or note."
                }
            },
            "additionalProperties": false
        }))
    }

    pub(super) fn run_input() -> JsonObject {
        obj(json!({
            "type": "object",
            "required": ["argv"],
            "properties": {
                "argv": {
                    "type": "array",
                    "items": { "type": "string" },
                    "minItems": 1,
                    "description": "Program and arguments. argv[0] is the program."
                },
                "env_refs": {
                    "type": "array",
                    "items": { "type": "string" },
                    "default": [],
                    "description": "Names of klef entries to resolve and inject into the env."
                },
                "cwd": {
                    "type": "string",
                    "description": "Working directory for the child. Must be under a policy workspace_root."
                },
                "timeout_ms": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 300_000,
                    "description": "Wall-clock timeout in milliseconds (default 30000, max 300000)."
                }
            },
            "additionalProperties": false
        }))
    }
}
