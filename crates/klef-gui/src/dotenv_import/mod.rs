//! Drag-drop / paste-path .env import flow.
//!
//! # Threat model
//!
//! The webview is a Svelte SPA bundled with the app — no remote content,
//! strict CSP. But Tauri commands form the trust boundary between Rust and
//! JS: anything we hand back to the webview can be inspected in dev-tools or
//! exfiltrated by an XSS in a future dependency. So:
//!
//! - **The full parsed plan, including plaintext values, stays in Rust.**
//!   `preview_dotenv_import` returns only metadata + redacted previews +
//!   a `session_id`; the actual secret values never round-trip through JS.
//! - **`apply_dotenv_import` does not trust client-supplied items or paths.**
//!   It looks the plan up by session id from server-side state and uses the
//!   stored `source_path` (canonicalized at preview time). The webview can
//!   only tell us which entries the user accepted.
//! - **`env_var` names are validated by `klef-core`** before being stored,
//!   so a malicious .env can't smuggle a shell-injection payload that would
//!   later render dangerously through `klef export`. (See
//!   `klef_core::store::validate_env_var`.)

mod helpers;

use crate::AppState;
use helpers::{
    canonicalize_source, klef_name_from_env_var, project_from_path, redact, rewrite_dotenv,
};
use serde::Serialize;

/// Max preview sessions held server-side. Bound prevents a chatty/buggy
/// frontend from leaking memory; oldest is evicted at capacity.
const MAX_SESSIONS: usize = 8;

/// Hard TTL for a preview session. If the modal isn't applied or cancelled
/// (e.g. the user dismissed the popover via blur / ⌘Q), the plaintext
/// values would otherwise sit in RAM until LRU eviction — which on light
/// usage can be hours. Five minutes is enough for any realistic
/// review-the-list flow and short enough that idle sessions don't linger.
const SESSION_TTL: std::time::Duration = std::time::Duration::from_mins(5);

/// Full parsed plan held server-side, keyed by a UUID session id. The
/// webview never sees `value`; it gets a `WebPlanItem` + session id and
/// apply re-loads from this state.
pub struct ServerPlan {
    pub source_path: std::path::PathBuf,
    pub suggested_project: String,
    pub items: Vec<ServerPlanItem>,
    /// Monotonic insert order — used to evict the oldest at `MAX_SESSIONS`.
    pub created_seq: u64,
    /// Wall-clock creation time, used to enforce `SESSION_TTL`.
    pub created_at: std::time::Instant,
}

pub struct ServerPlanItem {
    pub env_var: String,
    pub klef_name: String,
    pub value: String,
    pub status: ItemStatus,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ItemStatus {
    New,
    Conflict,
    Ref,
    Empty,
}

impl ItemStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Conflict => "conflict",
            Self::Ref => "ref",
            Self::Empty => "empty",
        }
    }

    const fn is_importable(self) -> bool {
        matches!(self, Self::New | Self::Conflict)
    }
}

#[derive(Serialize)]
pub struct WebPlanItem {
    pub env_var: String,
    pub klef_name: String,
    pub redacted_value: String,
    /// `new` | `conflict` | `ref` | `empty`
    pub status: &'static str,
}

#[derive(Serialize)]
pub struct WebPlan {
    pub session_id: String,
    pub suggested_project: String,
    pub source_path: String,
    pub items: Vec<WebPlanItem>,
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn preview_dotenv_import(
    path: String,
    state: tauri::State<'_, AppState>,
) -> Result<WebPlan, String> {
    use klef_core::envfile::{self, Value};
    let canonical = canonicalize_source(&path)?;
    let entries = envfile::parse(&canonical).map_err(|e| e.to_string())?;
    let existing: std::collections::HashSet<String> = state
        .store
        .list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|(n, _)| n)
        .collect();

    let server_items: Vec<ServerPlanItem> = entries
        .into_iter()
        .map(|e| match e.value {
            Value::Reference(target) => ServerPlanItem {
                env_var: e.key,
                klef_name: target,
                value: String::new(),
                status: ItemStatus::Ref,
            },
            Value::Literal(v) if v.is_empty() => ServerPlanItem {
                klef_name: klef_name_from_env_var(&e.key),
                value: v,
                env_var: e.key,
                status: ItemStatus::Empty,
            },
            Value::Literal(v) => {
                let name = klef_name_from_env_var(&e.key);
                let status = if existing.contains(&name) {
                    ItemStatus::Conflict
                } else {
                    ItemStatus::New
                };
                ServerPlanItem {
                    klef_name: name,
                    value: v,
                    env_var: e.key,
                    status,
                }
            }
        })
        .collect();

