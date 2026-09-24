//! Tauri-managed state and the off-main-thread helper every store command uses.

use klef_core::store::Store;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// Cheap to clone (all `Arc`s) so async commands can move a copy into
/// `spawn_blocking`.
#[derive(Clone)]
pub struct AppState {
    /// Behind a `Mutex` for two reasons: commands now run concurrently off the
    /// main thread, and the vault can be swapped at runtime (keychain ⇄ age).
    /// Serializing store calls here also keeps two GUI commands from racing
    /// for klef-core's inter-process flock and failing with "vault is locked".
    store: Arc<Mutex<Store>>,
    /// Server-side state for dotenv-import. See `dotenv_import` for the
    /// threat model — plaintext values never round-trip through JS.
    pub dotenv_sessions: Arc<Mutex<HashMap<String, crate::dotenv_import::ServerPlan>>>,
}

impl AppState {
    #[must_use]
    pub fn new(store: Store) -> Self {
        Self {
            store: Arc::new(Mutex::new(store)),
            dotenv_sessions: Arc::default(),
        }
    }

    /// Poisoning is ignored: the disk is the source of truth, so a panic
    /// mid-call leaves no in-memory invariant to protect.
    pub fn store(&self) -> MutexGuard<'_, Store> {
        self.store.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn swap_store(&self, store: Store) {
        *self.store() = store;
    }
}

/// Run blocking store work (keychain calls, which can raise a macOS password
/// prompt; age decrypts, which pay scrypt) on the blocking pool. Plain sync
/// Tauri commands run on the main thread and would freeze the popover.
pub async fn blocking<T, E>(f: impl FnOnce() -> Result<T, E> + Send + 'static) -> Result<T, String>
where
    T: Send + 'static,
    E: std::fmt::Display + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}
