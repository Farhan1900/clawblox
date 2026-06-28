---
title: Save, load, and resume
description: What engine state is captured in snapshots and how to make worlds resumable.
---

# Save, load, and resume

Clawblox engine worlds are designed for checkpointing and resume. This matters
because coding agents may work on a world across long runs, process restarts,
and generation checkpoints.

## Snapshot contents

Engine snapshots include:

- schema version, `game_id`, instance id, tick, status, elapsed time, and
  runtime settings
- `Workspace` children, gravity, fluid forces, terrain, and physical objects
- `ServerScriptService`, `ServerStorage`, and `ReplicatedStorage` children
- players, player GUI children, characters, humanoids, root parts, attributes,
  idle state, and character controller state
- collision groups and collision group pair settings
- pending agent inputs
- queued remote events and recent replicated events
- debris queue
- touch pairs
- pending bot names
- pending kick requests
- queued one-frame physics commands
- Lua VM state

Lua VM snapshots include coroutine schedules, pending coroutines, module cache,
signal connections, animation tracks, `BindToClose` callbacks, selected global
state, and userdata references.

## Authoring guidance

Even though Lua VM state is captured, world state is easiest for agents to
understand and repair when it is visible in the data model.

Prefer storing durable gameplay state in:

- instances
- attributes
- value objects
- `Workspace`
- `ServerStorage`
- `ReplicatedStorage`
- service state that the engine snapshots

Use Lua locals and tables for implementation state, but avoid making them the
only place where important long-term progress exists.

## Renderer state

Renderer-local state is presentation state. It should be safe to rebuild from
the latest spectator observation after refresh or resume.

If the renderer needs persistent information, store it on engine objects and
stream it through attributes or render metadata.

## Operator snapshot endpoint

`GET /snapshot` returns the restorable world snapshot. It is operator-facing,
not an agent action. Agent checkpoints combine world snapshots with agent
archives.

See [Checkpoints](/checkpoints) for the agent-side workflow.

## Restoring locally

Use `clawblox run --resume path/to/snapshot.json` to start a local engine world
from a previously captured runtime snapshot. Relative paths are resolved from
the world directory.

The local runtime snapshot wraps:

- `schema_version`
- `game_id`
- the engine instance snapshot
- joined local sessions
- local chat state

The engine instance snapshot has its own schema version. Restore is strict:
snapshots whose engine schema version does not match the running binary are
rejected with `unsupported snapshot schema version ...`. The embedded Lua VM
snapshot also has a format version and is rejected if it does not match.

## Persistence boundaries

Snapshots are meant to restore Clawblox engine state, not arbitrary process
state.

Good snapshot state:

- instance trees and attributes
- terrain voxels
- player and character state
- service state captured by the engine
- pending Lua coroutines and signal connections captured by the VM snapshot
- Clawblox userdata that has explicit snapshot support, such as instances,
  services, `RaycastParams`, `OverlapParams`, `RaycastResult`, animation tracks,
  and DataStore handles

Do not rely on snapshots for:

- browser-only renderer state
- external files or network side effects
- unsupported host userdata
- wall-clock assumptions; world time is restored from engine/runtime state
