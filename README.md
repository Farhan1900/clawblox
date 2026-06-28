# Clawblox

<p align="center">
  <img src="static/logo.png" alt="Clawblox" width="120"/>
</p>

**Clawblox** is an open-source Roblox-like engine to easily create environments and games for embodied agents.

## Install

**macOS / Linux:**
```bash
curl -fsSL https://clawblox.com/install.sh | sh
```

**Windows (CMD):**
```cmd
curl -fsSL https://clawblox.com/install.cmd -o install.cmd && install.cmd && del install.cmd
```

**Windows (PowerShell):**
```powershell
irm https://clawblox.com/install.ps1 | iex
```

**From source:**
```bash
git clone https://github.com/nacloos/clawblox.git
cd clawblox
cargo install --path .
```

Install pi coding agent:
```bash
npm install -g @mariozechner/pi-coding-agent
```

## Quick start

### 1. Run the agent in a world

Run the world server:
```bash
clawblox run worlds/mesa-world
# open http://localhost:8080
```

In another terminal, run the agent:

```bash
cd agent
npm install
bash run_agent.sh
```

### 2. Create your own world

```bash
clawblox init my-game
cd my-game
clawblox run
# open http://localhost:8080
```

This creates a game with `world.toml` (config), `main.lua` (game logic), and `API.md` (agent instructions).

## Run agents on Minecraft

Prerequisite: configure model auth for the agent (`ANTHROPIC_API_KEY` by default, or `npx @mariozechner/pi-ai login openai-codex` with `PI_PROVIDER=openai-codex`).

It's not required to have the clawblox cli installed.

Terminal 1 (start Minecraft stack):
```bash
cd worlds/minecraft
./run.sh
```

Terminal 2 (start an agent):
```bash
cd agent
npm install
./run_pi_minecraft.sh
```

Optional: point the agent to a non-default adapter URL:
```bash
WORLD_BASE_URL=http://localhost:8081 ./run_pi_minecraft.sh
```

## Features

- **Agent HTTP API**: LLM-friendly API to play games
- **Luau scripting**: Roblox-compatible API (work-in-progress)
- **3D physics**: Rapier3D engine
- **Custom renderers**: per-game Three.js renderers
- **DataStoreService**: persistent storage with local SQLite

## CLI commands

| Command | Description |
|---------|-------------|
| `clawblox init [name]` | Scaffold a new game |
| `clawblox run [path]` | Run locally (Ctrl+C to stop) |
| `clawblox run [path] --resume <snapshot.json>` | Resume a world from a saved snapshot |
| `clawblox start [path]` | Start in background |
| `clawblox stop` | Stop background server |
| `clawblox login` | Register/login |
| `clawblox docs` | View engine docs |

## Documentation

- [Scripting API](docs/scripting.md): Luau API reference
- [Agent API](docs/agent-api.md): HTTP API for AI agents
- [Custom renderers](docs/custom-renderers.md): Three.js renderer guide
- [Game design](docs/game-design.md): design guidelines
- [Running worlds](docs/running.md): local runs, recordings, and strict resume

## Run on your own environment

The agent and the environment run as separate processes talking over HTTP: the environment is a server exposing state and actions as endpoints, and the runner launches Claude Code as a client that joins, reads observations, and posts actions.

`agent/run_claude_agent.sh` runs Claude Code against any HTTP server that implements this protocol.

Required by the runner:

- `POST <join>?name=<agent>` returns JSON containing a session token
- `GET <skill>` returns markdown describing the action API
- `POST <leave>` (optional)

Recommended interface (convention used by the built-in worlds, see `worlds/mesa-world/API.md` for a concrete example):

- `POST /input` with body `{"type": "<Action>", "data": {...}}` queues the
  action and returns an observation from the next full input-processing tick
- `GET /observe` returns the current observation

All endpoints are authed via the session header (default `X-Session: <token>`). The agent reads `<skill>` at startup to learn your action schema, so the exact endpoint names and shapes are up to you.

`worlds/mesa-world` is a complete reference implementation of this protocol you can study or copy from.

Minimal run:

```bash
cd agent
bash run_claude_agent.sh
```

Override paths if your server differs:

| Var | Default |
|-----|---------|
| `WORLD_BASE_URL` | `http://localhost:8080` |
| `WORLD_JOIN_PATH` | `/join` |
| `WORLD_SKILL_PATH` | `/api.md` |
| `WORLD_LEAVE_PATH` | *(unset)* |
| `WORLD_SESSION_FIELD` | `session` |
| `WORLD_SESSION_HEADER` | `X-Session` |
| `CLAUDE_MODEL` | `claude-opus-4-6` |
| `WORKSPACE_DIR` | `agent/claude-runs/<name>/workspace` |

### What the runner adds on top of `claude`

- **Persistent memory workspace.** Each agent gets a workspace seeded from `agent/template/agent/` (`SOUL.md`, `SEMANTIC_MEMORY.md`, `EPISODIC_MEMORY.md`). The system prompt instructs the agent to read and evolve these files, which survive across sessions.
- **Auto-continue loop.** A `Stop` hook blocks shutdown with a "keep going" message so the agent keeps acting until you kill it. Disable with `CLAWBLOX_CLAUDE_AUTO_LOOP=0`.
- **Session resume.** Re-running the script resumes the same Claude session (`--continue`) and the same world session if `WORLD_SESSION_FILE` is set.
- **Reset loop.** `RESET_EVERY=1h` runs the agent for an hour, sends a warning prompt so it can save its memory files, then starts a fresh session. Combine with `DURATION_SECONDS` for a hard stop.
- **Sandbox.** `SANDBOX=1` wraps `claude` in `bwrap` with only the workspace writable. Requires `bubblewrap`.

The pi alternative is `agent/run_agent.sh` (same env vars, requires `npm install` in `agent/` first).


---

This research was sponsored by the Department of the Air Force Artificial Intelligence Accelerator and was
accomplished under Cooperative Agreement Number FA8750-19-2-1000. The views and conclusions
contained in this document are those of the authors and should not be interpreted as representing
the official policies, either expressed or implied, of the Department of the Air Force or the U.S.
Government. The U.S. Government is authorized to reproduce and distribute reprints for Government
purposes notwithstanding any copyright notation herein.
