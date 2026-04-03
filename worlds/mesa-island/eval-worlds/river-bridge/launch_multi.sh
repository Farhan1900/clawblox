#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../../../.." && pwd)"

if [[ -f "$ROOT_DIR/.env" ]]; then
  set -a
  source "$ROOT_DIR/.env"
  set +a
fi

NUM_WORLDS="${NUM_WORLDS:-5}"
AGENTS_PER_WORLD="${AGENTS_PER_WORLD:-1}"
BASE_PORT="${BASE_PORT:-8080}"
TMUX_SESSION="${TMUX_SESSION:-river-bridge}"
RECORD="${RECORD:-true}"
WORLD_DIR="${WORLD_DIR:-$SCRIPT_DIR}"
HEALTH_TIMEOUT="${HEALTH_TIMEOUT:-30}"
RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
AGENT_NAME_PREFIX="${AGENT_NAME_PREFIX:-river-bridge}"
STOP_GRACE_SECONDS="${STOP_GRACE_SECONDS:-10}"
GOAL="${GOAL:-}"
TEMPLATE_DIR="${TEMPLATE_DIR:-worlds/mesa-island/agent-template}"
RESULTS_ROOT="${RESULTS_ROOT:-}"
PORT_WAIT_SECONDS="${PORT_WAIT_SECONDS:-120}"
RESUME_PATH="${RESUME_PATH:-}"
RESET_EVERY="${RESET_EVERY:-}"
PI_MODEL="${PI_MODEL:-}"
AGENT_CHECKPOINT_AT_EPOCH="${AGENT_CHECKPOINT_AT_EPOCH:-}"
AGENT_CHECKPOINT_PROMPT="${AGENT_CHECKPOINT_PROMPT:-}"
AGENT_CHECKPOINT_TRIGGER_TURN="${AGENT_CHECKPOINT_TRIGGER_TURN:-1}"
AGENT_CHECKPOINT_DISABLE_AUTO_LOOP="${AGENT_CHECKPOINT_DISABLE_AUTO_LOOP:-0}"
CHECKPOINT_INTERVAL="${CHECKPOINT_INTERVAL:-1800}"  # periodic snapshot interval in seconds (0 = disabled)
SANDBOX="${SANDBOX:-}"
declare -a AGENT_TEMPLATE_DIRS=()

AGENT_SCRIPT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/run_agent.sh"
if [[ ! -f "$AGENT_SCRIPT" ]]; then
  echo "Error: run_agent.sh not found at $AGENT_SCRIPT"
  exit 1
fi

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
SANDBOX=$(printf '%q' "$SANDBOX")
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

Launches $NUM_WORLDS mesa world servers and $AGENTS_PER_WORLD agents per world in tmux.

Options:
  --stop                       Gracefully stop worlds/agents, then kill the tmux session
  --status                     Show running session info and exit
  --force                      Skip confirmation when replacing an existing session
  --num-worlds N               Number of world servers
  --agents-per-world N         Agents per world
  --base-port PORT             First server port
  --tmux-session NAME          Tmux session name
  --record true|false          Enable/disable recording
  --world-dir PATH             World directory (relative to project root or absolute)
  --health-timeout SECONDS     Server health-check timeout
  --run-id ID                  Run identifier under results/
  --agent-name-prefix PREFIX   Agent name prefix
  --stop-grace-seconds N       Wait after Ctrl+C before force-killing tmux
  --port-wait-seconds N        Wait up to N seconds for target ports to free before failing
  --goal TEXT                  Custom startup objective passed to agents
  --template PATH              Agent template directory (relative to project root or absolute)
  --agent-template PATH        Per-agent template directory; repeat in agent index order
  --reset-every SECONDS        Restart each agent session on this interval
  --model MODEL_ID             Model passed to pi via PI_MODEL
  --results-root PATH          Results root directory (default: <world-dir>/results)
  --resume PATH                Resume an existing run directory under results/
  --checkpoint-interval SECS   Periodic snapshot interval (default: 1800, 0 = disabled)
  --sandbox                    Run agents in a bubblewrap sandbox (requires bwrap)
  --clawblox-bin PATH          Path or command name for clawblox binary

