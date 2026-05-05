#!/usr/bin/env bash
# PostToolUse hook: warn Claude when a Write/Edit grows a non-doc file
# past the 300-line cap (which pre-commit will hard-reject).

LIMIT=300
SKIP_PATTERN='\.(md|toml|txt|lock|json|yaml|yml)$|^Cargo\.lock$'

input=$(cat)
file=$(echo "$input" | jq -r '.tool_input.file_path // .tool_response.filePath // empty')

[ -z "$file" ] && exit 0
[ -f "$file" ] || exit 0
echo "$file" | grep -Eq "$SKIP_PATTERN" && exit 0

lines=$(wc -l < "$file" | tr -d ' ')
if [ "$lines" -gt "$LIMIT" ]; then
  jq -n \
    --arg f "$file" \
    --arg n "$lines" \
    --arg lim "$LIMIT" \
    '{systemMessage: ("klef line-cap: \($f) is now \($n) lines (limit \($lim)). Split it before committing or pre-commit will reject the commit.")}'
fi
