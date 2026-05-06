# Changelog

All notable changes to klef are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
