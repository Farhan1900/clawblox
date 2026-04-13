-- Mesa World 2 - Mesa world with pickup, placement, and throwing

local RunService = game:GetService("RunService")
local Players = game:GetService("Players")
local AgentInputService = game:GetService("AgentInputService")

-- Workspace.Gravity = 196.2
Workspace.Gravity = 10

local BLOCK_SIZE = 0.75
local BALL_SIZE = 1.5
local PLANK_SIZE = Vector3.new(9, 0.45, 1.8)
local RING_SIZE = Vector3.new(2.9, 0.4, 2.9)
local PICKUP_RANGE = 6
local PLACE_RANGE = 7
local HOLD_OFFSET = Vector3.new(0, 3.5, 0)
local THROW_SPAWN_OFFSET_Y = 2.5
local THROW_FORWARD_OFFSET = 1.75
local THROW_SPEED = 24
local THROW_UPWARD_SPEED = 6
local THROW_CARRY_VELOCITY_FACTOR = 0.35
local THROW_COLLISION_GRACE_PERIOD = 0.18
local MIN_THROW_SPEED = 0
local MAX_THROW_SPEED = 10
local PLAYER_SPAWN_POSITION = Vector3.new(0, 5, 0)
local OUT_OF_BOUNDS_Y = -200
local WORLD_BOUNDARY_HALF_EXTENT = 90
local WORLD_BOUNDARY_HEIGHT = 80
local WORLD_BOUNDARY_THICKNESS = 6

local heldObjectsByPlayer = {}
local placingObjects = {} -- { object, startPos, targetPos, elapsed, duration }
local thrownObjects = {} -- { object, elapsed, duration }
local trackedCarryables = {}

local ROCK_COLOR = Color3.fromRGB(52, 49, 44)
local ORB_COLOR = Color3.fromRGB(0, 220, 200)
local ORB_BASE_SIZE = Vector3.new(0.9, 0.9, 0.9)

local blockColors = {
    Sand = Color3.fromRGB(214, 186, 128),
    Slate = Color3.fromRGB(148, 136, 124),
    Wood = Color3.fromRGB(156, 110, 72),
    Gold = Color3.fromRGB(232, 192, 74),
    Coral = Color3.fromRGB(255, 99, 72),
}

local blockPhysics = PhysicalProperties.new(
    0.7, -- density
    0.9, -- friction
    0.03, -- elasticity
    1.0, -- friction weight
    1.0 -- elasticity weight
)

local ballPhysics = PhysicalProperties.new(
    3.5,
    1.2,
    0.02,
    1.0,
    1.0
)

local plankPhysics = PhysicalProperties.new(
    0.5,
    0.8,
    0.05,
    1.0,
    1.0
)

local ringPhysics = PhysicalProperties.new(
    0.45,
    0.35,
    0.18,
    1.0,
    1.0
)

--------------------------------------------------------------------------------
-- GROUND
--------------------------------------------------------------------------------

local ground = Instance.new("Part")
ground.Name = "Ground"
ground.Size = Vector3.new(180, 10, 180)
ground.Position = Vector3.new(0, -5, 0)
ground.Anchored = true
ground.Color = Color3.fromRGB(85, 239, 196)
ground:SetAttribute("RenderRole", "Ground")
ground.Parent = Workspace

