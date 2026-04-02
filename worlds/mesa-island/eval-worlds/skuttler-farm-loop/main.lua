-- Skuttler Farm Loop - Small flat world for testing bulb transport and clump farming.

local RunService = game:GetService("RunService")
local Players = game:GetService("Players")
local AgentInputService = game:GetService("AgentInputService")
local ServerStorage = game:GetService("ServerStorage")

Workspace.Gravity = 10

local PICKUP_RANGE = 6
local PLACE_RANGE = 7
local CONSUME_RANGE = 3
local GROUND_SIZE = Vector3.new(52, 10, 52)
local PLAYER_SPAWN_POSITION = Vector3.new(0, 5, 0)
local OUT_OF_BOUNDS_Y = -40

local GLOWBULB_COLOR = Color3.fromRGB(0, 220, 200)
local GLOWBULB_BASE_SIZE = Vector3.new(0.7, 0.7, 0.7)
local GLOWBULB_MAX_BITE_STEPS = 4
local GLOWBULB_MIN_SCALE = 0.35
local GLOWBULB_REGROW_SECONDS = 45
local GLOWBULB_HOLD_OFFSET = Vector3.new(0, 3.1, 0)
local GLOWBULB_PLACE_DURATION = 0.18

local SKUTTLER_COLOR = Color3.fromRGB(180, 100, 60)
local SKUTTLER_RESIDUE_COLOR = Color3.fromRGB(196, 142, 78)
local SKUTTLER_SIZE = Vector3.new(0.6, 0.55, 0.8)
local SKUTTLER_SPEED = 3.0
local SKUTTLER_FLEE_RANGE = 5
local SKUTTLER_FLEE_SPEED = 6.5
local SKUTTLER_ATTRACT_RANGE = 16
local SKUTTLER_BITE_DISTANCE = 1.35
local SKUTTLER_BITE_COOLDOWN = 1.25
local SKUTTLER_FEED_WINDUP = 1.0
local SKUTTLER_FORAGE_COOLDOWN_AFTER_BITE = 10
local SKUTTLER_SECRETION_PER_BITE = 1
local SKUTTLER_SECRETION_FOR_CLUMP = 2
local SKUTTLER_CLUMP_COOLDOWN = 10
local SKUTTLER_HOME_SOFT_RADIUS = 10
local SKUTTLER_HOME_HARD_RADIUS = 14
local SKUTTLER_WANDER_INTERVAL = 2.5
local SKUTTLER_OBSTACLE_PADDING = 0.35
local SKUTTLER_RECOVERY_SECONDS = 1.1
local SKUTTLER_RECOVERY_ANGLE = math.rad(65)

local CLUMP_COLOR = Color3.fromRGB(196, 142, 78)
local CLUMP_SIZE = Vector3.new(0.38, 0.38, 0.38)
local CLUMP_HOLD_OFFSET = Vector3.new(0, 3.1, 0)

local trackedCarryables = {}
local activeMoveStates = {}
local moveFeedbackConnections = {}
local resetPlayerToSpawn

local function getOrCreateChild(parent, className, name)
    local child = parent:FindFirstChild(name)
    if child then
        return child
    end
    child = Instance.new(className)
    child.Name = name
    child.Parent = parent
    return child
end

local serverStateRoot = getOrCreateChild(ServerStorage, "Folder", "SkuttlerFarmLoopState")
local serverGlobals = getOrCreateChild(serverStateRoot, "Folder", "Globals")
local serverPlayerState = getOrCreateChild(serverStateRoot, "Folder", "Players")
local serverFloraState = getOrCreateChild(serverStateRoot, "Folder", "Flora")
local serverCreatureState = getOrCreateChild(serverStateRoot, "Folder", "Creatures")

local function destroyChildren(folder)
    for _, child in ipairs(folder:GetChildren()) do
        child:Destroy()
    end
end

local function nextServerCounter(attrName)
    local value = (serverGlobals:GetAttribute(attrName) or 0) + 1
    serverGlobals:SetAttribute(attrName, value)
    return value
end

local function playerStateKey(player)
    return tostring(math.floor(player.UserId))
end

local function getOrCreatePlayerState(player)
    local folder = getOrCreateChild(serverPlayerState, "Folder", playerStateKey(player))
    local heldObject = folder:FindFirstChild("HeldObject")
    if not heldObject then
        heldObject = Instance.new("ObjectValue")
        heldObject.Name = "HeldObject"
        heldObject.Parent = folder
    end
    return folder, heldObject
