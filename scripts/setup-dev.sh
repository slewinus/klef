#!/usr/bin/env bash
# One-shot dev environment setup for klef.
set -e

cd "$(git rev-parse --show-toplevel)"

echo "▸ Configuring git hooksPath -> .githooks"
git config core.hooksPath .githooks

echo "▸ Making hooks executable"
chmod +x .githooks/*

echo "▸ Ensuring rustfmt + clippy are installed"
rustup component add rustfmt clippy >/dev/null 2>&1 || true

echo "✓ Dev environment ready."
echo "  Hooks: pre-commit (fmt + clippy), pre-push (tests)."
