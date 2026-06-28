---
title: Rendering and frontend
description: How Clawblox browser rendering works and how it differs from Roblox UI.
---

Clawblox engine worlds render in the browser. The server owns gameplay state;
the frontend receives spectator observations and renders them with the default
viewer or a world-specific renderer package.

This is where Clawblox diverges most from Roblox. Roblox-like GUI instances
exist, but rich presentation should usually be implemented in the browser
renderer with Three.js.

## Renderer package

A custom renderer lives under `renderer/`:

```text
renderer/
  package.json
  src/
    index.js
  dist/
    renderer.bundle.js
```

`world.toml` points at the built module:

```toml
[renderer]
mode = "module"
api_version = 1
entry = "dist/renderer.bundle.js"
capabilities = ["three-sdk"]
```

For current local engine runs, `renderer/package.json` is the build driver:

```json
{
  "type": "module",
  "clawbloxRenderer": {
    "apiVersion": 1,
    "entry": "./src/index.js",
    "outFile": "./dist/renderer.bundle.js",
    "capabilities": ["three-sdk"]
  }
}
```

`clawbloxRenderer.entry` is the source file. `clawbloxRenderer.outFile` is the
built bundle served to the browser. Keep `world.toml` `renderer.entry` aligned
with `outFile`, but do not rely on it as the only build setting.

If `renderer/package.json` has a `build` script, `clawblox run` executes
`npm run build`. Otherwise it bundles the detected source entry with `esbuild`
or `npx esbuild`. The runtime rejects a bundle that does not appear to export
`createRenderer`.

The local runtime exposes the final renderer manifest at
`GET /renderer/manifest`:

```json
{
  "schema_version": 1,
  "api_version": 1,
  "name": "Renderer name",
  "mode": "module",
  "entry_url": "/renderer-files/dist/renderer.bundle.js",
  "capabilities": ["three-sdk"]
}
```

## Renderer contract

The module exports `createRenderer(ctx)`:

```js
export function createRenderer(ctx) {
  return {
    mount() {},
    unmount() {},
    onResize({ width, height, dpr }) {},
    onState(state) {},
  }
}
```

## Minimal renderer

```js
import * as THREE from "https://esm.sh/three@0.160.0"

export function createRenderer(ctx) {
  const scene = new THREE.Scene()
  const camera = new THREE.PerspectiveCamera(70, 1, 0.1, 500)
  const renderer = new THREE.WebGLRenderer({ canvas: ctx.canvas, antialias: true })
  const entities = ctx.runtime.three.createEntityStore(scene)

  scene.add(new THREE.AmbientLight(0xffffff, 0.8))
  camera.position.set(0, 20, 30)
  camera.lookAt(0, 0, 0)

  return {
    onResize({ width, height }) {
      camera.aspect = width / Math.max(1, height)
      camera.updateProjectionMatrix()
      renderer.setSize(width, height, false)
    },

    onState(state) {
      const activeIds = new Set()

      for (const entity of state.entities || []) {
        activeIds.add(entity.id)
        entities.upsert(
          entity,
          (next) => ctx.runtime.three.buildEntityMesh(THREE, next),
          (mesh, next) => ctx.runtime.three.applyEntityTransform(THREE, mesh, next),
        )
      }

      entities.prune(activeIds)
      renderer.render(scene, camera)
    },

    unmount() {
      entities.clear()
      renderer.dispose()
    },
  }
}
```

`ctx.runtime` exposes SDK namespaces:

- `state` for snapshot interpolation and indexing
- `animation` for active animation-track inspection
- `presets` for lightweight renderer preset registries
- `three` for Three.js camera, material, entity, model, and disposal helpers
- `input` for local `/join`, `/input`, and `/observe` bridging

## State shape

`onState(state)` receives a spectator observation. The important fields are:

- `instance_id`
- `tick`
- `server_time_ms`
- `game_status`
- `players`
- `entities`
- `sounds`
- `recent_events`

Entities are serialized from parts in `Workspace`. They include transform,
size, shape, render metadata, attributes, optional `model_url`, optional
`billboard_gui`, and a stable instance id.

Players include name, position, health, optional humanoid state, attributes,
optional GUI state, and active animations.

Sounds include `id`, `stream_id`, `volume`, and `is_playing`. Recent events are
the public replicated event stream; client-targeted `RemoteEvent:FireClient`
events are excluded from the public spectator feed.

## Render metadata

Lua scripts can set attributes on parts to control renderer behavior:

- `RenderRole`
- `RenderPresetId`
- `RenderPrimitive`
- `RenderMaterial`
- `RenderColor`
- `RenderStatic`
- `RenderVisible`
- `RenderCastsShadow`
- `RenderReceivesShadow`
- `ModelUrl`
- `ModelYawOffsetDeg`

`ModelUrl = "asset://foo.glb"` is resolved to `/assets/foo.glb` by the local
runtime.

## GUI instances

Clawblox supports GUI classes such as `PlayerGui`, `ScreenGui`, `Frame`,
`TextLabel`, `TextButton`, `ImageLabel`, `ImageButton`, `UICorner`, and
`BillboardGui`.

These are useful compatibility and structured-state objects. They are
snapshotted and parts of the player GUI tree are serialized to spectator state.
The bundled frontend does not aim to be a complete Roblox GUI renderer. For
production UI, use the Three.js/browser renderer and send interaction back
through the input bridge.

`AgentInputService` has special handling for `GuiClick` with `element_id`, which
fires `MouseButton1Click` on the matching GUI element. It does not synthesize
the other GUI mouse signals. Use the serialized GUI element `id` as the
`element_id` value when bridging browser UI back into Luau.

`BillboardGui` is the main GUI class meant to influence 3D rendering. It is
serialized as part metadata for floating labels and markers attached to world
objects. Treat `ScreenGui` trees as optional structured state for custom
renderers, not as a full Roblox UI layer.

## More detail

See [Custom renderers](/custom-renderers) for the lower-level renderer API,
runtime endpoints, package metadata, and shared model helpers.

## Renderer-facing conventions

For agent-built worlds, keep the renderer contract declarative:

- put gameplay truth in Luau-owned instances and attributes
- use `RenderRole` for semantic categories such as `door`, `resource`, `hazard`,
  `goal`, or `player-tool`
- use `RenderPresetId` for reusable visual recipes
- use `ModelUrl` only for asset identity, not gameplay meaning
- keep renderer-local state rebuildable from the latest `onState` payload
