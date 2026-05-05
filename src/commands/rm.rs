use crate::error::KlefError;
use crate::store::Store;

/// Stub for the rm command.
///
/// # Errors
///
/// Always returns `BackendUnavailable` as this is a stub.
pub fn run(_store: &Store, _name: &str, _yes: bool) -> Result<(), KlefError> {
    Err(KlefError::BackendUnavailable("not implemented".into()))
}
