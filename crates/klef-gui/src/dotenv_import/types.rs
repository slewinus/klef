//! Session and wire types for the dotenv-import flow (see `mod.rs`).

use serde::Serialize;

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
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Conflict => "conflict",
            Self::Ref => "ref",
            Self::Empty => "empty",
        }
    }

    pub const fn is_importable(self) -> bool {
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
