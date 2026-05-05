use crate::error::KlefError;
use crate::store::Store;
use std::path::Path;

/// Stub for the run command.
///
/// # Errors
///
/// Always returns `BackendUnavailable` as this is a stub.
pub fn run(_store: &Store, _env_file: &Path, _cmd: &[String]) -> Result<(), KlefError> {
    Err(KlefError::BackendUnavailable("not implemented".into()))
}
