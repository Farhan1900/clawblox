---
title: Clawblox agents
description: Run and manage embodied agents in worlds.
---

# Clawblox agents

Clawblox agents are for running long-lived agents in interactive worlds.

An agent has a name, a workspace, session files, transcripts, logs, and a
connection to a world. Clawblox keeps those pieces together so runs are easier
to inspect, continue, checkpoint, and compare.

Agents are not tied to Clawblox engine. They can run in any world that exposes
the world interface.

## What Clawblox manages

- world access and session headers
- agent workspaces and memory files
- driver sessions for tools like Codex, Claude Code, and Kimi
- logs, transcripts, and rerunnable command files
- optional sandboxing
- checkpoints that can include world state and agent state

## CLI

The simplest path is to start an agent when you run a world:

```bash
clawblox run https://github.com/nacloos/clawblox/tree/main/worlds/mesa-small-world-3 --agent codex
```

In the CLI, `--agent codex` means “start a managed Clawblox agent using the
Codex driver.”

## Python

Python gives you the same pieces as objects:

```python
from pathlib import Path
from clawblox import Agent, World, default_system_prompt

world = World(dir="https://github.com/nacloos/clawblox/tree/main/worlds/mesa-small-world-3")
world.start(port=8080)

agent = Agent(
    agent="codex",
    name="Rocky",
    dir=Path("runs/rocky"),
    model="gpt-5.5",
)

world.connect(agent=agent)
agent.start(
    initial_prompt="Begin.",
    system_prompt=default_system_prompt(),
)
```

The Python constructor currently uses `agent="codex"` for the driver name.
