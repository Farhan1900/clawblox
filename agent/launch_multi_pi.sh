#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

NUM_WORLDS="${NUM_WORLDS:-1}"
AGENTS_PER_WORLD="${AGENTS_PER_WORLD:-1}"
BASE_PORT="${BASE_PORT:-8080}"
TMUX_SESSION="${TMUX_SESSION:-clawblox-pi}"
RECORD="${RECORD:-true}"
WORLD_DIR="${WORLD_DIR:-}"
HEALTH_TIMEOUT="${HEALTH_TIMEOUT:-30}"
RUN_ID="${RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
LAUNCH_INSTANCE_ID="${CLAWBLOX_LAUNCH_INSTANCE_ID:-$(date -u +%Y%m%dT%H%M%SZ)-$$-$RANDOM}"
AGENT_NAME_PREFIX="${AGENT_NAME_PREFIX:-agent}"
STOP_GRACE_SECONDS="${STOP_GRACE_SECONDS:-10}"
GOAL="${GOAL:-}"
TEMPLATE_DIR="${TEMPLATE_DIR:-$ROOT_DIR/worlds/mesa-island/agent-template}"
RESULTS_ROOT="${RESULTS_ROOT:-}"
PORT_WAIT_SECONDS="${PORT_WAIT_SECONDS:-120}"
RESUME_PATH="${RESUME_PATH:-}"
RESET_EVERY="${RESET_EVERY:-}"
DURATION="${DURATION:-}"
SYSTEM_PROMPT_TEMPLATE="${SYSTEM_PROMPT_TEMPLATE:-}"
CHECKPOINT_WARNING_SECONDS="${CHECKPOINT_WARNING_SECONDS:-300}"
PI_PROVIDER="${PI_PROVIDER:-}"
PI_MODEL="${PI_MODEL:-}"
PI_EXTENSION_PATH="${PI_EXTENSION_PATH:-}"
PI_NO_GLOBAL_EXTENSIONS="${PI_NO_GLOBAL_EXTENSIONS:-1}"
EXPECTED_RUN_ID="${CLAWBLOX_EXPECT_RUN_ID:-}"
EXPECTED_LAUNCH_INSTANCE_ID="${CLAWBLOX_EXPECT_LAUNCH_INSTANCE_ID:-}"
CURRENT_AUTO_TIMER_PID="${CLAWBLOX_AUTO_TIMER_PID:-}"
AUTO_STOP_PID="${AUTO_STOP_PID:-}"

AGENT_SCRIPT="$SCRIPT_DIR/run_agent.sh"
if [[ ! -f "$AGENT_SCRIPT" ]]; then
  echo "Error: Pi agent runner not found at $AGENT_SCRIPT"
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

archive_existing_run_dir() {
  local run_dir="$1"
  local ts archive_dir
  ts="$(date -u +%Y%m%dT%H%M%SZ)"
  archive_dir="${run_dir}.preforce-${ts}"
  mv "$run_dir" "$archive_dir"
  echo "Archived existing run directory to: $archive_dir"
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
BACKEND=pi
EOF
}

write_session_metadata() {
  local target="$1"
  write_run_metadata "$target"
  cat >>"$target" <<EOF
LAUNCH_INSTANCE_ID=$(printf '%q' "$LAUNCH_INSTANCE_ID")
AUTO_STOP_PID=$(printf '%q' "$AUTO_STOP_PID")
EOF
}

load_session_metadata() {
  local meta_file="$1"
  local -n out_run_id_ref="$2"
  local -n out_run_dir_ref="$3"
  local -n out_num_worlds_ref="$4"
  local -n out_base_port_ref="$5"
  local -n out_launch_instance_ref="$6"
  local -n out_auto_stop_pid_ref="$7"
  local -a meta_fields=()

  out_run_id_ref=""
  out_run_dir_ref=""
  out_num_worlds_ref=""
  out_base_port_ref=""
  out_launch_instance_ref=""
  out_auto_stop_pid_ref=""

  [[ -f "$meta_file" ]] || return 1

  mapfile -t meta_fields < <(
    bash -lc '
      set -euo pipefail
      # shellcheck disable=SC1090
      source "$1"
      printf "%s\n%s\n%s\n%s\n%s\n%s\n" \
        "${RUN_ID:-}" \
        "${RUN_DIR:-}" \
        "${NUM_WORLDS:-}" \
        "${BASE_PORT:-}" \
        "${LAUNCH_INSTANCE_ID:-}" \
        "${AUTO_STOP_PID:-}"
    ' _ "$meta_file"
  )
  out_run_id_ref="${meta_fields[0]:-}"
  out_run_dir_ref="${meta_fields[1]:-}"
  out_num_worlds_ref="${meta_fields[2]:-}"
  out_base_port_ref="${meta_fields[3]:-}"
  out_launch_instance_ref="${meta_fields[4]:-}"
  out_auto_stop_pid_ref="${meta_fields[5]:-}"
}

