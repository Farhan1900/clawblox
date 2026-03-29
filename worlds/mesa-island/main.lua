-- Mesa Island - An island world with alien flora

local RunService = game:GetService("RunService")
local Players = game:GetService("Players")
local AgentInputService = game:GetService("AgentInputService")
local ServerStorage = game:GetService("ServerStorage")

-- Workspace.Gravity = 196.2
Workspace.Gravity = 10

local BLOCK_SIZE = 0.75
local PICKUP_RANGE = 6
local PLACE_RANGE = 7
local SPAWN_HEIGHT = 5
local SPAWN_OFFSET_RADIUS = 4
local SPAWN_CLEARANCE_SIZE = Vector3.new(4, 6, 4)
local DAY_LENGTH_SECONDS = 60 * 60
local DAYLIGHT_START_HOUR = 6
local DAYLIGHT_END_HOUR = 18
local serverGlobals
local getNextSpawnPosition
-- Spawn points on open ground, centered on safe lowland areas.
local SPAWN_POINTS = {
    Vector3.new(25, SPAWN_HEIGHT, -20),     -- south of center hill
    Vector3.new(40, SPAWN_HEIGHT, -55),     -- south-east plain
    Vector3.new(-30, SPAWN_HEIGHT, 25),     -- west lowland
    Vector3.new(38, SPAWN_HEIGHT, 50),      -- east of NE ledge
    Vector3.new(-45, SPAWN_HEIGHT, -35),    -- north of SW plateau
    Vector3.new(80, SPAWN_HEIGHT, -25),     -- south of east ridge
    Vector3.new(-35, SPAWN_HEIGHT, 55),     -- south of NW stones
    Vector3.new(20, SPAWN_HEIGHT, 85),      -- north
    Vector3.new(25, SPAWN_HEIGHT, -90),     -- south cove
    Vector3.new(130, SPAWN_HEIGHT, 25),     -- east peninsula tip
}
local OUT_OF_BOUNDS_Y = -200

local trackedCarryables = {}
local activeMoveStates = {}
local moveFeedbackConnections = {}
local bindMoveFeedback
local findNearestCarryableObject
local createSecretionClump
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
local SKUTTLER_BITE_DISTANCE = 1.35
local SKUTTLER_BITE_COOLDOWN = 5
local SKUTTLER_FORAGE_COOLDOWN_AFTER_BITE = 180
local SKUTTLER_SECRETION_PER_BITE = 1
local SKUTTLER_SECRETION_FOR_CLUMP = 2
local SKUTTLER_CLUMP_COOLDOWN = 14

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

local serverStateRoot = getOrCreateChild(ServerStorage, "Folder", "MesaIslandState")
serverGlobals = getOrCreateChild(serverStateRoot, "Folder", "Globals")
local serverPlayerState = getOrCreateChild(serverStateRoot, "Folder", "Players")
local serverPlacementState = getOrCreateChild(serverStateRoot, "Folder", "Placements")
local serverFloraState = getOrCreateChild(serverStateRoot, "Folder", "Flora")
local serverCreatureState = getOrCreateChild(serverStateRoot, "Folder", "Creatures")
local serverBlockState = getOrCreateChild(serverStateRoot, "Folder", "Blocks")

if serverGlobals:GetAttribute("NextSpawnIndex") == nil then
    serverGlobals:SetAttribute("NextSpawnIndex", 0)
end
if serverGlobals:GetAttribute("NextPlacementId") == nil then
    serverGlobals:SetAttribute("NextPlacementId", 0)
end
if serverGlobals:GetAttribute("NextCreatureId") == nil then
    serverGlobals:SetAttribute("NextCreatureId", 0)
end
if serverGlobals:GetAttribute("NextResidueId") == nil then
    serverGlobals:SetAttribute("NextResidueId", 0)
end
if serverGlobals:GetAttribute("StaticWorldInitialized") == nil then
    serverGlobals:SetAttribute("StaticWorldInitialized", false)
end
if serverGlobals:GetAttribute("GlowBulbsInitialized") == nil then
    serverGlobals:SetAttribute("GlowBulbsInitialized", false)
end
if serverGlobals:GetAttribute("CreaturesInitialized") == nil then
    serverGlobals:SetAttribute("CreaturesInitialized", false)
end
if serverGlobals:GetAttribute("BlocksInitialized") == nil then
    serverGlobals:SetAttribute("BlocksInitialized", false)
end
if serverGlobals:GetAttribute("DayCycleElapsedSeconds") == nil then
    serverGlobals:SetAttribute("DayCycleElapsedSeconds", 0)
end
if serverGlobals:GetAttribute("CurrentDayNumber") == nil then
    serverGlobals:SetAttribute("CurrentDayNumber", 1)
end

local function getDayPhase(daylightHour)
    if daylightHour < 10 then
        return "Morning"
    elseif daylightHour < 14 then
        return "Midday"
    end
    return "Afternoon"
end

local function getDayCycleState(elapsedSeconds)
    local safeElapsed = math.max(0, elapsedSeconds or 0)
    local secondsIntoDay = safeElapsed % DAY_LENGTH_SECONDS
    local dayProgress = secondsIntoDay / DAY_LENGTH_SECONDS
    local daylightHours = DAYLIGHT_END_HOUR - DAYLIGHT_START_HOUR
    local daylightHour = DAYLIGHT_START_HOUR + dayProgress * daylightHours
    local totalMinutes = math.floor(daylightHour * 60 + 0.5)
    local hour = math.floor(totalMinutes / 60)
    local minute = totalMinutes % 60
    local dayNumber = math.floor(safeElapsed / DAY_LENGTH_SECONDS) + 1

    return {
        elapsedSeconds = safeElapsed,
        secondsIntoDay = secondsIntoDay,
        dayNumber = dayNumber,
        dayProgress = dayProgress,
        daylightHour = daylightHour,
        dayPhase = getDayPhase(daylightHour),
        clockText = string.format("%02d:%02d", hour, minute),
    }
end

local function applyDayCycleAttributes(player, dayState)
    player:SetAttribute("DayLengthSeconds", DAY_LENGTH_SECONDS)
    player:SetAttribute("DayNumber", dayState.dayNumber)
    player:SetAttribute("DayProgress", math.floor(dayState.dayProgress * 1000 + 0.5) / 1000)
    player:SetAttribute("DayTimeSeconds", math.floor(dayState.secondsIntoDay + 0.5))
    player:SetAttribute("DaylightHour", math.floor(dayState.daylightHour * 100 + 0.5) / 100)
    player:SetAttribute("DayPhase", dayState.dayPhase)
    player:SetAttribute("ClockTime", dayState.clockText)
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
-- GROUND
--------------------------------------------------------------------------------

-- Ocean: large flat plane surrounding the island
if not serverGlobals:GetAttribute("StaticWorldInitialized") then
    local ocean = Instance.new("Part")
    ocean.Name = "Ocean"
    ocean.Size = Vector3.new(600, 1, 600)
    ocean.Position = Vector3.new(0, -1.0, 0)
    ocean.Anchored = true
    ocean.CanCollide = false
    ocean.Color = Color3.fromRGB(30, 90, 140)
    ocean.Transparency = 0.4
    ocean:SetAttribute("RenderRole", "Ocean")
    ocean.Parent = Workspace

    -- Island ground: irregular shape built from overlapping parts
