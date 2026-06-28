---
title: Clawblox docs
description: Run embodied agents in interactive worlds with Clawblox.
---

# Clawblox docs

Clawblox is a framework for embodied agents.

It helps with the parts around the agent: starting a world, connecting an
agent, keeping its workspace and session files together, recording runs, and
resuming from saved state.

You can use Clawblox with its built-in 3D engine, or with another simulator
that exposes the same world interface.

## Quick start

Install the CLI:

```bash
curl -fsSL https://clawblox.com/install.sh | sh
```

Run a world from GitHub:

```bash
clawblox run https://github.com/nacloos/clawblox/tree/main/worlds/mesa-small-world-3
# open http://localhost:8080
```

Run the same world with an agent:

```bash
clawblox run https://github.com/nacloos/clawblox/tree/main/worlds/mesa-small-world-3 --agent codex
```

Create a local Clawblox engine world:

```bash
clawblox init my-world
cd my-world
clawblox run
```

## Two parts

- Clawblox agents manage agents, sessions, workspaces, transcripts, sandboxing,
  and checkpoints.
- Clawblox engine is the built-in Roblox-like engine for making worlds.

A world is a folder with a `world.toml` file. It can be local, or it can be a
GitHub folder URL that Clawblox downloads into its home directory.

## Start here

- [Running worlds](/running): run local and remote worlds, record sessions, and resume from snapshots.
- [World interface](/world-interface): use Clawblox agents with Clawblox engine or another simulator.
- [Agent protocol](/agent-protocol): expose observations and actions to agents.
- [Scripting API](/scripting): write Clawblox engine worlds with Luau.
- [Custom renderers](/custom-renderers): build per-world Three.js renderers.

## Core ideas

- Worlds are environments that agents can join, observe, and act in.
- `world.toml` tells Clawblox that a folder is a world and how to run it.
- Clawblox agents can run against Clawblox engine worlds or external runtimes.
- Runs can be recorded, replayed, checkpointed, and resumed.

## Links

- [Main site](https://clawblox.com)
- [GitHub](https://github.com/nacloos/clawblox)
- [Paper](https://clawblox.com/paper)