kill_recorded_auto_stop_pid() {
  local meta_file="$1"
  local skip_pid="${2:-}"
  local meta_run_id meta_run_dir meta_num_worlds meta_base_port meta_launch_instance meta_auto_stop_pid
  local pid_cmd

  if ! load_session_metadata \
    "$meta_file" \
    meta_run_id \
    meta_run_dir \
    meta_num_worlds \
    meta_base_port \
    meta_launch_instance \
    meta_auto_stop_pid; then
    return 0
  fi

  [[ -n "$meta_auto_stop_pid" ]] || return 0
  if [[ -n "$skip_pid" && "$meta_auto_stop_pid" == "$skip_pid" ]]; then
    return 0
  fi
  if ! kill -0 "$meta_auto_stop_pid" 2>/dev/null; then
    return 0
  fi
  pid_cmd="$(ps -o args= -p "$meta_auto_stop_pid" 2>/dev/null | tr '\n' ' ' || true)"
  if [[ "$pid_cmd" != *"$(basename "$0")"* ]]; then
    return 0
  fi
  kill "$meta_auto_stop_pid" 2>/dev/null || true
}

world_root_for_index() {
  local world_index="$1"
  printf '%s/worlds/world-%s' "$RUN_DIR" "$world_index"
}

world_log_for_index() {
  local world_index="$1"
  printf '%s/logs/world.log' "$(world_root_for_index "$world_index")"
}

world_spectator_token_file() {
  local world_index="$1"
  printf '%s/spectator_token.txt' "$(world_root_for_index "$world_index")"
}

extract_spectator_token_from_log() {
  local world_log="$1"
  [[ -f "$world_log" ]] || return 1
  sed -n 's|^Spectator frontend: .*spectator_token=\([A-Za-z0-9-][A-Za-z0-9-]*\)$|\1|p' "$world_log" | tail -n 1
}

extract_spectator_token_from_tmux_pane() {
  local world_index="$1"
  tmux capture-pane -J -p -S -2000 -t "${TMUX_SESSION}:world-${world_index}" 2>/dev/null \
    | sed -n 's|^Spectator frontend: .*spectator_token=\([A-Za-z0-9-][A-Za-z0-9-]*\)$|\1|p' \
    | tail -n 1
}

cache_world_spectator_token() {
  local world_index="$1"
  local token_file token
  token_file="$(world_spectator_token_file "$world_index")"
  token="$(extract_spectator_token_from_log "$(world_log_for_index "$world_index")")" || true
  if [[ -z "$token" ]]; then
    token="$(extract_spectator_token_from_tmux_pane "$world_index")" || return 1
  fi
  [[ -n "$token" ]] || return 1
  printf '%s\n' "$token" >"$token_file"
}

load_world_spectator_token() {
  local world_index="$1"
  local token_file
  token_file="$(world_spectator_token_file "$world_index")"
  if [[ -f "$token_file" ]]; then
    tr -d '[:space:]' <"$token_file"
    return 0
  fi
  cache_world_spectator_token "$world_index" >/dev/null 2>&1 || return 1
  tr -d '[:space:]' <"$token_file"
}

