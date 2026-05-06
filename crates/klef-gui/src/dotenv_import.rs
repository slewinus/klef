//! Drag-drop / paste-path .env import flow.
//!
//! Two Tauri commands:
//! - `preview_dotenv_import`: parses the file, suggests a project name from
//!   the parent directory, classifies each entry as new/conflict/ref/empty.
//!   Returns the plan to the frontend so the user can review before apply.
//! - `apply_dotenv_import`: writes the accepted entries to the Store with
//!   the chosen `project:<name>` tag.

use crate::AppState;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct DotenvPlanItem {
    pub env_var: String,
    pub klef_name: String,
    pub redacted_value: String,
    /// Plain (untruncated) value, sent back so the frontend can echo it
    /// verbatim into apply without storing it in JS state any longer
    /// than necessary. Internal pipeline metadata, not displayed.
    pub value: String,
    /// `new` | `conflict` | `ref` | `empty`
    pub status: String,
}

#[derive(Serialize)]
pub struct DotenvPlan {
    pub suggested_project: String,
    pub items: Vec<DotenvPlanItem>,
    pub source_path: String,
}

fn redact(v: &str) -> String {
    let n = v.chars().count();
    if n <= 6 {
        format!("*** ({n} chars)")
    } else {
        let prefix: String = v.chars().take(4).collect();
        format!("{prefix}*** ({n} chars)")
    }
}

fn klef_name_from_env_var(k: &str) -> String {
    k.chars()
        .map(|c| {
            if c == '_' {
                '-'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect()
}

fn project_from_path(p: &std::path::Path) -> String {
    p.parent().and_then(|d| d.file_name()).map_or_else(
        || "unknown".to_string(),
        |n| n.to_string_lossy().replace([' ', '/'], "-").to_lowercase(),
    )
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn preview_dotenv_import(
    path: String,
    state: tauri::State<'_, AppState>,
) -> Result<DotenvPlan, String> {
    use klef_core::envfile::{self, Value};
    let pb = std::path::PathBuf::from(&path);
    let entries = envfile::parse(&pb).map_err(|e| e.to_string())?;
    let existing: std::collections::HashSet<String> = state
        .store
        .list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    let items: Vec<DotenvPlanItem> = entries
        .into_iter()
        .map(|e| match e.value {
            Value::Reference(target) => DotenvPlanItem {
                env_var: e.key,
                klef_name: target,
                redacted_value: String::new(),
                value: String::new(),
                status: "ref".to_string(),
            },
            Value::Literal(v) if v.is_empty() => DotenvPlanItem {
                klef_name: klef_name_from_env_var(&e.key),
                redacted_value: redact(&v),
                value: v,
                env_var: e.key,
                status: "empty".to_string(),
            },
            Value::Literal(v) => {
                let name = klef_name_from_env_var(&e.key);
                let status = if existing.contains(&name) {
                    "conflict"
                } else {
                    "new"
                }
                .to_string();
                DotenvPlanItem {
                    klef_name: name,
                    redacted_value: redact(&v),
                    value: v,
                    env_var: e.key,
                    status,
                }
            }
        })
        .collect();
    Ok(DotenvPlan {
        suggested_project: project_from_path(&pb),
        items,
        source_path: path,
    })
}

#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
pub fn apply_dotenv_import(
    items: Vec<DotenvPlanItem>,
    project: String,
    state: tauri::State<'_, AppState>,
) -> Result<u32, String> {
    let project_tag = format!("project:{project}");
    let mut count = 0u32;
    for it in items {
        if it.status != "new" && it.status != "conflict" {
            continue;
        }
        // force=true so 'conflict' overwrites — the user already accepted
        // in the preview UI.
        state
            .store
            .add(
                &it.klef_name,
                &it.value,
                Some(it.env_var),
                None,
                vec![project_tag.clone()],
                true,
            )
            .map_err(|e| e.to_string())?;
        count += 1;
    }
    Ok(count)
}
