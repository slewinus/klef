# macOS Keychain auto-configuration — design

**Status:** Draft v2 (revised after security review)
**Date:** 2026-05-07

## Goal

Eliminate the recurring "type your login password to unlock the keychain" prompts that plague klef users on macOS, without silently modifying a global system security setting on the user's behalf.

## The problem

By default macOS does not auto-lock the login keychain — but corporate MDM policies, security-conscious user setups, and some macOS configurations enable a short auto-lock timeout (10–30 min). When the keychain is locked, every klef call that reads or writes a value (`get`, `show`, `run`, `import`, `export`, MCP `klef_run`) triggers a system prompt asking for the user's login password.

For a tool whose entire value proposition is "fast, frictionless secret access," this defeats the purpose.

## Design principle: surface, don't mutate

An earlier draft of this spec proposed silently auto-applying the fix on first run. A security review pushed back: modifying a global macOS security setting without explicit user consent is too aggressive, and the proposed opt-out env var is useless to first-run users who don't yet know it exists.

The revised approach:
- klef **never modifies** the user's login keychain settings on its own.
- klef **detects** the issue and **surfaces** it via a one-time stderr banner that prints in-context, right when the user is about to be prompted for their password.
- The banner contains the exact command to fix it (`klef keychain configure`).
- The user runs the command (or not) — explicit consent is preserved.

This satisfies both the user's "don't make me discover a hidden command" goal (the banner shows the command in-context, unmissable) and the reviewer's "don't silently modify global state" requirement (modification only happens on explicit user action).

## Non-goals

- Solving the prompt for users on locked-down corporate Macs whose MDM enforces a non-zero timeout. Those users will see klef's fix get reverted by MDM at the next sync. Documented limitation.
- Touching keychain behavior on Linux. Secret Service has different semantics; no analogous fix.
- Replacing the user's login keychain with a dedicated klef keychain.
- A `klef doctor` umbrella command. We focus on a single targeted command.
- A `klef keychain status` or `klef keychain revert` subcommand in v1. The applied state is observable via the marker file and via `security show-keychain-info`; revert is achieved by running the precise command klef prints at apply time. We can add these subcommands later if user feedback demands them.

## Architecture

Three layers:

1. **`klef-core::macos_keychain`** — gated `#[cfg(target_os = "macos")]`. Pure data and shell-out operations. UI-agnostic.
2. **`klef-cli`** — uses (1) for the `klef keychain configure` subcommand and the one-time banner.
3. **`klef-gui`** — uses (1) for a "Fix Keychain prompts" settings button. UI design out of scope of this spec; the spec only ensures the core helpers are reachable from the GUI crate.

### `klef-core::macos_keychain` public API

```rust
#[cfg(target_os = "macos")]
pub mod macos_keychain {
    pub struct KeychainStatus {
        pub path: PathBuf,                  // resolved via `security default-keychain`
        pub timeout_seconds: Option<u64>,   // None = no-timeout
        pub lock_on_sleep: bool,
    }

    pub enum KeychainHelperError { /* I/O + parsing + missing default keychain */ }

    /// Read current login keychain settings.
    pub fn current_status() -> Result<KeychainStatus, KeychainHelperError>;

    /// True iff `current_status()` indicates settings that won't trigger
    /// password re-prompts during a session: no timeout AND not lock-on-sleep.
    pub fn is_already_friendly(s: &KeychainStatus) -> bool;

    /// Apply the friendly settings (no timeout, no lock-on-sleep) by invoking
    /// `/usr/bin/security set-keychain-settings <path>`. Does NOT touch any
    /// marker or printing — those are caller concerns.
    pub fn apply_friendly_settings(s: &KeychainStatus) -> Result<(), KeychainHelperError>;

    /// Build the precise shell command that reverts to the prior state.
    /// e.g. `security set-keychain-settings -t 600 -l <path>`.
    pub fn build_revert_command(prev: &KeychainStatus) -> String;
}

#[cfg(not(target_os = "macos"))]
pub mod macos_keychain {} // empty
```

The CLI/GUI layers add their own UI (banner text, marker file persistence, subcommand wiring).

### Resolving the keychain path

Hardcoded `~/Library/Keychains/login.keychain-db` is wrong: the user's default keychain may differ. We resolve via:

```
/usr/bin/security default-keychain
```

The output is a quoted path on stdout (e.g., `"/Users/oscarr/Library/Keychains/login.keychain-db"`). Strip surrounding quotes, use as the canonical path for all subsequent operations. If `default-keychain` fails or returns unexpected output, the helper returns an error and the CLI/GUI layer handles it (banner shows nothing, subcommand prints the error).

### Shell-out via `/usr/bin/security`

Always invoked with the absolute path `/usr/bin/security` (not via `$PATH`) to avoid surprises from a user-shadowed `security` in their PATH.

Three invocations the helper performs:

