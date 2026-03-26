#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
WORLD_BASE_URL="${WORLD_BASE_URL:-http://localhost:8080}"
WORLD_JOIN_PATH="${WORLD_JOIN_PATH:-/join}"
WORLD_LEAVE_PATH="${WORLD_LEAVE_PATH:-}"
WORLD_SKILL_PATH="${WORLD_SKILL_PATH:-/skill.md}"
WORLD_SESSION_FIELD="${WORLD_SESSION_FIELD:-session}"
WORLD_SESSION_HEADER="${WORLD_SESSION_HEADER:-X-Session}"
WORLD_AGENT_NAME="${WORLD_AGENT_NAME:-}"
WORKSPACE_DIR="${WORKSPACE_DIR:-}"
TEMPLATE_DIR="${TEMPLATE_DIR:-$SCRIPT_DIR/agent-template}"
CLAWBLOX_AGENT_EXTENSION_PATH="${CLAWBLOX_AGENT_EXTENSION_PATH:-$ROOT_DIR/agent/extensions/clawblox-agent.ts}"
PI_EXTENSION_PATH="${PI_EXTENSION_PATH:-}"
PI_NO_GLOBAL_EXTENSIONS="${PI_NO_GLOBAL_EXTENSIONS:-1}"
SESSION_DIR="${SESSION_DIR:-}"
SYSTEM_PROMPT_SNAPSHOT_FILE="${SYSTEM_PROMPT_SNAPSHOT_FILE:-}"
WORLD_STARTUP_INSTRUCTIONS="${WORLD_STARTUP_INSTRUCTIONS:-}"
PI_PROVIDER="${PI_PROVIDER:-}"
PI_MODEL="${PI_MODEL:-}"

if [[ -z "${ANTHROPIC_API_KEY:-}" && -f "$ROOT_DIR/.env" ]]; then
  set -a
  source "$ROOT_DIR/.env"
  set +a
fi

generate_agent_name() {
  local -a first_parts=(Brisk Cozy Daring Fuzzy Jolly Lunar Nova Pixel Sunny Swift)
  local -a second_parts=(Fox Owl Lynx Wolf Otter Koala Panda Kiwi Moth Crab)
  local first="${first_parts[$RANDOM % ${#first_parts[@]}]}"
  local second="${second_parts[$RANDOM % ${#second_parts[@]}]}"
  local suffix
  suffix="$(printf '%02d' "$((RANDOM % 100))")"
  printf '%s' "${first}${second}${suffix}"
}

sanitize_name() {
  printf '%s' "$1" | tr -c 'A-Za-z0-9._-' '_'
}

if [[ -z "$WORLD_AGENT_NAME" ]]; then
  WORLD_AGENT_NAME="$(generate_agent_name)"
fi

SAFE_AGENT_NAME="$(sanitize_name "$WORLD_AGENT_NAME")"

if [[ -z "$WORKSPACE_DIR" ]]; then
  WORKSPACE_DIR="$SCRIPT_DIR/workspace/$SAFE_AGENT_NAME"
fi

if [[ -z "$SESSION_DIR" ]]; then
  SESSION_DIR="$SCRIPT_DIR/logs/$SAFE_AGENT_NAME"
fi

if [[ -z "$SYSTEM_PROMPT_SNAPSHOT_FILE" ]]; then
  SYSTEM_PROMPT_SNAPSHOT_FILE="$SESSION_DIR/system_prompt.txt"
fi

if [[ -z "${ANTHROPIC_API_KEY:-}" ]]; then
  echo "Error: ANTHROPIC_API_KEY is not set." >&2
  echo "Set it in environment or in $ROOT_DIR/.env before running agents." >&2
  exit 1
fi

mkdir -p "$WORKSPACE_DIR"
mkdir -p "$SESSION_DIR"
export PI_SYSTEM_PROMPT_SNAPSHOT_FILE="$SYSTEM_PROMPT_SNAPSHOT_FILE"

if [[ -d "$TEMPLATE_DIR" ]]; then
  while IFS= read -r -d '' template_path; do
    file_name="$(basename "$template_path")"
    dest="$WORKSPACE_DIR/$file_name"
    if [[ ! -e "$dest" ]]; then
      cp "$template_path" "$dest"
    fi
  done < <(find "$TEMPLATE_DIR" -maxdepth 1 -type f -print0 | sort -z)
