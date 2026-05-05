use crate::error::KlefError;
use crate::store::Store;

/// Stub for the get command.
///
/// # Errors
///
/// Always returns `BackendUnavailable` as this is a stub.
pub fn run_get(_store: &Store, _name: &str) -> Result<(), KlefError> {
    Err(KlefError::BackendUnavailable("not implemented".into()))
}

/// Stub for the show command.
///
/// # Errors
///
/// Always returns `BackendUnavailable` as this is a stub.
pub fn run_show(_store: &Store, _name: &str) -> Result<(), KlefError> {
    Err(KlefError::BackendUnavailable("not implemented".into()))
}
