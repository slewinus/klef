//! Library entrypoint for the `klef` binary. The actual `main()` lives in
//! `main.rs` and just parses clap then calls [`run`]. Everything in this crate
//! is TTY/CLI-coupled — anything reusable by the GUI should live in
//! `klef-core` instead.

pub mod cli;
pub mod commands;

#[cfg(target_os = "macos")]
mod macos_keychain_banner;

use cli::{Cli, Command};
use klef_core::error::KlefError;

/// Dispatch the parsed CLI to the appropriate command handler.
///
/// # Errors
///
/// Returns an error if the backend or command dispatch fails.
pub fn run(cli: Cli) -> Result<(), KlefError> {
    let store = klef_core::build_store(cli.backend.as_deref())?;
    #[cfg(target_os = "macos")]
    if macos_keychain_banner::backend_is_keychain(&store)
        && macos_keychain_banner::command_touches_values(&cli.command)
    {
        macos_keychain_banner::maybe_emit_banner(&mut std::io::stderr());
    }
    match cli.command {
        Command::Add {
            name,
            r#as,
            note,
            force,
            value_from_file,
            tag,
        } => commands::add::run(
            &store,
            &name,
            r#as,
            note,
            force,
            value_from_file.as_deref(),
            tag,
        ),
        Command::Get { name } => commands::get::run_get(&store, &name),
        Command::Show { name } => commands::get::run_show(&store, &name),
        Command::List {
            format,
            verbose,
            filter,
            tag,
        } => commands::list::run(&store, format, verbose, filter.as_deref(), tag.as_deref()),
        Command::Rm { name, yes } => commands::rm::run(&store, &name, yes),
        Command::Edit {
            name,
            note,
            r#as,
            value_from_file,
            tag,
            clear_tags,
            note_edit,
        } => commands::edit::run(
            &store,
            &name,
            r#as,
            note,
            value_from_file.as_deref(),
            tag,
            clear_tags,
            note_edit,
        ),
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
        Command::Tags => commands::tags::run(&store),
        Command::Names => commands::names::run(&store),
        #[cfg(feature = "mcp")]
        Command::Mcp { policy } => commands::mcp::run(store, policy),
        #[cfg(target_os = "macos")]
        Command::Keychain { action } => match action {
            cli::KeychainAction::Configure => commands::keychain::run(),
        },
    }
}
