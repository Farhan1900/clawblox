-- Mesa World - A simple mesa world with a player and ground

local RunService = game:GetService("RunService")
local Players = game:GetService("Players")
local AgentInputService = game:GetService("AgentInputService")

-- Workspace.Gravity = 196.2
Workspace.Gravity = 10

local BLOCK_SIZE = 0.75
local ENABLE_LARGE_RING = true
local PICKUP_RANGE = 6
local PLACE_RANGE = 7
local HOLD_OFFSET = Vector3.new(0, 3.5, 0)
local THROW_SPAWN_OFFSET_Y = 2.5
local THROW_FORWARD_OFFSET = 1.75
local THROW_SPEED = 16
local THROW_UPWARD_SPEED = 6
local THROW_CARRY_VELOCITY_FACTOR = 0.35
local THROW_COLLISION_GRACE_PERIOD = 0.18
local MIN_THROW_SPEED = 0
local MAX_THROW_SPEED = 10
local PLAYER_SPAWN_POSITION = Vector3.new(0, 5, 0)
local OUT_OF_BOUNDS_Y = -200
local WORLD_BOUNDARY_HALF_EXTENT = 50
local WORLD_BOUNDARY_HEIGHT = 80
local WORLD_BOUNDARY_THICKNESS = 6

local heldBlocksByPlayer = {}
local placingBlocks = {} -- { block, startCFrame, targetCFrame, elapsed, duration }
local thrownBlocks = {} -- { block, elapsed, duration }
local trackedBlocks = {}

local function findWorkspaceChild(name)
    return Workspace:FindFirstChild(name)
end

local blockColors = {
    Sand = Color3.fromRGB(214, 186, 128),
    Slate = Color3.fromRGB(148, 136, 124),
    Gold = Color3.fromRGB(232, 192, 74),
}

local blockPhysics = PhysicalProperties.new(
    0.7, -- density
    0.9, -- friction
    0.03, -- elasticity
    1.0, -- friction weight
    1.0 -- elasticity weight
)

local terrainPhysics = PhysicalProperties.new(
    2.0, -- density
    1.4, -- friction
    0.0, -- elasticity
    1.0, -- friction weight
    1.0 -- elasticity weight
)

--------------------------------------------------------------------------------
-- GROUND
--------------------------------------------------------------------------------

local ground = findWorkspaceChild("Ground") or Instance.new("Part")
ground.Name = "Ground"
ground.Size = Vector3.new(100, 10, 100)
ground.Position = Vector3.new(0, -5, 0)
ground.Anchored = true
ground.Color = Color3.fromRGB(85, 239, 196)
ground.CustomPhysicalProperties = terrainPhysics
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
    local wall = findWorkspaceChild(wallDef.name) or Instance.new("Part")
    wall.Name = wallDef.name
    wall.Size = wallDef.size
    wall.Position = wallDef.pos
    wall.Anchored = true
    wall.Transparency = 1
    wall.CanCollide = true
    wall.CanQuery = false
    wall.CanTouch = false
    wall.CustomPhysicalProperties = terrainPhysics
    wall.Parent = Workspace
end

local mesas = {
    -- Ground-level formations
    { name = "LowStep",       size = {8, 2, 10},     pos = {8, 1, 18} },
    { name = "LargeMesa",     size = {20, 4, 16},    pos = {20, 2, 20} },
    { name = "SmallPlateau",  size = {10, 2, 8},     pos = {-30, 1, 28} },
    { name = "SteppingStone", size = {8, 2.5, 8},    pos = {-18, 1.25, -12} },
    { name = "Ridge",         size = {6, 3, 25},     pos = {35, 1.5, -10} },
    { name = "TallMesa",      size = {12, 6, 10},    pos = {-25, 3, -20} },
    -- NE climbing path: large mesa (4) -> high shelf (7) -> peak (11)
    { name = "HighShelf",     size = {10, 7, 8},     pos = {25, 3.5, 28} },
    { name = "Peak",          size = {5, 11, 5},     pos = {28, 5.5, 32} },
    -- SW climbing path: tall mesa (6) -> tower base (7) -> summit (10) -> spire (20)
    { name = "TowerBase",     size = {8, 7, 8},      pos = {-28, 3.5, -26} },
    { name = "Summit",        size = {6, 10, 6},     pos = {-28, 5, -30} },
    { name = "Spire",         size = {4, 20, 4},     pos = {-30, 10, -33} },
}

