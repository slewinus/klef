use crate::cli::StatusFormat;
use crate::error::KlefError;
use crate::store::Store;

const KLEF_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Print runtime diagnostic state.
///
/// # Errors
/// Returns an error if the index can't be loaded.
pub fn run(store: &Store, format: StatusFormat) -> Result<(), KlefError> {
    let entries = store.list()?;
    let orphans = store.orphan_index_entries()?;
    let healthy = orphans.is_empty();
    let index_path = store_index_path();
    let backend = store.backend_description();

    match format {
        StatusFormat::Text => print_text(entries.len(), &orphans, &index_path, &backend),
        StatusFormat::Json => print_json(entries.len(), &orphans, &index_path, &backend)?,
    }

    if !healthy {
        std::process::exit(1);
    }
    Ok(())
}

fn store_index_path() -> String {
    std::env::var("KLEF_INDEX_PATH").unwrap_or_else(|_| {
        dirs::config_dir().map_or_else(
            || "(unresolved)".to_string(),
            |p| p.join("klef").join("index.json").display().to_string(),
        )
    })
}

fn print_text(key_count: usize, orphans: &[String], index_path: &str, backend: &str) {
    println!("klef         {KLEF_VERSION}");
    println!("backend      {backend}");
    println!("index        {index_path}");
    println!("keys         {key_count} in index");
    if orphans.is_empty() {
        println!("desync       none");
    } else {
        println!(
            "desync       {} orphan(s) in index: {}",
            orphans.len(),
            orphans.join(", ")
        );
    }
}

fn print_json(
    key_count: usize,
    orphans: &[String],
    index_path: &str,
    backend: &str,
) -> Result<(), KlefError> {
    let body = serde_json::json!({
        "klef_version": KLEF_VERSION,
        "backend": backend,
        "index_path": index_path,
        "keys": key_count,
        "desync": orphans,
    });
    let s = serde_json::to_string_pretty(&body).map_err(|e| KlefError::IndexCorrupt {
        path: std::path::PathBuf::new(),
        reason: e.to_string(),
    })?;
    println!("{s}");
    Ok(())
}