end

local function getPlayerState(player)
    return serverPlayerState:FindFirstChild(playerStateKey(player))
end

local function getHeldObjectForPlayer(player)
    local folder = getPlayerState(player)
    local heldObject = folder and folder:FindFirstChild("HeldObject")
    return heldObject and heldObject.Value or nil
end

local function isObjectHeld(object)
    for _, player in ipairs(Players:GetPlayers()) do
        if getHeldObjectForPlayer(player) == object then
            return true
        end
    end
    return false
end

local function setHeldObjectForPlayer(player, object)
    local _, heldObject = getOrCreatePlayerState(player)
    heldObject.Value = object
end

local function resetWorldState()
    for _, inst in ipairs(Workspace:GetChildren()) do
        if inst:IsA("Part") then
            local role = inst:GetAttribute("RenderRole")
            if role == "Ground" or role == "GlowBulb" or role == "Skuttler" or role == "Clump" then
                inst:Destroy()
            end
        end
    end

    destroyChildren(serverPlayerState)
    destroyChildren(serverFloraState)
    destroyChildren(serverCreatureState)
    serverGlobals:SetAttribute("NextCreatureId", 0)
    serverGlobals:SetAttribute("NextClumpId", 0)

    local ground = Instance.new("Part")
    ground.Name = "Ground"
    ground.Size = GROUND_SIZE
    ground.Position = Vector3.new(0, -GROUND_SIZE.Y * 0.5, 0)
    ground.Anchored = true
    ground.Color = Color3.fromRGB(85, 239, 196)
    ground:SetAttribute("RenderRole", "Ground")
    ground.Parent = Workspace
end

resetWorldState()

local function getHumanoidRootPart(player)
    local character = player.Character
    if character then
        return character:FindFirstChild("HumanoidRootPart")
    end
    return nil
end

local function parsePosition(pos)
    if type(pos) ~= "table" then
        return nil
    end
    local x = tonumber(pos[1])
    local y = tonumber(pos[2])
    local z = tonumber(pos[3])
    if not x or not y or not z or x ~= x or y ~= y or z ~= z then
        return nil
    end
    return Vector3.new(x, y, z)
end

local function canReach(player, worldPosition, maxDistance)
    local hrp = getHumanoidRootPart(player)
    return hrp and (hrp.Position - worldPosition).Magnitude <= maxDistance or false
end

local function isOverLand(x, z)
    return math.abs(x) <= (GROUND_SIZE.X * 0.5 - 1) and math.abs(z) <= (GROUND_SIZE.Z * 0.5 - 1)
end

local function randomWanderDir()
    local angle = math.random() * math.pi * 2
    return Vector3.new(math.cos(angle), 0, math.sin(angle))
end

local function rotateFlatDir(dir, angle)
    local unit = dir.Magnitude > 0.01 and dir.Unit or randomWanderDir()
    local cosA = math.cos(angle)
    local sinA = math.sin(angle)
    return Vector3.new(
        unit.X * cosA - unit.Z * sinA,
        0,
        unit.X * sinA + unit.Z * cosA
    )
end

local function setGlowBulbMetadata(bulb)
    bulb:SetAttribute("RenderRole", "GlowBulb")
    bulb:SetAttribute("IsEdible", true)
    bulb:SetAttribute("IsCarryable", true)
    bulb:SetAttribute("CarryKind", "Flora")
    bulb:SetAttribute("PlacementDuration", GLOWBULB_PLACE_DURATION)
    bulb:SetAttribute("HoldOffset", GLOWBULB_HOLD_OFFSET)
end

local function clampGlowBulbBiteStep(step)
    return math.clamp(math.floor(step or 0), 0, GLOWBULB_MAX_BITE_STEPS)
end

local function getGlowBulbScaleForStep(step)
    local clampedStep = clampGlowBulbBiteStep(step)
    local alpha = (clampedStep - 1) / (GLOWBULB_MAX_BITE_STEPS - 1)
    return GLOWBULB_MIN_SCALE + (1 - GLOWBULB_MIN_SCALE) * alpha
end

