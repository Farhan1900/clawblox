-- Mesa World - A simple mesa world with a player and ground

local RunService = game:GetService("RunService")
local Players = game:GetService("Players")
local AgentInputService = game:GetService("AgentInputService")

-- Workspace.Gravity = 196.2
Workspace.Gravity = 10

local BLOCK_SIZE = 0.75
local PICKUP_RANGE = 6
local PLACE_RANGE = 7
local PLAYER_SPAWN_POSITION = Vector3.new(0, 5, 0)
local OUT_OF_BOUNDS_Y = -200
local ACTION_MOVE = "d982hdeu"
local ACTION_STOP = "q5mrt9ks"
local ACTION_JUMP = "v7nl2cqp"
local ACTION_PICKUP = "x1f4bwau"
local ACTION_PLACE = "z8yk6rhm"
local ENTITY_GROUND = "n7wq1mga"
local ENTITY_MESA_PREFIX = "u4c9k"
local ENTITY_BLOCK_PREFIX = "b6p2t"
local ATTR_RENDER_KEY = "m3r8a"
local ATTR_RENDER_GROUND = "g7nd2"
local ATTR_RENDER_MESA = "s4lf0"
local ATTR_RENDER_PICKUP = "p9u1x"
local ATTR_PICKUP_KEY = "q4t7n"
local ATTR_COLOR_KEY = "c6x3w"
local ATTR_COLOR_SAND = "h2z9e"
local ATTR_COLOR_SLATE = "j4l8b"

local heldBlocksByPlayer = {}
local placingBlocks = {} -- { block, startPos, targetPos, elapsed, duration }
local trackedBlocks = {}

local function findWorkspaceChild(name)
    return Workspace:FindFirstChild(name)
end

local blockColors = {
    Sand = Color3.fromRGB(214, 186, 128),
    Slate = Color3.fromRGB(148, 136, 124),
}
local blockColorCodes = {
    Sand = ATTR_COLOR_SAND,
    Slate = ATTR_COLOR_SLATE,
}

local blockPhysics = PhysicalProperties.new(
    0.7, -- density
    0.9, -- friction
    0.03, -- elasticity
    1.0, -- friction weight
    1.0 -- elasticity weight
)

--------------------------------------------------------------------------------
-- GROUND
--------------------------------------------------------------------------------

local ground = findWorkspaceChild(ENTITY_GROUND) or Instance.new("Part")
ground.Name = ENTITY_GROUND
ground.Size = Vector3.new(100, 10, 100)
ground.Position = Vector3.new(0, -5, 0)
ground.Anchored = true
ground.Color = Color3.fromRGB(85, 239, 196)
ground:SetAttribute(ATTR_RENDER_KEY, ATTR_RENDER_GROUND)
ground.Parent = Workspace

local mesas = {
    -- Ground-level formations
    { name = ENTITY_MESA_PREFIX .. "01", size = {8, 2, 10},     pos = {8, 1, 18} },
    { name = ENTITY_MESA_PREFIX .. "02", size = {20, 4, 16},    pos = {20, 2, 20} },
    { name = ENTITY_MESA_PREFIX .. "03", size = {10, 2, 8},     pos = {-30, 1, 28} },
    { name = ENTITY_MESA_PREFIX .. "04", size = {8, 2.5, 8},    pos = {-18, 1.25, -12} },
    { name = ENTITY_MESA_PREFIX .. "05", size = {6, 3, 25},     pos = {35, 1.5, -10} },
    { name = ENTITY_MESA_PREFIX .. "06", size = {12, 6, 10},    pos = {-25, 3, -20} },
    -- NE climbing path: large mesa (4) -> high shelf (7) -> peak (11)
    { name = ENTITY_MESA_PREFIX .. "07", size = {10, 7, 8},     pos = {25, 3.5, 28} },
    { name = ENTITY_MESA_PREFIX .. "08", size = {5, 11, 5},     pos = {28, 5.5, 32} },
    -- SW climbing path: tall mesa (6) -> tower base (7) -> summit (10) -> spire (20)
    { name = ENTITY_MESA_PREFIX .. "09", size = {8, 7, 8},      pos = {-28, 3.5, -26} },
    { name = ENTITY_MESA_PREFIX .. "10", size = {6, 10, 6},     pos = {-28, 5, -30} },
    { name = ENTITY_MESA_PREFIX .. "11", size = {4, 20, 4},     pos = {-30, 10, -33} },
}

