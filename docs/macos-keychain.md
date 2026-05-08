# macOS keychain — frequent password prompts

If klef keeps prompting you for your login password every 10–30 minutes
on macOS, the cause is the login keychain's auto-lock timeout, not klef
itself. macOS re-locks the keychain after the timeout and every klef
call to read a value (`get`, `show`, `run`, MCP `klef_run`) triggers a
re-unlock prompt.

## One-shot fix

```bash
klef keychain configure
```

This runs `security set-keychain-settings` against your default keychain
(no flags = no timeout, no lock-on-sleep). klef writes a marker at
`~/Library/Application Support/klef/keychain-configured` (the platform
config dir) recording your prior settings so the post-run output shows
the exact revert command.

After running, you should see no further password prompts during the
current login session. The keychain still locks at logout/reboot — your
data is no less secure at rest, only the auto-lock-during-session
behavior changes.

## Tradeoff

Disabling auto-lock means an attacker with physical access to your
unlocked Mac no longer faces a re-prompt for keychain items. They
already have your browser sessions, ssh-agent keys, etc. — so the
marginal increase in attack surface is small but non-zero. If your
threat model is "someone briefly walks up to my unlocked screen", keep
the timeout and accept the prompts.

## Opt out

To suppress the banner without applying the fix:

```bash
export KLEF_NO_KEYCHAIN_AUTOCONFIG=1
```

This only suppresses the in-context banner; running `klef keychain
configure` still works (it's an explicit user action).

## Reverting

The post-run output of `klef keychain configure` prints the precise
revert command using your prior state, e.g.:

```
security set-keychain-settings -u -t 600 -l /Users/you/Library/Keychains/login.keychain-db
```

Or you can adjust the timeout via Keychain Access.app: open it,
right-click the `login` keychain, "Change settings for keychain login…",
configure as you wish.

## Corporate Mac / MDM

If your machine is managed by an MDM (Jamf, Intune, etc.) that enforces
a non-zero keychain timeout for compliance reasons, klef's fix will get
reverted at the next sync. For these setups: do not run `klef keychain
configure`. Either accept the prompts or use the `--backend age:...`
file backend with `KLEF_PASSPHRASE` for non-interactive flows.

## What klef detects automatically

When klef is about to read or write a keychain value AND your default
keychain has auto-lock enabled, klef prints a one-time banner pointing
you at `klef keychain configure`. The banner suppresses itself after one
showing (marker file). It re-shows if your keychain state changes or if
the marker is older than 7 days.

The banner does NOT print from `klef mcp` because Claude Desktop captures
that process's stderr to log files you don't read. Pure-MCP-only users
will discover the fix via this doc or the GUI's settings panel.
