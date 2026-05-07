# MCP server — design

**Status:** Draft
**Date:** 2026-05-07
**Tracking issue:** [#24](https://github.com/slewinus/klef/issues/24)

## Goal

Expose klef to MCP clients (Claude Code, Claude Desktop, Cursor, Aider) so an AI agent can use API keys without ever receiving plaintext values into its context.

## Non-goals

- Remote MCP (over network). klef stays local-first.
- Caching values. Always re-resolve from `Store`.
- Per-call interactive approval prompts. Authorization is policy-driven (a static file the user owns), not runtime-interactive. Interactive approval can be added later as an extension; the MCP server itself stays headless.
- Exposing mutation tools (`add`, `rm`, `edit`). Read-only by design.

## Threat model

This is **not** a zero-knowledge system. A malicious agent can craft an `argv` that exfiltrates a value (e.g., `curl` with the key in a query string). What changes vs. the alternative `klef_get`-style design:

- **Without this design:** every normal use puts plaintext secrets into the agent's context → passive leak into transcripts, client-side logs, and any cloud-hosted LLM provider's request logs. Failure mode is silent and continuous.
- **With this design:** secrets only enter agent-visible output if the agent issues an explicitly extractive `argv` → leaves an audit-log trail, rejectable by user-side policy (allowlist of `(argv_pattern, env_refs)` pairs).

Risk shifts from "passive systematic leak" to "active detectable exfil".

## Tool surface

Two tools, both read-only with respect to the secret store.

### `klef_list`

Input (all optional):
```json
{ "tag": "ai", "filter": "stripe" }
```

Output: array of metadata, same shape as `klef list --format json`:
```json
[
  { "name": "stripe", "note": "live key", "tags": ["billing"], "added_at": "2026-04-12T09:11:03Z" }
]
```

No policy check. Reveals no values, only metadata.

### `klef_run`

Input:
```json
{
  "argv": ["npm", "start"],
  "env_refs": ["stripe", "anthropic"],
  "cwd": "/Users/oscarr/Desktop/myproj",
  "timeout_ms": 30000
}
```

- `argv`: required, non-empty, no implicit shell.
- `env_refs`: required (may be empty array), names that exist in the `Store`.
- `cwd`: optional, must be under a configured `workspace_root` if set.
- `timeout_ms`: optional, default 30000, max 300000 (hardcap).

Output (success):
```json
{
  "exit_code": 0,
  "stdout": "Server on port 3000\n",
  "stderr": "",
  "duration_ms": 1843,
  "stdout_truncated": false,
  "stderr_truncated": false,
  "timed_out": false,
  "encoding": "utf8"
}
```

Output (deny / validation error): MCP `isError: true` with one of:
- `policy: no rule matches argv [...] with env_refs [...]`
- `policy: program 'bash' is on the shell denylist`
- `policy: cwd '/etc' is not under any workspace_root`
- `policy: timeout_ms 999999 exceeds max 300000`
- `store: env_ref 'stripe' not found`
- `audit: failed to write log entry, refusing call`

### Tools NOT exposed

- `klef_get` — would leak values to the agent. Removed from the surface.
- `klef_export` — same. To populate a `.env`, the agent writes the *reference* `klef:<name>` directly; values stay in the keychain.
- `klef_add` / `klef_rm` / `klef_edit` — mutation stays manual.

## Architecture

New subcommand `klef mcp [--policy PATH]`. Code lives in `crates/klef-cli/src/commands/mcp/`:

| File | Responsibility |
|---|---|
| `mod.rs` | Entry point `run(store, policy_path)`, MCP loop. |
| `protocol.rs` | Thin wrappers over `rmcp` (init, tool registration, dispatch). |
| `policy.rs` | TOML parse, argv glob matching, shell denylist, `Decision::{Allow, Deny}`. |
| `tools.rs` | Handlers `klef_list`, `klef_run`. |
| `audit.rs` | Append NDJSON, fail-closed. |
| `redact.rs` | Best-effort substitution of resolved values in stdout/stderr. |

`Store` is reused as-is via `klef_core::build_store`. No changes to `klef-core`.

### Dependencies & feature flag

```toml
[features]
default = []
mcp = ["dep:rmcp", "dep:tokio"]
```

The `mcp` feature is off by default. `cargo install klef` produces a binary without MCP support; Homebrew and official release binaries enable it. CI runs with `--all-features`.

`rmcp` (Anthropic-published Rust SDK) handles JSON-RPC framing, initialize handshake, capability negotiation, tool schema validation. `tokio` (multi-thread runtime) is required by `rmcp`'s async surface.

## Policy file

Default location: `~/.config/klef/mcp-policy.toml` (override with `--policy PATH`).

```toml
# Roots under which `cwd` requests are accepted. If empty/unset, klef_run
# ignores client-supplied cwd and uses the cwd of the `klef mcp` process.
workspace_roots = ["/Users/oscarr/Desktop", "/Users/oscarr/code"]

[[allow]]
argv = ["npm", "run", "*"]
env_refs = ["stripe", "anthropic"]

[[allow]]
argv = ["cargo", "test"]
env_refs = []

[[allow]]
argv = ["curl", "https://api.stripe.com/*"]
env_refs = ["stripe"]
```

### Matching semantics

A rule matches a request `(argv, env_refs)` if:

1. `argv.len() == rule.argv.len()`, AND
2. each `argv[i]` matches `rule.argv[i]` as a glob (`*`, `?` wildcards within a single token; no path-separator semantics — these are token-level globs), AND
3. every requested `env_ref` is present in `rule.env_refs`.

Rules are evaluated in file order; the first one that fully covers `(argv, env_refs)` wins. If none cover, deny.

### Hard-coded shell denylist

Even if a rule matches, the request is denied if `Path::file_name(argv[0])` is in:

```
sh, bash, zsh, fish, dash, ksh, csh, tcsh, ash,
python, python3, ruby, perl, lua, awk,
node, deno, bun,
eval, exec, env
```

Justification: these accept arbitrary code as arguments and bypass rule intent. To run scripted logic, the user wraps it in an explicit script invoked directly.

### First-run UX

If the policy file does not exist when `klef mcp` starts:
1. klef writes a commented skeleton (the example above) to the path, with **no active rules**.
2. Logs `wrote skeleton policy to <path>; edit and reload to enable klef_run` to stderr.
3. Server starts. `klef_list` works. All `klef_run` requests deny with `policy: no rule matches ...` until the user edits.

## Runtime semantics — `klef_run`

### Process lifecycle

- Spawn via `tokio::process::Command`, `kill_on_drop(true)`.
- New process group on Unix (`setsid`) so timeout kills the entire descendant tree.
- Stdin: `Stdio::null()`.
- Stdout/stderr: captured to in-memory buffers, read incrementally, truncated at 1 MB each. `*_truncated` flag set when truncation occurred.
- On timeout: `killpg(SIGTERM)` → wait 2s → `killpg(SIGKILL)`. Return `timed_out: true` with whatever was captured pre-kill.

### Environment passed to the child

- All `env_refs` resolved via `Store::get_value`. A `KeyNotFound` for any requested ref → MCP error `policy: env_ref '<name>' not found in store`.
- Whitelist inherited from the parent: `PATH`, `HOME`, `USER`, `LANG`, `LC_ALL`, `TERM`, `TMPDIR`.
- All other parent env vars are **not** passed. The child does not inherit Claude Code's environment.

### Output encoding

- If both stdout and stderr are valid UTF-8 → return as strings, `encoding: "utf8"`.
- Otherwise → return base64 of raw bytes, `encoding: "base64"`.

### Best-effort redaction

After capture, before returning:
- For each resolved `(name, value)` pair where `value.len() > 4`, replace every byte-occurrence of `value` in stdout/stderr with `[REDACTED:<name>]`.
- Operates on raw bytes before UTF-8 conversion to handle binary streams.
- Values ≤ 4 bytes are skipped (false-positive risk too high — e.g. a `PORT=3000` value would match every literal `3000` in output).

This is documented as best-effort. It catches common accidental leaks (verbose-mode programs that print their config). It does not prevent active exfil (base64, XOR, side-channel via timing, or sending the value as part of a legitimate API request whose response is then returned to the agent).

## Audit log

Path: `~/.config/klef/audit.log`. Format: NDJSON (one JSON object per line). Open mode: `O_APPEND` for atomic concurrent writes.

### Entry — allow

```json
{
  "ts": "2026-05-07T14:23:11.234Z",
  "tool": "klef_run",
  "argv": ["npm", "start"],
  "env_refs": ["stripe", "anthropic"],
  "cwd": "/Users/oscarr/Desktop/myproj",
  "decision": "allow",
  "matched_rule_index": 2,
  "exit_code": 0,
  "duration_ms": 1843,
  "stdout_bytes": 142,
  "stderr_bytes": 0,
  "stdout_truncated": false,
  "stderr_truncated": false,
  "timed_out": false
}
```

### Entry — deny

```json
{
  "ts": "...",
  "tool": "klef_run",
  "argv": ["bash", "-c", "echo $STRIPE"],
  "env_refs": ["stripe"],
  "decision": "deny",
  "reason": "shell_denylist:bash"
}
```

`reason` enum: `no_rule_match`, `shell_denylist:<program>`, `cwd_not_in_workspace_roots`, `timeout_exceeds_max`, `env_ref_not_found:<name>`.

### `klef_list` entry (compact)

```json
{ "ts": "...", "tool": "klef_list", "decision": "allow", "count_returned": 7 }
```

### Failure mode — fail-closed

If the audit log write fails (disk full, permissions, FS error), the call is **denied** and an MCP error returned: `audit: <OS error message>`. No call proceeds without a corresponding audit entry. No internal log rotation; the user manages retention via `logrotate` or manual truncation.

## Tests

### Unit tests

- `policy.rs`: glob matching (exact, wildcard, length mismatch), shell denylist (bare name + absolute path), workspace_roots (in/out/symlink resolution), env_refs subset semantics, multi-rule precedence.
- `redact.rs`: simple replace, multi-occurrence, binary streams, ≤ 4-byte values skipped, overlapping values.
- `audit.rs`: entry shapes for allow & deny, fail-closed when the log path is unwritable.

### Integration tests — `crates/klef-cli/tests/mcp.rs`

Fake MCP client speaking JSON-RPC over pipes + file-backed store via `KLEF_TEST_BACKEND=file:`. Scenarios:

1. `initialize` → `tools/list` returns `klef_list` and `klef_run` only.
2. `klef_list` happy path; with `filter`; with `tag`.
3. `klef_run` allow: `exit_code` correct, stdout received, audit entry written.
4. `klef_run` deny via shell denylist (`bash -c …`): MCP error, audit entry with `reason: shell_denylist:bash`.
5. `klef_run` deny via no rule match: MCP error.
6. `klef_run` timeout: `timed_out: true`, partial stdout captured, descendant tree killed (verified by checking no stray children of the test process group).
7. `klef_run` redaction: a policy-allowed wrapper script that prints the secret → output contains `[REDACTED:<name>]`, not the raw value.
8. Audit fail-closed: invalid log path → all calls deny with `audit:` reason.

### Smoke test (manual, documented in `docs/mcp.md`)

- Add klef to `~/Library/Application Support/Claude/claude_desktop_config.json`.
- Ask Claude Desktop "list my klef keys" → response lists names.
- Ask Claude Desktop "show me my Stripe key" → Claude reports it cannot (no `klef_get` tool exposed).

### CI

`.github/workflows/ci.yml` already runs `cargo test --workspace --all-features`. The `mcp` feature is included in `--all-features`, so CI exercises this code on macOS and Ubuntu.

The `.githooks/pre-commit` line cap (< 300 lines/file) is respected by construction via the `mod/protocol/policy/tools/audit/redact` split.

## Out of scope (future work)

- Interactive approval (notification or GUI side-channel) for first-time use without policy edit.
- Remote MCP transport (HTTP/SSE).
- An `klef mcp policy add <pattern>` helper to edit the policy without hand-editing TOML.
- A programmatic API (Rust crate) for embedding the MCP server in other tools.
