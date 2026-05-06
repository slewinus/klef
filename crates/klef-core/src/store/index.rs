use crate::error::KlefError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeyMeta {
    pub env_var: String,
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub added_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    /// Last time the secret value was accessed by an explicit user action
    /// (e.g. copy from the GUI). The CLI does NOT update this — `klef get`
    /// stays a pure read so a clipboard copy from a script can't pollute
    /// the field. Optional + skip-if-none keeps backward compat with
    /// pre-v0.4 index files.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "time::serde::rfc3339::option"
    )]
    pub last_used_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexData {
    pub version: u32,
    pub keys: BTreeMap<String, KeyMeta>,
}

impl Default for IndexData {
    fn default() -> Self {
        Self {
            version: 1,
            keys: BTreeMap::new(),
        }
    }
}

pub struct IndexFile {
    path: PathBuf,
}

impl IndexFile {
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    // IndexFile is never constructed in const context.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Load index data from disk, or return empty if not found.
    ///
    /// # Errors
    /// Returns `IndexWrite` for file system errors, `IndexCorrupt` for parse errors.
    pub fn load(&self) -> Result<IndexData, KlefError> {
        if !self.path.exists() {
            return Ok(IndexData::default());
        }
        let bytes = std::fs::read(&self.path).map_err(KlefError::IndexWrite)?;
        serde_json::from_slice(&bytes).map_err(|e| KlefError::IndexCorrupt {
            path: self.path.clone(),
            reason: e.to_string(),
        })
    }

    /// Save index data atomically (write to temp, rename to final).
    ///
    /// # Errors
    /// Returns `IndexWrite` for file system errors, `IndexCorrupt` for serialization errors.
    pub fn save(&self, data: &IndexData) -> Result<(), KlefError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(KlefError::IndexWrite)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(data).map_err(|e| KlefError::IndexCorrupt {
            path: self.path.clone(),
            reason: e.to_string(),
        })?;
        std::fs::write(&tmp, bytes).map_err(KlefError::IndexWrite)?;
        std::fs::rename(&tmp, &self.path).map_err(KlefError::IndexWrite)?;
        Ok(())
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use time::macros::datetime;

    fn sample_meta() -> KeyMeta {
        KeyMeta {
            env_var: "STRIPE_API_KEY".into(),
            note: Some("prod".into()),
            tags: vec![],
            added_at: datetime!(2026-05-05 19:57:00 UTC),
            updated_at: datetime!(2026-05-05 19:57:00 UTC),
            last_used_at: None,
        }
    }

    #[test]
    fn load_missing_returns_empty() {
        let dir = tempdir().unwrap();
        let f = IndexFile::new(dir.path().join("index.json"));
        let data = f.load().unwrap();
        assert_eq!(data.version, 1);
        assert!(data.keys.is_empty());
    }

    #[test]
    fn save_then_load_round_trip() {
        let dir = tempdir().unwrap();
        let f = IndexFile::new(dir.path().join("index.json"));
        let mut data = IndexData::default();
        data.keys.insert("stripe".into(), sample_meta());
        f.save(&data).unwrap();
        let reloaded = f.load().unwrap();
        assert_eq!(reloaded, data);
    }

    #[test]
    fn save_creates_parent_dirs() {
        let dir = tempdir().unwrap();
        let f = IndexFile::new(dir.path().join("nested/sub/index.json"));
        f.save(&IndexData::default()).unwrap();
        assert!(f.path().exists());
    }

    #[test]
    fn load_corrupt_returns_index_corrupt() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("index.json");
        std::fs::write(&p, b"not json").unwrap();
        let f = IndexFile::new(p);
        assert!(matches!(f.load(), Err(KlefError::IndexCorrupt { .. })));
    }
}
