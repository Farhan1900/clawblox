-- River Bridge - Flat world with a river crossing puzzle.
-- Block/bond/carry mechanics are identical to Mesa Island.

local RunService = game:GetService("RunService")
local Players = game:GetService("Players")
local AgentInputService = game:GetService("AgentInputService")
local ServerStorage = game:GetService("ServerStorage")

Workspace.Gravity = 10

local BLOCK_SIZE = 0.75
local PICKUP_RANGE = 6
local PLACE_RANGE = 7
local SPAWN_HEIGHT = 5
local PLAYER_SPAWN_POSITION = Vector3.new(-16, SPAWN_HEIGHT, 0)
local OUT_OF_BOUNDS_Y = -25
local BLOCK_OUT_OF_BOUNDS_MODE = "destroy" -- "destroy" or "respawn"

local BANK_LENGTH_Z = 40
local BANK_WIDTH_X = 22
local BANK_HEIGHT = 10
local RIVER_WIDTH_X = 8
local BANK_CENTER_OFFSET_X = (RIVER_WIDTH_X * 0.5) + (BANK_WIDTH_X * 0.5)

local trackedCarryables = {}
local blockBondTargets = {}
local heldAssemblyStates = {}
local frozenBondBlocks = {}
local activeMoveStates = {}
local moveFeedbackConnections = {}
local pendingWorldReset = false
local bindMoveFeedback
local findNearestCarryableObject
local createSecretionClump
local setupPlayer
local resetPlayerToSpawn
local DEFAULT_HOLD_OFFSET = Vector3.new(0, 3.5, 0)
local DEFAULT_PLACE_DURATION = 0.3

local BLOCK_TYPE_NAME = "Stone"
local BLOCK_RENDER_ROLE = "StoneBlock"
local ROCK_COLOR = Color3.fromRGB(52, 49, 44)
local STONE_BLOCK_COLOR = Color3.fromRGB(140, 135, 125)

local blockColors = {
    [BLOCK_TYPE_NAME] = STONE_BLOCK_COLOR,
}

local blockPhysics = PhysicalProperties.new(
    0.7, -- density
    0.9, -- friction
    0.03, -- elasticity
    1.0, -- friction weight
    1.0 -- elasticity weight
)

local RESIDUE_COLOR = Color3.fromRGB(196, 142, 78)
local RESIDUE_SET_COLOR = Color3.fromRGB(120, 90, 64)
local RESIDUE_CLUMP_SIZE = Vector3.new(0.38, 0.38, 0.38)
local RESIDUE_HOLD_OFFSET = Vector3.new(0, 3.1, 0)
local RESIDUE_PLACE_RANGE = 1.2
local RESIDUE_CONTACT_PADDING = 0.14
local RESIDUE_BOND_NAME = "SkuttlerBond"

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

local serverStateRoot = getOrCreateChild(ServerStorage, "Folder", "ConstructionTestState")
local serverGlobals = getOrCreateChild(serverStateRoot, "Folder", "Globals")
local serverPlayerState = getOrCreateChild(serverStateRoot, "Folder", "Players")
local serverPlacementState = getOrCreateChild(serverStateRoot, "Folder", "Placements")
local serverBlockState = getOrCreateChild(serverStateRoot, "Folder", "Blocks")

local function destroyChildren(folder)
    for _, child in ipairs(folder:GetChildren()) do
        child:Destroy()
    end
end

local function clearTable(map)
    for key, _ in pairs(map) do
        map[key] = nil
    end
end

if serverGlobals:GetAttribute("NextPlacementId") == nil then
    serverGlobals:SetAttribute("NextPlacementId", 0)
end
if serverGlobals:GetAttribute("NextResidueId") == nil then
    serverGlobals:SetAttribute("NextResidueId", 0)
end
if serverGlobals:GetAttribute("StaticWorldInitialized") == nil then
    serverGlobals:SetAttribute("StaticWorldInitialized", false)
end
if serverGlobals:GetAttribute("BlocksInitialized") == nil then
    serverGlobals:SetAttribute("BlocksInitialized", false)
end

local function resetWorldState()
    for _, inst in ipairs(Workspace:GetChildren()) do
        if inst:IsA("Part") then
            local role = inst:GetAttribute("RenderRole")
            if role == "Ground" or role == "Ocean" or role == "GlowBulb" or role == BLOCK_RENDER_ROLE or role == "Clump" then
                inst:Destroy()
            end
        end
    end

    destroyChildren(serverPlayerState)
    destroyChildren(serverPlacementState)
    destroyChildren(serverBlockState)
    serverGlobals:SetAttribute("NextPlacementId", 0)
    serverGlobals:SetAttribute("NextResidueId", 0)
    serverGlobals:SetAttribute("StaticWorldInitialized", false)
    serverGlobals:SetAttribute("BlocksInitialized", false)
end

local function initializeStaticWorld()
    if serverGlobals:GetAttribute("StaticWorldInitialized") then
        return
    end

    local westBank = Instance.new("Part")
    westBank.Name = "WestBank"
    westBank.Size = Vector3.new(BANK_WIDTH_X, BANK_HEIGHT, BANK_LENGTH_Z)
    westBank.Position = Vector3.new(-BANK_CENTER_OFFSET_X, -BANK_HEIGHT * 0.5, 0)
    westBank.Anchored = true
    westBank.Color = Color3.fromRGB(85, 239, 196)
    westBank:SetAttribute("RenderRole", "Ground")
    westBank.Parent = Workspace

    local eastBank = Instance.new("Part")
    eastBank.Name = "EastBank"
    eastBank.Size = Vector3.new(BANK_WIDTH_X, BANK_HEIGHT, BANK_LENGTH_Z)
    eastBank.Position = Vector3.new(BANK_CENTER_OFFSET_X, -BANK_HEIGHT * 0.5, 0)
    eastBank.Anchored = true
    eastBank.Color = Color3.fromRGB(85, 239, 196)
    eastBank:SetAttribute("RenderRole", "Ground")
    eastBank.Parent = Workspace

    local river = Instance.new("Part")
    river.Name = "River"
    river.Size = Vector3.new(RIVER_WIDTH_X, 3, BANK_LENGTH_Z)
    river.Position = Vector3.new(0, -1.5, 0)
    river.Anchored = true
    river.CanCollide = false
    river.Color = Color3.fromRGB(61, 168, 212)
    river.Material = Enum.Material.SmoothPlastic
    river.Transparency = 0.2
    river:SetAttribute("RenderRole", "Ocean")
    river.Parent = Workspace

    local goalMarker = Instance.new("Part")
    goalMarker.Name = "GoalMarker"
    goalMarker.Size = Vector3.new(2.5, 1.0, 2.5)
    goalMarker.Position = Vector3.new(16, 0.5, 0)
    goalMarker.Anchored = true
    goalMarker.CanCollide = false
    goalMarker.Color = Color3.fromRGB(245, 214, 87)
    goalMarker.Material = Enum.Material.Neon
    goalMarker:SetAttribute("RenderRole", "GlowBulb")
    goalMarker.Parent = Workspace

    serverGlobals:SetAttribute("StaticWorldInitialized", true)
