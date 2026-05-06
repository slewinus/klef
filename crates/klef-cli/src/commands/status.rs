use crate::cli::StatusFormat;
use klef_core::error::KlefError;
use klef_core::store::Store;

const KLEF_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Print runtime diagnostic state.
///
/// # Errors
/// Returns an error if the index can't be loaded.
pub fn run(store: &Store, format: StatusFormat) -> Result<(), KlefError> {
    let entries = store.list()?;
    let index_orphans = store.orphan_index_entries()?;
    let backend_orphans = store.orphan_backend_entries()?;
    let healthy = index_orphans.is_empty() && backend_orphans.as_ref().is_none_or(Vec::is_empty);
    let index_path = store_index_path();
    let backend = store.backend_description();

    match format {
        StatusFormat::Text => print_text(
            entries.len(),
            &index_orphans,
            backend_orphans.as_deref(),
            &index_path,
            &backend,
        ),
        StatusFormat::Json => print_json(
            entries.len(),
            &index_orphans,
            backend_orphans.as_deref(),
            &index_path,
            &backend,
        )?,
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

fn print_text(
    key_count: usize,
    index_orphans: &[String],
    backend_orphans: Option<&[String]>,
    index_path: &str,
    backend: &str,
) {
    println!("klef         {KLEF_VERSION}");
    println!("backend      {backend}");
    println!("index        {index_path}");
    println!("keys         {key_count} in index");

    if index_orphans.is_empty() {
        println!("desync (i→b) none");
    } else {
        println!(
            "desync (i→b) {} orphan(s) in index: {}",
            index_orphans.len(),
            index_orphans.join(", ")
        );
    }

    match backend_orphans {
        None => println!("desync (b→i) unavailable (backend cannot enumerate)"),
        Some([]) => println!("desync (b→i) none"),
        Some(o) => println!(
            "desync (b→i) {} orphan(s) in backend: {}",
            o.len(),
            o.join(", ")
        ),
    }
}

fn print_json(
    key_count: usize,
    index_orphans: &[String],
    backend_orphans: Option<&[String]>,
    index_path: &str,
    backend: &str,
) -> Result<(), KlefError> {
    let body = serde_json::json!({
        "klef_version": KLEF_VERSION,
        "backend": backend,
        "index_path": index_path,
        "keys": key_count,
        "desync": {
            "index_to_backend": index_orphans,
            "backend_to_index": backend_orphans,
        },
    });
    let s = serde_json::to_string_pretty(&body).map_err(|e| KlefError::IndexCorrupt {
        path: std::path::PathBuf::new(),
        reason: e.to_string(),
    })?;
    println!("{s}");
    Ok(())
}
