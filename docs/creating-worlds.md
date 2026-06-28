---
title: Creating worlds
description: How a Clawblox engine world is laid out and what files agents should edit first.
---

A Clawblox engine world is a directory that Clawblox can load and run directly.
The folder is the unit you edit, checkpoint, resume, and hand to coding agents.

## Create a world

Create a new world directory:

```bash
clawblox init my-world
cd my-world
clawblox run
```

Or initialize the current directory:

```bash
clawblox init
clawblox run
```

`clawblox run` defaults to the current directory. You can also pass an explicit
world path:

```bash
clawblox run ./my-world
```

## Generated layout

```text
my-world/
  world.toml
  main.lua
  API.md
  docs/
  assets/
  renderer/
    package.json
    src/
      index.js
  .gitignore
```

`world.toml` is the manifest. A folder is a world when it has this file.

`main.lua` is the default server script.

`API.md` is the agent-facing contract. Keep it close to the actual Luau input
handlers.

`docs/` is a copy of the Clawblox docs bundled into new worlds. The CLI tries
to fetch the latest docs and falls back to its embedded copy.

`assets/` is created for GLB models, textures, images, audio, and other static
files. It is served by the local runtime at `/assets/*`.

`renderer/` contains a package scaffold with `renderer/package.json` and
`renderer/src/index.js`. The package metadata tells the local run flow which
source file to bundle and which output file to serve.

## Script tree

The engine also supports an optional script tree:

```toml
[scripts]
main = "main.lua"
tree = "scripts"
skill = "API.md"
```

The expected tree layout uses service roots such as:

```text
scripts/
  ServerScriptService/
  Workspace/
```

Use the tree when a world has grown beyond a single `main.lua`.

## Agent-friendly workflow

1. Update `API.md` with the intended observations and actions.
2. Put durable gameplay state in instances, attributes, values, services, or
   documented storage objects.
3. Implement the server behavior in Luau.
4. Expose renderer-facing state through stable attributes and render metadata.
5. Test with both direct player input and agent-style input.

## Init template behavior

The generated world includes a `MoveTo` action in `API.md`, a basic floor in
`main.lua`, an `AgentInputService.InputReceived` handler, and a starter
Three.js renderer. Treat it as a scaffold, not as the recommended final
architecture for every world.

The generated `renderer/package.json` includes:

```json
{
  "type": "module",
  "clawbloxRenderer": {
    "apiVersion": 1,
    "entry": "./src/index.js",
    "outFile": "./dist/renderer.bundle.js",
    "capabilities": []
  }
}
```

When `clawblox run` starts the world, it builds this package, verifies that the
output appears to export `createRenderer`, and serves the bundle through the
runtime renderer manifest.