end

resetWorldState()
initializeStaticWorld()

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
    local heldObject = folder:FindFirstChild("HeldObject") or folder:FindFirstChild("HeldBlock")
    if not heldObject then
        heldObject = Instance.new("ObjectValue")
        heldObject.Name = "HeldObject"
        heldObject.Parent = folder
    elseif heldObject.Name ~= "HeldObject" then
        heldObject.Name = "HeldObject"
    end
    return folder, heldObject
end

local function getPlayerState(player)
    return serverPlayerState:FindFirstChild(playerStateKey(player))
end

local function getHeldObjectForPlayer(player)
    local folder = getPlayerState(player)
    if not folder then
        return nil
    end
    local heldObject = folder:FindFirstChild("HeldObject") or folder:FindFirstChild("HeldBlock")
    if not heldObject then
        return nil
    end
    return heldObject.Value
end

--------------------------------------------------------------------------------
-- HELPERS
--------------------------------------------------------------------------------

local function getHumanoidRootPart(player)
    local character = player.Character
    if character then
        return character:FindFirstChild("HumanoidRootPart")
    end
    return nil
end

local function parsePosition(pos)
    if type(pos) ~= "table" then return nil end
    local x = tonumber(pos[1])
    local y = tonumber(pos[2])
    local z = tonumber(pos[3])
    if not x or not y or not z then return nil end
    if x ~= x or y ~= y or z ~= z then return nil end -- NaN check
    return Vector3.new(x, y, z)
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

local function canReach(player, worldPosition, maxDistance)
    local hrp = getHumanoidRootPart(player)
    if not hrp then
        return false
    end
    return (hrp.Position - worldPosition).Magnitude <= maxDistance
end

--------------------------------------------------------------------------------
-- BLOCKS
--------------------------------------------------------------------------------

local function isBlockPart(inst)
    return inst and inst:IsA("Part") and inst:GetAttribute("CarryKind") == "Block"
end

local function isResidueClump(inst)
    return inst and inst:IsA("Part") and inst:GetAttribute("CarryKind") == "Clump"
end

local releaseHeldObjectReferences
local setBlockMetadata

local function getHiddenBlockState(block)
    if not isBlockPart(block) then
        return nil
    end

    local state = getOrCreateChild(serverBlockState, "Folder", block.Name)
    local partRef = getOrCreateChild(state, "ObjectValue", "Part")
    partRef.Value = block

    local colorName = BLOCK_TYPE_NAME
    state:SetAttribute("BaseColorName", colorName)
    if state:GetAttribute("CoatingPhase") == nil then
        state:SetAttribute("CoatingPhase", "Bare")
    end
    if state:GetAttribute("SpawnPos") == nil then
        state:SetAttribute("SpawnPos", block.Position)
    end

    return state
end

local function getBlockBaseColor(block)
    local state = getHiddenBlockState(block)
    local colorName = state and state:GetAttribute("BaseColorName") or block:GetAttribute("ItemColorName") or BLOCK_TYPE_NAME
    return blockColors[colorName] or blockColors[BLOCK_TYPE_NAME]
end

local function getBlockCoatingPhase(block)
    local state = getHiddenBlockState(block)
    return state and state:GetAttribute("CoatingPhase") or "Bare"
end

local function getBlockBondKey(block)
    if not isBlockPart(block) then
        return nil
    end
    return block.Name
end

local function getBlockBondTargets(block)
    local key = getBlockBondKey(block)
    local targetNames = key and blockBondTargets[key] or nil
    if type(targetNames) ~= "table" then
        return {}
    end

    local targets = {}
    local dirty = false
    for targetName, _ in pairs(targetNames) do
        local target = Workspace:FindFirstChild(targetName)
        if isBlockPart(target) then
            table.insert(targets, target)
        else
            targetNames[targetName] = nil
            dirty = true
        end
    end

    if dirty and next(targetNames) == nil then
        blockBondTargets[key] = nil
    end

    return targets
end

local function hasBlockBondTarget(block, targetBlock)
    local key = getBlockBondKey(block)
    local targetKey = getBlockBondKey(targetBlock)
    if not key or not targetKey then
        return false
    end
    local targetNames = blockBondTargets[key]
    return type(targetNames) == "table" and targetNames[targetKey] == true
end

local function addBlockBondTarget(block, targetBlock)
    local key = getBlockBondKey(block)
    local targetKey = getBlockBondKey(targetBlock)
    if not key or not targetKey then
        return
    end

    local targetNames = blockBondTargets[key]
    if type(targetNames) ~= "table" then
        targetNames = {}
        blockBondTargets[key] = targetNames
    end
    targetNames[targetKey] = true
end

local function removeBlockBondTarget(block, targetBlock)
    local key = getBlockBondKey(block)
    local targetKey = getBlockBondKey(targetBlock)
    if not key then
        return
    end

    local targetNames = blockBondTargets[key]
    if type(targetNames) ~= "table" then
        return
    end

    if targetKey then
        targetNames[targetKey] = nil
    end

    if next(targetNames) == nil then
        blockBondTargets[key] = nil
    end
