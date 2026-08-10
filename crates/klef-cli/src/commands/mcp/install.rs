//! `klef mcp install` — register klef as an MCP server with the agent CLIs
//! installed on this machine.
//!
//! This shells out to each client's own `mcp add` rather than editing their
//! config files. Claude Code keeps its servers in `~/.claude.json`, Codex in
//! `~/.codex/config.toml` — different formats, both of which hold plenty of
//! unrelated user settings. Hand-merging them means owning a parser per client
//! and risking someone's whole config on a bug in ours, forever. Their CLIs
//! already do it correctly and stay correct when the format changes.
//!
//! Claude Desktop has no CLI, so it gets a printed snippet instead of a write.

use crate::outln;
use klef_core::error::KlefError;
use std::path::{Path, PathBuf};
use std::process::Command;

/// An agent CLI that can register an MCP server for us.
struct Client {
    /// Program looked up on `PATH`.
    bin: &'static str,
    label: &'static str,
    /// Arguments preceding the `--` separator.
    lead: &'static [&'static str],
}

const CLIENTS: &[Client] = &[
    Client {
        bin: "claude",
        label: "Claude Code",
        // `-s user` so it applies everywhere, not just the current directory.
        lead: &["mcp", "add", "klef", "-s", "user"],
    },
    Client {
        bin: "codex",
        label: "Codex",
        lead: &["mcp", "add", "klef"],
    },
];

/// Run `klef mcp install`.
///
/// # Errors
///
/// Returns an error only if klef cannot determine its own path. A client that
/// is absent, or whose `mcp add` fails, is reported and skipped — one broken
/// client must not abort the others.
pub fn run(dry_run: bool) -> Result<(), KlefError> {
    let exe = klef_path()?;
    outln!("klef binary: {}", exe.display());
    outln!();

    let mut installed = 0usize;
    let mut found = 0usize;

    for client in CLIENTS {
        if !on_path(client.bin) {
            outln!("· {} — not installed, skipped", client.label);
            continue;
        }
        found += 1;
        let cmdline = render(client, &exe);
        if dry_run {
            outln!("· {} — would run:\n    {cmdline}", client.label);
            continue;
        }
        match invoke(client, &exe) {
            Outcome::Added => {
                installed += 1;
                outln!("✓ {} — klef registered", client.label);
            }
            Outcome::AlreadyPresent => {
                installed += 1;
                outln!("✓ {} — already registered, left alone", client.label);
            }
            Outcome::Failed(msg) => {
                outln!("✗ {} — {msg}", client.label);
                outln!("    run it yourself to see the full error:\n    {cmdline}");
            }
        }
    }

    if found == 0 {
        outln!("No supported agent CLI found on PATH (looked for: claude, codex).");
    }
    outln!();
    print_desktop_snippet(&exe);

    if installed > 0 && !dry_run {
        outln!();
        outln!("Restart the client, then ask it to list your klef keys.");
        outln!("It gets names and notes only — `klef_list` never returns a value.");
    }
    Ok(())
}

/// Absolute path to the running klef binary, symlinks resolved.
///
/// Resolution matters: Homebrew puts a symlink in `bin/`, and writing that into
/// a client config would break the moment the link is repointed or removed.
fn klef_path() -> Result<PathBuf, KlefError> {
    let exe = std::env::current_exe().map_err(|e| {
        KlefError::BackendUnavailable(format!("cannot locate the klef binary: {e}"))
    })?;
    Ok(exe.canonicalize().unwrap_or(exe))
}

/// Whether `bin` resolves to something executable on `PATH`.
fn on_path(bin: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| {
        let candidate = dir.join(bin);
        candidate.is_file() && is_executable(&candidate)
    })
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(_p: &Path) -> bool {
    true
}

/// The command line klef would run, for `--dry-run` and error messages.
fn render(client: &Client, exe: &Path) -> String {
    format!(
        "{} {} -- {} mcp",
        client.bin,
        client.lead.join(" "),
        exe.display()
    )
}

/// What happened for one client.
enum Outcome {
    Added,
    /// The client refused because klef is already registered. Clients differ
    /// here — Codex overwrites silently, Claude Code errors — and both mean
    /// the user's setup is correct, so neither is reported as a failure.
    AlreadyPresent,
    Failed(String),
}

/// Run the client's `mcp add`.
fn invoke(client: &Client, exe: &Path) -> Outcome {
    let out = match Command::new(client.bin)
        .args(client.lead)
        .arg("--")
        .arg(exe)
        .arg("mcp")
        .output()
    {
        Ok(o) => o,
        Err(e) => return Outcome::Failed(format!("could not run `{}`: {e}", client.bin)),
    };

    if out.status.success() {
        return Outcome::Added;
    }
    let joined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    if is_already_registered(&joined) {
        return Outcome::AlreadyPresent;
    }
    Outcome::Failed(
        first_line(&joined).unwrap_or_else(|| format!("`mcp add` exited {}", out.status)),
    )
}

/// Whether a client's failure output means "this name is already taken".
fn is_already_registered(output: &str) -> bool {
    let lower = output.to_lowercase();
    lower.contains("already exists") || lower.contains("already configured")
}

/// First non-empty line of a client's output, trimmed.
fn first_line(s: &str) -> Option<String> {
    s.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(ToString::to_string)
}

fn print_desktop_snippet(exe: &Path) {
    outln!("Claude Desktop has no CLI — add this to claude_desktop_config.json:");
    outln!();
    outln!("  \"mcpServers\": {{");
    outln!("    \"klef\": {{");
    outln!("      \"command\": \"{}\",", exe.display());
    outln!("      \"args\": [\"mcp\"]");
    outln!("    }}");
    outln!("  }}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_puts_the_binary_after_the_separator() {
        let line = render(&CLIENTS[0], Path::new("/opt/homebrew/bin/klef"));
        assert_eq!(
            line,
            "claude mcp add klef -s user -- /opt/homebrew/bin/klef mcp"
        );
    }

    #[test]
    fn codex_line_has_no_scope_flag() {
        let line = render(&CLIENTS[1], Path::new("/usr/local/bin/klef"));
        assert_eq!(line, "codex mcp add klef -- /usr/local/bin/klef mcp");
    }

    #[test]
    fn on_path_finds_a_real_program_and_not_a_fake_one() {
        assert!(on_path("sh"), "sh must be on PATH");
        assert!(!on_path("klef-definitely-not-a-real-program"));
    }

    #[test]
    fn first_line_skips_leading_blanks() {
        assert_eq!(first_line("\n\n  boom  \nrest"), Some("boom".to_string()));
        assert_eq!(first_line("   \n  "), None);
    }

    #[test]
    fn already_registered_is_recognised_not_reported_as_failure() {
        // Real Claude Code output when the name is taken.
        assert!(is_already_registered(
            "Error: An MCP server named klef already exists in the user config"
        ));
        assert!(is_already_registered("already configured"));
        assert!(!is_already_registered("Error: command not found"));
    }
}
