---
title: 3D workspace
summary: "How Workspace represents the physical, visible world."
read_when:
  - Placing or moving objects in the world
  - Understanding the Workspace container
---

`Workspace` is the root of the 3D world. Parts, models, terrain, characters,
sounds, and interactive objects live here.

Belonging to `Workspace` is just ordinary instance parenting; there is no
separate membership system. Parenting an instance to
`Workspace` makes it a Workspace child; setting `Parent = nil` or destroying the
instance removes it. `Workspace:GetChildren()` and
`Workspace:GetDescendants()` read that tree directly.

## Workspace service

`Workspace` exposes world-level fields (`Gravity`, `FluidForces`,
`FallenPartsDestroyHeight`, `CurrentCamera`) plus instance-style tree APIs such
as `GetChildren`, `GetDescendants`, `FindFirstChild`, `FindFirstChildOfClass`,
and `WaitForChild`. See the [engine API reference](/engine-api-reference) for
the full Workspace field and method list.

## Parts and models

Use `Part` for physical primitives and `Model` for grouped objects.

`Model.PrimaryPart` can be used with pivot APIs such as `GetPivot`, `PivotTo`,
`GetPrimaryPartCFrame`, and `SetPrimaryPartCFrame`.

Static environment geometry should usually be anchored and tagged or attributed
clearly for renderers and agents.

## Terrain

`Workspace` creates a `Terrain` child automatically. Terrain is included in
snapshots and can be edited from Luau.

Supported terrain methods:

- `FillBlock(cframe, size, material)`
- `ReadVoxelChannels(region, resolution, channelIds)`
- `WriteVoxelChannels(region, resolution, channels)`

Terrain voxel channel resolution must be `4`. `Region3` bounds for voxel
channel reads/writes must be aligned to that grid and contain at least one
voxel.

Supported channel names are:

- `SolidMaterial`
- `SolidOccupancy`
- `LiquidOccupancy`

`SolidOccupancy` and `LiquidOccupancy` values are clamped to `0..1`. Empty
voxels are removed from terrain storage.

## Spatial queries

Use raycasts and overlap queries for world interaction: `Workspace:Raycast`,
`Workspace:FindPartOnRay`, `Workspace:FindPartOnRayWithIgnoreList`,
`Workspace:GetPartBoundsInBox`, `Workspace:GetPartBoundsInRadius`, and
`Workspace:GetPartsInPart`. Queries are configured with `RaycastParams` and
`OverlapParams`. See the [engine API reference](/engine-api-reference) for the
full query signatures and parameter fields.

Defaults match the common Roblox call shape: `FilterType` is `Exclude`,
`FilterDescendantsInstances` is empty, `CollisionGroup` is `Default`, and
`RespectCanCollide` is `false`. `RaycastParams.IgnoreWater` defaults to
`false`; when it is `true`, terrain water voxels are skipped by raycasts.
`OverlapParams.MaxParts = 0` means unlimited results. Include/exclude filters
apply to the listed instances and their descendants.

Queries only consider parts with `CanQuery = true`. When `RespectCanCollide` is
enabled, they also skip parts with `CanCollide = false`. Query collision groups
use the same collision matrix as physics contacts.

## Renderer-facing state

Spectator entities are serialized primarily from parts in `Workspace`. Use
attributes such as `RenderRole`, `RenderPresetId`, `RenderStatic`, and
`ModelUrl` to describe visual meaning.

Use the `Static` tag or `RenderStatic` attribute for geometry that does not need
dynamic updates.

## Patterns

Use stable names and attributes for objects agents must manipulate:

```lua
local button = Instance.new("Part")
button.Name = "OpenGateButton"
button.Anchored = true
button:SetAttribute("RenderRole", "button")
button:SetAttribute("Action", "open_gate")
button.Parent = Workspace
```

For buildable or pickup objects, keep the object itself in `Workspace` and put
interaction state on attributes such as `IsCarryable`, `OwnerUserId`,
`PlacedBy`, or world-specific action names.

## Static and dynamic observations

Agent player observations include nearby dynamic Workspace parts and
attribute-bearing folders that are not tagged `Static`. Parts are filtered by
the configured observation radius, or by the smaller `ObservationRadius`
attribute on a specific `Player` when present.

Spectator observations are renderer-facing and include Workspace parts for the
browser view, along with render metadata, models, billboard GUI state, sounds,
players, and recent public replicated events.

Use the `Static` tag for geometry that agents do not need in every observation.
Describe important static layout in `API.md`, or expose purpose-specific
attributes and action names on nearby dynamic objects that agents can inspect.