end

local function hasAnyBlockBondTargets(block)
    return #getBlockBondTargets(block) > 0
end

local function setBlockBondFrozen(block, frozen)
    local key = getBlockBondKey(block)
    if not key then
        return
    end

    if frozen then
        frozenBondBlocks[key] = true
    else
        frozenBondBlocks[key] = nil
    end
end

local function isBlockBondFrozen(block)
    local key = getBlockBondKey(block)
    return key ~= nil and frozenBondBlocks[key] == true
end

local function setAssemblyBondFrozen(memberStates, frozen)
    for _, memberState in ipairs(memberStates or {}) do
        setBlockBondFrozen(memberState.object, frozen)
    end
end

local function removeBlockBonds(block)
    if not block or not block.Parent then
        return
    end

    for object, _ in pairs(trackedCarryables) do
        if object.Parent and isBlockPart(object) then
            for _, child in ipairs(object:GetChildren()) do
                if child:IsA("WeldConstraint")
                    and child.Name == RESIDUE_BOND_NAME
                    and (child.Part0 == block or child.Part1 == block)
                then
                    local otherBlock = child.Part0 == block and child.Part1 or child.Part0
                    if isBlockPart(otherBlock) then
                        removeBlockBondTarget(otherBlock, block)
                    end
                    child:Destroy()
                end
            end
        end
    end

    removeBlockBondTarget(block, nil)
    block.Anchored = false
end

local function refreshBlockAppearance(block)
    if not isBlockPart(block) then
        return
    end

    local state = getHiddenBlockState(block)
    local phase = state and state:GetAttribute("CoatingPhase") or "Bare"
    local baseColor = getBlockBaseColor(block)

    if phase == "Coated" then
        block.Color = baseColor:Lerp(RESIDUE_COLOR, 0.48)
        block.Material = Enum.Material.SmoothPlastic
    elseif phase == "Set" then
        block.Color = baseColor:Lerp(RESIDUE_SET_COLOR, 0.42)
        block.Material = Enum.Material.Slate
    else
        block.Color = baseColor
        block.Material = Enum.Material.SmoothPlastic
    end
end

local function clearBlockTreatment(block)
    if not isBlockPart(block) then
        return
    end

    removeBlockBonds(block)
    local state = getHiddenBlockState(block)
    state:SetAttribute("CoatingPhase", "Bare")
    refreshBlockAppearance(block)
end

local function getBlockSpawnPosition(block)
    local state = getHiddenBlockState(block)
    local spawnPos = state and state:GetAttribute("SpawnPos")
    if typeof(spawnPos) == "Vector3" then
        return spawnPos
    end
    return block.Position
end

local function getBondedBlocks(rootBlock)
    if not isBlockPart(rootBlock) then
        return {}
    end

    local blocks = {}
    local visited = {}
    local pending = { rootBlock }

    while #pending > 0 do
        local block = table.remove(pending)
        local blockKey = getBlockBondKey(block)
        if block and block.Parent and blockKey and not visited[blockKey] then
            visited[blockKey] = true
            table.insert(blocks, block)

            for _, target in ipairs(getBlockBondTargets(block)) do
                local targetKey = getBlockBondKey(target)
                if targetKey and not visited[targetKey] then
                    table.insert(pending, target)
                end
            end
        end
    end

    return blocks
end

local function getCarryAssemblyMembers(rootObject)
    if isBlockPart(rootObject) then
        local bondedBlocks = getBondedBlocks(rootObject)
        if #bondedBlocks > 0 then
            return bondedBlocks
        end
    end
    return { rootObject }
end

local function getCarryRootObject(rootObject)
    if not isBlockPart(rootObject) then
        return rootObject
    end

    local bondedBlocks = getBondedBlocks(rootObject)
    if #bondedBlocks == 0 then
        return rootObject
    end

    for _, block in ipairs(bondedBlocks) do
        local phase = getBlockCoatingPhase(block)
        if phase == "Set" or phase == "Coated" then
            return block
        end
    end

    return bondedBlocks[1]
end

local function captureAssemblyRelativeCFrames(rootObject)
    local rootCFrame = rootObject.CFrame
    local members = {}
    for _, member in ipairs(getCarryAssemblyMembers(rootObject)) do
        if member and member.Parent then
            table.insert(members, {
                object = member,
                relativeCFrame = rootCFrame:ToObjectSpace(member.CFrame),
            })
        end
    end
    return members
end

local function applyAssemblyRootCFrame(rootObject, rootCFrame, memberStates)
    if memberStates then
        for _, memberState in ipairs(memberStates) do
            local member = memberState.object
            if member and member.Parent then
                member.CFrame = rootCFrame * memberState.relativeCFrame
                member.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
                member.AssemblyAngularVelocity = Vector3.new(0, 0, 0)
            end
        end
    else
        rootObject.CFrame = rootCFrame
        rootObject.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
        rootObject.AssemblyAngularVelocity = Vector3.new(0, 0, 0)

        local members = captureAssemblyRelativeCFrames(rootObject)
        for _, memberState in ipairs(members) do
            local member = memberState.object
            if member and member.Parent and member ~= rootObject then
                member.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
                member.AssemblyAngularVelocity = Vector3.new(0, 0, 0)
            end
        end
    end
end

local function getCurrentRootRotationCFrame(object)
    local pitch, yaw, roll = object.CFrame:ToOrientation()
    return CFrame.fromOrientation(pitch, yaw, roll)
end

local function getTargetRootCFrame(object, targetPosition, rotation)
    local rotationCFrame = getCurrentRootRotationCFrame(object)
    if rotation ~= nil then
        local pitch, yaw, roll = parseRotationDegrees(rotation)
        if pitch == nil then
            return nil
        end
        rotationCFrame = CFrame.fromOrientation(math.rad(pitch), math.rad(yaw), math.rad(roll))
    end
    return CFrame.new(targetPosition) * rotationCFrame
end

