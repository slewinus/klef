#!/usr/bin/env bash
# Stop-hook helper: run `cargo check` and surface failures to the user.
# Silent on success.

cd "$(dirname "$0")/.." || exit 0

out=$(cargo check --quiet --all-targets 2>&1)
if [ $? -ne 0 ]; then
  jq -nR --arg msg "$out" '{systemMessage: ("klef: cargo check failed at end of turn:\n" + $msg)}'
fi