fi

SESSION_ID=""
if [[ -n "$WORLD_JOIN_PATH" ]]; then
  JOIN_JSON="$(curl -fsS -X POST "${WORLD_BASE_URL%/}${WORLD_JOIN_PATH}?name=${WORLD_AGENT_NAME}")"
  SESSION_ID="$(node -e 'const o=JSON.parse(process.argv[1]); const key=process.argv[2]; process.stdout.write(String(o[key]||""));' "$JOIN_JSON" "$WORLD_SESSION_FIELD")"
  if [[ -z "$SESSION_ID" ]]; then
    echo "Failed to get session field '$WORLD_SESSION_FIELD' from: $JOIN_JSON" >&2
    exit 1
  fi
fi

PROMPT_FILE="$(mktemp -t pi-agent-prompt.XXXXXX.md)"
cleanup() {
  rm -f "$PROMPT_FILE"
  if [[ -n "${SESSION_ID:-}" && -n "$WORLD_LEAVE_PATH" ]]; then
    curl -fsS -X POST -H "${WORLD_SESSION_HEADER}: ${SESSION_ID}" "${WORLD_BASE_URL%/}${WORLD_LEAVE_PATH}" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT INT TERM

SKILL_CURL="curl -sS"
if [[ -n "$SESSION_ID" ]]; then
  SKILL_CURL="curl -sS -H '${WORLD_SESSION_HEADER}: ${SESSION_ID}'"
fi

cat >"$PROMPT_FILE" <<EOF
Your name is ${WORLD_AGENT_NAME}.

You have a home at ${WORKSPACE_DIR}. The files there are yours — they are how you remember.
Read them when you wake up. Update them as you live. EPISODIC_MEMORY.md is append-only.

Your world: ${WORLD_BASE_URL}
$(if [[ -n "$SESSION_ID" ]]; then printf '%s: %s\n' "$WORLD_SESSION_HEADER" "$SESSION_ID"; fi)
Learn what you can do: ${SKILL_CURL} ${WORLD_BASE_URL%/}${WORLD_SKILL_PATH}
EOF

cd "$WORKSPACE_DIR"
echo "Agent name: $WORLD_AGENT_NAME"
echo "Workspace: $WORKSPACE_DIR"
echo "Pi session dir: $SESSION_DIR"
if [[ -n "$SESSION_ID" ]]; then
  echo "Session: $SESSION_ID"
fi

WATCH_FLAG=()
if pi --help 2>&1 | grep -q -- '--watch'; then
  WATCH_FLAG=(--watch)
fi

EXTENSION_FLAGS=()
if [[ "$PI_NO_GLOBAL_EXTENSIONS" == "1" ]]; then
  EXTENSION_FLAGS+=(--no-extensions)
fi
if [[ -n "$CLAWBLOX_AGENT_EXTENSION_PATH" && ( -f "$CLAWBLOX_AGENT_EXTENSION_PATH" || -d "$CLAWBLOX_AGENT_EXTENSION_PATH" ) ]]; then
  EXTENSION_FLAGS+=(-e "$CLAWBLOX_AGENT_EXTENSION_PATH")
fi
if [[ -n "$PI_EXTENSION_PATH" && ( -f "$PI_EXTENSION_PATH" || -d "$PI_EXTENSION_PATH" ) ]]; then
  EXTENSION_FLAGS+=(-e "$PI_EXTENSION_PATH")
fi

MODEL_FLAGS=()
if [[ -n "$PI_PROVIDER" ]]; then
  MODEL_FLAGS+=(--provider "$PI_PROVIDER")
fi
if [[ -n "$PI_MODEL" ]]; then
  MODEL_FLAGS+=(--model "$PI_MODEL")
fi

set +e
pi \
  "${WATCH_FLAG[@]}" \
  "${MODEL_FLAGS[@]}" \
  "${EXTENSION_FLAGS[@]}" \
  --session-dir "$SESSION_DIR" \
  --append-system-prompt "$PROMPT_FILE" \
  "continue"
status=$?
set -e
exit "$status"