local function applyGlowBulbVisuals(floraState, bulb)
    if floraState:GetAttribute("Harvested") then
        bulb.Size = GLOWBULB_BASE_SIZE * GLOWBULB_MIN_SCALE
        bulb.Transparency = 1
        bulb.CanQuery = false
        return
    end

    local biteStep = clampGlowBulbBiteStep(floraState:GetAttribute("BiteStep"))
    local scale = getGlowBulbScaleForStep(biteStep)
    bulb.Size = GLOWBULB_BASE_SIZE * scale
    bulb.Transparency = 0
    bulb.CanQuery = true
end

local function getFloraStateForPart(part)
    if not part or not part:IsA("Part") then
        return nil
    end
    return serverFloraState:FindFirstChild(part.Name)
end

local function takeGlowBulbBite(floraState, bulb)
    if not floraState or not bulb or not bulb.Parent or floraState:GetAttribute("Harvested") then
        return false
    end
    local biteStep = clampGlowBulbBiteStep(floraState:GetAttribute("BiteStep"))
    if biteStep <= 0 then
        return false
    end

    local nextStep = math.max(0, biteStep - 1)
    floraState:SetAttribute("BiteStep", nextStep)
    if nextStep <= 0 then
        floraState:SetAttribute("Harvested", true)
        floraState:SetAttribute("RegrowTimer", GLOWBULB_REGROW_SECONDS)
    end
    applyGlowBulbVisuals(floraState, bulb)
    return true
end

local function createGlowBulb(name, position)
    local bulb = Instance.new("Part")
    bulb.Name = name
    bulb.Shape = Enum.PartType.Ball
    bulb.Size = GLOWBULB_BASE_SIZE
    bulb.Position = position
    bulb.Anchored = true
    bulb.CanCollide = false
    bulb.Color = GLOWBULB_COLOR
    bulb.Material = Enum.Material.Neon
    setGlowBulbMetadata(bulb)
    bulb.Parent = Workspace

    local floraState = getOrCreateChild(serverFloraState, "Folder", name)
    local partRef = getOrCreateChild(floraState, "ObjectValue", "Part")
    partRef.Value = bulb
    floraState:SetAttribute("Species", "GlowBulb")
    floraState:SetAttribute("BiteStep", GLOWBULB_MAX_BITE_STEPS)
    floraState:SetAttribute("Harvested", false)
    floraState:SetAttribute("RegrowTimer", 0)
    applyGlowBulbVisuals(floraState, bulb)
    trackedCarryables[bulb] = true
end

local function setClumpMetadata(clump)
    clump:SetAttribute("RenderRole", "Clump")
    clump:SetAttribute("IsCarryable", true)
    clump:SetAttribute("CarryKind", "Clump")
    clump:SetAttribute("PlacementDuration", 0.12)
    clump:SetAttribute("HoldOffset", CLUMP_HOLD_OFFSET)
end

local function createSecretionClump(position)
    local clump = Instance.new("Part")
    clump.Name = "Clump_" .. tostring(nextServerCounter("NextClumpId"))
    clump.Shape = Enum.PartType.Block
    clump.Size = CLUMP_SIZE
    clump.Position = position
    clump.Anchored = true
    clump.CanCollide = false
    clump.Color = CLUMP_COLOR
    clump.Material = Enum.Material.SmoothPlastic
    clump.Transparency = 0.08
    setClumpMetadata(clump)
    clump.Parent = Workspace
    trackedCarryables[clump] = true
    return clump
end

local function refreshSkuttlerAppearance(part, creatureState)
    local secretionLevel = creatureState:GetAttribute("SecretionLevel") or 0
    local tintAlpha = math.clamp(secretionLevel / (SKUTTLER_SECRETION_FOR_CLUMP * 2), 0, 0.45)
    part.Color = SKUTTLER_COLOR:Lerp(SKUTTLER_RESIDUE_COLOR, tintAlpha)
    part.Material = tintAlpha > 0 and Enum.Material.Slate or Enum.Material.SmoothPlastic
end

