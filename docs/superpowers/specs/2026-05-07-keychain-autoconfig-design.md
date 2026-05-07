# macOS Keychain auto-configuration — design

**Status:** Draft
**Date:** 2026-05-07

## Goal

Eliminate the recurring "type your login password to unlock the keychain" prompts that plague klef users on macOS, without requiring them to discover or run any command.

## The problem

By default macOS does not auto-lock the login keychain — but corporate MDM policies, security-conscious user setups, and some macOS configurations enable a short auto-lock timeout (10–30 min). When the keychain is locked, every klef call (`get`, `show`, `run`, MCP `klef_run`) triggers a system prompt asking for the user's login password.

For a tool whose entire value proposition is "fast, frictionless secret access," this defeats the purpose.

## Non-goals

- Solving the prompt for users on locked-down corporate Macs whose MDM enforces a non-zero timeout. Those users will see klef's auto-config get reverted by MDM at the next sync. Documented limitation; out of scope for klef code.
- Touching keychain behavior on Linux. Secret Service has different semantics, no analogous fix needed.
- Replacing the user's login keychain with a dedicated klef keychain. That's a much larger change ("option β3" in the brainstorm) we reject in favor of the simpler global timeout fix.
- A user-facing `klef doctor` command for keychain diagnostics. Users shouldn't need to discover such a command — the fix should be automatic.

## Threat model considerations

This change disables auto-lock on the user's login keychain, which has security implications:

- **Reduced cost of post-login attack**: an attacker with physical access to the unlocked Mac no longer faces a re-prompt for keychain items. They already have everything else (browser session, files, ssh keys held in agent) — so the marginal increase in attack surface is small.
- **MDM-enforced timeouts may be there for compliance reasons.** klef's auto-config silently overrides this, which could violate the user's organization's security posture. The opt-out env var (`KLEF_NO_KEYCHAIN_AUTOCONFIG=1`) and the printed revert command provide an escape hatch.
- **Users who explicitly chose a short timeout** (rare but possible) will see klef revert their setting. The persistence marker (see below) ensures klef only does this once — after that, klef respects whatever the user later sets.

## Architecture

A small new module in `klef-cli`: `crates/klef-cli/src/macos_keychain.rs`. Plataform-gated to compile only on macOS; on Linux the module body is empty.

Public surface:

```rust
#[cfg(target_os = "macos")]
pub fn ensure_configured();

#[cfg(not(target_os = "macos"))]
pub fn ensure_configured() {} // no-op
```

Called once in `klef-cli::lib::run()` before the command dispatch:

```rust
pub fn run(cli: Cli) -> Result<(), KlefError> {
    crate::macos_keychain::ensure_configured();
    let store = klef_core::build_store(cli.backend.as_deref())?;
    // ... existing dispatch
}
```

The function is best-effort: it never propagates errors back to `run()`. A failure prints a warning to stderr and returns; klef continues normally.

## Logic

```text
1. Skip if env var KLEF_NO_KEYCHAIN_AUTOCONFIG is set (any value).
2. Skip if marker file ~/.config/klef/keychain-configured exists.
3. Run `security show-keychain-info <login-keychain-path>`.
4. If the output indicates already-no-timeout AND no lock-on-sleep:
     - Write the marker (so future runs skip this work).
     - Return.
5. Run `security set-keychain-settings <login-keychain-path>` (no flags).
   - On success: print one-time stderr message (see below), write marker, return.
   - On failure: print warning to stderr (no marker written, klef will retry next run).
```

### Paths

- Login keychain: `~/Library/Keychains/login.keychain-db` (resolved via `dirs::home_dir()`).
- Marker file: `~/.config/klef/keychain-configured` (under `dirs::config_dir()` per existing klef convention; matches the location of `index.json`).

The marker file is empty. Its existence is the only signal.

### Shell-out via `security`

Two invocations, both via `std::process::Command::new("security")`:

| Command | Purpose | Output we care about |
|---|---|---|
| `security show-keychain-info <path>` | Detect current timeout / lock-on-sleep | combined output text containing `no-timeout` or `timeout=Ns`, plus optional `lock-on-sleep` |
| `security set-keychain-settings <path>` | Apply: remove timeout, remove lock-on-sleep | exit code (0 = success) |

The `security` binary always exists on macOS at `/usr/bin/security`. We rely on `$PATH` resolution.

Note on `show-keychain-info`: macOS versions vary on whether the info goes to stdout or stderr. The parser reads both streams (concatenated) to be robust.

### One-time stderr message on first config

On successful application of the fix:

