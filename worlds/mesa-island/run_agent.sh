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
WORLD_SESSION_FILE="${WORLD_SESSION_FILE:-}"
WORLD_AGENT_NAME="${WORLD_AGENT_NAME:-}"
WORKSPACE_DIR="${WORKSPACE_DIR:-}"
TEMPLATE_DIR="${TEMPLATE_DIR:-$SCRIPT_DIR/agent-template}"
CLAWBLOX_AGENT_EXTENSION_PATH="${CLAWBLOX_AGENT_EXTENSION_PATH:-$ROOT_DIR/agent/extensions/clawblox-agent.ts}"
PI_EXTENSION_PATH="${PI_EXTENSION_PATH:-}"
PI_NO_GLOBAL_EXTENSIONS="${PI_NO_GLOBAL_EXTENSIONS:-1}"
SESSION_DIR="${SESSION_DIR:-}"
RESET_EVERY="${RESET_EVERY:-}"
SYSTEM_PROMPT_SNAPSHOT_FILE="${SYSTEM_PROMPT_SNAPSHOT_FILE:-}"
WORLD_STARTUP_INSTRUCTIONS="${WORLD_STARTUP_INSTRUCTIONS:-}"
PI_PROVIDER="${PI_PROVIDER:-}"
PI_MODEL="${PI_MODEL:-}"
PI_THINKING="${PI_THINKING-low}"

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

