use crate::cli::ListFormat;
use crate::error::KlefError;
use crate::store::Store;

/// Stub for the list command.
///
/// # Errors
///
/// Always returns `BackendUnavailable` as this is a stub.
pub fn run(_store: &Store, _format: ListFormat) -> Result<(), KlefError> {
    Err(KlefError::BackendUnavailable("not implemented".into()))
}
