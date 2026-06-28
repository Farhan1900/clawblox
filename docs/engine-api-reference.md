---
title: Engine API reference
description: Current supported and partially supported Roblox-like APIs in the Clawblox engine.
---

This page is the index for the Clawblox engine API surface. It should answer:
"Does Clawblox support this Roblox class, property, method, event, datatype, or
enum?"

## Status labels

Use these labels when expanding this reference:

- **Supported**: intended to work for normal world code
- **Partial**: implemented, but missing Roblox behavior or edge cases
- **Compatibility no-op**: accepted so Roblox-like scripts can run, but not
  modeled by the engine
- **Clawblox-specific**: intentionally not a Roblox API
- **Indexed**: implemented surface listed here, with details documented on the
  topic page or in a later table on this page

## Classes

`Instance.new(className, parent?)` supports:

- `Part`
- `Model`
- `Humanoid`
- `Player`
- `Folder`
- `Terrain`
- `Animation`
- `Animator`
- `Script`
- `ModuleScript`
- `ServerScriptService`
- `ServerStorage`
- `IntValue`
- `NumberValue`
- `BoolValue`
- `StringValue`
- `ObjectValue`
- `CFrameValue`
- `Weld`
- `WeldConstraint`
- `RemoteEvent`
- `BodyVelocity`
- `BodyForce`
- `BodyAngularVelocity`
- `Attachment`
- `VectorForce`
- `AlignPosition`
- `AlignOrientation`
- `BillboardGui`
- `ScreenGui`
- `Frame`
- `TextLabel`
- `TextButton`
- `ImageLabel`
- `ImageButton`
- `UICorner`
- `Sound`

The engine also implements service or singleton classes that are not generally
constructed with `Instance.new`:

- `Instance`
- `Workspace`
- `Players`
- `RunService`
- `Camera`
- `PlayerGui`
- `BasePart`
- `ReplicatedStorage`

`BasePart` is an inheritance/base-class name for `Part`. Use `Instance.new("Part")`
when creating physical parts. `PlayerGui` is created for `Player` instances by
the Players service rather than directly constructed by world code.
`ReplicatedStorage` is available through `game:GetService("ReplicatedStorage")`.

`IsA` includes these compatibility base classes:

- `Instance`
- `BasePart`
- `ValueBase`
- `Constraint`
- `BodyMover`
- `GuiBase2d`
- `LayerCollector`
- `GuiObject`
- `GuiButton`
- `UIComponent`

## Services

Available through `game:GetService`:

- `Workspace`
- `Players`
- `RunService`
- `AgentInputService`
- `DataStoreService`
- `Debris` / `DebrisService`
- `PhysicsService`
- `RemoteEventService`
- `ServerScriptService`
- `ServerStorage`
- `ReplicatedStorage`
- `HttpService`

Unknown service names return `nil`.

The `game.ServiceName` shortcut is available for common singleton services:
`Workspace`, `Players`, `RunService`, `PhysicsService`, `RemoteEventService`,
`Debris` / `DebrisService`, `ServerScriptService`, `ServerStorage`, and
`ReplicatedStorage`. Use `game:GetService("HttpService")`,
`game:GetService("AgentInputService")`, and
`game:GetService("DataStoreService")` for services that are not exposed through
the shortcut.

`game:BindToClose(callback)` registers shutdown callbacks. Registered
callbacks are included in Lua VM snapshots.

`DataStoreService:GetDataStore(name)` returns a key-value store with
`GetAsync`, `SetAsync`, `RemoveAsync`, and `UpdateAsync`.
`UpdateAsync` uses compare-and-set retry semantics for Roblox-style atomic
updates. Returning `nil` from the transform callback cancels the update.
Values are serialized through JSON and require a configured datastore backend.

`DataStoreService:GetOrderedDataStore(name)` returns an ordered store with
`SetAsync`, `GetSortedAsync`, and `GetAsync`. Ordered store values must include
a `score` field; `GetSortedAsync(ascending, limit)` returns `{ key, value }`
entries sorted by that score.

`RunService` exposes:

- fields: `Heartbeat`, `Stepped`
- methods: `IsServer()`, `IsClient()`

`Stepped` fires before physics with `(time, deltaTime)`. `Heartbeat` fires
after physics with `(deltaTime)`.

`AgentInputService` exposes:

- field: `InputReceived`
- methods: `GetInputs(player)`, `HasPendingInputs(player)`

`GetInputs(player)` returns an array of `{ type, data }` entries and clears the
pending queue for that player.

`InputReceived` fires with `(player, inputType, inputData)`. JSON input values
are converted into Luau values: objects become tables with string keys, arrays
become 1-indexed tables, numbers become Lua numbers/integers, booleans become
booleans, strings become strings, and JSON null becomes `nil`.

