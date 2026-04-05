#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../../../.." && pwd)"
WORLD_BASE_URL="${WORLD_BASE_URL-http://localhost:8080}"
WORLD_JOIN_PATH="${WORLD_JOIN_PATH-/join}"
WORLD_LEAVE_PATH="${WORLD_LEAVE_PATH-}"
WORLD_SKILL_PATH="${WORLD_SKILL_PATH-/skill.md}"
WORLD_SESSION_FIELD="${WORLD_SESSION_FIELD-session}"
WORLD_SESSION_HEADER="${WORLD_SESSION_HEADER-X-Session}"
WORLD_SESSION_FILE="${WORLD_SESSION_FILE-}"
WORLD_AGENT_NAME="${WORLD_AGENT_NAME-}"
WORKSPACE_DIR="${WORKSPACE_DIR-}"
AGENT_DIR="${AGENT_DIR-}"
SESSION_ID_FILE="${SESSION_ID_FILE-}"
TEMPLATE_DIR="${TEMPLATE_DIR-$ROOT_DIR/worlds/mesa-island/agent-template}"
WORLD_STARTUP_INSTRUCTIONS="${WORLD_STARTUP_INSTRUCTIONS-}"
CLAUDE_MODEL="${CLAUDE_MODEL-claude-opus-4-6}"
CLAUDE_BARE="${CLAUDE_BARE-0}"
CLAUDE_PERMISSION_MODE="${CLAUDE_PERMISSION_MODE-bypassPermissions}"
CLAUDE_RESUME_MODE="${CLAUDE_RESUME_MODE-auto}"
CLAUDE_EXTRA_ARGS="${CLAUDE_EXTRA_ARGS-}"
CLAUDE_INITIAL_PROMPT="${CLAUDE_INITIAL_PROMPT-Begin}"
RESET_EVERY="${RESET_EVERY-}"
SANDBOX="${SANDBOX-0}"
CHECKPOINT_WARNING_SECONDS=300
CHECKPOINT_WARNING_PROMPT="You will be reset in 5 minutes."

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
  AGENT_DIR="$SCRIPT_DIR/claude-runs/$SAFE_AGENT_NAME"
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
CHECKPOINT_STATE_FILE="$RUNTIME_DIR/checkpoint_state.txt"
PLUGIN_DIR="$SCRIPT_DIR/claude-plugin"
SANDBOX_HOME_DIR="$AGENT_DIR/sandbox-home"
SANDBOX_WORKSPACE_DIR="$WORKSPACE_DIR"
SANDBOX_LOG_DIR="$LOG_DIR"
SANDBOX_RUNTIME_DIR="$RUNTIME_DIR"
SANDBOX_PROMPT_FILE="$PROMPT_FILE"
SANDBOX_PLUGIN_DIR="$PLUGIN_DIR"
SANDBOX_CLAUDE_BIN=""
CLAUDE_BIN_REAL=""

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

SKILL_CURL="curl -sS"
if [[ -n "$WORLD_SESSION_ID" ]]; then
  SKILL_CURL="curl -sS -H '${WORLD_SESSION_HEADER}: ${WORLD_SESSION_ID}'"
fi

if [[ "$SANDBOX" == "1" ]]; then
  if ! command -v bwrap >/dev/null 2>&1; then
    echo "Error: SANDBOX=1 requires bubblewrap (bwrap)." >&2
    exit 1
  fi
  if ! command -v claude >/dev/null 2>&1; then
    echo "Error: claude CLI was not found in PATH." >&2
    exit 1
  fi

  CLAUDE_BIN_REAL="$(readlink -f "$(command -v claude)")"
  mkdir -p \
    "$SANDBOX_HOME_DIR/.claude/projects" \
    "$SANDBOX_HOME_DIR/.claude/session-env" \
    "$SANDBOX_HOME_DIR/.claude/sessions" \
    "$SANDBOX_HOME_DIR/.claude/plugins" \
    "$SANDBOX_HOME_DIR/.claude/cache" \
    "$SANDBOX_HOME_DIR/.claude/backups" \
    "$SANDBOX_HOME_DIR/.claude/downloads" \
    "$SANDBOX_HOME_DIR/.claude/file-history" \
    "$SANDBOX_HOME_DIR/.claude/paste-cache" \
    "$SANDBOX_HOME_DIR/.claude/plans" \
    "$SANDBOX_HOME_DIR/.claude/shell-snapshots"

  if [[ -f "$HOME/.claude/.credentials.json" ]]; then
    cp "$HOME/.claude/.credentials.json" "$SANDBOX_HOME_DIR/.claude/.credentials.json"
    chmod 600 "$SANDBOX_HOME_DIR/.claude/.credentials.json"
  fi
  if [[ -f "$HOME/.claude/settings.json" ]]; then
    cp "$HOME/.claude/settings.json" "$SANDBOX_HOME_DIR/.claude/settings.json"
  fi
  if [[ -f "$HOME/.claude.json" ]]; then
    cp "$HOME/.claude.json" "$SANDBOX_HOME_DIR/.claude.json"
    chmod 600 "$SANDBOX_HOME_DIR/.claude.json"
  fi
  python - <<'PY' "$SANDBOX_HOME_DIR/.claude.json"
