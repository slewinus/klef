# Changelog

All notable changes to klef are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Security

- **GUI preview sessions now have a 5-min TTL**, pruned opportunistically on every `preview_dotenv_import` and `apply_dotenv_import`. Previously the plaintext values held server-side could linger in RAM until LRU eviction (up to 8 imports later) if the user dismissed the popover without explicit cancel or apply. The hard guarantees (no plaintext in JS, server-side path canonicalization, single-use sessions) are unchanged.
- **`klef export` revalidates `env_var` at render time** so a legacy index from a pre-validation klef install (or a tampered `index.json`) can't smuggle a shell-injection payload through `eval "$(klef export ...)"`. Without this, `add`-time validation alone would let an existing bad name through. Regression test: `crates/klef-cli/tests/export_revalidate.rs`.
- **All file writes use a centralized private-write helper** (`klef_core::fsx`): `write_private` (mode 0600) and `write_inheriting` (mirrors source file perms, falls back to 0600). Applied to `klef-core` index, `klef-cli` audit log + backup `.age` tmp, `klef-core` age-vault `.age.tmp`, and the `.env` rewrite tmps in both CLI import and GUI dotenv import. The `.env` rewrites use `write_inheriting` so a 0640 team-shared `.env` isn't accidentally tightened — but is also never loosened to umask defaults.

- **`env_var` names are now validated** (`^[A-Za-z_][A-Za-z0-9_]*$`) in `klef_core::store` before being stored. `klef add --as`, `klef edit --as`, the GUI import path, and backup restore all reject names containing shell metacharacters. Previously a malicious `.env` file or backup bundle could smuggle a payload like `FOO; rm -rf $HOME #` that would render dangerously through `klef export | eval`. New `KlefError::InvalidEnvVar` variant.
- **Index file and MCP audit log are now 0600 on Unix** (parent dirs 0700). Previously these files inherited the user's umask (commonly 022 → world-readable). They never contained secret values, but key names, env-var names, notes, tags, and audit metadata (argv, cwd, env_refs) are still potentially sensitive.
- **`klef edit --note-edit` tempfile** now uses `tempfile::Builder` with `O_CREAT|O_EXCL` and mode 0600 on Unix. Previously a predictable `/tmp/klef-edit-<pid>-<nanos>.txt` allowed symlink races on shared systems.
- **GUI dotenv import refactored to keep plaintext server-side.** `preview_dotenv_import` now returns only redacted previews + a session id; the actual parsed values stay in Rust state keyed by that id. `apply_dotenv_import` looks the plan up by session id and uses the canonicalized `source_path` stored at preview time — the webview can no longer round-trip values or override the write target. New `cancel_dotenv_import` command lets the modal explicitly free the session. Threat model documented in module header.
- **`docs/mcp.md`** clarifies that MCP stream redaction is a guardrail, not a DLP barrier — values <5 bytes, encoded variants, and partial leaks are explicitly out of scope. The hard guarantee is the policy denylist + argv allowlist + audit log.

### Fixed

- **GUI (klef-gui)** : `LSUIElement=true` ajouté au bundle macOS via `crates/klef-gui/Info.plist`. L'app est purement menu-bar — elle n'apparaît plus dans le Dock (et donc plus de carré bleu générique quand le cache LaunchServices est désynchronisé après reboot).
- **`npm run check`** (svelte-check) is now clean: `onMount` async-callback typing in `ui/App.svelte` rewritten to return a synchronous teardown.

## [0.4.0] - 2026-05-08

### Added

