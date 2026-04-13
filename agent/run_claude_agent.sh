#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Prevent stray API keys from triggering Claude's key detection prompt
unset ANTHROPIC_API_KEY 2>/dev/null || true
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
TEMPLATE_DIR="${TEMPLATE_DIR:-$SCRIPT_DIR/template/agent}"
WORLD_STARTUP_INSTRUCTIONS="${WORLD_STARTUP_INSTRUCTIONS-}"
CLAUDE_MODEL="${CLAUDE_MODEL-claude-opus-4-6}"
CLAUDE_BARE="${CLAUDE_BARE-0}"
CLAUDE_PERMISSION_MODE="${CLAUDE_PERMISSION_MODE-bypassPermissions}"
CLAUDE_RESUME_MODE="${CLAUDE_RESUME_MODE-auto}"
CLAUDE_EXTRA_ARGS="${CLAUDE_EXTRA_ARGS-}"
CLAUDE_INITIAL_PROMPT="${CLAUDE_INITIAL_PROMPT-Begin}"
RESET_EVERY="${RESET_EVERY-}"
SANDBOX="${SANDBOX-0}"
CHECKPOINT_WARNING_SECONDS="${CHECKPOINT_WARNING_SECONDS:-300}"
CHECKPOINT_WARNING_PROMPT="${CHECKPOINT_WARNING_PROMPT:-You will be reset in 5 minutes. Update your workspace memory files now.}"
CLAUDE_FAILURE_IDLE_GRACE_SECONDS="${CLAUDE_FAILURE_IDLE_GRACE_SECONDS:-90}"
CLAUDE_FAILURE_STALL_GRACE_SECONDS="${CLAUDE_FAILURE_STALL_GRACE_SECONDS:-180}"
CLAUDE_FAILURE_RESTART_BACKOFF_SECONDS="${CLAUDE_FAILURE_RESTART_BACKOFF_SECONDS:-5}"

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

