#!/usr/bin/env bash
# Enforce a 300-line cap for non-documentation files.
set -euo pipefail

max_lines=300
violations=()

while IFS= read -r path; do
    case "$path" in
        *.md|*.toml|*.lock|*.txt|*.json|*.yaml|*.yml)
            continue
            ;;
        *.png|*.jpg|*.jpeg|*.gif|*.icns|*.ico|*.svg|*.webp)
            continue
            ;;
    esac

    if [[ ! -f "$path" ]]; then
        continue
    fi

    line_count=$(wc -l < "$path")
    if (( line_count > max_lines )); then
        violations+=("$path:$line_count")
    fi
done < <(rg --files -g '!target')

if (( ${#violations[@]} > 0 )); then
    printf 'Files over %s lines:
' "$max_lines" >&2
    printf '  %s
' "${violations[@]}" >&2
    exit 1
fi

echo "✓ line-count policy OK"
