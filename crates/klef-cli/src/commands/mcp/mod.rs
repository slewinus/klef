//! `klef mcp` — MCP server exposing `klef_list` (metadata) and `klef_run`
//! (process spawn with klef: refs injected). See `docs/mcp.md`.

pub mod policy;

use klef_core::error::KlefError;
use klef_core::store::Store;
use std::path::PathBuf;

/// Entry point for `klef mcp`. Loads the policy, starts the rmcp server
/// over stdio, and blocks until stdin closes.
///
/// # Errors
///
/// Returns an error if the policy file cannot be loaded or the server
/// cannot start.
pub fn run(_store: Store, _policy_path: Option<PathBuf>) -> Result<(), KlefError> {
    Err(KlefError::BackendUnavailable(
        "klef mcp: not yet implemented".into(),
    ))
}
