#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
LAUNCH_SCRIPT="${LAUNCH_SCRIPT:-$SCRIPT_DIR/launch_multi.sh}"

EXPERIMENT_DIR="${EXPERIMENT_DIR:-}"
TEMPLATE_WORKSPACE="${TEMPLATE_WORKSPACE:-}"
SCRATCH_TEMPLATE="${SCRATCH_TEMPLATE:-$ROOT_DIR/agent/template/agent}"
DURATION="${DURATION:-1h}"
RUN_TAG="${RUN_TAG:-$(date -u +%Y%m%dT%H%M%SZ)}"
TMUX_PREFIX="${TMUX_PREFIX:-mesa-eval}"
WORLD_DIR="${WORLD_DIR:-worlds/mesa-world}"
RECORD="${RECORD:-true}"
HEALTH_TIMEOUT="${HEALTH_TIMEOUT:-30}"
STOP_GRACE_SECONDS="${STOP_GRACE_SECONDS:-5}"
PORT_WAIT_SECONDS="${PORT_WAIT_SECONDS:-120}"
AGENT_NAME_PREFIX="${AGENT_NAME_PREFIX:-mesa}"
BASE_PORT_HIGHEST="${BASE_PORT_HIGHEST:-8110}"
BASE_PORT_TOWER="${BASE_PORT_TOWER:-8111}"
BASE_PORT_HIGHEST_SCRATCH="${BASE_PORT_HIGHEST_SCRATCH:-8112}"
BASE_PORT_TOWER_SCRATCH="${BASE_PORT_TOWER_SCRATCH:-8113}"
CLAWBLOX_BIN="${CLAWBLOX_BIN:-}"

require_value() {
  local opt="$1"
  local maybe_value="${2:-}"
  if [[ -z "$maybe_value" ]]; then
    echo "Error: $opt requires a value."
    usage
    exit 1
  fi
}

sanitize_name() {
  printf '%s' "$1" | tr -c 'A-Za-z0-9._-' '_'
}

is_nonnegative_int() {
  [[ "$1" =~ ^[0-9]+$ ]]
}

parse_duration_seconds() {
  local value="$1"
  if [[ ! "$value" =~ ^([0-9]+)([smh]?)$ ]]; then
    echo "Error: invalid duration '$value'. Use 3600, 60m, or 1h." >&2
    exit 1
  fi

  local amount="${BASH_REMATCH[1]}"
  local unit="${BASH_REMATCH[2]}"
  case "$unit" in
    ""|"s")
      printf '%s\n' "$amount"
      ;;
    "m")
      printf '%s\n' $((amount * 60))
      ;;
    "h")
      printf '%s\n' $((amount * 3600))
      ;;
    *)
      echo "Error: unsupported duration unit in '$value'." >&2
      exit 1
      ;;
  esac
}