Environment variables:
  NUM_WORLDS        Number of world servers (default: 5)
  AGENTS_PER_WORLD  Agents per world (default: 1)
  BASE_PORT         First server port (default: 8080)
  TMUX_SESSION      Tmux session name (default: clawblox)
  RECORD            Enable recording (default: true)
  WORLD_DIR         World directory relative to project root (default: worlds/mesa-world)
  HEALTH_TIMEOUT    Seconds to wait for each server to become healthy (default: 30)
  RUN_ID            Run identifier under results/ (default: UTC timestamp)
  AGENT_NAME_PREFIX Agent name prefix (default: mesa)
  STOP_GRACE_SECONDS Seconds to wait after Ctrl+C before force-closing tmux (default: 5)
  PORT_WAIT_SECONDS Seconds to wait for target ports to free before failing (default: 120)
  GOAL              Custom startup objective passed to agents
  TEMPLATE_DIR      Agent template directory (run_agent.sh TEMPLATE_DIR override)
  RESET_EVERY       Restart each agent session every N seconds
  PI_MODEL          Model passed to pi via --model
  RESULTS_ROOT      Results root directory (default: <world-dir>/results)
  AGENT_CHECKPOINT_AT_EPOCH       Unix epoch seconds when checkpoint prompt should be sent
  AGENT_CHECKPOINT_PROMPT         Checkpoint message sent from clawblox-agent extension
  AGENT_CHECKPOINT_TRIGGER_TURN   1/0 trigger a turn when checkpoint prompt is sent (default: 1)
  AGENT_CHECKPOINT_DISABLE_AUTO_LOOP 1/0 disable continue loop when checkpoint fires (default: 0)
  CHECKPOINT_INTERVAL Periodic snapshot interval in seconds (default: 1800, 0 = disabled)
  SANDBOX           Run agents in a bubblewrap sandbox (default: empty/off, set to 1)
  CLAWBLOX_BIN      Path to clawblox binary (auto-detected)
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

is_nonnegative_int() {
  [[ "$1" =~ ^[0-9]+$ ]]
}

is_positive_int() {
  [[ "$1" =~ ^[1-9][0-9]*$ ]]
}

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

