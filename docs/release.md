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
