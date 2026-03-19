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
OBJECTIVES="${OBJECTIVES:-highest,tower}"
COHORTS="${COHORTS:-experienced,scratch}"
REPEATS="${REPEATS:-1}"
BASE_PORT_START="${BASE_PORT_START:-8110}"
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
LEGACY_PORT_ARGS_USED=0

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

is_positive_int() {
  [[ "$1" =~ ^[1-9][0-9]*$ ]]
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

parse_csv_list() {
  local value="$1"
  local item
  local -a raw_items=()
  IFS=',' read -r -a raw_items <<< "$value"
  for item in "${raw_items[@]}"; do
    item="${item//[[:space:]]/}"
    [[ -n "$item" ]] || continue
    printf '%s\n' "$item"
  done
}

validate_objective() {
  case "$1" in
    highest|tower)
      ;;
    *)
      echo "Error: unsupported objective '$1'. Use highest or tower."
      exit 1
      ;;
  esac
}

validate_cohort() {
  case "$1" in
    experienced|scratch)
      ;;
    *)
      echo "Error: unsupported cohort '$1'. Use experienced or scratch."
      exit 1
      ;;
  esac
}

goal_for_objective() {
  case "$1" in
    highest)
      printf '%s\n' "Reach the highest point in the world as quickly as possible."
      ;;
    tower)
      printf '%s\n' "Build the largest stable tower possible as quickly as possible."
      ;;
    *)
      echo "Error: unsupported objective '$1'." >&2
      exit 1
      ;;
  esac
}

display_name_for_objective() {
  case "$1" in
    highest)
      printf '%s\n' "highest-point"
      ;;
    tower)
      printf '%s\n' "tower"
      ;;
    *)
      echo "Error: unsupported objective '$1'." >&2
      exit 1
      ;;
  esac
}

template_for_cohort() {
  case "$1" in
    experienced)
      printf '%s\n' "$TEMPLATE_ABS_WORKSPACE"
      ;;
    scratch)
      printf '%s\n' "$SCRATCH_ABS_TEMPLATE"
      ;;
    *)
      echo "Error: unsupported cohort '$1'." >&2
      exit 1
      ;;
  esac
}

legacy_port_for_task() {
  local objective="$1"
  local cohort="$2"
  case "${objective}:${cohort}" in
    highest:experienced)
      printf '%s\n' "$BASE_PORT_HIGHEST"
      ;;
    tower:experienced)
      printf '%s\n' "$BASE_PORT_TOWER"
      ;;
    highest:scratch)
      printf '%s\n' "$BASE_PORT_HIGHEST_SCRATCH"
      ;;
    tower:scratch)
      printf '%s\n' "$BASE_PORT_TOWER_SCRATCH"
      ;;
    *)
      echo "Error: unsupported legacy port mapping for ${objective}:${cohort}" >&2
      exit 1
      ;;
  esac
}

usage() {
  cat <<EOF
Usage: $(basename "$0") [options]

Launch one or more 1-agent eval runs in parallel across selected objectives,
cohorts, and repeat count.

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
  --objectives LIST             Comma-separated: highest,tower (default: highest,tower)
  --cohorts LIST                Comma-separated: experienced,scratch (default: experienced,scratch)
  --repeats N                   Runs per objective/cohort combo (default: 1)
  --base-port-start PORT        First port used for auto-assigned task ports (default: 8110)
  --record true|false           Forwarded to launch_multi.sh (default: true)
  --health-timeout SECONDS      Forwarded to launch_multi.sh (default: 30)
  --stop-grace-seconds N        Forwarded to launch_multi.sh (default: 5)
  --port-wait-seconds N         Forwarded to launch_multi.sh (default: 120)
  --agent-name-prefix PREFIX    Forwarded to launch_multi.sh (default: mesa)
  --base-port-highest PORT      Legacy single-run port override for highest/experienced
  --base-port-tower PORT        Legacy single-run port override for tower/experienced
  --base-port-highest-scratch PORT  Legacy single-run port override for highest/scratch
  --base-port-tower-scratch PORT    Legacy single-run port override for tower/scratch
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
    --objectives)
      require_value "$1" "${2:-}"
      OBJECTIVES="$2"
      shift 2
      ;;
    --cohorts)
      require_value "$1" "${2:-}"
      COHORTS="$2"
      shift 2
      ;;
    --repeats)
      require_value "$1" "${2:-}"
      REPEATS="$2"
      shift 2
      ;;
    --base-port-start)
      require_value "$1" "${2:-}"
      BASE_PORT_START="$2"
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
      LEGACY_PORT_ARGS_USED=1
      shift 2
      ;;
    --base-port-tower)
      require_value "$1" "${2:-}"
      BASE_PORT_TOWER="$2"
      LEGACY_PORT_ARGS_USED=1
      shift 2
      ;;
    --base-port-highest-scratch)
      require_value "$1" "${2:-}"
      BASE_PORT_HIGHEST_SCRATCH="$2"
      LEGACY_PORT_ARGS_USED=1
      shift 2
      ;;
    --base-port-tower-scratch)
      require_value "$1" "${2:-}"
      BASE_PORT_TOWER_SCRATCH="$2"
      LEGACY_PORT_ARGS_USED=1
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
if ! is_nonnegative_int "$BASE_PORT_START"; then
  echo "Error: --base-port-start must be a non-negative integer (got '$BASE_PORT_START')."
  exit 1
fi
if ! is_positive_int "$REPEATS"; then
  echo "Error: --repeats must be a positive integer (got '$REPEATS')."
  exit 1
