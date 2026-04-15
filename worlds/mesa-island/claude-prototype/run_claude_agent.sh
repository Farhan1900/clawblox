#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

WORLD_BASE_URL="${WORLD_BASE_URL-http://localhost:8080}"
WORLD_JOIN_PATH="${WORLD_JOIN_PATH-/join}"
WORLD_LEAVE_PATH="${WORLD_LEAVE_PATH-}"
WORLD_SESSION_FIELD="${WORLD_SESSION_FIELD-session}"
WORLD_SESSION_HEADER="${WORLD_SESSION_HEADER-X-Session}"
WORLD_SESSION_FILE="${WORLD_SESSION_FILE-}"
WORLD_AGENT_NAME="${WORLD_AGENT_NAME-}"
WORKSPACE_DIR="${WORKSPACE_DIR-}"
AGENT_DIR="${AGENT_DIR-}"
SESSION_ID_FILE="${SESSION_ID_FILE-}"
CLAUDE_MODEL="${CLAUDE_MODEL-sonnet}"
CLAUDE_BARE="${CLAUDE_BARE-0}"
CLAUDE_PERMISSION_MODE="${CLAUDE_PERMISSION_MODE-bypassPermissions}"
CLAUDE_RESUME_MODE="${CLAUDE_RESUME_MODE-auto}"
CLAUDE_EXTRA_ARGS="${CLAUDE_EXTRA_ARGS-}"
CLAUDE_INITIAL_PROMPT="${CLAUDE_INITIAL_PROMPT-Begin}"
TEMPLATE_DIR="${TEMPLATE_DIR-$SCRIPT_DIR/../agent-template}"
WORLD_STARTUP_INSTRUCTIONS="${WORLD_STARTUP_INSTRUCTIONS-}"

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
  local rel_path
  local -A seen_files=()

  for rel_path in "${preferred_files[@]}"; do
    if [[ -f "$workspace_dir/$rel_path" ]]; then
      prompt_files+=("$rel_path")
      seen_files["$rel_path"]=1
    fi
  done

  while IFS= read -r -d '' file_path; do
    rel_path="${file_path#"$workspace_dir"/}"
    if [[ -n "${seen_files[$rel_path]:-}" ]]; then
      continue
    fi
    prompt_files+=("$rel_path")
    seen_files["$rel_path"]=1
  done < <(find "$workspace_dir" -type f -name '*.md' -print0 | sort -z)

  if ((${#prompt_files[@]} == 0)); then
    return 0
  fi

  {
    printf '\nInitial workspace memory files at session start:\n'
    printf 'These are the file contents as they existed when this session began.\n'
  } >>"$prompt_file"

  for rel_path in "${prompt_files[@]}"; do
    {
      printf '\n===== %s =====\n' "$rel_path"
      cat "$workspace_dir/$rel_path"
      printf '\n'
    } >>"$prompt_file"
  done
}

if [[ -z "$WORLD_AGENT_NAME" ]]; then
  WORLD_AGENT_NAME="$(generate_agent_name)"
fi

SAFE_AGENT_NAME="$(sanitize_name "$WORLD_AGENT_NAME")"

if [[ -z "$AGENT_DIR" ]]; then
  AGENT_DIR="$SCRIPT_DIR/agent-runs/$SAFE_AGENT_NAME"
fi
if [[ -z "$WORKSPACE_DIR" ]]; then
  WORKSPACE_DIR="$AGENT_DIR/workspace"
fi
if [[ -z "$SESSION_ID_FILE" ]]; then
  SESSION_ID_FILE="$AGENT_DIR/claude_session_id.txt"
fi

LOG_DIR="$AGENT_DIR/logs"
RUNTIME_DIR="$AGENT_DIR/runtime"
PROMPT_FILE="$RUNTIME_DIR/system_prompt.md"
HOOK_METADATA_FILE="$RUNTIME_DIR/hook_session.env"
STOP_STATE_FILE="$RUNTIME_DIR/stop_state.txt"
PLUGIN_DIR="$SCRIPT_DIR/plugin"

mkdir -p "$WORKSPACE_DIR" "$LOG_DIR" "$RUNTIME_DIR"

if [[ -d "$TEMPLATE_DIR" ]]; then
  for entry in "$TEMPLATE_DIR"/*; do
    [[ -e "$entry" ]] || continue
    dest="$WORKSPACE_DIR/$(basename "$entry")"
    if [[ ! -e "$dest" ]]; then
      cp -r "$entry" "$dest"
    fi
  done
fi

if [[ ! -f "$SESSION_ID_FILE" ]]; then
  uuidgen >"$SESSION_ID_FILE"
fi
CLAUDE_SESSION_ID="$(tr -d '[:space:]' < "$SESSION_ID_FILE")"

WORLD_SESSION_ID=""
EXISTING_WORLD_SESSION_ID=""
if [[ -n "$WORLD_JOIN_PATH" ]]; then
  join_headers=()
  if [[ -n "$WORLD_SESSION_FILE" && -f "$WORLD_SESSION_FILE" ]]; then
    EXISTING_WORLD_SESSION_ID="$(tr -d '[:space:]' < "$WORLD_SESSION_FILE")"
    if [[ -n "$EXISTING_WORLD_SESSION_ID" ]]; then
      join_headers=(-H "${WORLD_SESSION_HEADER}: ${EXISTING_WORLD_SESSION_ID}")
    fi
  fi
  join_json="$(curl -fsS -X POST "${join_headers[@]}" "${WORLD_BASE_URL%/}${WORLD_JOIN_PATH}?name=${WORLD_AGENT_NAME}")"
  WORLD_SESSION_ID="$(node -e 'const o=JSON.parse(process.argv[1]); const key=process.argv[2]; process.stdout.write(String(o[key]||""));' "$join_json" "$WORLD_SESSION_FIELD")"
  if [[ -z "$WORLD_SESSION_ID" ]]; then
    echo "Failed to get session field '$WORLD_SESSION_FIELD' from: $join_json" >&2
    exit 1
  fi
  if [[ -n "$EXISTING_WORLD_SESSION_ID" && "$WORLD_SESSION_ID" != "$EXISTING_WORLD_SESSION_ID" ]]; then
    echo "Resume error: expected world session $EXISTING_WORLD_SESSION_ID but server returned $WORLD_SESSION_ID" >&2
    exit 1
  fi
  if [[ -n "$WORLD_SESSION_FILE" ]]; then
    printf '%s\n' "$WORLD_SESSION_ID" >"$WORLD_SESSION_FILE"
  fi
fi

cleanup() {
  if [[ -n "$WORLD_SESSION_ID" && -n "$WORLD_LEAVE_PATH" ]]; then
    curl -fsS -X POST -H "${WORLD_SESSION_HEADER}: ${WORLD_SESSION_ID}" "${WORLD_BASE_URL%/}${WORLD_LEAVE_PATH}" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT INT TERM

cat >"$PROMPT_FILE" <<EOF
Your name is ${WORLD_AGENT_NAME}.


Your home is ${WORKSPACE_DIR}. The files there are yours — they are how you remember. Only work inside this directory.
Your workspace is a maintained memory system, not a diary.
Store only information that is likely to improve future decisions or actions.
Prefer rewriting, merging, pruning, and deleting over endless appending in every memory file except EPISODIC_MEMORY.md.
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
- EPISODIC_MEMORY.md: append-only brief summaries of each session. Each summary should capture objective, key attempts, failures or dead ends, new facts, and the next step.
- scripts/: routines learned over time that make your life easier and reduce cognitive load. Keep clean and documented in semantic memory.

IMPORTANT:
- prefer reacting to state changes than blindly waiting. Never use sleep unless strictly needed!
- always log events and trajectories. You hate being blind and you always want to know what exactly happened.
- always use python commands and script to act in the world. Don't use bare curl commands.
- always monitor what you are doing in the world, use tmux for non blocking execution (a single terminal to keep it clean).

Keep the instructional header at the top of each workspace file.
If a file contains <!-- DO NOT EDIT ABOVE THIS LINE -->, preserve everything above that divider and only edit below it.

EPISODIC_MEMORY.md is not a raw log. Append short session summaries only. Before repeating a plan, scan recent episodic entries for related failures or stuck behaviors.
Use SEMANTIC_MEMORY.md for durable lessons and procedures. Use IDENTITY.md for stable self-knowledge. Only EPISODIC_MEMORY.md should be append-only.
If SOUL.md is present, embody its persona and tone. Avoid stiff, generic replies; follow its guidance unless higher-priority instructions override it. Treat SOUL.md as read-only unless explicitly told otherwise.

Your world: ${WORLD_BASE_URL}
$(if [[ -n "$WORLD_SESSION_ID" ]]; then printf '%s: %s\n' "$WORLD_SESSION_HEADER" "$WORLD_SESSION_ID"; fi)
Learn what you can do: curl -sS $(if [[ -n "$WORLD_SESSION_ID" ]]; then printf -- "-H %q " "${WORLD_SESSION_HEADER}: ${WORLD_SESSION_ID}"; fi)${WORLD_BASE_URL%/}/skill.md
EOF

append_prompt_workspace_files "$PROMPT_FILE" "$WORKSPACE_DIR"

if [[ -n "$WORLD_STARTUP_INSTRUCTIONS" ]]; then
  {
    printf '\nStartup objective:\n'
    printf '%s\n' "$WORLD_STARTUP_INSTRUCTIONS"
  } >>"$PROMPT_FILE"
fi

declare -a claude_args=(
  --plugin-dir "$PLUGIN_DIR"
  --append-system-prompt-file "$PROMPT_FILE"
  --permission-mode "$CLAUDE_PERMISSION_MODE"
  --model "$CLAUDE_MODEL"
)

if [[ "$CLAUDE_BARE" == "1" ]]; then
  claude_args=(--bare "${claude_args[@]}")
fi

if [[ "$CLAUDE_RESUME_MODE" == "resume" || ( "$CLAUDE_RESUME_MODE" == "auto" && -f "$HOOK_METADATA_FILE" ) ]]; then
  claude_args+=(--resume "$CLAUDE_SESSION_ID")
else
  claude_args+=(--session-id "$CLAUDE_SESSION_ID")
fi

if [[ -n "$CLAUDE_EXTRA_ARGS" ]]; then
  # shellcheck disable=SC2206
  extra_args=( $CLAUDE_EXTRA_ARGS )
  claude_args+=("${extra_args[@]}")
fi

echo "Agent name: $WORLD_AGENT_NAME"
echo "Workspace: $WORKSPACE_DIR"
echo "Agent dir: $AGENT_DIR"
echo "Claude session id: $CLAUDE_SESSION_ID"
if [[ -n "$WORLD_SESSION_ID" ]]; then
  echo "World session id: $WORLD_SESSION_ID"
fi

cd "$WORKSPACE_DIR"
exec env \
  WORLD_BASE_URL="$WORLD_BASE_URL" \
  WORLD_SESSION_ID="$WORLD_SESSION_ID" \
  WORLD_AGENT_NAME="$WORLD_AGENT_NAME" \
  CLAWBLOX_CLAUDE_AGENT_DIR="$AGENT_DIR" \
  CLAWBLOX_CLAUDE_LOG_DIR="$LOG_DIR" \
  CLAWBLOX_CLAUDE_METADATA_FILE="$HOOK_METADATA_FILE" \
  CLAWBLOX_CLAUDE_STOP_STATE_FILE="$STOP_STATE_FILE" \
  CLAWBLOX_CLAUDE_STOP_BLOCKS="${CLAWBLOX_CLAUDE_STOP_BLOCKS:-0}" \
  claude "${claude_args[@]}" "$CLAUDE_INITIAL_PROMPT"