for _, m in ipairs(mesas) do
    local mesa = findWorkspaceChild(m.name) or Instance.new("Part")
    mesa.Name = m.name
    mesa.Size = Vector3.new(m.size[1], m.size[2], m.size[3])
    mesa.Position = Vector3.new(m.pos[1], m.pos[2], m.pos[3])
    mesa.Anchored = true
    mesa.Color = Color3.fromRGB(200, 160, 100)
    mesa.CustomPhysicalProperties = terrainPhysics
    mesa:SetAttribute("RenderRole", "Mesa")
    mesa.Parent = Workspace
end

--------------------------------------------------------------------------------
-- BLOCKS
--------------------------------------------------------------------------------

local function setPickupMetadata(block, colorName, renderRole)
    block:SetAttribute("RenderRole", renderRole or "PickupObject")
    block:SetAttribute("ItemColorName", colorName)
    block:SetAttribute("IsPickupObject", true)
end

local function createBlock(name, position, colorName)
    local existing = findWorkspaceChild(name)
    if existing then
        setPickupMetadata(existing, colorName, "PickupObject")
        trackedBlocks[existing] = true
        return existing
    end
    local block = Instance.new("Part")
    block.Name = name
    block.Size = Vector3.new(BLOCK_SIZE, BLOCK_SIZE, BLOCK_SIZE)
    block.Position = position
    block.Anchored = false
    block.Material = Enum.Material.Slate
    block.Color = blockColors[colorName]
    block.CustomPhysicalProperties = blockPhysics
    setPickupMetadata(block, colorName, "PickupObject")
    block.Parent = Workspace
    trackedBlocks[block] = true
    return block
end

local function createRing(name, position, colorName, rotationDegrees, size)
    local ring = Instance.new("Part")
    ring.Name = name
    ring.Size = size
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
    ring.CustomPhysicalProperties = PhysicalProperties.new(
        0.45,
        0.35,
        0.18,
        1.0,
        1.0
    )
    setPickupMetadata(ring, colorName, "Ring")
    ring.Parent = Workspace
    trackedBlocks[ring] = true
    return ring
end

local baseBlockPile = {
    { pos = Vector3.new(14.5, 6.5, -1.9), color = "Slate" },
    { pos = Vector3.new(15.2, 6.9, -1.3), color = "Slate" },
    { pos = Vector3.new(15.8, 7.3, -1.8), color = "Slate" },
    { pos = Vector3.new(14.8, 7.8, -0.9), color = "Slate" },
    { pos = Vector3.new(15.5, 8.2, -1.2), color = "Slate" },
    { pos = Vector3.new(15.0, 8.7, -1.6), color = "Slate" },
    { pos = Vector3.new(14.7, 9.1, -1.1), color = "Slate" },
    { pos = Vector3.new(15.6, 9.6, -1.5), color = "Slate" },
    { pos = Vector3.new(15.1, 10.0, -0.8), color = "Slate" },
    { pos = Vector3.new(15.3, 10.5, -1.3), color = "Slate" },
    { pos = Vector3.new(14.9, 11.0, -1.7), color = "Slate" },
    { pos = Vector3.new(15.4, 11.4, -1.0), color = "Slate" },
}

for index, blockDef in ipairs(baseBlockPile) do
    createBlock("Block_" .. index, blockDef.pos, blockDef.color)
end

