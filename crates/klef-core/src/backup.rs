//! On-disk format types for `klef backup` / `klef restore`.
//!
//! This module owns the JSON bundle schema (versioned, strict) and the
//! `from_store` constructor. The actual age encryption, file I/O, and TTY
//! prompts live in the CLI (`crates/klef-cli/src/commands/backup.rs`).
//!
//! The split exists because `Store::restore_phase_*` needs `BundleEntry`,
//! and the future GUI needs the same types without depending on the CLI.

use crate::error::KlefError;
use crate::store::Store;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Versioned, strict-schema bundle as written to disk before age encryption.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    pub format_version: u32,
    pub tool: String,
    pub klef_version: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    pub source: BundleSource,
    pub entries: Vec<BundleEntry>,
}

/// Source machine metadata recorded at backup time.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleSource {
    pub hostname: String,
    pub platform: String,
}

/// One vault entry as stored in a bundle.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleEntry {
    pub name: String,
    pub value: String,
    pub keychain_service: String,
    pub keychain_account: String,
    pub env_var: String,
    pub note: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub added_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

impl Bundle {
    pub const FORMAT_VERSION: u32 = 1;
    pub const TOOL: &'static str = "klef";

    /// Build a `Bundle` snapshot from the current `Store` state.
    ///
    /// # Errors
    ///
    /// Returns an error if the store cannot be read or a value cannot be
    /// retrieved from the backend.
    pub fn from_store(store: &Store) -> Result<Self, KlefError> {
        let entries_meta = store.list()?;
        let mut entries = Vec::with_capacity(entries_meta.len());
        for (name, meta) in entries_meta {
            let value = store.get_value(&name)?;
            entries.push(BundleEntry {
                keychain_service: "klef".to_string(),
                keychain_account: name.clone(),
                env_var: meta.env_var,
                note: meta.note,
                tags: meta.tags,
                added_at: meta.added_at,
                updated_at: meta.updated_at,
                name,
                value,
            });
        }
        Ok(Self {
            format_version: Self::FORMAT_VERSION,
            tool: Self::TOOL.to_string(),
            klef_version: env!("CARGO_PKG_VERSION").to_string(),
            created_at: OffsetDateTime::now_utc(),
            source: BundleSource {
                hostname: gethostname::gethostname().to_string_lossy().into_owned(),
                platform: detect_platform().to_string(),
            },
            entries,
        })
    }
}

const fn detect_platform() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "macos"
    }
    #[cfg(target_os = "linux")]
    {
        "linux"
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        "other"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_serializes_and_strict_schema_rejects_unknown_fields() {
        let bundle = Bundle {
            format_version: 1,
            tool: "klef".to_string(),
            klef_version: "0.2.0".to_string(),
            created_at: time::macros::datetime!(2026-05-06 12:00:00 UTC),
            source: BundleSource {
                hostname: "test-host".to_string(),
                platform: "macos".to_string(),
            },
            entries: vec![BundleEntry {
                name: "stripe-prod".to_string(),
                value: "sk_live_xxxxx".to_string(),
                keychain_service: "klef".to_string(),
                keychain_account: "stripe-prod".to_string(),
                env_var: "STRIPE_API_KEY".to_string(),
                note: Some("compte prod".to_string()),
                tags: vec![],
                added_at: time::macros::datetime!(2026-05-05 19:57:00 UTC),
                updated_at: time::macros::datetime!(2026-05-06 08:30:00 UTC),
            }],
        };

        let json = serde_json::to_string(&bundle).unwrap();
        let back: Bundle = serde_json::from_str(&json).unwrap();
        assert_eq!(back.format_version, 1);
        assert_eq!(back.tool, "klef");
        assert_eq!(back.entries.len(), 1);
        assert_eq!(back.entries[0].name, "stripe-prod");

        let bad = r#"{"format_version":1,"tool":"klef","klef_version":"0.2.0",
            "created_at":"2026-05-06T12:00:00Z",
            "source":{"hostname":"h","platform":"macos"},
            "entries":[],"unknown_field":"oops"}"#;
        assert!(
            serde_json::from_str::<Bundle>(bad).is_err(),
            "expected rejection of unknown field"
        );
    }

    #[test]
    fn detect_platform_returns_known_string() {
        let p = detect_platform();
        assert!(
            p == "macos" || p == "linux" || p == "other",
            "unexpected platform: {p}"
        );
    }
}
