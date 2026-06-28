---
title: Custom renderers
description: Low-level renderer package, runtime SDK, and spectator state reference.
---

# Custom renderers

`clawblox run` includes an embedded frontend and supports per-world custom renderers.

## File layout

```text
my-world/
  world.toml
  main.lua
  renderer/
    package.json
    src/
      index.js
    dist/
      renderer.bundle.js
```

## world.toml

```toml
[renderer]
name = "Default World Renderer"
mode = "module"
api_version = 1
entry = "dist/renderer.bundle.js"
capabilities = ["presets", "animation-tracks", "three-sdk", "input-bridge"]
```

- `entry` is relative to `renderer/` and should point to the built bundle.
- Current local runs use `renderer/package.json` `clawbloxRenderer.outFile` as
  the build output served in `/renderer/manifest`; keep this path aligned with
  `renderer.entry`.
- Renderer package metadata lives in `renderer/package.json` under
  `clawbloxRenderer`.

## Renderer Package (`renderer/package.json`)

Minimal example:

```json
{
  "name": "@clawblox-renderers/my-world",
  "private": true,
  "type": "module",
  "clawbloxRenderer": {
    "apiVersion": 1,
    "entry": "./src/index.js",
    "outFile": "./dist/renderer.bundle.js",
    "capabilities": ["three-sdk"]
  }
}
```

Notes:
- If `scripts.build` exists, `clawblox run` executes `npm run build` in `renderer/`.
- Otherwise, `clawblox run` bundles `clawbloxRenderer.entry` with esbuild automatically.

## Build and Load Behavior

`clawblox run` is package-first and fail-fast:

- Requires `renderer/package.json`
- Builds renderer package output from `clawbloxRenderer.outFile`, defaulting to
  `dist/renderer.bundle.js`
- Validates output exists and contains `createRenderer`
- Serves built artifact through `/renderer-files/*`

No fallback renderer is used in this flow. If package build/validation fails, `clawblox run` exits with an error.

## Import compatibility tips

Prefer `esm.sh` URLs for Three.js ecosystem imports in browser-loaded renderers:

```js
import * as THREE from "https://esm.sh/three@0.160.0"
import { GLTFLoader } from "https://esm.sh/three@0.160.0/examples/jsm/loaders/GLTFLoader.js"
```

Why: many `three/examples/jsm/*` files internally import bare specifiers like `"three"`. Those can fail in plain browser module loading if unresolved. The local runtime ships an import map for `three`, but using `esm.sh` directly is usually the smoothest path.

## Renderer contract (api_version = 1)

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

## `ctx` runtime SDK

`ctx` contains:

- `apiVersion`
- `manifest`
- `canvas`
- `log(level, message, data?)`
- `runtime`

### Core

- `runtime.state.createSnapshotBuffer({ maxSnapshots, interpolationDelayMs })`
- `runtime.state.indexById(items)`
- `runtime.animation.findTrack(player, predicate)`
- `runtime.animation.hasTrackMatching(player, /regex/)`
- `runtime.animation.mapPlayersByRootPart(players)`
- `runtime.presets.createPresetRegistry(initial)`

### Three.js (`runtime.three`)

- `createFollowCameraController(THREE, camera, options)`
- `createCameraModeController(THREE, camera, options)`
- `createPresetMaterialLibrary(initial)`
- `materialFromRender(THREE, render, presetLib?)`
- `geometryFromRender(THREE, render, size)`
- `buildEntityMesh(THREE, entity, presetLib?)`
- `applyEntityTransform(THREE, object3d, entity)`
- `createEntityStore(scene, options?)` (upsert/prune/dispose)
- `disposeObject3D(object3d)`
- `createModelTemplateCache()`
- `createSharedModelRenderer(THREE, scene, options?)`
- `createModelEntityController(THREE, options?)`
- `classifyAnimationTracks(player)`
- `applyRendererPreset(THREE, renderer, preset)`
- `applyLightingPreset(THREE, scene, preset)`

### Shared Model Renderer (`createSharedModelRenderer`)

Use `runtime.three.createSharedModelRenderer(...)` for Roblox-like `ModelUrl` rendering when many entities reference the same static mesh asset. World code still creates ordinary individual parts; each part keeps its own physics, transform, observation identity, pickup behavior, and `ModelUrl`. The renderer/runtime may share the loaded mesh resource internally.

The helper:

- counts repeated `model_url` / `ModelUrl` values each frame
- loads each GLB asset once
- renders repeated non-skinned, non-animated GLBs with `THREE.InstancedMesh`
- falls back to temporary primitive boxes while the shared asset is loading
- rejects skinned, animated, or complex material-array models so they can use `createModelEntityController`

