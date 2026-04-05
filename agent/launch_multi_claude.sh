#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

if [[ -f "$ROOT_DIR/.env" ]]; then
  set -a
  source "$ROOT_DIR/.env"
  set +a
fi

NUM_WORLDS="${NUM_WORLDS:-1}"
AGENTS_PER_WORLD="${AGENTS_PER_WORLD:-1}"
BASE_PORT="${BASE_PORT:-8080}"
TMUX_SESSION="${TMUX_SESSION:-clawblox-claude}"
RECORD="${RECORD:-true}"
WORLD_DIR="${WORLD_DIR:-}"
HEALTH_TIMEOUT="${HEALTH_TIMEOUT:-30}"
RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
AGENT_NAME_PREFIX="${AGENT_NAME_PREFIX:-agent}"
STOP_GRACE_SECONDS="${STOP_GRACE_SECONDS:-10}"
GOAL="${GOAL:-}"
TEMPLATE_DIR="${TEMPLATE_DIR:-$ROOT_DIR/worlds/mesa-island/agent-template}"
RESULTS_ROOT="${RESULTS_ROOT:-}"
PORT_WAIT_SECONDS="${PORT_WAIT_SECONDS:-120}"
RESUME_PATH="${RESUME_PATH:-}"
RESET_EVERY="${RESET_EVERY:-}"
DURATION="${DURATION:-}"
CHECKPOINT_INTERVAL="${CHECKPOINT_INTERVAL:-1800}"
CLAUDE_MODEL="${CLAUDE_MODEL:-claude-opus-4-6}"
CLAUDE_PERMISSION_MODE="${CLAUDE_PERMISSION_MODE:-bypassPermissions}"
CLAUDE_BARE="${CLAUDE_BARE:-0}"
CLAUDE_EXTRA_ARGS="${CLAUDE_EXTRA_ARGS:-}"
CLAWBLOX_CLAUDE_STOP_BLOCKS="${CLAWBLOX_CLAUDE_STOP_BLOCKS:-0}"
CLAUDE_USE_ENV_AUTH="${CLAUDE_USE_ENV_AUTH:-0}"
SANDBOX="${SANDBOX:-0}"
declare -a AGENT_TEMPLATE_DIRS=()

AGENT_SCRIPT="$SCRIPT_DIR/run_claude_agent.sh"
if [[ ! -f "$AGENT_SCRIPT" ]]; then
  echo "Error: Claude agent runner not found at $AGENT_SCRIPT"
  exit 1
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
      echo "Error: invalid duration format '$input' (use e.g. 2h, 30m, 1h30m, or seconds)" >&2
      return 1
    fi
  done
  printf '%s' "$total"
}

sanitize_name() {
  printf '%s' "$1" | tr -c 'A-Za-z0-9._-' '_'
}