local function resetFallenBlockGroup(block)
    local bondedBlocks = getBondedBlocks(block)
    if #bondedBlocks == 0 then
        return
    end

    for _, bondedBlock in ipairs(bondedBlocks) do
        releaseHeldObjectReferences(bondedBlock)
        clearBlockTreatment(bondedBlock)
    end

    for _, bondedBlock in ipairs(bondedBlocks) do
        bondedBlock.Anchored = true
        bondedBlock.CanCollide = false
        bondedBlock.Position = getBlockSpawnPosition(bondedBlock)
        bondedBlock.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
        bondedBlock.AssemblyAngularVelocity = Vector3.new(0, 0, 0)
        setBlockMetadata(bondedBlock, bondedBlock:GetAttribute("ItemColorName"))
        bondedBlock.Anchored = false
        bondedBlock.CanCollide = true
        trackedCarryables[bondedBlock] = true
    end
end

local function handleFallenBlockGroup(block)
    if BLOCK_OUT_OF_BOUNDS_MODE == "respawn" then
        resetFallenBlockGroup(block)
        return
    end

    local bondedBlocks = getBondedBlocks(block)
    if #bondedBlocks == 0 then
        return
    end

    for _, bondedBlock in ipairs(bondedBlocks) do
        releaseHeldObjectReferences(bondedBlock)
        clearBlockTreatment(bondedBlock)
        trackedCarryables[bondedBlock] = nil
        if bondedBlock.Parent then
            bondedBlock:Destroy()
        end
    end
end

local function getBondTargets(block)
    if not isBlockPart(block) then
        return {}, false
    end

    local params = OverlapParams.new()
    params.FilterType = Enum.RaycastFilterType.Exclude
    params.FilterDescendantsInstances = { block }

    local hits = Workspace:GetPartBoundsInBox(
        CFrame.new(block.Position),
        block.Size + Vector3.new(RESIDUE_CONTACT_PADDING, RESIDUE_CONTACT_PADDING, RESIDUE_CONTACT_PADDING),
        params
    )

    local blockTargets = {}
    local seenBlocks = {}
    local terrainHit = false
    for _, hit in ipairs(hits) do
        if hit:IsA("Part") then
            local role = hit:GetAttribute("RenderRole")
            if role == "Ground" or role == "Mesa" or role == "Rock" then
                terrainHit = true
            end
            if isBlockPart(hit) then
                local targetKey = getBlockBondKey(hit)
                if targetKey and not seenBlocks[targetKey] then
                    seenBlocks[targetKey] = true
                    table.insert(blockTargets, hit)
                end
            end
        end
    end

    return blockTargets, terrainHit
end

local function hasResidueBondBetween(blockA, blockB)
    if not isBlockPart(blockA) or not isBlockPart(blockB) then
        return false
    end

    for _, parent in ipairs({ blockA, blockB }) do
        for _, child in ipairs(parent:GetChildren()) do
            if child:IsA("WeldConstraint")
                and child.Name == RESIDUE_BOND_NAME
                and ((child.Part0 == blockA and child.Part1 == blockB)
                    or (child.Part0 == blockB and child.Part1 == blockA))
            then
                return true
            end
        end
    end

    return false
end

local function trySetBlockBond(block)
    if not isBlockPart(block) then
        return false
    end
    if isBlockBondFrozen(block) then
        return false
    end

    local state = getHiddenBlockState(block)
    local phase = getBlockCoatingPhase(block)
    local targets, terrainHit = getBondTargets(block)

    local should_mark_set = phase == "Coated"
    local createdBond = false

    for _, target in ipairs(targets) do
        if not isBlockBondFrozen(target) then
            local can_initiate = phase == "Coated"
            local bond_owner = block
            local bond_other = target
            if not can_initiate and getBlockCoatingPhase(target) == "Set" then
                can_initiate = true
                bond_owner = target
                bond_other = block
            end

            if can_initiate then
                if should_mark_set then
                    state:SetAttribute("CoatingPhase", "Set")
                    refreshBlockAppearance(block)
                    should_mark_set = false
                end
                if not (hasBlockBondTarget(block, target) and hasResidueBondBetween(block, target)) then
                    local bond = Instance.new("WeldConstraint")
                    bond.Name = RESIDUE_BOND_NAME
                    bond.Part0 = bond_owner
                    bond.Part1 = bond_other
                    bond.Parent = bond_owner
                    addBlockBondTarget(block, target)
                    addBlockBondTarget(target, block)
                end
                createdBond = true
            end
        end
    end

    if createdBond then
        return true
    end

    if terrainHit then
        if phase ~= "Coated" then
            return false
        end
        if should_mark_set then
            state:SetAttribute("CoatingPhase", "Set")
            refreshBlockAppearance(block)
        end
        if not hasAnyBlockBondTargets(block) and block.Anchored then
            return true
        end

        if not hasAnyBlockBondTargets(block) then
            removeBlockBonds(block)
            block.Anchored = true
        end
        return true
    end

    return false
end

local function coatBlock(block)
    if not isBlockPart(block) then
        return false
    end

    removeBlockBonds(block)
    local state = getHiddenBlockState(block)
    state:SetAttribute("CoatingPhase", "Coated")
    refreshBlockAppearance(block)
    trySetBlockBond(block)
    return true
end

local function setResidueMetadata(clump)
    clump:SetAttribute("RenderRole", "Clump")
    clump:SetAttribute("IsCarryable", true)
    clump:SetAttribute("CarryKind", "Clump")
    clump:SetAttribute("PlacementDuration", 0.12)
    clump:SetAttribute("HoldOffset", RESIDUE_HOLD_OFFSET)
end

createSecretionClump = function(position)
    local clumpId = nextServerCounter("NextResidueId")
    local clump = Instance.new("Part")
    clump.Name = "Clump_" .. clumpId
    clump.Shape = Enum.PartType.Block
    clump.Size = RESIDUE_CLUMP_SIZE
    clump.Position = position
    clump.Anchored = false
    clump.CanCollide = true
    clump.Color = RESIDUE_COLOR
    clump.Material = Enum.Material.SmoothPlastic
    clump.Transparency = 0.08
    clump.CustomPhysicalProperties = PhysicalProperties.new(0.35, 1.0, 0.0, 1.0, 1.0)
    setResidueMetadata(clump)
    clump.Parent = Workspace
    trackedCarryables[clump] = true
    return clump
