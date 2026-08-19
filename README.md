# Clawblox

<p align="center">
  <img src="static/logo.png" alt="Clawblox" width="120"/>
</p>

**Clawblox** is an open-source Roblox-like engine to easily create worlds for
embodied agents.

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

## Quick start

### 1. Run the agent in a world

```bash
clawblox run worlds/mesa-world --agent codex
# open http://localhost:8080
```

This starts the world server and a managed coding agent connected to it.

### 2. Create your own world

```bash
clawblox init my-game
cd my-game
clawblox run
# open http://localhost:8080
```

This creates a world with `world.toml` (config), `main.lua` (world logic), and
`API.md` (agent instructions).

## Features

- **Agent HTTP API**: LLM-friendly API for embodied agents
- **Luau scripting**: Roblox-compatible API (work-in-progress)
- **3D physics**: Rapier3D engine
- **Custom renderers**: per-world Three.js renderers
- **DataStoreService**: persistent storage with local SQLite

## CLI commands

| Command | Description |
|---------|-------------|
| `clawblox init [name]` | Scaffold a new world |
| `clawblox run [path]` | Run locally (Ctrl+C to stop) |
| `clawblox run [path] --resume <snapshot.json>` | Resume a world from a saved snapshot |
| `clawblox start [path]` | Start in background |
| `clawblox stop` | Stop background server |
| `clawblox login` | Register/login |
| `clawblox docs` | View engine docs |

## Documentation

- [Agents](docs/agents.md): run coding agents in worlds
- [Worlds](docs/worlds.md): worlds and the world protocol
- [Scripting](docs/scripting.md): Luau scripting guide
- [Custom renderers](docs/custom-renderers.md): Three.js renderer guide
- [Running worlds](docs/running.md): local runs, recording, and resume

## Run on your own world

Clawblox isn't tied to the built-in engine. Any server that implements the world
protocol (join, observe, and input over HTTP) can be a Clawblox world, and agents
connect to it the same way. `worlds/mesa-world` is a complete reference
implementation you can study or copy from.

See [Worlds](docs/worlds.md) for the protocol and
[World interface](docs/world-interface.md) for the implementer contract, including
the launch environment and snapshot/resume requirements.

---

This research was sponsored by the Department of the Air Force Artificial Intelligence Accelerator and was
accomplished under Cooperative Agreement Number FA8750-19-2-1000. The views and conclusions
contained in this document are those of the authors and should not be interpreted as representing
the official policies, either expressed or implied, of the Department of the Air Force or the U.S.
Government. The U.S. Government is authorized to reproduce and distribute reprints for Government
purposes notwithstanding any copyright notation herein.
