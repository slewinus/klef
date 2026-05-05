use clap::{Parser, Subcommand};
use clap_complete::Shell;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "klef", version, about = "Local-first vault for API keys.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Add a new key. Reads value from a TTY prompt or stdin.
    Add {
        name: String,
        #[arg(long, value_name = "VAR")]
        r#as: Option<String>,
        #[arg(long)]
        note: Option<String>,
        #[arg(long)]
        force: bool,
    },
    /// Print the value of a key on stdout.
    Get { name: String },
    /// Display a key's value formatted for human reading.
    Show { name: String },
    /// List stored keys (names + metadata, never values).
    List {
        #[arg(long, value_enum, default_value_t = ListFormat::Table)]
        format: ListFormat,
    },
    /// Remove a key.
    #[command(alias = "remove")]
    Rm {
        name: String,
        #[arg(long)]
        yes: bool,
    },
    /// Edit a key (prompts for new value if no flag given).
    Edit {
        name: String,
        #[arg(long)]
        note: Option<String>,
        #[arg(long, value_name = "VAR")]
        r#as: Option<String>,
    },
    /// Rename a key.
    Rename { old: String, new: String },
    /// Print `export VAR=value` lines for eval.
    Export {
        names: Vec<String>,
        #[arg(long, value_enum, default_value_t = ExportFormat::Shell)]
        format: ExportFormat,
    },
    /// Run a command with `klef:<name>` references in `.env` resolved.
    Run {
        #[arg(long, value_name = "FILE", default_value = ".env")]
        env_file: PathBuf,
        #[arg(last = true)]
        cmd: Vec<String>,
    },
    /// Generate shell completion script for <shell> on stdout.
    Completions { shell: Shell },
    /// Print runtime diagnostic state (backend, index, key count, desync).
    Status {
        #[arg(long, value_enum, default_value_t = StatusFormat::Text)]
        format: StatusFormat,
    },
    /// Bulk import secrets from a .env file.
    Import {
        file: PathBuf,
        #[arg(long, value_name = "PREFIX")]
        prefix: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        rewrite: bool,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub enum ListFormat {
    Table,
    Json,
}

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub enum ExportFormat {
    Shell,
    Dotenv,
}

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub enum StatusFormat {
    Text,
    Json,
}