- **MCP server** (`klef mcp`) — exposes `klef_list` (metadata) and `klef_run` (process spawn with `klef:` refs injected as env vars) to MCP clients like Claude Desktop and Claude Code. Authorization is policy-driven via a TOML file (`~/Library/Application Support/klef/mcp-policy.toml` on macOS, `~/.config/klef/mcp-policy.toml` on Linux). Pre-spawn fail-closed audit log (NDJSON, `audit.log` in the same config dir). Best-effort byte-level redaction of resolved secret values in captured stdout/stderr. Hard shell denylist on `argv[0]` (`sh`, `bash`, `python`, `node`, `pwsh`, `osascript`, …). Unix process-group kill on timeout (SIGTERM → 2s grace → SIGKILL). Behind `mcp` Cargo feature (off by default; enabled by Homebrew + release binaries). Closes #24. See [`docs/mcp.md`](./docs/mcp.md).
- **`klef keychain configure`** (macOS only) — explicit user-action that disables the login keychain auto-lock (`security set-keychain-settings`) to stop frequent password prompts. Records prior settings (timeout, lock-on-sleep) so the post-run output prints the exact revert command (`security set-keychain-settings -u -t 600 -l <path>`). Idempotent: prints "already configured" when the state is already friendly. See [`docs/macos-keychain.md`](./docs/macos-keychain.md).
- **One-time banner** (macOS only) — when klef is about to read or write a Keychain value AND the default keychain has auto-lock enabled, klef prints a one-time stderr banner pointing the user at `klef keychain configure`. Marker file persists across runs (TTL 7 days; re-shown if keychain state drifts). Opt-out via `KLEF_NO_KEYCHAIN_AUTOCONFIG=1`. Banner does NOT print from `klef mcp` (Claude Desktop captures stderr to log files).
- `klef-core::macos_keychain` public module: shared helpers (`current_status`, `apply_friendly_settings`, `is_already_friendly`, `build_revert_command`) for CLI and the future GUI button. Empty on non-macOS.

### Changed

- `klef-core` bumped from 0.1.0 to 0.2.0 (adds the public `macos_keychain` module). All previously-stable API unchanged.
- `klef-cli` `unsafe_code` workspace lint downgraded from `forbid` to `deny` (CLI crate only) to allow scoped `#[allow(unsafe_code)]` blocks for `setsid` / `killpg` (`klef mcp` process-group kill on Unix) and Rust 2024's `unsafe { std::env::set_var }` in tests. `klef-core` retains `forbid(unsafe_code)`.

### Fixed

- N/A.

## [0.3.0] - 2026-05-06

### Added