end
local groundParts = {
    -- Main body (overlapping ovals to break the square)
    { size = {220, 10, 180}, pos = {0, -5, 0} },
    { size = {160, 10, 240}, pos = {10, -5, 20} },
    -- Peninsulas and bays
    { size = {80, 10, 60},  pos = {120, -5, 30} },   -- east peninsula
    { size = {70, 10, 80},  pos = {-110, -5, -20} },  -- west bulge
    { size = {60, 10, 70},  pos = {-20, -5, 120} },   -- north spit
    { size = {50, 10, 60},  pos = {40, -5, -110} },   -- south cove
}

if not serverGlobals:GetAttribute("StaticWorldInitialized") then
    for i, g in ipairs(groundParts) do
        local part = Instance.new("Part")
        part.Name = "Ground_" .. i
        part.Size = Vector3.new(g.size[1], g.size[2], g.size[3])
        part.Position = Vector3.new(g.pos[1], g.pos[2], g.pos[3])
        part.Anchored = true
        part.Color = Color3.fromRGB(85, 239, 196)
        part:SetAttribute("RenderRole", "Ground")
        part.Parent = Workspace
    end
end

-- Mesas: organized into natural clusters
local mesas = {
    -- === Central highlands: a modest hill the player spawns near ===
    { name = "CenterHill",    size = {30, 8, 30},    pos = {0, 4, 0} },
    { name = "CenterStep",    size = {18, 3, 14},    pos = {18, 1.5, 10} },

    -- === Northeast cluster: ascending path to a peak ===
    { name = "NELedge",       size = {35, 5, 25},    pos = {60, 2.5, 55} },
    { name = "NEShelf",       size = {22, 10, 18},   pos = {72, 5, 70} },
    { name = "NEPeak",        size = {12, 16, 12},   pos = {78, 8, 82} },

    -- === Southwest cluster: larger footprint, same elevation profile ===
    { name = "SWSteppingStone", size = {10, 2.5, 10},   pos = {-65, 1.25, -62} },
    { name = "SWTallMesa",      size = {16, 6, 14},     pos = {-72, 3, -70} },
    { name = "SWTowerBase",     size = {10, 7, 10},     pos = {-75, 3.5, -76} },
    { name = "SWSummit",        size = {8, 10, 8},      pos = {-75, 5, -80} },
    { name = "SWSpire",         size = {5, 20, 5},      pos = {-77, 10, -83} },

    -- === East ridge: long narrow formation ===
    { name = "EastRidge",     size = {16, 6, 60},    pos = {100, 3, 10} },

    -- === Northwest scattered stones ===
    { name = "NWStone1",      size = {14, 3, 12},    pos = {-50, 1.5, 70} },
    { name = "NWStone2",      size = {10, 4, 10},    pos = {-65, 2, 85} },
}

if not serverGlobals:GetAttribute("StaticWorldInitialized") then
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
end

--------------------------------------------------------------------------------
-- ROCKS (decorative, with collision)
--------------------------------------------------------------------------------

-- Rock clusters on open ground, away from mesa footprints
-- Each cluster has 2-4 rocks of varying size grouped tightly
local _rockSeed = 9999
local function rockRandom()
    _rockSeed = (_rockSeed * 16807) % 2147483647
    return (_rockSeed - 1) / 2147483646
end

local rockClusters = {
    -- South-east open plain
    { cx = 40, cz = -60, n = 3 },
    -- West lowland
    { cx = -35, cz = 15, n = 3 },
    -- North open area
    { cx = 15, cz = 95, n = 2 },
    -- East peninsula tip
    { cx = 130, cz = 35, n = 2 },
    -- South cove
    { cx = 30, cz = -105, n = 3 },
}