local function createSkuttler(position)
    local part = Instance.new("Part")
    part.Name = "Skuttler"
    part.Size = SKUTTLER_SIZE
    part.Position = Vector3.new(position.X, SKUTTLER_SIZE.Y * 0.5, position.Z)
    part.Anchored = true
    part.CanCollide = true
    part.Material = Enum.Material.SmoothPlastic
    part:SetAttribute("RenderRole", "Skuttler")
    part:SetAttribute("Species", "Skuttler")
    part.Parent = Workspace

    local creatureState = getOrCreateChild(serverCreatureState, "Folder", "Creature_" .. tostring(nextServerCounter("NextCreatureId")))
    local partRef = getOrCreateChild(creatureState, "ObjectValue", "Part")
    partRef.Value = part
    creatureState:SetAttribute("HomePos", position)
    creatureState:SetAttribute("WanderDir", randomWanderDir())
    creatureState:SetAttribute("WanderTimer", math.random() * SKUTTLER_WANDER_INTERVAL)
    creatureState:SetAttribute("RecoveryDir", Vector3.new(0, 0, 0))
    creatureState:SetAttribute("RecoveryTimer", 0)
    creatureState:SetAttribute("FeedTargetName", "")
    creatureState:SetAttribute("FeedTimer", 0)
    creatureState:SetAttribute("FeedCooldown", 0)
    creatureState:SetAttribute("ForageCooldown", 0)
    creatureState:SetAttribute("ClumpCooldown", 0)
    creatureState:SetAttribute("SecretionLevel", 0)
    refreshSkuttlerAppearance(part, creatureState)
    return part
end

local bulbPositions = {
    Vector3.new(-6, 0.5, -4),
    Vector3.new(-4, 0.5, -7),
    Vector3.new(-1, 0.5, -5),
    Vector3.new(6, 0.5, 4),
    Vector3.new(4, 0.5, 7),
    Vector3.new(1, 0.5, 5),
}
for index, pos in ipairs(bulbPositions) do
    createGlowBulb("GlowBulb_" .. tostring(index), pos)
end

createSkuttler(Vector3.new(-10, 0, 0))
createSkuttler(Vector3.new(10, 0, 0))
createSkuttler(Vector3.new(0, 0, 10))

local function getHumanoid(player)
    local character = player.Character
    if character then
        return character:FindFirstChild("Humanoid")
    end
    return nil
end

local function bindMoveFeedback(player, humanoid)
    local existing = moveFeedbackConnections[player]
    if existing then
        existing:Disconnect()
    end
    moveFeedbackConnections[player] = humanoid.MoveToFinished:Connect(function(reached)
        if not activeMoveStates[player] then
            return
        end
        activeMoveStates[player] = nil
        player:SetAttribute("LastActionResult", reached and "MoveToFinished:Reached" or "MoveToFinished:Blocked")
    end)
end

local function setupPlayer(player)
    local playerState = getOrCreatePlayerState(player)
    if playerState:GetAttribute("Initialized") then
        return
    end
    playerState:SetAttribute("Initialized", true)

    local humanoid = getHumanoid(player)
    if humanoid then
        humanoid.WalkSpeed = 16
        humanoid.JumpPower = 50
        bindMoveFeedback(player, humanoid)
    end
    resetPlayerToSpawn(player)
end

resetPlayerToSpawn = function(player)
    local hrp = getHumanoidRootPart(player)
    local humanoid = getHumanoid(player)
    if hrp then
        hrp.Position = PLAYER_SPAWN_POSITION
        hrp.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
        hrp.AssemblyAngularVelocity = Vector3.new(0, 0, 0)
    end
    if humanoid then
        activeMoveStates[player] = nil
        humanoid:CancelMoveTo()
    end
end

local function getObjectHoldOffset(object)
    local offset = object and object:GetAttribute("HoldOffset")
    if typeof(offset) == "Vector3" then
        return offset
    end
    return Vector3.new(0, 3.0, 0)
end

local function findNearestCarryableObject(targetPosition, maxDistance)
    local closestObject = nil
    local closestDistance = maxDistance
    for object, _ in pairs(trackedCarryables) do
        if object.Parent then
            local distance = (object.Position - targetPosition).Magnitude
            if distance <= closestDistance then
                closestObject = object
                closestDistance = distance
            end
        end
    end
    return closestObject
end

local function pickupObject(player, inputData)
    if getHeldObjectForPlayer(player) then
        player:SetAttribute("LastActionResult", "PickupFailed:AlreadyHolding")
        return
    end
    local targetPosition = inputData and parsePosition(inputData.position)
    if not targetPosition then
        player:SetAttribute("LastActionResult", "PickupFailed:InvalidPosition")
        return
    end
    if not canReach(player, targetPosition, PICKUP_RANGE) then
        player:SetAttribute("LastActionResult", "PickupFailed:OutOfRange")
        return
    end

    local object = findNearestCarryableObject(targetPosition, 1.5)
    if not object then
        player:SetAttribute("LastActionResult", "PickupFailed:NoObject")
        return
    end

    object.Anchored = true
    object.CanCollide = false
    object.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
    object.AssemblyAngularVelocity = Vector3.new(0, 0, 0)
    setHeldObjectForPlayer(player, object)
    player:SetAttribute("LastActionResult", "PickupSuccess")