if ENABLE_LARGE_RING then
    createRing("Ring_1", Vector3.new(45.4, 0.28, -45.7), "Gold", { 0, 32, 0 }, Vector3.new(3.6, 0.56, 3.6))
end

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

local function setPlayerBlockState(player, block)
    -- Intentionally keep pickup/held state implicit in observations.
    -- Agents can infer state from world changes and LastActionResult.
end

local function setupPlayer(player, shouldSpawnAtStart)
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
            if shouldSpawnAtStart ~= false then
                hrp.Position = PLAYER_SPAWN_POSITION
            end
        end
    end

    player:SetAttribute("LastActionResult", "Ready")
end

local function cleanupPlayer(player)
    local heldBlock = heldBlocksByPlayer[player]
    if heldBlock then
        heldBlock:Destroy()
        heldBlocksByPlayer[player] = nil
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

local function releaseBlockReferences(block)
    for player, heldBlock in pairs(heldBlocksByPlayer) do
        if heldBlock == block then
            heldBlocksByPlayer[player] = nil
            setPlayerBlockState(player, nil)
            player:SetAttribute("LastActionResult", "ResetAfterFall")
        end
    end

    local i = 1
    while i <= #placingBlocks do
        if placingBlocks[i].block == block then
            table.remove(placingBlocks, i)
        else
            i = i + 1
        end
    end

    local thrownIndex = 1
    while thrownIndex <= #thrownBlocks do
        if thrownBlocks[thrownIndex].block == block then
            table.remove(thrownBlocks, thrownIndex)
        else
            thrownIndex = thrownIndex + 1
        end
    end
end

local function destroyBlock(block)
    if not block.Parent then
        return
    end
    releaseBlockReferences(block)
    trackedBlocks[block] = nil
    block:Destroy()
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

local function findNearestBlock(targetPosition, maxDistance)
    local closestBlock = nil
    local closestDistance = maxDistance

    for _, inst in ipairs(Workspace:GetDescendants()) do
        if inst:IsA("Part") and inst:GetAttribute("IsPickupObject") then
            local distance = (inst.Position - targetPosition).Magnitude
            if distance <= closestDistance then
                closestBlock = inst
                closestDistance = distance
            end
        end
    end

    return closestBlock
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

local function releaseHeldBlock(player)
    local block = heldBlocksByPlayer[player]
    if not block then
        return nil
    end

    heldBlocksByPlayer[player] = nil
    setPlayerBlockState(player, nil)
    return block
end

local function parsePosition(pos)
    if type(pos) ~= "table" then return nil end
    local x = tonumber(pos[1])
    local y = tonumber(pos[2])
    local z = tonumber(pos[3])
    if not x or not y or not z then return nil end
    if x ~= x or y ~= y or z ~= z then return nil end
    return Vector3.new(x, y, z)
end

local function pickupBlock(player, inputData)
    if heldBlocksByPlayer[player] then
        player:SetAttribute("LastActionResult", "PickupFailed:AlreadyHolding")
        return
    end

    if not inputData or not inputData.position then
        player:SetAttribute("LastActionResult", "PickupFailed:MissingPosition")
        return
    end

    local targetPosition = parsePosition(inputData.position)
    if not targetPosition then
        player:SetAttribute("LastActionResult", "PickupFailed:InvalidPosition")
        return
    end
    if not canReach(player, targetPosition, PICKUP_RANGE) then
        player:SetAttribute("LastActionResult", "PickupFailed:OutOfRange")
        return
    end

    local block = findNearestBlock(targetPosition, 1.5)
    if not block then
        player:SetAttribute("LastActionResult", "PickupFailed:NoBlock")
        return
    end

    heldBlocksByPlayer[player] = block
    block.Anchored = false
    block.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
    block.AssemblyAngularVelocity = Vector3.new(0, 0, 0)
    block.CanCollide = false
    setPlayerBlockState(player, block)
    player:SetAttribute("LastActionResult", "PickupSuccess")
end

