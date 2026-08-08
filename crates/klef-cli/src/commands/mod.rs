pub mod add;
pub mod backup;
pub mod completions;
pub mod discover;
pub mod edit;
pub mod export;
pub mod get;
pub mod import;
#[cfg(target_os = "macos")]
pub mod keychain;
pub mod list;
#[cfg(feature = "mcp")]
pub mod mcp;
pub mod names;
pub mod rename;
pub mod restore;
pub mod rm;
pub mod run;
pub mod set_note;
pub mod status;
pub mod tags;

/// Strip klef's own secret-bearing env vars before spawning a child process.
///
/// `KLEF_PASSPHRASE` unlocks the whole age vault. Every process klef starts on
/// the user's behalf — `klef run -- <cmd>`, `$EDITOR` for `edit --note-edit` —
/// would otherwise inherit it, handing the vault's master key to `npm start`
/// and to every postinstall script underneath it.
///
/// The MCP path doesn't need this: `mcp::run_proc` builds the child env from
/// scratch with `env_clear` plus an explicit whitelist.
pub fn scrub_secret_env(cmd: &mut std::process::Command) {
    cmd.env_remove("KLEF_PASSPHRASE");
}
