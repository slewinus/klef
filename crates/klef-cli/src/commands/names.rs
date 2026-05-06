use klef_core::error::KlefError;
use klef_core::store::Store;

/// Print each stored key name on its own line. Internal helper for shell completions.
///
/// # Errors
/// Returns an error if the index can't be loaded.
pub fn run(store: &Store) -> Result<(), KlefError> {
    for (name, _meta) in store.list()? {
        println!("{name}");
    }
    Ok(())
}
