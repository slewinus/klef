use crate::error::KlefError;
use crate::store::Store;
use std::io::{IsTerminal, Write};

/// Print the value of `name` on stdout.
///
/// # Errors
///
/// Returns `KeyNotFound` if the key does not exist.
/// Returns `Io` if writing to stdout fails.
pub fn run_get(store: &Store, name: &str) -> Result<(), KlefError> {
    let value = store.get_value(name)?;
    let mut out = std::io::stdout().lock();
    out.write_all(value.as_bytes()).map_err(KlefError::Io)?;
    if std::io::stdout().is_terminal() {
        out.write_all(b"\n").map_err(KlefError::Io)?;
    }
    Ok(())
}

/// Print the value and metadata of `name`.
///
/// # Errors
///
/// Returns `KeyNotFound` if the key does not exist.
pub fn run_show(store: &Store, name: &str) -> Result<(), KlefError> {
    let value = store.get_value(name)?;
    let meta = store.meta(name)?;
    println!("name:    {name}");
    println!("env var: {}", meta.env_var);
    if let Some(note) = &meta.note {
        println!("note:    {note}");
    }
    println!("value:   {value}");
    Ok(())
}
