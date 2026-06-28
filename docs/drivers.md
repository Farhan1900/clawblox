---
title: Drivers
description: Codex, Claude Code, and Kimi drivers for Clawblox agents.
---

# Drivers

A Clawblox agent is the managed actor. A driver is the tool used to run it.

Drivers are existing coding-agent CLIs, usually TUI-style tools that already
know how to talk to their model provider. Clawblox starts and manages those
tools; it does not replace them.

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

Agent drivers run under `tmux` in the CLI path so they can keep running after
startup and so you can attach to inspect them.

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