collect_session_recordings() {
  local window_name
  tmux list-windows -t "$TMUX_SESSION" -F "#{window_name}" 2>/dev/null | while IFS= read -r window_name; do
    [[ "$window_name" == world-* ]] || continue
    tmux capture-pane -J -p -S -2000 -t "${TMUX_SESSION}:${window_name}" 2>/dev/null \
      | sed -n 's/^Recording to: //p' \
      | tail -n 1
  done | awk 'NF && !seen[$0]++'
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

do_stop() {
  local recording_file
  local pane_target
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
          echo "Snapshot capture failed. Aborting shutdown so the live run remains resumable." >&2
          exit 1
        fi
      fi
    fi

    # Stop server windows first (world-*) so recordings finalize cleanly
    echo "Stopping world servers..."
    while IFS= read -r window_name; do
      [[ "$window_name" == world-* ]] || continue
      tmux send-keys -t "${TMUX_SESSION}:${window_name}" C-c 2>/dev/null || true
    done < <(tmux list-windows -t "$TMUX_SESSION" -F "#{window_name}" 2>/dev/null)

    echo "Waiting ${STOP_GRACE_SECONDS}s for server shutdown..."
    sleep "$STOP_GRACE_SECONDS"

    # Now kill the entire session (agents + any remaining processes)
    if tmux has-session -t "$TMUX_SESSION" 2>/dev/null; then
      echo "Killing tmux session '$TMUX_SESSION'..."
      tmux kill-session -t "$TMUX_SESSION" 2>/dev/null || true
    else
      echo "Tmux session '$TMUX_SESSION' already exited."
    fi

    # Finalize recordings AFTER killing tmux so no process holds the DB open
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
  echo
  echo "Attach with: tmux attach -t $TMUX_SESSION"
}

FORCE=0
ACTION="run"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --stop)
      ACTION="stop"
      shift
      ;;
    --status)
      ACTION="status"
      shift
      ;;
    --force)
      FORCE=1
      shift
      ;;
    --num-worlds)
      require_value "$1" "${2:-}"
      NUM_WORLDS="$2"
      shift 2
      ;;
    --agents-per-world)
      require_value "$1" "${2:-}"
      AGENTS_PER_WORLD="$2"
      shift 2
      ;;
    --base-port)
      require_value "$1" "${2:-}"
      BASE_PORT="$2"
      shift 2
      ;;
    --tmux-session)
      require_value "$1" "${2:-}"
      TMUX_SESSION="$2"
      shift 2
      ;;
    --record)
      require_value "$1" "${2:-}"
      RECORD="$2"
      shift 2
      ;;
    --world-dir)
      require_value "$1" "${2:-}"
      WORLD_DIR="$2"
      shift 2
      ;;
    --health-timeout)
      require_value "$1" "${2:-}"
      HEALTH_TIMEOUT="$2"
      shift 2
      ;;
    --run-id)
      require_value "$1" "${2:-}"
      RUN_ID="$2"
      shift 2
      ;;
    --agent-name-prefix)
      require_value "$1" "${2:-}"
      AGENT_NAME_PREFIX="$2"
      shift 2
      ;;
    --stop-grace-seconds)
      require_value "$1" "${2:-}"
      STOP_GRACE_SECONDS="$2"
      shift 2
      ;;
    --port-wait-seconds)
      require_value "$1" "${2:-}"
      PORT_WAIT_SECONDS="$2"
      shift 2
      ;;
    --clawblox-bin)
      require_value "$1" "${2:-}"
      CLAWBLOX_BIN="$2"
      shift 2
      ;;
    --goal)
      require_value "$1" "${2:-}"
      GOAL="$2"
      shift 2
      ;;
    --reset-every)
      require_value "$1" "${2:-}"
      RESET_EVERY="$2"
      shift 2
      ;;
    --model)
      require_value "$1" "${2:-}"
      PI_MODEL="$2"
      shift 2
      ;;
    --template|--template-dir)
      require_value "$1" "${2:-}"
      TEMPLATE_DIR="$2"
      shift 2
      ;;
    --agent-template)
      require_value "$1" "${2:-}"
      AGENT_TEMPLATE_DIRS+=("$2")
      shift 2
      ;;
    --results-root)
      require_value "$1" "${2:-}"
      RESULTS_ROOT="$2"
      shift 2
      ;;
    --resume)
      require_value "$1" "${2:-}"
      RESUME_PATH="$2"
      shift 2
      ;;
    --checkpoint-interval)
      require_value "$1" "${2:-}"
      CHECKPOINT_INTERVAL="$2"
      shift 2
      ;;
    --sandbox)
      SANDBOX=1
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1"
      usage
      exit 1
      ;;
  esac
done

if ! is_nonnegative_int "$STOP_GRACE_SECONDS"; then
  echo "Error: STOP_GRACE_SECONDS must be a non-negative integer (got '$STOP_GRACE_SECONDS')."
  exit 1
fi
if ! is_nonnegative_int "$PORT_WAIT_SECONDS"; then
  echo "Error: PORT_WAIT_SECONDS must be a non-negative integer (got '$PORT_WAIT_SECONDS')."
  exit 1
fi
if [[ -n "$RESET_EVERY" ]] && ! is_positive_int "$RESET_EVERY"; then
  echo "Error: RESET_EVERY must be a positive integer in seconds (got '$RESET_EVERY')."
  exit 1
fi
if ! is_nonnegative_int "$CHECKPOINT_INTERVAL"; then
  echo "Error: CHECKPOINT_INTERVAL must be a non-negative integer in seconds (got '$CHECKPOINT_INTERVAL')."
  exit 1
fi

case "$ACTION" in
  stop)
    do_stop
    exit 0
    ;;
  status)
    do_status
    exit 0
    ;;
esac

if ! is_positive_int "$NUM_WORLDS"; then
  echo "Error: NUM_WORLDS must be a positive integer (got '$NUM_WORLDS')."
  exit 1
fi
if ! is_positive_int "$AGENTS_PER_WORLD"; then
  echo "Error: AGENTS_PER_WORLD must be a positive integer (got '$AGENTS_PER_WORLD')."
  exit 1
fi
if ! is_positive_int "$BASE_PORT" || ((BASE_PORT > 65535)); then
  echo "Error: BASE_PORT must be an integer in [1, 65535] (got '$BASE_PORT')."
  exit 1