end

local function placeHeldObject(player, inputData)
    local object = getHeldObjectForPlayer(player)
    if not object then
        player:SetAttribute("LastActionResult", "PlaceFailed:EmptyHands")
        return
    end
    local targetPosition = inputData and parsePosition(inputData.position)
    if not targetPosition then
        player:SetAttribute("LastActionResult", "PlaceFailed:InvalidPosition")
        return
    end
    if not canReach(player, targetPosition, PLACE_RANGE) then
        player:SetAttribute("LastActionResult", "PlaceFailed:OutOfRange")
        return
    end
    if not isOverLand(targetPosition.X, targetPosition.Z) then
        player:SetAttribute("LastActionResult", "PlaceFailed:InvalidGround")
        return
    end

    local targetCenter = Vector3.new(targetPosition.X, object.Size.Y * 0.5, targetPosition.Z)
    for other, _ in pairs(trackedCarryables) do
        if other ~= object and other.Parent and not isObjectHeld(other) then
            local clearance = ((math.max(other.Size.X, other.Size.Z) + math.max(object.Size.X, object.Size.Z)) * 0.5) + 0.1
            if (other.Position - targetCenter).Magnitude < clearance then
                player:SetAttribute("LastActionResult", "PlaceFailed:Occupied")
                return
            end
        end
    end

    object.Position = targetCenter
    object.Anchored = true
    object.CanCollide = false
    object.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
    object.AssemblyAngularVelocity = Vector3.new(0, 0, 0)
    setHeldObjectForPlayer(player, nil)
    player:SetAttribute("LastActionResult", "PlaceSuccess")
end

local function findNearestEdible(targetPosition, maxDistance)
    local closest = nil
    local closestDist = maxDistance
    for _, floraState in ipairs(serverFloraState:GetChildren()) do
        local partRef = floraState:FindFirstChild("Part")
        local part = partRef and partRef.Value or nil
        if part and part.Parent and not floraState:GetAttribute("Harvested") and not isObjectHeld(part) then
            local dist = (part.Position - targetPosition).Magnitude
            if dist <= closestDist then
                closest = part
                closestDist = dist
            end
        end
    end
    return closest
end

local function consumeFlora(player, inputData)
    local targetPosition = inputData and parsePosition(inputData.position)
    if not targetPosition then
        player:SetAttribute("LastActionResult", "ConsumeFailed:InvalidPosition")
        return
    end
    if not canReach(player, targetPosition, CONSUME_RANGE) then
        player:SetAttribute("LastActionResult", "ConsumeFailed:OutOfRange")
        return
    end
    local plant = findNearestEdible(targetPosition, 2.0)
    local floraState = plant and getFloraStateForPart(plant) or nil
    if not floraState or not takeGlowBulbBite(floraState, plant) then
        player:SetAttribute("LastActionResult", "ConsumeFailed:NothingEdible")
        return
    end
    player:SetAttribute("LastActionResult", "ConsumeSuccess:GlowBulb")
end

if AgentInputService then
    AgentInputService.InputReceived:Connect(function(player, inputType, inputData)
        local humanoid = getHumanoid(player)
        if not humanoid then
            return
        end

        if inputType == "MoveTo" and inputData and inputData.position then
            local target = parsePosition(inputData.position)
            if not target then
                player:SetAttribute("LastActionResult", "MoveToFailed:InvalidPosition")
                return
            end
            activeMoveStates[player] = { target = target }
            player:SetAttribute("LastActionResult", "MoveToIssued")
            humanoid:MoveTo(target)
        elseif inputType == "Stop" then
            activeMoveStates[player] = nil
            player:SetAttribute("LastActionResult", "StopSuccess")
            humanoid:CancelMoveTo()
        elseif inputType == "Jump" then
            player:SetAttribute("LastActionResult", "JumpIssued")
            humanoid.Jump = true
        elseif inputType == "Pickup" then
            pickupObject(player, inputData)
        elseif inputType == "Place" then
            placeHeldObject(player, inputData)
        elseif inputType == "Consume" then
            consumeFlora(player, inputData)
        end
    end)
end

