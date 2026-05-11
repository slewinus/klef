//! Unit tests for the TTL-based session pruning.
//!
//! Lives in a separate file so `mod.rs` stays under the line-cap and the
//! `unchecked_time_subtraction` clippy lint can be scoped without polluting
//! production code.

#![allow(clippy::unchecked_time_subtraction)]

use super::*;

fn mk_plan(created_at: std::time::Instant) -> ServerPlan {
    ServerPlan {
        source_path: std::path::PathBuf::from("/tmp/.env"),
        suggested_project: "test".to_string(),
        items: vec![],
        created_seq: 0,
        created_at,
    }
}

#[test]
fn prune_drops_sessions_past_ttl() {
    let mut sessions = std::collections::HashMap::new();
    // One session well past TTL, one fresh.
    let past = std::time::Instant::now() - SESSION_TTL - std::time::Duration::from_secs(1);
    sessions.insert("old".to_string(), mk_plan(past));
    sessions.insert("new".to_string(), mk_plan(std::time::Instant::now()));
    prune_expired(&mut sessions);
    assert!(
        !sessions.contains_key("old"),
        "expired session should be gone"
    );
    assert!(sessions.contains_key("new"), "fresh session should survive");
}

#[test]
fn prune_keeps_sessions_inside_ttl() {
    let mut sessions = std::collections::HashMap::new();
    // 30 seconds old — comfortably inside the 5-min window.
    let recent = std::time::Instant::now() - std::time::Duration::from_secs(30);
    sessions.insert("recent".to_string(), mk_plan(recent));
    prune_expired(&mut sessions);
    assert!(sessions.contains_key("recent"));
}
