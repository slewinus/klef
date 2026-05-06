#!/usr/bin/env bash
# Generate a populated klef.rb from homebrew/klef.rb after a release is published.
# Usage:
#   scripts/update-homebrew-formula.sh vX.Y.Z [output-path]
#
# - Downloads the four release tarballs from
#   https://github.com/slewinus/klef/releases/download/<TAG>/...
# - Computes their SHA-256.
# - Substitutes __VERSION__ and __SHA_*__ placeholders in homebrew/klef.rb.
# - Writes the result to the output path (default: /tmp/klef.rb).

set -euo pipefail

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
    echo "usage: $0 vX.Y.Z [output-path]" >&2
    exit 64
fi

TAG="$1"
VERSION="${TAG#v}"
OUTPUT_PATH="${2:-/tmp/klef.rb}"
TEMPLATE="$(dirname "$0")/../homebrew/klef.rb"
BASE_URL="https://github.com/slewinus/klef/releases/download/${TAG}"

if [ ! -f "$TEMPLATE" ]; then
    echo "error: template not found at $TEMPLATE" >&2
    exit 1
fi

WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

# List of targets: "triple PLACEHOLDER_SUFFIX"
TARGETS="
aarch64-apple-darwin AARCH64_APPLE_DARWIN
x86_64-apple-darwin X86_64_APPLE_DARWIN
aarch64-unknown-linux-gnu AARCH64_UNKNOWN_LINUX_GNU
x86_64-unknown-linux-gnu X86_64_UNKNOWN_LINUX_GNU
"

content="$(cat "$TEMPLATE")"
content="${content//__VERSION__/$VERSION}"

while IFS=' ' read -r target placeholder; do
    [ -z "$target" ] && continue
    tarball="klef-${TAG}-${target}.tar.gz"
    url="${BASE_URL}/${tarball}"
    echo "▸ downloading $url" >&2
    curl --fail --silent --show-error --location -o "${WORKDIR}/${tarball}" "$url" || {
        echo "error: failed to download $tarball — has the release been published?" >&2
        exit 1
    }
    sha="$(shasum -a 256 "${WORKDIR}/${tarball}" | awk '{print $1}')"
    echo "  sha256: $sha" >&2
    content="${content//__SHA_${placeholder}__/$sha}"
done <<< "$TARGETS"

# Sanity check: no placeholders should remain.
if echo "$content" | grep -q '__'; then
    echo "error: placeholders remain in output:" >&2
    echo "$content" | grep '__' >&2
    exit 1
fi

echo "$content" > "$OUTPUT_PATH"
echo "✓ wrote populated formula to $OUTPUT_PATH" >&2