agent_display_name_for_index() {
  local idx="$1"
  local -a creature_names=(Eko Moa Rua Tavi Oni Zev Ika Pala Sori Nyx)
  if ((idx < ${#creature_names[@]})); then
    printf '%s' "${creature_names[$idx]}"
  else
    printf '%s' "${AGENT_NAME_PREFIX}-a${idx}"
  fi
}

session_meta_file() {
  printf '%s/.clawblox-multi/%s.env' "$ROOT_DIR" "$(sanitize_name "$TMUX_SESSION")"
}

write_run_metadata() {
  local target="$1"
  mkdir -p "$(dirname "$target")"
  cat >"$target" <<EOF
RUN_ID=$(printf '%q' "$RUN_ID")
RUN_SAFE_ID=$(printf '%q' "$RUN_SAFE_ID")
RUN_DIR=$(printf '%q' "$RUN_DIR")
WORLDS_ROOT=$(printf '%q' "$WORLDS_ROOT")
WORLD_ABS_DIR=$(printf '%q' "$WORLD_ABS_DIR")
NUM_WORLDS=$(printf '%q' "$NUM_WORLDS")
AGENTS_PER_WORLD=$(printf '%q' "$AGENTS_PER_WORLD")
BASE_PORT=$(printf '%q' "$BASE_PORT")
RECORD=$(printf '%q' "$RECORD")
RESET_EVERY=$(printf '%q' "$RESET_EVERY")
CLAWBLOX_BIN=$(printf '%q' "$CLAWBLOX_BIN")
TMUX_SESSION=$(printf '%q' "$TMUX_SESSION")
EOF
}

save_world_snapshot() {
  local world_index="$1"
  local port="$2"
  local world_root="$RUN_DIR/worlds/world-$world_index"
  local resume_dir="$world_root/resume"
  local snapshot_file="$resume_dir/latest.json"
  local tmp_file="$snapshot_file.tmp"
  mkdir -p "$resume_dir"
  if curl -fsS "http://localhost:${port}/snapshot" >"$tmp_file"; then
    local ts
    ts="$(date -u +%Y%m%dT%H%M%SZ)"
    cp "$tmp_file" "$resume_dir/checkpoint_${ts}.json"
    mv "$tmp_file" "$snapshot_file"
    echo "Saved snapshot: $snapshot_file (checkpoint_${ts}.json)"
    return 0
  fi
  rm -f "$tmp_file"
  echo "Error: failed to save snapshot for world-$world_index on port $port" >&2
  return 1
}

usage() {
  cat <<EOF
Usage: $(basename "$0") [--stop|--status] [options]

Launch a clawblox world with Claude Code agents in tmux.
EOF
}

require_value() {
  local opt="$1"
  local maybe_value="${2:-}"
  if [[ -z "$maybe_value" ]]; then
    echo "Error: $opt requires a value."
    usage
    exit 1
  fi
}

is_nonnegative_int() { [[ "$1" =~ ^[0-9]+$ ]]; }
is_positive_int() { [[ "$1" =~ ^[1-9][0-9]*$ ]]; }

resolve_clawblox_bin() {
  if [[ -n "${CLAWBLOX_BIN:-}" ]]; then
    if command -v "$CLAWBLOX_BIN" >/dev/null 2>&1 || [[ -x "$CLAWBLOX_BIN" ]]; then
      return 0
    fi
    echo "Error: CLAWBLOX_BIN '$CLAWBLOX_BIN' was not found."
    exit 1
  fi
  if [[ -f "$ROOT_DIR/target/release/clawblox" ]]; then
    CLAWBLOX_BIN="$ROOT_DIR/target/release/clawblox"
  elif command -v clawblox >/dev/null 2>&1; then
    CLAWBLOX_BIN="clawblox"
  else
    echo "Error: clawblox binary not found in target/release/ or PATH"
    exit 1
  fi
}

verify_recording_db() {
  local recording_file="$1"
  if [[ ! -f "$recording_file" ]]; then
    return 0
  fi
  if [[ -f "${recording_file}-wal" || -f "${recording_file}-shm" ]]; then
    echo "Warning: recording not finalized cleanly: $recording_file"
  fi
}

collect_session_recordings() {
  local window_name
  tmux list-windows -t "$TMUX_SESSION" -F "#{window_name}" 2>/dev/null | while IFS= read -r window_name; do
    [[ "$window_name" == world-* ]] || continue
    tmux capture-pane -J -p -S -2000 -t "${TMUX_SESSION}:${window_name}" 2>/dev/null | sed -n 's/^Recording to: //p' | tail -n 1
  done | awk 'NF && !seen[$0]++'
}

do_stop() {
  local recordings=()
  local meta_file
  local i
  local port
  local snapshot_failed=0
  if tmux has-session -t "$TMUX_SESSION" 2>/dev/null; then
    mapfile -t recordings < <(collect_session_recordings)
    meta_file="$(session_meta_file)"
    if [[ -f "$meta_file" ]]; then
      # shellcheck disable=SC1090
      source "$meta_file"
      if [[ -n "${RUN_DIR:-}" && -n "${NUM_WORLDS:-}" && -n "${BASE_PORT:-}" ]]; then
        echo "Saving world snapshots..."
        for ((i = 0; i < NUM_WORLDS; i++)); do
          port=$((BASE_PORT + i))
          if ! save_world_snapshot "$i" "$port"; then
            snapshot_failed=1
          fi
        done
        if ((snapshot_failed != 0)); then
          if ((FORCE == 1)); then
            echo "Warning: snapshot capture failed, continuing with --force." >&2
          else
            echo "Snapshot capture failed. Aborting shutdown so the live run remains resumable." >&2
            exit 1
          fi
        fi
      fi
    fi
    echo "Stopping world servers..."
    while IFS= read -r window_name; do
      [[ "$window_name" == world-* ]] || continue
      tmux send-keys -t "${TMUX_SESSION}:${window_name}" C-c 2>/dev/null || true
    done < <(tmux list-windows -t "$TMUX_SESSION" -F "#{window_name}" 2>/dev/null)
    echo "Waiting ${STOP_GRACE_SECONDS}s for shutdown..."
    sleep "$STOP_GRACE_SECONDS"
    if tmux has-session -t "$TMUX_SESSION" 2>/dev/null; then
      echo "Killing tmux session '$TMUX_SESSION'..."
      tmux kill-session -t "$TMUX_SESSION" 2>/dev/null || true
    fi
    if ((${#recordings[@]} > 0)); then
      echo "Verifying recording databases..."
      for recording_file in "${recordings[@]}"; do
        verify_recording_db "$recording_file"
      done
    fi
    echo "Stopped."
    rm -f "$meta_file"
  else
    echo "No tmux session '$TMUX_SESSION' found."
  fi
}

do_status() {
  if ! tmux has-session -t "$TMUX_SESSION" 2>/dev/null; then
    echo "No tmux session '$TMUX_SESSION' running."
    exit 1
  fi
  echo "Session: $TMUX_SESSION"
  echo "Windows:"
  tmux list-windows -t "$TMUX_SESSION" -F "  #{window_index}: #{window_name} (#{window_panes} panes)"
}

wait_for_server() {
  local port="$1"
  local deadline=$((SECONDS + HEALTH_TIMEOUT))
  while ((SECONDS < deadline)); do
    if curl -sf "http://localhost:${port}/skill.md" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  return 1
}

port_is_in_use() {
  local port="$1"
  (echo >/dev/tcp/127.0.0.1/"$port") >/dev/null 2>&1
}

ensure_ports_available() {
  local i port waited=0 deadline=0
  local busy_ports=() pids=()
  while true; do
    busy_ports=()
    for ((i = 0; i < NUM_WORLDS; i++)); do
      port=$((BASE_PORT + i))
      if port_is_in_use "$port"; then
        busy_ports+=("$port")
      fi
    done
    if ((${#busy_ports[@]} == 0)); then
      return 0
    fi
    if ((PORT_WAIT_SECONDS == 0)); then
      break
    fi
    if ((waited == 0)); then
      deadline=$((SECONDS + PORT_WAIT_SECONDS))
      echo "Target ports are busy (${busy_ports[*]}). Waiting up to ${PORT_WAIT_SECONDS}s for release..."
    fi
    if ((SECONDS >= deadline)); then
      break
    fi
    waited=1
    sleep 1
  done
  for port in "${busy_ports[@]}"; do
    echo "Error: target port $port is already in use."
    if command -v lsof >/dev/null 2>&1; then
      mapfile -t pids < <(lsof -nP -t -iTCP:"$port" -sTCP:LISTEN 2>/dev/null | sort -u)
      if ((${#pids[@]} > 0)); then
        echo "  listening pids: ${pids[*]}"
      fi
    fi
  done
  exit 1
}

require_claude_auth() {
  if ! command -v claude >/dev/null 2>&1; then
    echo "Error: claude CLI was not found in PATH."
    exit 1
  fi
  if ! claude auth status --text >/dev/null 2>&1; then
    echo "Error: Claude Code is not logged in. Run: claude auth login"
    exit 1
  fi
}

FORCE=0
ACTION="run"
CLI_NUM_WORLDS=0
CLI_AGENTS_PER_WORLD=0
CLI_BASE_PORT=0
CLI_TMUX_SESSION=0
CLI_RECORD=0
CLI_RESET_EVERY=0
CLI_CLAWBLOX_BIN=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --stop) ACTION="stop"; shift ;;
    --status) ACTION="status"; shift ;;
    --force) FORCE=1; shift ;;
    --num-worlds) require_value "$1" "${2:-}"; NUM_WORLDS="$2"; CLI_NUM_WORLDS=1; shift 2 ;;
    --agents-per-world) require_value "$1" "${2:-}"; AGENTS_PER_WORLD="$2"; CLI_AGENTS_PER_WORLD=1; shift 2 ;;
    --base-port) require_value "$1" "${2:-}"; BASE_PORT="$2"; CLI_BASE_PORT=1; shift 2 ;;
    --tmux-session) require_value "$1" "${2:-}"; TMUX_SESSION="$2"; CLI_TMUX_SESSION=1; shift 2 ;;
    --record) require_value "$1" "${2:-}"; RECORD="$2"; CLI_RECORD=1; shift 2 ;;
    --world-dir) require_value "$1" "${2:-}"; WORLD_DIR="$2"; shift 2 ;;
    --health-timeout) require_value "$1" "${2:-}"; HEALTH_TIMEOUT="$2"; shift 2 ;;
    --run-id) require_value "$1" "${2:-}"; RUN_ID="$2"; shift 2 ;;
    --agent-name-prefix) require_value "$1" "${2:-}"; AGENT_NAME_PREFIX="$2"; shift 2 ;;
    --stop-grace-seconds) require_value "$1" "${2:-}"; STOP_GRACE_SECONDS="$2"; shift 2 ;;
    --port-wait-seconds) require_value "$1" "${2:-}"; PORT_WAIT_SECONDS="$2"; shift 2 ;;
    --clawblox-bin) require_value "$1" "${2:-}"; CLAWBLOX_BIN="$2"; CLI_CLAWBLOX_BIN=1; shift 2 ;;
    --goal) require_value "$1" "${2:-}"; GOAL="$2"; shift 2 ;;
    --reset-every) require_value "$1" "${2:-}"; RESET_EVERY="$2"; CLI_RESET_EVERY=1; shift 2 ;;
    --duration) require_value "$1" "${2:-}"; DURATION="$2"; shift 2 ;;
    --model) require_value "$1" "${2:-}"; CLAUDE_MODEL="$2"; shift 2 ;;
    --permission-mode) require_value "$1" "${2:-}"; CLAUDE_PERMISSION_MODE="$2"; shift 2 ;;
    --claude-extra-args) require_value "$1" "${2:-}"; CLAUDE_EXTRA_ARGS="$2"; shift 2 ;;
    --stop-blocks) require_value "$1" "${2:-}"; CLAWBLOX_CLAUDE_STOP_BLOCKS="$2"; shift 2 ;;
    --use-env-auth) CLAUDE_USE_ENV_AUTH=1; shift ;;
    --sandbox) SANDBOX=1; shift ;;
    --template|--template-dir) require_value "$1" "${2:-}"; TEMPLATE_DIR="$2"; shift 2 ;;
    --system-prompt) require_value "$1" "${2:-}"; SYSTEM_PROMPT_TEMPLATE="$2"; shift 2 ;;
    --agent-template) require_value "$1" "${2:-}"; AGENT_TEMPLATE_DIRS+=("$2"); shift 2 ;;
    --results-root) require_value "$1" "${2:-}"; RESULTS_ROOT="$2"; shift 2 ;;
    --resume) require_value "$1" "${2:-}"; RESUME_PATH="$2"; shift 2 ;;
    --checkpoint-interval) require_value "$1" "${2:-}"; CHECKPOINT_INTERVAL="$2"; shift 2 ;;
    --help|-h) usage; exit 0 ;;
    *) echo "Unknown option: $1"; usage; exit 1 ;;
  esac