local invisibleWalls = {
    {
        name = "BoundaryNorth",
        size = Vector3.new(WORLD_BOUNDARY_HALF_EXTENT * 2, WORLD_BOUNDARY_HEIGHT, WORLD_BOUNDARY_THICKNESS),
        pos = Vector3.new(0, WORLD_BOUNDARY_HEIGHT / 2, WORLD_BOUNDARY_HALF_EXTENT + WORLD_BOUNDARY_THICKNESS / 2),
    },
    {
        name = "BoundarySouth",
        size = Vector3.new(WORLD_BOUNDARY_HALF_EXTENT * 2, WORLD_BOUNDARY_HEIGHT, WORLD_BOUNDARY_THICKNESS),
        pos = Vector3.new(0, WORLD_BOUNDARY_HEIGHT / 2, -WORLD_BOUNDARY_HALF_EXTENT - WORLD_BOUNDARY_THICKNESS / 2),
    },
    {
        name = "BoundaryEast",
        size = Vector3.new(WORLD_BOUNDARY_THICKNESS, WORLD_BOUNDARY_HEIGHT, WORLD_BOUNDARY_HALF_EXTENT * 2),
        pos = Vector3.new(WORLD_BOUNDARY_HALF_EXTENT + WORLD_BOUNDARY_THICKNESS / 2, WORLD_BOUNDARY_HEIGHT / 2, 0),
    },
    {
        name = "BoundaryWest",
        size = Vector3.new(WORLD_BOUNDARY_THICKNESS, WORLD_BOUNDARY_HEIGHT, WORLD_BOUNDARY_HALF_EXTENT * 2),
        pos = Vector3.new(-WORLD_BOUNDARY_HALF_EXTENT - WORLD_BOUNDARY_THICKNESS / 2, WORLD_BOUNDARY_HEIGHT / 2, 0),
    },
}

for _, wallDef in ipairs(invisibleWalls) do
    local wall = Instance.new("Part")
    wall.Name = wallDef.name
    wall.Size = wallDef.size
    wall.Position = wallDef.pos
    wall.Anchored = true
    wall.Transparency = 1
    wall.CanCollide = true
    wall.CanQuery = false
    wall.CanTouch = false
    wall.Parent = Workspace
end

local mesas = {
    -- Spread out so the larger world has distinct regions while the center stays traversable.
    { name = "LowStep",       size = {10, 2, 12},    pos = {18, 1, 28} },
    { name = "LargeMesa",     size = {24, 4, 20},    pos = {34, 2, 30} },
    { name = "SmallPlateau",  size = {12, 2, 10},    pos = {-50, 1, 38} },
    { name = "SteppingStone", size = {12, 2.5, 12},  pos = {-35, 1.25, -27} },
    { name = "Ridge",         size = {8, 3, 40},     pos = {60, 1.5, -18} },
    { name = "TallMesa",      size = {14, 6, 12},    pos = {-42, 3, -34} },
    { name = "HighShelf",     size = {14, 7, 12},    pos = {46, 3.5, 46} },
    { name = "Peak",          size = {8, 11, 8},     pos = {54, 5.5, 56} },
    { name = "TowerBase",     size = {10, 7, 10},    pos = {-46, 3.5, -42} },
    { name = "Summit",        size = {8, 10, 8},     pos = {-46, 5, -50} },
    { name = "Spire",         size = {5, 20, 5},     pos = {-50, 10, -56} },
    { name = "NorthStone",    size = {14, 3, 12},    pos = {-24, 1.5, 64} },
    { name = "NorthStone2",   size = {10, 4, 10},    pos = {-40, 2, 76} },
}

for _, m in ipairs(mesas) do
    local mesa = Instance.new("Part")
    mesa.Name = m.name
    mesa.Size = Vector3.new(m.size[1], m.size[2], m.size[3])
    mesa.Position = Vector3.new(m.pos[1], m.pos[2], m.pos[3])
    mesa.Anchored = true
    mesa.Color = Color3.fromRGB(200, 160, 100)
    mesa:SetAttribute("RenderRole", "Mesa")
    mesa.Parent = Workspace
end

local function isWithinFootprint(def, x, z, padding)
    local extra = padding or 0
    local px = def.pos[1]
    local pz = def.pos[3]
    local halfWidth = def.size[1] / 2 + extra
    local halfDepth = def.size[3] / 2 + extra
    return x >= px - halfWidth and x <= px + halfWidth and z >= pz - halfDepth and z <= pz + halfDepth
end

local function isInsideMesaFootprint(x, z, padding)
    for _, mesa in ipairs(mesas) do
        if isWithinFootprint(mesa, x, z, padding) then
            return true
        end
    end
    return false
end

--------------------------------------------------------------------------------
-- ROCKS
--------------------------------------------------------------------------------

local rockSeed = 9999
local rockFootprints = {}
local function rockRandom()
    rockSeed = (rockSeed * 16807) % 2147483647
    return (rockSeed - 1) / 2147483646