| Command | Purpose |
|---|---|
| `/usr/bin/security default-keychain` | Resolve the path. |
| `/usr/bin/security show-keychain-info <path>` | Read current settings. macOS versions vary on stdout vs stderr; helper reads both streams (concatenated) and parses for `no-timeout`, `timeout=Ns`, `lock-on-sleep`. |
| `/usr/bin/security set-keychain-settings [-t N] [-l] <path>` | Apply settings (no flags = no-timeout + no-lock-on-sleep). |

## CLI: the banner

### Trigger conditions (all must hold)

1. `cfg(target_os = "macos")`.
2. Effective backend is Keychain (not `--backend age:...`, not the debug-only file backend).
3. Command will read or write a Keychain value: one of `get`, `show`, `run`, `import`, `export`, `add`, `edit`, `rm`, `set-note`, `rename`, `mcp`. **NOT** triggered for: `list`, `status`, `completions`, `names`, `tags`, `discover`, `backup`, `restore`. (`backup`/`restore` involve all keys but happen rarely; users initiating these have given consent to a heavy operation already, so omitting the banner there is fine.)
4. `KLEF_NO_KEYCHAIN_AUTOCONFIG` is not set.
5. Marker file does not exist.
6. `current_status()` succeeds and `is_already_friendly()` returns false.

If any condition fails: silent, no banner.

### Banner text

```
klef: heads up — your macOS keychain auto-locks (timeout: 600s, lock-on-sleep: yes).
      You'll be prompted for your password on every klef call until this is fixed.
      One-shot fix (modifies macOS Keychain settings, not your klef data):
          klef keychain configure
      To suppress this notice without fixing:
          export KLEF_NO_KEYCHAIN_AUTOCONFIG=1
```

The exact `timeout: Xs` and `lock-on-sleep: yes/no` values come from `current_status()`.

### Marker after banner

After printing the banner once, klef writes the marker (with `applied: false`) and never prints the banner again unless the user deletes the marker. The marker's role here is *suppress repetitive nagging*, nothing more.

### Trigger placement in code

In `klef-cli/src/lib.rs::run()`, after `build_store(...)` and before the `match cli.command` dispatch:

```rust
#[cfg(target_os = "macos")]
{
    if backend_is_keychain(&store) && command_touches_values(&cli.command) {
        let _ = banner::maybe_show(); // best-effort, never propagates
    }
}
```

Two small predicate functions. `backend_is_keychain` queries the Store's backend description string. `command_touches_values` is a static match on `cli.command` listing the value-touching variants.