done

case "$ACTION" in
  stop) do_stop; exit 0 ;;
  status) do_status; exit 0 ;;
esac

resolve_clawblox_bin
require_claude_auth

if tmux has-session -t "$TMUX_SESSION" 2>/dev/null; then
  if [[ "$FORCE" -eq 1 ]]; then
    echo "Replacing existing session '$TMUX_SESSION'..."
    do_stop
  else
    echo "Tmux session '$TMUX_SESSION' already exists."
    exit 1
  fi
fi

if [[ -n "$RESUME_PATH" ]]; then
  requested_num_worlds="$NUM_WORLDS"
  requested_agents_per_world="$AGENTS_PER_WORLD"
  requested_base_port="$BASE_PORT"
  requested_tmux_session="$TMUX_SESSION"
  requested_record="$RECORD"
  requested_reset_every="$RESET_EVERY"
  requested_clawblox_bin="${CLAWBLOX_BIN:-}"
  if [[ "$RESUME_PATH" = /* ]]; then
    RUN_DIR="$RESUME_PATH"
  else
    RUN_DIR="$ROOT_DIR/$RESUME_PATH"
  fi
  if [[ ! -d "$RUN_DIR" ]]; then
    echo "Error: resume run directory not found at $RUN_DIR"
    exit 1
  fi
  RUN_META_FILE="$RUN_DIR/run.env"
  if [[ ! -f "$RUN_META_FILE" ]]; then
    echo "Error: missing run metadata at $RUN_META_FILE"
    exit 1
  fi
  # shellcheck disable=SC1090
  source "$RUN_META_FILE"
  if ((CLI_NUM_WORLDS == 1)); then
    NUM_WORLDS="$requested_num_worlds"
  fi
  if ((CLI_AGENTS_PER_WORLD == 1)); then
    AGENTS_PER_WORLD="$requested_agents_per_world"
  fi
  if ((CLI_BASE_PORT == 1)); then
    BASE_PORT="$requested_base_port"
  fi
  if ((CLI_TMUX_SESSION == 1)); then
    TMUX_SESSION="$requested_tmux_session"
  fi
  if ((CLI_RECORD == 1)); then
    RECORD="$requested_record"
  fi
  if ((CLI_RESET_EVERY == 1)); then
    RESET_EVERY="$requested_reset_every"
  fi
  if ((CLI_CLAWBLOX_BIN == 1)); then
    CLAWBLOX_BIN="$requested_clawblox_bin"
  fi
  RESUME_PATH="$RUN_DIR"
  RESULTS_ABS_ROOT="$(dirname "$RUN_DIR")"
  for ((i = 0; i < NUM_WORLDS; i++)); do
    if [[ ! -f "$RUN_DIR/worlds/world-$i/resume/latest.json" ]]; then
      echo "Error: missing resume snapshot for world-$i at $RUN_DIR/worlds/world-$i/resume/latest.json"
      exit 1
    fi
  done
else
  if [[ -z "$WORLD_DIR" ]]; then
    echo "Error: --world-dir is required"
    exit 1
  fi
  if [[ "$WORLD_DIR" = /* ]]; then
    WORLD_ABS_DIR="$WORLD_DIR"
  else
    WORLD_ABS_DIR="$ROOT_DIR/${WORLD_DIR#./}"
  fi
fi

if [[ ! -d "$WORLD_ABS_DIR" ]]; then
  echo "Error: world directory not found at $WORLD_ABS_DIR"
  exit 1
fi

TEMPLATE_ABS_DIR="$TEMPLATE_DIR"
if [[ "$TEMPLATE_ABS_DIR" != /* ]]; then
  TEMPLATE_ABS_DIR="$ROOT_DIR/$TEMPLATE_ABS_DIR"
fi

if [[ -z "${SYSTEM_PROMPT_TEMPLATE:-}" ]]; then
  if [[ -f "$WORLD_ABS_DIR/system_prompt.md" ]]; then
    SYSTEM_PROMPT_TEMPLATE="$WORLD_ABS_DIR/system_prompt.md"
  elif [[ -f "$TEMPLATE_ABS_DIR/system_prompt.md" ]]; then
    SYSTEM_PROMPT_TEMPLATE="$TEMPLATE_ABS_DIR/system_prompt.md"
  else
    echo "Error: no system_prompt.md found in world dir ($WORLD_ABS_DIR) or template dir ($TEMPLATE_ABS_DIR)" >&2
    exit 1
  fi
fi

RUN_SAFE_ID="$(sanitize_name "$RUN_ID")"
if [[ -z "${RESULTS_ABS_ROOT:-}" ]]; then
  if [[ -n "$RESULTS_ROOT" ]]; then
    if [[ "$RESULTS_ROOT" = /* ]]; then
      RESULTS_ABS_ROOT="$RESULTS_ROOT"
    else
      RESULTS_ABS_ROOT="$ROOT_DIR/$RESULTS_ROOT"
    fi
  else
    RESULTS_ABS_ROOT="$WORLD_ABS_DIR/results"
  fi
fi

if [[ -z "${RUN_DIR:-}" ]]; then
  RUN_DIR="$RESULTS_ABS_ROOT/$RUN_SAFE_ID"
fi
if [[ -z "${WORLDS_ROOT:-}" ]]; then
  WORLDS_ROOT="$RUN_DIR/worlds"
fi

if [[ -n "$RESUME_PATH" ]]; then
  for ((i = 0; i < NUM_WORLDS; i++)); do
    for ((j = 0; j < AGENTS_PER_WORLD; j++)); do
      agent_display_name="$(agent_display_name_for_index "$j")"
      agent_name="$(sanitize_name "${agent_display_name}-r${RUN_SAFE_ID}-w${i}-a${j}")"
      agent_dir="$RUN_DIR/worlds/world-$i/agents/$agent_name"
      if [[ ! -f "$agent_dir/claude_session_id.txt" ]]; then
        echo "Error: missing Claude session file for resume at $agent_dir/claude_session_id.txt"
        exit 1
      fi
      if [[ ! -f "$agent_dir/world_session.txt" ]]; then
        echo "Error: missing world session file for resume at $agent_dir/world_session.txt"
        exit 1
      fi
    done
  done
fi

if [[ -z "$RESUME_PATH" && -e "$RUN_DIR" ]]; then
  echo "Error: run directory already exists: $RUN_DIR"
  echo "Set RUN_ID to a unique value and retry."
  exit 1
fi

ensure_ports_available
mkdir -p "$WORLDS_ROOT"
printf '%s\n' "$RUN_DIR" >"$RESULTS_ABS_ROOT/latest_run.txt"
write_run_metadata "$RUN_DIR/run.env"
write_run_metadata "$(session_meta_file)"

echo "Starting $NUM_WORLDS world servers..."
tmux new-session -d -s "$TMUX_SESSION" -n "world-0"
tmux set-option -t "$TMUX_SESSION" remain-on-exit on

for ((i = 0; i < NUM_WORLDS; i++)); do
  port=$((BASE_PORT + i))
  world_root="$WORLDS_ROOT/world-$i"
  world_log_dir="$world_root/logs"
  world_log="$world_log_dir/world.log"
  world_record_dir="$world_root/recordings"
  world_agents_dir="$world_root/agents"
  world_resume_dir="$world_root/resume"
  world_resume_file="$world_resume_dir/latest.json"
  world_resume_arg=""
  mkdir -p "$world_log_dir" "$world_record_dir" "$world_agents_dir" "$world_resume_dir"
  if ((i > 0)); then
    tmux new-window -t "$TMUX_SESSION" -n "world-$i"
  fi
  if [[ -n "$RESUME_PATH" ]]; then
    world_resume_arg="--resume '$world_resume_file' "
  fi
  if [[ "$RECORD" == "true" ]]; then
    tmux send-keys -t "${TMUX_SESSION}:world-$i" \
      "cd '$ROOT_DIR' && '$CLAWBLOX_BIN' run '$WORLD_ABS_DIR' --port $port ${world_resume_arg}--record --record-dir '$world_record_dir' 2>&1 | tee -a '$world_log'" Enter
  else
    tmux send-keys -t "${TMUX_SESSION}:world-$i" \
      "cd '$ROOT_DIR' && '$CLAWBLOX_BIN' run '$WORLD_ABS_DIR' --port $port ${world_resume_arg}2>&1 | tee -a '$world_log'" Enter
  fi
done

echo "Waiting for servers to become healthy..."
for ((i = 0; i < NUM_WORLDS; i++)); do
  port=$((BASE_PORT + i))
  printf "  world-%d (port %d)... " "$i" "$port"
  if wait_for_server "$port"; then
    echo "ready"
  else
    echo "TIMEOUT (${HEALTH_TIMEOUT}s)"
  fi
done

DURATION_SECONDS=""
if [[ -n "$DURATION" ]]; then
  DURATION_SECONDS="$(parse_duration_seconds "$DURATION")"
fi

echo "Launching $AGENTS_PER_WORLD Claude agents per world..."
for ((i = 0; i < NUM_WORLDS; i++)); do
  port=$((BASE_PORT + i))
  url="http://localhost:${port}"
  startup_instructions=""
  if [[ -n "$GOAL" ]]; then
    startup_instructions="$GOAL"
  fi
  tmux new-window -t "$TMUX_SESSION" -n "agents-$i"
  for ((j = 0; j < AGENTS_PER_WORLD; j++)); do
    pane_target="${TMUX_SESSION}:agents-$i"
    if ((j > 0)); then
      pane_target="$(tmux split-window -P -F '#{pane_id}' -t "${TMUX_SESSION}:agents-$i")"
      tmux select-layout -t "${TMUX_SESSION}:agents-$i" tiled
    else
      pane_target="$(tmux display-message -p -t "${TMUX_SESSION}:agents-$i" '#{pane_id}')"
    fi
    agent_display_name="$(agent_display_name_for_index "$j")"
    agent_name="$(sanitize_name "${agent_display_name}-r${RUN_SAFE_ID}-w${i}-a${j}")"
    world_agents_dir="$WORLDS_ROOT/world-$i/agents"
    agent_dir="$world_agents_dir/$agent_name"
    agent_log_dir="$agent_dir/logs"
    agent_workspace="$agent_dir/workspace"
    agent_session_file="$agent_dir/claude_session_id.txt"
    agent_log_file="$agent_log_dir/agent.log"
    agent_world_session_file="$agent_dir/world_session.txt"
    mkdir -p "$agent_log_dir" "$agent_workspace"

    template_env_prefix="TEMPLATE_DIR=$(printf '%q' "$TEMPLATE_ABS_DIR") SYSTEM_PROMPT_TEMPLATE=$(printf '%q' "$SYSTEM_PROMPT_TEMPLATE") "
    startup_env_prefix=""
    if [[ -n "$startup_instructions" ]]; then
      startup_instructions_q="$(printf '%q' "$startup_instructions")"
      startup_env_prefix="WORLD_STARTUP_INSTRUCTIONS=${startup_instructions_q} "
    fi

    reset_wrapper_prefix="bash '$AGENT_SCRIPT'"

    auth_env_prefix="unset ANTHROPIC_API_KEY ANTHROPIC_OAUTH_TOKEN; "
    if [[ "$CLAUDE_USE_ENV_AUTH" == "1" ]]; then
      auth_env_prefix=""
    fi

    sandbox_env_prefix=""
    if [[ "$SANDBOX" == "1" ]]; then
      sandbox_env_prefix="SANDBOX=1 "
    fi

    tmux pipe-pane -o -t "$pane_target" "cat >> '$(printf "%q" "$agent_log_file")'"
    tmux send-keys -t "$pane_target" \
      "cd '$ROOT_DIR' && ${auth_env_prefix}WORLD_BASE_URL='$url' WORLD_AGENT_NAME='$agent_display_name' AGENT_DIR='$agent_dir' WORKSPACE_DIR='$agent_workspace' SESSION_ID_FILE='$agent_session_file' WORLD_SESSION_FILE='$agent_world_session_file' RESET_EVERY=$(printf '%q' "$RESET_EVERY") DURATION_SECONDS=$(printf '%q' "$DURATION_SECONDS") CLAUDE_MODEL=$(printf '%q' "$CLAUDE_MODEL") CLAUDE_PERMISSION_MODE=$(printf '%q' "$CLAUDE_PERMISSION_MODE") CLAUDE_BARE=$(printf '%q' "$CLAUDE_BARE") CLAUDE_EXTRA_ARGS=$(printf '%q' "$CLAUDE_EXTRA_ARGS") CLAWBLOX_CLAUDE_STOP_BLOCKS=$(printf '%q' "$CLAWBLOX_CLAUDE_STOP_BLOCKS") ${sandbox_env_prefix}${template_env_prefix}${startup_env_prefix}${reset_wrapper_prefix}" Enter
  done
done

CHECKPOINT_WARNING_SECONDS="${CHECKPOINT_WARNING_SECONDS:-300}"
if [[ -n "$DURATION" ]]; then
  duration_secs="$(parse_duration_seconds "$DURATION")"
  # Agent gets duration + warning time to save workspace
  total_wait=$(( duration_secs + CHECKPOINT_WARNING_SECONDS + 30 ))
  echo "Run will auto-stop after ${DURATION} (${duration_secs}s + ${CHECKPOINT_WARNING_SECONDS}s warning)..."
  (
    sleep "$total_wait"
    echo "Duration ${DURATION} + warning elapsed. Stopping..."
    bash "$0" --stop --tmux-session "$TMUX_SESSION"
  ) &
  disown
fi

echo "tmux attach -t $TMUX_SESSION"
