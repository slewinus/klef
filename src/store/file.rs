use crate::error::KlefError;
use crate::store::backend::Backend;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Default, Serialize, Deserialize)]
struct FileData {
    secrets: BTreeMap<String, String>,
}

pub struct FileBackend {
    path: PathBuf,
    lock: Mutex<()>,
}

impl FileBackend {
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    // FileBackend is never constructed in const context; const fn buys nothing here.
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            lock: Mutex::new(()),
        }
    }

    fn load(&self) -> Result<FileData, KlefError> {
        if !self.path.exists() {
            return Ok(FileData::default());
        }
        let bytes = std::fs::read(&self.path).map_err(KlefError::Io)?;
        serde_json::from_slice(&bytes).map_err(|e| KlefError::IndexCorrupt {
            path: self.path.clone(),
            reason: e.to_string(),
        })
    }

    fn save(&self, data: &FileData) -> Result<(), KlefError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(KlefError::Io)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        // serde_json::to_vec_pretty is infallible for BTreeMap<String, String>,
        // but mapping to IndexWrite keeps the error contract honest if the
        // shape ever changes to one that can fail to serialize.
        let bytes = serde_json::to_vec_pretty(data)
            .map_err(|e| KlefError::IndexWrite(std::io::Error::other(e.to_string())))?;
        std::fs::write(&tmp, bytes).map_err(KlefError::IndexWrite)?;
        std::fs::rename(&tmp, &self.path).map_err(KlefError::IndexWrite)?;
        Ok(())
    }
}

impl Backend for FileBackend {
    fn describe(&self) -> String {
        format!("file:{}", self.path.display())
    }

    fn get(&self, name: &str) -> Result<String, KlefError> {
        let _g = self.lock.lock().unwrap();
        let data = self.load()?;
        data.secrets
            .get(name)
            .cloned()
            .ok_or_else(|| KlefError::KeyNotFound(name.to_string()))
    }

    fn set(&self, name: &str, value: &str) -> Result<(), KlefError> {
        let _g = self.lock.lock().unwrap();
        let mut data = self.load()?;
        data.secrets.insert(name.to_string(), value.to_string());
        self.save(&data)
    }

    fn remove(&self, name: &str) -> Result<(), KlefError> {
        let _g = self.lock.lock().unwrap();
        let mut data = self.load()?;
        data.secrets
            .remove(name)
            .ok_or_else(|| KlefError::KeyNotFound(name.to_string()))?;
        self.save(&data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn round_trip_persists_to_disk() {
        let d = tempdir().unwrap();
        let p = d.path().join("secrets.json");
        let b = FileBackend::new(p.clone());
        b.set("k", "v").unwrap();

        // Simulate another process opening the same file.
        let b2 = FileBackend::new(p);
        assert_eq!(b2.get("k").unwrap(), "v");
    }

    #[test]
    fn missing_key_returns_not_found() {
        let d = tempdir().unwrap();
        let b = FileBackend::new(d.path().join("s.json"));
        assert!(matches!(b.get("nope"), Err(KlefError::KeyNotFound(_))));
    }

    #[test]
    fn remove_then_get_fails() {
        let d = tempdir().unwrap();
        let b = FileBackend::new(d.path().join("s.json"));
        b.set("k", "v").unwrap();
        b.remove("k").unwrap();
        assert!(matches!(b.get("k"), Err(KlefError::KeyNotFound(_))));
    }
}
