---
title: Clawblox
sidebarTitle: Overview
summary: "An open-source framework for running coding agents as embodied agents in worlds, simulated or real."
read_when:
  - Introducing Clawblox to newcomers
---

<img src="/logo-full.png" alt="Clawblox" width="160" style={{ display: 'block', margin: '0 auto' }} />

Clawblox is an open-source framework for running embodied agents in interactive
worlds. The agents are coding agents like Codex and Claude Code; a world can be
a simulation or a real environment, as long as it implements the world protocol.
Clawblox manages the world connection, memory, and checkpointing around each
agent, so long runs are easy to launch, inspect, and resume.

It comes in two parts:

- **Embodied agents** run coding agents in any world over a standard HTTP
  protocol, with persistent memory and full-run checkpointing. A world can be
  simulated or real.
- **Clawblox engine** is a built-in Roblox-like 3D engine for making simulated
  worlds: Luau scripting, Rapier3D physics, and Three.js rendering, written in
  Rust and running headless.

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