for _, m in ipairs(mesas) do
    local mesa = findWorkspaceChild(m.name) or Instance.new("Part")
    mesa.Name = m.name
    mesa.Size = Vector3.new(m.size[1], m.size[2], m.size[3])
    mesa.Position = Vector3.new(m.pos[1], m.pos[2], m.pos[3])
    mesa.Anchored = true
    mesa.Color = Color3.fromRGB(200, 160, 100)
    mesa:SetAttribute(ATTR_RENDER_KEY, ATTR_RENDER_MESA)
    mesa.Parent = Workspace
end

--------------------------------------------------------------------------------
-- BLOCKS
--------------------------------------------------------------------------------

local function setBlockMetadata(block, colorCode)
    block:SetAttribute(ATTR_RENDER_KEY, ATTR_RENDER_PICKUP)
    block:SetAttribute(ATTR_COLOR_KEY, colorCode)
    block:SetAttribute(ATTR_PICKUP_KEY, true)
end

local function createBlock(name, position, colorName)
    local existing = findWorkspaceChild(name)
    if existing then
        setBlockMetadata(existing, blockColorCodes[colorName])
        trackedBlocks[existing] = true
        return existing
    end
    local block = Instance.new("Part")
    block.Name = name
    block.Size = Vector3.new(BLOCK_SIZE, BLOCK_SIZE, BLOCK_SIZE)
    block.Position = position
    block.Anchored = false
    block.Material = Enum.Material.SmoothPlastic
    block.Color = blockColors[colorName]
    block.CustomPhysicalProperties = blockPhysics
    setBlockMetadata(block, blockColorCodes[colorName])
    block.Parent = Workspace
    trackedBlocks[block] = true
    return block
end

local blockPile = {
    { pos = Vector3.new(14.5, 6.5, -1.9), color = "Sand" },
    { pos = Vector3.new(15.2, 6.9, -1.3), color = "Slate" },
    { pos = Vector3.new(15.8, 7.3, -1.8), color = "Sand" },
    { pos = Vector3.new(14.8, 7.8, -0.9), color = "Slate" },
    { pos = Vector3.new(15.5, 8.2, -1.2), color = "Slate" },
    { pos = Vector3.new(15.0, 8.7, -1.6), color = "Sand" },
    { pos = Vector3.new(14.7, 9.1, -1.1), color = "Slate" },
    { pos = Vector3.new(15.6, 9.6, -1.5), color = "Sand" },
    { pos = Vector3.new(15.1, 10.0, -0.8), color = "Slate" },
    { pos = Vector3.new(15.3, 10.5, -1.3), color = "Sand" },
    { pos = Vector3.new(14.9, 11.0, -1.7), color = "Slate" },
    { pos = Vector3.new(15.4, 11.4, -1.0), color = "Sand" },
}

for index, blockDef in ipairs(blockPile) do
    createBlock(ENTITY_BLOCK_PREFIX .. string.format("%02d", index), blockDef.pos, blockDef.color)
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
end

local function setActionResult(player, result)
    player:SetAttribute("LastActionResult", result)
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
        local hrp = character:FindFirstChild("HumanoidRootPart")
        if hrp then
            hrp:SetAttribute("ModelUrl", "asset://player.glb")
            hrp:SetAttribute("ModelYawOffsetDeg", 180)
            hrp.Position = PLAYER_SPAWN_POSITION
        end
    end

    setActionResult(player, nil)
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
        if inst:IsA("Part") and inst:GetAttribute(ATTR_PICKUP_KEY) then
            local distance = (inst.Position - targetPosition).Magnitude
            if distance <= closestDistance then
                closestBlock = inst
                closestDistance = distance
            end
        end
    end

    return closestBlock
