---
title: Installation
description: Install Clawblox and check the tools needed to run agents.
---

Install the Clawblox CLI first.

## macOS and Linux

```bash
curl -fsSL https://clawblox.com/install.sh | sh
```

Check that it works:

```bash
clawblox --version
```

## Windows

Command Prompt:

```cmd
curl -fsSL https://clawblox.com/install.cmd -o install.cmd && install.cmd && del install.cmd
```

PowerShell:

```powershell
irm https://clawblox.com/install.ps1 | iex
```

## Agent requirements

You can run worlds with only the Clawblox CLI.

To run agents, you also need:

- `tmux`
- one agent driver installed and logged in, such as Codex, Claude Code, or Kimi

The CLI command:

```bash
clawblox run <world> --agent codex
```

starts the world, then starts the Codex driver inside a tmux session. The same
pattern applies to `--agent claude` and `--agent kimi`.

## First check

After installing Clawblox, run a world without an agent:

```bash
clawblox run https://github.com/nacloos/clawblox/tree/main/worlds/mesa-small-world-3
```

Open:

```text
http://localhost:8080
```

Once that works, try running the same world with an agent:

```bash
clawblox run https://github.com/nacloos/clawblox/tree/main/worlds/mesa-small-world-3 --agent codex
```
