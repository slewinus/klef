# Changelog

All notable changes to klef are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