end

local function findNearestBlock(targetPosition, maxDistance)
    local closestBlock = nil
    local closestDistance = maxDistance

    for _, inst in ipairs(Workspace:GetDescendants()) do
        if isBlockPart(inst) then
            local distance = (inst.Position - targetPosition).Magnitude
            if distance <= closestDistance then
                closestBlock = inst
                closestDistance = distance
            end
        end
    end

    return closestBlock
end

setBlockMetadata = function(block, colorName)
    block:SetAttribute("RenderRole", BLOCK_RENDER_ROLE)
    block:SetAttribute("ItemColorName", BLOCK_TYPE_NAME)
    block:SetAttribute("IsPickupObject", true)
    block:SetAttribute("IsCarryable", true)
    block:SetAttribute("CarryKind", "Block")
    block:SetAttribute("PlacementDuration", DEFAULT_PLACE_DURATION)
    block:SetAttribute("HoldOffset", DEFAULT_HOLD_OFFSET)
    local state = getHiddenBlockState(block)
    if state:GetAttribute("SpawnPos") == nil then
        state:SetAttribute("SpawnPos", block.Position)
    end
    refreshBlockAppearance(block)
end

local function createBlock(name, position, colorName)
    local block = Instance.new("Part")
    block.Name = name
    block.Size = Vector3.new(BLOCK_SIZE, BLOCK_SIZE, BLOCK_SIZE)
    block.Position = position
    block.Anchored = false
    block.Material = Enum.Material.Slate
    block.Color = blockColors[BLOCK_TYPE_NAME]
    block.CustomPhysicalProperties = blockPhysics
    setBlockMetadata(block, BLOCK_TYPE_NAME)
    block.Parent = Workspace
    trackedCarryables[block] = true
    return block
end

local function isCarryableObject(inst)
    return inst:IsA("Part") and (inst:GetAttribute("IsCarryable") or inst:GetAttribute("IsPickupObject"))
end

local function rebuildTrackedCarryables()
    trackedCarryables = {}
    blockBondTargets = {}
    for _, inst in ipairs(Workspace:GetDescendants()) do
        if inst:IsA("Part") and inst:GetAttribute("IsPickupObject") then
            inst:SetAttribute("IsCarryable", true)
            inst:SetAttribute("CarryKind", inst:GetAttribute("CarryKind") or "Block")
            inst:SetAttribute("PlacementDuration", inst:GetAttribute("PlacementDuration") or DEFAULT_PLACE_DURATION)
            if inst:GetAttribute("HoldOffset") == nil then
                inst:SetAttribute("HoldOffset", DEFAULT_HOLD_OFFSET)
            end
            if inst:GetAttribute("CarryKind") == "Block" then
                getHiddenBlockState(inst)
                refreshBlockAppearance(inst)
            end
        elseif isResidueClump(inst) then
            setResidueMetadata(inst)
        end
        if isCarryableObject(inst) then
            trackedCarryables[inst] = true
        end
    end

    for object, _ in pairs(trackedCarryables) do
        if object.Parent and isBlockPart(object) then
            for _, child in ipairs(object:GetChildren()) do
                if child:IsA("WeldConstraint") and child.Name == RESIDUE_BOND_NAME then
                    local part0 = child.Part0
                    local part1 = child.Part1
                    if isBlockPart(part0) and isBlockPart(part1) then
                        addBlockBondTarget(part0, part1)
                        addBlockBondTarget(part1, part0)
                    end
                end
            end
        end
    end
end

local function initializeBlocks()
    if serverGlobals:GetAttribute("BlocksInitialized") then
        return
    end

    local blockIndex = 0
    local blockPileOrigin = Vector3.new(-22, 0.5, -14)
    local blockPileSpacing = BLOCK_SIZE + 0.05
    for layer = 0, 2 do
        for row = 0, 2 do
            for col = 0, 2 do
                if layer < 2 or (row < 2 and col < 2) then
                    blockIndex = blockIndex + 1
                    createBlock(
                        "Block_" .. blockIndex,
                        blockPileOrigin + Vector3.new(
                            col * blockPileSpacing,
                            layer * blockPileSpacing,
                            row * blockPileSpacing
                        ),
                        BLOCK_TYPE_NAME
                    )
                end
            end
        end
    end

    local clumpPileOrigin = Vector3.new(-8.5, 0.25, 14)
    for layer = 0, 1 do
        for row = 0, 2 do
            for col = 0, 2 do
                local shouldSpawn = not (layer == 1 and row == 2 and col > 0)
                if shouldSpawn then
                    createSecretionClump(
                        clumpPileOrigin + Vector3.new(
                            col * 0.5,
                            layer * 0.35,
                            row * 0.5
                        )
                    )
                end
            end
        end
    end

    serverGlobals:SetAttribute("BlocksInitialized", true)
end

local function fullResetWorld()
    resetWorldState()
    clearTable(trackedCarryables)
    clearTable(blockBondTargets)
    clearTable(heldAssemblyStates)
    clearTable(frozenBondBlocks)
    clearTable(activeMoveStates)
    initializeStaticWorld()
    initializeBlocks()
    rebuildTrackedCarryables()
    for _, player in ipairs(Players:GetPlayers()) do
        setupPlayer(player, true)
        resetPlayerToSpawn(player)
        player:SetAttribute("LastActionResult", "ResetSuccess")
    end
end

initializeBlocks()
rebuildTrackedCarryables()

--------------------------------------------------------------------------------
-- PLAYER
--------------------------------------------------------------------------------

local function setHeldObjectForPlayer(player, object)
    local _, heldObject = getOrCreatePlayerState(player)
    heldObject.Value = object
end

local function getHeldAssemblyFolder(player)
    local playerState = getOrCreatePlayerState(player)
    return getOrCreateChild(playerState, "Folder", "HeldAssembly")
end

