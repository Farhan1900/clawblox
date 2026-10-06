---
title: Luau scripting
summary: "Roblox-like Luau globals, services, scheduling, and resumability."
read_when:
  - Writing world logic in Luau
  - Looking up a global, service, or scheduling API
---

Clawblox implements a Roblox-like Luau scripting system for engine worlds. Many
Roblox patterns work, but the supported surface is the Clawblox subset
documented here and in the API reference.

## Global Objects

### game
The Roblox-style root object for the world data model.

```lua
local Workspace = game:GetService("Workspace")
local Players = game:GetService("Players")
local RunService = game:GetService("RunService")
local ServerStorage = game:GetService("ServerStorage")
```

### Workspace
Global reference to `game:GetService("Workspace")`.

### Players
Global reference to `game:GetService("Players")`.

### ServerStorage
Global reference to `game:GetService("ServerStorage")`.

`ServerStorage` is server-only state. It is not part of agent observations, but it is part of the engine-owned world state and is included in resume snapshots.

Use it for authoritative hidden world state that must survive resume but should not be visible to agents.

Examples:

- per-player effect timers
- creature AI state
- world-global counters and spawn indices
- hidden references between gameplay objects

Do not use `ServerStorage` for static design-time constants. Put fixed parameters in code or config.

## Resumability Contract

Clawblox resume is engine-level and strict. The engine snapshots authoritative
state that lives in engine-managed APIs, and it also captures Lua VM state for
supported runtime objects.

Resumable by contract:

- the DataModel instance tree
- `Workspace`
- `ServerStorage`
- `ReplicatedStorage`
- instance properties
- instance attributes
- player objects and character state
- physics state attached to engine objects
- documented engine service state
- pending Lua coroutines, `task.wait` schedules, yielded signal callbacks,
  `RunService` connections, module cache, animation tracks, and
  `BindToClose` callbacks
- supported Clawblox userdata such as instances, services, `RaycastParams`,
  `OverlapParams`, `RaycastResult`, animation tracks, and DataStore handles

Do not make these the only source of durable gameplay truth:

- browser-only renderer state
- external files, network side effects, or process-local handles
- unsupported host userdata
- hidden Lua tables that are difficult for agents to inspect or repair

Authoring rule:

- if losing a value on restart would change gameplay, store it in engine-managed state
- if a value can be recomputed from engine-managed state, it can stay local

Good pattern:

```lua
local ServerStorage = game:GetService("ServerStorage")
local stateFolder = ServerStorage:FindFirstChild("WorldState") or Instance.new("Folder")
stateFolder.Name = "WorldState"
stateFolder.Parent = ServerStorage
stateFolder:SetAttribute("NextSpawnIndex", 12)
```

Bad pattern:

```lua
local nextSpawnIndex = 12 -- only copy of authoritative state
```

---

## Global Functions

### tick()
Returns the time in seconds since the world started. Used for timing and animations.

```lua
local startTime = tick()
-- ... later
local elapsed = tick() - startTime
print("Elapsed:", elapsed, "seconds")
```

**Note:** Unlike Roblox's `tick()` which returns Unix epoch time, Clawblox returns time since world start for simplicity.

### wait(seconds?)
Yields the current thread for the specified duration (default: 0 seconds / 1 frame).

```lua
wait(2)  -- Wait 2 seconds
wait()   -- Wait 1 frame
```

Delegates to `task.wait()`. Returns the actual elapsed time.

### print(...)
Outputs to the world console.

```lua
print("Hello", "World", 42)  -- Output: Hello	World	42
```

### warn(...)
Outputs a warning to the world console.

```lua
warn("Something unexpected happened")
```

---

## task Library

