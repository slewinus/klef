# klef — Notes for Claude

A local-first CLI vault for API keys, backed by the OS keychain. See [README.md](./README.md) for the user-facing pitch, [docs/design/2026-05-05-mvp-design.md](./docs/design/2026-05-05-mvp-design.md) for the full design spec, and [docs/plans/2026-05-05-mvp-implementation.md](./docs/plans/2026-05-05-mvp-implementation.md) for the step-by-step implementation plan.

## Hard rules

- **300-line cap on non-doc files.** Enforced two ways: (1) a `PostToolUse` hook warns me as soon as a Write/Edit pushes a file over the limit; (2) `scripts/check-lines.sh` runs at pre-commit and rejects the commit. Skipped extensions: `.md`, `.toml`, `.txt`, `.lock`, `.json`, `.yaml`, `.yml`. If a file is approaching the cap, split it by responsibility — don't paper over it.
- **TDD for every feature.** The plan is structured as failing-test → minimal impl → passing test → commit. Don't skip the failing-test step; if a behavior isn't worth a test, it's probably not worth shipping.
- **Never touch the real Keychain in automated tests.**
  - In-process Rust tests: `MemoryBackend` (`HashMap` + `Mutex`) — instantiated directly in test code.
  - Cross-process E2E (`assert_cmd`, smoke scripts): `FileBackend` via `KLEF_TEST_BACKEND=file:/path/to/secrets.json`. `MemoryBackend` does NOT survive across process boundaries — every `cargo run -- ...` is a fresh process with empty memory, so it can't be used for E2E.
  - The real Keychain is verified only by manual smoke tests at release time (Task 17 of the plan).
- **English in code-facing strings** (error messages, CLI help, log lines). French is fine in markdown docs and code comments — but commit messages stay English / conventional.

## Architectural conventions

- **bin + lib crate layout.** All logic lives in `src/lib.rs` and the modules it declares; `src/main.rs` is a thin entry that calls `klef::run(Cli::parse())` and prints errors. Don't move logic back into `main.rs` — the lib boundary is what makes tests reachable.
- **`Backend` trait is the only abstraction over secret storage.** Commands consume `Store` (which owns a `Box<dyn Backend>` + `IndexFile`), never `keyring` directly. Adding a new backend (file-based, etc.) must not require touching the commands.
- **`klef run` uses `std::os::unix::process::CommandExt::exec()` on Unix** — a true `execvp` replacement. macOS + Linux are the MVP targets, so this is fine. There's a `#[cfg(not(unix))]` `Command::status()` fallback purely so the crate compiles on Windows; don't rely on it.
- **Index file holds metadata only — never the secret values.** Values live exclusively in the backend (Keychain or memory). If you find yourself wanting to write a secret to disk in plain JSON, stop and re-read the design.
- **Atomic writes** for `IndexFile`: write `index.json.tmp` then `rename` to the final path. Never write the destination file directly.
- **Error type is `KlefError`** (`thiserror`-derived enum in `src/error.rs`). Don't introduce `anyhow` or `Box<dyn Error>` in this crate; route everything through `KlefError` so exit codes stay deterministic (see §7 of the design).
- **Don't reach for a dotenv crate.** The `.env` parser lives in `src/envfile.rs` and is intentionally homegrown — we need exact control over what counts as a `klef:` reference.

## Security boundaries

- **`KLEF_TEST_BACKEND` is debug-only.** The env-var-driven `FileBackend` selection in `lib::backend_from_env` is gated behind `cfg(debug_assertions)`. Release binaries (`cargo install`, `cargo build --release`) ignore the variable completely and always use the keychain. Don't move that gate without thinking — it's the protection against env-var attacks redirecting secret reads/writes to an attacker-controlled file.
- **Never commit real secrets to test fixtures, snapshots, or example .env files.** Test stdin in `tests/cli.rs` uses obvious dummies (`"sk_live"`, `"v"`, `"sk_test_demo_value"`). Keep it that way. CI logs and snapshot fixtures must be greppable for these patterns to confirm no real secret leaked in.
- **`get`, `show`, `export`, `run` are exfiltration surfaces by design** — they exist to print or inject secret values. Any change to error/log output in these commands must verify the value isn't accidentally written to stderr, log files, or panic messages.
- **Don't widen the `KlefError` `Debug` output to include secret values.** The `Debug` impls are auto-derived; if a future variant carries a secret, opt out of the derive for that variant.

## Tooling

| Hook | When | What |
|---|---|---|
| `.githooks/pre-commit` | Before every commit | `check-lines.sh` → `cargo fmt --check` → `cargo clippy -D warnings` |
| `.githooks/pre-push` | Before every push | `cargo test --all-features` |
| `.claude/settings.json` PostToolUse | After Write/Edit | `rustfmt` on `.rs` files + `claude-line-guard.sh` warning |
| `.claude/settings.json` Stop | End of turn | `cargo check`, system message if it fails |

If `cargo` complains about `clippy::pedantic` warnings, fix them — don't `#[allow]` your way out unless there's a real reason (then add a one-line comment explaining why).

## Workflow expectations

- **Small commits, conventional messages.** `feat:`, `test:`, `refactor:`, `chore:`, `docs:`. The plan's tasks already split commits cleanly — follow it.
- **One logical thing per commit.** If a task says "add `klef rename`", everything in that commit is for `klef rename`. Don't bundle drive-by clippy fixes from elsewhere.
- **No commits without permission.** The user runs the commits. I prepare the diff and tell them when something is ready.
- **No pushes, no PRs, no `git tag`** without an explicit ask.

## When in doubt

- Re-read the design spec before deviating from it.
- If a decision isn't covered, surface it explicitly to the user — don't make silent calls on architecture.
- The §12 of the design lists the only questions still open at brainstorming time; everything else is locked.
