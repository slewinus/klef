use crate::error::KlefError;
use crate::store::Store;
use std::io::{IsTerminal, Read};

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
) -> Result<(), KlefError> {
    let _meta = store.meta(name)?; // confirms key exists
    let meta_only = env_var.is_some() || note.is_some();

    if meta_only {
        let note_update = note.map(Some);
        store.update_meta(name, env_var, note_update)?;
        println!("✓ '{name}' metadata updated");
        return Ok(());
    }

    let value = if std::io::stdin().is_terminal() {
        rpassword::prompt_password(format!("New value for '{name}': "))
            .map_err(|e| KlefError::BackendUnavailable(e.to_string()))?
    } else {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(KlefError::Io)?;
        buf
    };
    store.add(name, value.trim(), None, None, true)?;
    println!("✓ '{name}' value updated");
    Ok(())
}