save_world_snapshot() {
  local world_index="$1"
  local port="$2"
  local world_root spectator_token="" tmp_file snapshot_file resume_dir
  local -a curl_args=()
  world_root="$(world_root_for_index "$world_index")"
  resume_dir="$world_root/resume"
  snapshot_file="$resume_dir/latest.json"
  tmp_file="$snapshot_file.tmp"
  mkdir -p "$resume_dir"
  if spectator_token="$(load_world_spectator_token "$world_index" 2>/dev/null)"; then
    curl_args=(-H "X-Spectator-Token: $spectator_token")
  fi
  if curl -fsS "${curl_args[@]}" "http://localhost:${port}/snapshot" >"$tmp_file"; then
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

Launch a clawblox world with Pi agents in tmux.
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

require_pi_runtime() {
  if ! command -v pi >/dev/null 2>&1; then
    echo "Error: pi CLI was not found in PATH."
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
  local meta_file i port snapshot_failed=0 meta_run_id="" meta_run_dir="" meta_num_worlds="" meta_base_port="" meta_launch_instance_id="" meta_auto_stop_pid=""
  if tmux has-session -t "$TMUX_SESSION" 2>/dev/null; then
    mapfile -t recordings < <(collect_session_recordings)
    meta_file="$(session_meta_file)"
    if load_session_metadata \
      "$meta_file" \
      meta_run_id \
      meta_run_dir \
      meta_num_worlds \
      meta_base_port \
      meta_launch_instance_id \
      meta_auto_stop_pid; then
      if [[ -n "$EXPECTED_RUN_ID" && "$meta_run_id" != "$EXPECTED_RUN_ID" ]]; then
        echo "Skipping stop for '$TMUX_SESSION': expected run '$EXPECTED_RUN_ID', current owner is '${meta_run_id:-unknown}'."
        return 0
      fi
      if [[ -n "$EXPECTED_LAUNCH_INSTANCE_ID" && "$meta_launch_instance_id" != "$EXPECTED_LAUNCH_INSTANCE_ID" ]]; then
        echo "Skipping stop for '$TMUX_SESSION': expected launch instance '$EXPECTED_LAUNCH_INSTANCE_ID', current owner is '${meta_launch_instance_id:-unknown}'."
        return 0
      fi
      kill_recorded_auto_stop_pid "$meta_file" "$CURRENT_AUTO_TIMER_PID"
      if [[ -n "$meta_run_dir" && -n "$meta_num_worlds" && -n "$meta_base_port" ]]; then
        echo "Saving world snapshots..."
        for ((i = 0; i < meta_num_worlds; i++)); do
          port=$((meta_base_port + i))
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
    if curl -sf "http://localhost:${port}/api.md" >/dev/null 2>&1; then
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
    --template|--template-dir) require_value "$1" "${2:-}"; TEMPLATE_DIR="$2"; shift 2 ;;
    --system-prompt) require_value "$1" "${2:-}"; SYSTEM_PROMPT_TEMPLATE="$2"; shift 2 ;;
    --results-root) require_value "$1" "${2:-}"; RESULTS_ROOT="$2"; shift 2 ;;
    --resume) require_value "$1" "${2:-}"; RESUME_PATH="$2"; shift 2 ;;
    --model|--pi-model) require_value "$1" "${2:-}"; PI_MODEL="$2"; shift 2 ;;
    --provider|--pi-provider) require_value "$1" "${2:-}"; PI_PROVIDER="$2"; shift 2 ;;
    --pi-extension) require_value "$1" "${2:-}"; PI_EXTENSION_PATH="$2"; shift 2 ;;
    --allow-global-extensions) PI_NO_GLOBAL_EXTENSIONS=0; shift ;;
    --no-global-extensions) PI_NO_GLOBAL_EXTENSIONS=1; shift ;;
    --sandbox|--use-env-auth) shift ;;
    --help|-h) usage; exit 0 ;;
    *) echo "Unknown option: $1"; usage; exit 1 ;;
  esac
done

case "$ACTION" in
  stop) do_stop; exit 0 ;;
  status) do_status; exit 0 ;;
esac

resolve_clawblox_bin
require_pi_runtime

if tmux has-session -t "$TMUX_SESSION" 2>/dev/null; then
  if [[ "$FORCE" -eq 1 ]]; then
    echo "Replacing existing session '$TMUX_SESSION'..."
    kill_recorded_auto_stop_pid "$(session_meta_file)"
    do_stop
  else
    echo "Tmux session '$TMUX_SESSION' already exists."
    exit 1
  fi
fi

if [[ -f "$(session_meta_file)" ]]; then
  kill_recorded_auto_stop_pid "$(session_meta_file)"
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