`Debris:AddItem(instance, lifetime?)` destroys an instance after an
engine-time delay. The default lifetime is `10` seconds.

`RemoteEventService` exposes:

- `FireAllClients(name, payload)`
- `FireAllClientsUnreliable(name, payload)`

Empty event names are ignored. Payloads are converted from Lua values to
JSON-like values.

`HttpService` exposes `JSONEncode(value)` and `JSONDecode(jsonString)`.
Invalid JSON decodes to `nil`; the engine does not expose outbound HTTP
requests through this service.

## Datatypes and globals

Registered globals include:

- `Vector2`
- `Vector3`
- `CFrame`
- `Color3`
- `DateTime`
- `BrickColor`
- `Enum`
- `PhysicalProperties`
- `Ray`
- `Region3`
- `UDim`
- `UDim2`
- `RaycastParams`
- `OverlapParams`
- `task`
- `coroutine`
- `table`
- `math`
- `string`
- `utf8`
- `bit32`
- `buffer`

Important constructors and helpers:

- `Vector2.new(x?, y?)`; constants `zero`, `one`, `xAxis`, `yAxis`
- `Vector3.new(x?, y?, z?)`; constants `zero`, `one`, `xAxis`, `yAxis`,
  `zAxis`
- `CFrame.new()`, `CFrame.new(Vector3)`, `CFrame.new(x, y, z)`,
  `CFrame.new(position, target)`, `CFrame.lookAt`,
  `CFrame.fromEulerAnglesXYZ`, `CFrame.Angles`, `CFrame.fromOrientation`
- `Color3.new`, `Color3.fromRGB`, `Color3.fromHSV`, `Color3.fromHex`
- `UDim.new`, `UDim2.new`, `UDim2.fromScale`, `UDim2.fromOffset`
- `Ray.new`, `Region3.new`, `PhysicalProperties.new`
- `DateTime.now`, `DateTime.fromUnixTimestamp`,
  `DateTime.fromUnixTimestampMillis`
- `BrickColor.new`, `BrickColor.Gray`

## Query result types

`RaycastResult` exposes:

- `Instance`
- `Position`
- `Normal`
- `Distance`
- `Material`

`RaycastParams` exposes:

- `FilterType`
- `FilterDescendantsInstances`
- `IgnoreWater`
- `CollisionGroup`
- `RespectCanCollide`

`OverlapParams` exposes:

- `FilterType`
- `FilterDescendantsInstances`
- `MaxParts`
- `CollisionGroup`
- `RespectCanCollide`

Both parameter types default to `FilterType = Enum.RaycastFilterType.Exclude`,
an empty filter list, `CollisionGroup = "Default"`, and
`RespectCanCollide = false`. `RaycastParams.IgnoreWater` defaults to `false`.
`OverlapParams.MaxParts = 0` means unlimited results.

## Instance fields and methods

All instances expose core identity, parenting, tree, attribute, tag, and signal
APIs:

- fields: `Name`, `ClassName`, `Parent`, `ChildAdded`, `ChildRemoved`,
  `DescendantAdded`, `DescendantRemoving`, `AncestryChanged`, `Destroying`,
  `AttributeChanged`
- methods: `SetAttribute`, `GetAttribute`, `GetAttributes`,
  `GetAttributeChangedSignal`, `GetPropertyChangedSignal`, `AddTag`, `HasTag`,
  `RemoveTag`, `GetTags`, `Clone`, `Destroy`, `ClearAllChildren`,
  `FindFirstChild`, `FindFirstChildOfClass`, `FindFirstChildWhichIsA`,
  `GetChildren`, `GetDescendants`, `GetDebugId`, `IsA`, `IsDescendantOf`,
  `WaitForChild`

## Major class property groups

`BasePart` and `Part` expose transform, physics, material, and touch state:

- `Position`, `CFrame`, `Size`, `Anchored`, `CanCollide`, `CanQuery`,
  `CanTouch`, `CollisionGroup`, `Transparency`, `Color`, `BrickColor`,
  `Material`, `TopSurface`, `BottomSurface`, `Velocity`,
  `AssemblyLinearVelocity`, `AssemblyAngularVelocity`, `Massless`,
  `EnableFluidForces`, `CustomPhysicalProperties`,
  `CurrentPhysicalProperties`, `Density`, `SpecificGravity`, `Friction`,
  `Elasticity`, `FrictionWeight`, `ElasticityWeight`, `Shape`, `Touched`,
  `TouchEnded`

`Model` exposes `PrimaryPart`.

`Workspace` exposes:

- fields: `Gravity`, `FluidForces`, `FallenPartsDestroyHeight`,
  `CurrentCamera`, `ChildAdded`, `ChildRemoved`, `DescendantAdded`,
  `DescendantRemoving`, `AncestryChanged`
