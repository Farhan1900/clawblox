---
title: Rendering and frontend
summary: "How browser rendering works and how it differs from Roblox UI."
read_when:
  - Working on how a world is displayed
  - Coming from Roblox and expecting its UI model
---

Clawblox engine worlds render in the browser. The server owns gameplay state;
the frontend receives spectator observations and renders them with the default
viewer or a world-specific renderer package.

This is where Clawblox diverges most from Roblox. Roblox-like GUI instances
exist, but rich presentation should usually be implemented in the browser
renderer with Three.js.

## Server-authoritative rendering

The simulation runs on the server, which holds the authoritative state of the
world. Each tick it serializes a spectator observation and streams it to the
browser over a live websocket. The frontend is a pure view: it does not own
gameplay state, it just turns the latest observation into pixels.

Because of this, a renderer is rebuildable from the most recent payload. The
runtime calls your renderer with each new observation, and your code reconciles
the scene against it. There is no hidden client-side game state to keep in sync;
if the renderer is dropped and recreated, it can reconstruct the full scene from
the next observation.

## Renderer packages

A custom renderer is a small JavaScript package that lives under `renderer/` in
a world and exports a `createRenderer(ctx)` factory. The runtime builds the
package, serves the bundle to the browser, and calls your factory with a `ctx`
whose `ctx.runtime` SDK exposes helpers grouped into namespaces:

- `state` for snapshot interpolation and indexing
- `animation` for active animation-track inspection
- `presets` for lightweight renderer preset registries
- `three` for Three.js camera, material, entity, model, and disposal helpers
- `input` for local `/join`, `/input`, and `/observe` bridging

You would write a custom renderer when the default viewer is not enough: to give
a world its own art direction, custom camera behavior, bespoke models, or browser
UI that sends interaction back through the input bridge. Worlds that just need a
functional 3D view can rely on the default viewer.

See [Custom renderers](/custom-renderers) for the renderer SDK, the
`createRenderer(ctx)` contract, package metadata, build behavior, and runtime
endpoints.

## Render metadata

Lua scripts drive presentation by tagging parts with attributes instead of baking
visuals into gameplay code. Attributes such as `RenderRole`, `RenderPresetId`,
`RenderPrimitive`, `RenderMaterial`, and `RenderColor` let world code describe how
a part should look without the renderer needing to understand gameplay. A part can
also reference a static mesh asset with `ModelUrl` (for example
`ModelUrl = "asset://foo.glb"`, which the local runtime resolves to
`/assets/foo.glb`).

This keeps gameplay truth in Luau-owned instances while the renderer reads simple,
declarative hints. `RenderRole` carries semantic categories, `RenderPresetId`
selects a reusable visual recipe, and `ModelUrl` is asset identity, not gameplay
meaning.

See [Custom renderers](/custom-renderers) for the full render-metadata attribute
reference and the `asset://` resolution rules.

## Spectator state

`onState(state)` receives a spectator observation. At a high level it carries
`instance_id`, `tick`, `server_time_ms`, and `game_status`, plus the lists the
renderer draws from:

- `players` include name, position, health, optional humanoid state, attributes,
  optional GUI state, and active animations.
- `entities` are serialized from parts in `Workspace`. They include transform,
  size, shape, render metadata, attributes, an optional model reference, an
  optional billboard GUI, and a stable instance id.
- `sounds` describe active audio streams.
- `recent_events` carry the public replicated event stream.

See [Custom renderers](/custom-renderers) for the full spectator state reference,
including the `SpectatorObservation` type and every player, entity, sound, and
event field.

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

## Renderer-facing conventions

For coding-agent-built worlds, keep the renderer contract declarative:

- put gameplay truth in Luau-owned instances and attributes
- use `RenderRole` for semantic categories such as `door`, `resource`, `hazard`,
  `goal`, or `player-tool`
- use `RenderPresetId` for reusable visual recipes
- use `ModelUrl` only for asset identity, not gameplay meaning
- keep renderer-local state rebuildable from the latest `onState` payload
