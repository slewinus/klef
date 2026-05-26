//! Atomic restore: hold the exclusive lock across both phases so no other
//! klef process can mutate the store between writing the backend entries
//! and committing the index.
//!
//! See #72 for the race scenarios this guards against. Before this fix,
//! `restore_phase_1` and `restore_phase_2` each acquired and released the
//! flock independently, so a concurrent `add`/`rm`/`rename`/`restore`
//! could interleave between phases and corrupt the index/backend
//! invariants documented at the top of `commands/restore.rs`.

use super::ops::Store;
use crate::backup::BundleEntry;
use crate::error::KlefError;
use crate::store::KeyMeta;

impl Store {
    /// Atomically restore all entries from a backup bundle.
    ///
    /// Holds the exclusive file lock for the full operation, so concurrent
    /// klef processes will block (or fail with `BackendUnavailable` after
    /// the configured retry budget) instead of interleaving with the
    /// restore and corrupting the store.
    ///
    /// Replaces the legacy 2-phase API (`restore_phase_1` +
    /// `restore_phase_2`), which is now `#[deprecated]`.
    ///
    /// # Errors
    ///
    /// Returns `InvalidKeyName` or `InvalidEnvVar` if any bundle entry has
    /// a malformed name or env-var (preflight, no writes performed).
    /// Returns `BackendUnavailable` if the inter-process lock cannot be
    /// acquired within the retry budget. Propagates any backend write or
    /// index save failure encountered while the lock is held.
    pub fn restore(&self, entries: &[BundleEntry]) -> Result<(), KlefError> {
        // Preflight validation (no lock held — fast-fail before any I/O
        // mutation). Mirrors the check that used to live in phase 2.
        for entry in entries {
            super::validate_name(&entry.name)?;
            super::validate_env_var(&entry.env_var)?;
        }

        // Acquire the exclusive lock ONCE — held until end of function
        // via RAII Drop. This is the whole point of the fix (#72).
        let _lock = self.lock()?;

        // Phase 1: backend writes.
        for entry in entries {
            self.backend.set(&entry.name, &entry.value)?;
        }

        // Phase 2: rewrite index from bundle entries.
        let mut data = self.meta.load_index()?;
        for entry in entries {
            data.keys.insert(
                entry.name.clone(),
                KeyMeta {
                    env_var: entry.env_var.clone(),
                    note: entry.note.clone(),
                    tags: entry.tags.clone(),
                    added_at: entry.added_at,
                    updated_at: entry.updated_at,
                    last_used_at: None,
                },
            );
        }
        self.meta.save_index(&data)?;

        Ok(())
    }
}
