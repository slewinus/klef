use klef_core::error::KlefError;
use klef_core::store::Store;

/// Rename a secret.
///
/// # Errors
///
/// Returns an error if the key does not exist, already exists under the new name,
/// or if the backend or index fails.
pub fn run(store: &Store, old: &str, new: &str) -> Result<(), KlefError> {
    store.rename(old, new)?;
    println!("✓ '{old}' renamed to '{new}'");
    Ok(())
}