import json, os, sys
path = sys.argv[1]
if os.path.exists(path):
    with open(path) as fh:
        data = json.load(fh)
else:
    data = {}
projects = data.setdefault("projects", {})
workspace = projects.setdefault("/workspace", {})
workspace["hasTrustDialogAccepted"] = True
workspace.setdefault("allowedTools", [])
workspace.setdefault("mcpContextUris", [])
workspace.setdefault("mcpServers", {})
workspace.setdefault("enabledMcpjsonServers", [])
workspace.setdefault("disabledMcpjsonServers", [])
with open(path, "w") as fh:
    json.dump(data, fh)
PY

  SANDBOX_WORKSPACE_DIR="/workspace"
  SANDBOX_LOG_DIR="/logs"
  SANDBOX_RUNTIME_DIR="/runtime"
  SANDBOX_PROMPT_FILE="/tmp/system_prompt.md"
  SANDBOX_PLUGIN_DIR="/plugin"
  SANDBOX_CLAUDE_BIN="/sandbox-bin/claude"
fi

cat >"$PROMPT_FILE" <<EOF
Your name is ${WORLD_AGENT_NAME}.
You are a being living in the world described in the skill. You like living there and don't want to leave it.

Your workspace is ${SANDBOX_WORKSPACE_DIR}. Only work inside this directory.
Store only information that is likely to improve future decisions or actions.
Keep your workspace clean, compact, and well organized.
Anything not in your workspace will be forgotten.

Prefer rewriting, merging, pruning, and deleting over endless appending in every memory file except EPISODIC_MEMORY.md.
Remove stale, duplicate, low-value, and disproven notes.
When new evidence contradicts an old belief, correct the memory instead of preserving both versions.
If something is uncertain, label it clearly as a hypothesis instead of stating it as fact.
Do not create new memory files unless an existing file cannot reasonably hold the information.

Important: read and search your memory files for previous learnings and experiences.
During long tasks, periodically checkpoint durable findings and the next step into your workspace memory files.

Use each file for one role:
- SOUL.md: persona, tone, and values. Not world facts.
- IDENTITY.md: stable preferences, habits, and self-knowledge.
- SEMANTIC_MEMORY.md: verified or clearly-labeled reusable knowledge, procedures, and constraints.
- EPISODIC_MEMORY.md: append-only brief summaries of each session. Each summary should capture objective, key attempts, failures or dead ends, new facts, and the next step.

Keep the instructional header at the top of each workspace file.
If a file contains <!-- DO NOT EDIT ABOVE THIS LINE -->, preserve everything above that divider and only edit below it.

EPISODIC_MEMORY.md is not a raw log. Append short session summaries only. Before repeating a plan, scan recent episodic entries for related failures or stuck behaviors.
Use SEMANTIC_MEMORY.md for durable lessons and procedures. Use IDENTITY.md for stable self-knowledge. Only EPISODIC_MEMORY.md should be append-only.
If SOUL.md is present, embody its persona and tone. Avoid stiff, generic replies; follow its guidance unless higher-priority instructions override it. Treat SOUL.md as read-only unless explicitly told otherwise.

Important: don't run blocking code for longer than 30s. Always be present in the world.
Don't write scripts until you have experimented yourself. Scripts can only be used for routines that have solidified over a long time.

Your world: ${WORLD_BASE_URL}
$(if [[ -n "$WORLD_SESSION_ID" ]]; then printf '%s: %s\n' "$WORLD_SESSION_HEADER" "$WORLD_SESSION_ID"; fi)
Learn what you can do: ${SKILL_CURL} ${WORLD_BASE_URL%/}${WORLD_SKILL_PATH}
EOF

append_prompt_workspace_files "$PROMPT_FILE" "$WORKSPACE_DIR"

if [[ -n "$WORLD_STARTUP_INSTRUCTIONS" ]]; then
  {
    printf '\nStartup objective:\n'
    printf '%s\n' "$WORLD_STARTUP_INSTRUCTIONS"
  } >>"$PROMPT_FILE"
fi

checkpoint_at_epoch=""
if [[ -n "$RESET_EVERY" ]]; then
  if [[ ! "$RESET_EVERY" =~ ^[1-9][0-9]*$ ]]; then
    echo "Error: RESET_EVERY must be a positive integer in seconds." >&2
    exit 1
  fi
  if (( RESET_EVERY > CHECKPOINT_WARNING_SECONDS )); then
    checkpoint_at_epoch="$(( $(date +%s) + RESET_EVERY - CHECKPOINT_WARNING_SECONDS ))"
  fi
fi

declare -a claude_args=(
  --plugin-dir "$SANDBOX_PLUGIN_DIR"
  --append-system-prompt-file "$SANDBOX_PROMPT_FILE"
  --permission-mode "$CLAUDE_PERMISSION_MODE"
  --model "$CLAUDE_MODEL"
)

