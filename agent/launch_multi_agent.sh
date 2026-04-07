#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BACKEND="${AGENT_BACKEND:-claude}"
declare -a PASSTHROUGH_ARGS=()

usage() {
  cat <<EOF
Usage: $(basename "$0") [--backend claude|pi] [launcher args...]

Dispatch to the backend-specific multi-agent launcher.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --backend)
      if [[ $# -lt 2 || -z "${2:-}" ]]; then
        echo "Error: --backend requires a value." >&2
        usage
        exit 1
      fi
      BACKEND="$2"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      PASSTHROUGH_ARGS+=("$1")
      shift
      ;;
  esac
done

case "$BACKEND" in
  claude) TARGET_SCRIPT="$SCRIPT_DIR/launch_multi_claude.sh" ;;
  pi) TARGET_SCRIPT="$SCRIPT_DIR/launch_multi_pi.sh" ;;
  *)
    echo "Error: unsupported backend '$BACKEND'. Expected 'claude' or 'pi'." >&2
    exit 1
    ;;
esac

if [[ ! -f "$TARGET_SCRIPT" ]]; then
  echo "Error: launcher not found at $TARGET_SCRIPT" >&2
  exit 1
fi

exec bash "$TARGET_SCRIPT" "${PASSTHROUGH_ARGS[@]}"