- methods: `Raycast`, `FindPartOnRay`, `FindPartOnRayWithIgnoreList`,
  `GetPartBoundsInBox`, `GetPartBoundsInRadius`, `GetPartsInPart`,
  `GetChildren`, `GetDescendants`, `FindFirstChild`, `WaitForChild`,
  `FindFirstChildOfClass`

`FindPartOnRay` and `FindPartOnRayWithIgnoreList` return the hit instance and
hit position, or `nil` when nothing is hit.

`Humanoid` exposes health, movement, display, and state events:

- `Health`, `MaxHealth`, `WalkSpeed`, `JumpPower`, `JumpHeight`, `Jump`,
  `AutoRotate`, `HealthDisplayDistance`, `NameDisplayDistance`, `HipHeight`,
  `MoveDirection`, `Died`, `HealthChanged`, `Running`, `MoveToFinished`,
  `StateChanged`

`Player` exposes `UserId`, `DisplayName`, `Character`, `CharacterAdded`,
`CharacterRemoving`, and `PlayerGui`.

GUI objects expose Roblox-like layout and visual fields:

- `Position`, `Size`, `AnchorPoint`, `Rotation`, `ZIndex`, `LayoutOrder`,
  `Visible`, `BackgroundColor3`, `BackgroundTransparency`, `BorderColor3`,
  `BorderSizePixel`, `Text`, `TextColor3`, `TextSize`, `TextTransparency`,
  `TextStrokeTransparency`, `Font`, `TextScaled`, `TextXAlignment`,
  `TextYAlignment`, `Image`, `ImageColor3`, `ImageTransparency`,
  `DisplayOrder`, `IgnoreGuiInset`, `ResetOnSpawn`, `Enabled`,
  `MouseButton1Click`, `MouseButton1Down`, `MouseButton1Up`, `MouseEnter`,
  `MouseLeave`

`BillboardGui` also exposes `StudsOffset`, `AlwaysOnTop`, `Adornee`, and
`Enabled`.

Physics constraint and actuator classes expose:

- `Weld` / `WeldConstraint`: `Part0`, `Part1`, `C0`, `C1`, `Enabled`
- `BodyVelocity`: `Velocity`, `MaxForce`, `P`
- `BodyForce`: `Force`
- `BodyAngularVelocity`: `AngularVelocity`, `MaxTorque`, `P`
- `VectorForce`: `Attachment0`, `Attachment1`, `Force`, `RelativeTo`,
  `ApplyAtCenterOfMass`, `Enabled`
- `AlignPosition`: `Attachment0`, `Attachment1`, `Position`,
  `Responsiveness`, `RigidityEnabled`, `MaxForce`, `MaxVelocity`
- `AlignOrientation`: `Attachment0`, `Attachment1`, `Responsiveness`,
  `RigidityEnabled`, `MaxTorque`

Media classes expose:

- `Animation.AnimationId`
- `Animator:LoadAnimation`, `Animator:GetPlayingAnimationTracks`
- `Sound.StreamId`, `Volume`, `PlaybackSpeed`, `IsPlaying`, `Ended`,
  `Stopped`, `Play`, `Stop`, `MarkEnded`

## Enums

Implemented enum families include:

- `Enum.PartType`: `Ball`, `Block`, `Cylinder`, `Ring`, `Wedge`
- `Enum.PartShape`: legacy alias with the same values as `PartType`
- `Enum.Material`: `Air`, `Plastic`, `Wood`, `Metal`, `Glass`, `Neon`,
  `Concrete`, `Brick`, `Granite`, `Grass`, `Ice`, `Sand`, `Fabric`, `Marble`,
  `Slate`, `SmoothPlastic`, `ForceField`, `Water`
- `Enum.ActuatorRelativeTo`: `Attachment0`, `Attachment1`, `World`
- `Enum.FluidForces`: `Default`, `Experimental`
- `Enum.HumanoidStateType`: `Running`, `Landed`, `Jumping`, `Freefall`,
  `Dead`, `Physics`, `None`
- `Enum.RaycastFilterType`: `Include`, `Exclude`; aliases `Whitelist` and
  `Blacklist`
- `Enum.TextXAlignment`: `Left`, `Center`, `Right`
- `Enum.TextYAlignment`: `Top`, `Center`, `Bottom`
- `Enum.Font`: `GothamBold`, `Gotham`, `SourceSans`, `SourceSansBold`
- `Enum.SurfaceType`: `Smooth`, `Studs`, `Inlet`, `Universal`, `Weld`,
  `Glue`, `Hinge`, `Motor`, `SteppingMotor`, `SmoothNoOutlines`

## Compatibility no-ops

`BasePart:SetNetworkOwner` and `BasePart:SetNetworkOwnershipAuto` are accepted,
but Clawblox currently runs server-authoritative simulation, so network
ownership is not modeled.
