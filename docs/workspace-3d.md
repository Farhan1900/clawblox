---
title: 3D workspace
description: How Workspace represents the physical and visible world.
---

# 3D workspace

`Workspace` is the root of the 3D world. Parts, models, terrain, characters,
sounds, and interactive objects live here.

Workspace membership is the normal instance tree. Parenting an instance to
`Workspace` makes it a Workspace child; setting `Parent = nil` or destroying the
instance removes it. `Workspace:GetChildren()` and
`Workspace:GetDescendants()` read that tree directly.

## Workspace service

`Workspace` exposes world-level fields:

- `Gravity`
- `FluidForces`
- `FallenPartsDestroyHeight`
- `CurrentCamera`

It also exposes instance-style tree APIs such as `GetChildren`,
`GetDescendants`, `FindFirstChild`, `FindFirstChildOfClass`, and
`WaitForChild`.

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

Use raycasts and overlap queries for world interaction:

- `Workspace:Raycast(origin, direction, params)`
- `Workspace:FindPartOnRay(ray)`
- `Workspace:FindPartOnRayWithIgnoreList(ray, ignoreList)`
- `Workspace:GetPartBoundsInBox(cframe, size, params)`
- `Workspace:GetPartBoundsInRadius(position, radius, params)`
- `Workspace:GetPartsInPart(part, params)`

`RaycastParams` supports `FilterType`, `FilterDescendantsInstances`,
`IgnoreWater`, `CollisionGroup`, and `RespectCanCollide`.

`OverlapParams` supports `FilterType`, `FilterDescendantsInstances`,
`MaxParts`, `CollisionGroup`, and `RespectCanCollide`.

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

## Reference expansion

Camera behavior and the exact relationship between static geometry, player
observations, and spectator observations still need a dedicated reference once
the renderer docs settle.