fi
if ! is_positive_int "$HEALTH_TIMEOUT"; then
  echo "Error: HEALTH_TIMEOUT must be a positive integer (got '$HEALTH_TIMEOUT')."
  exit 1
fi
if [[ "$RECORD" != "true" && "$RECORD" != "false" ]]; then
  echo "Error: RECORD must be 'true' or 'false' (got '$RECORD')."
  exit 1
fi

resolve_clawblox_bin

CLAWBLOX_SUPPORTS_RECORD_DIR=0
if "$CLAWBLOX_BIN" run --help 2>&1 | grep -q -- '--record-dir'; then
  CLAWBLOX_SUPPORTS_RECORD_DIR=1
fi

if tmux has-session -t "$TMUX_SESSION" 2>/dev/null; then
  if [[ "$FORCE" -eq 1 ]]; then
    echo "Replacing existing session '$TMUX_SESSION'..."
    do_stop
  else
    echo "Tmux session '$TMUX_SESSION' already exists."
    echo "Use --force to replace it, or --stop to kill it."
    exit 1
  fi
fi

if [[ -n "$RESUME_PATH" ]]; then
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
  RESUME_PATH="$RUN_DIR"
  RESULTS_ABS_ROOT="$(dirname "$RUN_DIR")"
  for ((i = 0; i < NUM_WORLDS; i++)); do
    if [[ ! -f "$RUN_DIR/worlds/world-$i/resume/latest.json" ]]; then
      echo "Error: missing resume snapshot for world-$i at $RUN_DIR/worlds/world-$i/resume/latest.json"
      exit 1
    fi
  done