    let suggested_project = project_from_path(&canonical);
    let session_id = uuid::Uuid::new_v4().to_string();

    // Webview-safe view (no plaintext) before moving server_items into state.
    let web_items: Vec<WebPlanItem> = server_items
        .iter()
        .map(|it| WebPlanItem {
            env_var: it.env_var.clone(),
            klef_name: it.klef_name.clone(),
            redacted_value: if matches!(it.status, ItemStatus::Ref) {
                String::new()
            } else {
                redact(&it.value)
            },
            status: it.status.as_str(),
        })
        .collect();

    let created_seq = next_seq();
    {
        let mut sessions = state
            .dotenv_sessions
            .lock()
            .map_err(|e| format!("session lock poisoned: {e}"))?;
        // Opportunistic GC: drop any session past its TTL before deciding
        // whether to evict on size. Klef-gui is a popover app with no
        // background thread, so cleanup is amortized on every preview.
        prune_expired(&mut sessions);
        if sessions.len() >= MAX_SESSIONS
            && let Some(oldest_key) = sessions
                .iter()
                .min_by_key(|(_, p)| p.created_seq)
                .map(|(k, _)| k.clone())
        {
            sessions.remove(&oldest_key);
        }
        sessions.insert(
            session_id.clone(),
            ServerPlan {
                source_path: canonical.clone(),
                suggested_project: suggested_project.clone(),
                items: server_items,
                created_seq,
                created_at: std::time::Instant::now(),
            },
        );
    }

    Ok(WebPlan {
        session_id,
        suggested_project,
        source_path: canonical.to_string_lossy().into_owned(),
        items: web_items,
    })
}

fn next_seq() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    SEQ.fetch_add(1, Ordering::Relaxed)
}

/// Drop every session older than `SESSION_TTL`. Called opportunistically
/// from `preview` and `apply` — no background thread needed.
fn prune_expired(sessions: &mut std::collections::HashMap<String, ServerPlan>) {
    let now = std::time::Instant::now();
    sessions.retain(|_, p| now.duration_since(p.created_at) < SESSION_TTL);
}

/// Apply the previewed import. `accepted` is the list of `env_var` names
/// the user checked in the preview UI; anything not in this set is skipped.
/// Items the server marked as `ref` or `empty` are always skipped
/// regardless of this list.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn apply_dotenv_import(
    session_id: String,
    project: String,
    rewrite_source: bool,
    accepted: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<u32, String> {
    // Single-use: pull the session out so a retry can't re-import entries
    // that already landed. Prune expired sessions first so a stale id
    // returns the same "expired" error as an unknown one.
    let plan = {
        let mut sessions = state
            .dotenv_sessions
            .lock()
            .map_err(|e| format!("session lock poisoned: {e}"))?;
        prune_expired(&mut sessions);
        sessions
            .remove(&session_id)
            .ok_or_else(|| "unknown or expired import session".to_string())?
    };

    let accept_set: std::collections::HashSet<&str> = accepted.iter().map(String::as_str).collect();
    let project_tag = format!("project:{project}");
    let mut count = 0u32;
    let mut imported_pairs: Vec<(String, String)> = Vec::new();
    for it in &plan.items {
        if !it.status.is_importable() {
            continue;
        }
        if !accept_set.contains(it.env_var.as_str()) {
            continue;
        }
        // force=true: 'conflict' overwrites — user accepted in preview UI.
        // env_var is re-validated by Store::add (defense-in-depth against
        // a malicious .env smuggling a shell-injection payload).
        state
            .store
            .add(
                &it.klef_name,
                &it.value,
                Some(it.env_var.clone()),
                None,
                vec![project_tag.clone()],
                true,
            )
            .map_err(|e| e.to_string())?;
        imported_pairs.push((it.env_var.clone(), it.klef_name.clone()));
        count += 1;
    }
    if rewrite_source && !imported_pairs.is_empty() {
        // source_path was canonicalized at preview time — we never accept
        // a frontend-supplied write target.
        rewrite_dotenv(&plan.source_path, &imported_pairs).map_err(|e| e.to_string())?;
    }
    Ok(count)
}

/// Cancel a preview session without applying. Frees the server-side state.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn cancel_dotenv_import(
    session_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    {
        let mut sessions = state
            .dotenv_sessions
            .lock()
            .map_err(|e| format!("session lock poisoned: {e}"))?;
        sessions.remove(&session_id);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