end

local function canReach(player, worldPosition, maxDistance)
    local hrp = getHumanoidRootPart(player)
    if not hrp then
        return false
    end
    return (hrp.Position - worldPosition).Magnitude <= maxDistance
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
        setActionResult(player, "failure")
        return
    end

    if not inputData or not inputData.position then
        setActionResult(player, "invalid")
        return
    end

    local targetPosition = parsePosition(inputData.position)
    if not targetPosition then
        setActionResult(player, "invalid")
        return
    end
    if not canReach(player, targetPosition, PICKUP_RANGE) then
        setActionResult(player, "failure")
        return
    end

    local block = findNearestBlock(targetPosition, 1.5)
    if not block then
        setActionResult(player, "failure")
        return
    end

    heldBlocksByPlayer[player] = block
    block.CanCollide = false
    setPlayerBlockState(player, block)
    setActionResult(player, "success")
end

local function positionOccupied(player, targetPosition)
    local params = OverlapParams.new()
    params.FilterType = Enum.RaycastFilterType.Exclude
    params.FilterDescendantsInstances = { player.Character, heldBlocksByPlayer[player] }

    local hits = Workspace:GetPartBoundsInBox(
        CFrame.new(targetPosition),
        Vector3.new(BLOCK_SIZE, BLOCK_SIZE, BLOCK_SIZE),
        params
    )

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
        setActionResult(player, "failure")
        return
    end

    if not inputData or not inputData.position then
        setActionResult(player, "invalid")
        return
    end

    local targetPosition = parsePosition(inputData.position)
    if not targetPosition then
        setActionResult(player, "invalid")
        return
    end
    if not canReach(player, targetPosition, PLACE_RANGE) then
        setActionResult(player, "failure")
        return
    end

    if positionOccupied(player, targetPosition) then
        setActionResult(player, "failure")
        return
    end

    local startPos = block.Position
    block.Anchored = true
    block.CanCollide = false
    heldBlocksByPlayer[player] = nil
    setPlayerBlockState(player, nil)

    table.insert(placingBlocks, {
        block = block,
        startPos = startPos,
        targetPos = targetPosition,
        elapsed = 0,
        duration = 0.3,
    })

    setActionResult(player, "success")
end

if AgentInputService then
    AgentInputService.InputReceived:Connect(function(player, inputType, inputData)
        local humanoid = getHumanoid(player)
        if not humanoid then return end

        if inputType == ACTION_MOVE and inputData and inputData.position then
            local targetPosition = parsePosition(inputData.position)
            if not targetPosition then
                setActionResult(player, "invalid")
                return
            end
            humanoid:MoveTo(targetPosition)
            setActionResult(player, "success")
        elseif inputType == ACTION_MOVE then
            setActionResult(player, "invalid")
        elseif inputType == ACTION_STOP then
            humanoid:CancelMoveTo()
            setActionResult(player, "success")
        elseif inputType == ACTION_JUMP then
            humanoid.Jump = true
            setActionResult(player, "success")
        elseif inputType == ACTION_PICKUP then
            pickupBlock(player, inputData)
        elseif inputType == ACTION_PLACE then
            placeBlock(player, inputData)
        else
            setActionResult(player, "invalid")
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
            heldBlock.Position = hrp.Position + Vector3.new(0, 3.5, 0)
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
        p.block.Position = p.startPos:Lerp(p.targetPos, t)

        if alpha >= 1 then
            p.block.Anchored = false
            p.block.CanCollide = true
            setBlockMetadata(p.block, p.block:GetAttribute(ATTR_COLOR_KEY))
            table.remove(placingBlocks, i)
        else
            i = i + 1
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

Players.PlayerAdded:Connect(setupPlayer)
Players.PlayerRemoving:Connect(cleanupPlayer)

for _, player in ipairs(Players:GetPlayers()) do
    setupPlayer(player)
end

print("=== Mesa World ===")
