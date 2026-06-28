---
title: Data model
description: The Roblox-style instance tree used by Clawblox engine worlds.
---

The Clawblox data model is a Roblox-style tree of instances. Luau scripts
create, parent, query, tag, and configure instances; the engine uses that tree
as the authoritative structure of the world.

The instance tree is the source of truth for membership. Parenting an instance
to `Workspace`, `ServerStorage`, or `ReplicatedStorage` makes it a child of that
service; setting `Parent = nil` or destroying the instance removes it.

## Instances

Every object has:

- `ClassName`
- `Name`
- `Parent`
- children
- attributes
- tags
- events
- methods

Core instance events include:

- `ChildAdded`
- `ChildRemoved`
- `DescendantAdded`
- `DescendantRemoving`
- `AncestryChanged`
- `Destroying`
- `AttributeChanged`

Core methods include:

- `SetAttribute`
- `GetAttribute`
- `GetAttributes`
- `GetAttributeChangedSignal`
- `GetPropertyChangedSignal`
- `AddTag`
- `HasTag`
- `RemoveTag`
- `GetTags`
- `Clone`
- `Destroy`
- `ClearAllChildren`
- `FindFirstChild`
- `FindFirstChildOfClass`
- `FindFirstChildWhichIsA`
- `GetChildren`
- `GetDescendants`
- `GetDebugId`
- `IsA`
- `IsDescendantOf`
- `WaitForChild`

Supported attribute value types are `nil`, boolean, number, string, `Vector3`,
`Color3`, and array-style tables containing supported attribute values.

Tags are stored directly on instances. They are useful for renderer metadata
and world queries, but they are not currently exposed through a separate
`CollectionService`.

## Supported classes

The engine currently registers these instance classes:

- `Instance`
- `Folder`
- `Workspace`
- `Terrain`
- `Model`
- `BasePart`
- `Part`
- `Camera`
- `Player`
- `Humanoid`
- `Animation`
- `Animator`
- `Script`
- `ModuleScript`
- `ServerScriptService`
- `ServerStorage`
- `ReplicatedStorage`
- `RemoteEvent`
- `Sound`
- value objects: `IntValue`, `NumberValue`, `BoolValue`, `StringValue`,
  `ObjectValue`, `CFrameValue`
- physics objects: `Weld`, `WeldConstraint`, `BodyVelocity`, `BodyForce`,
  `BodyAngularVelocity`, `Attachment`, `VectorForce`, `AlignPosition`,
  `AlignOrientation`
- GUI objects: `PlayerGui`, `ScreenGui`, `Frame`, `TextLabel`, `TextButton`,
  `ImageLabel`, `ImageButton`, `BillboardGui`, `UICorner`

## Services and storage

Use services for top-level runtime capabilities:

```lua
local Workspace = game:GetService("Workspace")
local Players = game:GetService("Players")
local ServerStorage = game:GetService("ServerStorage")
```

`Workspace` is visible and physical world state.

`ServerStorage` is server-only state.

`ReplicatedStorage` is shared state intended to be visible to clients and
renderers.

## Value objects

`IntValue`, `NumberValue`, `BoolValue`, `StringValue`, `ObjectValue`, and
`CFrameValue` expose:

- `Value`
- `Changed`

`ObjectValue.Value` accepts an `Instance` or `nil`. `CFrameValue.Value` accepts
`CFrame`.

## Scripts and remote events

`Script` and `ModuleScript` expose:

- `Source`
- `Disabled`

`RemoteEvent` exposes:

- `OnServerEvent`
- `FireAllClients`
- `FireClient`

`FireAllClients` and `FireClient` serialize Lua arguments into JSON-like
payloads for replicated event streams. Instance events use the `RemoteEvent`
instance name as the event name. `FireClient(player, ...)` targets one player;
public spectator observations include only non-targeted events.

## Parenting and replication rules

Parenting is the main ownership operation. Use `instance.Parent = target` or
`instance:set_parent`-equivalent APIs exposed through Clawblox internals by
setting `Parent` from Luau. Moving an instance updates the tree immediately and
fires ancestry/child/descendant signals.

Important roots:

- `Workspace`: physical and visible world state
- `ServerScriptService`: server scripts
- `ServerStorage`: hidden authoritative state
- `ReplicatedStorage`: shared objects intended for renderer/client-facing use
- `Player.PlayerGui`: per-player GUI compatibility tree and GUI click targets

Replication is not Roblox client/server replication. Clawblox serializes
engine observations and spectator state from the authoritative server-side data
model. Put renderer-relevant metadata on instances through stable names,
attributes, tags, and supported properties.

Destroyed or unparented instances are not in the active tree. Snapshots still
preserve engine-owned references that are needed to restore valid runtime
state, but world authors should not rely on orphan instances as gameplay
storage.

## Authoring rules

Use the instance tree as the durable structure of the world:

- Put visible and physical objects under `Workspace`.
- Put hidden authoritative state under `ServerStorage`.
- Put renderer/client-facing shared state under `ReplicatedStorage`.
- Use attributes and value objects for state that agents, renderers, or resume
  logic need to inspect.
- Use stable names for objects that agents will target through actions.

Do not use orphan instances as storage. Snapshots preserve some engine-owned
or Lua-held references so running code can resume, but authored world state is
much easier to inspect and repair when it is parented under a clear service
root.

For exact class, property, method, and enum support, use
[Engine API reference](/engine-api-reference).