to_abs_path() {
  local value="$1"
  if [[ "$value" = /* ]]; then
    printf '%s\n' "$value"
  else
    printf '%s\n' "$ROOT_DIR/$value"
  fi
}

usage() {
  cat <<EOF
Usage: $(basename "$0") [options]

Launch four 1-agent eval runs in parallel:
  1) experienced workspace: highest point as quickly as possible
  2) experienced workspace: largest stable tower as quickly as possible
  3) scratch template: highest point as quickly as possible
  4) scratch template: largest stable tower as quickly as possible

All runs auto-stop after --duration.
Results are saved under <experiment-dir>/evals/<world-name>/...

Required:
  --experiment-dir PATH         Existing experiment dir (e.g., results/genexp-... )
  --template-workspace PATH     Experienced template workspace to copy into each eval agent

Options:
  --scratch-template PATH       Template for "from scratch" baseline (default: agent/template/agent)
  --duration D                  Runtime per eval (default: 1h; formats: 3600, 60m, 1h)
  --run-tag TAG                 Tag used in run ids (default: UTC timestamp)
  --tmux-prefix PREFIX          Prefix for tmux sessions (default: mesa-eval)
  --world-dir PATH              World dir forwarded to launch_multi.sh
  --record true|false           Forwarded to launch_multi.sh (default: true)
  --health-timeout SECONDS      Forwarded to launch_multi.sh (default: 30)
  --stop-grace-seconds N        Forwarded to launch_multi.sh (default: 5)
  --port-wait-seconds N         Forwarded to launch_multi.sh (default: 120)
  --agent-name-prefix PREFIX    Forwarded to launch_multi.sh (default: mesa)
  --base-port-highest PORT      Base port for experienced highest eval (default: 8110)
  --base-port-tower PORT        Base port for experienced tower eval (default: 8111)
  --base-port-highest-scratch PORT  Base port for scratch highest eval (default: 8112)
  --base-port-tower-scratch PORT    Base port for scratch tower eval (default: 8113)
  --clawblox-bin PATH           Forwarded to launch_multi.sh
  --help                        Show this message
EOF
}

if [[ ! -f "$LAUNCH_SCRIPT" ]]; then
  echo "Error: launch script not found at $LAUNCH_SCRIPT"
  exit 1
fi

while [[ $# -gt 0 ]]; do
  case "$1" in
    --experiment-dir)
      require_value "$1" "${2:-}"
      EXPERIMENT_DIR="$2"
      shift 2
      ;;
    --template-workspace)
      require_value "$1" "${2:-}"
      TEMPLATE_WORKSPACE="$2"
      shift 2
      ;;
    --duration)
      require_value "$1" "${2:-}"
      DURATION="$2"
      shift 2
      ;;
    --scratch-template)
      require_value "$1" "${2:-}"
      SCRATCH_TEMPLATE="$2"
      shift 2
      ;;
    --run-tag)
      require_value "$1" "${2:-}"
      RUN_TAG="$2"
      shift 2
      ;;
    --tmux-prefix)
      require_value "$1" "${2:-}"
      TMUX_PREFIX="$2"
      shift 2
      ;;
    --world-dir)
      require_value "$1" "${2:-}"
      WORLD_DIR="$2"
      shift 2
      ;;
    --record)
      require_value "$1" "${2:-}"
      RECORD="$2"
      shift 2
      ;;
    --health-timeout)
      require_value "$1" "${2:-}"
      HEALTH_TIMEOUT="$2"
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
    --agent-name-prefix)
      require_value "$1" "${2:-}"
      AGENT_NAME_PREFIX="$2"
      shift 2
      ;;
    --base-port-highest)
      require_value "$1" "${2:-}"
      BASE_PORT_HIGHEST="$2"
      shift 2
      ;;
    --base-port-tower)
      require_value "$1" "${2:-}"
      BASE_PORT_TOWER="$2"
      shift 2
      ;;
    --base-port-highest-scratch)
      require_value "$1" "${2:-}"
      BASE_PORT_HIGHEST_SCRATCH="$2"
      shift 2
      ;;
    --base-port-tower-scratch)
      require_value "$1" "${2:-}"
      BASE_PORT_TOWER_SCRATCH="$2"
      shift 2
      ;;
    --clawblox-bin)
      require_value "$1" "${2:-}"
      CLAWBLOX_BIN="$2"
      shift 2
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

if [[ -z "$EXPERIMENT_DIR" || -z "$TEMPLATE_WORKSPACE" ]]; then
  echo "Error: --experiment-dir and --template-workspace are required."
  usage
  exit 1
fi

if ! is_nonnegative_int "$HEALTH_TIMEOUT"; then
  echo "Error: --health-timeout must be a non-negative integer (got '$HEALTH_TIMEOUT')."
  exit 1
fi
if ! is_nonnegative_int "$STOP_GRACE_SECONDS"; then
  echo "Error: --stop-grace-seconds must be a non-negative integer (got '$STOP_GRACE_SECONDS')."
  exit 1
fi
if ! is_nonnegative_int "$PORT_WAIT_SECONDS"; then
  echo "Error: --port-wait-seconds must be a non-negative integer (got '$PORT_WAIT_SECONDS')."
  exit 1
fi
if ! is_nonnegative_int "$BASE_PORT_HIGHEST"; then
  echo "Error: --base-port-highest must be a non-negative integer (got '$BASE_PORT_HIGHEST')."
  exit 1
fi
if ! is_nonnegative_int "$BASE_PORT_TOWER"; then
  echo "Error: --base-port-tower must be a non-negative integer (got '$BASE_PORT_TOWER')."
  exit 1
fi
if ! is_nonnegative_int "$BASE_PORT_HIGHEST_SCRATCH"; then
  echo "Error: --base-port-highest-scratch must be a non-negative integer (got '$BASE_PORT_HIGHEST_SCRATCH')."
  exit 1
fi
if ! is_nonnegative_int "$BASE_PORT_TOWER_SCRATCH"; then
  echo "Error: --base-port-tower-scratch must be a non-negative integer (got '$BASE_PORT_TOWER_SCRATCH')."
  exit 1
fi
ports=("$BASE_PORT_HIGHEST" "$BASE_PORT_TOWER" "$BASE_PORT_HIGHEST_SCRATCH" "$BASE_PORT_TOWER_SCRATCH")
for ((i = 0; i < ${#ports[@]}; i++)); do
  for ((j = i + 1; j < ${#ports[@]}; j++)); do
    if [[ "${ports[$i]}" == "${ports[$j]}" ]]; then
      echo "Error: all base ports must be unique (found duplicate: ${ports[$i]})."
      exit 1
    fi
  done
done

DURATION_SECONDS="$(parse_duration_seconds "$DURATION")"
if [[ "$DURATION_SECONDS" -le 0 ]]; then
  echo "Error: duration must be > 0."
  exit 1
fi

WORLD_ABS_DIR="$(to_abs_path "$WORLD_DIR")"
if [[ ! -d "$WORLD_ABS_DIR" ]]; then
  echo "Error: world directory not found at $WORLD_ABS_DIR"
  exit 1
fi
WORLD_NAME_SAFE="$(sanitize_name "$(basename "$WORLD_ABS_DIR")")"

EXPERIMENT_ABS_DIR="$(to_abs_path "$EXPERIMENT_DIR")"
TEMPLATE_ABS_WORKSPACE="$(to_abs_path "$TEMPLATE_WORKSPACE")"
SCRATCH_ABS_TEMPLATE="$(to_abs_path "$SCRATCH_TEMPLATE")"
if [[ ! -d "$EXPERIMENT_ABS_DIR" ]]; then
  echo "Error: experiment directory not found at $EXPERIMENT_ABS_DIR"
  exit 1
fi
if [[ ! -d "$TEMPLATE_ABS_WORKSPACE" ]]; then
  echo "Error: template workspace not found at $TEMPLATE_ABS_WORKSPACE"
  exit 1
fi
if [[ ! -d "$SCRATCH_ABS_TEMPLATE" ]]; then
  echo "Error: scratch template directory not found at $SCRATCH_ABS_TEMPLATE"
  exit 1
fi

RUN_TAG_SAFE="$(sanitize_name "$RUN_TAG")"
HIGHEST_EXP_SESSION="$(sanitize_name "${TMUX_PREFIX}-highest-exp")"
TOWER_EXP_SESSION="$(sanitize_name "${TMUX_PREFIX}-tower-exp")"
HIGHEST_SCRATCH_SESSION="$(sanitize_name "${TMUX_PREFIX}-highest-scratch")"
TOWER_SCRATCH_SESSION="$(sanitize_name "${TMUX_PREFIX}-tower-scratch")"

HIGHEST_EXP_RUN_ID="$(sanitize_name "highest-exp-${RUN_TAG_SAFE}")"
TOWER_EXP_RUN_ID="$(sanitize_name "tower-exp-${RUN_TAG_SAFE}")"
HIGHEST_SCRATCH_RUN_ID="$(sanitize_name "highest-scratch-${RUN_TAG_SAFE}")"
TOWER_SCRATCH_RUN_ID="$(sanitize_name "tower-scratch-${RUN_TAG_SAFE}")"

EVALS_ROOT="$EXPERIMENT_ABS_DIR/evals/$WORLD_NAME_SAFE"
HIGHEST_EXP_RESULTS_ROOT="$EVALS_ROOT/highest/experienced"
TOWER_EXP_RESULTS_ROOT="$EVALS_ROOT/tower/experienced"
HIGHEST_SCRATCH_RESULTS_ROOT="$EVALS_ROOT/highest/scratch"
TOWER_SCRATCH_RESULTS_ROOT="$EVALS_ROOT/tower/scratch"

cleanup_done=0
stop_session_if_running() {
  local session_name="$1"
  if tmux has-session -t "$session_name" 2>/dev/null; then
    bash "$LAUNCH_SCRIPT" --tmux-session "$session_name" --stop --stop-grace-seconds "$STOP_GRACE_SECONDS" || true
  fi
}

cleanup() {
  if ((cleanup_done == 1)); then
    return
  fi
  cleanup_done=1
  stop_session_if_running "$HIGHEST_EXP_SESSION"
  stop_session_if_running "$TOWER_EXP_SESSION"
  stop_session_if_running "$HIGHEST_SCRATCH_SESSION"
  stop_session_if_running "$TOWER_SCRATCH_SESSION"
}
trap cleanup EXIT INT TERM

launch_eval() {
  local name="$1"
  local session="$2"
  local port="$3"
  local results_root="$4"
  local run_id="$5"
  local goal="$6"
  local template_path="$7"

  echo "Launching $name eval..."
  local -a cmd=(
    bash "$LAUNCH_SCRIPT"
    --force
    --num-worlds 1
    --agents-per-world 1
    --base-port "$port"
    --tmux-session "$session"
    --world-dir "$WORLD_DIR"
    --record "$RECORD"
    --health-timeout "$HEALTH_TIMEOUT"
    --stop-grace-seconds "$STOP_GRACE_SECONDS"
    --port-wait-seconds "$PORT_WAIT_SECONDS"
    --agent-name-prefix "$AGENT_NAME_PREFIX"
    --results-root "$results_root"
    --run-id "$run_id"
    --template "$template_path"
    --goal "$goal"
  )
  if [[ -n "$CLAWBLOX_BIN" ]]; then
    cmd+=(--clawblox-bin "$CLAWBLOX_BIN")
  fi
  "${cmd[@]}"
}

echo "Starting parallel evals"
echo "  Experiment: $EXPERIMENT_ABS_DIR"
echo "  World dir:  $WORLD_ABS_DIR"
echo "  Evals root: $EVALS_ROOT"
echo "  Experienced template: $TEMPLATE_ABS_WORKSPACE"
echo "  Scratch template:     $SCRATCH_ABS_TEMPLATE"
echo "  Duration:   ${DURATION_SECONDS}s"
echo "  Highest exp:     session=$HIGHEST_EXP_SESSION port=$BASE_PORT_HIGHEST run_id=$HIGHEST_EXP_RUN_ID"
echo "  Tower exp:       session=$TOWER_EXP_SESSION port=$BASE_PORT_TOWER run_id=$TOWER_EXP_RUN_ID"
echo "  Highest scratch: session=$HIGHEST_SCRATCH_SESSION port=$BASE_PORT_HIGHEST_SCRATCH run_id=$HIGHEST_SCRATCH_RUN_ID"
echo "  Tower scratch:   session=$TOWER_SCRATCH_SESSION port=$BASE_PORT_TOWER_SCRATCH run_id=$TOWER_SCRATCH_RUN_ID"
echo

launch_eval \
  "highest-point (experienced)" \
  "$HIGHEST_EXP_SESSION" \
  "$BASE_PORT_HIGHEST" \
  "$HIGHEST_EXP_RESULTS_ROOT" \
  "$HIGHEST_EXP_RUN_ID" \
  "Reach the highest point in the world as quickly as possible." \
  "$TEMPLATE_ABS_WORKSPACE"

launch_eval \
  "tower (experienced)" \
  "$TOWER_EXP_SESSION" \
  "$BASE_PORT_TOWER" \
  "$TOWER_EXP_RESULTS_ROOT" \
  "$TOWER_EXP_RUN_ID" \
  "Build the largest stable tower possible as quickly as possible." \
  "$TEMPLATE_ABS_WORKSPACE"

launch_eval \
  "highest-point (scratch)" \
  "$HIGHEST_SCRATCH_SESSION" \
  "$BASE_PORT_HIGHEST_SCRATCH" \
  "$HIGHEST_SCRATCH_RESULTS_ROOT" \
  "$HIGHEST_SCRATCH_RUN_ID" \
  "Reach the highest point in the world as quickly as possible." \
  "$SCRATCH_ABS_TEMPLATE"

launch_eval \
  "tower (scratch)" \
  "$TOWER_SCRATCH_SESSION" \
  "$BASE_PORT_TOWER_SCRATCH" \
  "$TOWER_SCRATCH_RESULTS_ROOT" \
  "$TOWER_SCRATCH_RUN_ID" \
  "Build the largest stable tower possible as quickly as possible." \
  "$SCRATCH_ABS_TEMPLATE"

echo "All evals are running. Sleeping for ${DURATION_SECONDS}s..."
sleep "$DURATION_SECONDS"

echo "Time elapsed; stopping eval sessions..."
stop_session_if_running "$HIGHEST_EXP_SESSION"
stop_session_if_running "$TOWER_EXP_SESSION"
stop_session_if_running "$HIGHEST_SCRATCH_SESSION"
stop_session_if_running "$TOWER_SCRATCH_SESSION"
cleanup_done=1

echo
echo "Finished."
echo "  Highest exp:     $HIGHEST_EXP_RESULTS_ROOT/$HIGHEST_EXP_RUN_ID"
echo "  Tower exp:       $TOWER_EXP_RESULTS_ROOT/$TOWER_EXP_RUN_ID"
echo "  Highest scratch: $HIGHEST_SCRATCH_RESULTS_ROOT/$HIGHEST_SCRATCH_RUN_ID"
echo "  Tower scratch:   $TOWER_SCRATCH_RESULTS_ROOT/$TOWER_SCRATCH_RUN_ID"
