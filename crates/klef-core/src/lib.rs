//! Core library for klef: storage backends, secret index, env-file parsing,
//! and `Store` orchestration. The CLI (`klef-cli`) and the future GUI
//! (`klef-gui`) both consume this crate; nothing here should know about TTY
//! prompts, clap, stdout formatting, or any other UI concern.

pub mod backup;
pub mod envfile;
pub mod error;
pub mod store;

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
    let backend: Box<dyn Backend>;
    let meta: Box<dyn MetaStore>;

    if let Some(spec) = backend_spec {
        if let Some(path) = spec.strip_prefix("age:") {
            if path.is_empty() {
                return Err(KlefError::BackendUnavailable(
                    "--backend age: requires a path (e.g. age:/path/to/secrets.age)".to_string(),
                ));
            }
            // Both backend and meta share the same Arc<AgeBackendInner>, so the
            // passphrase is cached across both trait calls and only one vault
            // file is ever read/written. The global index file is never touched.
            let age = AgeBackend::new(PathBuf::from(path));
            backend = Box::new(age.clone());
            meta = Box::new(age);
        } else if spec.starts_with("file:") {
            return Err(KlefError::BackendUnavailable(
                "file: backend is debug-only; use age: for production".to_string(),
            ));
        } else {
            return Err(KlefError::BackendUnavailable(format!(
                "unknown backend spec '{spec}' (supported: age:/path/to/file.age)"
            )));
        }
    } else if let Some(b) = backend_from_env() {
        backend = b;
        meta = Box::new(IndexFile::new(index_path()?));
    } else {
        backend = Box::new(KeychainBackend::new());
        meta = Box::new(IndexFile::new(index_path()?));
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
