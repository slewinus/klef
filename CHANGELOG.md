# Changelog

All notable changes to klef are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `klef import <file.env>` bulk-import command — derives klef names from env keys, shows a redacted plan, prompts before writing, optional `--rewrite` replaces literals with `klef:` references in the source file (#7).
- `klef status [--format text|json]` diagnostic command — prints version, backend, index path, key count, and detects desync (keys in index but missing from backend). Exit 1 when desync detected (#8).
- `klef set-note <name> <text>` — shortcut for `klef edit <name> --note <text>` (partial #25).
- `klef add` and `klef edit` accept `--value-from-file <FILE>` for multi-line secrets (PEM keys, JSON service-account files, JWTs, certs). Trailing whitespace stripped to match stdin/prompt behavior (partial #25).
- `klef list --verbose` adds an `ADDED` column showing when each key was first saved (`YYYY-MM-DD`) (#16).
- `klef list --filter <PATTERN>` filters by case-insensitive substring on name or note. Composes with `--verbose` (#17).
- `klef remove` alias for `klef rm`. The acronym stays for muscle memory; `remove` exists for explicit-form preference.

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

[Unreleased]: https://github.com/slewinus/klef/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/slewinus/klef/releases/tag/v0.1.0
