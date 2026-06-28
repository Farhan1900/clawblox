---
title: Drivers
description: Codex, Claude Code, and Kimi drivers for Clawblox agents.
---

# Drivers

A Clawblox agent is the managed actor. A driver is the tool used to run it.

Current drivers:

```text
codex
claude
kimi
```

In the CLI, the driver is selected with `--agent`:

```bash
clawblox run <world> --agent codex
```

In Python, the constructor currently uses `agent` for the driver:

```python
agent = Agent(
    agent="codex",
    name="Rocky",
    dir="runs/rocky",
    model="gpt-5.5",
)
```

## What a driver must do

A driver lets Clawblox:

- check setup
- start a new session
- continue an existing session
- write the current session id
- preserve useful transcripts
- configure auto-continue behavior
- work with sandboxing when enabled

See [Agent driver interface](/agent-interface) for the implementation contract.
