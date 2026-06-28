# frontend_runtime

Embedded local frontend for `clawblox run`.

- Served from the CLI binary at `/`
- Loads renderer metadata from `GET /renderer/manifest`
- Loads custom renderer modules from `renderer/` via `/renderer-files/*`

## Renderer API

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

## SDK namespaces

- `ctx.runtime.state` - snapshot interpolation + indexing
- `ctx.runtime.animation` - animation-track inspection
- `ctx.runtime.presets` - lightweight preset registry
- `ctx.runtime.three` - Three.js lifecycle/camera/material/entity helpers
- `ctx.runtime.input` - local `join/input/observe` bridge + keyboard mapping

These helpers are designed to support renderer complexity similar to FPS/FallGuys frontends.

## Replay viewport readiness

When the runtime is embedded as `/replay/view`, it posts this message to its
parent after renderer state has been applied, SDK-tracked assets such as GLB
models have loaded, and a frame has painted:

```js
{
  type: 'replay:viewport-ready',
  version: 1,
  href: window.location.href,
  tick
}
```

Replay containers use this signal to hide loading UI and to swap preloaded
combined-recording segments without exposing partially rendered frames.

## Viewport-only runtime mode

Replay containers that provide their own chrome can load the runtime with
`?viewport=1`. In this mode the runtime owns only the canvas, renderer loading,
spectator WebSocket, and renderer asset loading. Chat, player lists, replay
controls, agent/workspace panels, behavior panels, and metadata polling are not
initialized.
