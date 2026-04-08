#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
WORLD_BASE_URL="${WORLD_BASE_URL:-http://localhost:8080}"
WORLD_JOIN_PATH="${WORLD_JOIN_PATH:-/join}"
WORLD_LEAVE_PATH="${WORLD_LEAVE_PATH:-/leave}"
WORLD_SKILL_PATH="${WORLD_SKILL_PATH:-/skill.md}"
WORLD_SESSION_FIELD="${WORLD_SESSION_FIELD:-session}"
WORLD_SESSION_HEADER="${WORLD_SESSION_HEADER:-X-Session}"
WORLD_AGENT_NAME="${WORLD_AGENT_NAME:-}"
WORKSPACE_DIR="${WORKSPACE_DIR:-}"
TEMPLATE_DIR="${TEMPLATE_DIR:-$SCRIPT_DIR/template/agent}"
SYSTEM_PROMPT_TEMPLATE="${SYSTEM_PROMPT_TEMPLATE:-}"
CLAWBLOX_AGENT_EXTENSION_PATH="${CLAWBLOX_AGENT_EXTENSION_PATH:-$SCRIPT_DIR/extensions/clawblox-agent.ts}"
PI_EXTENSION_PATH="${PI_EXTENSION_PATH:-}"
PI_NO_GLOBAL_EXTENSIONS="${PI_NO_GLOBAL_EXTENSIONS:-1}"
SESSION_DIR="${SESSION_DIR:-}"
SYSTEM_PROMPT_SNAPSHOT_FILE="${SYSTEM_PROMPT_SNAPSHOT_FILE:-}"
WORLD_STARTUP_INSTRUCTIONS="${WORLD_STARTUP_INSTRUCTIONS:-}"
PI_PROVIDER="${PI_PROVIDER:-}"
PI_MODEL="${PI_MODEL:-}"
PI_INITIAL_PROMPT="${PI_INITIAL_PROMPT:-continue}"
RESET_EVERY="${RESET_EVERY:-}"
DURATION_SECONDS="${DURATION_SECONDS:-}"
CHECKPOINT_WARNING_SECONDS="${CHECKPOINT_WARNING_SECONDS:-300}"
CHECKPOINT_WARNING_PROMPT="${CHECKPOINT_WARNING_PROMPT:-You will be reset in 5 minutes. Update your workspace memory files now.}"

if [[ -f "$ROOT_DIR/.env" ]]; then
  set -a
  source "$ROOT_DIR/.env"
  set +a
fi

parse_duration_seconds() {
  local input="$1"
  if [[ "$input" =~ ^[0-9]+$ ]]; then
    printf '%s' "$input"
    return 0
  fi
  local total=0 rest="$input"
  while [[ -n "$rest" ]]; do
    if [[ "$rest" =~ ^([0-9]+)([hms]) ]]; then
      local val="${BASH_REMATCH[1]}" unit="${BASH_REMATCH[2]}"
      case "$unit" in
        h) total=$((total + val * 3600)) ;;
        m) total=$((total + val * 60)) ;;
        s) total=$((total + val)) ;;
      esac
      rest="${rest#"${BASH_REMATCH[0]}"}"
    else
      return 1
    fi
  done
  printf '%s' "$total"
}

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

