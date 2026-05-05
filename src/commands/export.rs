use crate::cli::ExportFormat;
use crate::error::KlefError;
use crate::store::Store;

/// Stub for the export command.
///
/// # Errors
///
/// Always returns `BackendUnavailable` as this is a stub.
pub fn run(_store: &Store, _names: &[String], _format: ExportFormat) -> Result<(), KlefError> {
    Err(KlefError::BackendUnavailable("not implemented".into()))
}
