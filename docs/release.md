# Releasing klef

## TL;DR

```bash
# 1. Update CHANGELOG.md: move [Unreleased] to [X.Y.Z]
# 2. Bump Cargo.toml version (only the crates that actually changed)
# 3. Commit + push to main
# 4. Tag FROM MAIN — check where you are first, see the warning below:
git checkout main && git pull --ff-only
git tag -a vX.Y.Z -m "klef vX.Y.Z"
git push origin vX.Y.Z
# 5. The release workflow (.github/workflows/release.yml) builds binaries
#    for 4 platforms and attaches them to the GitHub Release automatically.
# 6. VERIFY the workflow actually ran and the release has 8 assets:
gh run list --workflow=release.yml --limit 1
gh release view vX.Y.Z --json assets --jq '.assets | length'
# 7. Publish to crates.io (manual, in dependency order):
cargo publish -p klef-core && cargo publish -p klef
# 8. Bump the Homebrew formula (see the "Homebrew" section below).
```

## Tag from main, not from wherever you happen to be

`git tag` tags `HEAD`. Run it on a feature branch and the tag lands on a commit
that isn't on `main` — the release then builds from that tree, and the tag turns
into a dangling reference the moment the branch is rebased or deleted.

This has happened: v0.4.3 was first tagged from a docs branch, and the release
workflow started building the wrong tree before it was caught. Recovery, if you
notice in time:

```bash
gh run cancel <run-id>            # before the release job publishes anything
git push origin :refs/tags/vX.Y.Z # delete the remote tag
git tag -d vX.Y.Z
git tag -a vX.Y.Z <main-sha> -m "klef vX.Y.Z" && git push origin vX.Y.Z
```

Afterwards, confirm the tag is reachable from main:

```bash
git merge-base --is-ancestor "$(git rev-list -n1 vX.Y.Z)" origin/main && echo OK
```

## Verifying without tagging

Use the workflow_dispatch trigger:

```bash
gh workflow run release.yml -f tag=v0.0.0-test
gh run watch
```

This builds artifacts but does NOT create a release (the `release` job is gated
on `github.event_name == 'push'`). Artifacts download from the workflow run page.

## After tagging: check that the workflow actually ran

Pushing the tag is not proof the release built. v0.4.2 was tagged and its
release page written by hand, but no workflow run ever fired, so the release
sat with zero assets while `cargo install` and Homebrew silently kept serving
0.4.1. Always confirm:

```bash
gh run list --workflow=release.yml --limit 1   # must show the new tag
gh release view vX.Y.Z --json assets --jq '.assets | length'   # must be 8
```

Then check the binary is actually the one you meant to ship. The release job
builds with `--features mcp` and verifies `klef mcp --help` on every natively
executable target — that guard exists because v0.4.0 through v0.4.3 all shipped
tarballs with no MCP server while the CHANGELOG said otherwise. Nothing caught
it because the GUI's CI job *does* build with the feature, so the binary bundled
inside `klef.app` had MCP and only the tarballs didn't. Spot-check anyway:

```bash
tar -xzf klef-vX.Y.Z-aarch64-apple-darwin.tar.gz
./klef-vX.Y.Z-aarch64-apple-darwin/klef mcp --help >/dev/null && echo "mcp OK"
```

If the release exists but has no assets, don't delete it — deleting throws away
hand-written release notes. Build the artifacts with the dispatch trigger above
and attach them to the existing release:

```bash
gh run download <run-id> -D /tmp/rel
gh release upload vX.Y.Z /tmp/rel/*/*.tar.gz /tmp/rel/*/*.sha256
```

## crates.io

`cargo publish` is NOT part of the release workflow — it's manual, and it's two
publishes in dependency order:

```bash
cargo publish -p klef-core   # must land first; klef depends on the exact version
cargo publish -p klef
```

Skipping this is invisible from the GitHub side: the release looks complete
while `cargo install klef` keeps installing the previous version.

## Build matrix

| Target | Runner | Notes |
|---|---|---|
| x86_64-apple-darwin | macos-latest (Apple Silicon) | Cross-compiled |
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

## Homebrew

The tap lives at [`slewinus/homebrew-tap`](https://github.com/slewinus/homebrew-tap)
and carries two artifacts for the same `klef` token:

- `Formula/klef.rb` — the CLI, all four targets (macOS Intel + ARM, Linux x86_64 + ARM).
  This is what the bare `brew install klef` resolves to.
- `Casks/klef.rb` — the macOS menu bar `.app` with the CLI bundled inside it,
  Apple Silicon only. Needs an explicit `brew install --cask klef`.

Homebrew refuses to load formulae *and* casks from third-party taps until the
user runs `brew trust slewinus/tap`. That is not something the tap can opt out
of, so the install instructions in both READMEs have to spell it out.

### Bumping the formula after a release

1. Tag the new version and confirm the workflow published all 8 assets (above).
2. Regenerate the formula against the published tarballs:
   ```bash
   scripts/update-homebrew-formula.sh vX.Y.Z /path/to/homebrew-tap/Formula/klef.rb
   ```
   The script downloads each tarball and computes its SHA-256, so it fails loudly
   if an asset is missing.
3. Sanity-check before pushing:
   ```bash
   brew style /path/to/homebrew-tap/Formula/klef.rb
   brew fetch --formula --force slewinus/tap/klef   # verifies url + sha256
   ```
4. Commit and push to the tap.

The cask is bumped separately and only when a `.dmg` exists for the version —
`release.yml` does not build the GUI today, so the cask lags the formula. See
[#123](https://github.com/slewinus/klef/issues/123).

Automating the formula bump via a workflow step that opens a PR on the tap repo
is tracked in [#10](https://github.com/slewinus/klef/issues/10).

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

**Asymmetric mode.** `klef backup <out.age> --recipient age1...` encrypts to a
public key; restore it with the matching private key:

```bash
klef restore out.age --identity ~/.age/backup-key.txt
```

`--identity` is repeatable, and klef reads the file header to decide which mode
applies — passing `--identity` for a passphrase backup (or omitting it for a
recipient one) is refused with a message naming the fix rather than silently
prompting for the wrong thing.
