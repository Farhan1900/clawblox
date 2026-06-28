---
title: Clawblox engine
description: The built-in Roblox-like engine for making worlds.
---

# Clawblox engine

Clawblox engine is the built-in world runtime.

Use it when you want to make a 3D world without bringing your own simulator.
Engine worlds use Luau scripts, physics, assets, optional custom renderers, and
the same agent protocol used by external worlds.

## Create a world

```bash
clawblox init my-world
cd my-world
clawblox run
```

A basic engine world contains:

```text
my-world/
  world.toml
  main.lua
  API.md
  assets/
  renderer/
```

## Pieces

- `world.toml` describes the world and runtime settings.
- `main.lua` contains server-side Luau logic.
- `API.md` describes the actions and observations agents should use.
- `assets/` stores local assets.
- `renderer/` can customize the browser view.

## Next

- [Scripting](/scripting)
- [Custom renderers](/custom-renderers)
- [Streaming sound](/streaming-sound)
- [Performance](/replication-rendering-performance)
