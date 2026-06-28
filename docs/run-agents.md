---
title: Run agents
description: Start agents in worlds from the CLI or Python.
---

# Run agents

The quickest way to run an agent is to pass `--agent` when starting a world.

```bash
clawblox run https://github.com/nacloos/clawblox/tree/main/worlds/mesa-small-world-3 --agent codex
```

This starts the world, then starts a managed Clawblox agent connected to it.

## Requirements

Agent runs use existing coding-agent CLIs. Install and log in to the driver you
want to use first:

```text
codex    Codex CLI
claude   Claude Code
kimi     Kimi Code
```

You also need `tmux`. Clawblox starts the driver inside a tmux session so the
agent can keep running, be resumed, and be inspected.

## CLI options

```bash
clawblox run <world> --agent codex
clawblox run <world> --agent claude
clawblox run <world> --agent kimi
```

Useful options:

```bash
clawblox run <world> \
  --agent codex \
  --agent-name Rocky \
  --model gpt-5.5 \
  --agent-dir runs/rocky \
  --sandbox
```

## Python

Use Python when you want to orchestrate worlds and agents from a script.

```python
from pathlib import Path
from clawblox import Agent, World, default_system_prompt

world = World(dir="worlds/mesa-small-world-3")
world.start(port=8080, record=True, record_dir=Path("runs/r001/recordings"))

agent = Agent(
    agent="codex",
    name="Rocky",
    dir=Path("runs/r001/agents/rocky"),
    model="gpt-5.5",
    sandbox=True,
)

access = world.connect(agent=agent)
agent.start(
    initial_prompt="Begin.",
    system_prompt=default_system_prompt(),
)
```

`world.connect(agent=agent)` joins the world, writes the world session id into
the agent directory, and sets the environment the agent needs to talk to the
world.