end

local rockClusters = {
    { cx = 44, cz = -56, n = 3 },
    { cx = -54, cz = 16, n = 3 },
    { cx = 12, cz = 74, n = 2 },
    { cx = 72, cz = 28, n = 2 },
    { cx = -8, cz = -72, n = 3 },
}

local rockIndex = 0
for _, cluster in ipairs(rockClusters) do
    local clusterCreated = 0
    local attempts = 0
    while clusterCreated < cluster.n and attempts < 32 do
        attempts = attempts + 1
        local ox = (rockRandom() - 0.5) * 8
        local oz = (rockRandom() - 0.5) * 8
        local x = cluster.cx + ox
        local z = cluster.cz + oz
        if not isInsideMesaFootprint(x, z, 1.5) then
            clusterCreated = clusterCreated + 1
            rockIndex = rockIndex + 1

            local radius = 1.2 + rockRandom() * 1.2
            local sx = radius * 2 * (1.0 + rockRandom() * 0.5)
            local sy = radius * 2 * (0.5 + rockRandom() * 0.5)
            local sz = radius * 2 * (1.0 + rockRandom() * 0.5)

            local rock = Instance.new("Part")
            rock.Name = "Rock_" .. rockIndex
            rock.Size = Vector3.new(sx, sy, sz)
            rock.Position = Vector3.new(x, sy / 2, z)
            rock.Anchored = false
            rock.Color = ROCK_COLOR
            rock.CustomPhysicalProperties = PhysicalProperties.new(
                8.0,
                0.9,
                0.0,
                1.0,
                1.0
            )
            rock:SetAttribute("RenderRole", "Rock")
            rock.Parent = Workspace
            table.insert(rockFootprints, {
                x = x,
                z = z,
                halfWidth = sx / 2,
                halfDepth = sz / 2,
            })
        end
    end
end

local function isInsideRockFootprint(x, z, padding)
    local extra = padding or 0
    for _, rock in ipairs(rockFootprints) do
        if x >= rock.x - rock.halfWidth - extra
            and x <= rock.x + rock.halfWidth + extra
            and z >= rock.z - rock.halfDepth - extra
            and z <= rock.z + rock.halfDepth + extra then
            return true
        end
    end
    return false
end

--------------------------------------------------------------------------------
-- ORBS
--------------------------------------------------------------------------------

local orbSeed = 4242
local function orbRandom()
    orbSeed = (orbSeed * 48271) % 2147483647
    return (orbSeed - 1) / 2147483646
end

local function createOrb(name, position, scale)
    local orb = Instance.new("Part")
    orb.Name = name
    orb.Shape = Enum.PartType.Ball
    orb.Size = ORB_BASE_SIZE * scale
    orb.Position = position
    orb.Anchored = true
    orb.CanCollide = false
    orb.Color = ORB_COLOR
    orb.Material = Enum.Material.Neon
    orb:SetAttribute("RenderRole", "GlowBulb")
    orb:SetAttribute("IsCarryable", true)
    orb:SetAttribute("CarryKind", "Orb")
    orb:SetAttribute("CarryCollisionEnabled", true)
    orb.Parent = Workspace
    trackedCarryables[orb] = true
end

local orbClusters = {
    { cx = 22, cz = 18, n = 3 },
    { cx = 56, cz = 42, n = 3 },
    { cx = -36, cz = -26, n = 2 },
    { cx = 54, cz = -42, n = 2 },
    { cx = -20, cz = 68, n = 3 },
}

local orbIndex = 0
for _, cluster in ipairs(orbClusters) do
    local clusterCreated = 0
    local attempts = 0
    while clusterCreated < cluster.n and attempts < 24 do
        attempts = attempts + 1
        local ox = (orbRandom() - 0.5) * 6
        local oz = (orbRandom() - 0.5) * 6
        local x = cluster.cx + ox
        local z = cluster.cz + oz
        if not isInsideMesaFootprint(x, z, 0.5) and not isInsideRockFootprint(x, z, 1.0) then
            clusterCreated = clusterCreated + 1
            orbIndex = orbIndex + 1
            createOrb(
                "Orb_" .. orbIndex,
                Vector3.new(x, 0.55, z),
                0.85 + orbRandom() * 0.35
            )
        end
    end