For `klef mcp`, the banner is shown at startup if the conditions hold (the user sees it on stderr in Claude Desktop's MCP logs or in the Claude Code session output).

## CLI: `klef keychain configure`

A new subcommand that applies the fix explicitly.

### CLI shape

```rust
// In cli.rs
Keychain {
    #[command(subcommand)]
    action: KeychainAction,
}

enum KeychainAction {
    /// Disable macOS keychain auto-lock to stop password re-prompts.
    Configure,
}
```

For v1 only `Configure`. Future `Status` / `Revert` subcommands fit the same shape.

### Behavior

```
1. cfg(target_os = "macos") only. On Linux: print "this command is macOS-only" and exit non-zero.
2. Read current_status() — if it errors, print the error and exit non-zero.
3. If is_already_friendly(): print "macOS keychain is already configured for klef. Nothing to do." Write the marker. Exit 0.
4. Save current status as `prev` for revert command construction.
5. Call apply_friendly_settings(). If it errors, print + exit non-zero.
6. Write the marker with prev state baked in (see below).
7. Print:
    "klef: macOS keychain configured. You should no longer be prompted for your password during this login session.
     To revert: <build_revert_command(prev)>"
   Exit 0.
```

## Marker file

Path: `~/.config/klef/keychain-configured` (under `dirs::config_dir()`, matches existing klef convention).

Format: JSON.

After banner shown but no apply:
```json
{
  "applied": false,
  "banner_shown_at": "2026-05-07T14:23:45Z"
}
```

After `klef keychain configure`:
```json
{
  "applied": true,
  "configured_at": "2026-05-07T14:23:45Z",
  "keychain_path": "/Users/oscarr/Library/Keychains/login.keychain-db",
  "prev_timeout_seconds": 600,
  "prev_lock_on_sleep": true
}
```

The marker's job is twofold:
1. Suppress the banner on subsequent runs.
2. (When `applied: true`) record the prior state so the revert command in the post-apply message is faithful: `security set-keychain-settings -t 600 -l <path>` for the example above.

## Opt-out

`KLEF_NO_KEYCHAIN_AUTOCONFIG` env variable, any value (including empty). When set:
- The banner is never shown.
- `klef keychain configure` still works (it's an explicit user action; opt-out is for the *passive surface*, not for forbidding the user from using the explicit command).

## Failure modes

| Failure | klef behavior |
|---|---|
| `/usr/bin/security` missing or broken | Helper returns error. Banner code swallows; subcommand prints error and exits non-zero. |
| `default-keychain` returns no path | Same as above. Banner silently skips; subcommand exits non-zero. |
| `show-keychain-info` returns unparseable output | Treated as "unknown state" → banner doesn't show; subcommand exits non-zero with the raw error. |
| `set-keychain-settings` fails (permissions, MDM, etc.) | Subcommand prints error, NO marker written, exit non-zero. |
| Marker write fails | Best-effort: print warning, but the underlying `security` change has already been applied. The user can re-run the command if they want the marker. |
| User runs klef as root via `sudo` | Modifies root's keychain, not the user's. Documented gotcha; not detected by code. |

## Testing

### Unit tests (in `klef-core/src/macos_keychain.rs`, gated `#[cfg(target_os = "macos")]`)

The shell-out is encapsulated behind a trait so tests inject mock outputs:

```rust
trait SecurityCli {
    fn default_keychain(&self) -> Result<String, KeychainHelperError>;
    fn show_keychain_info(&self, path: &Path) -> Result<String, KeychainHelperError>;
    fn set_keychain_settings(&self, path: &Path, timeout: Option<u64>, lock_on_sleep: bool) -> Result<(), KeychainHelperError>;
}
```

Production impl wraps `Command::new("/usr/bin/security")`. Test impl is a struct with configurable returns. Public functions (`current_status`, `apply_friendly_settings`, etc.) take `impl SecurityCli` so tests can substitute.

Tests:

1. `default_keychain_strips_quotes` — input `"/path/to/login.keychain-db"\n`, expect `/path/to/login.keychain-db`.
2. `parse_show_no_timeout_no_lock` — input `Keychain "..." no-timeout`, expect `KeychainStatus { timeout_seconds: None, lock_on_sleep: false, .. }`.
3. `parse_show_timeout_with_lock` — input `Keychain "..." timeout=600s lock-on-sleep`, expect `Some(600), true`.
4. `is_already_friendly_true_only_when_both_clear` — table-driven: friendly only when timeout None AND lock_on_sleep false.
5. `build_revert_command_includes_timeout_and_lock` — given prev state with `timeout: 600, lock_on_sleep: true`, output contains `-t 600 -l`.
6. `build_revert_command_no_lock` — given prev `timeout: 30, lock_on_sleep: false`, output contains `-t 30` and NOT `-l`.
7. `apply_friendly_settings_invokes_security_with_no_flags` — mock SecurityCli, assert `set_keychain_settings(path, None, false)` was called.

### CLI-level tests (`klef-cli/src/...`)

8. `banner_does_not_show_when_marker_present` — set fake config dir with marker, run banner check, assert silent.
9. `banner_does_not_show_when_env_var_set` — set `KLEF_NO_KEYCHAIN_AUTOCONFIG`, assert silent.
10. `banner_shown_writes_marker_with_applied_false` — happy path, assert marker JSON has `applied: false`.
11. `keychain_configure_writes_marker_with_prev_state` — happy path with mock helper, assert marker JSON contains the prev state fields.
12. `keychain_configure_idempotent_when_already_friendly` — mock returns `is_already_friendly == true`, assert no `set_keychain_settings` call.

Pure Rust, no real `security` binary touched, no real keychain touched.

### Integration / smoke

None automated. Manual smoke test documented in the implementation plan: install klef on a Mac with `security set-keychain-settings -t 600 -l <login>`, run `klef get something`, verify banner appears once, then run `klef keychain configure`, verify settings change, verify marker contents.

### CI

Already runs on macOS + Linux. Linux compiles out the module entirely. macOS runs the unit tests via mocks.

## GUI integration (out of detailed scope)

`klef-gui` will gain a settings panel button "Fix macOS Keychain prompts". When clicked:
- Calls `klef_core::macos_keychain::current_status()` to show the current state.
- Confirms with the user (modal: "klef will run `security set-keychain-settings ...`. Continue?").
- Calls `apply_friendly_settings()`.
- Reports success + the revert command.

The GUI does NOT show the CLI banner or write the CLI's marker file — those are CLI concerns. The GUI may track its own UI state for "already shown the user this option."

GUI implementation is its own follow-up; this spec ensures the core helpers exist and are reachable from the GUI crate.

## Documentation

Two doc updates as part of the implementation:

1. **`README.md`**: in the platforms / macOS section, add a one-line note: "On macOS, klef uses your login keychain. If you see frequent password prompts, run `klef keychain configure` (one-shot) — see `docs/macos-keychain.md` for details."
2. **`docs/macos-keychain.md`** (new): explains the timeout problem, the fix, the security tradeoffs, the opt-out env var, the revert command, and the corporate-MDM caveat.

(The earlier draft referenced `docs/troubleshooting.md` which doesn't exist. Replaced with the dedicated `docs/macos-keychain.md`.)

## Out of scope (future work)

- `klef keychain status` and `klef keychain revert` subcommands (logical extensions if v1 generates demand).
- Auto-config for Linux Secret Service auto-lock policies.
- A "klef-managed dedicated keychain" (option β3 from brainstorm).
- Detection of MDM-enforced timeouts that will be reverted at next sync.
- An interactive confirmation prompt (`y/N`) inside `klef keychain configure`. The command is already explicit; an extra prompt would be redundant.
