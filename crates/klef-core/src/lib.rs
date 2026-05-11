//! Core library for klef: storage backends, secret index, env-file parsing,
//! and `Store` orchestration. The CLI (`klef-cli`) and the future GUI
//! (`klef-gui`) both consume this crate; nothing here should know about TTY
//! prompts, clap, stdout formatting, or any other UI concern.

pub mod backup;
pub mod dto;
pub mod envfile;
pub mod error;
pub mod fsx;
pub mod store;

#[cfg(target_os = "macos")]
pub mod macos_keychain;

pub use dto::{BackendConfig, KeyDto, TagSummaryDto};

use error::KlefError;
use std::path::PathBuf;
use store::{AgeBackend, Backend, IndexFile, KeychainBackend, MetaStore, Store};

/// Build a `Store` from an optional backend spec string.
///
/// Spec grammar (matching the CLI `--backend` flag):
/// - `None` → keychain (production default)
/// - `Some("age:/path/to/vault.age")` → age-encrypted file backend
///
/// In debug builds, `KLEF_TEST_BACKEND=file:/path` overrides to the file
/// backend with the global index. Release builds ignore that env var.
///
/// # Errors
///
/// Returns `BackendUnavailable` if the spec is malformed or the index path
/// cannot be resolved.
pub fn build_store(backend_spec: Option<&str>) -> Result<Store, KlefError> {
    let config = match backend_spec {
        Some(spec) => Some(dto::BackendConfig::from_spec(spec)?),
        None => None,
    };
    build_store_from_config(config.as_ref())
}

/// Build a `Store` from a parsed [`BackendConfig`] (the DTO form).
///
/// Used by the GUI which persists its backend choice as JSON; the CLI goes
/// through [`build_store`] which parses the `--backend` spec string first.
///
/// `None` means "use the default" (Keychain in production, or
/// `KLEF_TEST_BACKEND` in debug builds).
///
/// # Errors
///
/// Returns `BackendUnavailable` if the index path cannot be resolved.
pub fn build_store_from_config(config: Option<&dto::BackendConfig>) -> Result<Store, KlefError> {
    let backend: Box<dyn Backend>;
    let meta: Box<dyn MetaStore>;

    match config {
        Some(dto::BackendConfig::AgeFile { path }) => {
            // Both backend and meta share the same Arc<AgeBackendInner>, so the
            // passphrase is cached across both trait calls and only one vault
            // file is ever read/written. The global index file is never touched.
            let age = AgeBackend::new(path.clone());
            backend = Box::new(age.clone());
            meta = Box::new(age);
        }
        Some(dto::BackendConfig::Keychain) => {
            backend = Box::new(KeychainBackend::new());
            meta = Box::new(IndexFile::new(index_path()?));
        }
        None => {
            if let Some(b) = backend_from_env() {
                backend = b;
            } else {
                backend = Box::new(KeychainBackend::new());
            }
            meta = Box::new(IndexFile::new(index_path()?));
        }
    }

    Ok(Store::new(backend, meta))
}

/// Resolve the canonical index file path. Honors `KLEF_INDEX_PATH` for tests
/// and falls back to `<config_dir>/klef/index.json`.
///
/// # Errors
///
/// Returns `BackendUnavailable` if the OS config directory cannot be resolved.
pub fn index_path() -> Result<PathBuf, KlefError> {
    if let Some(p) = std::env::var_os("KLEF_INDEX_PATH") {
        return Ok(PathBuf::from(p));
    }
    let base = dirs::config_dir().ok_or_else(|| {
        KlefError::BackendUnavailable("could not resolve config directory".into())
    })?;
    Ok(base.join("klef").join("index.json"))
}

/// Pick a non-default backend from `KLEF_TEST_BACKEND` if and only if this is
/// a debug build. Release binaries always return `None` so the keychain is the
/// only honored backend — the env var is simply ignored. This prevents an
/// attacker with environment-variable control from redirecting reads/writes
/// to a file they own.
#[cfg(debug_assertions)]
fn backend_from_env() -> Option<Box<dyn Backend>> {
    use store::FileBackend;
    match std::env::var("KLEF_TEST_BACKEND").as_deref() {
        Ok(spec) if spec.starts_with("file:") => {
            Some(Box::new(FileBackend::new(PathBuf::from(&spec[5..]))))
        }
        _ => None,
    }
}

#[cfg(not(debug_assertions))]
fn backend_from_env() -> Option<Box<dyn Backend>> {
    None
}
