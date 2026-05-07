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

Path: `~/.config/klef/mcp-policy.toml`. First run writes a commented skeleton; edit it to enable `klef_run`.

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

Every call (allow or deny) writes one NDJSON entry to `~/.config/klef/audit.log`. If the log can't be written, the call is denied (fail-closed). No internal rotation — manage with `logrotate` if you keep it forever.

## Threat model

This is **not** a zero-knowledge system. A malicious agent can craft an `argv` that exfiltrates a value (e.g., `curl` with the key in a query string). What this design changes vs. exposing a `klef_get` tool:

- Without `klef_run`: every normal use puts plaintext secrets into the agent's context — passive, continuous leak into transcripts and provider logs.
- With `klef_run`: secrets only enter agent-visible output if the agent issues an explicitly extractive `argv` — leaves an audit trail, rejectable via policy.

Risk shifts from "passive systematic leak" to "active detectable exfil".
