//! Data-transfer objects shared across `klef-core`'s consumers.
//!
//! These types are the public contract used by the CLI, GUI, and a future
//! MCP server: stable, serializable, and intentionally not exposing internal
//! types like `KeyMeta` or `Backend`.
//!
//! Crucially, no DTO carries the secret value. Values are fetched lazily via
//! `Store::get_value(name)` at the moment of use (clipboard copy, etc.) so
//! that long-lived UI state never holds plaintext.

use crate::error::KlefError;
use crate::store::KeyMeta;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use time::OffsetDateTime;

/// One key as exposed to a UI (CLI render, GUI list, MCP response).
///
/// Does **not** include the value. Fetch with `Store::get_value(name)` at the
/// point of use.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeyDto {
    pub name: String,
    pub env_var: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub added_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

impl From<(String, KeyMeta)> for KeyDto {
    fn from((name, meta): (String, KeyMeta)) -> Self {
        Self {
            name,
            env_var: meta.env_var,
            tags: meta.tags,
            note: meta.note,
            added_at: meta.added_at,
            updated_at: meta.updated_at,
        }
    }
}

/// One tag with its key count, as returned by `klef tags` and used by the
/// GUI sidebar.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TagSummaryDto {
    pub name: String,
    pub key_count: usize,
}

impl From<(String, usize)> for TagSummaryDto {
    fn from((name, key_count): (String, usize)) -> Self {
        Self { name, key_count }
    }
}

/// Persisted backend selection.
///
/// The GUI stores this in its settings file and passes it to `Store`
/// construction at startup. The CLI uses the `--backend` flag, which maps to
/// the same enum via [`BackendConfig::from_spec`].
///
/// `Keychain` is the production default on macOS and Linux desktop sessions.
/// `AgeFile` covers headless / CI / Docker environments and the
/// "encrypted-file" backend opt-in.
///
/// **Field naming**: this enum (and all DTOs in this module) serializes with
/// `snake_case` to match the rest of klef's on-disk schema (`KeyMeta`,
/// `Bundle`). JS consumers via Tauri can read `cfg.kind` / `cfg.path`
/// directly with no translation cost.
///
/// **No backup recipients here**: `klef backup --recipient <key>` is a
/// per-operation argument, not a property of the persisted backend. The GUI
/// will track recipients separately (`BackupConfig` DTO) when it implements
/// the backup UI in a later sprint, or it'll just be a per-operation flag
/// passed at the moment of `Bundle` creation. Either way, not a property of
/// `BackendConfig`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BackendConfig {
    Keychain,
    AgeFile { path: PathBuf },
}

impl BackendConfig {
    /// Render this config as a `--backend` spec string compatible with the
    /// CLI flag. Returns `None` for `Keychain` (it has no spec; absence of
    /// `--backend` means keychain).
    #[must_use]
    pub fn to_spec(&self) -> Option<String> {
        match self {
            Self::Keychain => None,
            Self::AgeFile { path } => Some(format!("age:{}", path.display())),
        }
    }

    /// Parse a `--backend` spec string into a `BackendConfig`.
    ///
    /// # Errors
    ///
    /// Returns `BackendUnavailable` if the spec is malformed or unknown.
    pub fn from_spec(spec: &str) -> Result<Self, KlefError> {
        if let Some(path) = spec.strip_prefix("age:") {
            if path.is_empty() {
                return Err(KlefError::BackendUnavailable(
                    "--backend age: requires a path (e.g. age:/path/to/secrets.age)".to_string(),
                ));
            }
            return Ok(Self::AgeFile {
                path: PathBuf::from(path),
            });
        }
        if spec.starts_with("file:") {
            return Err(KlefError::BackendUnavailable(
                "file: backend is debug-only; use age: for production".to_string(),
            ));
        }
        Err(KlefError::BackendUnavailable(format!(
            "unknown backend spec '{spec}' (supported: age:/path/to/file.age)"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn sample_meta() -> KeyMeta {
        KeyMeta {
            env_var: "STRIPE_API_KEY".to_string(),
            note: Some("compte prod".to_string()),
            tags: vec!["billing".to_string(), "prod".to_string()],
            added_at: datetime!(2026-05-05 19:57:00 UTC),
            updated_at: datetime!(2026-05-06 08:30:00 UTC),
        }
    }

    #[test]
    fn key_dto_from_meta_preserves_all_fields() {
        let dto: KeyDto = ("stripe-prod".to_string(), sample_meta()).into();
        assert_eq!(dto.name, "stripe-prod");
        assert_eq!(dto.env_var, "STRIPE_API_KEY");
        assert_eq!(dto.tags, vec!["billing", "prod"]);
        assert_eq!(dto.note.as_deref(), Some("compte prod"));
        assert_eq!(dto.added_at, datetime!(2026-05-05 19:57:00 UTC));
        assert_eq!(dto.updated_at, datetime!(2026-05-06 08:30:00 UTC));
    }

    #[test]
    fn key_dto_round_trips_through_json() {
        let original: KeyDto = ("stripe".to_string(), sample_meta()).into();
        let json = serde_json::to_string(&original).unwrap();
        let back: KeyDto = serde_json::from_str(&json).unwrap();
        assert_eq!(original, back);
    }

    #[test]
    fn key_dto_skips_empty_optional_fields_in_json() {
        let dto = KeyDto {
            name: "x".to_string(),
            env_var: "X".to_string(),
            tags: vec![],
            note: None,
            added_at: datetime!(2026-05-05 0:00 UTC),
            updated_at: datetime!(2026-05-05 0:00 UTC),
        };
        let json = serde_json::to_string(&dto).unwrap();
        assert!(!json.contains("tags"));
        assert!(!json.contains("note"));
    }

    #[test]
    fn tag_summary_dto_round_trips() {
        let summary: TagSummaryDto = ("billing".to_string(), 3).into();
        let json = serde_json::to_string(&summary).unwrap();
        let back: TagSummaryDto = serde_json::from_str(&json).unwrap();
        assert_eq!(summary, back);
    }

    #[test]
    fn backend_config_keychain_has_no_spec() {
        assert!(BackendConfig::Keychain.to_spec().is_none());
    }

    #[test]
    fn backend_config_age_file_round_trips_through_spec() {
        let cfg = BackendConfig::AgeFile {
            path: PathBuf::from("/tmp/x.age"),
        };
        let spec = cfg.to_spec().unwrap();
        assert_eq!(spec, "age:/tmp/x.age");
        let back = BackendConfig::from_spec(&spec).unwrap();
        assert_eq!(cfg, back);
    }

    #[test]
    fn backend_config_round_trips_through_json() {
        let cfg = BackendConfig::AgeFile {
            path: PathBuf::from("/tmp/x.age"),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: BackendConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, back);
    }

    #[test]
    fn backend_config_keychain_serializes_with_kind_tag() {
        let json = serde_json::to_string(&BackendConfig::Keychain).unwrap();
        assert_eq!(json, r#"{"kind":"keychain"}"#);
    }

    #[test]
    fn backend_config_from_spec_rejects_empty_age() {
        assert!(BackendConfig::from_spec("age:").is_err());
    }

    #[test]
    fn backend_config_from_spec_rejects_file_backend() {
        assert!(BackendConfig::from_spec("file:/tmp/x.json").is_err());
    }

    #[test]
    fn backend_config_from_spec_rejects_unknown_scheme() {
        assert!(BackendConfig::from_spec("vault:/foo").is_err());
    }
}
