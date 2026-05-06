# Using klef from an LLM agent

This document teaches an LLM agent (Claude Code, Cursor, ChatGPT) how to drive `klef` to manage user secrets without leaking values.

## Mental model in one paragraph

`klef` is a local CLI that stores secrets in the OS keychain (macOS Keychain, Linux Secret Service). It maps human-friendly names (`stripe`, `anthropic-api-key`) to opaque values. The killer trick: instead of writing the secret value into a project's `.env`, you write a reference (`STRIPE_KEY=klef:stripe`), and `klef run -- <cmd>` resolves it at runtime so the secret never touches disk in plaintext.

## Decision table

| User intent | klef command |
|---|---|
| "Save my Stripe key" | `echo -n "<value>" \| klef add stripe` (or interactive: just `klef add stripe`) |
| "Show me my Stripe key" | `klef get stripe` (prints the value, pipeable) |
| "What keys do I have?" | `klef list` |
| "Find my keys related to billing" | `klef list --filter billing` |
| "When did I add this key?" | `klef list --verbose` (adds an `ADDED` column) |
| "Delete this key" | `klef rm <name>` (prompts) or `klef rm <name> --yes` |
| "Update the value" | `klef edit <name>` (re-prompts) |
| "Update only the note" | `klef set-note <name> "<text>"` |
| "Rename this key" | `klef rename <old> <new>` |
| "Bulk-import an existing `.env`" | `klef import path/to/.env` (interactive) or `klef import path/to/.env --yes --rewrite` to also rewrite the source `.env` with klef references |
| "Run my dev server with secrets injected" | `klef run -- npm start` (assuming `.env` has `klef:` references; non-default file: `klef run --env-file .env.dev -- npm start`) |
| "Pull a key into my shell" | `eval $(klef export <name>)` |
| "Diagnostics: how is klef configured?" | `klef status` (or `klef status --format json` for parsing) |
| "Add a multi-line secret (e.g. a PEM)" | `klef add <name> --value-from-file path/to/cert.pem` |
| "Use klef in a Docker container or CI" | `KLEF_PASSPHRASE=secret klef --backend age:./vault.age <cmd>` |

## Exit codes (parse them programmatically)

- `0`: success.
- `1`: generic error.
- `2`: `KeyNotFound`. The user named a key that doesn't exist; suggest `klef list` to check.
- `3`: `BrokenReference`. A `klef:<name>` in the user's `.env` doesn't match any stored key.
- `4`: `BackendUnavailable` / `BackendDenied`. The OS keychain isn't reachable. On Linux, suggest installing `gnome-keyring` or `KWallet`. On macOS, the user may need to unlock their login keychain via Keychain Access.app.
- `64`: bad CLI usage (sysexits.h EX_USAGE). Re-read the help.

## JSON output for parsing

Two commands have stable JSON output:

```bash
klef list --format json
# {
#   "stripe": { "env_var": "STRIPE_API_KEY", "note": "...", "added_at": "...", "updated_at": "..." }
# }

klef status --format json
# {
#   "klef_version": "0.2.0",
#   "backend": "KeychainBackend",
#   "index_path": "...",
#   "keys": 3,
#   "desync": []
# }
```

The `desync` field is a list of key names that are in the index but missing from the backend. Empty list = healthy.

## Critical: never print secret values back to the user without their explicit ask

`klef get` and `klef show` print the value to stdout. If the user said "what is my Stripe key?", a literal `klef get stripe` is correct. But if the user said "wire up my Stripe key in this script", DON'T capture the output and embed it in code — use the reference pattern (`STRIPE_KEY=klef:stripe` in `.env`, then `klef run -- node app.js`).

## When `klef` is not installed

Suggest `cargo install --git https://github.com/slewinus/klef --tag v0.2.0` (Rust toolchain required) or `brew tap slewinus/tap && brew install klef` (macOS / Linux desktop, after the Homebrew tap is published).

## Common pitfalls

- The user expects `klef add stripe sk_live_xyz` to work because that's how `aws configure` and others work. It doesn't — klef reads values from stdin or an interactive prompt to keep them out of shell history. The error message hints at the right form.
- Tab-completion on key names works in zsh after `klef completions zsh > ~/.zfunc/_klef` and a `compinit` reload. Bash and fish only get static completion (subcommands and flags) for now (issue #28).
- `klef run` uses Unix `execvp` so klef itself disappears from the process tree once the child starts. Signals (SIGINT, SIGTERM) reach the child directly.