if [[ -n "$SYSTEM_PROMPT_TEMPLATE" && "$SYSTEM_PROMPT_TEMPLATE" != /* ]]; then
  SYSTEM_PROMPT_TEMPLATE="$ROOT_DIR/$SYSTEM_PROMPT_TEMPLATE"
fi

if [[ -z "$SYSTEM_PROMPT_TEMPLATE" ]]; then
  if [[ -f "$WORLD_ABS_DIR/system_prompt.md" ]]; then
    SYSTEM_PROMPT_TEMPLATE="$WORLD_ABS_DIR/system_prompt.md"
  elif [[ -f "$TEMPLATE_ABS_DIR/system_prompt.md" ]]; then
    SYSTEM_PROMPT_TEMPLATE="$TEMPLATE_ABS_DIR/system_prompt.md"
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

if [[ -z "$RESUME_PATH" && -e "$RUN_DIR" ]]; then
  if ((FORCE == 1)); then
    archive_existing_run_dir "$RUN_DIR"
  else
    echo "Error: run directory already exists: $RUN_DIR"
    echo "Set RUN_ID to a unique value and retry."
    exit 1
  fi
fi

ensure_ports_available
mkdir -p "$WORLDS_ROOT"
printf '%s\n' "$RUN_DIR" >"$RESULTS_ABS_ROOT/latest_run.txt"
write_run_metadata "$RUN_DIR/run.env"
write_session_metadata "$(session_meta_file)"

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
    cache_world_spectator_token "$i" >/dev/null 2>&1 || true
    echo "ready"
  else
    echo "TIMEOUT (${HEALTH_TIMEOUT}s)"
  fi
done

DURATION_SECONDS=""
if [[ -n "$DURATION" ]]; then
  DURATION_SECONDS="$(parse_duration_seconds "$DURATION")"
fi

RESET_EVERY_SECONDS=""
if [[ -n "$RESET_EVERY" ]]; then
  RESET_EVERY_SECONDS="$(parse_duration_seconds "$RESET_EVERY")"
fi

echo "Launching $AGENTS_PER_WORLD Pi agents per world..."
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
    agent_session_dir="$agent_dir/session"
    agent_log_file="$agent_log_dir/agent.log"
    mkdir -p "$agent_log_dir" "$agent_workspace" "$agent_session_dir"

    template_env_prefix="TEMPLATE_DIR=$(printf '%q' "$TEMPLATE_ABS_DIR") "
    prompt_env_prefix=""
    if [[ -n "$SYSTEM_PROMPT_TEMPLATE" ]]; then
      prompt_env_prefix="SYSTEM_PROMPT_TEMPLATE=$(printf '%q' "$SYSTEM_PROMPT_TEMPLATE") "
    fi
    startup_env_prefix=""
    if [[ -n "$startup_instructions" ]]; then
      startup_instructions_q="$(printf '%q' "$startup_instructions")"
      startup_env_prefix="WORLD_STARTUP_INSTRUCTIONS=${startup_instructions_q} "
    fi
    provider_env_prefix="PI_PROVIDER=$(printf '%q' "$PI_PROVIDER") "
    model_env_prefix="PI_MODEL=$(printf '%q' "$PI_MODEL") "
    extension_env_prefix="PI_EXTENSION_PATH=$(printf '%q' "$PI_EXTENSION_PATH") PI_NO_GLOBAL_EXTENSIONS=$(printf '%q' "$PI_NO_GLOBAL_EXTENSIONS") "

    tmux pipe-pane -o -t "$pane_target" "cat >> '$(printf "%q" "$agent_log_file")'"
    tmux send-keys -t "$pane_target" \
      "cd '$ROOT_DIR' && WORLD_BASE_URL='$url' WORLD_AGENT_NAME='$agent_display_name' WORKSPACE_DIR='$agent_workspace' SESSION_DIR='$agent_session_dir' RESET_EVERY=$(printf '%q' "$RESET_EVERY_SECONDS") DURATION_SECONDS=$(printf '%q' "$DURATION_SECONDS") CHECKPOINT_WARNING_SECONDS=$(printf '%q' "$CHECKPOINT_WARNING_SECONDS") ${provider_env_prefix}${model_env_prefix}${extension_env_prefix}${template_env_prefix}${prompt_env_prefix}${startup_env_prefix}bash '$AGENT_SCRIPT'" Enter
  done
done

if [[ -n "$DURATION" ]]; then
  duration_secs="$(parse_duration_seconds "$DURATION")"
  total_wait=$(( duration_secs + CHECKPOINT_WARNING_SECONDS + 30 ))
  echo "Run will auto-stop after ${DURATION} (${duration_secs}s + ${CHECKPOINT_WARNING_SECONDS}s warning)..."
  (
    expected_run_id="$RUN_ID"
    expected_launch_instance_id="$LAUNCH_INSTANCE_ID"
    sleep "$total_wait"
    echo "Duration ${DURATION} + warning elapsed. Stopping..."
    CLAWBLOX_EXPECT_RUN_ID="$expected_run_id" \
      CLAWBLOX_EXPECT_LAUNCH_INSTANCE_ID="$expected_launch_instance_id" \
      CLAWBLOX_AUTO_TIMER_PID="$BASHPID" \
      bash "$0" --stop --tmux-session "$TMUX_SESSION"
  ) &
  AUTO_STOP_PID="$!"
  write_session_metadata "$(session_meta_file)"
  disown
fi

echo "tmux attach -t $TMUX_SESSION"
