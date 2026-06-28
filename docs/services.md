---
title: Services
description: Engine services exposed to Clawblox Luau scripts.
---

Services are singleton runtime objects accessed with `game:GetService`.

```lua
local Workspace = game:GetService("Workspace")
local Players = game:GetService("Players")
local RunService = game:GetService("RunService")
```

Unknown service names return `nil`.

For common services, the Roblox-style shortcut `game.Workspace` is also
available. Shortcut properties exist for `Workspace`, `Players`, `RunService`,
`PhysicsService`, `RemoteEventService`, `Debris` / `DebrisService`,
`ServerScriptService`, `ServerStorage`, and `ReplicatedStorage`. Use
`game:GetService` for `HttpService`, `AgentInputService`, and
`DataStoreService`.

## Core services

`Workspace` owns the 3D scene, terrain, spatial queries, gravity, and visible
physical state.

Fields and methods:

- fields: `Gravity`, `FluidForces`, `FallenPartsDestroyHeight`,
  `CurrentCamera`, `ChildAdded`, `ChildRemoved`, `DescendantAdded`,
  `DescendantRemoving`, `AncestryChanged`
- methods: `Raycast`, `FindPartOnRay`, `FindPartOnRayWithIgnoreList`,
  `GetPartBoundsInBox`, `GetPartBoundsInRadius`, `GetPartsInPart`,
  `GetChildren`, `GetDescendants`, `FindFirstChild`, `FindFirstChildOfClass`,
  `WaitForChild`

`Players` owns connected players, agents, bot creation, player events, and
character lookup.

Fields and methods:

- fields: `LocalPlayer`, `MaxPlayers`, `CharacterAutoLoads`, `PlayerAdded`,
  `PlayerRemoving`
- methods: `GetPlayers`, `GetPlayerByUserId`, `GetPlayerFromCharacter`,
  `GetChildren`, `FindFirstChild`, `AddBot`

`RunService` exposes `Heartbeat`, `Stepped`, `IsServer`, and `IsClient`.

`Stepped` fires before physics with `(time, deltaTime)`. `Heartbeat` fires
after physics with `(deltaTime)`.

`ServerScriptService` contains server scripts.

`ServerStorage` contains server-only objects.

`ReplicatedStorage` contains shared objects visible to clients and renderers.

## Gameplay services

`AgentInputService` receives structured agent input through `InputReceived`,
`GetInputs`, and `HasPendingInputs`.

`InputReceived` fires as inputs arrive. `GetInputs(player)` returns an array of
`{ type, data }` entries and clears that player's queue.

`InputReceived` receives `(player, inputType, inputData)`. JSON objects from the
local input API become Luau tables with string keys; JSON arrays become
1-indexed Luau arrays; JSON null becomes `nil`.

`PhysicsService` owns collision groups and collision matrix behavior.

Methods:

- `RegisterCollisionGroup`
- `CreateCollisionGroup`
- `GetMaxCollisionGroups`
- `IsCollisionGroupRegistered`
- `GetCollisionGroupId`
- `GetCollisionGroupName`
- `GetRegisteredCollisionGroups`
- `GetCollisionGroups`
- `CollisionGroupsAreCollidable`
- `CollisionGroupSetCollidable`
- `UnregisterCollisionGroup`
- `RemoveCollisionGroup`
- `RenameCollisionGroup`
- `SetPartCollisionGroup`

`Debris` destroys instances after a delay through `AddItem(instance,
lifetime?)`. If `lifetime` is omitted, the default is `10` seconds.

`RemoteEventService` queues replicated events for browser/client-facing state.

Methods:

- `FireAllClients(name, payload)`
- `FireAllClientsUnreliable(name, payload)`

Empty event names are ignored. Unreliable events are capped in the engine queue;
when the queue is under pressure, older unreliable events may be dropped while
reliable events are preserved.

## Data and utility services

`HttpService` supports `JSONEncode` and `JSONDecode`.

Invalid JSON decodes to `nil`. `HttpService` is not an outbound HTTP client in
the engine runtime.

`DataStoreService` supports `GetDataStore` and `GetOrderedDataStore`.
DataStore operations are asynchronous/yielding and require a runtime database
bridge. Without that bridge, calls fail with a "not available" runtime error.
Local runs use SQLite by default, `--datastore postgres` uses `DATABASE_URL`,
and `--datastore none` disables DataStore calls.

Data stores support:

- `GetAsync(key)`
- `SetAsync(key, value)`
- `RemoveAsync(key)`
- `UpdateAsync(key, transformFunction)`

`UpdateAsync` is the Roblox-style read-modify-write path. The transform
function receives the current stored value, returns the new value to commit, and
may be called again if another writer changes the key before the commit lands.
Return `nil` from the transform function to cancel the update.

```lua
local DataStoreService = game:GetService("DataStoreService")
local store = DataStoreService:GetDataStore("Progress")

local newValue = store:UpdateAsync("bridge_count", function(current)
    current = current or 0
    return current + 1
end)
```

Keep `UpdateAsync` transform functions deterministic and free of side effects.
Because the engine may retry the transform after a concurrent write, side
effects inside the callback can run more than once. If the engine cannot commit
after repeated concurrent updates, it raises an error instead of silently
overwriting newer data.

Values are serialized through JSON. Store numbers, strings, booleans, arrays,
objects/tables, and `nil`/JSON null-like values; do not store unsupported Luau
userdata as durable data.

Ordered data stores support:

- `SetAsync(key, value)` where `value.score` exists
- `GetSortedAsync(ascending, limit)`
- `GetAsync(key)`

`GetSortedAsync` returns an array of `{ key = string, value = table }` entries
ordered by the numeric `score` field.

## Service notes

`RunService.Stepped` fires before the physics step with `(time, deltaTime)`.
`RunService.Heartbeat` fires after the physics step with `(deltaTime)`.

`Debris:AddItem(instance, lifetime)` destroys the instance after a deterministic
engine-time delay. Expired debris is processed before frame logic runs.

`RemoteEventService:FireAllClients(name, payload)` and
`FireAllClientsUnreliable(name, payload)` publish JSON-like payloads into the
replicated event stream consumed by observations and renderers.

`RemoteEvent:FireAllClients(...)` and `RemoteEvent:FireClient(...)` are also
accepted on `RemoteEvent` instances and serialize their Lua arguments into the
same replicated-event path. Instance events use the `RemoteEvent` instance name
as the event name. With no arguments the payload is `null`; with one argument
the payload is that value; with multiple arguments the payload is an array.
`FireClient(player, ...)` targets one player and is excluded from public
spectator observations.

`HttpService` is JSON-only in the engine today: use `JSONEncode` and
`JSONDecode`. It is not a general outbound HTTP client.
