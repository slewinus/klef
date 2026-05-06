# Releasing klef

## TL;DR

```bash
# 1. Update CHANGELOG.md: move [Unreleased] to [X.Y.Z]
# 2. Bump Cargo.toml version
# 3. Commit + push to main
# 4. Tag and push:
git tag -a vX.Y.Z -m "klef vX.Y.Z"
git push origin vX.Y.Z
# 5. The release workflow (.github/workflows/release.yml) builds binaries
#    for 4 platforms and attaches them to the GitHub Release automatically.
# 6. Update Homebrew formula (see docs/release.md "Homebrew" section once #10 lands).
```

## Verifying without tagging

Use the workflow_dispatch trigger:

```bash
gh workflow run release.yml -f tag=v0.0.0-test
gh run watch
```

This builds artifacts but does NOT create a release. Artifacts download from the workflow run page.

## Build matrix

| Target | Runner | Notes |
|---|---|---|
| x86_64-apple-darwin | macos-13 (Intel) | Native build |
| aarch64-apple-darwin | macos-latest (Apple Silicon) | Native build |
| x86_64-unknown-linux-gnu | ubuntu-latest | Native; libdbus-1-dev installed |
| aarch64-unknown-linux-gnu | ubuntu-24.04-arm | Native; libdbus-1-dev installed |

Linux Secret Service support requires `libdbus-1-dev` at build time. End-users running the binary still need a Secret Service implementation (gnome-keyring, KWallet) at runtime; otherwise klef emits the platform hint introduced in #9.

## macOS gatekeeper

Binaries are NOT codesigned or notarized in this release flow (tracked in #20). On first run, macOS may quarantine them. Workaround for users:

```bash
xattr -d com.apple.quarantine ~/.local/bin/klef
```

A real codesigning + notarization pipeline is the next big distribution improvement and lives in a future issue.

## Homebrew (one-time setup)

The `slewinus/homebrew-tap` repo doesn't exist yet. To set it up the first time:

1. Create a new public GitHub repo named `homebrew-tap` under your account (`slewinus/homebrew-tap`).
2. From that repo's local clone, create the directory layout:
   ```
   homebrew-tap/
   └── Formula/
       └── klef.rb
   ```
3. Tag a release of klef (e.g. `v0.2.0`). The `release.yml` workflow builds the four tarballs and attaches them to the GitHub Release.
4. From this klef repo, run:
   ```bash
   scripts/update-homebrew-formula.sh v0.2.0 /path/to/homebrew-tap/Formula/klef.rb
   ```
5. Commit and push the populated formula to the tap repo.
6. End-users can now install:
   ```bash
   brew tap slewinus/tap
   brew install klef
   ```

## Homebrew (subsequent releases)

After step 1-2 are done once, releases just need:

1. Tag the new version (`vX.Y.Z`).
2. Wait for the release workflow to publish binaries.
3. Run the update script:
   ```bash
   scripts/update-homebrew-formula.sh vX.Y.Z ~/code/homebrew-tap/Formula/klef.rb
   ```
4. Commit and push the bumped formula.

A future enhancement (out of scope for v0.2) automates the formula bump via a workflow step that opens a PR on the tap repo on every release. See [#10](https://github.com/slewinus/klef/issues/10) for the tracking discussion.

## Headless / CI / Docker — age backend

When the OS keychain isn't available (Linux servers without gnome-keyring,
CI runners, Docker containers), use the age-encrypted file backend:

```bash
# Interactive use — prompts for passphrase on first call (twice for confirmation)
klef --backend age:/path/to/secrets.age add stripe

# CI use — passphrase via env var (set by the CI secret manager)
KLEF_PASSPHRASE=$RUNNER_SECRET klef --backend age:./secrets.age get stripe
```

The vault is a single age-encrypted file. Every `get`/`set`/`remove`
decrypts → mutates → re-encrypts atomically (tmp + rename).

**Setup** in a fresh CI:
1. Create the vault locally (interactive): `klef --backend age:./secrets.age add my-secret`
2. Store `./secrets.age` in a separate private repo or CI secret manager
3. In CI, fetch the file + the passphrase, then:
   `KLEF_PASSPHRASE=$P klef --backend age:./secrets.age run -- ./script.sh`

**Passphrase loss = unrecoverable**. age has no backdoor. Document your passphrase policy.

**Asymmetric mode** (`--recipient age1...`) is not yet supported on the read side
in v0.4 — passphrase only. File a follow-up if you need YubiKey-resident keys
for a CI scenario.