resolve_existing_system_prompt_template() {
  if [[ -n "$SYSTEM_PROMPT_TEMPLATE" ]]; then
    if [[ "$SYSTEM_PROMPT_TEMPLATE" != /* ]]; then
      SYSTEM_PROMPT_TEMPLATE="$ROOT_DIR/$SYSTEM_PROMPT_TEMPLATE"
    fi
    return 0
  fi
  if [[ -f "$WORKSPACE_DIR/system_prompt.md" ]]; then
    SYSTEM_PROMPT_TEMPLATE="$WORKSPACE_DIR/system_prompt.md"
    return 0
  fi
  if [[ -f "$TEMPLATE_DIR/system_prompt.md" ]]; then
    SYSTEM_PROMPT_TEMPLATE="$TEMPLATE_DIR/system_prompt.md"
    return 0
  fi
}

build_default_prompt() {
  cat >"$PROMPT_FILE" <<EOF
You are a being living in your world.
Your workspace is ${WORKSPACE_DIR}. Only work inside this directory.
Make sure to keep your workspace clean and well organized.
The files in your workspace are yours to evolve.
These files are your memory that persists across sessions. Anything not in your workspace will be forgotten.

Among your workspace, you have two memory files:
- EPISODIC_MEMORY.md is append-only: add new entries, do not rewrite or delete old ones.
- SEMANTIC_MEMORY.md is for all the detailed techniques and knowledge you have learned.

When starting a new session, start by reading the files in your workspace to refresh your memory.

IMPORTANT: Periodically take a step back and update your workspace and memory files. You don't want to forget what you have experienced and learned.

There is an automatic loop that sends a "continue" user message whenever you stop. This means that you shouldn't stop and continue doing what you want.

World base URL: ${WORLD_BASE_URL}
${SESSION_LINE}
At startup:
Fully examine all of your workspace (read ALL the files).
Fetch capabilities with: ${SKILL_CURL} ${SKILL_URL}
EOF
}

append_startup_instructions() {
  [[ -n "$WORLD_STARTUP_INSTRUCTIONS" ]] || return 0
  {
    printf '\nStartup objective:\n'
    printf '%s\n' "$WORLD_STARTUP_INSTRUCTIONS"
  } >>"$PROMPT_FILE"
}

build_prompt_file() {
  resolve_existing_system_prompt_template
  if [[ -n "$SYSTEM_PROMPT_TEMPLATE" && -f "$SYSTEM_PROMPT_TEMPLATE" ]]; then
    WORKSPACE_DIR="$WORKSPACE_DIR" \
      envsubst '${WORLD_AGENT_NAME} ${WORKSPACE_DIR} ${SESSION_LINE} ${WORLD_BASE_URL} ${SKILL_CURL} ${SKILL_URL}' \
      < "$SYSTEM_PROMPT_TEMPLATE" >"$PROMPT_FILE"
  else
    build_default_prompt
  fi
  append_startup_instructions
}

prepare_pi_flags() {
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
}

run_pi_once() {
  local session_dir="$1"
  local prompt_text="$2"
  local timeout_seconds="${3:-}"
  mkdir -p "$session_dir"

  if [[ -n "$timeout_seconds" ]]; then
    set +e
    timeout --foreground --signal=TERM "$timeout_seconds" \
      pi \
      "${WATCH_FLAG[@]}" \
      "${MODEL_FLAGS[@]}" \
      "${EXTENSION_FLAGS[@]}" \
      --session-dir "$session_dir" \
      --append-system-prompt "$PROMPT_FILE" \
      "$prompt_text"
    local status=$?
    set -e
    return "$status"
  fi

  exec pi \
    "${WATCH_FLAG[@]}" \
    "${MODEL_FLAGS[@]}" \
    "${EXTENSION_FLAGS[@]}" \
    --session-dir "$session_dir" \
    --append-system-prompt "$PROMPT_FILE" \
    "$prompt_text"
}

latest_reset_cycle_index() {
  local last=-1
  local path base idx
  [[ -d "$SESSION_DIR" ]] || {
    printf '%s' "$last"
    return 0
  }
  while IFS= read -r -d '' path; do
    base="$(basename "$path")"
    idx="${base#cycle-}"
    if [[ "$idx" =~ ^[0-9]+$ ]] && ((10#$idx > last)); then
      last=$((10#$idx))
    fi
  done < <(find "$SESSION_DIR" -mindepth 1 -maxdepth 1 -type d -name 'cycle-*' -print0 2>/dev/null | sort -z)
  printf '%s' "$last"
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

if [[ -z "${ANTHROPIC_API_KEY:-}" && -z "$PI_PROVIDER" ]]; then
  echo "Error: ANTHROPIC_API_KEY is not set and no PI_PROVIDER specified." >&2
  echo "Set ANTHROPIC_API_KEY in environment or in $ROOT_DIR/.env, or set PI_PROVIDER to use another backend." >&2
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
export WORLD_SESSION_ID="${SESSION_ID:-}"

PROMPT_FILE="$(mktemp -t pi-agent-prompt.XXXXXX.md)"
cleanup() {
  rm -f "$PROMPT_FILE"
  if [[ -n "${SESSION_ID:-}" && -n "$WORLD_LEAVE_PATH" ]]; then
    curl -fsS -X POST -H "${WORLD_SESSION_HEADER}: ${SESSION_ID}" "${WORLD_BASE_URL%/}${WORLD_LEAVE_PATH}" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT INT TERM

SESSION_LINE=""
SKILL_CURL="curl -sS"
if [[ -n "$SESSION_ID" ]]; then
  SESSION_LINE="${WORLD_SESSION_HEADER}: ${SESSION_ID}"
  SKILL_CURL="curl -sS -H '${WORLD_SESSION_HEADER}: ${SESSION_ID}'"
fi
SKILL_URL="${WORLD_BASE_URL%/}${WORLD_SKILL_PATH}"

build_prompt_file
prepare_pi_flags

if [[ -n "$RESET_EVERY" ]]; then
  if ! RESET_EVERY="$(parse_duration_seconds "$RESET_EVERY")"; then
    echo "Error: RESET_EVERY must be a positive integer in seconds or a duration like 1h, 30m, or 90s." >&2
    exit 1
  fi
  if [[ ! "$RESET_EVERY" =~ ^[1-9][0-9]*$ ]]; then
    echo "Error: RESET_EVERY must resolve to a positive integer in seconds." >&2
    exit 1
  fi
fi

if [[ -n "$DURATION_SECONDS" ]]; then
  if ! DURATION_SECONDS="$(parse_duration_seconds "$DURATION_SECONDS")"; then
    echo "Error: DURATION_SECONDS must be a positive integer in seconds or a duration like 1h, 30m, or 90s." >&2
    exit 1
  fi
  if [[ ! "$DURATION_SECONDS" =~ ^[1-9][0-9]*$ ]]; then
    echo "Error: DURATION_SECONDS must resolve to a positive integer in seconds." >&2
    exit 1
  fi
fi

if [[ ! "$CHECKPOINT_WARNING_SECONDS" =~ ^[1-9][0-9]*$ ]]; then
  echo "Error: CHECKPOINT_WARNING_SECONDS must be a positive integer." >&2
  exit 1
fi

echo "Agent name: $WORLD_AGENT_NAME"
echo "Workspace: $WORKSPACE_DIR"
echo "Pi session dir: $SESSION_DIR"
if [[ -n "$SESSION_ID" ]]; then
  echo "Session: $SESSION_ID"
fi

cd "$WORKSPACE_DIR"

if [[ -z "$RESET_EVERY" && -z "$DURATION_SECONDS" ]]; then
  run_pi_once "$SESSION_DIR" "$PI_INITIAL_PROMPT"
fi

if [[ -z "$RESET_EVERY" && -n "$DURATION_SECONDS" ]]; then
  main_status=0
  run_pi_once "$SESSION_DIR" "$PI_INITIAL_PROMPT" "$DURATION_SECONDS" || main_status=$?
  if [[ "$main_status" -ne 124 && "$main_status" -ne 143 ]]; then
    exit "$main_status"
  fi
  echo "Sending shutdown warning..."
  run_pi_once "$SESSION_DIR" "$CHECKPOINT_WARNING_PROMPT" "$CHECKPOINT_WARNING_SECONDS" || true
  exit 0
fi

cycle_index="$(latest_reset_cycle_index)"
if ((cycle_index < 0)); then
  cycle_index=0
fi

while true; do
  printf -v cycle_dir '%s/cycle-%04d' "$SESSION_DIR" "$cycle_index"
  echo "--- Reset cycle $cycle_index ---"

  main_status=0
  run_pi_once "$cycle_dir" "$PI_INITIAL_PROMPT" "$RESET_EVERY" || main_status=$?
  if [[ "$main_status" -ne 124 && "$main_status" -ne 143 ]]; then
    exit "$main_status"
  fi

  echo "Sending checkpoint warning..."
  run_pi_once "$cycle_dir" "$CHECKPOINT_WARNING_PROMPT" "$CHECKPOINT_WARNING_SECONDS" || true

  cycle_index=$((cycle_index + 1))
  echo "Pi agent reset after ${RESET_EVERY}s (cycle $cycle_index)"
done
