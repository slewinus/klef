//! Handler functions for `klef_list` and `klef_run`.
//!
//! Pure orchestration over `Store` + `Policy` + `run_proc` + `redact` +
//! `audit`. The rmcp adapter in `mod.rs` translates JSON-RPC requests into
//! calls here.

use crate::commands::mcp::audit::{Audit, Entry, now_iso};
use crate::commands::mcp::policy::{Decision, Policy, Request as PolReq};
use crate::commands::mcp::redact;
use crate::commands::mcp::run_proc::{self, DEFAULT_TIMEOUT_MS, HARDCAP_TIMEOUT_MS, ProcRequest};
use crate::commands::mcp::tools_audit::{
    format_deny, human_deny, record_completed, record_deny, record_started,
};
use klef_core::store::Store;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct ListInput {
    pub tag: Option<String>,
    pub filter: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListEntry {
    pub name: String,
    pub note: Option<String>,
    pub tags: Vec<String>,
    pub added_at: String,
}

#[derive(Debug, Deserialize)]
pub struct RunInput {
    pub argv: Vec<String>,
    #[serde(default)]
    pub env_refs: Vec<String>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct RunOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub timed_out: bool,
    pub encoding: &'static str,
}

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("policy: {0}")]
    Policy(String),
    #[error("store: env_ref '{0}' not found")]
    EnvRefNotFound(String),
    #[error("audit: {0}")]
    Audit(String),
    #[error("internal: {0}")]
    Internal(String),
}

pub struct Ctx {
    pub store: Arc<Store>,
    pub policy: Arc<Policy>,
    pub audit: Audit,
}

/// List metadata-only entries, optionally filtered by tag and/or substring.
///
/// # Errors
/// `Internal` on store/index errors; `Audit` if append fails.
pub async fn klef_list(ctx: &Ctx, input: ListInput) -> Result<Vec<ListEntry>, ToolError> {
    let store = ctx.store.clone();
    let entries = tokio::task::spawn_blocking(move || store.list())
        .await
        .map_err(|e| ToolError::Internal(e.to_string()))?
        .map_err(|e| ToolError::Internal(e.to_string()))?;

    let needle = input.filter.as_deref().map(str::to_lowercase);
    let tag = input.tag.as_deref();
    let filtered: Vec<ListEntry> = entries
        .into_iter()
        .filter_map(|(name, meta)| {
            if let Some(t) = tag
                && !meta.tags.iter().any(|x| x == t)
            {
                return None;
            }
            if let Some(n) = needle.as_deref() {
                let matches_name = name.to_lowercase().contains(n);
                let matches_note = meta
                    .note
                    .as_deref()
                    .is_some_and(|x| x.to_lowercase().contains(n));
                if !matches_name && !matches_note {
                    return None;
                }
            }
            let added_at = meta
                .added_at
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default();
            Some(ListEntry {
                name,
                note: meta.note,
                tags: meta.tags,
                added_at,
            })
        })
        .collect();

    let count = filtered.len();
    ctx.audit
        .record(&Entry {
            ts: now_iso(),
            tool: "klef_list",
            argv: None,
            env_refs: None,
            cwd: None,
            decision: "allow",
            phase: None,
            matched_rule_index: None,
            reason: None,
            exit_code: None,
            duration_ms: None,
            stdout_bytes: None,
            stderr_bytes: None,
            stdout_truncated: None,
            stderr_truncated: None,
            timed_out: None,
            count_returned: Some(count),
        })
        .map_err(|e| ToolError::Audit(e.to_string()))?;

    Ok(filtered)
}