local function nearestPlayerPosition(pos, maxRange)
    local closest = nil
    local closestDist = maxRange
    for _, player in ipairs(Players:GetPlayers()) do
        local hrp = getHumanoidRootPart(player)
        if hrp then
            local dist = (hrp.Position - pos).Magnitude
            if dist < closestDist then
                closest = hrp.Position
                closestDist = dist
            end
        end
    end
    return closest
end

local function nearestFloraTarget(pos, maxRange)
    local closestPart = nil
    local closestState = nil
    local closestDist = maxRange
    for _, floraState in ipairs(serverFloraState:GetChildren()) do
        local partRef = floraState:FindFirstChild("Part")
        local part = partRef and partRef.Value or nil
        if part and part.Parent and not floraState:GetAttribute("Harvested") and not isObjectHeld(part) then
            local dist = (part.Position - pos).Magnitude
            if dist < closestDist then
                closestPart = part
                closestState = floraState
                closestDist = dist
            end
        end
    end
    return closestPart, closestState, closestDist
end

local function isSkuttlerObstacle(selfPart, hit)
    if not hit:IsA("Part") or hit == selfPart then
        return false
    end
    return hit:GetAttribute("CarryKind") == "Clump"
end

local function isSkuttlerPathBlocked(part, moveDir)
    if moveDir.Magnitude <= 0.01 then
        return false
    end
    local params = OverlapParams.new()
    params.FilterType = Enum.RaycastFilterType.Exclude
    params.FilterDescendantsInstances = { part }
    local forward = moveDir.Unit
    local lookAhead = math.max(SKUTTLER_SIZE.X, SKUTTLER_SIZE.Z) * 0.75 + SKUTTLER_OBSTACLE_PADDING
    local probeCenter = part.Position + forward * lookAhead
    local probeSize = Vector3.new(
        SKUTTLER_SIZE.X + SKUTTLER_OBSTACLE_PADDING,
        math.max(SKUTTLER_SIZE.Y, 1.0),
        SKUTTLER_SIZE.Z + SKUTTLER_OBSTACLE_PADDING
    )
    local hits = Workspace:GetPartBoundsInBox(CFrame.new(probeCenter), probeSize, params)
    for _, hit in ipairs(hits) do
        if isSkuttlerObstacle(part, hit) then
            return true
        end
    end
    return false
end

local function maybeDropClump(creatureState, part, inhibited)
    if inhibited then
        return
    end
    local secretionLevel = creatureState:GetAttribute("SecretionLevel") or 0
    local clumpCooldown = creatureState:GetAttribute("ClumpCooldown") or 0
    if secretionLevel < SKUTTLER_SECRETION_FOR_CLUMP or clumpCooldown > 0 then
        return
    end

    local dropPosition = Vector3.new(part.Position.X, CLUMP_SIZE.Y * 0.5, part.Position.Z)
    for object, _ in pairs(trackedCarryables) do
        if object ~= part and object.Parent and (object.Position - dropPosition).Magnitude <= 0.7 then
            return
        end
    end

    createSecretionClump(dropPosition)
    creatureState:SetAttribute("SecretionLevel", secretionLevel - SKUTTLER_SECRETION_FOR_CLUMP)
    creatureState:SetAttribute("ClumpCooldown", SKUTTLER_CLUMP_COOLDOWN)
    refreshSkuttlerAppearance(part, creatureState)
end

