# klef

> **klef stores your API keys in the OS Keychain and resolves them at runtime in your `.env` (`STRIPE_KEY=klef:stripe`). No master password, no cloud, no plaintext on disk.**

[![Crates.io](https://img.shields.io/crates/v/klef.svg)](https://crates.io/crates/klef)
[![CI](https://github.com/slewinus/klef/actions/workflows/ci.yml/badge.svg)](https://github.com/slewinus/klef/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust 1.85+](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)
[![Platforms](https://img.shields.io/badge/platforms-macOS%20%7C%20Linux-lightgrey)](#status)

A local vault for your API keys and secrets — so you stop losing them in Dashlane, Notes, or scattered `.env` files.

## The problem

You have 15 API keys (Stripe, Anthropic, OpenAI, Gemini, Telnyx, etc.). You stash them in Dashlane, in text files, in lost `.env` files. When you start a project, you spend 10 minutes hunting them down — and worst of all, you copy-paste the value into the project's `.env`, so it lingers in plaintext on your disk.

## The solution

A local CLI that:
- **Stores** your keys in the OS Keychain — encryption handled by Apple/GNOME, not by us.
- **Retrieves** a key with a single command: `klef get stripe`.
- **Injects** values into your projects through **references** in the `.env` instead of plaintext values:
  ```
  STRIPE_KEY=klef:stripe          # reference — resolved at runtime
  ANTHROPIC_KEY=klef:anthropic    # same
  ```
  Then `klef run -- npm start` resolves it all and runs your command with the right env vars.
- **Stays 100% local** — no server, no cloud, no telemetry.

## Why not another tool?

| | klef | 1Password CLI | doppler / infisical | direnv + .env |
|---|---|---|---|---|
| Local-first | ✅ | ❌ (1P account) | ❌ (cloud) | ✅ |
| Native Keychain storage | ✅ | via `op` | ❌ | ❌ |
| References in `.env` | ✅ `klef:` | ✅ `op://` | ✅ `{{var}}` | ❌ literal |
| No master password | ✅ (Touch ID) | ❌ | ❌ | ✅ |
| Free | ✅ | $3/month | freemium | ✅ |
| Multi-machine sync | ❌ (v0.4) | ✅ | ✅ | ❌ |

klef targets the single-user, single-machine, local-first, free use case. The competitors are excellent — it's just a different niche. (Comparison verified on 2026-05-06.)

## Demo

```bash
# You have a .env lying around with plaintext secrets:
$ cat .env
STRIPE_API_KEY=sk_live_xyz
ANTHROPIC_API_KEY=sk-ant-zzz
PORT=3000

# One command to import everything into the Keychain and rewrite the .env as references:
$ klef import .env --rewrite
ENV VAR             KLEF NAME             VALUE
STRIPE_API_KEY      stripe-api-key        sk_l*** (16 chars)
ANTHROPIC_API_KEY   anthropic-api-key     sk-a*** (12 chars)
PORT                port                  *** (4 chars)
Import 3 key(s)? [y/N] y
✓ STRIPE_API_KEY → klef:stripe-api-key
✓ ANTHROPIC_API_KEY → klef:anthropic-api-key
✓ PORT → klef:port
Imported 3 key(s).
Rewrote .env (3 reference(s) replaced).

$ cat .env
STRIPE_API_KEY=klef:stripe-api-key
ANTHROPIC_API_KEY=klef:anthropic-api-key
PORT=klef:port

# Now run your app like before — klef resolves the references on the fly:
$ klef run -- node app.js
Server on port 3000, Stripe wired ✓
```

[![asciicast](https://asciinema.org/a/5z9zCmNWd1igb3MH.svg)](https://asciinema.org/a/5z9zCmNWd1igb3MH)

_Cast source: [`docs/klef-demo.cast`](./docs/klef-demo.cast) — re-uploadable if asciinema.org goes down._

## Install

### Cargo (recommended)

```bash
cargo install klef
```

### Homebrew (macOS / Linux desktop)

```bash
brew tap slewinus/tap
brew install klef
```

### Pre-built binaries

Available on the [Releases page](https://github.com/slewinus/klef/releases) — macOS Intel + Apple Silicon, Linux x86_64 + ARM. Unpack and move into your `$PATH`.

### Shell auto-completion

```bash
# zsh
klef completions zsh > ~/.zfunc/_klef

# bash
klef completions bash > /usr/local/etc/bash_completion.d/klef

# fish
klef completions fish > ~/.config/fish/completions/klef.fish
```

> Static completion of subcommands and flags works today. Dynamic completion of key names (e.g. `klef get <TAB>`) is tracked in [#28](https://github.com/slewinus/klef/issues/28) and not yet implemented.

## Commands

| Command | Role |
|---|---|
| `klef add <name>` | Add a key (TTY prompt or stdin). Use `--value-from-file <FILE>` for multi-line secrets (PEM, JSON). |
| `klef get <name>` | Print the value (pipeable). |
| `klef show <name>` | Value + metadata. |
| `klef list [--format table\|json] [-v\|--verbose] [--filter PATTERN]` | List keys (never values). `--verbose` adds the date added, `--filter` does substring search. |
| `klef rm <name>` (alias `remove`) | Remove a key. |
| `klef edit <name>` | Change the value or metadata. `--value-from-file` for multi-line secrets. `--note-edit` opens `$VISUAL`/`$EDITOR` to edit the note. |
| `klef set-note <name> <text>` | Shortcut for `edit --note`. |
| `klef rename <old> <new>` | Rename a key. |
| `klef export <name>... [--format shell\|dotenv]` | Emit `export` lines. |
| `klef import <file.env> [--prefix P] [--dry-run] [--rewrite] [--yes]` | Bulk-import from an existing `.env`. `--rewrite` replaces literal values with `klef:` references in the source file. |
| `klef run [--env-file FILE] -- <cmd>` | Resolve `klef:<name>` in `.env` and exec `<cmd>`. |
| `klef status [--format text\|json]` | Diagnostics: version, backend, index path, key count, desync. Exit 1 on desync. |
| `klef completions <shell>` | Generate the auto-completion script. |

Run `klef --help` or `klef <cmd> --help` for the details of each option.

## Stack

- **Language**: Rust (2024 edition)
- **Storage**: native Keychain via [`keyring`](https://crates.io/crates/keyring) — Apple Security framework on macOS, Secret Service on Linux.
- **CLI**: [`clap`](https://crates.io/crates/clap) (derive)
- **No server, no cloud, no account, no telemetry.**

## Dev

```bash
# Setup hooks (run once after clone)
./scripts/setup-dev.sh

# Build / test (cargo workspace: klef-core + klef-cli + klef-gui)
cargo build --workspace
cargo test --workspace --all-features
cargo run -p klef -- --help
```

### GUI (klef-gui)

The Tauri crate has a Svelte frontend that must be bundled before any `cargo build/run -p klef-gui` (because `tauri::generate_context!` validates `frontendDist` at compile time):

```bash
cd crates/klef-gui
npm ci                # once
npm run build         # on every frontend change (or use `npm run dev` alongside cargo run)
cd ../..
cargo run -p klef-gui # menu bar mode: icon top-right, click to open (no Dock icon — LSUIElement=true)
```

The git hooks (`fmt`, `clippy`, `tests`, line-cap < 300 lines/file) are versioned under `.githooks/`. CI on macOS + Ubuntu via GitHub Actions (`.github/workflows/ci.yml`).

## Documentation

- **Quickstart**: [examples/quickstart/](./examples/quickstart/) — `.env` + consumer script, end-to-end smoke test.
- **macOS users**: if you see frequent password prompts, run `klef keychain configure` once — see [`docs/macos-keychain.md`](./docs/macos-keychain.md).
- **Changelog**: [CHANGELOG.md](./CHANGELOG.md)

## For AI agents

klef ships documentation designed for AI assistants:

- **[`llms.txt`](./llms.txt)**: navigation index (following the [llmstxt.org](https://llmstxt.org/) convention)
- **[`llms-full.txt`](./llms-full.txt)**: concatenated doc for single-prompt ingestion
- **[`docs/llm-usage.md`](./docs/llm-usage.md)**: concrete patterns — decision table, exit codes, JSON outputs
- **[`docs/mcp.md`](./docs/mcp.md)**: MCP server (`klef mcp`) — let Claude/Cursor use your keys without ever seeing the plaintext value.

Claude Code, Cursor and ChatGPT agents can ingest these files and know how to drive klef without hallucinating.

## Status

✅ **v0.4.1** tagged (2026-05-11) — security patch on v0.4.0. Five audit findings fixed (shell-safe `env_var` validation, 0600 perms on metadata, O_EXCL tempfile for `--note-edit`, GUI dotenv import reworked to keep plaintext on the Rust side, MCP redaction documented as best-effort).

What's new in v0.4 (cumulative):
- **MCP server** (`klef mcp`) — `klef_list` (metadata) + `klef_run` (spawn process with `klef:` refs injected as env vars) for Claude Desktop / Claude Code. Per-rule TOML policy, fail-closed NDJSON audit log, best-effort byte-exact redaction, shell denylist. Closes [#24](https://github.com/slewinus/klef/issues/24). Doc: [`docs/mcp.md`](./docs/mcp.md).
- **macOS GUI** (`klef.app`) — menu-bar app (no Dock icon, `LSUIElement=true`) with a ⌘⇧K popover. Search, auto-clear copy, drag-and-drop a `.env` for bulk import + rewrite as `klef:` refs. The CLI binary is bundled inside the `.app` for a unified install. Closes [#18](https://github.com/slewinus/klef/issues/18).
- **`klef keychain configure`** (macOS) — disables Keychain auto-lock to stop the repeated prompts. Idempotent, prints the revert command.
- **One-time banner** (macOS) pointing to `keychain configure` when the Keychain is locked.

Security (v0.4.1):
- `env_var` validation (`^[A-Za-z_][A-Za-z0-9_]*$`) on write **and** on render — closes the `klef export | eval` path even for legacy indexes.
- `klef_core::fsx::{write_private, write_inheriting}` helper: 0600 on `index.json`, `audit.log`, `.age.tmp`, `backup.tmp`; source-file perms preserved on `.env` rewrites.
- GUI dotenv import: plaintext never returned to the webview, session-id kept on the Rust side with a 5-min TTL, single-use on apply.

- **Supported platforms**: macOS (native Keychain + menu-bar GUI) + Linux desktop (Secret Service) + headless Linux / CI / Docker via `--backend age:./vault.age` (closes [#12](https://github.com/slewinus/klef/issues/12)).
- **Out of scope**: Windows, multi-machine sync.
- **Roadmap**: see [issues by milestone](https://github.com/slewinus/klef/milestones). v0.5+ tracked in [#125](https://github.com/slewinus/klef/issues/125) (Rust-side clipboard, MCP `output_policy`, audit log retention).

## License

[MIT](./LICENSE) — © 2026 Oscar R.