generate_session_id() {
  if command -v uuidgen >/dev/null 2>&1; then
    uuidgen
    return 0
  fi

  if [[ -r /proc/sys/kernel/random/uuid ]]; then
    cat /proc/sys/kernel/random/uuid
    return 0
  fi

  if command -v python3 >/dev/null 2>&1; then
    python3 -c 'import uuid; print(uuid.uuid4())'
    return 0
  fi

  echo "Error: could not generate session id; install uuidgen or python3." >&2
  exit 1
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
PLUGIN_DIR="${PLUGIN_DIR-$SCRIPT_DIR/claude-plugin}"
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
  generate_session_id >"$SESSION_ID_FILE"
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
  # Persist the most recent runtime session metadata for later debugging/snapshots.
  if [[ -f "$HOOK_METADATA_FILE" ]]; then
    sync_latest_session_artifacts
  fi

  if [[ -n "$WORLD_SESSION_ID" && -n "$WORLD_LEAVE_PATH" ]]; then
    curl -fsS -X POST -H "${WORLD_SESSION_HEADER}: ${WORLD_SESSION_ID}" "${WORLD_BASE_URL%/}${WORLD_LEAVE_PATH}" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT INT TERM

sync_latest_session_artifacts() {
  if [[ ! -f "$HOOK_METADATA_FILE" ]]; then
    return 0
  fi

  local SESSION_ID="" TRANSCRIPT_PATH=""
  local src=""

  # shellcheck disable=SC1090
  source "$HOOK_METADATA_FILE"

  if [[ -n "$SESSION_ID" ]]; then
    printf '%s\n' "$SESSION_ID" >"$SESSION_ID_FILE"
  fi

  src="$TRANSCRIPT_PATH"
  if [[ -n "$src" && "$SANDBOX" == "1" ]]; then
    src="${SANDBOX_HOME_DIR}${src#/home/agent}"
  fi
  if [[ -n "$src" && -f "$src" ]]; then
    mkdir -p "$AGENT_DIR/session"
    cp "$src" "$AGENT_DIR/session/" 2>/dev/null || true
  fi
}

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

  if [[ -n "${ANTHROPIC_OAUTH_TOKEN:-}" ]]; then
    # Use the provided OAuth token instead of host credentials
    printf '{"claudeAiOauth":{"accessToken":"%s"}}' "$ANTHROPIC_OAUTH_TOKEN" \
      > "$SANDBOX_HOME_DIR/.claude/.credentials.json"
    chmod 600 "$SANDBOX_HOME_DIR/.claude/.credentials.json"
  elif [[ -f "$HOME/.claude/.credentials.json" ]]; then
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

SESSION_LINE=""
if [[ -n "$WORLD_SESSION_ID" ]]; then
  SESSION_LINE="${WORLD_SESSION_HEADER}: ${WORLD_SESSION_ID}"
fi
SKILL_URL="${WORLD_BASE_URL%/}${WORLD_SKILL_PATH}"
PROMPT_WORKSPACE_DIR="${SANDBOX_WORKSPACE_DIR:-$WORKSPACE_DIR}"

export WORLD_AGENT_NAME SESSION_LINE WORLD_BASE_URL SKILL_CURL SKILL_URL

SYSTEM_PROMPT_TEMPLATE="${SYSTEM_PROMPT_TEMPLATE-$SCRIPT_DIR/system_prompt.md}"
if [[ -f "$SYSTEM_PROMPT_TEMPLATE" ]]; then
  WORKSPACE_DIR="$PROMPT_WORKSPACE_DIR" \
    envsubst '${WORLD_AGENT_NAME} ${WORKSPACE_DIR} ${SESSION_LINE} ${WORLD_BASE_URL} ${SKILL_CURL} ${SKILL_URL}' \
    < "$SYSTEM_PROMPT_TEMPLATE" > "$PROMPT_FILE"
else
  echo "Error: system prompt template not found at $SYSTEM_PROMPT_TEMPLATE" >&2
  exit 1
fi

append_prompt_workspace_files "$PROMPT_FILE" "$WORKSPACE_DIR"

if [[ -n "$WORLD_STARTUP_INSTRUCTIONS" ]]; then
  {
    printf '\nStartup objective:\n'
    printf '%s\n' "$WORLD_STARTUP_INSTRUCTIONS"
  } >>"$PROMPT_FILE"
fi

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

# Build base claude args (session-specific args added per invocation)
declare -a claude_base_args=(
  --plugin-dir "$SANDBOX_PLUGIN_DIR"
  --append-system-prompt-file "$SANDBOX_PROMPT_FILE"
  --permission-mode "$CLAUDE_PERMISSION_MODE"
  --model "$CLAUDE_MODEL"
)
if [[ "$CLAUDE_BARE" == "1" ]]; then
  claude_base_args=(--bare "${claude_base_args[@]}")
fi
if [[ -n "$CLAUDE_EXTRA_ARGS" ]]; then
  # shellcheck disable=SC2206
  claude_base_args+=( $CLAUDE_EXTRA_ARGS )
fi

# Build env vars for claude process
declare -a claude_env_args=()
if [[ "$SANDBOX" == "1" ]]; then
  claude_env_args=(
    WORLD_BASE_URL="$WORLD_BASE_URL"
    WORLD_SESSION_ID="$WORLD_SESSION_ID"
    WORLD_AGENT_NAME="$WORLD_AGENT_NAME"
    CLAWBLOX_CLAUDE_AGENT_DIR="/workspace"
    CLAWBLOX_CLAUDE_LOG_DIR="$SANDBOX_LOG_DIR"
    CLAWBLOX_CLAUDE_METADATA_FILE="$SANDBOX_RUNTIME_DIR/$(basename "$HOOK_METADATA_FILE")"
    CLAWBLOX_CLAUDE_AUTO_LOOP="${CLAWBLOX_CLAUDE_AUTO_LOOP:-1}"
  )
else
  claude_env_args=(
    WORLD_BASE_URL="$WORLD_BASE_URL"
    WORLD_SESSION_ID="$WORLD_SESSION_ID"
    WORLD_AGENT_NAME="$WORLD_AGENT_NAME"
    CLAWBLOX_CLAUDE_AGENT_DIR="$AGENT_DIR"
    CLAWBLOX_CLAUDE_LOG_DIR="$LOG_DIR"
    CLAWBLOX_CLAUDE_METADATA_FILE="$HOOK_METADATA_FILE"
    CLAWBLOX_CLAUDE_AUTO_LOOP="${CLAWBLOX_CLAUDE_AUTO_LOOP:-1}"
  )
fi

# Build bwrap prefix for sandbox mode
declare -a bwrap_prefix=()
CLAUDE_CMD="claude"
if [[ "$SANDBOX" == "1" ]]; then
  bwrap_prefix=(
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
  [[ -d /lib64 ]] && bwrap_prefix+=(--ro-bind /lib64 /lib64)
  # Mount only resolv.conf for DNS — do NOT mount all of /mnt/wsl, which
  # exposes the host filesystem via Docker bind mounts.
  [[ -f /mnt/wsl/resolv.conf ]] && bwrap_prefix+=(--ro-bind /mnt/wsl/resolv.conf /mnt/wsl/resolv.conf)
  bwrap_prefix+=(--)
  CLAUDE_CMD="$SANDBOX_CLAUDE_BIN"
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

RECOVERY_LOG_FILE="$RUNTIME_DIR/recovery.log"
RECOVERY_REQUEST_FILE="$RUNTIME_DIR/recovery_request.env"

utc_now() {
  date -u +%Y-%m-%dT%H:%M:%SZ
}

log_recovery_event() {
  local event="$1"
  local details="${2:-}"
  printf '%s event=%s pid=%s details=%q\n' \
    "$(utc_now)" \
    "$event" \
    "$$" \
    "$details" >>"$RECOVERY_LOG_FILE"
}

map_runtime_path() {
  local src="${1:-}"
  if [[ -z "$src" ]]; then
    return 0
  fi
  if [[ "$SANDBOX" == "1" && "$src" == /home/agent/* ]]; then
    printf '%s\n' "${SANDBOX_HOME_DIR}${src#/home/agent}"
    return 0
  fi
  printf '%s\n' "$src"
}

file_mtime_epoch() {
  local path="$1"
  if [[ ! -e "$path" ]]; then
    printf '0\n'
    return 0
  fi
  stat -c %Y "$path" 2>/dev/null || printf '0\n'
}

current_runtime_session_id() {
  if [[ -f "$SESSION_ID_FILE" ]]; then
    tr -d '[:space:]' < "$SESSION_ID_FILE"
    return 0
  fi
  printf '%s' "$CLAUDE_SESSION_ID"
}

resolve_session_transcript_path() {
  local session_id="$1"
  local session_start_file="$LOG_DIR/session_start.${session_id}.json"
  local src=""

  if [[ -f "$session_start_file" ]]; then
    src="$(
      INPUT_JSON="$(cat "$session_start_file")" node <<'EOF'
const data = JSON.parse(process.env.INPUT_JSON ?? "{}");
process.stdout.write(String(data.transcript_path ?? ""));
EOF
    )"
  elif [[ -f "$HOOK_METADATA_FILE" ]]; then
    local SESSION_ID="" TRANSCRIPT_PATH=""
    # shellcheck disable=SC1090
    source "$HOOK_METADATA_FILE"
    if [[ "${SESSION_ID:-}" == "$session_id" ]]; then
      src="${TRANSCRIPT_PATH:-}"
    fi
  fi

  map_runtime_path "$src"
}

write_recovery_request() {
  local invocation_id="$1"
  local session_id="$2"
  local reason="$3"
  local error_type="$4"
  local trigger_file="$5"
  local tmp_file="$RECOVERY_REQUEST_FILE.tmp"

  cat >"$tmp_file" <<EOF
INVOCATION_ID=$(printf '%q' "$invocation_id")
SESSION_ID=$(printf '%q' "$session_id")
REASON=$(printf '%q' "$reason")
ERROR_TYPE=$(printf '%q' "$error_type")
TRIGGER_FILE=$(printf '%q' "$trigger_file")
RECORDED_AT=$(printf '%q' "$(utc_now)")
EOF
  mv "$tmp_file" "$RECOVERY_REQUEST_FILE"
}

stop_failure_fields_for_session() {
  local session_id="$1"
  local stop_failure_file="$LOG_DIR/stop_failure.${session_id}.json"
  if [[ ! -f "$stop_failure_file" ]]; then
    return 1
  fi

  INPUT_JSON="$(cat "$stop_failure_file")" node <<'EOF'
const data = JSON.parse(process.env.INPUT_JSON ?? "{}");
const fields = [
  data.error ?? "",
  data.transcript_path ?? "",
];
for (const value of fields) process.stdout.write(String(value) + "\n");
EOF
}

fresh_stop_failure_for_session() {
  local session_id="$1"
  local started_epoch="$2"
  local stop_failure_file="$LOG_DIR/stop_failure.${session_id}.json"
  local seen_epoch

  if [[ ! -f "$stop_failure_file" ]]; then
    return 1
  fi
  seen_epoch="$(file_mtime_epoch "$stop_failure_file")"
  (( seen_epoch >= started_epoch ))
}

start_failure_watchdog() {
  local runner_pid="$1"
  local invocation_id="$2"
  local session_id="$3"
  local started_epoch="$4"

  (
    local stop_failure_file="$LOG_DIR/stop_failure.${session_id}.json"
    local idle_prompt_file="$LOG_DIR/notification.idle_prompt.${session_id}.json"
    local transcript_path=""
    local self_pid="$BASHPID"
    local timeout_pid=""
    local stop_failure_epoch=0
    local idle_prompt_epoch=0
    local last_progress_epoch=0
    local failure_error=""
    local progress_epoch=0
    local transcript_epoch=0
    local agent_log_epoch=0
    local now=0
    local -a target_pids=()
    local pid=""

    child_pids() {
      local parent_pid="$1"
      ps -o pid= --ppid "$parent_pid" 2>/dev/null | awk '{print $1}' | while IFS= read -r child_pid; do
        [[ -n "$child_pid" ]] || continue
        [[ "$child_pid" == "$self_pid" ]] && continue
        printf '%s\n' "$child_pid"
      done
    }

    collect_descendants() {
      local parent_pid="$1"
      local child_pid=""
      while IFS= read -r child_pid; do
        [[ -n "$child_pid" ]] || continue
        collect_descendants "$child_pid"
        printf '%s\n' "$child_pid"
      done < <(child_pids "$parent_pid")
    }

    find_timeout_child() {
      local parent_pid="$1"
      local latest=""
      while IFS= read -r latest; do
        :
      done < <(child_pids "$parent_pid")
      printf '%s\n' "$latest"
    }

    terminate_active_invocation() {
      local parent_pid="$1"
      local root_pid="$2"
      local -a descendants=()
      local target_pid=""
      local remaining_pid=""

      if [[ -z "$root_pid" ]]; then
        return 1
      fi

      mapfile -t descendants < <(collect_descendants "$root_pid")
      for target_pid in "${descendants[@]}"; do
        kill -TERM "$target_pid" 2>/dev/null || true
      done
      kill -TERM "$root_pid" 2>/dev/null || true
      sleep 5
      for target_pid in "${descendants[@]}"; do
        if kill -0 "$target_pid" 2>/dev/null; then
          kill -KILL "$target_pid" 2>/dev/null || true
        fi
      done
      if kill -0 "$root_pid" 2>/dev/null; then
        kill -KILL "$root_pid" 2>/dev/null || true
      fi

      remaining_pid="$(find_timeout_child "$parent_pid")"
      [[ "$remaining_pid" != "$root_pid" ]]
    }

    while kill -0 "$runner_pid" 2>/dev/null; do
      timeout_pid="$(find_timeout_child "$runner_pid")"
      if [[ -z "$timeout_pid" ]]; then
        sleep 1
        continue
      fi

      if (( stop_failure_epoch == 0 )) && [[ -f "$stop_failure_file" ]]; then
        stop_failure_epoch="$(file_mtime_epoch "$stop_failure_file")"
        if (( stop_failure_epoch < started_epoch )); then
          stop_failure_epoch=0
        else
          mapfile -t failure_fields < <(stop_failure_fields_for_session "$session_id" || true)
          failure_error="${failure_fields[0]:-unknown}"
          transcript_path="$(map_runtime_path "${failure_fields[1]:-}")"
          if [[ -z "$transcript_path" ]]; then
            transcript_path="$(resolve_session_transcript_path "$session_id")"
          fi
          log_recovery_event "stop_failure_observed" "session_id=$session_id error=$failure_error stop_failure_epoch=$stop_failure_epoch transcript_path=$transcript_path"
        fi
      fi

      if (( stop_failure_epoch > 0 )); then
        if (( idle_prompt_epoch == 0 )) && [[ -f "$idle_prompt_file" ]]; then
          idle_prompt_epoch="$(file_mtime_epoch "$idle_prompt_file")"
          if (( idle_prompt_epoch < stop_failure_epoch )); then
            idle_prompt_epoch=0
          else
            log_recovery_event "idle_prompt_observed" "session_id=$session_id idle_prompt_epoch=$idle_prompt_epoch"
          fi
        fi

        progress_epoch=0
        if [[ -n "$transcript_path" && -f "$transcript_path" ]]; then
          transcript_epoch="$(file_mtime_epoch "$transcript_path")"
          (( transcript_epoch > progress_epoch )) && progress_epoch="$transcript_epoch"
        fi
        agent_log_epoch="$(file_mtime_epoch "$LOG_DIR/agent.log")"
        (( agent_log_epoch > progress_epoch )) && progress_epoch="$agent_log_epoch"
        (( progress_epoch > last_progress_epoch )) && last_progress_epoch="$progress_epoch"

        now="$(date +%s)"
        if (( idle_prompt_epoch > 0 && now - idle_prompt_epoch >= CLAUDE_FAILURE_IDLE_GRACE_SECONDS )); then
          write_recovery_request "$invocation_id" "$session_id" "stop_failure_idle_prompt" "$failure_error" "$idle_prompt_file"
          log_recovery_event "recovery_requested" "invocation_id=$invocation_id session_id=$session_id reason=stop_failure_idle_prompt error=$failure_error runner_pid=$runner_pid timeout_pid=$timeout_pid"
          terminate_active_invocation "$runner_pid" "$timeout_pid" || true
          break
        fi

        if (( now - stop_failure_epoch >= CLAUDE_FAILURE_STALL_GRACE_SECONDS && last_progress_epoch <= stop_failure_epoch )); then
          write_recovery_request "$invocation_id" "$session_id" "stop_failure_stalled" "$failure_error" "$stop_failure_file"
          log_recovery_event "recovery_requested" "invocation_id=$invocation_id session_id=$session_id reason=stop_failure_stalled error=$failure_error runner_pid=$runner_pid timeout_pid=$timeout_pid"
          terminate_active_invocation "$runner_pid" "$timeout_pid" || true
          break
        fi
      fi

      sleep 2
    done
  ) &
  printf '%s\n' "$!"
}

run_claude_until_deadline() {
  local deadline_epoch="$1"
  local session_mode="$2"
  local session_id="$3"
  local prompt_text="$4"
  local label="$5"
  local restart_count=0

  while true; do
    local now_epoch remaining_seconds invocation_id invocation_started_epoch watchdog_pid
    local -a session_args=()
    local status=0
    local backoff_seconds=0
    local recovery_request_present=0

    now_epoch="$(date +%s)"
    remaining_seconds=$((deadline_epoch - now_epoch))
    if (( remaining_seconds <= 0 )); then
      printf '124\n'
      return 0
    fi

    rm -f "$RECOVERY_REQUEST_FILE"
    if [[ "$session_mode" == "continue" ]]; then
      session_args=(--continue)
      session_id="$(current_runtime_session_id)"
    else
      session_args=(--session-id "$session_id")
    fi

    invocation_id="$(generate_session_id)"
    invocation_started_epoch="$(date +%s)"
    log_recovery_event "claude_window_start" "label=$label invocation_id=$invocation_id session_mode=$session_mode session_id=$session_id remaining_seconds=$remaining_seconds"

    watchdog_pid="$(start_failure_watchdog "$$" "$invocation_id" "$session_id" "$invocation_started_epoch")"
    set +e
    timeout --foreground --signal=TERM "$remaining_seconds" \
      env "${claude_env_args[@]}" \
      "${bwrap_prefix[@]}" "$CLAUDE_CMD" "${claude_base_args[@]}" "${session_args[@]}" "$prompt_text"
    status=$?
    set -e

    if [[ -n "${watchdog_pid:-}" ]]; then
      kill "$watchdog_pid" 2>/dev/null || true
      wait "$watchdog_pid" 2>/dev/null || true
    fi

    sync_latest_session_artifacts
    if [[ -f "$RECOVERY_REQUEST_FILE" ]]; then
      local INVOCATION_ID="" SESSION_ID="" REASON="" ERROR_TYPE="" TRIGGER_FILE=""
      # shellcheck disable=SC1090
      source "$RECOVERY_REQUEST_FILE"
      if [[ "${INVOCATION_ID:-}" == "$invocation_id" ]]; then
        recovery_request_present=1
        log_recovery_event "claude_window_recover" "label=$label invocation_id=$invocation_id session_id=${SESSION_ID:-$session_id} reason=${REASON:-unknown} error=${ERROR_TYPE:-unknown} status=$status"
      fi
    fi

    if (( recovery_request_present == 1 )); then
      restart_count=$((restart_count + 1))
      session_mode="continue"
      session_id="$(current_runtime_session_id)"
      backoff_seconds=$((CLAUDE_FAILURE_RESTART_BACKOFF_SECONDS * restart_count))
      (( backoff_seconds > 60 )) && backoff_seconds=60
      sleep "$backoff_seconds"
      continue
    fi

    if fresh_stop_failure_for_session "$session_id" "$invocation_started_epoch"; then
      restart_count=$((restart_count + 1))
      session_mode="continue"
      session_id="$(current_runtime_session_id)"
      backoff_seconds=$((CLAUDE_FAILURE_RESTART_BACKOFF_SECONDS * restart_count))
      (( backoff_seconds > 60 )) && backoff_seconds=60
      log_recovery_event "claude_window_restart_after_exit" "label=$label invocation_id=$invocation_id session_id=$session_id status=$status backoff_seconds=$backoff_seconds"
      sleep "$backoff_seconds"
      continue
    fi

    printf '%s\n' "$status"
    return 0
  done
}

# --- No reset, no duration: single session, exec into claude ---
if [[ -z "$RESET_EVERY" && -z "${DURATION_SECONDS:-}" ]]; then
  if [[ "$CLAUDE_RESUME_MODE" == "resume" || ( "$CLAUDE_RESUME_MODE" == "auto" && -f "$HOOK_METADATA_FILE" ) ]]; then
    session_flag=(--continue)
  else
    session_flag=(--session-id "$CLAUDE_SESSION_ID")
  fi
  exec env "${claude_env_args[@]}" \
    "${bwrap_prefix[@]}" "$CLAUDE_CMD" "${claude_base_args[@]}" "${session_flag[@]}" "$CLAUDE_INITIAL_PROMPT"
fi

# --- Duration only (no reset): run for full duration, then warning session ---
if [[ -z "$RESET_EVERY" && -n "${DURATION_SECONDS:-}" ]]; then
  if [[ "$CLAUDE_RESUME_MODE" == "resume" || ( "$CLAUDE_RESUME_MODE" == "auto" && -f "$HOOK_METADATA_FILE" ) ]]; then
    session_flag=(--continue)
  else
    session_flag=(--session-id "$CLAUDE_SESSION_ID")
  fi

  # Main session: run for the full duration
  main_deadline_epoch=$(( $(date +%s) + DURATION_SECONDS ))
  main_session_mode="continue"
  main_session_id="$(current_runtime_session_id)"
  if [[ "${session_flag[0]}" == "--session-id" ]]; then
    main_session_mode="session_id"
    main_session_id="${session_flag[1]}"
  fi
  main_status="$(run_claude_until_deadline "$main_deadline_epoch" "$main_session_mode" "$main_session_id" "$CLAUDE_INITIAL_PROMPT" "duration_main")"

  if [[ "$main_status" -ne 124 && "$main_status" -ne 143 ]]; then
    exit "$main_status"
  fi

  # Warning session: continue with checkpoint prompt, extra time to save
  echo "Sending shutdown warning..."
  warning_deadline_epoch=$(( $(date +%s) + CHECKPOINT_WARNING_SECONDS ))
  run_claude_until_deadline "$warning_deadline_epoch" "continue" "$(current_runtime_session_id)" "$CHECKPOINT_WARNING_PROMPT" "duration_warning" >/dev/null
  sync_latest_session_artifacts
  exit 0
fi

# --- Reset loop: full session → warning → fresh session ---
reset_index=0
while true; do
  echo "--- Reset cycle $reset_index ---"

  # Choose session args: first run or fresh session after reset
  if (( reset_index == 0 )) && [[ "$CLAUDE_RESUME_MODE" == "resume" || ( "$CLAUDE_RESUME_MODE" == "auto" && -f "$HOOK_METADATA_FILE" ) ]]; then
    session_args=(--continue)
  else
    new_session_id="$(generate_session_id)"
    session_args=(--session-id "$new_session_id")
  fi

  cycle_deadline_epoch=$(( $(date +%s) + RESET_EVERY ))
  cycle_session_mode="continue"
  cycle_session_id="$(current_runtime_session_id)"
  if [[ "${session_args[0]}" == "--session-id" ]]; then
    cycle_session_mode="session_id"
    cycle_session_id="${session_args[1]}"
  fi

  # Main session: run for full RESET_EVERY duration, restarting on recoverable Claude failures.
  main_status="$(run_claude_until_deadline "$cycle_deadline_epoch" "$cycle_session_mode" "$cycle_session_id" "$CLAUDE_INITIAL_PROMPT" "reset_cycle_${reset_index}_main")"

  # If claude exited on its own (not timeout), stop
  if [[ "$main_status" -ne 124 && "$main_status" -ne 143 ]]; then
    exit "$main_status"
  fi

  # Warning session: continue the same session with checkpoint prompt
  echo "Sending checkpoint warning..."
  warning_deadline_epoch=$(( $(date +%s) + CHECKPOINT_WARNING_SECONDS ))
  run_claude_until_deadline "$warning_deadline_epoch" "continue" "$(current_runtime_session_id)" "$CHECKPOINT_WARNING_PROMPT" "reset_cycle_${reset_index}_warning" >/dev/null
  sync_latest_session_artifacts

  reset_index=$((reset_index + 1))
  echo "Claude agent reset after ${RESET_EVERY}s (cycle $reset_index)"
done