/// Run a child process with klef-resolved env and full policy + audit.
///
/// # Errors
/// `Policy` on denial; `EnvRefNotFound` if a key is missing; `Audit` on
/// pre-spawn audit-write failure (fail-closed); `Internal` for spawn issues.
pub async fn klef_run(ctx: &Ctx, mut input: RunInput) -> Result<RunOutput, ToolError> {
    // Per spec: empty workspace_roots ⇒ ignore client-supplied cwd entirely.
    if ctx.policy.workspace_roots.is_empty() {
        input.cwd = None;
    }

    // 1. Validate timeout up front.
    let timeout_ms = input.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
    if timeout_ms > HARDCAP_TIMEOUT_MS {
        let reason = format!("timeout_exceeds_max:{timeout_ms}");
        record_deny(&ctx.audit, &input, &reason)?;
        return Err(ToolError::Policy(format!(
            "timeout_ms {timeout_ms} exceeds max {HARDCAP_TIMEOUT_MS}"
        )));
    }

    // 2. Policy evaluation.
    let pol_req = PolReq {
        argv: &input.argv,
        env_refs: &input.env_refs,
        cwd: input.cwd.as_deref(),
    };
    let matched_rule_index = match ctx.policy.evaluate(&pol_req) {
        Decision::Allow { matched_rule_index } => matched_rule_index,
        Decision::Deny { reason } => {
            let reason_str = format_deny(&reason);
            record_deny(&ctx.audit, &input, &reason_str)?;
            return Err(ToolError::Policy(human_deny(&reason, &input)));
        }
    };

    // 3. Resolve env_refs from store. Inject under each key's `env_var`
    //    metadata (e.g. `STRIPE_API_KEY`), not the klef key name.
    let store_for_meta = ctx.store.clone();
    let metas = tokio::task::spawn_blocking(move || store_for_meta.list())
        .await
        .map_err(|e| ToolError::Internal(e.to_string()))?
        .map_err(|e| ToolError::Internal(e.to_string()))?;
    let mut resolved: Vec<(String, String, String)> = Vec::with_capacity(input.env_refs.len());
    for name in &input.env_refs {
        let Some(meta) = metas
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, m)| m.clone())
        else {
            let reason = format!("env_ref_not_found:{name}");
            record_deny(&ctx.audit, &input, &reason)?;
            return Err(ToolError::EnvRefNotFound(name.clone()));
        };
        let store = ctx.store.clone();
        let n = name.clone();
        let v = tokio::task::spawn_blocking(move || store.get_value(&n))
            .await
            .map_err(|e| ToolError::Internal(e.to_string()))?;
        let Ok(value) = v else {
            let reason = format!("env_ref_not_found:{name}");
            record_deny(&ctx.audit, &input, &reason)?;
            return Err(ToolError::EnvRefNotFound(name.clone()));
        };
        resolved.push((name.clone(), meta.env_var, value));
    }

    // 4. Pre-spawn audit gate: if this fails, NOTHING runs.
    record_started(&ctx.audit, &input, matched_rule_index)?;

    // 5. Spawn the child.
    let env: HashMap<String, String> = resolved
        .iter()
        .map(|(_, var, val)| (var.clone(), val.clone()))
        .collect();
    let proc_req = ProcRequest {
        argv: input.argv.clone(),
        env,
        cwd: input.cwd.clone(),
        timeout_ms,
    };
    let mut result = run_proc::spawn_and_capture(proc_req)
        .await
        .map_err(|e| ToolError::Internal(e.to_string()))?;

    // 6. Best-effort redaction. Redact on the klef key name (not env var
    //    name) to keep the placeholder stable across rename / env_var
    //    reconfiguration.
    let redact_pairs: Vec<(String, String)> = resolved
        .iter()
        .map(|(name, _, val)| (name.clone(), val.clone()))
        .collect();
    redact::redact(&mut result.stdout, &redact_pairs);
    redact::redact(&mut result.stderr, &redact_pairs);

    // 7. UTF-8 vs base64 encoding decision.
    let (stdout_str, stderr_str, encoding) = encode_outputs(&result.stdout, &result.stderr);

    // 8. Post-spawn audit. The secret has already flowed; if this write
    //    fails, log to stderr and continue — failing now is just hiding
    //    observability.
    if let Err(e) = record_completed(&ctx.audit, &input, matched_rule_index, &result) {
        eprintln!("klef mcp: audit completion write failed: {e}");
    }

    Ok(RunOutput {
        exit_code: result.exit_code,
        stdout: stdout_str,
        stderr: stderr_str,
        duration_ms: result.duration_ms,
        stdout_truncated: result.stdout_truncated,
        stderr_truncated: result.stderr_truncated,
        timed_out: result.timed_out,
        encoding,
    })
}

#[path = "tools_encode.rs"]
mod encode;
use encode::encode_outputs;

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