- `klef discover [--root PATH] [--depth N] [--include PATTERN]...` walks the filesystem, finds every `.env`, builds a deduplicated import plan, and bulk-adds with confirmation. Skips `node_modules`, `.git`, `target`, `.venv`, etc. Conflict modes: `--on-conflict first-found|last-found` (#21).
- `klef discover --skip-pattern <REGEX>` (repeatable) and `--skip-defaults` (built-in list: PORT, NODE_ENV, *_HOST, *_TIMEOUT, etc.) to exclude non-secret config from the import plan (#37).
- `klef backup <out.age> [--recipient KEY]...` and `klef restore <in.age> [--force]` — encrypted full-vault dump and reconstruction via [age](https://github.com/FiloSottile/age). Bundle includes values, env-var names, notes, tags, and timestamps. Restore is 3-phase atomic (preflight → backend writes → index commit); klef's view of the vault is atomic across restore. Plaintext is held in zeroize buffers from serialization through encryption. Strict schema (`#[serde(deny_unknown_fields)]`) on read (#22).
- Tags for organizing keys: `klef add --tag T1 --tag T2`, `klef edit --tag T --clear-tags`, `klef list --tag T` (composes with `--filter`), `klef list --verbose` adds a `TAGS` column, `klef show` displays the tags line, new `klef tags` command lists all tags with key counts. Tags sorted and deduped on write (#36).
- `klef edit <name> --note-edit` opens `$VISUAL` (or `$EDITOR`) with the current note pre-filled, saves the trimmed file content as the new note. Falls back to a single-line stdin prompt. Empty result clears the note (#14).
- Bash and fish dynamic completion of stored key names (extends the zsh implementation from v0.2.0). `klef show str<Tab>` now suggests `stripe` in all three shells (#28 follow-up).
- AI-readable documentation: `llms.txt` (navigation index), `llms-full.txt` (one-shot LLM context, auto-generated via `scripts/regenerate-llms-full.sh`), `docs/llm-usage.md` (decision tables, exit codes, JSON output reference) — follows the [llmstxt.org](https://llmstxt.org/) convention (#33).
- README hero blockquote with the elevator pitch, comparison table vs 1Password CLI / doppler / infisical / direnv, asciinema demo cast embedded ([asciicast](https://asciinema.org/a/5z9zCmNWd1igb3MH)), and `cargo install klef` install snippet (klef now on [crates.io](https://crates.io/crates/klef)) (#38, #39, #41).

### Fixed

- `Store::remove` previously swallowed all backend errors with `let _ = self.backend.remove(name);`, so a Keychain permission denial silently appeared as a successful delete. Now `KeyNotFound` is tolerated (the legitimate "secret already gone manually" case) but any other backend error propagates and the index is NOT modified.

## [0.2.0] - 2026-05-06

### Added

- `klef import <file.env>` bulk-import command — derives klef names from env keys, shows a redacted plan, prompts before writing, optional `--rewrite` replaces literals with `klef:` references in the source file (#7).
- `klef status [--format text|json]` diagnostic command — prints version, backend, index path, key count, and detects desync (keys in index but missing from backend). Exit 1 when desync detected (#8).
- `klef set-note <name> <text>` — shortcut for `klef edit <name> --note <text>` (partial #25).
- `klef add` and `klef edit` accept `--value-from-file <FILE>` for multi-line secrets (PEM keys, JSON service-account files, JWTs, certs). Trailing whitespace stripped to match stdin/prompt behavior (partial #25).
- `klef list --verbose` adds an `ADDED` column showing when each key was first saved (`YYYY-MM-DD`) (#16).
- `klef list --filter <PATTERN>` filters by case-insensitive substring on name or note. Composes with `--verbose` (#17).
- `klef remove` alias for `klef rm`. The acronym stays for muscle memory; `remove` exists for explicit-form preference.
- Zsh dynamic completion for key names (`klef show str<Tab>` → `stripe`). Adds a hidden `klef _names` helper that the completion script invokes at runtime. Bash and fish keep static completion for now (#28; bash/fish dynamic are follow-up work).
- GitHub Actions release workflow (`.github/workflows/release.yml`) builds binaries on tag push for x86_64-apple-darwin, aarch64-apple-darwin, x86_64-unknown-linux-gnu, and aarch64-unknown-linux-gnu — all on native runners. Each tarball ships with binary + LICENSE + README + CHANGELOG. `workflow_dispatch` trigger lets us dry-run the pipeline (#11).
- Homebrew formula scaffolding: `homebrew/klef.rb` template + `scripts/update-homebrew-formula.sh` that downloads release tarballs, computes SHA-256s, and produces a populated formula ready to commit to a separate `slewinus/homebrew-tap` repo. One-time setup is documented in `docs/release.md` (#10).

### Changed

- When `KeychainBackend` fails (`PlatformFailure` or `NoStorageAccess`), the error message now includes a platform-specific hint — Linux points at gnome-keyring/KWallet setup, the headless umbrella (#26), and the v0.3 file backend plan (#12); macOS suggests Keychain Access.app to check the lock state (#9).

### Fixed

- `klef add stripe somevalue` previously emitted clap's cryptic `unexpected argument 'somevalue'`. Now intercepted with a hint pointing at the stdin/prompt path; exit code 64 (sysexits.h `EX_USAGE`). Same for `klef edit` (#15).

## [0.1.0] - 2026-05-05

First public release. Local-first CLI vault for API keys.

### Added

- 9 commands: `add`, `get`, `show`, `list`, `rm`, `edit`, `rename`, `export`, `run`.
- macOS Keychain backend (via `keyring` crate, `apple-native` feature) with Touch ID / system password prompts.
- Linux Secret Service backend (via `keyring` crate, `sync-secret-service` feature, `crypto-rust` for pure-Rust crypto). Tested on GNOME desktop sessions; Linux headless support deferred to v0.3.
- `klef run --env-file .env -- <cmd>`: parses `.env`, resolves `klef:<name>` references against the keychain, and `execvp`s the wrapped command on Unix so signals propagate naturally.
- `KeyMeta` index in `~/Library/Application Support/klef/index.json` (macOS) / `${XDG_CONFIG_HOME:-~/.config}/klef/index.json` (Linux). Stores env-var name, optional note, and timestamps. Atomic writes via `.json.tmp` + rename.
- `Backend` trait abstraction: `KeychainBackend` (production), `MemoryBackend` (in-process tests), `FileBackend` (cross-process E2E tests, foundation for future encrypted backend).
- Quickstart example in `examples/quickstart/` showing the `klef run` flow end-to-end.

### Security

- `KLEF_TEST_BACKEND` env-var override is gated behind `cfg(debug_assertions)` so release binaries never honor it. Closes an env-var redirection attack vector.
- `keyring` crate compiled with explicit `apple-native` + `sync-secret-service` + `crypto-rust` features (the default crates.io build ships zero backends and silently no-ops).
- No telemetry. No cloud. No external network calls.

[Unreleased]: https://github.com/slewinus/klef/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/slewinus/klef/releases/tag/v0.3.0
[0.2.0]: https://github.com/slewinus/klef/releases/tag/v0.2.0
[0.1.0]: https://github.com/slewinus/klef/releases/tag/v0.1.0
