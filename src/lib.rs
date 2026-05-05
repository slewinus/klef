pub mod cli;
pub mod commands;
pub mod envfile;
pub mod error;
pub mod store;

use cli::{Cli, Command};
use error::KlefError;
use std::path::PathBuf;
use store::{Backend, FileBackend, KeychainBackend, Store};

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
        } => commands::add::run(&store, &name, r#as, note, force),
        Command::Get { name } => commands::get::run_get(&store, &name),
        Command::Show { name } => commands::get::run_show(&store, &name),
        Command::List { format } => commands::list::run(&store, format),
        Command::Rm { name, yes } => commands::rm::run(&store, &name, yes),
        Command::Edit { name, note, r#as } => commands::edit::run(&store, &name, r#as, note),
        Command::Rename { old, new } => commands::rename::run(&store, &old, &new),
        Command::Export { names, format } => commands::export::run(&store, &names, format),
        Command::Run { env_file, cmd } => commands::run::run(&store, &env_file, &cmd),
    }
}

fn build_store() -> Result<Store, KlefError> {
    let index_path = index_path()?;
    let backend: Box<dyn Backend> = match std::env::var("KLEF_TEST_BACKEND").as_deref() {
        Ok(spec) if spec.starts_with("file:") => {
            Box::new(FileBackend::new(PathBuf::from(&spec[5..])))
        }
        _ => Box::new(KeychainBackend::new()),
    };
    Ok(Store::new(backend, index_path))
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
