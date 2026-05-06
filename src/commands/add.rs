use crate::error::KlefError;
use crate::store::Store;
use std::io::{IsTerminal, Read};
use std::path::Path;

/// Add or update a secret, prompting for value via terminal or stdin.
///
/// # Errors
///
/// Returns an error if reading the value fails, the name is invalid, the key
/// already exists without --force, or the backend/index operations fail.
pub fn run(
    store: &Store,
    name: &str,
    env_var: Option<String>,
    note: Option<String>,
    force: bool,
    value_from_file: Option<&Path>,
    tags: Vec<String>,
) -> Result<(), KlefError> {
    let value = read_value(name, value_from_file)?;
    store.add(name, value.trim(), env_var, note, tags, force)?;
    println!("✓ '{name}' saved");
    Ok(())
}

fn read_value(name: &str, value_from_file: Option<&Path>) -> Result<String, KlefError> {
    if let Some(path) = value_from_file {
        return std::fs::read_to_string(path).map_err(KlefError::Io);
    }
    if std::io::stdin().is_terminal() {
        let prompt = format!("Paste value for '{name}': ");
        let v = rpassword::prompt_password(prompt)
            .map_err(|e| KlefError::BackendUnavailable(e.to_string()))?;
        Ok(v)
    } else {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .map_err(KlefError::Io)?;
        Ok(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{MemoryBackend, Store};
    use tempfile::tempdir;

    fn store() -> (Store, tempfile::TempDir) {
        let d = tempdir().unwrap();
        (
            Store::new(Box::new(MemoryBackend::new()), d.path().join("i.json")),
            d,
        )
    }

    #[test]
    fn add_persists_value_and_meta() {
        let (s, _d) = store();
        s.add("stripe", "v", None, Some("hi".into()), vec![], false)
            .unwrap();
        let m = s.meta("stripe").unwrap();
        assert_eq!(m.env_var, "STRIPE_API_KEY");
        assert_eq!(m.note.as_deref(), Some("hi"));
        assert_eq!(s.get_value("stripe").unwrap(), "v");
    }

    #[test]
    fn read_value_from_file_works() {
        let d = tempdir().unwrap();
        let p = d.path().join("secret.txt");
        std::fs::write(&p, "sk_live_xyz\n").unwrap();
        let v = read_value("test", Some(&p)).unwrap();
        // Note: the consumer trims later via store.add, so the helper itself
        // doesn't strip. Here we just confirm the file content is read.
        assert_eq!(v, "sk_live_xyz\n");
    }
}