local function readStoredAssemblyMemberStates(folder)
    local memberStates = {}
    for _, memberFolder in ipairs(folder:GetChildren()) do
        if memberFolder:IsA("Folder") then
            local memberRef = memberFolder:FindFirstChild("Object")
            local relativeCFrame = memberFolder:FindFirstChild("RelativeCFrame")
            local member = memberRef and memberRef.Value or nil
            if member and member.Parent and relativeCFrame then
                table.insert(memberStates, {
                    object = member,
                    relativeCFrame = relativeCFrame.Value,
                })
            end
        end
    end
    return memberStates
end

local function getHeldAssemblyState(player, object)
    local folder = getHeldAssemblyFolder(player)
    local objectRef = folder:FindFirstChild("Object")
    local storedObject = objectRef and objectRef.Value or nil
    if storedObject and storedObject.Parent and (object == nil or storedObject == object or object.Parent) then
        return {
            object = storedObject,
            memberStates = readStoredAssemblyMemberStates(folder),
        }
    end
    return nil
end

local function popHeldAssemblyState(player, object)
    local state = getHeldAssemblyState(player, object)
    if state then
        local folder = getHeldAssemblyFolder(player)
        for _, child in ipairs(folder:GetChildren()) do
            child:Destroy()
        end
    end
    return state
end

local function clearHeldAssemblyState(player)
    local state = popHeldAssemblyState(player)
    if state then
        setAssemblyBondFrozen(state.memberStates, false)
    end
end

local function setHeldAssemblyState(player, object, memberStates)
    clearHeldAssemblyState(player)
    local frozenStates = memberStates or captureAssemblyRelativeCFrames(object)
    local folder = getHeldAssemblyFolder(player)
    local objectRef = getOrCreateChild(folder, "ObjectValue", "Object")
    objectRef.Value = object
    for index, memberState in ipairs(frozenStates) do
        local memberFolder = Instance.new("Folder")
        memberFolder.Name = "Member_" .. index
        memberFolder.Parent = folder
        local memberRef = getOrCreateChild(memberFolder, "ObjectValue", "Object")
        local relativeCFrame = getOrCreateChild(memberFolder, "CFrameValue", "RelativeCFrame")
        memberRef.Value = memberState.object
        relativeCFrame.Value = memberState.relativeCFrame
    end
    setAssemblyBondFrozen(frozenStates, true)
    return frozenStates
end

local function reconcileBlockAssembly(memberStates)
    local blocks = {}
    for _, memberState in ipairs(memberStates or {}) do
        local member = memberState.object
        if member and member.Parent and isBlockPart(member) then
            table.insert(blocks, member)
        end
    end

    for _, block in ipairs(blocks) do
        if getBlockCoatingPhase(block) == "Coated" then
            trySetBlockBond(block)
        end
    end
    for _, block in ipairs(blocks) do
        if getBlockCoatingPhase(block) ~= "Coated" then
            trySetBlockBond(block)
        end
    end
end

local function createPlacementState(object, startPos, targetPos, duration, memberStates)
    local placementId = nextServerCounter("NextPlacementId")
    local folder = getOrCreateChild(serverPlacementState, "Folder", "Placement_" .. placementId)
    local objectRef = getOrCreateChild(folder, "ObjectValue", "Object")
    local startCFrameValue = getOrCreateChild(folder, "CFrameValue", "StartCFrame")
    local targetCFrameValue = getOrCreateChild(folder, "CFrameValue", "TargetCFrame")
    local membersFolder = getOrCreateChild(folder, "Folder", "Members")
    objectRef.Value = object
    startCFrameValue.Value = startPos
    targetCFrameValue.Value = targetPos
    for _, child in ipairs(membersFolder:GetChildren()) do
        child:Destroy()
    end
    local frozenStates = memberStates or captureAssemblyRelativeCFrames(object)
    setAssemblyBondFrozen(frozenStates, true)
    for index, memberState in ipairs(frozenStates) do
        local memberFolder = Instance.new("Folder")
        memberFolder.Name = "Member_" .. index
        memberFolder.Parent = membersFolder
        local memberRef = getOrCreateChild(memberFolder, "ObjectValue", "Object")
        local relativeCFrame = getOrCreateChild(memberFolder, "CFrameValue", "RelativeCFrame")
        memberRef.Value = memberState.object
        relativeCFrame.Value = memberState.relativeCFrame
    end
    folder:SetAttribute("Elapsed", 0)
    folder:SetAttribute("Duration", duration)
    return folder
end

local function getPlacementObjectRef(placement)
    return placement:FindFirstChild("Object") or placement:FindFirstChild("Block")
end

local function getObjectHoldOffset(object)
    local offset = object and object:GetAttribute("HoldOffset")
    if typeof(offset) == "Vector3" then
        return offset
    end
    return DEFAULT_HOLD_OFFSET
end

local function getObjectPlacementDuration(object)
    local duration = object and object:GetAttribute("PlacementDuration")
    if typeof(duration) == "number" and duration > 0 then
        return duration
    end
    return DEFAULT_PLACE_DURATION
end

local function getPlacementMemberStates(placement)
    local membersFolder = placement:FindFirstChild("Members")
    if not membersFolder then
        local objectRef = getPlacementObjectRef(placement)
        local object = objectRef and objectRef.Value or nil
        if object and object.Parent then
            return captureAssemblyRelativeCFrames(object)
        end
        return {}
    end

    local memberStates = {}
    for _, memberFolder in ipairs(membersFolder:GetChildren()) do
        local memberRef = memberFolder:FindFirstChild("Object")
        local relativeCFrame = memberFolder:FindFirstChild("RelativeCFrame")
        local member = memberRef and memberRef.Value or nil
        if member and member.Parent and relativeCFrame then
            table.insert(memberStates, {
                object = member,
                relativeCFrame = relativeCFrame.Value,
            })
        end
    end
    return memberStates
end

local function prepareObjectForHolding(object, memberStates)
    object.Anchored = true
    object.CanCollide = false
    for _, memberState in ipairs(memberStates or captureAssemblyRelativeCFrames(object)) do
        local member = memberState.object
        if member and member.Parent then
            if member ~= object then
                member.CanCollide = false
            end
            member.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
            member.AssemblyAngularVelocity = Vector3.new(0, 0, 0)
        end
    end
