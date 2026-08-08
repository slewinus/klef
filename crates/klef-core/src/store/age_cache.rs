//! File-stamp bookkeeping for the age backend's decrypted-vault cache.
//!
//! Extracted from `age_backend` so that file stays under the 300-line cap,
//! same as `age_crypto`.
//!
//! # Why a cache exists at all
//!
//! A single `Store::add` calls `load_index` → `get` → `set` → `save_index`, and
//! `AgeBackend` serves all four. Without a cache each one decrypts the whole
//! vault from scratch: four scrypt derivations plus two more for the
//! re-encrypts. Measured on an M-series laptop that is ~8s to add one key, and
//! ~3s to read one. scrypt is deliberately expensive; the fix is to stop paying
//! it repeatedly for the same bytes, not to weaken the KDF.
//!
//! # Why stamps and not a TTL
//!
//! Another klef process can publish a new vault at any time. klef only ever does
//! that via tmp + rename under the exclusive flock, so a new version always has
//! a different `(len, mtime)`. Comparing the stamp is therefore exact for every
//! writer that goes through klef, and needs no invalidation protocol between
//! processes.

use std::path::Path;
use std::time::SystemTime;

/// Identity of one version of a vault file: `(len, mtime)`.
pub(super) type Stamp = (u64, Option<SystemTime>);

/// Stamp `path`, or `None` if it can't be stat'd (missing file, permissions).
///
/// `None` never compares equal to a cached stamp, so an unreadable file always
/// misses the cache — the safe direction.
///
// ponytail: an in-place rewrite by a non-klef process within the same mtime
// tick and at an identical length would be missed. Hash the ciphertext if that
// ever stops being theoretical.
pub(super) fn stamp(path: &Path) -> Option<Stamp> {
    let m = std::fs::metadata(path).ok()?;
    Some((m.len(), m.modified().ok()))
}

/// Read a cache entry, but only if it still describes the file at `current`.
///
/// An unstampable file (`current == None`) always misses, even against an entry
/// that also failed to stamp — `None == None` must not be read as "unchanged".
pub(super) fn hit<T: Clone>(entry: Option<&(Stamp, T)>, current: Option<Stamp>) -> Option<T> {
    let (cached, value) = entry?;
    (current? == *cached).then(|| value.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn missing_file_has_no_stamp() {
        let d = tempdir().unwrap();
        assert!(stamp(&d.path().join("nope.age")).is_none());
    }

    #[test]
    fn rewriting_at_a_different_length_changes_the_stamp() {
        let d = tempdir().unwrap();
        let p = d.path().join("v.age");
        std::fs::write(&p, b"short").unwrap();
        let before = stamp(&p).unwrap();
        std::fs::write(&p, b"a considerably longer payload").unwrap();
        assert_ne!(before, stamp(&p).unwrap());
    }

    #[test]
    fn an_untouched_file_keeps_its_stamp() {
        let d = tempdir().unwrap();
        let p = d.path().join("v.age");
        std::fs::write(&p, b"payload").unwrap();
        assert_eq!(stamp(&p).unwrap(), stamp(&p).unwrap());
    }

    #[test]
    fn hit_returns_the_value_only_on_an_identical_stamp() {
        let entry = ((7u64, None), "cached".to_string());
        assert_eq!(hit(Some(&entry), Some((7, None))), Some("cached".into()));
        assert_eq!(hit(Some(&entry), Some((8, None))), None);
    }

    #[test]
    fn hit_misses_on_an_unstampable_file() {
        // Both sides failing to stat must not read as "unchanged".
        let entry = ((7u64, None), "cached".to_string());
        assert_eq!(hit(Some(&entry), None), None);
        assert_eq!(hit(None::<&(Stamp, String)>, Some((7, None))), None);
    }
}
