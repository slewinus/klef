//! Handler functions for `klef_list` and `klef_run`.
//!
//! Pure orchestration over `Store` + `Policy` + `run_proc` + `redact` +
//! `audit`. The rmcp adapter in `mod.rs` translates JSON-RPC requests into
//! calls here.

use crate::commands::mcp::audit::{Audit, Entry, now_iso};
use crate::commands::mcp::policy::{Decision, DenyReason, Policy, Request as PolReq};
use crate::commands::mcp::redact;
use crate::commands::mcp::run_proc::{self, DEFAULT_TIMEOUT_MS, HARDCAP_TIMEOUT_MS, ProcRequest};
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
/// audit-write failure; `Internal` for spawn/runtime issues.
pub async fn klef_run(ctx: &Ctx, input: RunInput) -> Result<RunOutput, ToolError> {
    // 1. Validate timeout up front.
    let timeout_ms = input.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
    if timeout_ms > HARDCAP_TIMEOUT_MS {
        let reason = format!("timeout_exceeds_max:{timeout_ms}");
        record_deny(ctx, &input, &reason)?;
        return Err(ToolError::Policy(format!(
            "timeout_ms {timeout_ms} exceeds max {HARDCAP_TIMEOUT_MS}"
        )));
    }

    // 2. Policy evaluation.
    let cwd_ref = input.cwd.as_deref();
    let pol_req = PolReq {
        argv: &input.argv,
        env_refs: &input.env_refs,
        cwd: cwd_ref,
    };
    let matched_rule_index = match ctx.policy.evaluate(&pol_req) {
        Decision::Allow { matched_rule_index } => matched_rule_index,
        Decision::Deny { reason } => {
            let reason_str = format_deny(&reason);
            record_deny(ctx, &input, &reason_str)?;
            return Err(ToolError::Policy(human_deny(&reason, &input)));
        }
    };

    // 3. Resolve env_refs from store.
    let mut resolved: Vec<(String, String)> = Vec::with_capacity(input.env_refs.len());
    for name in &input.env_refs {
        let store = ctx.store.clone();
        let n = name.clone();
        let v = tokio::task::spawn_blocking(move || store.get_value(&n))
            .await
            .map_err(|e| ToolError::Internal(e.to_string()))?;
        if let Ok(value) = v {
            resolved.push((name.clone(), value));
        } else {
            let reason = format!("env_ref_not_found:{name}");
            record_deny(ctx, &input, &reason)?;
            return Err(ToolError::EnvRefNotFound(name.clone()));
        }
    }

    // 4. Spawn the child.
    let env: HashMap<String, String> = resolved.iter().cloned().collect();
    let proc_req = ProcRequest {
        argv: input.argv.clone(),
        env,
        cwd: input.cwd.clone(),
        timeout_ms,
    };
    let mut result = run_proc::spawn_and_capture(proc_req)
        .await
        .map_err(|e| ToolError::Internal(e.to_string()))?;

    // 5. Best-effort redaction (mutates buffers in place).
    redact::redact(&mut result.stdout, &resolved);
    redact::redact(&mut result.stderr, &resolved);

    // 6. UTF-8 vs base64 encoding decision.
    let (stdout_str, stderr_str, encoding) = encode_outputs(&result.stdout, &result.stderr);

    // 7. Audit allow.
    ctx.audit
        .record(&Entry {
            ts: now_iso(),
            tool: "klef_run",
            argv: Some(&input.argv),
            env_refs: Some(&input.env_refs),
            cwd: input.cwd.as_deref().and_then(|p| p.to_str()),
            decision: "allow",
            matched_rule_index: Some(matched_rule_index),
            reason: None,
            exit_code: Some(result.exit_code),
            duration_ms: Some(result.duration_ms),
            stdout_bytes: Some(result.stdout.len()),
            stderr_bytes: Some(result.stderr.len()),
            stdout_truncated: Some(result.stdout_truncated),
            stderr_truncated: Some(result.stderr_truncated),
            timed_out: Some(result.timed_out),
            count_returned: None,
        })
        .map_err(|e| ToolError::Audit(e.to_string()))?;

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

fn record_deny(ctx: &Ctx, input: &RunInput, reason: &str) -> Result<(), ToolError> {
    ctx.audit
        .record(&Entry {
            ts: now_iso(),
            tool: "klef_run",
            argv: Some(&input.argv),
            env_refs: Some(&input.env_refs),
            cwd: input.cwd.as_deref().and_then(|p| p.to_str()),
            decision: "deny",
            matched_rule_index: None,
            reason: Some(reason.to_string()),
            exit_code: None,
            duration_ms: None,
            stdout_bytes: None,
            stderr_bytes: None,
            stdout_truncated: None,
            stderr_truncated: None,
            timed_out: None,
            count_returned: None,
        })
        .map_err(|e| ToolError::Audit(e.to_string()))
}

fn format_deny(r: &DenyReason) -> String {
    match r {
        DenyReason::ShellDenylist(p) => format!("shell_denylist:{p}"),
        DenyReason::CwdNotInWorkspaceRoots => "cwd_not_in_workspace_roots".into(),
        DenyReason::NoRuleMatch => "no_rule_match".into(),
    }
}

fn human_deny(r: &DenyReason, input: &RunInput) -> String {
    match r {
        DenyReason::ShellDenylist(p) => format!("program '{p}' is on the shell denylist"),
        DenyReason::CwdNotInWorkspaceRoots => {
            let p = input
                .cwd
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            format!("cwd {p:?} is not under any workspace_root")
        }
        DenyReason::NoRuleMatch => {
            format!(
                "no rule matches argv {:?} with env_refs {:?}",
                input.argv, input.env_refs
            )
        }
    }
}

#[path = "tools_encode.rs"]
mod encode;
use encode::encode_outputs;

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
