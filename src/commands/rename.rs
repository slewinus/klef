use crate::error::KlefError;
use crate::store::Store;

/// Stub for the rename command.
///
/// # Errors
///
/// Always returns `BackendUnavailable` as this is a stub.
pub fn run(_store: &Store, _old: &str, _new: &str) -> Result<(), KlefError> {
    Err(KlefError::BackendUnavailable("not implemented".into()))
}
