use crate::error::KlefError;
use crate::store::Store;

/// Stub for the add command.
///
/// # Errors
///
/// Always returns `BackendUnavailable` as this is a stub.
pub fn run(
    _store: &Store,
    _name: &str,
    _env_var: Option<String>,
    _note: Option<String>,
    _force: bool,
) -> Result<(), KlefError> {
    Err(KlefError::BackendUnavailable("not implemented".into()))
}
