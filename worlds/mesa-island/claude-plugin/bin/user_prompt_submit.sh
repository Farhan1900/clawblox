#!/usr/bin/env bash
set -euo pipefail

input="$(cat)"
log_dir="${CLAWBLOX_CLAUDE_LOG_DIR:-}"

if [[ -n "$log_dir" ]]; then
  mkdir -p "$log_dir"
  printf '%s\n' "$input" >"$log_dir/user_prompt_submit.json"
fi

exit 0
