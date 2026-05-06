//! Inter-process advisory lock for klef vault / index files.
//!
//! Each `Store` mutation acquires an exclusive flock on a sibling `.lock`
//! file before doing the load → mutate → save sequence. This prevents two
//! concurrent klef processes (e.g. CLI + future GUI) from racing each other
//! and silently losing writes (closes #61).
//!
//! Reads are intentionally NOT locked: the atomic tmp+rename pattern in
//! every save guarantees readers see either the previous state or the new
//! one, never a torn file.
//!
//! ## Lifecycle
//!
//! - Acquire: tries `try_lock_exclusive` up to a few times with a short
//!   backoff. If contention persists past the budget, returns
//!   `BackendUnavailable("vault is locked …")` so the GUI doesn't freeze.
//! - Release: RAII via `Drop`. The kernel also releases on process exit, so
//!   a hard kill of one klef process leaves no stale lock for the next.
//!
//! ## Platform notes
//!
//! - Unix: `flock(LOCK_EX | LOCK_NB)` — advisory, well-behaved.
//! - Windows: `LockFileEx` — mandatory locks; works the same from klef's
//!   perspective but other processes that open the file get errors instead
//!   of being merely advised.

use crate::error::KlefError;
use fs4::fs_std::FileExt;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Number of `try_lock` attempts before giving up.
const MAX_ATTEMPTS: u32 = 5;
/// Wait between attempts. Total budget ≈ (`MAX_ATTEMPTS` - 1) × this.
const RETRY_DELAY: Duration = Duration::from_millis(50);

/// RAII guard for an exclusive file lock.
///
/// Drop releases the lock. If the process dies, the kernel releases it.
pub struct FileLock {
    file: File,
    #[allow(dead_code)] // kept for diagnostics
    path: PathBuf,
}

impl FileLock {
    /// Acquire an exclusive lock on `<resource>.lock` next to the resource.
    ///
    /// `resource_path` is the file the lock protects (e.g. the index or the
    /// age vault). The lock file itself is `<resource_path>.lock`. The
    /// resource file need not exist — only its parent directory.
    ///
    /// # Errors
    ///
    /// Returns `BackendUnavailable` if the lock cannot be acquired within
    /// the retry budget (another klef process holds it), or `Io` if the
    /// lock file can't be opened (parent missing, permissions).
    pub fn acquire(resource_path: &Path) -> Result<Self, KlefError> {
        let lock_path = lock_path_for(resource_path);
        if let Some(parent) = lock_path.parent() {
            std::fs::create_dir_all(parent).map_err(KlefError::Io)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(KlefError::Io)?;

        for attempt in 0..MAX_ATTEMPTS {
            match FileExt::try_lock_exclusive(&file) {
                Ok(true) => {
                    return Ok(Self {
                        file,
                        path: lock_path,
                    });
                }
                Ok(false) => {
                    if attempt + 1 < MAX_ATTEMPTS {
                        std::thread::sleep(RETRY_DELAY);
                    }
                }
                Err(e) => {
                    return Err(KlefError::BackendUnavailable(format!(
                        "could not lock {}: {e}",
                        lock_path.display()
                    )));
                }
            }
        }
        Err(KlefError::BackendUnavailable(format!(
            "vault is locked by another klef process ({}); \
             close the other klef invocation and retry",
            lock_path.display()
        )))
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

/// `<path>.lock`, used as the advisory-lock file for `path`.
fn lock_path_for(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_owned();
    name.push(".lock");
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn acquire_and_drop_releases() {
        let d = tempdir().unwrap();
        let resource = d.path().join("vault.age");
        let l1 = FileLock::acquire(&resource).unwrap();
        drop(l1);
        // Second acquire after drop must succeed without retries kicking in.
        let _l2 = FileLock::acquire(&resource).unwrap();
    }

    #[test]
    fn second_acquire_while_held_returns_busy() {
        let d = tempdir().unwrap();
        let resource = d.path().join("vault.age");
        let _held = FileLock::acquire(&resource).unwrap();
        let r = FileLock::acquire(&resource);
        assert!(matches!(r, Err(KlefError::BackendUnavailable(_))));
    }

    #[test]
    fn lock_path_is_sibling_dot_lock() {
        let p = std::path::Path::new("/tmp/v.age");
        assert_eq!(
            lock_path_for(p),
            std::path::PathBuf::from("/tmp/v.age.lock")
        );
    }
}
