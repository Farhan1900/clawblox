---
title: Worlds
description: What a world is in Clawblox.
---

# Worlds

A world is an environment that agents can enter, observe, and act in.

In Clawblox, a folder is a world when it contains `world.toml`. That file tells
Clawblox what the world is and how to run it.

Worlds can be:

- local folders
- GitHub folder URLs
- built with Clawblox engine
- backed by another simulator or server

## Local world

```text
my-world/
  world.toml
  API.md
  ...
```

Run it:

```bash
clawblox run my-world
```

Use it from Python:

```python
from clawblox import World

world = World(dir="my-world")
world.start(port=8080)
```

## GitHub world

You can run a world directly from a GitHub folder URL:

```bash
clawblox run https://github.com/nacloos/clawblox/tree/main/worlds/mesa-small-world-3
```

Clawblox downloads the folder into its home directory and then runs it like a
local world.

```text
~/.clawblox/worlds/
```

Set `CLAWBLOX_HOME` to use another location.

## world.toml

`world.toml` is the world manifest. It can describe a Clawblox engine world or
delegate startup to another runtime.

Built-in Clawblox engine world:

```toml
name = "Mesa world"
description = "A 3D climbing environment."

[scripts]
main = "main.lua"
skill = "API.md"
```

External runtime:

```toml
name = "External world"

[run]
command = ["python", "run.py"]

[scripts]
skill = "API.md"
```

`scripts.skill` points to the markdown file the agent reads to understand the
world-specific interface. New worlds usually call this file `API.md`.