local function parseRotationDegrees(rotation)
    if rotation == nil then
        return nil
    end
    if type(rotation) == "number" then
        if rotation ~= rotation then
            return nil
        end
        return 0, tonumber(rotation), 0
    end
    if type(rotation) ~= "table" then
        return nil
    end

    local pitch = tonumber(rotation.pitch or rotation.x or rotation[1] or 0)
    local yaw = tonumber(rotation.yaw or rotation.y or rotation[2] or 0)
    local roll = tonumber(rotation.roll or rotation.z or rotation[3] or 0)
    if not pitch or not yaw or not roll then
        return nil
    end
    if pitch ~= pitch or yaw ~= yaw or roll ~= roll then
        return nil
    end
    return pitch, yaw, roll
end

local function getCurrentRotationCFrame(object)
    local pitch, yaw, roll = object.CFrame:ToOrientation()
    return CFrame.fromOrientation(pitch, yaw, roll)
end

local function getTargetPlacementCFrame(object, targetPosition, rotation)
    local rotationCFrame = getCurrentRotationCFrame(object)
    if rotation ~= nil then
        local pitch, yaw, roll = parseRotationDegrees(rotation)
        if pitch == nil then
            return nil
        end
        rotationCFrame = CFrame.fromOrientation(math.rad(pitch), math.rad(yaw), math.rad(roll))
    end
    return CFrame.new(targetPosition) * rotationCFrame
end

local function positionOccupied(player, targetCFrame, object)
    local params = OverlapParams.new()
    params.FilterType = Enum.RaycastFilterType.Exclude
    params.FilterDescendantsInstances = { player.Character, heldBlocksByPlayer[player] }

    local probe = Instance.new("Part")
    probe.Size = object.Size
    probe.CFrame = targetCFrame
    probe.Shape = object.Shape

    local hits = Workspace:GetPartsInPart(probe, params)
    probe:Destroy()

    for _, hit in ipairs(hits) do
        if hit:IsA("Part") then
            return true
        end
    end

    return false
end