end

local function restoreObjectAfterPlacement(object, memberStates)
    for _, memberState in ipairs(memberStates or captureAssemblyRelativeCFrames(object)) do
        local member = memberState.object
        if member and member.Parent then
            member.Anchored = false
            member.CanCollide = true
        end
    end
end

local function dropHeldObject(player)
    local heldObject = getHeldObjectForPlayer(player)
    if not heldObject then
        return
    end

    local hrp = getHumanoidRootPart(player)
    if hrp then
        applyAssemblyRootCFrame(
            heldObject,
            CFrame.new(hrp.Position + getObjectHoldOffset(heldObject)) * getCurrentRootRotationCFrame(heldObject)
        )
    end
    restoreObjectAfterPlacement(heldObject)
    clearHeldAssemblyState(player)
    setHeldObjectForPlayer(player, nil)
end

setupPlayer = function(player, forceReset)
    local playerState = getOrCreatePlayerState(player)
    if forceReset then
        playerState:SetAttribute("Initialized", false)
    end
    if playerState:GetAttribute("Initialized") then
        return
    end

    local humanoid = nil
    local character = player.Character
    if character then
        humanoid = character:FindFirstChild("Humanoid")
    end

    if humanoid then
        humanoid.WalkSpeed = 16
        humanoid.JumpPower = 50
        bindMoveFeedback(player, humanoid)
    end

    if character then
        character:SetAttribute("ModelUrl", "asset://player.glb")

        local hrp = character:FindFirstChild("HumanoidRootPart")
        if hrp then
            hrp:SetAttribute("ModelYawOffsetDeg", 180)
            hrp.Position = PLAYER_SPAWN_POSITION
            hrp.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
            hrp.AssemblyAngularVelocity = Vector3.new(0, 0, 0)
        end
    end

    player:SetAttribute("LastActionResult", "Ready")
    playerState:SetAttribute("Initialized", true)
end

local function cleanupPlayer(player)
    activeMoveStates[player] = nil
    local moveConnection = moveFeedbackConnections[player]
    if moveConnection then
        moveConnection:Disconnect()
        moveFeedbackConnections[player] = nil
    end

    dropHeldObject(player)
end

resetPlayerToSpawn = function(player)
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
        activeMoveStates[player] = nil
        humanoid:CancelMoveTo()
    end
end

releaseHeldObjectReferences = function(object)
    for _, player in ipairs(Players:GetPlayers()) do
        local heldObject = getHeldObjectForPlayer(player)
        if heldObject == object then
            clearHeldAssemblyState(player)
            setHeldObjectForPlayer(player, nil)
            player:SetAttribute("LastActionResult", "ResetAfterFall")
        end
    end

    for _, placement in ipairs(serverPlacementState:GetChildren()) do
        local objectRef = getPlacementObjectRef(placement)
        local shouldDestroyPlacement = objectRef and objectRef.Value == object
        if not shouldDestroyPlacement then
            for _, memberState in ipairs(getPlacementMemberStates(placement)) do
                if memberState.object == object then
                    shouldDestroyPlacement = true
                    break
                end
            end
        end
        if shouldDestroyPlacement then
            setAssemblyBondFrozen(getPlacementMemberStates(placement), false)
            placement:Destroy()
        end
    end
end

local function destroyCarryableObject(object)
    if not object.Parent then
        return
    end
    releaseHeldObjectReferences(object)
    trackedCarryables[object] = nil
    local blockState = isBlockPart(object) and serverBlockState:FindFirstChild(object.Name) or nil
    if blockState then
        blockState:Destroy()
    end
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

bindMoveFeedback = function(player, humanoid)
    local existingConnection = moveFeedbackConnections[player]
    if existingConnection then
        existingConnection:Disconnect()
    end

    moveFeedbackConnections[player] = humanoid.MoveToFinished:Connect(function(reached)
        local moveState = activeMoveStates[player]
        if not moveState then
            return
        end

        activeMoveStates[player] = nil
        if reached then
            player:SetAttribute("LastActionResult", "MoveToFinished:Reached")
        else
            player:SetAttribute("LastActionResult", "MoveToFinished:Blocked")
        end
    end)
end

findNearestCarryableObject = function(targetPosition, maxDistance)
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

local function pickupObject(player, inputData)
    if getHeldObjectForPlayer(player) then
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

    local object = findNearestCarryableObject(targetPosition, 1.5)
    if not object then
        player:SetAttribute("LastActionResult", "PickupFailed:NoObject")
        return
    end

    local carryRoot = getCarryRootObject(object)
    local memberStates = captureAssemblyRelativeCFrames(carryRoot)
    prepareObjectForHolding(carryRoot, memberStates)
    setHeldObjectForPlayer(player, carryRoot)
    setHeldAssemblyState(player, carryRoot, memberStates)
    player:SetAttribute("LastActionResult", "PickupSuccess")
end

local function positionOccupied(player, targetRootCFrame, object)
    local heldState = getHeldAssemblyState(player, object)
    local memberStates = heldState and heldState.memberStates or captureAssemblyRelativeCFrames(object)
    local params = OverlapParams.new()
    params.FilterType = Enum.RaycastFilterType.Exclude
    params.FilterDescendantsInstances = { player.Character }
    for _, memberState in ipairs(memberStates) do
        table.insert(params.FilterDescendantsInstances, memberState.object)
    end

    for _, memberState in ipairs(memberStates) do
        local projectedCFrame = targetRootCFrame * memberState.relativeCFrame
        local member = memberState.object
        local hits = Workspace:GetPartBoundsInBox(
            projectedCFrame,
            member.Size,
            params
        )

        for _, hit in ipairs(hits) do
            if hit:IsA("Part") then
                return true
            end
        end
    end

    return false
end

