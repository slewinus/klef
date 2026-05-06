use crate::error::KlefError;
use crate::store::Store;

/// List all tags in the vault with the count of keys carrying each.
///
/// # Errors
/// Returns an error if the index can't be loaded.
pub fn run(store: &Store) -> Result<(), KlefError> {
    let counts = store.tags_with_counts()?;
    if counts.is_empty() {
        println!("(no tags in use)");
        return Ok(());
    }
    let name_w = counts.keys().map(String::len).max().unwrap_or(3).max(3);
    println!("{:<name_w$}  KEYS", "TAG");
    for (tag, count) in counts {
        println!("{tag:<name_w$}  {count}");
    }
    Ok(())
}
