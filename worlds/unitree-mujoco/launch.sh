#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
TMUX_SESSION="${TMUX_SESSION:-unitree}"
PORT="${PORT:-8080}"
INTERFACE="${INTERFACE:-docker0}"
CONTAINER="${CONTAINER:-g1-bricklaying}"
COMPOSE_DIR="$SCRIPT_DIR/G1-Bricklaying-Simulation"

# Load API key
if [[ -z "${ANTHROPIC_API_KEY:-}" && -f "$ROOT_DIR/.env" ]]; then
  set -a
  source "$ROOT_DIR/.env"
  set +a
fi

usage() {
  cat <<EOF
Usage: $(basename "$0") [--stop|--status] [options]

Launches the Unitree G1 MuJoCo simulation with HTTP bridge and Claude agent.
All components run inside the Docker container '$CONTAINER'.

Options:
  --stop                Stop the session
  --status              Show session info
  --port PORT           Bridge server port (default: 8080)
  --interface IFACE     DDS network interface (default: docker0)
  --tmux-session NAME   Tmux session name (default: unitree)
  --no-agent            Skip launching the agent
EOF
}

LAUNCH_AGENT=1
ACTION="run"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --stop)    ACTION="stop"; shift ;;
    --status)  ACTION="status"; shift ;;
    --port)    PORT="$2"; shift 2 ;;
    --interface) INTERFACE="$2"; shift 2 ;;
    --tmux-session) TMUX_SESSION="$2"; shift 2 ;;
    --no-agent) LAUNCH_AGENT=0; shift ;;
    --help|-h) usage; exit 0 ;;
    *) echo "Unknown option: $1"; usage; exit 1 ;;
  esac
done

if [[ "$ACTION" == "stop" ]]; then
  # Kill tmux session
  if tmux has-session -t "$TMUX_SESSION" 2>/dev/null; then
    echo "Killing tmux session '$TMUX_SESSION'..."
    tmux kill-session -t "$TMUX_SESSION" 2>/dev/null || true
  fi
  # Restart container to kill ALL processes and clear zombies
  if docker ps --format '{{.Names}}' 2>/dev/null | grep -q "^${CONTAINER}$"; then
    echo "Restarting container to kill all processes..."
    cd "$COMPOSE_DIR" && docker compose restart >/dev/null 2>&1
    sleep 5
  fi
  echo "Stopped."
  exit 0
fi

if [[ "$ACTION" == "status" ]]; then
  if tmux has-session -t "$TMUX_SESSION" 2>/dev/null; then
    echo "Session: $TMUX_SESSION"
    tmux list-windows -t "$TMUX_SESSION" -F "  #{window_index}: #{window_name}"
    echo
    echo "Attach: tmux attach -t $TMUX_SESSION"
  else
    echo "No session '$TMUX_SESSION' running."
  fi
  exit 0
fi

if tmux has-session -t "$TMUX_SESSION" 2>/dev/null; then
  echo "Session '$TMUX_SESSION' already exists. Use --stop first."
  exit 1
fi

# Ensure clean container
echo "Starting Docker container..."
cd "$COMPOSE_DIR"
docker compose up -d --remove-orphans 2>&1 | grep -v "^$"
echo "Restarting container for clean state..."
docker compose restart >/dev/null 2>&1
sleep 5

# Verify container
if ! docker ps --format '{{.Names}}' | grep -q "^${CONTAINER}$"; then
  echo "Error: container '$CONTAINER' not running."
  exit 1
fi

echo
echo "=== Unitree G1 Simulation Launcher ==="
echo "  Container: $CONTAINER"
echo "  Bridge:    http://localhost:${PORT} (inside container)"
echo "  Interface: $INTERFACE"
echo "  Session:   $TMUX_SESSION"
echo

# Window 1: MuJoCo simulator
echo "Starting simulator..."
tmux new-session -d -s "$TMUX_SESSION" -n simulator
tmux send-keys -t "$TMUX_SESSION:simulator" \
  "docker exec -it $CONTAINER bash -c 'cd /ws/Unitree-Mujoco-Dex3/simulate_python && MUJOCO_GL=egl python3 unitree_mujoco.py'" Enter

echo "Waiting for simulator (5s)..."
sleep 5

# Window 2: HTTP bridge server
echo "Starting bridge server..."
tmux new-window -t "$TMUX_SESSION" -n bridge
tmux send-keys -t "$TMUX_SESSION:bridge" \
  "docker exec -it $CONTAINER bash -c 'cd /ws/g1-bricklaying && python3 demo/bridge_server.py --port $PORT --interface $INTERFACE'" Enter

echo "Waiting for bridge server..."
BRIDGE_TIMEOUT=120
BRIDGE_ELAPSED=0
while ((BRIDGE_ELAPSED < BRIDGE_TIMEOUT)); do
  if docker exec "$CONTAINER" curl -sf "http://localhost:${PORT}/health" >/dev/null 2>&1; then
    echo "Bridge server ready (${BRIDGE_ELAPSED}s)."
    break
  fi
  sleep 5
  BRIDGE_ELAPSED=$((BRIDGE_ELAPSED + 5))
  printf "  %ds...\r" "$BRIDGE_ELAPSED"
done
if ((BRIDGE_ELAPSED >= BRIDGE_TIMEOUT)); then
  echo "Warning: bridge not ready after ${BRIDGE_TIMEOUT}s. Starting agent anyway."
fi

# Window 3: Claude agent
if [[ "$LAUNCH_AGENT" -eq 1 ]]; then
  if [[ -z "${ANTHROPIC_API_KEY:-}" ]]; then
    echo "Warning: ANTHROPIC_API_KEY not set. Agent will not start."
    echo "Set it in $ROOT_DIR/.env"
  else
    echo "Starting agent..."
    tmux new-window -t "$TMUX_SESSION" -n agent
    tmux send-keys -t "$TMUX_SESSION:agent" \
      "docker exec -it -e ANTHROPIC_API_KEY='${ANTHROPIC_API_KEY}' $CONTAINER bash -c 'cd /ws/g1-bricklaying && bash demo/run_agent_docker.sh'" Enter
  fi
fi

echo
echo "========================================"
echo "  Unitree G1 Simulation"
echo "========================================"
echo "  Simulator: tmux window 'simulator'"
echo "  Bridge:    tmux window 'bridge'"
echo "  Agent:     tmux window 'agent'"
echo "========================================"
echo
echo "  tmux attach -t $TMUX_SESSION"
echo "  Stop: $(basename "$0") --stop"
echo