The `task` library provides thread scheduling functions, matching the [Roblox task library](https://create.roblox.com/docs/reference/engine/libraries/task).

### task.spawn(functionOrThread, ...args)
Creates a new coroutine and resumes it immediately with the given arguments. If passed an existing thread, resumes that thread instead.

```lua
task.spawn(function(msg)
    print(msg)  -- prints immediately
end, "Hello")
```

Returns the thread.

### task.delay(seconds, function, ...args)
Schedules a function to run after `seconds` have elapsed. The function receives the provided arguments.

```lua
task.delay(2, function()
    print("2 seconds later")
end)

task.delay(1, function(a, b)
    print(a + b)  -- prints 30 after 1 second
end, 10, 20)
```

Returns the thread (can be passed to `task.cancel`).

### task.defer(functionOrThread, ...args)
Schedules a function or thread to run on the next resumption cycle (next tick). Similar to `task.spawn` but does not resume immediately.

```lua
task.defer(function()
    print("runs next tick")
end)
```

Returns the thread.

### task.wait(seconds?)
Yields the current thread for `seconds` (default: 0, meaning resume next tick). Returns the actual elapsed time.

```lua
local elapsed = task.wait(1)
print("Waited", elapsed, "seconds")
```

### task.cancel(thread)
Cancels a scheduled thread so it never runs.

```lua
local t = task.delay(5, function()
    print("this will never print")
end)
task.cancel(t)
```

---

## Classes

The full class, property, method, event, and default tables live in the
[engine API reference](/engine-api-reference). The data model concepts behind
instances, parenting, and attributes are in [data model](/data-model). This
section shows the common authoring patterns.

Every object is an `Instance`. Construct one with `Instance.new`:

```lua
local part = Instance.new("Part")
local part = Instance.new("Part", parent)  -- with parent
```

A `Part` is the basic physical object:

```lua
local part = Instance.new("Part")
part.Shape = Enum.PartType.Ball
part.Size = Vector3.new(4, 4, 4)
part.Position = Vector3.new(0, 10, 0)
part.Anchored = false
part.Parent = Workspace
```

`Folder` groups instances:

```lua
local folder = Instance.new("Folder")
folder.Name = "Weapons"
folder.Parent = Workspace
```

`Weld` rigidly attaches two parts:

```lua
local weld = Instance.new("Weld")
weld.Part0 = partA
weld.Part1 = partB
weld.C0 = CFrame.new(0, 1, 0)
weld.Parent = partA
```

### Physics properties

In Roblox-style APIs, `Friction`, `Density`, and `Elasticity` are not direct
`BasePart` fields. Use `CustomPhysicalProperties` instead:

```lua
local part = Instance.new("Part")
part.CustomPhysicalProperties = PhysicalProperties.new(
    1.0, -- density
    0.9, -- friction
    0.05, -- elasticity
    1.0, -- friction weight
    1.0 -- elasticity weight
)
```

To clear the override:

```lua
part.CustomPhysicalProperties = nil
```

`BasePart` also exposes impulse methods (`ApplyImpulse`,
`ApplyImpulseAtPosition`, `ApplyAngularImpulse`) and `GetMass`. For welded
assemblies these operate on the shared physics assembly. Use
`ApplyImpulse(velocity * assemblyMass)` when you want to give an assembly a
target linear velocity without injecting artificial spin. See
[physics](/physics) for the full forces and constraints workflow.

## Services

### Players
Manages connected players. See the [engine API reference](/engine-api-reference)
for the full Players API and [players and characters](/players-and-characters)
for character and humanoid behavior.

```lua
local Players = game:GetService("Players")

Players.PlayerAdded:Connect(function(player)
    print(player.Name .. " joined!")

    player.CharacterAdded:Connect(function(character)
        local humanoid = character:FindFirstChild("Humanoid")
        humanoid.MaxHealth = 200
        humanoid.Health = 200
    end)
end)
```

---

### Workspace
The 3D world container. See the [engine API reference](/engine-api-reference)
for the full Workspace API and [3D workspace](/workspace-3d) for spatial query
and terrain details.

```lua
-- Raycast example
local result = Workspace:Raycast(origin, direction * 100)
if result then
    print("Hit:", result.Instance.Name)
    print("Position:", result.Position)
    print("Normal:", result.Normal)
    print("Distance:", result.Distance)
end
```

---

### RunService
Frame loop and timing. `Stepped` fires before physics with `(time, deltaTime)`;
`Heartbeat` fires after physics with `(deltaTime)`. `IsServer()` and
`IsClient()` report the runtime side. See the
[engine API reference](/engine-api-reference) for the full RunService API.

```lua
local RunService = game:GetService("RunService")

RunService.Heartbeat:Connect(function(deltaTime)
    -- Runs every frame
end)
```

---

### HttpService
Provides JSON encoding and decoding through `JSONEncode(value)` and
`JSONDecode(json)`. Invalid JSON decodes to `nil`; the service is not an
outbound HTTP client. See the [engine API reference](/engine-api-reference) and
[services](/services) for details.

```lua
local HttpService = game:GetService("HttpService")

local data = { score = 100, name = "Player1", items = {"sword", "shield"} }
local json = HttpService:JSONEncode(data)
print(json) -- {"items":["sword","shield"],"name":"Player1","score":100}

local decoded = HttpService:JSONDecode(json)
print(decoded.score) -- 100
```

---

### DataStoreService
Persistent key-value storage backed by the database. `GetDataStore(name)`
returns a store with the yielding methods `GetAsync`, `SetAsync`, `RemoveAsync`,
and `UpdateAsync`. `GetOrderedDataStore(name)` returns an ordered store with
`SetAsync` (values must include a `score` field), `GetSortedAsync`, and
`GetAsync`. See the [engine API reference](/engine-api-reference) for signatures
and [services](/services) for `UpdateAsync` retry semantics and datastore
backends.

```lua
local DataStoreService = game:GetService("DataStoreService")

-- Basic key-value storage
local playerStore = DataStoreService:GetDataStore("PlayerData")
playerStore:SetAsync("player_123", { coins = 500, level = 5 })
local data = playerStore:GetAsync("player_123")
print(data.coins) -- 500

-- Atomic update
playerStore:UpdateAsync("player_123", function(old)
    old.coins = old.coins + 100
    return old
end)

-- Ordered leaderboard
local leaderboard = DataStoreService:GetOrderedDataStore("Leaderboard")
leaderboard:SetAsync("player_123", { score = 1500 })
local top10 = leaderboard:GetSortedAsync(false, 10)
for _, entry in ipairs(top10) do
    print(entry.key, entry.value.score)
end
```

---

### AgentInputService

**Clawblox extension**: handles input from coding agents via the local runtime API.

Worlds should support both renderer-driven player input and coding-agent input
through `AgentInputService` so the same Luau code works for all player types.
It fires `InputReceived(player, inputType, data)` and exposes `GetInputs(player)`
(get and clear pending inputs) and `HasPendingInputs(player)`. See the
[engine API reference](/engine-api-reference) and
[observations and input](/observations-and-input) for the full contract.

#### Input Flow

```
Agent or renderer  →  POST /input  →  AgentInputService  →  InputReceived event  →  World script
```

#### Usage

**Event-based** (recommended for discrete actions like Fire, Melee):
```lua
local AgentInputService = game:GetService("AgentInputService")

AgentInputService.InputReceived:Connect(function(player, inputType, data)
    if inputType == "Fire" then
        -- data.direction is {dx, dy, dz}
        local dir = data.direction
        fireWeapon(player, Vector3.new(dir[1], dir[2], dir[3]))

    elseif inputType == "MoveTo" then
        -- data.position is {x, y, z}
        local pos = data.position
        local humanoid = player.Character:FindFirstChild("Humanoid")
        if humanoid then
            humanoid:MoveTo(Vector3.new(pos[1], pos[2], pos[3]))
        end

    elseif inputType == "Melee" then
        meleeAttack(player)
    end
end)
```

**Polling** (for continuous state):
```lua
RunService.Heartbeat:Connect(function(dt)
    for _, player in ipairs(Players:GetPlayers()) do
        local inputs = AgentInputService:GetInputs(player)
        for _, input in ipairs(inputs) do
            processInput(player, input.type, input.data)
        end
    end
end)
```

---

## GUI

GUI classes build 2D interfaces. `Frame`, `TextLabel`, `TextButton`,
`ImageLabel`, and `ImageButton` inherit the shared `GuiObject` layout and visual
properties; `TextButton` and `ImageButton` also fire `MouseButton1Click`,
`MouseButton1Down`, `MouseButton1Up`, `MouseEnter`, and `MouseLeave`. See the
[engine API reference](/engine-api-reference) for the full GUI property,
default, and event tables.

Parent a `ScreenGui` under a player's `PlayerGui` for on-screen interfaces, or
use a `BillboardGui` to attach a GUI to a part in 3D space:

```lua
local billboard = Instance.new("BillboardGui")
billboard.Size = UDim2.fromOffset(100, 30)
billboard.StudsOffset = Vector3.new(0, 3, 0)
billboard.AlwaysOnTop = true
billboard.Adornee = character:FindFirstChild("HumanoidRootPart")
billboard.Parent = character
```

### GUI Example

```lua
local Players = game:GetService("Players")

Players.PlayerAdded:Connect(function(player)
    player.CharacterAdded:Connect(function()
        local screenGui = Instance.new("ScreenGui")
        screenGui.Parent = player.PlayerGui

        -- Health bar background
        local bg = Instance.new("Frame")
        bg.Size = UDim2.fromOffset(200, 20)
        bg.Position = UDim2.fromOffset(10, 10)
        bg.BackgroundColor3 = Color3.fromRGB(40, 40, 40)
        bg.Parent = screenGui

        -- Health bar fill
        local fill = Instance.new("Frame")
        fill.Size = UDim2.fromScale(1, 1)
        fill.BackgroundColor3 = Color3.fromRGB(0, 200, 0)
        fill.Parent = bg

        -- Health text
        local label = Instance.new("TextLabel")
        label.Size = UDim2.fromScale(1, 1)
        label.BackgroundTransparency = 1
        label.Text = "100 / 100"
        label.TextColor3 = Color3.new(1, 1, 1)
        label.TextSize = 14
        label.Parent = bg
    end)
end)
```

---

## Agent API docs

Worlds define their controls and rules in an `API.md` file. AI agents read
this file to learn how to operate the world. The path is configured by
`scripts.api` in `world.toml`.

**Location**: `API.md` in the world directory.

The API.md file should include:

- available input types and their data format
- observation fields the agent should rely on
- world rules and mechanics
- success/failure conditions for actions
- strategy or examples when useful

### Example Structure

```text
mesa-island/
  world.toml
  API.md
  main.lua
```

### Observation Format

Local observations are returned by `GET /observe` with the `X-Session` header
returned by `POST /join?name=...`. `POST /input` also returns a fresh
observation after the next input-processing tick.

```json
{
  "tick": 1234,
  "game_status": "playing",
  "player": {
    "id": "uuid",
    "position": [x, y, z],
    "health": 100,
    "attributes": { ... }  // World-specific data set via SetAttribute
  },
  "other_players": [ ... ],
  "world": {
    "entities": [ ... ]  // Dynamic (non-static) workspace entities
  },
  "events": [],
  "recent_events": [ ... ]
}
```

The `attributes` field contains whatever the world script sets via
`player:SetAttribute()`. This keeps the engine generic while allowing worlds to
define their own data.

The `world` field contains nearby dynamic workspace entities: parts and
attribute-bearing folders that do **not** have the `"Static"` tag. Parts are
filtered by the configured observation radius. Static geometry should be
documented in `API.md` when agents need to reason about it, or exposed through
world-specific attributes/actions. Browser spectator rendering uses a separate
spectator observation stream.

---

## Data Types

The engine registers Roblox-like datatypes including `Vector2`, `Vector3`,
`CFrame`, `Color3`, `UDim`, `UDim2`, `Ray`, `Region3`, `PhysicalProperties`,
`BrickColor`, and `DateTime`, plus the query types `RaycastResult`,
`RaycastParams`, and `OverlapParams`. See the
[engine API reference](/engine-api-reference) for the full constructor, member,
and operator tables.

Spatial query parameters configure raycasts and overlap queries:

```lua
local params = RaycastParams.new()
params.FilterType = Enum.RaycastFilterType.Exclude
params.FilterDescendantsInstances = {character}

local result = Workspace:Raycast(origin, direction, params)
```

```lua
local params = OverlapParams.new()
params.FilterType = Enum.RaycastFilterType.Exclude
params.FilterDescendantsInstances = {character}
params.MaxParts = 20

local parts = Workspace:GetPartBoundsInRadius(Vector3.new(0, 5, 0), 12, params)
```

---

## Enums

The engine implements Roblox-like enum families including `Enum.PartType`
(with the legacy `Enum.PartShape` alias), `Enum.Material`,
`Enum.HumanoidStateType`, `Enum.RaycastFilterType`, `Enum.ActuatorRelativeTo`,
`Enum.FluidForces`, `Enum.TextXAlignment`, `Enum.TextYAlignment`, `Enum.Font`,
and `Enum.SurfaceType`. See the [engine API reference](/engine-api-reference)
for the full list of enum values.

---

## Events Pattern

Clawblox uses the Roblox `:Connect()` pattern for events:

```lua
local connection = event:Connect(function(...)
    -- handler
end)

-- Later, to disconnect:
connection:Disconnect()

-- One-time listener:
event:Once(function(...)
    -- fires once then auto-disconnects
end)
```

---

## 3D models (GLB) and assets

You can render a 3D model (`.glb` file) on a Part instead of the default primitive shape by setting the `ModelUrl` attribute.

### Asset Protocol (`asset://`)

Place world assets (models, images, audio) in the `assets/` directory and reference them using the `asset://` protocol:

```lua
part:SetAttribute("ModelUrl", "asset://models/tree.glb")
```

The local runtime resolves `asset://` URLs to `/assets/...`, served from the
world's `assets/` directory.

World authors should normally use `asset://` rather than hard-coded runtime
paths.

**Allowed file types:** `.glb`, `.gltf`, `.png`, `.jpg`, `.jpeg`, `.wav`, `.mp3`, `.ogg`

When using deployment flows, assets under `assets/` are the intended portable
asset source.

### Legacy: Static Files

For backwards compatibility, URLs starting with `/static/` are still supported. Place files in a `static/` directory and reference them directly:

```lua
part:SetAttribute("ModelUrl", "/static/models/knight.glb")
```

Use `asset://` for new worlds unless you specifically need compatibility with
older `/static/` references.

### Skeletal Animations

GLB models with skeletal animations are supported. The engine auto-detects animations named `walk`, `run`, and `idle` and plays them based on the character's movement state.

### Example

```lua
local character = Instance.new("Part")
character.Name = "Knight"
character.Size = Vector3.new(2, 4, 2)
character.Position = Vector3.new(0, 2, 0)
character.Anchored = true
character:SetAttribute("ModelUrl", "asset://models/knight.glb")
character.Parent = Workspace
```

---

## Example: Complete world

```lua
-- Chase world: zombies chase players

local Players = game:GetService("Players")
local RunService = game:GetService("RunService")

local zombies = {}
local ZOMBIE_SPEED = 10
local ZOMBIE_DAMAGE = 15
local SPAWN_INTERVAL = 5

local lastSpawn = 0

-- Spawn a zombie at random position
local function spawnZombie()
    local zombie = Instance.new("Part")
    zombie.Name = "Zombie"
    zombie.Size = Vector3.new(4, 6, 2)
    zombie.Color = Color3.fromRGB(0, 150, 0)
    zombie.Position = Vector3.new(
        math.random(-50, 50),
        3,
        math.random(-50, 50)
    )
    zombie.Anchored = true
    zombie:SetAttribute("Health", 100)
    zombie.Parent = Workspace

    table.insert(zombies, zombie)
    return zombie
end

-- Find nearest player to a position
local function getNearestPlayer(position)
    local nearest = nil
    local nearestDist = math.huge

    for _, player in ipairs(Players:GetPlayers()) do
        local character = player.Character
        if character then
            local humanoid = character:FindFirstChild("Humanoid")
            local rootPart = character:FindFirstChild("HumanoidRootPart")

            if humanoid and humanoid.Health > 0 and rootPart then
                local dist = (rootPart.Position - position).Magnitude
                if dist < nearestDist then
                    nearest = player
                    nearestDist = dist
                end
            end
        end
    end

    return nearest, nearestDist
end

-- Main world loop
RunService.Heartbeat:Connect(function(dt)
    -- Spawn zombies periodically
    lastSpawn = lastSpawn + dt
    if lastSpawn >= SPAWN_INTERVAL then
        spawnZombie()
        lastSpawn = 0
    end

    -- Update zombies
    for i = #zombies, 1, -1 do
        local zombie = zombies[i]

        if zombie:GetAttribute("Health") <= 0 then
            zombie:Destroy()
            table.remove(zombies, i)
        else
            local nearest, dist = getNearestPlayer(zombie.Position)

            if nearest and dist < 100 then
                local character = nearest.Character
                local rootPart = character:FindFirstChild("HumanoidRootPart")

                if dist < 3 then
                    -- Attack!
                    local humanoid = character:FindFirstChild("Humanoid")
                    if humanoid then
                        humanoid:TakeDamage(ZOMBIE_DAMAGE * dt)
                    end
                else
                    -- Chase
                    local direction = (rootPart.Position - zombie.Position).Unit
                    zombie.Position = zombie.Position + direction * ZOMBIE_SPEED * dt
                end
            end
        end
    end
end)

-- Handle player joining
Players.PlayerAdded:Connect(function(player)
    print(player.Name .. " joined the world!")
end)

print("Zombie Chase loaded!")
```
