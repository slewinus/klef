use crate::cli::Cli;
use crate::error::KlefError;
use clap::CommandFactory;
use clap_complete::Shell;

/// Print the shell completion script for `shell` to stdout.
///
/// # Errors
///
/// Currently never returns an error; the signature stays consistent with the other commands so
/// future failures (e.g. writing the script through a file handle) can surface uniformly.
pub fn run(shell: Shell) -> Result<(), KlefError> {
    let mut cmd = Cli::command();
    let bin_name = cmd.get_name().to_string();
    clap_complete::generate(shell, &mut cmd, bin_name, &mut std::io::stdout());
    Ok(())
}