local function updateSkuttlers(dt)
    for _, creatureState in ipairs(serverCreatureState:GetChildren()) do
        local partRef = creatureState:FindFirstChild("Part")
        local part = partRef and partRef.Value or nil
        if not part or not part.Parent then
            creatureState:Destroy()
        else
            local pos = part.Position
            local flatPos = Vector3.new(pos.X, 0, pos.Z)
            local moveDir = Vector3.new(0, 0, 0)
            local moveSpeed = SKUTTLER_SPEED
            local fleeing = false
            local isFeeding = false
            local foodPart = nil
            local foodState = nil
            local foodDist = SKUTTLER_ATTRACT_RANGE

            local feedCooldown = math.max(0, (creatureState:GetAttribute("FeedCooldown") or 0) - dt)
            local forageCooldown = math.max(0, (creatureState:GetAttribute("ForageCooldown") or 0) - dt)
            local clumpCooldown = math.max(0, (creatureState:GetAttribute("ClumpCooldown") or 0) - dt)
            local feedTimer = math.max(0, (creatureState:GetAttribute("FeedTimer") or 0) - dt)
            local recoveryTimer = math.max(0, (creatureState:GetAttribute("RecoveryTimer") or 0) - dt)
            local feedTargetName = creatureState:GetAttribute("FeedTargetName") or ""
            creatureState:SetAttribute("FeedCooldown", feedCooldown)
            creatureState:SetAttribute("ForageCooldown", forageCooldown)
            creatureState:SetAttribute("ClumpCooldown", clumpCooldown)
            creatureState:SetAttribute("FeedTimer", feedTimer)
            creatureState:SetAttribute("RecoveryTimer", recoveryTimer)

            local playerPos = nearestPlayerPosition(pos, SKUTTLER_FLEE_RANGE)
            if playerPos then
                local away = flatPos - Vector3.new(playerPos.X, 0, playerPos.Z)
                if away.Magnitude > 0.01 then
                    moveDir = away.Unit
                end
                moveSpeed = SKUTTLER_FLEE_SPEED
                fleeing = true
            end

            if not fleeing and feedTimer > 0 then
                isFeeding = true
            end

            if not fleeing and feedTargetName ~= "" then
                foodState = serverFloraState:FindFirstChild(feedTargetName)
                local foodRef = foodState and foodState:FindFirstChild("Part")
                foodPart = foodRef and foodRef.Value or nil
                if foodPart and foodPart.Parent and not foodState:GetAttribute("Harvested") then
                    foodDist = (foodPart.Position - pos).Magnitude
                else
                    foodPart = nil
                    foodState = nil
                    feedTargetName = ""
                    creatureState:SetAttribute("FeedTargetName", "")
                end
            end

            if not fleeing and not isFeeding and forageCooldown <= 0 then
                if not foodPart then
                    foodPart, foodState, foodDist = nearestFloraTarget(pos, SKUTTLER_ATTRACT_RANGE)
                end
                if foodPart then
                    local toward = Vector3.new(foodPart.Position.X, 0, foodPart.Position.Z) - flatPos
                    if toward.Magnitude > SKUTTLER_BITE_DISTANCE + 0.25 then
                        moveDir = toward.Unit
                    else
                        local hadFeedTarget = feedTargetName ~= ""
                        feedTargetName = foodPart.Name
                        creatureState:SetAttribute("FeedTargetName", feedTargetName)
                        if not hadFeedTarget and feedCooldown <= 0 and feedTimer <= 0 then
                            creatureState:SetAttribute("FeedTimer", SKUTTLER_FEED_WINDUP)
                            feedTimer = SKUTTLER_FEED_WINDUP
                            isFeeding = true
                        end
                    end
                end
            end

            local homePos = creatureState:GetAttribute("HomePos") or Vector3.new(0, 0, 0)
            local homeOffset = flatPos - Vector3.new(homePos.X, 0, homePos.Z)
            local homeDir = homeOffset.Magnitude > 0.01 and (-homeOffset.Unit) or randomWanderDir()

            if not fleeing and not isFeeding then
                local recoveryDir = creatureState:GetAttribute("RecoveryDir") or Vector3.new(0, 0, 0)
                if recoveryTimer > 0 and recoveryDir.Magnitude > 0.01 then
                    moveDir = recoveryDir.Unit
                end

                if homeOffset.Magnitude > SKUTTLER_HOME_HARD_RADIUS then
                    moveDir = homeDir
                elseif homeOffset.Magnitude > SKUTTLER_HOME_SOFT_RADIUS and moveDir.Magnitude < 0.01 then
                    moveDir = rotateFlatDir(homeDir, math.rad((math.random() - 0.5) * 40))
                end

                if moveDir.Magnitude < 0.01 then
                    local wanderTimer = (creatureState:GetAttribute("WanderTimer") or 0) - dt
                    if wanderTimer <= 0 then
                        local wanderDir = randomWanderDir()
                        wanderTimer = SKUTTLER_WANDER_INTERVAL + math.random() * 1.5
                        creatureState:SetAttribute("WanderDir", wanderDir)
                    end
                    creatureState:SetAttribute("WanderTimer", wanderTimer)
                    moveDir = creatureState:GetAttribute("WanderDir") or Vector3.new(0, 0, 0)
                end
            end

            if isFeeding then
                moveDir = Vector3.new(0, 0, 0)
            end

            if moveDir.Magnitude > 0.01 then
                local delta = moveDir.Unit * moveSpeed * dt
                local newX = pos.X + delta.X
                local newZ = pos.Z + delta.Z
                if isOverLand(newX, newZ) and not isSkuttlerPathBlocked(part, moveDir) then
                    part.Position = Vector3.new(newX, SKUTTLER_SIZE.Y * 0.5, newZ)
                    creatureState:SetAttribute("RecoveryTimer", 0)
                elseif not fleeing and not isFeeding then
                    creatureState:SetAttribute("RecoveryDir", rotateFlatDir(homeDir, math.random() < 0.5 and SKUTTLER_RECOVERY_ANGLE or -SKUTTLER_RECOVERY_ANGLE))
                    creatureState:SetAttribute("RecoveryTimer", SKUTTLER_RECOVERY_SECONDS)
                end
            end

            if fleeing then
                creatureState:SetAttribute("FeedTimer", 0)
                creatureState:SetAttribute("FeedTargetName", "")
            elseif feedTargetName ~= "" and feedTimer <= 0 then
                foodState = serverFloraState:FindFirstChild(feedTargetName)
                local foodRef = foodState and foodState:FindFirstChild("Part")
                foodPart = foodRef and foodRef.Value or nil
                if foodPart and foodPart.Parent and not foodState:GetAttribute("Harvested")
                    and (foodPart.Position - part.Position).Magnitude <= SKUTTLER_BITE_DISTANCE + 0.35
                    and feedCooldown <= 0
                then
                    if takeGlowBulbBite(foodState, foodPart) then
                        creatureState:SetAttribute("FeedCooldown", SKUTTLER_BITE_COOLDOWN)
                        creatureState:SetAttribute("ForageCooldown", SKUTTLER_FORAGE_COOLDOWN_AFTER_BITE + math.random() * 2)
                        creatureState:SetAttribute("SecretionLevel", (creatureState:GetAttribute("SecretionLevel") or 0) + SKUTTLER_SECRETION_PER_BITE)
                        local awayFromFood = flatPos - Vector3.new(foodPart.Position.X, 0, foodPart.Position.Z)
                        if awayFromFood.Magnitude > 0.01 then
                            creatureState:SetAttribute("WanderDir", awayFromFood.Unit)
                            creatureState:SetAttribute("WanderTimer", SKUTTLER_WANDER_INTERVAL)
                        end
                        refreshSkuttlerAppearance(part, creatureState)
                    end
                end
                creatureState:SetAttribute("FeedTargetName", "")
            end

            maybeDropClump(creatureState, part, fleeing or isFeeding)
        end
    end