end

--------------------------------------------------------------------------------
-- BLOCKS
--------------------------------------------------------------------------------

local function setCarryableMetadata(object, colorName, renderRole, carryKind)
    object:SetAttribute("RenderRole", renderRole or "PickupObject")
    object:SetAttribute("ItemColorName", colorName)
    object:SetAttribute("IsPickupObject", true)
    object:SetAttribute("IsCarryable", true)
    object:SetAttribute("CarryKind", carryKind or renderRole or "Carryable")
    object:SetAttribute("CarryCollisionEnabled", true)
end

local function createBlock(name, position, colorName)
    local block = Instance.new("Part")
    block.Name = name
    block.Size = Vector3.new(BLOCK_SIZE, BLOCK_SIZE, BLOCK_SIZE)
    block.Position = position
    block.Anchored = false
    block.Material = Enum.Material.SmoothPlastic
    block.Color = blockColors[colorName]
    block.CustomPhysicalProperties = blockPhysics
    setCarryableMetadata(block, colorName, "PickupObject", "Block")
    block.Parent = Workspace
    trackedCarryables[block] = true
    return block
end

local function createBall(name, position)
    local ball = Instance.new("Part")
    ball.Name = name
    ball.Shape = Enum.PartType.Ball
    ball.Size = Vector3.new(BALL_SIZE, BALL_SIZE, BALL_SIZE)
    ball.Position = position
    ball.Anchored = false
    ball.Material = Enum.Material.SmoothPlastic
    ball.Color = blockColors.Coral
    ball.CustomPhysicalProperties = ballPhysics
    setCarryableMetadata(ball, "Coral", "Ball", "Ball")
    ball.Parent = Workspace
    trackedCarryables[ball] = true
    return ball
end

local function createPlank(name, position, colorName, rotationDegrees)
    local plank = Instance.new("Part")
    plank.Name = name
    plank.Size = PLANK_SIZE
    local pitch = 0
    local yaw = 24
    local roll = 0
    if rotationDegrees then
        pitch = rotationDegrees[1] or pitch
        yaw = rotationDegrees[2] or yaw
        roll = rotationDegrees[3] or roll
    end
    plank.CFrame = CFrame.new(position) * CFrame.Angles(math.rad(pitch), math.rad(yaw), math.rad(roll))
    plank.Anchored = false
    plank.Material = Enum.Material.Wood
    plank.Color = blockColors[colorName]
    plank.CustomPhysicalProperties = plankPhysics
    setCarryableMetadata(plank, colorName, "Plank", "Plank")
    plank.Parent = Workspace
    trackedCarryables[plank] = true
    return plank
end

local function createRing(name, position, colorName, rotationDegrees, size)
    local ring = Instance.new("Part")
    ring.Name = name
    ring.Size = size or RING_SIZE
    ring.Shape = Enum.PartType.Ring
    local pitch = 0
    local yaw = 0
    local roll = 0
    if rotationDegrees then
        pitch = rotationDegrees[1] or pitch
        yaw = rotationDegrees[2] or yaw
        roll = rotationDegrees[3] or roll
    end
    ring.CFrame = CFrame.new(position) * CFrame.Angles(math.rad(pitch), math.rad(yaw), math.rad(roll))
    ring.Anchored = false
    ring.Material = Enum.Material.Metal
    ring.Color = blockColors[colorName]
    ring.CustomPhysicalProperties = ringPhysics
    setCarryableMetadata(ring, colorName, "Ring", "Ring")
    ring.Parent = Workspace
    trackedCarryables[ring] = true
    return ring
end

-- Place a lot of pickup cubes in small stacks across the larger map.
local blockClusters = {
    { cx = 34, cz = -66, n = 28, spread = 4.4 },
    { cx = -64, cz = 26, n = 28, spread = 4.4 },
    { cx = 22, cz = 82, n = 28, spread = 4.4 },
    { cx = 82, cz = 38, n = 28, spread = 4.4 },
    { cx = -18, cz = -82, n = 28, spread = 4.4 },
}

