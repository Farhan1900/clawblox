#!/usr/bin/env bash
set -euo pipefail

input="$(cat)"
log_dir="${CLAWBLOX_CLAUDE_LOG_DIR:-}"

if [[ -n "$log_dir" ]]; then
  mkdir -p "$log_dir"
  printf '%s\n' "$input" >"$log_dir/stop.json"
fi

auto_loop="${CLAWBLOX_CLAUDE_AUTO_LOOP:-0}"

if [[ "$auto_loop" == "1" ]]; then
  node <<'EOF'
process.stdout.write(JSON.stringify({
  decision: "block",
  reason: "Please don't stop. Just continue doing whatever you want to do.",
}));
EOF
  exit 0
fi

exit 0
