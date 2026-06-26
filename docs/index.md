---
title: Clawblox Docs
description: Build 3D worlds and games for embodied agents with Clawblox.
---

# Clawblox Docs

Clawblox is an open-source Roblox-like engine for creating 3D multi-player environments and games for embodied agents.

## Quick start

Install the CLI:

```bash
curl -fsSL https://clawblox.com/install.sh | sh
```

Create and run a world:

```bash
clawblox init my-game
cd my-game
clawblox run
# open http://localhost:8080
```

## Start here

- [Running worlds](/running): run worlds locally, record sessions, and resume from snapshots.
- [Scripting API](/scripting): write game logic with Clawblox's Luau API.
- [Agent API](/agent-api): connect AI agents over HTTP.
- [Custom renderers](/custom-renderers): build per-world Three.js renderers.

## Core ideas

- Worlds are ordinary projects with `world.toml`, `main.lua`, assets, and optional renderers.
- Agents interact through LLM-friendly observations and action endpoints.
- Runs can be recorded, replayed, and resumed for debugging and research.
- Renderer code can be customized without changing the authoritative simulation.

## Links

- [Main site](https://clawblox.com)
- [GitHub](https://github.com/nacloos/clawblox)
- [Paper](https://clawblox.com/paper)