local function placeHeldObject(player, inputData)
    local object = getHeldObjectForPlayer(player)
    if not object then
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
    local targetRootCFrame = getTargetRootCFrame(object, targetPosition, inputData.rotation)
    if not targetRootCFrame then
        player:SetAttribute("LastActionResult", "PlaceFailed:InvalidRotation")
        return
    end
    if not canReach(player, targetPosition, PLACE_RANGE) then
        player:SetAttribute("LastActionResult", "PlaceFailed:OutOfRange")
        return
    end

    if isResidueClump(object) then
        local targetBlock = findNearestBlock(targetPosition, RESIDUE_PLACE_RANGE)
        if targetBlock then
            setHeldObjectForPlayer(player, nil)
            coatBlock(targetBlock)
            destroyCarryableObject(object)
            player:SetAttribute("LastActionResult", "PlaceSuccess")
            return
        end
    end

    if positionOccupied(player, targetRootCFrame, object) then
        player:SetAttribute("LastActionResult", "PlaceFailed:Occupied")
        return
    end

    local heldState = popHeldAssemblyState(player, object)
    local memberStates = heldState and heldState.memberStates or captureAssemblyRelativeCFrames(object)
    for _, memberState in ipairs(memberStates) do
        local member = memberState.object
        if member and member.Parent then
            member.Anchored = true
            member.CanCollide = false
            member.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
            member.AssemblyAngularVelocity = Vector3.new(0, 0, 0)
        end
    end
    object.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
    object.AssemblyAngularVelocity = Vector3.new(0, 0, 0)

    local startRootCFrame = object.CFrame
    setHeldObjectForPlayer(player, nil)

    createPlacementState(object, startRootCFrame, targetRootCFrame, getObjectPlacementDuration(object), memberStates)

    player:SetAttribute("LastActionResult", "PlaceSuccess")
end

if AgentInputService then
    AgentInputService.InputReceived:Connect(function(player, inputType, inputData)
        local humanoid = getHumanoid(player)
        if not humanoid then return end

        if inputType == "MoveTo" and inputData and inputData.position then
            local pos = inputData.position
            local target = parsePosition(pos)
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
        elseif inputType == "Reset" then
            pendingWorldReset = true
            player:SetAttribute("LastActionResult", "ResetQueued")
        elseif inputType == "Pickup" then
            pickupObject(player, inputData)
        elseif inputType == "Place" then
            placeHeldObject(player, inputData)
        end
    end)
end

--------------------------------------------------------------------------------
-- MAIN LOOP
--------------------------------------------------------------------------------

RunService.Heartbeat:Connect(function(dt)
    if pendingWorldReset then
        pendingWorldReset = false
        fullResetWorld()
        return
    end

    for _, player in ipairs(Players:GetPlayers()) do
        local hrp = getHumanoidRootPart(player)
        if hrp and hrp.Position.Y < OUT_OF_BOUNDS_Y then
            resetPlayerToSpawn(player)
        end
    end

    -- Update held objects
    for _, player in ipairs(Players:GetPlayers()) do
        local heldObject = getHeldObjectForPlayer(player)
        local hrp = getHumanoidRootPart(player)
        if hrp and heldObject then
            applyAssemblyRootCFrame(
                heldObject,
                CFrame.new(hrp.Position + getObjectHoldOffset(heldObject)) * getCurrentRootRotationCFrame(heldObject)
            )
        end
    end

    -- Animate placing objects
    for _, placement in ipairs(serverPlacementState:GetChildren()) do
        local objectRef = getPlacementObjectRef(placement)
        local object = objectRef and objectRef.Value or nil
        if not object or not object.Parent then
            setAssemblyBondFrozen(getPlacementMemberStates(placement), false)
            placement:Destroy()
        else
        local elapsed = (placement:GetAttribute("Elapsed") or 0) + dt
        local duration = placement:GetAttribute("Duration") or DEFAULT_PLACE_DURATION
        local startCFrameValue = placement:FindFirstChild("StartCFrame")
        local targetCFrameValue = placement:FindFirstChild("TargetCFrame")
        local startCFrame = startCFrameValue and startCFrameValue.Value or object.CFrame
        local targetCFrame = targetCFrameValue and targetCFrameValue.Value or object.CFrame
        local alpha = math.min(elapsed / duration, 1)
        -- Smooth ease-out
        local t = 1 - (1 - alpha) * (1 - alpha)
        applyAssemblyRootCFrame(object, startCFrame:Lerp(targetCFrame, t), getPlacementMemberStates(placement))

        if alpha >= 1 then
            local placementMemberStates = getPlacementMemberStates(placement)
            restoreObjectAfterPlacement(object, placementMemberStates)
            setAssemblyBondFrozen(placementMemberStates, false)
            if object:GetAttribute("CarryKind") == "Block" then
                for _, memberState in ipairs(placementMemberStates) do
                    local member = memberState.object
                    if member and member.Parent and member:GetAttribute("CarryKind") == "Block" then
                        setBlockMetadata(member, member:GetAttribute("ItemColorName"))
                    end
                end
                reconcileBlockAssembly(placementMemberStates)
            end
            placement:Destroy()
        else
            placement:SetAttribute("Elapsed", elapsed)
        end
        end
    end

    local resetBlocks = {}
    local resetSeen = {}
    for object, _ in pairs(trackedCarryables) do
        if not object.Parent then
            trackedCarryables[object] = nil
        else
            if isBlockPart(object) and getBlockCoatingPhase(object) == "Coated" then
                trySetBlockBond(object)
            end
            if object.Position.Y < OUT_OF_BOUNDS_Y then
                if isBlockPart(object) then
                    local bondedBlocks = getBondedBlocks(object)
                    for _, bondedBlock in ipairs(bondedBlocks) do
                        if not resetSeen[bondedBlock] then
                            resetSeen[bondedBlock] = true
                            table.insert(resetBlocks, bondedBlock)
                        end
                    end
                else
                    destroyCarryableObject(object)
                end
            end
        end
    end

    for _, block in ipairs(resetBlocks) do
        if block.Parent and block.Position.Y < OUT_OF_BOUNDS_Y then
            handleFallenBlockGroup(block)
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

print("=== River Bridge ===")
