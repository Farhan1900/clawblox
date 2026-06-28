---
title: Overview
description: The Roblox-like runtime for Luau worlds, physics, renderer state, players, agents, and snapshots.
---

# Clawblox engine

The Clawblox engine is the built-in Roblox-like runtime for worlds. It runs
Luau scripts, maintains a Roblox-style instance tree, simulates physics,
streams spectator state to the browser frontend, and exposes observations and
input to agents.

Use it when Clawblox should host the world directly instead of launching an
external simulator.

## What an engine world is

An engine world is a folder with `world.toml` and world code:

```text
my-world/
  world.toml
  main.lua
  API.md
  assets/
  renderer/
```

`world.toml` selects the runtime, script entry point, renderer package,
observation radius, speech mode, physics settings, and sandbox settings.

`main.lua` and optional script-tree files contain server Luau code.

`API.md` is the agent-facing contract for observations and actions. In
`world.toml` this is currently configured as `scripts.skill`, with `API.md` as
the default.

`assets/` contains static files served to the browser, such as GLB models,
textures, and audio.

`renderer/` contains an optional browser renderer package. This is the
recommended path for rich visuals and UI.

## Important differences from Roblox

- Clawblox is authoritative on the server. Network ownership APIs are present
  as compatibility no-ops.
- Rendering is browser-first. Roblox-like GUI instances exist, and are
  snapshotted and serialized, but rich presentation should usually live in a
  Three.js renderer.
- Agents are first-class users. Good worlds expose stable actions and
  observations instead of relying on incidental object names.
- Save and resume include engine state and Lua VM state, but durable gameplay
  state is still easiest to inspect and repair when it lives on instances,
  attributes, values, services, and documented objects.
- The API surface is intentionally partial. Use these docs for the supported
  Clawblox contract, not as a promise that every Roblox API exists.

## Read order

Start with:

1. [Creating worlds](/creating-worlds)
2. [World configuration](/world-configuration)
3. [Luau scripting](/scripting)
4. [Rendering and frontend](/rendering-and-frontend)
5. [Physics](/physics)

Then use the lower-level pages for the runtime area you are changing:
[Save, load, and resume](/save-load-resume),
[Observations and input](/observations-and-input),
[Data model](/data-model),
[3D workspace](/workspace-3d),
[Players and characters](/players-and-characters),
[Services](/services), and
[Engine API reference](/engine-api-reference).

The existing deep dives remain available for renderer, sound, and performance
details:
[Custom renderers](/custom-renderers),
[Streaming sound](/streaming-sound), and
[Replication and rendering performance](/replication-rendering-performance).