local function placeBlock(player, inputData)
    local block = heldBlocksByPlayer[player]
    if not block then
        player:SetAttribute("LastActionResult", "PlaceFailed:EmptyHands")
        return
    end

    if not inputData or not inputData.position then
        player:SetAttribute("LastActionResult", "PlaceFailed:MissingPosition")
        return
    end

    local targetPosition = parsePosition(inputData.position)
    if not targetPosition then
        player:SetAttribute("LastActionResult", "PlaceFailed:InvalidPosition")
        return
    end
    if not canReach(player, targetPosition, PLACE_RANGE) then
        player:SetAttribute("LastActionResult", "PlaceFailed:OutOfRange")
        return
    end

    local targetCFrame = getTargetPlacementCFrame(block, targetPosition, inputData.rotation)
    if not targetCFrame then
        player:SetAttribute("LastActionResult", "PlaceFailed:InvalidRotation")
        return
    end

    if positionOccupied(player, targetCFrame, block) then
        player:SetAttribute("LastActionResult", "PlaceFailed:Occupied")
        return
    end

    local startCFrame = block.CFrame
    block.Anchored = true
    block.CanCollide = false
    releaseHeldBlock(player)

    table.insert(placingBlocks, {
        block = block,
        startCFrame = startCFrame,
        targetCFrame = targetCFrame,
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

    local direction = parsePosition(inputData.direction)
    if not direction then
        return nil
    end
    return normalizeOrNil(direction)
end

local function getThrowSpeed(inputData)
    if not inputData or inputData.speed == nil then
        return clamp(THROW_SPEED, MIN_THROW_SPEED, MAX_THROW_SPEED)
    end

    return clamp(tonumber(inputData.speed) or THROW_SPEED, MIN_THROW_SPEED, MAX_THROW_SPEED)
end

local function throwBlock(player, inputData)
    local hrp = getHumanoidRootPart(player)
    local block = heldBlocksByPlayer[player]
    if not hrp or not block then
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

    local blockToThrow = releaseHeldBlock(player)
    if not blockToThrow then
        player:SetAttribute("LastActionResult", "ThrowFailed:EmptyHands")
        return
    end

    blockToThrow.Anchored = false
    blockToThrow.CanCollide = false
    blockToThrow.Position = hrp.Position
        + Vector3.new(0, THROW_SPAWN_OFFSET_Y, 0)
        + throwDirection * THROW_FORWARD_OFFSET
    blockToThrow.AssemblyLinearVelocity = throwDirection * throwSpeed
        + Vector3.new(0, THROW_UPWARD_SPEED, 0)
        + hrp.AssemblyLinearVelocity * THROW_CARRY_VELOCITY_FACTOR
    blockToThrow.AssemblyAngularVelocity = Vector3.new(0, 8, 0)
    blockToThrow:SetNetworkOwner(nil)

    table.insert(thrownBlocks, {
        block = blockToThrow,
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
            local targetPosition = parsePosition(inputData.position)
            if not targetPosition then
                player:SetAttribute("LastActionResult", "MoveToFailed:InvalidPosition")
                return
            end
            humanoid:MoveTo(targetPosition)
        elseif inputType == "Stop" then
            humanoid:CancelMoveTo()
        elseif inputType == "Jump" then
            humanoid.Jump = true
        elseif inputType == "Pickup" then
            pickupBlock(player, inputData)
        elseif inputType == "Place" then
            placeBlock(player, inputData)
        elseif inputType == "Throw" then
            throwBlock(player, inputData)
        else
            player:SetAttribute("LastActionResult", "InvalidAction")
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

    -- Update held blocks
    for player, heldBlock in pairs(heldBlocksByPlayer) do
        local hrp = getHumanoidRootPart(player)
        if hrp and heldBlock then
            heldBlock.Position = hrp.Position + HOLD_OFFSET
            heldBlock.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
            heldBlock.AssemblyAngularVelocity = Vector3.new(0, 0, 0)
        end
    end

    -- Animate placing blocks
    local i = 1
    while i <= #placingBlocks do
        local p = placingBlocks[i]
        p.elapsed = p.elapsed + dt
        local alpha = math.min(p.elapsed / p.duration, 1)
        -- Smooth ease-out
        local t = 1 - (1 - alpha) * (1 - alpha)
        p.block.CFrame = p.startCFrame:Lerp(p.targetCFrame, t)

        if alpha >= 1 then
            p.block.Anchored = false
            p.block.CanCollide = true
            setPickupMetadata(
                p.block,
                p.block:GetAttribute("ItemColorName"),
                p.block:GetAttribute("RenderRole")
            )
            table.remove(placingBlocks, i)
        else
            i = i + 1
        end
    end

    local thrownIndex = 1
    while thrownIndex <= #thrownBlocks do
        local thrown = thrownBlocks[thrownIndex]
        if not thrown.block.Parent then
            table.remove(thrownBlocks, thrownIndex)
        else
            thrown.elapsed = thrown.elapsed + dt
            if thrown.elapsed >= thrown.duration then
                thrown.block.CanCollide = true
                table.remove(thrownBlocks, thrownIndex)
            else
                thrownIndex = thrownIndex + 1
            end
        end
    end

    for block, _ in pairs(trackedBlocks) do
        if not block.Parent then
            trackedBlocks[block] = nil
        elseif block.Position.Y < OUT_OF_BOUNDS_Y then
            destroyBlock(block)
        end
    end
end)

--------------------------------------------------------------------------------
-- INITIALIZATION
--------------------------------------------------------------------------------

Players.PlayerAdded:Connect(function(player)
    setupPlayer(player, true)
end)
Players.PlayerRemoving:Connect(cleanupPlayer)

for _, player in ipairs(Players:GetPlayers()) do
    setupPlayer(player, false)
end

print("=== Mesa World ===")
