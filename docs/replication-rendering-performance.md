# Replication and Rendering Performance

Large worlds can contain thousands of independently simulated parts. The runtime
keeps those parts interactive in the engine, but the live client view must avoid
treating every unchanged part as new replication and rendering work every frame.

This document describes the reusable live replication path used by the local
spectator WebSocket:

1. The WebSocket sends one full `SpectatorObservation` when the client connects.
2. Later WebSocket messages are `spectator_delta` frames.
3. Engine-side dirty state records which instances changed at the source.
4. The browser applies each delta to its materialized full view state.
5. Renderers receive the normal full state plus advisory delta metadata.
6. Shared model rendering updates only changed instance matrices when group
   structure is stable.

The goal is to preserve current behavior while cutting repeated serialization,
network, and GPU-update work for mostly static large scenes.

## WebSocket State Model

The first spectator WebSocket message must be a complete `SpectatorObservation`.
Clients need this base state before any delta can be applied.

After that, the server may send:

```json
{
  "type": "spectator_delta",
  "tick": 1234,
  "server_time_ms": 20566,
  "game_status": "running",
  "entities": [],
  "players": [],
  "sounds": [],
  "recent_events": []
}
```

`entities` is a list of entity changes, not a replacement list. An empty
`entities` array means "no entity changes"; it does not mean "clear all
entities".

Player data, sounds, and recent events are small enough to remain full
replacement fields in each delta.

## Entity Delta Format

Entity deltas have three forms:

```json
{ "id": 101, "add": { "...": "complete SpectatorEntity" } }
{ "id": 102, "rm": true }
{ "id": 103, "p": [1, 2, 3], "r": [[1, 0, 0], [0, 1, 0], [0, 0, 1]] }
```

Rules:

- `add` inserts or replaces the complete entity.
- `rm` removes the entity.
- `p` and `r` update only position and rotation.
- If render schema changes, the server sends `add`, not a transform-only update.
- If an entity disappears and reappears with the same id in one computed delta,
  the later `add` state wins.

Transform-only updates are valid only when all non-transform fields still match
the previous full state. This keeps renderers from needing schema-specific
patching logic for material, geometry, model URL, size, name, and attributes.

## Engine Dirty Tracking

Full entity observations are expensive in large worlds because they require a
Workspace scan and serialization of every observed part. The live view therefore
uses source-level dirty tracking after the initial snapshot.

Dirty state is marked when an instance changes in a replication-visible way:

- `Parent` changes mark the instance subtree and old/new parents.
- `SetAttribute` marks the changed instance.
- BasePart transform, size, shape, render, material, physics, and collision
  property writes mark the part.
- BillboardGui metadata marks the billboard, which resolves to its ancestor
  part for entity serialization.
- Physics writeback marks parts whose simulated position or rotation changed.
- Destroyed or unparented known parts become remove deltas.

Dirty state is non-destructive and revision based. Each live client keeps its
own replication cursor plus the set of entity ids already known by that client.
On each tick, the engine reads changes after that cursor, serializes only the
affected entity snapshots, and emits removals for known entities that left
`Workspace`. One viewer cannot consume changes before another viewer sees them.
Player state still updates at the live WebSocket rate because it is small and
expected to change frequently.

## Browser Delta Application

`frontend_runtime/dist/main.js` applies `spectator_delta` messages to the latest
full spectator state before calling a renderer. Renderers still receive a
complete state shape with `entities`, `players`, `tick`, and timing fields.

The materialized state also includes advisory metadata:

```js
state.__spectatorDelta = {
  changedEntityIds,
  removedEntityIds,
  addedEntityIds,
}
```

Renderer code should treat this metadata as an optimization hint. Correctness
must come from the full materialized state. A renderer that ignores
`__spectatorDelta` should still render correctly, just with more work.

## Shared Model Renderer

`runtime.three.createSharedModelRenderer(...)` is the reusable rendering helper
for many entities that reference the same `ModelUrl`.

It still preserves per-entity simulation identity:

- Each part remains a normal engine object.
- Physics, pickup, placement, touch, and observation identity stay per part.
- The renderer only shares loaded mesh resources and GPU instancing.

For stable instanced groups, the helper now tracks:

- entity id to instance index
- dirty entity ids
- structure changes caused by add/remove/group changes
- last known group count

When structure changes, the renderer rebuilds the affected instanced group. When
only transforms change, it updates just the dirty instance matrices. When no
entities in a group changed, it skips matrix writes for that group.

## Renderer Hot Path Rules

Large-scene renderers should follow these rules:

- Avoid per-entity allocations inside `onState`.
- Reuse `Map`, `Set`, `Vector3`, `Quaternion`, and `Matrix4` scratch objects.
- Cache expensive derived data when the source entity array identity is stable.
- Skip material/color writes when the value has not changed.
- Use `createSharedModelRenderer` for repeated static GLB/ModelUrl entities.
- Treat `__spectatorDelta` as advisory and keep a correct full-state fallback.

## Profiling

Use the built-in profiler while running a large world:

```sh
CLAWBLOX_PROFILE=1 CLAWBLOX_PROFILE_TICKS=120 clawblox run
```

For spectator performance, watch:

- `observation.spectator`
- dirty replication update counts
- `manager.process_instance lock`
- `manager.loop`
- WebSocket message count and byte size
- Browser frame time while the spectator view is open

A simple WebSocket byte probe:

```sh
python - <<'PY'
import asyncio, json, websockets

url = "ws://localhost:8080/spectate/ws?spectator_token=TOKEN"

async def main():
    async with websockets.connect(url, max_size=50_000_000) as ws:
        count = total = deltas = 0
        end = asyncio.get_event_loop().time() + 10
        while asyncio.get_event_loop().time() < end:
            msg = await ws.recv()
            total += len(msg.encode() if isinstance(msg, str) else msg)
            count += 1
            if json.loads(msg).get("type") == "spectator_delta":
                deltas += 1
        print("messages", count, "deltas", deltas,
              "total_bytes", total, "avg_bytes", total // max(count, 1))

asyncio.run(main())
PY
```

## Known Limits

- REST `/observe` still returns full observations.
- The first spectator snapshot is still large for large worlds.
- Player-only spectator observation currently duplicates player serialization
  logic from the full observation path and should be refactored when this area
  settles.
- Sound deltas currently remain tied to the existing spectator message shape;
  world entity replication is source-dirty, but sound replication should get a
  dedicated dirty path if large sound sets become common.
