//! Audit-entry construction helpers extracted from `tools.rs`.
//!
//! Two phases for the allow path (`started` pre-spawn, `completed`
//! post-spawn) plus the atomic deny entry and human/machine-readable
//! deny formatters. Lives in a sibling file to keep `tools.rs` under
//! the 300-line cap.

use crate::commands::mcp::audit::{Audit, Entry, now_iso};
use crate::commands::mcp::policy::DenyReason;
use crate::commands::mcp::run_proc::ProcResult;
use crate::commands::mcp::tools::{RunInput, ToolError};

/// Pre-spawn audit entry. If this write fails the spawn MUST NOT happen —
/// callers bubble the `ToolError::Audit` up so the caller fails closed.
///
/// # Errors
/// Returns `ToolError::Audit` if the underlying append fails.
pub fn record_started(
    audit: &Audit,
    input: &RunInput,
    matched_rule_index: usize,
) -> Result<(), ToolError> {
    audit
        .record(&Entry {
            ts: now_iso(),
            tool: "klef_run",
            argv: Some(&input.argv),
            env_refs: Some(&input.env_refs),
            cwd: input.cwd.as_deref().and_then(|p| p.to_str()),
            decision: "allow",
            phase: Some("started"),
            matched_rule_index: Some(matched_rule_index),
            reason: None,
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

/// Post-spawn audit entry. The secret has already flowed; if this write
/// fails the call still succeeds — caller logs to stderr.
///
/// # Errors
/// Returns the underlying `AuditError` so the caller can log without
/// failing the request.
pub fn record_completed(
    audit: &Audit,
    input: &RunInput,
    matched_rule_index: usize,
    result: &ProcResult,
) -> Result<(), super::audit::AuditError> {
    audit.record(&Entry {
        ts: now_iso(),
        tool: "klef_run",
        argv: Some(&input.argv),
        env_refs: Some(&input.env_refs),
        cwd: input.cwd.as_deref().and_then(|p| p.to_str()),
        decision: "allow",
        phase: Some("completed"),
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
}

/// Atomic deny entry — fails closed (caller propagates `ToolError::Audit`).
///
/// # Errors
/// Returns `ToolError::Audit` if the underlying append fails.
pub fn record_deny(audit: &Audit, input: &RunInput, reason: &str) -> Result<(), ToolError> {
    audit
        .record(&Entry {
            ts: now_iso(),
            tool: "klef_run",
            argv: Some(&input.argv),
            env_refs: Some(&input.env_refs),
            cwd: input.cwd.as_deref().and_then(|p| p.to_str()),
            decision: "deny",
            phase: None,
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

#[must_use]
pub fn format_deny(r: &DenyReason) -> String {
    match r {
        DenyReason::ShellDenylist(p) => format!("shell_denylist:{p}"),
        DenyReason::CwdNotInWorkspaceRoots => "cwd_not_in_workspace_roots".into(),
        DenyReason::NoRuleMatch => "no_rule_match".into(),
    }
}

#[must_use]
pub fn human_deny(r: &DenyReason, input: &RunInput) -> String {
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
