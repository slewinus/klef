use crate::error::KlefError;
use crate::store::Store;

/// Stub for the edit command.
///
/// # Errors
///
/// Always returns `BackendUnavailable` as this is a stub.
pub fn run(
    _store: &Store,
    _name: &str,
    _env_var: Option<String>,
    _note: Option<String>,
) -> Result<(), KlefError> {
    Err(KlefError::BackendUnavailable("not implemented".into()))
}