local blockSeed = 20240408
local function blockRandom()
    blockSeed = (blockSeed * 48271) % 2147483647
    return (blockSeed - 1) / 2147483646
end

local blockIndex = 0
for _, cluster in ipairs(blockClusters) do
    local lobeCenters = {}
    for _ = 1, 3 do
        local lobeAngle = blockRandom() * math.pi * 2
        local lobeRadius = math.sqrt(blockRandom()) * cluster.spread * 0.45
        table.insert(lobeCenters, {
            x = math.cos(lobeAngle) * lobeRadius,
            z = math.sin(lobeAngle) * lobeRadius,
        })
    end

    for j = 1, cluster.n do
        blockIndex = blockIndex + 1
        local layer = math.floor((j - 1) / 7)
        local ox, oz
        local placed = false
        for _ = 1, 24 do
            local lobe = lobeCenters[math.floor(blockRandom() * #lobeCenters) + 1]
            local angle = blockRandom() * math.pi * 2
            local radius = math.sqrt(blockRandom()) * cluster.spread * 0.7
            local stretch = 0.7 + blockRandom() * 0.6
            ox = lobe.x + math.cos(angle) * radius * stretch
            oz = lobe.z + math.sin(angle) * radius * (1.7 - stretch)
            local x = cluster.cx + ox
            local z = cluster.cz + oz
            if not isInsideMesaFootprint(x, z, 0.5) and not isInsideRockFootprint(x, z, 2.2) then
                placed = true
                break
            end
        end
        if not placed then
            ox = 0
            oz = 0
        end
        local oy = BLOCK_SIZE * 0.5 + layer * (BLOCK_SIZE * 1.05) + blockRandom() * 0.08
        local colorName = (j % 2 == 0) and "Slate" or "Sand"
        createBlock(
            "Block_" .. blockIndex,
            Vector3.new(cluster.cx + ox, oy, cluster.cz + oz),
            colorName
        )
    end
end

createBall("Ball_1", Vector3.new(69.8, 0.75, -73.2))
createPlank("Plank_1", Vector3.new(70.8, 0.9, -79.4), "Wood", { 0, 20, 0 })
createPlank("Plank_2", Vector3.new(74.2, 1.16, -77.5), "Wood", { 5, -28, 12 })
createPlank("Plank_3", Vector3.new(67.2, 1.02, -75.1), "Wood", { -6, 44, -10 })

createRing("Ring_1", Vector3.new(72.4, 0.22, -81.4), "Gold", nil, Vector3.new(2.7, 0.44, 2.7))
createRing("Ring_2", Vector3.new(75.1, 0.24, -83.3), "Gold", { 0, 18, 0 }, Vector3.new(3.2, 0.48, 3.2))
createRing("Ring_3", Vector3.new(78.2, 0.2, -79.9), "Gold", { 0, -26, 0 }, Vector3.new(2.2, 0.4, 2.2))
createRing("Ring_4", Vector3.new(80.6, 0.28, -82.1), "Gold", { 0, 34, 0 }, Vector3.new(3.6, 0.56, 3.6))
createRing("Ring_5", Vector3.new(83.4, 0.22, -84.7), "Gold", { 0, -12, 0 }, Vector3.new(2.8, 0.44, 2.8))
createRing("Ring_6", Vector3.new(85.8, 0.17, -80.8), "Gold", { 0, 26, 0 }, Vector3.new(1.95, 0.34, 1.95))
createRing("Ring_7", Vector3.new(77.0, 0.24, -86.0), "Gold", { 0, -40, 0 }, Vector3.new(3.05, 0.48, 3.05))

--------------------------------------------------------------------------------
-- PLAYER
--------------------------------------------------------------------------------

local function getHumanoidRootPart(player)
    local character = player.Character
    if character then
        return character:FindFirstChild("HumanoidRootPart")
    end
    return nil
end

local function setPlayerCarryState(player, object)
    -- Intentionally keep pickup/held state implicit in observations.
    -- Agents can infer state from world changes and LastActionResult.
end

local function setupPlayer(player)
    local humanoid = nil
    local character = player.Character
    if character then
        humanoid = character:FindFirstChild("Humanoid")
    end

    if humanoid then
        humanoid.WalkSpeed = 16
        humanoid.JumpPower = 50
        -- humanoid.JumpPower = 5
    end

    if character then
        character:SetAttribute("ModelUrl", "asset://player.glb")

        local hrp = character:FindFirstChild("HumanoidRootPart")
        if hrp then
            hrp:SetAttribute("ModelYawOffsetDeg", 180)
            hrp.Position = PLAYER_SPAWN_POSITION
        end
    end

    player:SetAttribute("LastActionResult", "Ready")
end

local function cleanupPlayer(player)
    local heldObject = heldObjectsByPlayer[player]
    if heldObject then
        heldObject:Destroy()
        heldObjectsByPlayer[player] = nil
    end
end

local function resetPlayerToSpawn(player)
    local character = player.Character
    if not character then
        return
    end

    local hrp = character:FindFirstChild("HumanoidRootPart")
    local humanoid = character:FindFirstChild("Humanoid")
    if hrp then
        hrp.Position = PLAYER_SPAWN_POSITION
        hrp.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
        hrp.AssemblyAngularVelocity = Vector3.new(0, 0, 0)
    end
    if humanoid then
        humanoid:CancelMoveTo()
    end
end

local function releaseCarryReferences(object)
    for player, heldObject in pairs(heldObjectsByPlayer) do
        if heldObject == object then
            heldObjectsByPlayer[player] = nil
            setPlayerCarryState(player, nil)
            player:SetAttribute("LastActionResult", "ResetAfterFall")
        end
    end

    local i = 1
    while i <= #placingObjects do
        if placingObjects[i].object == object then
            table.remove(placingObjects, i)
        else
            i = i + 1
        end
    end

    local thrownIndex = 1
    while thrownIndex <= #thrownObjects do
        if thrownObjects[thrownIndex].object == object then
            table.remove(thrownObjects, thrownIndex)
        else
            thrownIndex = thrownIndex + 1
        end
    end
end

local function destroyCarryable(object)
    if not object.Parent then
        return
    end
    releaseCarryReferences(object)
    trackedCarryables[object] = nil
    object:Destroy()
end

--------------------------------------------------------------------------------
-- INPUT HANDLING
--------------------------------------------------------------------------------

local function getHumanoid(player)
    local character = player.Character
    if character then
        return character:FindFirstChild("Humanoid")
    end
    return nil
end

local function isCarryableObject(inst)
    return inst:IsA("Part") and (inst:GetAttribute("IsCarryable") or inst:GetAttribute("IsPickupObject"))
end

local function findNearestCarryableObject(targetPosition, maxDistance)
    local closestObject = nil
    local closestDistance = maxDistance

    for _, inst in ipairs(Workspace:GetDescendants()) do
        if isCarryableObject(inst) then
            local distance = (inst.Position - targetPosition).Magnitude
            if distance <= closestDistance then
                closestObject = inst
                closestDistance = distance
            end
        end
    end

    return closestObject
end

local function normalizeOrNil(vector)
    local magnitude = vector.Magnitude
    if magnitude <= 0.001 then
        return nil
    end
    return vector / magnitude
end

local function canReach(player, worldPosition, maxDistance)
    local hrp = getHumanoidRootPart(player)
    if not hrp then
        return false
    end
    return (hrp.Position - worldPosition).Magnitude <= maxDistance
end

local function releaseHeldObject(player)
    local object = heldObjectsByPlayer[player]
    if not object then
        return nil
    end

    heldObjectsByPlayer[player] = nil
    setPlayerCarryState(player, nil)
    return object
end

local function pickupObject(player, inputData)
    if heldObjectsByPlayer[player] then
        player:SetAttribute("LastActionResult", "PickupFailed:AlreadyHolding")
        return
    end

    if not inputData or not inputData.position then
        player:SetAttribute("LastActionResult", "PickupFailed:MissingPosition")
        return
    end

    local pos = inputData.position
    local targetPosition = Vector3.new(pos[1], pos[2], pos[3])
    if not canReach(player, targetPosition, PICKUP_RANGE) then
        player:SetAttribute("LastActionResult", "PickupFailed:OutOfRange")
        return
    end

    local object = findNearestCarryableObject(targetPosition, 1.5)
    if not object then
        player:SetAttribute("LastActionResult", "PickupFailed:NoObject")
        return
    end

    heldObjectsByPlayer[player] = object
    object.Anchored = false
    object.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
    object.AssemblyAngularVelocity = Vector3.new(0, 0, 0)
    object.CanCollide = false
    setPlayerCarryState(player, object)
    player:SetAttribute("LastActionResult", "PickupSuccess")
end

local function positionOccupied(player, targetPosition, heldObject)
    local params = OverlapParams.new()
    params.FilterType = Enum.RaycastFilterType.Exclude
    params.FilterDescendantsInstances = { player.Character, heldObject }

    local probe = Instance.new("Part")
    probe.Size = heldObject and heldObject.Size or Vector3.new(BLOCK_SIZE, BLOCK_SIZE, BLOCK_SIZE)
    probe.CFrame = CFrame.new(targetPosition)
    if heldObject then
        probe.Shape = heldObject.Shape
    end

    local hits = Workspace:GetPartsInPart(probe, params)
    probe:Destroy()

    for _, hit in ipairs(hits) do
        if hit:IsA("Part") then
            return true
        end
    end

    return false
end

local function placeObject(player, inputData)
    local object = heldObjectsByPlayer[player]
    if not object then
        player:SetAttribute("LastActionResult", "PlaceFailed:EmptyHands")
        return
    end

    if not inputData or not inputData.position then
        player:SetAttribute("LastActionResult", "PlaceFailed:MissingPosition")
        return
    end

    local pos = inputData.position
    local targetPosition = Vector3.new(pos[1], pos[2], pos[3])
    if not canReach(player, targetPosition, PLACE_RANGE) then
        player:SetAttribute("LastActionResult", "PlaceFailed:OutOfRange")
        return
    end

    if positionOccupied(player, targetPosition, object) then
        player:SetAttribute("LastActionResult", "PlaceFailed:Occupied")
        return
    end

    local startPos = object.Position
    object.Anchored = true
    object.CanCollide = false
    releaseHeldObject(player)

    table.insert(placingObjects, {
        object = object,
        startPos = startPos,
        targetPos = targetPosition,
        elapsed = 0,
        duration = 0.3,
    })

    player:SetAttribute("LastActionResult", "PlaceSuccess")
end

local function clamp(value, minValue, maxValue)
    return math.max(minValue, math.min(maxValue, value))
end

local function getThrowDirection(inputData)
    if not inputData or not inputData.direction then
        return nil
    end

    local dir = inputData.direction
    return normalizeOrNil(Vector3.new(dir[1], dir[2], dir[3]))
end

local function getThrowSpeed(inputData)
    if not inputData or inputData.speed == nil then
        return clamp(THROW_SPEED, MIN_THROW_SPEED, MAX_THROW_SPEED)
    end

    return clamp(tonumber(inputData.speed) or THROW_SPEED, MIN_THROW_SPEED, MAX_THROW_SPEED)
end

local function throwObject(player, inputData)
    local hrp = getHumanoidRootPart(player)
    local object = heldObjectsByPlayer[player]
    if not hrp or not object then
        player:SetAttribute("LastActionResult", "ThrowFailed:EmptyHands")
        return
    end

    if not inputData or not inputData.direction then
        player:SetAttribute("LastActionResult", "ThrowFailed:MissingDirection")
        return
    end

    local throwDirection = getThrowDirection(inputData)
    if not throwDirection then
        player:SetAttribute("LastActionResult", "ThrowFailed:InvalidDirection")
        return
    end

    local throwSpeed = getThrowSpeed(inputData)

    local objectToThrow = releaseHeldObject(player)
    if not objectToThrow then
        player:SetAttribute("LastActionResult", "ThrowFailed:EmptyHands")
        return
    end

    objectToThrow.Anchored = false
    objectToThrow.CanCollide = false
    objectToThrow.Position = hrp.Position
        + Vector3.new(0, THROW_SPAWN_OFFSET_Y, 0)
        + throwDirection * THROW_FORWARD_OFFSET
    objectToThrow.AssemblyLinearVelocity = throwDirection * throwSpeed
        + Vector3.new(0, THROW_UPWARD_SPEED, 0)
        + hrp.AssemblyLinearVelocity * THROW_CARRY_VELOCITY_FACTOR
    objectToThrow.AssemblyAngularVelocity = Vector3.new(0, 8, 0)
    objectToThrow:SetNetworkOwner(nil)

    table.insert(thrownObjects, {
        object = objectToThrow,
        elapsed = 0,
        duration = THROW_COLLISION_GRACE_PERIOD,
    })

    player:SetAttribute("LastActionResult", "ThrowSuccess")
end

if AgentInputService then
    AgentInputService.InputReceived:Connect(function(player, inputType, inputData)
        local humanoid = getHumanoid(player)
        if not humanoid then return end

        if inputType == "MoveTo" and inputData and inputData.position then
            local pos = inputData.position
            humanoid:MoveTo(Vector3.new(pos[1], pos[2], pos[3]))
        elseif inputType == "Stop" then
            humanoid:CancelMoveTo()
        elseif inputType == "Jump" then
            humanoid.Jump = true
        elseif inputType == "Pickup" then
            pickupObject(player, inputData)
        elseif inputType == "Place" then
            placeObject(player, inputData)
        elseif inputType == "Throw" then
            throwObject(player, inputData)
        end
    end)
end

--------------------------------------------------------------------------------
-- MAIN LOOP
--------------------------------------------------------------------------------

RunService.Heartbeat:Connect(function(dt)
    for _, player in ipairs(Players:GetPlayers()) do
        local hrp = getHumanoidRootPart(player)
        if hrp and hrp.Position.Y < OUT_OF_BOUNDS_Y then
            resetPlayerToSpawn(player)
        end
    end

    -- Update held carryables
    for player, heldObject in pairs(heldObjectsByPlayer) do
        local hrp = getHumanoidRootPart(player)
        if hrp and heldObject then
            heldObject.Position = hrp.Position + HOLD_OFFSET
            heldObject.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
            heldObject.AssemblyAngularVelocity = Vector3.new(0, 0, 0)
        end
    end

    -- Animate placing carryables
    local i = 1
    while i <= #placingObjects do
        local p = placingObjects[i]
        p.elapsed = p.elapsed + dt
        local alpha = math.min(p.elapsed / p.duration, 1)
        -- Smooth ease-out
        local t = 1 - (1 - alpha) * (1 - alpha)
        p.object.Position = p.startPos:Lerp(p.targetPos, t)

        if alpha >= 1 then
            p.object.Anchored = false
            p.object.CanCollide = p.object:GetAttribute("CarryCollisionEnabled") == true
            table.remove(placingObjects, i)
        else
            i = i + 1
        end
    end

    local thrownIndex = 1
    while thrownIndex <= #thrownObjects do
        local thrown = thrownObjects[thrownIndex]
        if not thrown.object.Parent then
            table.remove(thrownObjects, thrownIndex)
        else
            thrown.elapsed = thrown.elapsed + dt
            if thrown.elapsed >= thrown.duration then
                thrown.object.CanCollide = thrown.object:GetAttribute("CarryCollisionEnabled") == true
                table.remove(thrownObjects, thrownIndex)
            else
                thrownIndex = thrownIndex + 1
            end
        end
    end

    for object, _ in pairs(trackedCarryables) do
        if not object.Parent then
            trackedCarryables[object] = nil
        elseif object.Position.Y < OUT_OF_BOUNDS_Y then
            destroyCarryable(object)
        end
    end
end)

--------------------------------------------------------------------------------
-- INITIALIZATION
--------------------------------------------------------------------------------

Players.PlayerAdded:Connect(setupPlayer)
Players.PlayerRemoving:Connect(cleanupPlayer)

for _, player in ipairs(Players:GetPlayers()) do
    setupPlayer(player)
end

print("=== Mesa World 2 ===")
