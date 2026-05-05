use crate::cli::ListFormat;
use crate::error::KlefError;
use crate::store::Store;

/// List all stored keys in the requested format (table or JSON).
///
/// # Errors
///
/// Returns an error if the store fails to load the index or secrets.
pub fn run(store: &Store, format: ListFormat) -> Result<(), KlefError> {
    let entries = store.list()?;
    match format {
        ListFormat::Table => print_table(&entries),
        ListFormat::Json => print_json(&entries)?,
    }
    Ok(())
}

fn print_table(entries: &[(String, crate::store::KeyMeta)]) {
    if entries.is_empty() {
        println!("(no keys stored)");
        return;
    }
    let name_w = entries
        .iter()
        .map(|(n, _)| n.len())
        .max()
        .unwrap_or(4)
        .max(4);
    let var_w = entries
        .iter()
        .map(|(_, m)| m.env_var.len())
        .max()
        .unwrap_or(7)
        .max(7);
    println!("{:<name_w$}  {:<var_w$}  NOTE", "NAME", "ENV_VAR");
    for (name, meta) in entries {
        let note = meta.note.as_deref().unwrap_or("-");
        println!("{name:<name_w$}  {:<var_w$}  {note}", meta.env_var);
    }
}

fn print_json(entries: &[(String, crate::store::KeyMeta)]) -> Result<(), KlefError> {
    let map: std::collections::BTreeMap<_, _> = entries
        .iter()
        .map(|(n, m)| (n.clone(), m.clone()))
        .collect();
    let s = serde_json::to_string_pretty(&map).map_err(|e| KlefError::IndexCorrupt {
        path: std::path::PathBuf::new(),
        reason: e.to_string(),
    })?;
    println!("{s}");
    Ok(())
}