Typical renderer flow:

```js
const sharedModels = ctx.runtime.three.createSharedModelRenderer(THREE, scene, {
  minInstances: 20,
})

function updateScene(obs) {
  sharedModels.beginFrame(obs.entities || [])
  for (const entity of obs.entities || []) {
    if (sharedModels.accepts(entity)) continue
    // existing primitive or unique animated model path
  }
  sharedModels.endFrame()
}
```

### Model Helper (`createModelEntityController`)

Use `runtime.three.createModelEntityController(...)` for model entities instead of manual GLTF fitting. It handles:

- GLTF loading/caching
- scaling model height to match `entity.size[1]`
- centering/alignment of model bounds to entity transform
- optional `model_yaw_offset_deg`
- basic walk/idle animation blending hooks

### Local input bridge (`runtime.input`)

- `createLocalInputClient({ baseUrl, playerName })`
  - Uses local `/join`, `/input`, `/observe`
- `inputClient.sendRemoteEvent(name, args?)`
  - Sends `type: "RemoteEvent"` with `{ name, args }` payload
- `bindKeyboardActions(inputClient, bindings, options?)`
  - Key-to-action mapping with tap/hold modes

Example:

```js
const input = ctx.runtime.input.createLocalInputClient({ playerName: 'render-bot' })
ctx.runtime.input.bindKeyboardActions(input, {
  KeyW: { mode: 'hold', type: 'MoveForward', data: {} },
  Space: { mode: 'tap', type: 'Jump', data: {} },
})
await input.sendRemoteEvent('PlayerInput', [{ x: 1, y: 0, z: 0 }])
```

## Runtime endpoints

- `GET /` - local frontend host
- `GET /renderer/manifest` - renderer metadata
- `GET /renderer-files/*` - static files from the world's `renderer/`
- `GET /assets/*` - world assets from `assets/` (used for `asset://...` URLs)
- `GET /spectate/ws` - live spectator observation stream

## State shape (`onState(state)`)

Custom renderers receive spectator observations shaped like:

```ts
type SpectatorObservation = {
  instance_id: string
  tick: number
  server_time_ms: number
  game_status: "waiting" | "playing" | "finished" | "failed"
  players: SpectatorPlayerInfo[]
  entities: SpectatorEntity[]
  sounds?: SpectatorSound[]
  recent_events: SpectatorReplicatedEvent[]
}
```

### `players[*]` fields

- `id`, `name`, `position`, `health`
- optional: `root_part_id`, `humanoid_state`, `attributes`, `gui`, `active_animations`

### `entities[*]` fields

- `id`, `type`, `position`
- optional: `rotation`, `size`, `health`, `pickup_type`, `model_url`, `model_yaw_offset_deg`, `billboard_gui`
- always includes `render`:

```ts
type SpectatorRender = {
  kind: "primitive" | "preset"
  role: string
  preset_id?: string
  primitive: string
  material: string
  color: [number, number, number]
  static: boolean
  casts_shadow: boolean
  receives_shadow: boolean
  visible: boolean
  double_sided: boolean
  transparency?: number
}
```

### Rotation format

If present, `entity.rotation` is a `3x3` matrix:

```ts
rotation?: [[number, number, number], [number, number, number], [number, number, number]]
```

This matches `runtime.three.applyEntityTransform`.

## Render metadata from Lua attributes

Renderer-facing metadata is controlled via part attributes:

- `RenderRole` (string)
- `RenderPresetId` (string) -> switches render `kind` to `"preset"`
- `RenderPrimitive` (string)
- `RenderMaterial` (string)
- `RenderColor` (`Color3` or `Vector3`)
- `RenderStatic` (bool)
- `RenderVisible` (bool)
- `RenderCastsShadow` (bool)
- `RenderReceivesShadow` (bool)

If omitted, runtime falls back to the part's normal shape/material/color/transparency.

## Model URLs (`asset://`)

For local runtime:

- Setting `ModelUrl = "asset://foo.glb"` on a part makes spectator payload expose a resolved URL.
- The CLI rewrites `asset://foo.glb` to `/assets/foo.glb` before sending state.
- Optional yaw offset is read from either `ModelYawOffsetDeg` or `model_yaw_offset_deg`.
- `ModelUrl` is a per-entity shared asset reference, similar in spirit to a Roblox `MeshPart` referencing a mesh asset. Individual entities remain separate simulation objects; renderer/runtime helpers may share static visual assets internally.
