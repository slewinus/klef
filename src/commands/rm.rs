use crate::error::KlefError;
use crate::store::Store;
use std::io::{BufRead, IsTerminal, Write};

/// Remove a secret from the store.
///
/// If stdin is a TTY and `--yes` is not set, prompts for confirmation.
///
/// # Errors
///
/// Returns `KlefError` if the removal fails.
pub fn run(store: &Store, name: &str, yes: bool) -> Result<(), KlefError> {
    if !yes && std::io::stdin().is_terminal() {
        print!("Delete '{name}'? [y/N] ");
        std::io::stdout().flush().ok();
        let mut line = String::new();
        std::io::stdin().lock().read_line(&mut line).ok();
        if !matches!(line.trim().to_lowercase().as_str(), "y" | "yes") {
            println!("aborted");
            return Ok(());
        }
    }
    store.remove(name)?;
    println!("✓ '{name}' removed");
    Ok(())
}