elif [[ "$WORLD_DIR" = /* ]]; then
  WORLD_ABS_DIR="$WORLD_DIR"
else
  WORLD_ABS_DIR="$ROOT_DIR/$WORLD_DIR"
fi

if [[ ! -d "$WORLD_ABS_DIR" ]]; then
  echo "Error: world directory not found at $WORLD_ABS_DIR"
  exit 1
fi

TEMPLATE_ABS_DIR=""
if [[ -n "$TEMPLATE_DIR" ]]; then
  if [[ "$TEMPLATE_DIR" = /* ]]; then
    TEMPLATE_ABS_DIR="$TEMPLATE_DIR"
  else
    TEMPLATE_ABS_DIR="$ROOT_DIR/$TEMPLATE_DIR"
  fi

  if [[ ! -d "$TEMPLATE_ABS_DIR" ]]; then
    echo "Error: template directory not found at $TEMPLATE_ABS_DIR"
    exit 1
  fi
fi

if ((${#AGENT_TEMPLATE_DIRS[@]} > AGENTS_PER_WORLD)); then
  echo "Error: received ${#AGENT_TEMPLATE_DIRS[@]} --agent-template values for only $AGENTS_PER_WORLD agents."
  exit 1
fi

declare -a AGENT_TEMPLATE_ABS_DIRS=()
if ((${#AGENT_TEMPLATE_DIRS[@]} > 0)); then
  for template_dir in "${AGENT_TEMPLATE_DIRS[@]}"; do
    template_abs_dir="$template_dir"
    if [[ "$template_abs_dir" != /* ]]; then
      template_abs_dir="$ROOT_DIR/$template_abs_dir"
    fi
    if [[ ! -d "$template_abs_dir" ]]; then
      echo "Error: agent template directory not found at $template_abs_dir"
      exit 1
    fi
    AGENT_TEMPLATE_ABS_DIRS+=("$template_abs_dir")
  done
fi

RUN_SAFE_ID="$(sanitize_name "$RUN_ID")"
if [[ -n "${RESULTS_ABS_ROOT:-}" ]]; then
  :
elif [[ -n "$RESULTS_ROOT" ]]; then
  if [[ "$RESULTS_ROOT" = /* ]]; then
    RESULTS_ABS_ROOT="$RESULTS_ROOT"
  else
    RESULTS_ABS_ROOT="$ROOT_DIR/$RESULTS_ROOT"
  fi
else
  RESULTS_ABS_ROOT="$WORLD_ABS_DIR/results"
fi

if [[ -n "$RESUME_PATH" ]]; then
  for ((i = 0; i < NUM_WORLDS; i++)); do
    for ((j = 0; j < AGENTS_PER_WORLD; j++)); do
      agent_display_name="$(agent_display_name_for_index "$j")"
      agent_name="$(sanitize_name "${agent_display_name}-r${RUN_SAFE_ID}-w${i}-a${j}")"
      agent_dir="$RUN_DIR/worlds/world-$i/agents/$agent_name"
      if [[ ! -d "$agent_dir/session" ]]; then
        echo "Error: missing agent session dir for resume at $agent_dir/session"
        exit 1
      fi
      if [[ ! -f "$agent_dir/world_session.txt" ]]; then
        echo "Error: missing world session file for resume at $agent_dir/world_session.txt"
        exit 1
      fi
    done
  done
fi

if [[ "$RECORD" == "true" && "$CLAWBLOX_SUPPORTS_RECORD_DIR" -ne 1 ]]; then
  echo "Error: '$CLAWBLOX_BIN run' does not support --record-dir."
  echo "Rebuild/update clawblox, or set RECORD=false."
  exit 1
fi

if [[ -z "${ANTHROPIC_API_KEY:-}" ]]; then
  echo "Error: ANTHROPIC_API_KEY is not set."
  echo "Add it to $ROOT_DIR/.env (or export it) before launching agents."
  exit 1
fi

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
  local i
  local port
  local waited=0
  local deadline=0
  local busy_ports=()
  local pids=()

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
  echo "Resolve busy ports, then rerun. If these are from a prior launch, run: $(basename "$0") --stop"
  exit 1
}

ensure_ports_available

if [[ -z "${RUN_DIR:-}" ]]; then
  RUN_DIR="$RESULTS_ABS_ROOT/$RUN_SAFE_ID"
fi
if [[ -z "${WORLDS_ROOT:-}" ]]; then
  WORLDS_ROOT="$RUN_DIR/worlds"
fi

if [[ -z "$RESUME_PATH" && -e "$RUN_DIR" ]]; then
  echo "Error: run directory already exists: $RUN_DIR"
  echo "Set RUN_ID to a unique value and retry."
  exit 1
fi

mkdir -p "$WORLDS_ROOT"
printf '%s\n' "$RUN_DIR" >"$RESULTS_ABS_ROOT/latest_run.txt"
write_run_metadata "$RUN_DIR/run.env"
write_run_metadata "$(session_meta_file)"

echo "Starting $NUM_WORLDS world servers (ports $BASE_PORT-$((BASE_PORT + NUM_WORLDS - 1)))..."

tmux new-session -d -s "$TMUX_SESSION" -n "world-0"
tmux set-option -t "$TMUX_SESSION" remain-on-exit on

WORLD0_ROOT="$WORLDS_ROOT/world-0"
WORLD0_LOG_DIR="$WORLD0_ROOT/logs"
WORLD0_LOG="$WORLD0_LOG_DIR/world.log"
WORLD0_RECORD_DIR="$WORLD0_ROOT/recordings"
WORLD0_AGENTS_DIR="$WORLD0_ROOT/agents"
WORLD0_RESUME_DIR="$WORLD0_ROOT/resume"
WORLD0_RESUME_FILE="$WORLD0_RESUME_DIR/latest.json"
mkdir -p "$WORLD0_LOG_DIR" "$WORLD0_RECORD_DIR" "$WORLD0_AGENTS_DIR" "$WORLD0_RESUME_DIR"
WORLD0_RESUME_ARG=""
if [[ -n "$RESUME_PATH" ]]; then
  WORLD0_RESUME_ARG="--resume '$WORLD0_RESUME_FILE' "
fi
if [[ "$RECORD" == "true" ]]; then
  tmux send-keys -t "${TMUX_SESSION}:world-0" \
    "cd '$ROOT_DIR' && '$CLAWBLOX_BIN' run '$WORLD_ABS_DIR' --port $BASE_PORT ${WORLD0_RESUME_ARG}--record --record-dir '$WORLD0_RECORD_DIR' 2>&1 | tee -a '$WORLD0_LOG'" Enter
else
  tmux send-keys -t "${TMUX_SESSION}:world-0" \
    "cd '$ROOT_DIR' && '$CLAWBLOX_BIN' run '$WORLD_ABS_DIR' --port $BASE_PORT ${WORLD0_RESUME_ARG}2>&1 | tee -a '$WORLD0_LOG'" Enter
fi

for ((i = 1; i < NUM_WORLDS; i++)); do
  port=$((BASE_PORT + i))
  world_root="$WORLDS_ROOT/world-$i"
  world_log_dir="$world_root/logs"
  world_log="$world_log_dir/world.log"
  world_record_dir="$world_root/recordings"
  world_agents_dir="$world_root/agents"
  world_resume_dir="$world_root/resume"
  world_resume_file="$world_resume_dir/latest.json"
  mkdir -p "$world_log_dir" "$world_record_dir" "$world_agents_dir" "$world_resume_dir"
  world_resume_arg=""
  if [[ -n "$RESUME_PATH" ]]; then
    world_resume_arg="--resume '$world_resume_file' "
  fi
  tmux new-window -t "$TMUX_SESSION" -n "world-$i"
  if [[ "$RECORD" == "true" ]]; then
    tmux send-keys -t "${TMUX_SESSION}:world-$i" \
      "cd '$ROOT_DIR' && '$CLAWBLOX_BIN' run '$WORLD_ABS_DIR' --port $port ${world_resume_arg}--record --record-dir '$world_record_dir' 2>&1 | tee -a '$world_log'" Enter
  else
    tmux send-keys -t "${TMUX_SESSION}:world-$i" \
      "cd '$ROOT_DIR' && '$CLAWBLOX_BIN' run '$WORLD_ABS_DIR' --port $port ${world_resume_arg}2>&1 | tee -a '$world_log'" Enter
  fi
done

echo "Waiting for servers to become healthy..."
ALL_HEALTHY=1
for ((i = 0; i < NUM_WORLDS; i++)); do
  port=$((BASE_PORT + i))
  printf "  world-%d (port %d)... " "$i" "$port"
  if wait_for_server "$port"; then
    echo "ready"
  else
    echo "TIMEOUT (${HEALTH_TIMEOUT}s)"
    ALL_HEALTHY=0
  fi
done

if [[ "$ALL_HEALTHY" -eq 0 ]]; then
  echo
  echo "Warning: some servers did not become healthy in time."
  echo "Agents will still be launched -- they may fail to join initially."
  echo
fi

echo "Launching $AGENTS_PER_WORLD agents per world..."

CHECKPOINT_ENV_PREFIX=""
if [[ -n "$AGENT_CHECKPOINT_AT_EPOCH" ]]; then
  CHECKPOINT_ENV_PREFIX+="AGENT_CHECKPOINT_AT_EPOCH=$(printf '%q' "$AGENT_CHECKPOINT_AT_EPOCH") "
  CHECKPOINT_ENV_PREFIX+="AGENT_CHECKPOINT_PROMPT=$(printf '%q' "$AGENT_CHECKPOINT_PROMPT") "
  CHECKPOINT_ENV_PREFIX+="AGENT_CHECKPOINT_TRIGGER_TURN=$(printf '%q' "$AGENT_CHECKPOINT_TRIGGER_TURN") "
  CHECKPOINT_ENV_PREFIX+="AGENT_CHECKPOINT_DISABLE_AUTO_LOOP=$(printf '%q' "$AGENT_CHECKPOINT_DISABLE_AUTO_LOOP") "
fi

for ((i = 0; i < NUM_WORLDS; i++)); do
  port=$((BASE_PORT + i))
  url="http://localhost:${port}"
  startup_instructions=""
  if [[ -n "$GOAL" ]]; then
    startup_instructions="$(cat <<EOF
First fetch capabilities with:
curl -sS -H "X-Session: <session-key>" ${url}/skill.md
Use the session key shown above in this prompt.

Then follow this objective:
${GOAL}
EOF
)"
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
    agent_session="$agent_dir/session"
    agent_log_file="$agent_log_dir/agent.log"
    agent_world_session_file="$agent_dir/world_session.txt"

    mkdir -p "$agent_log_dir" "$agent_workspace" "$agent_session"

    agent_template_abs_dir="$TEMPLATE_ABS_DIR"
    if ((j < ${#AGENT_TEMPLATE_ABS_DIRS[@]})); then
      agent_template_abs_dir="${AGENT_TEMPLATE_ABS_DIRS[$j]}"
    fi

    TEMPLATE_ENV_PREFIX=""
    if [[ -n "$agent_template_abs_dir" ]]; then
      TEMPLATE_ENV_PREFIX="TEMPLATE_DIR=$(printf '%q' "$agent_template_abs_dir") "
    fi

    RESET_ENV_PREFIX=""
    if [[ -n "$RESET_EVERY" ]]; then
      RESET_ENV_PREFIX="RESET_EVERY=$(printf '%q' "$RESET_EVERY") "
    fi

    MODEL_ENV_PREFIX=""
    if [[ -n "$PI_MODEL" ]]; then
      MODEL_ENV_PREFIX="PI_MODEL=$(printf '%q' "$PI_MODEL") "
    fi

    SANDBOX_ENV_PREFIX=""
    if [[ "$SANDBOX" == "1" ]]; then
      SANDBOX_ENV_PREFIX="SANDBOX=1 "
    fi

    if [[ -n "$startup_instructions" ]]; then
      startup_instructions_q="$(printf '%q' "$startup_instructions")"
      tmux pipe-pane -o -t "$pane_target" "cat >> '$(printf "%q" "$agent_log_file")'"
      tmux send-keys -t "$pane_target" \
        "cd '$ROOT_DIR' && WORLD_BASE_URL='$url' WORLD_AGENT_NAME='$agent_display_name' WORKSPACE_DIR='$agent_workspace' SESSION_DIR='$agent_session' WORLD_SESSION_FILE='$agent_world_session_file' ${TEMPLATE_ENV_PREFIX}${RESET_ENV_PREFIX}${MODEL_ENV_PREFIX}${CHECKPOINT_ENV_PREFIX}${SANDBOX_ENV_PREFIX}WORLD_STARTUP_INSTRUCTIONS=${startup_instructions_q} bash '$AGENT_SCRIPT'" Enter
    else
      tmux pipe-pane -o -t "$pane_target" "cat >> '$(printf "%q" "$agent_log_file")'"
      tmux send-keys -t "$pane_target" \
        "cd '$ROOT_DIR' && WORLD_BASE_URL='$url' WORLD_AGENT_NAME='$agent_display_name' WORKSPACE_DIR='$agent_workspace' SESSION_DIR='$agent_session' WORLD_SESSION_FILE='$agent_world_session_file' ${TEMPLATE_ENV_PREFIX}${RESET_ENV_PREFIX}${MODEL_ENV_PREFIX}${CHECKPOINT_ENV_PREFIX}${SANDBOX_ENV_PREFIX}bash '$AGENT_SCRIPT'" Enter
    fi
  done
done

TOTAL_AGENTS=$((NUM_WORLDS * AGENTS_PER_WORLD))
echo
echo "========================================"
echo "  Clawblox Multi-Instance Launcher"
echo "========================================"
echo "  Worlds:  $NUM_WORLDS (ports $BASE_PORT-$((BASE_PORT + NUM_WORLDS - 1)))"
echo "  Agents:  $TOTAL_AGENTS ($AGENTS_PER_WORLD per world)"
echo "  Record:  $RECORD"
echo "  Reset:   ${RESET_EVERY:-off}"
echo "  Checkpoint: $( ((CHECKPOINT_INTERVAL > 0)) && echo "${CHECKPOINT_INTERVAL}s" || echo "off" )"
echo "  Session: $TMUX_SESSION"
echo "  Results: $RUN_DIR"
echo "  Results Root: $RESULTS_ABS_ROOT"
echo "  World Data: $WORLDS_ROOT"
echo "========================================"
echo
echo "  tmux attach -t $TMUX_SESSION"
echo
echo "  Stop all:  $(basename "$0") --stop"
echo "  Status:    $(basename "$0") --status"
echo

# --- Periodic checkpoint loop ---
if ((CHECKPOINT_INTERVAL > 0)); then
  echo "Starting periodic checkpoints every ${CHECKPOINT_INTERVAL}s..."
  (
    while true; do
      sleep "$CHECKPOINT_INTERVAL"
      tmux has-session -t "$TMUX_SESSION" 2>/dev/null || break
      for ((i = 0; i < NUM_WORLDS; i++)); do
        save_world_snapshot "$i" "$((BASE_PORT + i))" || true
      done
    done
  ) &
  disown
fi
