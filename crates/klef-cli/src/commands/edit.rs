use klef_core::error::KlefError;
use klef_core::store::Store;
use std::io::{IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Edit a key: update value (no flags) or metadata only (with --note, --as, or --note-edit).
///
/// # Errors
///
/// Returns an error if the key does not exist, reading the value fails,
/// or the backend/index operations fail.
#[allow(clippy::too_many_arguments)]
pub fn run(
    store: &Store,
    name: &str,
    env_var: Option<String>,
    note: Option<String>,
    value_from_file: Option<&Path>,
    tags: Vec<String>,
    clear_tags: bool,
    note_edit: bool,
) -> Result<(), KlefError> {
    let meta = store.meta(name)?; // confirms key exists

    if note_edit {
        return run_note_edit(store, name, meta.note.as_deref());
    }

    let want_tags_change = !tags.is_empty() || clear_tags;
    let want_other_meta = env_var.is_some() || note.is_some();

    // Tags-only path: --tag / --clear-tags without any other meta or value flag.
    // This takes priority over the value-replacement path even on a TTY.
    if want_tags_change && !want_other_meta && value_from_file.is_none() {
        let final_tags = if clear_tags { vec![] } else { tags };
        store.set_tags(name, final_tags)?;
        println!("✓ '{name}' tags updated");
        return Ok(());
    }

    // Meta-only path (original behavior preserved): explicit --note / --as, no file.
    // Optionally also update tags in the same pass.
    let meta_only = want_other_meta && value_from_file.is_none();
    if meta_only {
        if want_tags_change {
            let final_tags = if clear_tags { vec![] } else { tags };
            store.set_tags(name, final_tags)?;
        }
        let note_update = note.map(Some);
        store.update_meta(name, env_var, note_update)?;
        println!("✓ '{name}' metadata updated");
        return Ok(());
    }

    // Value-replacement path (stdin or --value-from-file).
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
    // Preserve the existing note unless explicitly overridden.
    let note_to_use = note.or_else(|| meta.note.clone());
    // Use user's tags if specified; otherwise preserve existing tags.
    let final_tags = if want_tags_change {
        if clear_tags { vec![] } else { tags }
    } else {
        meta.tags
    };
    store.add(
        name,
        value.trim_end(),
        env_var,
        note_to_use,
        final_tags,
        true,
    )?;
    println!("✓ '{name}' value updated");
    Ok(())
}

fn run_note_edit(store: &Store, name: &str, current: Option<&str>) -> Result<(), KlefError> {
    let editor = std::env::var_os("VISUAL")
        .or_else(|| std::env::var_os("EDITOR"))
        .and_then(|v| v.into_string().ok())
        .filter(|s| !s.trim().is_empty());

    let raw = if let Some(cmd) = editor {
        edit_via_external(&cmd, current.unwrap_or(""))?
    } else {
        prompt_note_stdin(name, current)?
    };

    let trimmed = raw.trim();
    let new_note = if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    };
    store.update_meta(name, None, Some(new_note))?;
    println!("✓ '{name}' note updated");
    Ok(())
}

fn edit_via_external(editor: &str, current: &str) -> Result<String, KlefError> {
    let path = scratch_path();
    std::fs::write(&path, current).map_err(KlefError::Io)?;

    let parts: Vec<&str> = editor.split_whitespace().collect();
    let (program, args) = parts
        .split_first()
        .ok_or_else(|| io_other("VISUAL/EDITOR is empty"))?;

    let status = Command::new(program)
        .args(args)
        .arg(&path)
        .status()
        .map_err(KlefError::Io);

    let result = status.and_then(|s| {
        if s.success() {
            std::fs::read_to_string(&path).map_err(KlefError::Io)
        } else {
            Err(io_other(&format!("editor exited with status {s}")))
        }
    });

    let _ = std::fs::remove_file(&path);
    result
}

fn prompt_note_stdin(name: &str, current: Option<&str>) -> Result<String, KlefError> {
    if let Some(c) = current {
        println!("Current note: {c}");
    }
    print!("New note for '{name}' (empty to clear): ");
    std::io::stdout().flush().map_err(KlefError::Io)?;
    let mut buf = String::new();
    std::io::stdin()
        .read_line(&mut buf)
        .map_err(KlefError::Io)?;
    Ok(buf)
}

fn scratch_path() -> PathBuf {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    std::env::temp_dir().join(format!("klef-edit-{pid}-{nanos}.txt"))
}

fn io_other(msg: &str) -> KlefError {
    KlefError::Io(std::io::Error::other(msg))
}
