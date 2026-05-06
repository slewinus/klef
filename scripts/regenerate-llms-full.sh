#!/usr/bin/env bash
# Regenerate llms-full.txt from source documents.
# Run after updating README.md, docs/llm-usage.md.

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
OUT="${ROOT}/llms-full.txt"

cat > "$OUT" <<'HEADER'
<!--
  This file is generated. Do not edit by hand.
  Run scripts/regenerate-llms-full.sh after updating README.md,
  docs/llm-usage.md, or other source documents.
-->

# klef — full documentation for LLM consumption

This file concatenates README + LLM usage guide + condensed architectural notes
for one-shot LLM context loading. For navigation, see `llms.txt`.

---

## Project overview

HEADER

cat "${ROOT}/README.md" >> "$OUT"

cat >> "$OUT" <<'SEPARATOR'

---

## LLM usage patterns

SEPARATOR

cat "${ROOT}/docs/llm-usage.md" >> "$OUT"

cat >> "$OUT" <<'TAIL'

---

## Architectural overview (condensed)

klef is a Rust 2024 crate built as bin + lib. The CLI (`src/cli.rs`, clap derive)
parses arguments and dispatches to `src/commands/<name>.rs` modules. Each command
consumes a `Store` (`src/store/mod.rs`) which combines a `Backend` trait impl
(Keychain, File, or in-memory) with an `IndexFile` for metadata.

Errors flow through `KlefError` (`src/error.rs`), with deterministic exit codes:

- 0: success
- 1: generic error
- 2: KeyNotFound
- 3: BrokenReference (klef run couldn't resolve a klef:<name>)
- 4: BackendUnavailable / BackendDenied
- 64: bad CLI usage (sysexits.h EX_USAGE)

`klef run` uses Unix `execvp` to replace itself with the child process. Signals
propagate naturally; no zombie process.

The OS keychain is the default backend on macOS (Apple Security framework via
the `keyring` crate) and Linux desktop (Secret Service via `keyring`,
gnome-keyring or KWallet at runtime). Linux headless / CI / Docker support via
an encrypted file backend is the v0.3 roadmap (issue #12, umbrella #26).

---

## Configuration

- Index file: `~/Library/Application Support/klef/index.json` (macOS),
  `${XDG_CONFIG_HOME:-~/.config}/klef/index.json` (Linux). Override with
  `KLEF_INDEX_PATH`.
- OS keychain entries: service `klef`, account `<key>`.
- `KLEF_TEST_BACKEND=file:/path` switches to plaintext file backend in DEBUG
  builds only. Release binaries ignore it.

---

## Privacy / security

- 100% local: no network, no telemetry, no cloud, no master password.
- `get`, `show`, `export`, `run` are exfiltration-by-design surfaces.
- `list`, `status`, `completions`, hidden `_names` never print values.
TAIL

echo "wrote $OUT ($(wc -c < "$OUT") bytes)" >&2
