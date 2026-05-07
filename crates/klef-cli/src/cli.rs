use clap::{Parser, Subcommand};
use clap_complete::Shell;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "klef", version, about = "Local-first vault for API keys.")]
pub struct Cli {
    /// Backend selection. Default: OS keychain.
    /// Format: `age:/path/to/file.age` for an age-encrypted file backend
    /// (Linux headless, CI, Docker). Passphrase from `KLEF_PASSPHRASE` env var or TTY.
    #[arg(long, global = true, value_name = "SPEC")]
    pub backend: Option<String>,

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
        /// Read the secret value from FILE instead of stdin/prompt.
        /// Trailing whitespace is stripped (matches stdin/prompt behavior).
        #[arg(long, value_name = "FILE")]
        value_from_file: Option<PathBuf>,
        /// Tag the key. Repeatable. Tags are case-sensitive labels for organization.
        #[arg(long, value_name = "TAG")]
        tag: Vec<String>,
    },
    /// Print the value of a key on stdout.
    Get { name: String },
    /// Display a key's value formatted for human reading.
    Show { name: String },
    /// List stored keys (names + metadata, never values).
    List {
        #[arg(long, value_enum, default_value_t = ListFormat::Table)]
        format: ListFormat,
        /// Add ADDED column showing when each key was first added.
        #[arg(long, short = 'v')]
        verbose: bool,
        /// Filter entries by case-insensitive substring match on name or note.
        #[arg(long, value_name = "PATTERN")]
        filter: Option<String>,
        /// Filter to keys having this exact tag (case-sensitive).
        #[arg(long, value_name = "TAG")]
        tag: Option<String>,
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
        /// Read the new secret value from FILE instead of stdin/prompt.
        /// Trailing whitespace is stripped (matches stdin/prompt behavior).
        #[arg(long, value_name = "FILE")]
        value_from_file: Option<PathBuf>,
        /// Replace the key's tags with this set. Repeatable.
        #[arg(long, value_name = "TAG")]
        tag: Vec<String>,
        /// Remove all tags from the key.
        #[arg(long)]
        clear_tags: bool,
        /// Open `$VISUAL` (or `$EDITOR`) to edit the note. Falls back to a
        /// single-line stdin prompt if neither is set.
        #[arg(long, conflicts_with_all = ["note", "as", "value_from_file"])]
        note_edit: bool,
    },
    /// Rename a key.
    Rename { old: String, new: String },
    /// Shortcut for `klef edit <name> --note <text>`.
    SetNote { name: String, note: String },
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
    /// Walk the filesystem and bulk-import secrets from every .env file found.
    Discover {
        /// Root directory for the scan. Defaults to the current directory.
        #[arg(long, value_name = "PATH")]
        root: Option<PathBuf>,
        /// Max directory depth from root.
        #[arg(long, default_value_t = 4)]
        depth: usize,
        /// Filename patterns to consider. Repeatable. Defaults to common .env variants.
        #[arg(long, value_name = "PATTERN")]
        include: Vec<String>,
        /// Print the plan, don't write anything.
        #[arg(long)]
        dry_run: bool,
        /// Skip the interactive confirmation.
        #[arg(long)]
        yes: bool,
        /// Conflict resolution when the same env-var name appears in multiple files.
        #[arg(long, value_enum, default_value_t = ConflictMode::FirstFound)]
        on_conflict: ConflictMode,
        /// Regex patterns matched against env-var names (KEY side). Excludes matches from the import. Repeatable.
        #[arg(long, value_name = "PATTERN")]
        skip_pattern: Vec<String>,
        /// Apply a built-in skip list of common non-secret config names (`PORT`, `DB_NAME`, `NODE_ENV`, etc.).
        #[arg(long)]
        skip_defaults: bool,
    },
    /// Encrypted backup of the entire vault (values + metadata) to a single .age file.
    Backup {
        /// Output path. Convention: `.age` extension.
        output: PathBuf,
        /// Recipient public key for asymmetric encryption (e.g. age1...). Repeatable.
        /// If absent, the backup is encrypted with a passphrase prompted on stdin.
        #[arg(long, value_name = "KEY")]
        recipient: Vec<String>,
    },
    /// Restore the vault from a klef backup file.
    Restore {
        /// Input path.
        input: PathBuf,
        /// Overwrite existing keys instead of failing on conflict.
        /// WARNING: pre-existing values are NOT backed up automatically.
        #[arg(long)]
        force: bool,
    },
    /// List all tags in the vault with the count of keys carrying each.
    Tags,
    /// Internal: print one stored key name per line. Used by shell completion scripts.
    /// Hidden from --help.
    #[command(name = "_names", hide = true)]
    Names,
    /// Run an MCP server exposing `klef_list` and `klef_run` over stdio.
    /// See docs/mcp.md for setup with Claude Desktop / Claude Code.
    #[cfg(feature = "mcp")]
    Mcp {
        /// Path to the policy file. Default: ~/.config/klef/mcp-policy.toml.
        #[arg(long, value_name = "PATH")]
        policy: Option<PathBuf>,
    },
}

#[derive(Copy, Clone, Debug, clap::ValueEnum)]
pub enum ConflictMode {
    FirstFound,
    LastFound,
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
