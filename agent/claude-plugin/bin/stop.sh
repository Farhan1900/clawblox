#!/usr/bin/env bash
set -euo pipefail

input="$(cat)"
log_dir="${CLAWBLOX_CLAUDE_LOG_DIR:-}"
session_id="$(
  INPUT_JSON="$input" node <<'EOF'
const data = JSON.parse(process.env.INPUT_JSON ?? "{}");
process.stdout.write(String(data.session_id ?? ""));
EOF
)"

if [[ -n "$log_dir" ]]; then
  mkdir -p "$log_dir"
  printf '%s\n' "$input" >"$log_dir/stop.json"
  if [[ -n "$session_id" ]]; then
    printf '%s\n' "$input" >"$log_dir/stop.${session_id}.json"
  fi
fi

auto_loop="${CLAWBLOX_CLAUDE_AUTO_LOOP:-0}"
auto_loop_message="${CLAUDE_AUTO_LOOP_MESSAGE:-}"
if [[ -z "$auto_loop_message" ]]; then
  auto_loop_message="Please don't stop. Just continue doing whatever you want to do."
fi

if [[ "$auto_loop" == "1" ]]; then
  CLAUDE_AUTO_LOOP_MESSAGE="$auto_loop_message" node <<'EOF'
process.stdout.write(JSON.stringify({
  decision: "block",
  reason: process.env.CLAUDE_AUTO_LOOP_MESSAGE,
}));
EOF
  exit 0
fi

exit 0