fi

declare -a OBJECTIVE_LIST=()
mapfile -t OBJECTIVE_LIST < <(parse_csv_list "$OBJECTIVES")
if ((${#OBJECTIVE_LIST[@]} == 0)); then
  echo "Error: --objectives must include at least one value."
  exit 1
fi
declare -A seen_objectives=()
for objective in "${OBJECTIVE_LIST[@]}"; do
  validate_objective "$objective"
  if [[ -n "${seen_objectives[$objective]:-}" ]]; then
    echo "Error: duplicate objective '$objective' in --objectives."
    exit 1
  fi
  seen_objectives[$objective]=1
done

declare -a COHORT_LIST=()
mapfile -t COHORT_LIST < <(parse_csv_list "$COHORTS")
if ((${#COHORT_LIST[@]} == 0)); then
  echo "Error: --cohorts must include at least one value."
  exit 1
fi
declare -A seen_cohorts=()
for cohort in "${COHORT_LIST[@]}"; do
  validate_cohort "$cohort"
  if [[ -n "${seen_cohorts[$cohort]:-}" ]]; then
    echo "Error: duplicate cohort '$cohort' in --cohorts."
    exit 1
  fi
  seen_cohorts[$cohort]=1
done

if ((LEGACY_PORT_ARGS_USED == 1 && REPEATS != 1)); then
  echo "Error: legacy per-task port flags only support --repeats 1. Use --base-port-start for repeated runs."
  exit 1
fi

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
EVALS_ROOT="$EXPERIMENT_ABS_DIR/evals/$WORLD_NAME_SAFE"

declare -a TASK_NAMES=()
declare -a TASK_SESSIONS=()
declare -a TASK_PORTS=()
declare -a TASK_RESULTS_ROOTS=()
declare -a TASK_RUN_IDS=()
declare -a TASK_GOALS=()
declare -a TASK_TEMPLATES=()
declare -a TASK_RESULT_PATHS=()

task_counter=0
for objective in "${OBJECTIVE_LIST[@]}"; do
  objective_display="$(display_name_for_objective "$objective")"
  goal="$(goal_for_objective "$objective")"
  for cohort in "${COHORT_LIST[@]}"; do
    template_path="$(template_for_cohort "$cohort")"
    results_root="$EVALS_ROOT/$objective/$cohort"
    for ((repeat_index = 1; repeat_index <= REPEATS; repeat_index++)); do
      repeat_suffix="$(printf 'r%02d' "$repeat_index")"
      session_name="$(sanitize_name "${TMUX_PREFIX}-${objective}-${cohort}-${repeat_suffix}")"
      run_id="$(sanitize_name "${objective}-${cohort}-${RUN_TAG_SAFE}-${repeat_suffix}")"
      if ((LEGACY_PORT_ARGS_USED == 1)); then
        port="$(legacy_port_for_task "$objective" "$cohort")"
      else
        port=$((BASE_PORT_START + task_counter))
      fi
      TASK_NAMES+=("${objective_display} (${cohort}, run ${repeat_index})")
      TASK_SESSIONS+=("$session_name")
      TASK_PORTS+=("$port")
      TASK_RESULTS_ROOTS+=("$results_root")
      TASK_RUN_IDS+=("$run_id")
      TASK_GOALS+=("$goal")
      TASK_TEMPLATES+=("$template_path")
      TASK_RESULT_PATHS+=("$results_root/$run_id")
      task_counter=$((task_counter + 1))
    done
  done
done

declare -A seen_ports=()
for port in "${TASK_PORTS[@]}"; do
  if [[ -n "${seen_ports[$port]:-}" ]]; then
    echo "Error: duplicate task port $port. Adjust --base-port-start or legacy port flags."
    exit 1
  fi
  seen_ports[$port]=1
done

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
  local session_name
  for session_name in "${TASK_SESSIONS[@]}"; do
    stop_session_if_running "$session_name"
  done
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
echo "  Objectives: $(IFS=,; echo "${OBJECTIVE_LIST[*]}")"
echo "  Cohorts:    $(IFS=,; echo "${COHORT_LIST[*]}")"
echo "  Repeats:    $REPEATS"
echo "  Duration:   ${DURATION_SECONDS}s"
for ((i = 0; i < ${#TASK_NAMES[@]}; i++)); do
  echo "  Task $((i + 1)): ${TASK_NAMES[$i]} session=${TASK_SESSIONS[$i]} port=${TASK_PORTS[$i]} run_id=${TASK_RUN_IDS[$i]}"
done
echo

for ((i = 0; i < ${#TASK_NAMES[@]}; i++)); do
  launch_eval \
    "${TASK_NAMES[$i]}" \
    "${TASK_SESSIONS[$i]}" \
    "${TASK_PORTS[$i]}" \
    "${TASK_RESULTS_ROOTS[$i]}" \
    "${TASK_RUN_IDS[$i]}" \
    "${TASK_GOALS[$i]}" \
    "${TASK_TEMPLATES[$i]}"
done

echo "All evals are running. Sleeping for ${DURATION_SECONDS}s..."
sleep "$DURATION_SECONDS"

echo "Time elapsed; stopping eval sessions..."
for session_name in "${TASK_SESSIONS[@]}"; do
  stop_session_if_running "$session_name"
done
cleanup_done=1

echo
echo "Finished."
for ((i = 0; i < ${#TASK_RESULT_PATHS[@]}; i++)); do
  echo "  ${TASK_NAMES[$i]}: ${TASK_RESULT_PATHS[$i]}"
done
