#!/usr/bin/env bash
set -euo pipefail

input="$(cat)"
log_dir="${CLAWBLOX_CLAUDE_LOG_DIR:-}"

if [[ -n "$log_dir" ]]; then
  mkdir -p "$log_dir"
  printf '%s\n' "$input" >"$log_dir/stop.json"
fi

max_blocks="${CLAWBLOX_CLAUDE_STOP_BLOCKS:-0}"
state_file="${CLAWBLOX_CLAUDE_STOP_STATE_FILE:-}"

if [[ "$max_blocks" =~ ^[0-9]+$ ]] && (( max_blocks > 0 )) && [[ -n "$state_file" ]]; then
  mkdir -p "$(dirname "$state_file")"
  blocks_used=0
  if [[ -f "$state_file" ]]; then
    blocks_used="$(cat "$state_file" 2>/dev/null || printf '0')"
  fi
  if [[ "$blocks_used" =~ ^[0-9]+$ ]] && (( blocks_used < max_blocks )); then
    printf '%s\n' "$((blocks_used + 1))" >"$state_file"
    INPUT_REASON="Prototype stop hook requested another turn (${blocks_used}/${max_blocks} used)." \
      node <<'EOF'
const payload = {
  decision: "block",
  reason: process.env.INPUT_REASON ?? "Prototype stop hook requested another turn.",
};
process.stdout.write(JSON.stringify(payload));
EOF
    exit 0
  fi
fi

exit 0
