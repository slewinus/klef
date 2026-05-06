pub mod cli;
pub mod commands;
pub mod envfile;
pub mod error;
pub mod store;

use cli::{Cli, Command};
use error::KlefError;
use std::path::PathBuf;
use store::{Backend, KeychainBackend, Store};

/// Dispatch the parsed CLI to the appropriate command handler.
///
/// # Errors
///
/// Returns an error if the backend or command dispatch fails.
pub fn run(cli: Cli) -> Result<(), KlefError> {
    let store = build_store()?;
    match cli.command {
        Command::Add {
            name,
            r#as,
            note,
            force,
            value_from_file,
        } => commands::add::run(&store, &name, r#as, note, force, value_from_file.as_deref()),
        Command::Get { name } => commands::get::run_get(&store, &name),
        Command::Show { name } => commands::get::run_show(&store, &name),
        Command::List {
            format,
            verbose,
            filter,
        } => commands::list::run(&store, format, verbose, filter.as_deref()),
        Command::Rm { name, yes } => commands::rm::run(&store, &name, yes),
        Command::Edit {
            name,
            note,
            r#as,
            value_from_file,
        } => commands::edit::run(&store, &name, r#as, note, value_from_file.as_deref()),
        Command::Rename { old, new } => commands::rename::run(&store, &old, &new),
        Command::SetNote { name, note } => commands::set_note::run(&store, &name, &note),
        Command::Export { names, format } => commands::export::run(&store, &names, format),
        Command::Run { env_file, cmd } => commands::run::run(&store, &env_file, &cmd),
        Command::Completions { shell } => commands::completions::run(shell),
        Command::Status { format } => commands::status::run(&store, format),
        Command::Import {
            file,
            prefix,
            dry_run,
            rewrite,
            yes,
        } => commands::import::run(&store, &file, prefix.as_deref(), dry_run, rewrite, yes),
        Command::Discover {
            root,
            depth,
            include,
            dry_run,
            yes,
            on_conflict,
            skip_pattern,
            skip_defaults,
        } => commands::discover::run(
            &store,
            root.as_deref(),
            depth,
            include,
            dry_run,
            yes,
            on_conflict,
            &skip_pattern,
            skip_defaults,
        ),
        Command::Backup { output, recipient } => commands::backup::run(&store, &output, &recipient),
        Command::Restore { input, force } => commands::restore::run(&store, &input, force),
        Command::Names => commands::names::run(&store),
    }
}

fn build_store() -> Result<Store, KlefError> {
    let index_path = index_path()?;
    let backend = backend_from_env().unwrap_or_else(|| Box::new(KeychainBackend::new()));
    Ok(Store::new(backend, index_path))
}

/// Pick a non-default backend from `KLEF_TEST_BACKEND` if and only if this is a
/// debug build. Release binaries (`cargo install`, `cargo build --release`)
/// always return `None` so the keychain is the only honored backend — the env
/// var is simply ignored. This prevents an attacker with environment-variable
/// control from redirecting reads/writes to a file they own.
#[cfg(debug_assertions)]
fn backend_from_env() -> Option<Box<dyn Backend>> {
    use store::FileBackend;
    match std::env::var("KLEF_TEST_BACKEND").as_deref() {
        Ok(spec) if spec.starts_with("file:") => {
            Some(Box::new(FileBackend::new(PathBuf::from(&spec[5..]))))
        }
        _ => None,
    }
}

#[cfg(not(debug_assertions))]
fn backend_from_env() -> Option<Box<dyn Backend>> {
    None
}

fn index_path() -> Result<PathBuf, KlefError> {
    if let Some(p) = std::env::var_os("KLEF_INDEX_PATH") {
        return Ok(PathBuf::from(p));
    }
    let base = dirs::config_dir().ok_or_else(|| {
        KlefError::BackendUnavailable("could not resolve config directory".into())
    })?;
    Ok(base.join("klef").join("index.json"))
}