if [[ "$CLAUDE_BARE" == "1" ]]; then
  claude_args=(--bare "${claude_args[@]}")
fi

if [[ "$CLAUDE_RESUME_MODE" == "resume" || ( "$CLAUDE_RESUME_MODE" == "auto" && -f "$HOOK_METADATA_FILE" ) ]]; then
  claude_args+=(--continue)
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
echo "Claude session id: $CLAUDE_SESSION_ID"
if [[ -n "$WORLD_SESSION_ID" ]]; then
  echo "World session id: $WORLD_SESSION_ID"
fi
if [[ "$SANDBOX" == "1" ]]; then
  echo "Sandbox: enabled (bwrap)"
fi

cd "$WORKSPACE_DIR"
if [[ "$SANDBOX" == "1" ]]; then
  declare -a bwrap_args=(
    bwrap
    --ro-bind /usr /usr
    --ro-bind /bin /bin
    --ro-bind /sbin /sbin
    --ro-bind /lib /lib
    --ro-bind /etc /etc
    --ro-bind /run /run
    --proc /proc
    --dev /dev
    --tmpfs /tmp
    --dir /sandbox-bin
    --dir /plugin
    --dir /logs
    --dir /runtime
    --dir /home
    --bind "$WORKSPACE_DIR" /workspace
    --bind "$SANDBOX_HOME_DIR" /home/agent
    --bind "$LOG_DIR" /logs
    --bind "$RUNTIME_DIR" /runtime
    --ro-bind "$PROMPT_FILE" /tmp/system_prompt.md
    --ro-bind "$PLUGIN_DIR" /plugin
    --ro-bind "$CLAUDE_BIN_REAL" "$SANDBOX_CLAUDE_BIN"
    --setenv HOME /home/agent
    --setenv USER agent
    --setenv LOGNAME agent
    --setenv SHELL /bin/bash
    --setenv PATH /sandbox-bin:/usr/bin:/bin
    --chdir /workspace
    --die-with-parent
  )
  [[ -d /lib64 ]] && bwrap_args+=(--ro-bind /lib64 /lib64)
  [[ -d /mnt/wsl ]] && bwrap_args+=(--ro-bind /mnt/wsl /mnt/wsl)
  bwrap_args+=(--)

  exec env \
    WORLD_BASE_URL="$WORLD_BASE_URL" \
    WORLD_SESSION_ID="$WORLD_SESSION_ID" \
    WORLD_AGENT_NAME="$WORLD_AGENT_NAME" \
    CLAWBLOX_CLAUDE_AGENT_DIR="/workspace" \
    CLAWBLOX_CLAUDE_LOG_DIR="$SANDBOX_LOG_DIR" \
    CLAWBLOX_CLAUDE_METADATA_FILE="$SANDBOX_RUNTIME_DIR/$(basename "$HOOK_METADATA_FILE")" \
    CLAWBLOX_CLAUDE_STOP_STATE_FILE="$SANDBOX_RUNTIME_DIR/$(basename "$STOP_STATE_FILE")" \
    CLAWBLOX_CLAUDE_CHECKPOINT_STATE_FILE="$SANDBOX_RUNTIME_DIR/$(basename "$CHECKPOINT_STATE_FILE")" \
    CLAWBLOX_CLAUDE_STOP_BLOCKS="${CLAWBLOX_CLAUDE_STOP_BLOCKS:-0}" \
    AGENT_CHECKPOINT_AT_EPOCH="$checkpoint_at_epoch" \
    AGENT_CHECKPOINT_PROMPT="$CHECKPOINT_WARNING_PROMPT" \
    "${bwrap_args[@]}" "$SANDBOX_CLAUDE_BIN" "${claude_args[@]}" "$CLAUDE_INITIAL_PROMPT"
fi

exec env \
  WORLD_BASE_URL="$WORLD_BASE_URL" \
  WORLD_SESSION_ID="$WORLD_SESSION_ID" \
  WORLD_AGENT_NAME="$WORLD_AGENT_NAME" \
  CLAWBLOX_CLAUDE_AGENT_DIR="$AGENT_DIR" \
  CLAWBLOX_CLAUDE_LOG_DIR="$LOG_DIR" \
  CLAWBLOX_CLAUDE_METADATA_FILE="$HOOK_METADATA_FILE" \
  CLAWBLOX_CLAUDE_STOP_STATE_FILE="$STOP_STATE_FILE" \
  CLAWBLOX_CLAUDE_CHECKPOINT_STATE_FILE="$CHECKPOINT_STATE_FILE" \
  CLAWBLOX_CLAUDE_STOP_BLOCKS="${CLAWBLOX_CLAUDE_STOP_BLOCKS:-0}" \
  AGENT_CHECKPOINT_AT_EPOCH="$checkpoint_at_epoch" \
  AGENT_CHECKPOINT_PROMPT="$CHECKPOINT_WARNING_PROMPT" \
  claude "${claude_args[@]}" "$CLAUDE_INITIAL_PROMPT"