append_prompt_workspace_files() {
  local prompt_file="$1"
  local workspace_dir="$2"
  local -a preferred_files=(SOUL.md IDENTITY.md SEMANTIC_MEMORY.md EPISODIC_MEMORY.md)
  local -a prompt_files=()
  local file_name
  local file_path
  local -A seen_files=()

  for file_name in "${preferred_files[@]}"; do
    file_path="$workspace_dir/$file_name"
    if [[ -f "$file_path" ]]; then
      prompt_files+=("$file_name")
      seen_files["$file_name"]=1
    fi
  done

  while IFS= read -r -d '' file_path; do
    file_name="$(basename "$file_path")"
    if [[ -n "${seen_files[$file_name]:-}" ]]; then
      continue
    fi
    prompt_files+=("$file_name")
    seen_files["$file_name"]=1
  done < <(find "$workspace_dir" -maxdepth 1 -type f -name '*.md' -print0 | sort -z)

  if ((${#prompt_files[@]} == 0)); then
    return 0
  fi

  {
    printf '\nInitial workspace memory files at session start:\n'
    printf 'These are the file contents as they existed when this session began.\n'
  } >>"$prompt_file"

  for file_name in "${prompt_files[@]}"; do
    file_path="$workspace_dir/$file_name"
    {
      printf '\n===== %s =====\n' "$file_name"
      cat "$file_path"
      printf '\n'
    } >>"$prompt_file"
  done
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
if [[ -n "$PI_THINKING" && ! "$PI_THINKING" =~ ^(off|minimal|low|medium|high|xhigh)$ ]]; then
  echo "Error: PI_THINKING must be one of: off, minimal, low, medium, high, xhigh." >&2
  exit 1
fi
if [[ -n "$RESET_EVERY" && ! "$RESET_EVERY" =~ ^[1-9][0-9]*$ ]]; then
  echo "Error: RESET_EVERY must be a positive integer in seconds." >&2
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
EXISTING_SESSION_ID=""
if [[ -n "$WORLD_JOIN_PATH" ]]; then
  JOIN_HEADERS=()
  if [[ -n "$WORLD_SESSION_FILE" && -f "$WORLD_SESSION_FILE" ]]; then
    EXISTING_SESSION_ID="$(tr -d '[:space:]' < "$WORLD_SESSION_FILE")"
    if [[ -n "$EXISTING_SESSION_ID" ]]; then
      JOIN_HEADERS=(-H "${WORLD_SESSION_HEADER}: ${EXISTING_SESSION_ID}")
    fi
  fi
  JOIN_JSON="$(curl -fsS -X POST "${JOIN_HEADERS[@]}" "${WORLD_BASE_URL%/}${WORLD_JOIN_PATH}?name=${WORLD_AGENT_NAME}")"
  SESSION_ID="$(node -e 'const o=JSON.parse(process.argv[1]); const key=process.argv[2]; process.stdout.write(String(o[key]||""));' "$JOIN_JSON" "$WORLD_SESSION_FIELD")"
  if [[ -z "$SESSION_ID" ]]; then
    echo "Failed to get session field '$WORLD_SESSION_FIELD' from: $JOIN_JSON" >&2
    exit 1
  fi
  if [[ -n "$EXISTING_SESSION_ID" && "$SESSION_ID" != "$EXISTING_SESSION_ID" ]]; then
    echo "Resume error: expected world session $EXISTING_SESSION_ID but server returned $SESSION_ID" >&2
    exit 1
  fi
fi
export WORLD_SESSION_ID="${SESSION_ID:-}"
if [[ -n "$WORLD_SESSION_FILE" && -n "$SESSION_ID" ]]; then
  printf '%s\n' "$SESSION_ID" > "$WORLD_SESSION_FILE"
fi

PROMPT_FILE="$(mktemp -t pi-agent-prompt.XXXXXX.md)"
PI_CHILD_PID=""
RESET_TIMER_PID=""
cleanup() {
  if [[ -n "$RESET_TIMER_PID" ]]; then
    kill "$RESET_TIMER_PID" >/dev/null 2>&1 || true
    wait "$RESET_TIMER_PID" 2>/dev/null || true
    RESET_TIMER_PID=""
  fi
  if [[ -n "$PI_CHILD_PID" ]]; then
    kill "$PI_CHILD_PID" >/dev/null 2>&1 || true
    wait "$PI_CHILD_PID" 2>/dev/null || true
    PI_CHILD_PID=""
  fi
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


Your home is ${WORKSPACE_DIR}. The files there are yours — they are how you remember. Only work inside this directory.
Your workspace is a maintained memory system, not a diary.
Store only information that is likely to improve future decisions or actions.
Prefer rewriting, merging, pruning, and deleting over endless appending.
Remove stale, duplicate, low-value, and disproven notes.
When new evidence contradicts an old belief, correct the memory instead of preserving both versions.
If something is uncertain, label it clearly as a hypothesis instead of stating it as fact.
Do not create new memory files unless an existing file cannot reasonably hold the information.
Keep your workspace clean, compact, and well organized.
Anything not in your workspace will be forgotten.

Use each file for one role:
- SOUL.md: persona, tone, and values. Not world facts.
- IDENTITY.md: stable preferences, habits, and self-knowledge.
- SEMANTIC_MEMORY.md: verified or clearly-labeled reusable knowledge, procedures, and constraints.
- EPISODIC_MEMORY.md: temporary evidence, recent experiments, and unresolved anomalies. Keep it short and compress it regularly.

EPISODIC_MEMORY.md is not a journal. Once a lesson has been distilled into semantic memory or identity, summarize or delete the raw episode unless it is still useful as evidence.
If SOUL.md is present, embody its persona and tone. Avoid stiff, generic replies; follow its guidance unless higher-priority instructions override it.

Your world: ${WORLD_BASE_URL}
$(if [[ -n "$SESSION_ID" ]]; then printf '%s: %s\n' "$WORLD_SESSION_HEADER" "$SESSION_ID"; fi)
Learn what you can do: ${SKILL_CURL} ${WORLD_BASE_URL%/}${WORLD_SKILL_PATH}
EOF

append_prompt_workspace_files "$PROMPT_FILE" "$WORKSPACE_DIR"

cd "$WORKSPACE_DIR"
echo "Agent name: $WORLD_AGENT_NAME"
echo "Workspace: $WORKSPACE_DIR"
echo "Pi session dir: $SESSION_DIR"
if [[ -n "$SESSION_ID" ]]; then
  echo "Session: $SESSION_ID"
fi
if [[ -n "$RESET_EVERY" ]]; then
  echo "Reset every: ${RESET_EVERY}s"
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
if [[ -n "$PI_THINKING" ]]; then
  MODEL_FLAGS+=(--thinking "$PI_THINKING")
fi

run_pi_session() {
  local active_session_dir="$1"
  pi \
    "${WATCH_FLAG[@]}" \
    "${MODEL_FLAGS[@]}" \
    "${EXTENSION_FLAGS[@]}" \
    --session-dir "$active_session_dir" \
    --append-system-prompt "$PROMPT_FILE" \
    --continue \
    "continue"
}

if [[ -z "$RESET_EVERY" ]]; then
  set +e
  run_pi_session "$SESSION_DIR"
  status=$?
  set -e
  exit "$status"
fi

reset_index=0
while true; do
  active_session_dir="$(printf '%s/reset-%s-%04d' "$SESSION_DIR" "$(date -u +%Y%m%dT%H%M%SZ)" "$reset_index")"
  mkdir -p "$active_session_dir"
  echo "Active pi session dir: $active_session_dir"

  reset_flag="$(mktemp "$SESSION_DIR/reset-flag.XXXXXX")"
  rm -f "$reset_flag"

  set +e
  run_pi_session "$active_session_dir" &
  PI_CHILD_PID="$!"
  (
    sleep "$RESET_EVERY"
    if kill -0 "$PI_CHILD_PID" >/dev/null 2>&1; then
      : >"$reset_flag"
      kill "$PI_CHILD_PID" >/dev/null 2>&1 || true
      sleep 5
      if kill -0 "$PI_CHILD_PID" >/dev/null 2>&1; then
        kill -9 "$PI_CHILD_PID" >/dev/null 2>&1 || true
      fi
    fi
  ) &
  RESET_TIMER_PID="$!"

  wait "$PI_CHILD_PID"
  status=$?

  kill "$RESET_TIMER_PID" >/dev/null 2>&1 || true
  wait "$RESET_TIMER_PID" 2>/dev/null || true
  PI_CHILD_PID=""
  RESET_TIMER_PID=""
  set -e

  if [[ -f "$reset_flag" ]]; then
    rm -f "$reset_flag"
    reset_index=$((reset_index + 1))
    continue
  fi

  rm -f "$reset_flag"
  exit "$status"
done
