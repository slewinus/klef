use crate::error::KlefError;
use crate::store::Store;
use std::io::{IsTerminal, Read};
use std::path::Path;

/// Edit a key: update value (no flags) or metadata only (with --note and/or --as).
///
/// # Errors
///
/// Returns an error if the key does not exist, reading the value fails,
/// or the backend/index operations fail.
pub fn run(
    store: &Store,
    name: &str,
    env_var: Option<String>,
    note: Option<String>,
    value_from_file: Option<&Path>,
) -> Result<(), KlefError> {
    let meta = store.meta(name)?; // confirms key exists
    let meta_only = (env_var.is_some() || note.is_some()) && value_from_file.is_none();

    if meta_only {
        let note_update = note.map(Some);
        store.update_meta(name, env_var, note_update)?;
        println!("✓ '{name}' metadata updated");
        return Ok(());
    }

    let value = if let Some(path) = value_from_file {
        std::fs::read_to_string(path).map_err(KlefError::Io)?
    } else if std::io::stdin().is_terminal() {
        rpassword::prompt_password(format!("New value for '{name}': "))
            .map_err(|e| KlefError::BackendUnavailable(e.to_string()))?
    } else {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(KlefError::Io)?;
        buf
    };
    // Preserve the existing note unless explicitly overridden
    let note_to_use = note.or_else(|| meta.note.clone());
    store.add(name, value.trim(), env_var, note_to_use, true)?;
    println!("✓ '{name}' value updated");
    Ok(())
}