if not serverGlobals:GetAttribute("StaticWorldInitialized") then
    local rockIndex = 0
    for _, cluster in ipairs(rockClusters) do
        for j = 1, cluster.n do
            rockIndex = rockIndex + 1
            local ox = (rockRandom() - 0.5) * 8
            local oz = (rockRandom() - 0.5) * 8

            local radius = 1.2 + rockRandom() * 1.2
            local sx = radius * 2 * (1.0 + rockRandom() * 0.5)
            local sy = radius * 2 * (0.5 + rockRandom() * 0.5)
            local sz = radius * 2 * (1.0 + rockRandom() * 0.5)

            local rock = Instance.new("Part")
            rock.Name = "Rock_" .. rockIndex
            rock.Size = Vector3.new(sx, sy, sz)
            rock.Position = Vector3.new(cluster.cx + ox, sy / 2 + 0.5, cluster.cz + oz)
            rock.Anchored = false
            rock.Color = ROCK_COLOR
            rock.CustomPhysicalProperties = PhysicalProperties.new(
                8.0,  -- density (very heavy, needs multiple players to push)
                0.9,  -- friction (high, won't slide easily)
                0.0,  -- elasticity (no bounce)
                1.0,
                1.0
            )
            rock:SetAttribute("RenderRole", "Rock")
            rock.Parent = Workspace
        end
    end
    serverGlobals:SetAttribute("StaticWorldInitialized", true)
end

--------------------------------------------------------------------------------
-- HELPERS (needed by flora + input handling)
--------------------------------------------------------------------------------

-- Check if a point (x, z) is over land (inside any ground part's XZ footprint)
local function isOverLand(x, z)
    for _, g in ipairs(groundParts) do
        local gx, gz = g.pos[1], g.pos[3]
        local hw, hd = g.size[1] / 2, g.size[3] / 2
        if x >= gx - hw and x <= gx + hw and z >= gz - hd and z <= gz + hd then
            return true
        end
    end
    return false
end

local function getHumanoidRootPart(player)
    local character = player.Character
    if character then
        return character:FindFirstChild("HumanoidRootPart")
    end
    return nil
end

local function canReach(player, worldPosition, maxDistance)
    local hrp = getHumanoidRootPart(player)
    if not hrp then
        return false
    end
    return (hrp.Position - worldPosition).Magnitude <= maxDistance
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

local function isSpawnPositionClear(position)
    if not isOverLand(position.X, position.Z) then
        return false
    end
    if isInsideMesaFootprint(position.X, position.Z, 1.5) then
        return false
    end

    local hits = Workspace:GetPartBoundsInBox(CFrame.new(position), SPAWN_CLEARANCE_SIZE)
    for _, hit in ipairs(hits) do
        if hit:IsA("Part") then
            local role = hit:GetAttribute("RenderRole")
            if role ~= "Ground" and role ~= "Ocean" then
                return false
            end
        end
    end

    return true
end

getNextSpawnPosition = function()
    local nextSpawnIndex = (serverGlobals:GetAttribute("NextSpawnIndex") or 0) + 1
    serverGlobals:SetAttribute("NextSpawnIndex", nextSpawnIndex)
    local base = SPAWN_POINTS[((nextSpawnIndex - 1) % #SPAWN_POINTS) + 1]

    for _ = 1, 24 do
        local ox = (math.random() - 0.5) * (SPAWN_OFFSET_RADIUS * 2)
        local oz = (math.random() - 0.5) * (SPAWN_OFFSET_RADIUS * 2)
        local candidate = Vector3.new(base.X + ox, SPAWN_HEIGHT, base.Z + oz)
        if isSpawnPositionClear(candidate) then
            return candidate
        end
    end

    local fallback = Vector3.new(base.X, SPAWN_HEIGHT, base.Z)
    if isSpawnPositionClear(fallback) then
        return fallback
    end

    for _, alternate in ipairs(SPAWN_POINTS) do
        local candidate = Vector3.new(alternate.X, SPAWN_HEIGHT, alternate.Z)
        if isSpawnPositionClear(candidate) then
            return candidate
        end
    end

    return fallback
end

--------------------------------------------------------------------------------
-- FLORA
--------------------------------------------------------------------------------

local CONSUME_RANGE = 3
local GLOWBULB_REGROW_SECONDS = 180
local GLOWBULB_EFFECT_DELAY = 120   -- 2 minutes before immobilization kicks in
local GLOWBULB_EFFECT_DURATION = 300 -- 5 minutes immobilized
local GLOWBULB_IMMOBILIZATION_ENABLED = false -- temporary until the effect is reworked

local GLOWBULB_COLOR = Color3.fromRGB(0, 220, 200)
local GLOWBULB_BASE_SIZE = Vector3.new(0.7, 0.7, 0.7)
local GLOWBULB_MAX_BITE_STEPS = 4
local GLOWBULB_MIN_SCALE = 0.35

local function clampGlowBulbBiteStep(step)
    return math.clamp(math.floor(step or 0), 0, GLOWBULB_MAX_BITE_STEPS)
end

local function getGlowBulbScaleForStep(step)
    local clampedStep = clampGlowBulbBiteStep(step)
    if GLOWBULB_MAX_BITE_STEPS <= 1 then
        return 1
    end
    local alpha = (clampedStep - 1) / (GLOWBULB_MAX_BITE_STEPS - 1)
    return GLOWBULB_MIN_SCALE + (1 - GLOWBULB_MIN_SCALE) * alpha
end

local function applyGlowBulbVisuals(floraState, bulb)
    local harvested = floraState:GetAttribute("Harvested")
    if harvested then
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
    bulb:SetAttribute("RenderRole", "GlowBulb")
    bulb:SetAttribute("IsEdible", true)
    bulb.Parent = Workspace

    local floraState = getOrCreateChild(serverFloraState, "Folder", name)
    local partRef = getOrCreateChild(floraState, "ObjectValue", "Part")
    partRef.Value = bulb
    floraState:SetAttribute("Species", "GlowBulb")
    floraState:SetAttribute("Age", math.random() * 6.28)
    floraState:SetAttribute("BiteStep", GLOWBULB_MAX_BITE_STEPS)
    floraState:SetAttribute("Harvested", false)
    floraState:SetAttribute("RegrowTimer", 0)
    floraState:SetAttribute("SpawnPos", position)
    applyGlowBulbVisuals(floraState, bulb)
    return bulb
end

-- GlowBulbs cluster just outside mesa edges (shade-loving, at ground level)
-- Each cluster is 2-3 bulbs with small random offsets
local glowBulbClusters = {
    -- Outside CenterHill (edges at x=±15, z=±15)
    { cx = 18, cz = 5, n = 3 },     -- east edge
    { cx = -18, cz = -5, n = 2 },   -- west edge
    -- Outside NELedge (edges at x=42..78, z=42..68)
    { cx = 40, cz = 55, n = 3 },    -- west edge
    { cx = 65, cz = 70, n = 2 },    -- north edge
    -- Outside southwest spire approach
    { cx = -48, cz = -55, n = 2 },  -- east edge
    { cx = -65, cz = -40, n = 3 },  -- north edge
    -- Outside EastRidge (edges at x=92..108, z=-20..40)
    { cx = 90, cz = -25, n = 2 },   -- south-west corner
    { cx = 90, cz = 45, n = 2 },    -- north-west corner
    -- Outside NW stones (NWStone1 edges at x=-57..-43, z=64..76)
    { cx = -40, cz = 70, n = 2 },   -- east edge
    { cx = -58, cz = 78, n = 3 },   -- north edge
}

local function sampleGlowBulbPosition(cluster)
    for _ = 1, 24 do
        local ox = (math.random() - 0.5) * 6
        local oz = (math.random() - 0.5) * 6
        local x = cluster.cx + ox
        local z = cluster.cz + oz
        if isOverLand(x, z) and not isInsideMesaFootprint(x, z, 0.5) then
            return Vector3.new(x, 0.5, z)
        end
    end

    -- Fall back to the cluster center only if it is actually walkable.
    if isOverLand(cluster.cx, cluster.cz) and not isInsideMesaFootprint(cluster.cx, cluster.cz, 0.5) then
        return Vector3.new(cluster.cx, 0.5, cluster.cz)
    end

    return nil
end

if not serverGlobals:GetAttribute("GlowBulbsInitialized") then
    local glowBulbIndex = 0
    for _, cluster in ipairs(glowBulbClusters) do
        for j = 1, cluster.n do
            local bulbPosition = sampleGlowBulbPosition(cluster)
            if bulbPosition then
                glowBulbIndex = glowBulbIndex + 1
                createGlowBulb("GlowBulb_" .. glowBulbIndex, bulbPosition)
            end
        end
    end
    serverGlobals:SetAttribute("GlowBulbsInitialized", true)
end

local function findNearestEdible(targetPosition, maxDistance)
    local closest = nil
    local closestDist = maxDistance
    for _, floraState in ipairs(serverFloraState:GetChildren()) do
        local partRef = floraState:FindFirstChild("Part")
        local part = partRef and partRef.Value or nil
        local harvested = floraState:GetAttribute("Harvested")
        if part and part.Parent and not harvested and part:GetAttribute("IsEdible") then
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
    if not inputData or not inputData.position then
        player:SetAttribute("LastActionResult", "ConsumeFailed:MissingPosition")
        return
    end

    local pos = inputData.position
    local targetPosition = Vector3.new(pos[1], pos[2], pos[3])
    if not canReach(player, targetPosition, CONSUME_RANGE) then
        player:SetAttribute("LastActionResult", "ConsumeFailed:OutOfRange")
        return
    end

    local plant = findNearestEdible(targetPosition, 2.0)
    if not plant then
        player:SetAttribute("LastActionResult", "ConsumeFailed:NothingEdible")
        return
    end

    local floraState = serverFloraState:FindFirstChild(plant.Name)
    if not floraState then
        player:SetAttribute("LastActionResult", "ConsumeFailed:NothingEdible")
        return
    end

    local species = floraState:GetAttribute("Species") or "Unknown"
    if species == "GlowBulb" then
        if not takeGlowBulbBite(floraState, plant) then
            player:SetAttribute("LastActionResult", "ConsumeFailed:NothingEdible")
            return
        end
        player:SetAttribute("LastActionResult", string.format("ConsumeSuccess:%s", species))
    else
        floraState:SetAttribute("Harvested", true)
        floraState:SetAttribute("RegrowTimer", GLOWBULB_REGROW_SECONDS)
        plant.Transparency = 1
        plant.CanQuery = false
        player:SetAttribute("LastActionResult", "ConsumeSuccess:" .. species)
    end

    if species == "GlowBulb" and GLOWBULB_IMMOBILIZATION_ENABLED then
        local playerState = getOrCreatePlayerState(player)
        playerState:SetAttribute("ImmobilizeDelayRemaining", GLOWBULB_EFFECT_DELAY)
        playerState:SetAttribute("ImmobilizeDurationRemaining", GLOWBULB_EFFECT_DURATION)
        playerState:SetAttribute("ImmobilizeActive", false)
    end
end

--------------------------------------------------------------------------------
-- FAUNA
--------------------------------------------------------------------------------

local FAUNA_FLEE_RANGE = 8
local FAUNA_FLEE_SPEED = 7
local FAUNA_WANDER_INTERVAL = 3 -- seconds between wander direction changes
local FAUNA_BOUNDARY = 120 -- keep creatures within this radius of origin
local FAUNA_OBSTACLE_PADDING = 0.35

local speciesDefs = {
    Orbweaver = {
        shape = Enum.PartType.Ball,
        size = Vector3.new(1.0, 1.0, 1.0),
        color = Color3.fromRGB(120, 60, 200),
        speed = 2.5,
        fleeRange = FAUNA_FLEE_RANGE,
        hoverHeight = 3.0, -- extra height above ground (floats)
        attractedTo = "GlowBulb",
        attractRange = 20,
        motionStyle = "kinematic",
        carryable = false,
        canCollide = false,
    },
    Skuttler = {
        shape = Enum.PartType.Block,
        size = Vector3.new(0.6, 0.55, 0.8),
        color = Color3.fromRGB(180, 100, 60),
        speed = 3.5,
        fleeRange = 5,
        hoverHeight = 0, -- walks on ground
        attractedTo = "GlowBulb",
        attractRange = 18,
        motionStyle = "physics",
        carryable = true,
        canCollide = true,
        placementDuration = 0.25,
        holdOffset = Vector3.new(0, 3.0, 0),
        physicsProperties = PhysicalProperties.new(
            0.2,
            0.8,
            0.0,
            1.0,
            1.0
        ),
    },
    Drifter = {
        shape = Enum.PartType.Cylinder,
        size = Vector3.new(1.5, 0.3, 1.5),
        color = Color3.fromRGB(160, 200, 160),
        speed = 1.2,
        fleeRange = 0, -- ignores players
        hoverHeight = 0, -- glides on ground
        attractedTo = nil,
        attractRange = 0,
        motionStyle = "kinematic",
        carryable = false,
        canCollide = false,
    },
}

-- Ground Y for a creature: half its height (so bottom touches surface) + any hover
local function creatureGroundY(def)
    return def.size.Y / 2 + (def.hoverHeight or 0)
end

local function randomWanderDir()
    local angle = math.random() * math.pi * 2
    return Vector3.new(math.cos(angle), 0, math.sin(angle))
end

local function refreshCreatureAppearance(part, creatureState)
    if not part or not part.Parent or not creatureState then
        return
    end

    local speciesName = creatureState:GetAttribute("Species")
    local def = speciesDefs[speciesName]
    if not def then
        return
    end

    if speciesName == "Skuttler" then
        local secretionLevel = creatureState:GetAttribute("SecretionLevel") or 0
        local tintAlpha = math.clamp(secretionLevel / (SKUTTLER_SECRETION_FOR_CLUMP * 2), 0, 0.45)
        part.Color = def.color:Lerp(RESIDUE_COLOR, tintAlpha)
        part.Material = tintAlpha > 0 and Enum.Material.Slate or Enum.Material.SmoothPlastic
    else
        part.Color = def.color
        part.Material = Enum.Material.SmoothPlastic
    end
end

local function getCreatureStateForPart(part)
    local stateName = part and part:GetAttribute("CreatureStateName")
    if not stateName then
        return nil
    end
    return serverCreatureState:FindFirstChild(stateName)
end

local function setCreatureMotionMode(creatureState, mode)
    creatureState:SetAttribute("MotionMode", mode)

    local partRef = creatureState:FindFirstChild("Part")
    local part = partRef and partRef.Value or nil
    if not part or not part.Parent then
        return
    end

    local speciesName = creatureState:GetAttribute("Species")
    local def = speciesDefs[speciesName]
    if not def then
        return
    end

    if mode == "AI" then
        part.Anchored = def.motionStyle ~= "physics"
        part.CanCollide = def.canCollide == true
    else
        part.Anchored = true
        part.CanCollide = false
        part.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
        part.AssemblyAngularVelocity = Vector3.new(0, 0, 0)
    end
end

local function createCreature(speciesName, position)
    local def = speciesDefs[speciesName]
    if not def then
        warn("Unknown creature species: " .. tostring(speciesName))
        return nil
    end

    local part = Instance.new("Part")
    part.Name = speciesName
    part.Shape = def.shape
    part.Size = def.size
    part.Position = Vector3.new(position.X, creatureGroundY(def), position.Z)
    part.Anchored = def.motionStyle ~= "physics"
    part.CanCollide = def.canCollide == true
    part.Color = def.color
    part.Material = Enum.Material.SmoothPlastic
    if def.physicsProperties then
        part.CustomPhysicalProperties = def.physicsProperties
    end
    part:SetAttribute("RenderRole", speciesName)
    part:SetAttribute("Species", speciesName)
    part:SetAttribute("CarryKind", "Creature")
    part:SetAttribute("IsCarryable", def.carryable == true)
    part:SetAttribute("PlacementDuration", def.placementDuration or DEFAULT_PLACE_DURATION)
    part:SetAttribute("HoldOffset", def.holdOffset or DEFAULT_HOLD_OFFSET)
    part.Parent = Workspace

    local creatureId = nextServerCounter("NextCreatureId")
    local creatureStateName = "Creature_" .. creatureId
    local creatureState = getOrCreateChild(serverCreatureState, "Folder", creatureStateName)
    local partRef = getOrCreateChild(creatureState, "ObjectValue", "Part")
    partRef.Value = part
    creatureState:SetAttribute("Species", speciesName)
    creatureState:SetAttribute("WanderDir", randomWanderDir())
    creatureState:SetAttribute("WanderTimer", math.random() * FAUNA_WANDER_INTERVAL)
    creatureState:SetAttribute("HomePos", position)
    creatureState:SetAttribute("MotionMode", "AI")
    creatureState:SetAttribute("FeedCooldown", 0)
    creatureState:SetAttribute("ForageCooldown", 0)
    creatureState:SetAttribute("SecretionLevel", 0)
    creatureState:SetAttribute("ClumpCooldown", 0)
    part:SetAttribute("CreatureStateName", creatureStateName)
    refreshCreatureAppearance(part, creatureState)
    if def.carryable then
        trackedCarryables[part] = true
    end
    return part
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
    return closest, closestDist
end

local function nearestFloraPosition(pos, speciesFilter, maxRange)
    local closest = nil
    local closestDist = maxRange
    for _, floraState in ipairs(serverFloraState:GetChildren()) do
        local partRef = floraState:FindFirstChild("Part")
        local part = partRef and partRef.Value or nil
        local harvested = floraState:GetAttribute("Harvested")
        local species = floraState:GetAttribute("Species")
        if part and part.Parent and not harvested then
            if speciesFilter == nil or species == speciesFilter then
                local dist = (part.Position - pos).Magnitude
                if dist < closestDist then
                    closest = part.Position
                    closestDist = dist
                end
            end
        end
    end
    return closest, closestDist
end

local function nearestFloraTarget(pos, speciesFilter, maxRange)
    local closestPart = nil
    local closestState = nil
    local closestDist = maxRange
    for _, floraState in ipairs(serverFloraState:GetChildren()) do
        local partRef = floraState:FindFirstChild("Part")
        local part = partRef and partRef.Value or nil
        local harvested = floraState:GetAttribute("Harvested")
        local species = floraState:GetAttribute("Species")
        if part and part.Parent and not harvested then
            if speciesFilter == nil or species == speciesFilter then
                local dist = (part.Position - pos).Magnitude
                if dist < closestDist then
                    closestPart = part
                    closestState = floraState
                    closestDist = dist
                end
            end
        end
    end
    return closestPart, closestState, closestDist
end

local function isCreatureObstacle(selfPart, hit)
    if not hit:IsA("Part") or hit == selfPart then
        return false
    end
    if hit:GetAttribute("CarryKind") == "Block" or hit:GetAttribute("IsPickupObject") then
        return true
    end
    if hit:GetAttribute("RenderRole") == "Rock" then
        return true
    end
    return false
end

local function isCreaturePathBlocked(part, def, moveDir)
    if moveDir.Magnitude <= 0.01 then
        return false
    end

    local params = OverlapParams.new()
    params.FilterType = Enum.RaycastFilterType.Exclude
    params.FilterDescendantsInstances = { part }

    local forward = moveDir.Unit
    local lookAhead = math.max(def.size.X, def.size.Z) * 0.75 + FAUNA_OBSTACLE_PADDING
    local probeCenter = part.Position + forward * lookAhead
    local probeSize = Vector3.new(
        def.size.X + FAUNA_OBSTACLE_PADDING,
        math.max(def.size.Y, 1.0),
        def.size.Z + FAUNA_OBSTACLE_PADDING
    )

    local hits = Workspace:GetPartBoundsInBox(CFrame.new(probeCenter), probeSize, params)
    for _, hit in ipairs(hits) do
        if isCreatureObstacle(part, hit) then
            return true
        end
    end

    return false
end

local function maybeDropSkuttlerClump(creatureState, part, fleeing)
    if not creatureState or not part or not part.Parent or fleeing then
        return
    end

    if creatureState:GetAttribute("Species") ~= "Skuttler" then
        return
    end

    local secretionLevel = creatureState:GetAttribute("SecretionLevel") or 0
    local clumpCooldown = creatureState:GetAttribute("ClumpCooldown") or 0
    if secretionLevel < SKUTTLER_SECRETION_FOR_CLUMP or clumpCooldown > 0 then
        return
    end

    local dropPosition = Vector3.new(part.Position.X, RESIDUE_CLUMP_SIZE.Y * 0.6, part.Position.Z)
    for object, _ in pairs(trackedCarryables) do
        if object ~= part and object.Parent and (object.Position - dropPosition).Magnitude <= 0.7 then
            return
        end
    end

    createSecretionClump(dropPosition)
    creatureState:SetAttribute("SecretionLevel", secretionLevel - SKUTTLER_SECRETION_FOR_CLUMP)
    creatureState:SetAttribute("ClumpCooldown", SKUTTLER_CLUMP_COOLDOWN)
    refreshCreatureAppearance(part, creatureState)
end

local function updateCreatures(dt)
    for _, creatureState in ipairs(serverCreatureState:GetChildren()) do
        local partRef = creatureState:FindFirstChild("Part")
        local part = partRef and partRef.Value or nil
        if not part or not part.Parent then
            creatureState:Destroy()
        else
            local speciesName = creatureState:GetAttribute("Species")
            local def = speciesDefs[speciesName]
            if not def then
                creatureState:Destroy()
            else
                local feedCooldown = math.max(0, (creatureState:GetAttribute("FeedCooldown") or 0) - dt)
                local forageCooldown = math.max(0, (creatureState:GetAttribute("ForageCooldown") or 0) - dt)
                local clumpCooldown = math.max(0, (creatureState:GetAttribute("ClumpCooldown") or 0) - dt)
                creatureState:SetAttribute("FeedCooldown", feedCooldown)
                creatureState:SetAttribute("ForageCooldown", forageCooldown)
                creatureState:SetAttribute("ClumpCooldown", clumpCooldown)

                local motionMode = creatureState:GetAttribute("MotionMode") or "AI"
                if motionMode ~= "AI" then
                    if motionMode == "Held" then
                        part.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
                        part.AssemblyAngularVelocity = Vector3.new(0, 0, 0)
                    end
                else
                local pos = part.Position
                local flatPos = Vector3.new(pos.X, 0, pos.Z)
                local moveDir = Vector3.new(0, 0, 0)
                local moveSpeed = def.speed
                local fleeing = false
                local foodPart = nil
                local foodState = nil
                local foodDist = def.attractRange

                -- 1. Flee from nearby players
                if def.fleeRange > 0 then
                    local playerPos, playerDist = nearestPlayerPosition(pos, def.fleeRange)
                    if playerPos then
                        local away = (flatPos - Vector3.new(playerPos.X, 0, playerPos.Z))
                        if away.Magnitude > 0.01 then
                            moveDir = away.Unit
                        end
                        moveSpeed = FAUNA_FLEE_SPEED
                        fleeing = true
                    end
                end

                -- 2. Attract toward food source (if not fleeing)
                if not fleeing and forageCooldown <= 0 and def.attractedTo and def.attractRange > 0 then
                    foodPart, foodState, foodDist = nearestFloraTarget(pos, def.attractedTo, def.attractRange)
                    if foodPart then
                        local toward = Vector3.new(foodPart.Position.X, 0, foodPart.Position.Z) - flatPos
                        if toward.Magnitude > 1.5 then
                            moveDir = toward.Unit
                        else
                            moveDir = Vector3.new(0, 0, 0) -- arrived near food, idle
                        end
                    end
                end

                -- 3. Wander (if no other behavior active)
                if not fleeing and moveDir.Magnitude < 0.01 then
                    local wanderTimer = (creatureState:GetAttribute("WanderTimer") or 0) - dt
                    if wanderTimer <= 0 then
                        local wanderDir = randomWanderDir()
                        wanderTimer = FAUNA_WANDER_INTERVAL + math.random() * 2
                        creatureState:SetAttribute("WanderDir", wanderDir)
                    end
                    creatureState:SetAttribute("WanderTimer", wanderTimer)
                    moveDir = creatureState:GetAttribute("WanderDir") or Vector3.new(0, 0, 0)
                end

                -- 4. Boundary check: steer back toward home if too far
                local homePos = creatureState:GetAttribute("HomePos") or Vector3.new(0, 0, 0)
                local homeOffset = flatPos - Vector3.new(homePos.X, 0, homePos.Z)
                if homeOffset.Magnitude > FAUNA_BOUNDARY then
                    moveDir = -homeOffset.Unit
                end

                -- Apply movement (only if staying on land)
                if moveDir.Magnitude > 0.01 then
                    local delta = moveDir.Unit * moveSpeed * dt
                    local newX = pos.X + delta.X
                    local newZ = pos.Z + delta.Z
                    if isOverLand(newX, newZ) and not isCreaturePathBlocked(part, def, moveDir) then
                        if def.motionStyle == "physics" then
                            local currentVelocity = part.AssemblyLinearVelocity
                            local desiredVelocity = moveDir.Unit * moveSpeed
                            part.AssemblyLinearVelocity = Vector3.new(
                                desiredVelocity.X,
                                currentVelocity.Y,
                                desiredVelocity.Z
                            )
                            part.AssemblyAngularVelocity = Vector3.new(0, 0, 0)
                        else
                            part.Position = Vector3.new(newX, creatureGroundY(def), newZ)
                        end
                    else
                        local wanderDir = creatureState:GetAttribute("WanderDir")
                        if wanderDir then
                            creatureState:SetAttribute("WanderDir", -wanderDir)
                        end
                        if def.motionStyle == "physics" then
                            local currentVelocity = part.AssemblyLinearVelocity
                            part.AssemblyLinearVelocity = Vector3.new(0, currentVelocity.Y, 0)
                        end
                    end
                elseif def.motionStyle == "physics" then
                    local currentVelocity = part.AssemblyLinearVelocity
                    part.AssemblyLinearVelocity = Vector3.new(0, currentVelocity.Y, 0)
                    part.AssemblyAngularVelocity = Vector3.new(0, 0, 0)
                end

                if not fleeing and foodPart and foodState and foodDist <= SKUTTLER_BITE_DISTANCE and feedCooldown <= 0 then
                    if takeGlowBulbBite(foodState, foodPart) then
                        creatureState:SetAttribute("FeedCooldown", SKUTTLER_BITE_COOLDOWN)
                        creatureState:SetAttribute(
                            "ForageCooldown",
                            SKUTTLER_FORAGE_COOLDOWN_AFTER_BITE + math.random() * 4
                        )
                        creatureState:SetAttribute(
                            "SecretionLevel",
                            (creatureState:GetAttribute("SecretionLevel") or 0) + SKUTTLER_SECRETION_PER_BITE
                        )
                        local awayFromFood = flatPos - Vector3.new(foodPart.Position.X, 0, foodPart.Position.Z)
                        if awayFromFood.Magnitude > 0.01 then
                            creatureState:SetAttribute("WanderDir", awayFromFood.Unit)
                            creatureState:SetAttribute("WanderTimer", FAUNA_WANDER_INTERVAL + math.random())
                        end
                        refreshCreatureAppearance(part, creatureState)
                    end
                end

                maybeDropSkuttlerClump(creatureState, part, fleeing)
                end
            end
        end
    end
end

-- Spawn creatures near their ecological niches
local creatureSpawns = {
    -- Orbweavers are disabled while the physical fauna path is being tuned.
    -- { species = "Orbweaver", pos = Vector3.new(15, 0, 18) },
    -- { species = "Orbweaver", pos = Vector3.new(55, 0, 50) },
    -- { species = "Orbweaver", pos = Vector3.new(-60, 0, -50) },
    -- { species = "Orbweaver", pos = Vector3.new(-50, 0, 68) },

    -- Skuttlers: roam open ground between formations
    { species = "Skuttler", pos = Vector3.new(35, 0, -30) },    -- open south-east
    { species = "Skuttler", pos = Vector3.new(-30, 0, 30) },    -- open west
    { species = "Skuttler", pos = Vector3.new(80, 0, -20) },    -- east ridge area
    { species = "Skuttler", pos = Vector3.new(-20, 0, -70) },   -- south open
    { species = "Skuttler", pos = Vector3.new(30, 0, 90) },     -- north

    -- Drifters are disabled while the physical fauna path is being tuned.
    -- { species = "Drifter", pos = Vector3.new(40, 0, 30) },
    -- { species = "Drifter", pos = Vector3.new(-40, 0, 40) },
    -- { species = "Drifter", pos = Vector3.new(10, 0, -50) },
}

if not serverGlobals:GetAttribute("CreaturesInitialized") then
    for i, spawn in ipairs(creatureSpawns) do
        createCreature(spawn.species, spawn.pos)
    end
    serverGlobals:SetAttribute("CreaturesInitialized", true)
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

    return state
end

local function getBlockBaseColor(block)
    local state = getHiddenBlockState(block)
    local colorName = state and state:GetAttribute("BaseColorName") or block:GetAttribute("ItemColorName") or BLOCK_TYPE_NAME
    return blockColors[colorName] or blockColors[BLOCK_TYPE_NAME]
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
                    child:Destroy()
                end
            end
        end
    end

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

local function getNearestBondTarget(block)
    if not isBlockPart(block) then
        return nil, nil
    end

    local params = OverlapParams.new()
    params.FilterType = Enum.RaycastFilterType.Exclude
    params.FilterDescendantsInstances = { block }

    local hits = Workspace:GetPartBoundsInBox(
        CFrame.new(block.Position),
        block.Size + Vector3.new(RESIDUE_CONTACT_PADDING, RESIDUE_CONTACT_PADDING, RESIDUE_CONTACT_PADDING),
        params
    )

    local nearestBlock = nil
    local nearestBlockDistance = math.huge
    local terrainHit = nil
    for _, hit in ipairs(hits) do
        if hit:IsA("Part") then
            local role = hit:GetAttribute("RenderRole")
            if role == "Ground" or role == "Mesa" or role == "Rock" then
                terrainHit = hit
            end
            if isBlockPart(hit) then
                local distance = (hit.Position - block.Position).Magnitude
                if distance < nearestBlockDistance then
                    nearestBlock = hit
                    nearestBlockDistance = distance
                end
            end
        end
    end

    -- Prefer block-to-block bonds over terrain bonds
    if nearestBlock then
        return "Block", nearestBlock
    end
    if terrainHit then
        return "Terrain", terrainHit
    end

    return nil, nil
end

local function trySetBlockBond(block)
    if not isBlockPart(block) then
        return false
    end

    local state = getHiddenBlockState(block)
    if not state or state:GetAttribute("CoatingPhase") ~= "Coated" then
        return false
    end

    local targetKind, target = getNearestBondTarget(block)
    if not targetKind or not target then
        return false
    end

    removeBlockBonds(block)
    if targetKind == "Terrain" then
        block.Anchored = true
    else
        local bond = Instance.new("WeldConstraint")
        bond.Name = RESIDUE_BOND_NAME
        bond.Part0 = block
        bond.Part1 = target
        bond.Parent = block
    end

    state:SetAttribute("CoatingPhase", "Set")
    refreshBlockAppearance(block)
    return true
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

local function setBlockMetadata(block, colorName)
    block:SetAttribute("RenderRole", BLOCK_RENDER_ROLE)
    block:SetAttribute("ItemColorName", BLOCK_TYPE_NAME)
    block:SetAttribute("IsPickupObject", true)
    block:SetAttribute("IsCarryable", true)
    block:SetAttribute("CarryKind", "Block")
    block:SetAttribute("PlacementDuration", DEFAULT_PLACE_DURATION)
    block:SetAttribute("HoldOffset", DEFAULT_HOLD_OFFSET)
    getHiddenBlockState(block)
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
end

-- Block clusters placed near rock formations for a natural look
-- Rock clusters: (40,-60), (-35,15), (15,95), (130,35), (30,-105)
local blockClusters = {
    -- Near SE rocks (40, -60)
    { cx = 45, cz = -55, baseY = 6, n = 5 },
    { cx = 35, cz = -65, baseY = 6, n = 4 },
    { cx = 48, cz = -62, baseY = 6, n = 3 },

    -- Near W rocks (-35, 15)
    { cx = -30, cz = 20, baseY = 6, n = 5 },
    { cx = -40, cz = 12, baseY = 6, n = 4 },
    { cx = -32, cz = 10, baseY = 6, n = 3 },

    -- Near N rocks (15, 95)
    { cx = 20, cz = 100, baseY = 6, n = 4 },
    { cx = 10, cz = 90, baseY = 6, n = 4 },
    { cx = 18, cz = 92, baseY = 6, n = 3 },

    -- Near E rocks (130, 35)
    { cx = 125, cz = 30, baseY = 6, n = 5 },
    { cx = 133, cz = 40, baseY = 6, n = 3 },
    { cx = 128, cz = 38, baseY = 6, n = 3 },

    -- Near S cove rocks (30, -105)
    { cx = 35, cz = -100, baseY = 6, n = 4 },
    { cx = 25, cz = -110, baseY = 6, n = 4 },
    { cx = 33, cz = -108, baseY = 6, n = 3 },
}

if not serverGlobals:GetAttribute("BlocksInitialized") then
    local blockIndex = 0
    for _, cluster in ipairs(blockClusters) do
        for j = 1, cluster.n do
            blockIndex = blockIndex + 1
            local ox = (j - 1) * 0.7 - (cluster.n - 1) * 0.35
            local oz = ((j % 2) - 0.5) * 0.6
            local oy = (j - 1) * 0.5
            createBlock(
                "Block_" .. blockIndex,
                Vector3.new(cluster.cx + ox, cluster.baseY + oy, cluster.cz + oz),
                BLOCK_TYPE_NAME
            )
        end
    end
    serverGlobals:SetAttribute("BlocksInitialized", true)
end

rebuildTrackedCarryables()

--------------------------------------------------------------------------------
-- PLAYER
--------------------------------------------------------------------------------

local function setHeldObjectForPlayer(player, object)
    local _, heldObject = getOrCreatePlayerState(player)
    heldObject.Value = object
end

local function createPlacementState(object, startPos, targetPos, duration)
    local placementId = nextServerCounter("NextPlacementId")
    local folder = getOrCreateChild(serverPlacementState, "Folder", "Placement_" .. placementId)
    local objectRef = getOrCreateChild(folder, "ObjectValue", "Object")
    objectRef.Value = object
    folder:SetAttribute("StartPos", startPos)
    folder:SetAttribute("TargetPos", targetPos)
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

local function prepareObjectForHolding(object)
    object.Anchored = true
    object.CanCollide = false
    object.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
    object.AssemblyAngularVelocity = Vector3.new(0, 0, 0)

    local creatureState = getCreatureStateForPart(object)
    if creatureState then
        setCreatureMotionMode(creatureState, "Held")
    end
end

local function restoreObjectAfterPlacement(object)
    local creatureState = getCreatureStateForPart(object)
    if creatureState then
        setCreatureMotionMode(creatureState, "AI")
        return
    end

    object.Anchored = false
    object.CanCollide = true
end

local function dropHeldObject(player)
    local heldObject = getHeldObjectForPlayer(player)
    if not heldObject then
        return
    end

    local hrp = getHumanoidRootPart(player)
    if hrp then
        heldObject.Position = hrp.Position + getObjectHoldOffset(heldObject)
    end
    restoreObjectAfterPlacement(heldObject)
    setHeldObjectForPlayer(player, nil)
end

local function setupPlayer(player)
    local playerState = getOrCreatePlayerState(player)
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
        -- humanoid.JumpPower = 5
        bindMoveFeedback(player, humanoid)
    end

    if character then
        character:SetAttribute("ModelUrl", "asset://player.glb")

        local hrp = character:FindFirstChild("HumanoidRootPart")
        if hrp then
            hrp:SetAttribute("ModelYawOffsetDeg", 180)
            hrp.Position = getNextSpawnPosition()
            hrp.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
            hrp.AssemblyAngularVelocity = Vector3.new(0, 0, 0)
        end
    end

    player:SetAttribute("LastActionResult", "Ready")
    applyDayCycleAttributes(player, getDayCycleState(serverGlobals:GetAttribute("DayCycleElapsedSeconds") or 0))
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

local function resetPlayerToSpawn(player)
    local character = player.Character
    if not character then
        return
    end

    local hrp = character:FindFirstChild("HumanoidRootPart")
    local humanoid = character:FindFirstChild("Humanoid")
    if hrp then
        hrp.Position = getNextSpawnPosition()
        hrp.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
        hrp.AssemblyAngularVelocity = Vector3.new(0, 0, 0)
    end
    if humanoid then
        activeMoveStates[player] = nil
        humanoid:CancelMoveTo()
    end
end

local function releaseHeldObjectReferences(object)
    for _, player in ipairs(Players:GetPlayers()) do
        local heldObject = getHeldObjectForPlayer(player)
        if heldObject == object then
            setHeldObjectForPlayer(player, nil)
            player:SetAttribute("LastActionResult", "ResetAfterFall")
        end
    end

    for _, placement in ipairs(serverPlacementState:GetChildren()) do
        local objectRef = getPlacementObjectRef(placement)
        if objectRef and objectRef.Value == object then
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

    prepareObjectForHolding(object)
    setHeldObjectForPlayer(player, object)
    player:SetAttribute("LastActionResult", "PickupSuccess")
end

local function positionOccupied(player, targetPosition, objectSize)
    local params = OverlapParams.new()
    params.FilterType = Enum.RaycastFilterType.Exclude
    params.FilterDescendantsInstances = { player.Character, getHeldObjectForPlayer(player) }

    local hits = Workspace:GetPartBoundsInBox(
        CFrame.new(targetPosition),
        objectSize,
        params
    )

    for _, hit in ipairs(hits) do
        if hit:IsA("Part") then
            return true
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

    local pos = inputData.position
    local targetPosition = Vector3.new(pos[1], pos[2], pos[3])
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

    if positionOccupied(player, targetPosition, object.Size) then
        player:SetAttribute("LastActionResult", "PlaceFailed:Occupied")
        return
    end

    local creatureState = getCreatureStateForPart(object)
    if creatureState then
        setCreatureMotionMode(creatureState, "Placed")
    else
        object.Anchored = true
        object.CanCollide = false
    end
    object.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
    object.AssemblyAngularVelocity = Vector3.new(0, 0, 0)

    local startPos = object.Position
    setHeldObjectForPlayer(player, nil)

    createPlacementState(object, startPos, targetPosition, getObjectPlacementDuration(object))

    player:SetAttribute("LastActionResult", "PlaceSuccess")
end

if AgentInputService then
    AgentInputService.InputReceived:Connect(function(player, inputType, inputData)
        local humanoid = getHumanoid(player)
        if not humanoid then return end

        if inputType == "MoveTo" and inputData and inputData.position then
            local pos = inputData.position
            activeMoveStates[player] = {
                target = Vector3.new(pos[1], pos[2], pos[3]),
            }
            player:SetAttribute("LastActionResult", "MoveToIssued")
            humanoid:MoveTo(Vector3.new(pos[1], pos[2], pos[3]))
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

--------------------------------------------------------------------------------
-- MAIN LOOP
--------------------------------------------------------------------------------

RunService.Heartbeat:Connect(function(dt)
    local dayCycleElapsed = (serverGlobals:GetAttribute("DayCycleElapsedSeconds") or 0) + dt
    serverGlobals:SetAttribute("DayCycleElapsedSeconds", dayCycleElapsed)
    local dayState = getDayCycleState(dayCycleElapsed)
    serverGlobals:SetAttribute("CurrentDayNumber", dayState.dayNumber)

    for _, player in ipairs(Players:GetPlayers()) do
        applyDayCycleAttributes(player, dayState)
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
            heldObject.Position = hrp.Position + getObjectHoldOffset(heldObject)
            heldObject.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
            heldObject.AssemblyAngularVelocity = Vector3.new(0, 0, 0)
        end
    end

    -- Animate placing objects
    for _, placement in ipairs(serverPlacementState:GetChildren()) do
        local objectRef = getPlacementObjectRef(placement)
        local object = objectRef and objectRef.Value or nil
        if not object or not object.Parent then
            placement:Destroy()
        else
        local elapsed = (placement:GetAttribute("Elapsed") or 0) + dt
        local duration = placement:GetAttribute("Duration") or DEFAULT_PLACE_DURATION
        local startPos = placement:GetAttribute("StartPos") or object.Position
        local targetPos = placement:GetAttribute("TargetPos") or object.Position
        local alpha = math.min(elapsed / duration, 1)
        -- Smooth ease-out
        local t = 1 - (1 - alpha) * (1 - alpha)
        object.Position = startPos:Lerp(targetPos, t)

        if alpha >= 1 then
            restoreObjectAfterPlacement(object)
            if object:GetAttribute("CarryKind") == "Block" then
                setBlockMetadata(object, object:GetAttribute("ItemColorName"))
                trySetBlockBond(object)
            end
            placement:Destroy()
        else
            placement:SetAttribute("Elapsed", elapsed)
        end
        end
    end

    -- Update player effects (delayed immobilization)
    for _, player in ipairs(Players:GetPlayers()) do
        local playerState = getPlayerState(player)
        if playerState and not GLOWBULB_IMMOBILIZATION_ENABLED then
            local humanoid = getHumanoid(player)
            if humanoid then
                humanoid.WalkSpeed = 16
                humanoid.JumpPower = 50
            end
            playerState:SetAttribute("ImmobilizeDelayRemaining", 0)
            playerState:SetAttribute("ImmobilizeDurationRemaining", 0)
            playerState:SetAttribute("ImmobilizeActive", false)
        end
        local effectActive = playerState and playerState:GetAttribute("ImmobilizeActive")
        local delayRemaining = playerState and playerState:GetAttribute("ImmobilizeDelayRemaining")
        local durationRemaining = playerState and playerState:GetAttribute("ImmobilizeDurationRemaining")
        if playerState and (effectActive or (delayRemaining and delayRemaining > 0) or (durationRemaining and durationRemaining > 0)) then
        local humanoid = nil
        local character = player.Character
        if character then
            humanoid = character:FindFirstChild("Humanoid")
        end
        if not humanoid then
            playerState:SetAttribute("ImmobilizeDelayRemaining", 0)
            playerState:SetAttribute("ImmobilizeDurationRemaining", 0)
            playerState:SetAttribute("ImmobilizeActive", false)
        else
            delayRemaining = math.max(0, (delayRemaining or 0) - dt)
            if delayRemaining <= 0 and not effectActive and (durationRemaining or 0) > 0 then
                -- Immobilize
                effectActive = true
                playerState:SetAttribute("ImmobilizeActive", true)
                humanoid.WalkSpeed = 0
                humanoid.JumpPower = 0
                humanoid:CancelMoveTo()
            end
            playerState:SetAttribute("ImmobilizeDelayRemaining", delayRemaining)
            if effectActive then
                durationRemaining = math.max(0, (durationRemaining or 0) - dt)
                playerState:SetAttribute("ImmobilizeDurationRemaining", durationRemaining)
                if durationRemaining <= 0 then
                    -- Restore
                    humanoid.WalkSpeed = 16
                    humanoid.JumpPower = 50
                    playerState:SetAttribute("ImmobilizeActive", false)
                    playerState:SetAttribute("ImmobilizeDelayRemaining", 0)
                    playerState:SetAttribute("ImmobilizeDurationRemaining", 0)
                end
            end
        end
        end
    end

    -- Update fauna AI
    updateCreatures(dt)

    -- Update flora: pulsing glow + regrowth
    for _, floraState in ipairs(serverFloraState:GetChildren()) do
        local partRef = floraState:FindFirstChild("Part")
        local part = partRef and partRef.Value or nil
        if not part or not part.Parent then
            floraState:Destroy()
        elseif floraState:GetAttribute("Harvested") then
            local regrowTimer = math.max(0, (floraState:GetAttribute("RegrowTimer") or 0) - dt)
            floraState:SetAttribute("RegrowTimer", regrowTimer)
            if regrowTimer <= 0 then
                floraState:SetAttribute("BiteStep", GLOWBULB_MAX_BITE_STEPS)
                floraState:SetAttribute("Harvested", false)
                applyGlowBulbVisuals(floraState, part)
            end
        else
            local age = (floraState:GetAttribute("Age") or 0) + dt
            floraState:SetAttribute("Age", age)
            applyGlowBulbVisuals(floraState, part)
            part.Transparency = 0.15 + 0.15 * math.sin(age * 2.0)
        end
    end

    for object, _ in pairs(trackedCarryables) do
        if not object.Parent then
            trackedCarryables[object] = nil
        else
            if isBlockPart(object) then
                trySetBlockBond(object)
            end
            if object.Position.Y < OUT_OF_BOUNDS_Y then
            destroyCarryableObject(object)
            end
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

print("=== Mesa Island ===")
