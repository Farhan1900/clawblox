---
title: Engine API reference
summary: "Which Roblox-like APIs the engine supports, fully or partially."
read_when:
  - Checking whether a Roblox API is available
  - Porting Roblox code to Clawblox
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

`Players` exposes:

- fields: `LocalPlayer`, `MaxPlayers`, `CharacterAutoLoads`, `PlayerAdded`,
  `PlayerRemoving`
- methods: `GetPlayers`, `GetPlayerByUserId`, `GetPlayerFromCharacter`,
  `GetChildren`, `FindFirstChild`, `AddBot`

`MaxPlayers` is read-only from Luau and comes from `world.toml`.
`CharacterAutoLoads` is writable; when enabled, joined players get default
characters automatically. `AddBot(name)` queues a server-side bot player for
creation on the next tick.

`PhysicsService` owns collision groups with a maximum of 32 groups:

- `RegisterCollisionGroup`, `CreateCollisionGroup`, `UnregisterCollisionGroup`,
  `RemoveCollisionGroup`, `RenameCollisionGroup`
- `CollisionGroupSetCollidable`, `CollisionGroupsAreCollidable`
- `SetPartCollisionGroup`
- `GetCollisionGroups`, `GetRegisteredCollisionGroups`, `GetCollisionGroupId`,
  `GetCollisionGroupName`, `GetMaxCollisionGroups`, `IsCollisionGroupRegistered`

Collision groups affect real contacts in the physics layer.

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
JSON-like values. `FireAllClientsUnreliable` is bounded; older unreliable
events may be dropped when the queue is full, while reliable events are kept.

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

## Datatype members

`Vector2`:

- properties: `X`, `Y`, `Magnitude`, `Unit`
- methods: `Dot`, `Lerp`, `Unit`
- operators: `+`, `-`, component and scalar `*`, component and scalar `/`,
  unary `-`

`Vector3`:

- properties: `X`, `Y`, `Z`, `Magnitude`, `Unit`
- methods: `Dot`, `Cross`, `Lerp`, `FuzzyEq`
- operators: `+`, `-`, component and scalar `*`, component and scalar `/`,
  unary `-`

`CFrame`:

- properties: `Position`, `LookVector` (forward, `-Z`), `RightVector` (`+X`),
  `UpVector` (`+Y`), `X`, `Y`, `Z`
- methods: `Inverse`, `Lerp`, `ToWorldSpace`, `ToObjectSpace`,
  `PointToWorldSpace`, `PointToObjectSpace`, `GetComponents`,
  `ToEulerAnglesXYZ`, `ToOrientation`
- operators: `cf * cf`, `cf * v3`, `cf + v3`, `cf - v3`

`GetComponents` returns 12 numbers. `ToEulerAnglesXYZ` returns radians in XYZ
order; `ToOrientation` returns radians in YXZ order.

`Color3`:

- properties: `R`, `G`, `B` (each `0..1`)
- methods: `Lerp`, `ToHSV`, `ToHex`

`UDim`:

- properties: `Scale`, `Offset`
- operators: `+`, `-`

`UDim2`:

- properties: `X`, `Y`, `Width` (alias for `X`), `Height` (alias for `Y`)
- methods: `Lerp`
- operators: `+`, `-`

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

Supported attribute value types are `nil`, boolean, number, string, `Vector3`,
`Color3`, and array-style tables of supported attribute values.

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

`BasePart` and `Part` methods: `GetMass`, `GetConnectedParts`, `ApplyImpulse`,
`ApplyImpulseAtPosition`, `ApplyAngularImpulse`, `GetPivot`, `PivotTo`,
`GetPrimaryPartCFrame`, `SetPrimaryPartCFrame`. `ApplyImpulse` applies a
world-space impulse to the part's assembly at its center of mass.
`ApplyImpulseAtPosition` applies it at a world-space position, which can add
angular velocity. `ApplyAngularImpulse` applies a world-space angular impulse.
For welded assemblies these operate on the shared physics assembly.

In Roblox-style APIs, `Friction`, `Density`, and `Elasticity` are not direct
`BasePart` fields; set them through `CustomPhysicalProperties` with
`PhysicalProperties.new(density, friction, elasticity, frictionWeight,
elasticityWeight)`, or assign `nil` to clear the override.