```
klef: configured macOS keychain to remain unlocked for this session.
      to revert:    security set-keychain-settings -t 600 ~/Library/Keychains/login.keychain-db
      to opt out:   export KLEF_NO_KEYCHAIN_AUTOCONFIG=1
```

Printed only once (the marker prevents re-printing on subsequent runs).

On failure:

```
klef: could not configure macOS keychain ({error}).
      you may see frequent password prompts; klef will retry next run.
```

## Opt-out

Environment variable `KLEF_NO_KEYCHAIN_AUTOCONFIG`, any value (incl. empty). When set, `ensure_configured()` is a no-op without any side effects (no marker write, no stderr output).

Use cases:
- Corporate MDM users who don't want klef fighting their policy.
- Users on locked-down setups who explicitly want short timeouts.
- CI environments where touching the keychain is undesirable.

## Persistence — the marker file

The marker file at `~/.config/klef/keychain-configured` exists to make `ensure_configured()` idempotent across runs without re-checking `security show-keychain-info` on every klef invocation (avoiding ~5–20 ms of process spawn on every command).

Behavior:
- **Marker missing**: klef checks current state, applies fix if needed, creates marker on success.
- **Marker present**: klef skips entirely.
- **User wants klef to re-run autoconfig**: `rm ~/.config/klef/keychain-configured`. Documented in the failure message and in `docs/troubleshooting.md`.

If the user later re-enables a timeout manually (e.g., via Keychain Access.app), klef does NOT re-fix it — the marker is present, so klef respects the user's later choice.

## Failure modes

| Failure | klef behavior |
|---|---|
| `security` binary missing or `$PATH` issue | Warning on stderr, no marker, klef continues normally. |
| `security show-keychain-info` returns unexpected output | Treated as "not yet configured" → tries to set settings (idempotent op). If that fails too, warning + no marker. |
| `security set-keychain-settings` fails (permissions, missing keychain file, etc.) | Warning on stderr, no marker, klef continues normally. |
| Marker file write fails (config dir not writable) | Warning on stderr, klef continues. Without the marker, klef will retry next run, which is fine — the underlying setting is already applied. |
| User running klef as root via `sudo` | The login keychain affected is root's, not the actual user's. Documented gotcha; klef does not detect or refuse this case. |

## Testing

### Unit tests (in `macos_keychain.rs`, gated `#[cfg(target_os = "macos")]`)

The shell-out is encapsulated behind a trait so tests can mock it:

```rust
trait SecurityCli {
    fn show(&self, path: &Path) -> std::io::Result<String>;
    fn set(&self, path: &Path) -> std::io::Result<()>;
}
```

Production impl wraps `Command::new("security")`. Test impl is a struct with configurable returns.

Tests:

1. `parse_show_output_detects_no_timeout` — feed the expected `Keychain "..." no-timeout` string, assert `is_already_configured()` returns `true`.
2. `parse_show_output_detects_timeout_present` — feed `timeout=600s`, assert `false`.
3. `parse_show_output_detects_lock_on_sleep` — feed `no-timeout, lock-on-sleep`, assert `false` (we want both gone).
4. `marker_path_resolves_under_config_dir` — assert `marker_path()` returns a path under `dirs::config_dir().unwrap()`.
5. `opt_out_env_var_is_respected` — `set_var(KLEF_NO_KEYCHAIN_AUTOCONFIG, "1")`, run `ensure_configured()` against a mock that records calls, assert no calls made.
6. `existing_marker_is_respected` — create a temp dir as fake config dir, write a marker, run `ensure_configured()`, assert no `security` calls made.
7. `successful_apply_writes_marker_and_prints` — mock returns success, assert marker written, capture stderr, assert message present.
8. `failed_apply_does_not_write_marker` — mock returns error, assert no marker written.

Pure-Rust tests, no real `security` binary touched, no real keychain touched.

### Integration tests

None. The real `security` command and the user's actual keychain are not safe to exercise in CI.

### CI

`.github/workflows/ci.yml` already runs on macOS + Linux. On Linux the module is empty, all tests trivially compile out. On macOS the unit tests run via mocks.

## Out of scope (future work)

- A `klef doctor` command for diagnostic + manual revert. Useful but not blocking.
- Auto-config for Linux Secret Service auto-lock policies. Linux variants don't have a single coherent "timeout" knob; gnome-keyring and KWallet handle this differently.
- A "klef-managed dedicated keychain" (option β3 from brainstorm). Reconsidered if user feedback shows the global-login-keychain approach causes real friction.
- Detection of MDM-enforced timeout that will be reverted at next sync. Could be a logged hint but requires identifying MDM presence.
