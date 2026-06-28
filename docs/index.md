---
title: Clawblox
sidebarTitle: Overview
summary: "An open-source engine for building 3D worlds that AI agents act in, and for running coding agents as embodied agents in them."
read_when:
  - Introducing Clawblox to newcomers
---

<img src="/logo-full.png" alt="Clawblox" width="160" style={{ display: 'block', margin: '0 auto' }} />

Clawblox is an open-source engine for building 3D worlds that AI agents act in,
and for running coding agents as embodied agents in those worlds. It is written
in Rust, scripted in Luau, and runs headless, so one machine can drive many
agents across many worlds.

It comes in two parts:

- **Clawblox engine** for building worlds: Luau scripting (Roblox-compatible),
  Rapier3D physics, and Three.js rendering.
- **Clawblox agents** for running agents in them: existing coding agents like
  Codex, Claude Code, and Kimi, connected over a standard HTTP protocol, with
  persistent memory and full-run checkpointing.

## Quick start

Install the CLI:

```bash
curl -fsSL https://clawblox.com/install.sh | sh
```

Run a world and open it in the browser:

```bash
clawblox run https://github.com/nacloos/clawblox/tree/main/worlds/mesa-small-world-3
# open http://localhost:8080
```

Run the same world with an agent:

```bash
clawblox run https://github.com/nacloos/clawblox/tree/main/worlds/mesa-small-world-3 --agent codex
```

Agent runs need `tmux` and a coding agent (Codex, Claude Code, or Kimi)
installed and logged in.

Create your own world:

```bash
clawblox init my-world
cd my-world
clawblox run
```

## Start here

- [Installation](/installation): install the CLI and the tools agents need.
- [Running worlds](/running): run worlds locally, from GitHub, or with an agent.
- [Agents](/agents): run a coding agent in a world.
- [Clawblox engine](/engine): build a world of your own.

## Learn more

- [GitHub](https://github.com/nacloos/clawblox)
- [Paper](https://clawblox.com/paper)