end

RunService.Heartbeat:Connect(function(dt)
    for _, player in ipairs(Players:GetPlayers()) do
        local hrp = getHumanoidRootPart(player)
        if hrp and hrp.Position.Y < OUT_OF_BOUNDS_Y then
            resetPlayerToSpawn(player)
        end
    end

    for _, player in ipairs(Players:GetPlayers()) do
        local heldObject = getHeldObjectForPlayer(player)
        local hrp = getHumanoidRootPart(player)
        if heldObject and hrp then
            heldObject.Position = hrp.Position + getObjectHoldOffset(heldObject)
        end
    end

    for _, floraState in ipairs(serverFloraState:GetChildren()) do
        local partRef = floraState:FindFirstChild("Part")
        local part = partRef and partRef.Value or nil
        if part and part.Parent and floraState:GetAttribute("Harvested") then
            local regrowTimer = math.max(0, (floraState:GetAttribute("RegrowTimer") or 0) - dt)
            floraState:SetAttribute("RegrowTimer", regrowTimer)
            if regrowTimer <= 0 then
                floraState:SetAttribute("BiteStep", GLOWBULB_MAX_BITE_STEPS)
                floraState:SetAttribute("Harvested", false)
                applyGlowBulbVisuals(floraState, part)
            end
        end
    end

    updateSkuttlers(dt)

    for object, _ in pairs(trackedCarryables) do
        if not object.Parent then
            trackedCarryables[object] = nil
        end
    end
end)

Players.PlayerAdded:Connect(setupPlayer)
Players.PlayerRemoving:Connect(function(player)
    local connection = moveFeedbackConnections[player]
    if connection then
        connection:Disconnect()
        moveFeedbackConnections[player] = nil
    end
end)

for _, player in ipairs(Players:GetPlayers()) do
    setupPlayer(player)
end

print("=== Skuttler Farm Loop ===")
