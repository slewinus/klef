use crate::error::KlefError;
use crate::store::Store;

/// Shortcut for `klef edit <name> --note <text>`. Sets the note without
/// touching the secret value or the env-var name.
///
/// # Errors
/// Returns an error if the key is not found or the index can't be saved.
pub fn run(store: &Store, name: &str, note: &str) -> Result<(), KlefError> {
    store.update_meta(name, None, Some(Some(note.to_string())))?;
    println!("✓ '{name}' note updated");
    Ok(())
}
