mod plan;

use crate::cli::ConflictMode;
use klef_core::error::KlefError;
use klef_core::store::Store;
use plan::{DEFAULT_INCLUDE, build_plan, print_plan, walk};
use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

const DEFAULT_SKIP_PATTERNS: &[&str] = &[
    r"^PORT$",
    r"^.+_PORT$",
    r"^DEBUG$",
    r"^NODE_ENV$",
    r"^.+_TIMEOUT$",
    r"^.+_INTERVAL$",
    r"^COOKIE_.+$",
    r"^HOST$",
    r"^HOSTNAME$",
    r"^.+_HOST$",
    r"^.+_BIND$",
    r"^BIND_HOST$",
    r"^WORKER_MODE$",
    r"^ENVIRONMENT$",
];

fn compile_skip_patterns(
    user: &[String],
    use_defaults: bool,
) -> Result<Vec<regex::Regex>, KlefError> {
    let mut all: Vec<&str> = Vec::new();
    if use_defaults {
        all.extend(DEFAULT_SKIP_PATTERNS.iter().copied());
    }
    for pat in user {
        all.push(pat.as_str());
    }
    let mut compiled = Vec::with_capacity(all.len());
    for pat in &all {
        let re = regex::Regex::new(pat)
            .map_err(|e| KlefError::InvalidSkipPattern(format!("{pat}: {e}")))?;
        compiled.push(re);
    }
    Ok(compiled)
}

/// # Errors
/// Returns an error if the index can't be written to.
#[allow(clippy::too_many_arguments)]
pub fn run(
    store: &Store,
    root: Option<&Path>,
    depth: usize,
    include: Vec<String>,
    dry_run: bool,
    yes: bool,
    on_conflict: ConflictMode,
    skip_pattern: &[String],
    skip_defaults: bool,
) -> Result<(), KlefError> {
    let root = root.map_or_else(
        || std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        Path::to_path_buf,
    );
    let patterns: Vec<String> = if include.is_empty() {
        DEFAULT_INCLUDE.iter().map(|s| (*s).to_string()).collect()
    } else {
        include
    };

    let env_files = walk(&root, depth, &patterns);
    let skip_patterns = compile_skip_patterns(skip_pattern, skip_defaults)?;
    let discovered = build_plan(&env_files, on_conflict, &skip_patterns);

    print_plan(&discovered);

    if dry_run {
        return Ok(());
    }
    if discovered.picks.is_empty() {
        return Ok(());
    }

    if !yes && std::io::stdin().is_terminal() {
        let n_keys = discovered.picks.len();
        let n_files = discovered.files.len();
        print!("Import {n_keys} key(s) from {n_files} file(s)? [y/N] ");
        std::io::stdout().flush().map_err(KlefError::Io)?;
        let mut line = String::new();
        std::io::stdin()
            .lock()
            .read_line(&mut line)
            .map_err(KlefError::Io)?;
        if !matches!(line.trim().to_lowercase().as_str(), "y" | "yes") {
            println!("aborted");
            return Ok(());
        }
    }

    let mut imported = 0_usize;
    let mut skipped = Vec::<String>::new();

    for entry in &discovered.picks {
        match store.add(
            &entry.klef_name,
            &entry.value,
            Some(entry.env_var.clone()),
            None,
            vec![],
            false,
        ) {
            Ok(()) => {
                println!(
                    "✓ {} → klef:{}  (from {})",
                    entry.env_var,
                    entry.klef_name,
                    entry.source.display()
                );
                imported += 1;
            }
            Err(KlefError::KeyAlreadyExists(_)) => {
                skipped.push(entry.klef_name.clone());
            }
            Err(other) => return Err(other),
        }
    }

    println!();
    println!("Imported {imported} key(s).");
    if !skipped.is_empty() {
        println!(
            "Skipped {} (already existed): {}",
            skipped.len(),
            skipped.join(", ")
        );
    }

    Ok(())
}