`Model` exposes `PrimaryPart`, plus methods `GetPrimaryPartCFrame`,
`SetPrimaryPartCFrame`, `GetPivot`, and `PivotTo`.

`Workspace.Terrain` is created automatically and exposes:

- `FillBlock(cframe, size, material)`
- `ReadVoxelChannels(region, resolution, channelIds)`
- `WriteVoxelChannels(region, resolution, channels)`

Terrain voxel channel resolution must be `4`. `Region3` bounds must be aligned
to that grid and contain at least one voxel. Supported channel names are
`SolidMaterial`, `SolidOccupancy`, and `LiquidOccupancy`; `SolidOccupancy` and
`LiquidOccupancy` values are clamped to `0..1`, and empty voxels are removed
from terrain storage.

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
- methods: `TakeDamage`, `Move`, `MoveTo`, `CancelMoveTo`, `Jump`,
  `GetState`, `ChangeState`

`Player` exposes `UserId`, `DisplayName`, `Character`, `CharacterAdded`,
`CharacterRemoving`, and `PlayerGui`.

`Player` methods: `LoadCharacter`, `Kick`.

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
- `Humanoid:LoadAnimation`, which creates or reuses a child `Animator`
- `Sound.StreamId`, `Volume`, `PlaybackSpeed`, `IsPlaying`, `Ended`,
  `Stopped`, `Play`, `Stop`, `MarkEnded`

`Sound:Play()` requires a non-empty `StreamId`. `Sound:Stop()` is idempotent and
fires `Stopped` only when transitioning from playing to stopped. `MarkEnded()`
is Clawblox-specific and lets the renderer/runtime mark playback complete; it
fires `Ended` and `Stopped` when the sound was playing.

Value objects (`IntValue`, `NumberValue`, `BoolValue`, `StringValue`,
`ObjectValue`, `CFrameValue`) expose `Value` and `Changed`. `ObjectValue.Value`
accepts an `Instance` or `nil`; `CFrameValue.Value` accepts a `CFrame`.

`Script` and `ModuleScript` expose `Source` and `Disabled`.

`RemoteEvent` instances expose `OnServerEvent`, `FireAllClients`, and
`FireClient`. Instance events use the `RemoteEvent` instance name as the event
name and serialize Lua arguments into JSON-like payloads. With no arguments the
payload is `null`; with one argument the payload is that value; with multiple
arguments the payload is an array. `FireClient(player, ...)` targets one player
and is excluded from public spectator observations.

## Property defaults

Documented non-obvious defaults:

- `Workspace.Gravity`: `196.2`
- `BasePart`: `Size = (4, 1, 2)`, `Color = (0.6, 0.6, 0.6)`,
  `Transparency = 0`, `Anchored = false`, `CanCollide = true`,
  `CanQuery = true`, `CanTouch = true`, `Material = Plastic`,
  `Massless = false`, `CustomPhysicalProperties = nil`
- `Humanoid`: `Health = 100`, `MaxHealth = 100`, `WalkSpeed = 16`,
  `JumpPower = 50`, `JumpHeight = 7.2`, `AutoRotate = true`, `HipHeight = 2`
- GUI base: `BackgroundColor3 = (1, 1, 1)`, `BackgroundTransparency = 0`,
  `BorderColor3 = (0.1, 0.1, 0.1)`, `BorderSizePixel = 1`, `ZIndex = 0`,
  `LayoutOrder = 0`, `Visible = true`, `AnchorPoint = (0, 0)`, `Rotation = 0`
- `ScreenGui`: `DisplayOrder = 0`, `IgnoreGuiInset = false`, `Enabled = true`
- `BillboardGui`: `Size = {0,100,0,50}`, `AlwaysOnTop = false`,
  `Enabled = true`, `Adornee = nil`
- `TextLabel`: `Text = ""`, `TextColor3 = (0, 0, 0)`, `TextSize = 14`
  (minimum `1`), `TextTransparency = 0`, `TextScaled = false`,
  `TextXAlignment = Center`, `TextYAlignment = Center`
- `ImageLabel`: `Image = ""`, `ImageColor3 = (1, 1, 1)`, `ImageTransparency = 0`

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
