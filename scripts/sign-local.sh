#!/usr/bin/env bash
# Codesign a locally-built klef binary with a stable identity, so macOS stops
# asking for your login password on every single keychain access.
#
# Why this is needed
# ------------------
# `cargo build` produces an ad-hoc ("linker-signed") binary whose code identity
# is derived from its own bytes:
#
#     $ codesign -dv target/release/klef
#     Identifier=klef-2ad4d48fc8eac87c
#     TeamIdentifier=not set
#
# A macOS keychain item's ACL remembers *which program* was granted access, as a
# designated requirement. An ad-hoc identity gives it nothing durable to hold on
# to, so "Always Allow" never sticks and every rebuild looks like a brand-new,
# untrusted program. Hence the password prompt on every `klef get`.
#
# Signing with a real certificate produces a requirement that is independent of
# the binary's bytes:
#
#     designated => identifier "com.slewinus.klef" and anchor apple generic
#                   and certificate leaf[subject.CN] = "Apple Development: ..."
#
# Click "Always Allow" once and it holds across rebuilds.
#
# What this does NOT do
# ---------------------
# This is a local development convenience. It does not make the binary
# distributable: an "Apple Development" certificate isn't a "Developer ID
# Application" one, so binaries signed this way won't validate on anyone else's
# machine. Shipping signed builds to users needs the paid Apple Developer
# Program — tracked in https://github.com/slewinus/klef/issues/123.
#
# Usage:
#   scripts/sign-local.sh [path-to-binary]     # default: target/release/klef

set -euo pipefail

BIN="${1:-target/release/klef}"
BUNDLE_ID="com.slewinus.klef"

if [ ! -f "$BIN" ]; then
    echo "error: no binary at $BIN — run 'cargo build --release -p klef' first" >&2
    exit 1
fi

if [ "$(uname -s)" != "Darwin" ]; then
    echo "error: macOS only; keychain ACLs don't apply elsewhere" >&2
    exit 1
fi

# Prefer a distribution certificate when one exists, otherwise any development
# one. `find-identity -v` lists only valid, non-expired identities.
identities="$(security find-identity -v -p codesigning 2>/dev/null || true)"
# `|| true` on each pipeline: grep exits 1 when a certificate class is absent,
# and under `set -e` + `pipefail` that would abort before the fallback runs.
pick() { printf '%s\n' "$identities" | grep -o "\"$1: [^\"]*\"" | head -1 | tr -d '"' || true; }
identity="$(pick 'Developer ID Application')"
[ -n "$identity" ] || identity="$(pick 'Apple Development')"
[ -n "$identity" ] || identity="$(pick 'klef-dev')"

if [ -z "$identity" ]; then
    cat >&2 <<'EOS'
error: no code signing identity found in your keychain.

Create a free local one in Keychain Access:
  Keychain Access → Certificate Assistant → Create a Certificate…
  Name: klef-dev   Identity Type: Self Signed Root   Type: Code Signing

Then re-run this script. Any stable identity works — the point is that it
doesn't change when the binary does.
EOS
    exit 1
fi

echo "▸ signing $BIN"
echo "  identity: $identity"
codesign --force --sign "$identity" --identifier "$BUNDLE_ID" "$BIN"
codesign --verify --verbose=1 "$BIN"

echo
echo "  designated requirement now:"
codesign -d -r- "$BIN" 2>&1 | sed -n 's/^designated => /    /p'
cat <<'EOS'

Next keychain access will prompt once more — click "Always Allow". It should
stay quiet after that, including across rebuilds.
EOS
