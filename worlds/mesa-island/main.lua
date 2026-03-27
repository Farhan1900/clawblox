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
local DAY_LENGTH_SECONDS = 60 * 60
local DAYLIGHT_START_HOUR = 6
local DAYLIGHT_END_HOUR = 18
local serverGlobals
-- Spawn points on open ground, verified clear of all mesa/rock footprints
local SPAWN_POINTS = {
    Vector3.new(25, 2, -20),     -- south of center hill
    Vector3.new(40, 2, -55),     -- south-east plain
    Vector3.new(-30, 2, 25),     -- west lowland
    Vector3.new(38, 2, 50),      -- east of NE ledge
    Vector3.new(-45, 2, -35),    -- north of SW plateau
    Vector3.new(80, 2, -25),     -- south of east ridge
    Vector3.new(-35, 2, 55),     -- south of NW stones
    Vector3.new(20, 2, 85),      -- north
    Vector3.new(25, 2, -90),     -- south cove
    Vector3.new(130, 2, 25),     -- east peninsula tip
}
local function getNextSpawnPosition()
    local nextSpawnIndex = (serverGlobals:GetAttribute("NextSpawnIndex") or 0) + 1
    serverGlobals:SetAttribute("NextSpawnIndex", nextSpawnIndex)
    local base = SPAWN_POINTS[((nextSpawnIndex - 1) % #SPAWN_POINTS) + 1]
    -- Add small random offset so players don't stack
    local ox = (math.random() - 0.5) * 8
    local oz = (math.random() - 0.5) * 8
    return Vector3.new(base.X + ox, base.Y, base.Z + oz)
end
local OUT_OF_BOUNDS_Y = -200

local trackedBlocks = {}

local blockColors = {
    Sand = Color3.fromRGB(214, 186, 128),
    Slate = Color3.fromRGB(148, 136, 124),
    Coral = Color3.fromRGB(205, 120, 100),
    Moss = Color3.fromRGB(95, 160, 110),
    Obsidian = Color3.fromRGB(55, 50, 60),
    Clay = Color3.fromRGB(180, 140, 105),
}

local blockPhysics = PhysicalProperties.new(
    0.7, -- density
    0.9, -- friction
    0.03, -- elasticity
    1.0, -- friction weight
    1.0 -- elasticity weight
)

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

if serverGlobals:GetAttribute("NextSpawnIndex") == nil then
    serverGlobals:SetAttribute("NextSpawnIndex", 0)
end
if serverGlobals:GetAttribute("NextPlacementId") == nil then
    serverGlobals:SetAttribute("NextPlacementId", 0)
end
if serverGlobals:GetAttribute("NextCreatureId") == nil then
    serverGlobals:SetAttribute("NextCreatureId", 0)
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
    local heldBlock = folder:FindFirstChild("HeldBlock")
    if not heldBlock then
        heldBlock = Instance.new("ObjectValue")
        heldBlock.Name = "HeldBlock"
        heldBlock.Parent = folder
    end
    return folder, heldBlock
end

local function getPlayerState(player)
    return serverPlayerState:FindFirstChild(playerStateKey(player))
end

local function getHeldBlockForPlayer(player)
    local folder = getPlayerState(player)
    if not folder then
        return nil
    end
    local heldBlock = folder:FindFirstChild("HeldBlock")
    if not heldBlock then
        return nil
    end
    return heldBlock.Value
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

    -- === Southwest cluster: tall spire formation ===
    { name = "SWPlateau",     size = {30, 6, 25},    pos = {-65, 3, -55} },
    { name = "SWTower",       size = {18, 12, 18},   pos = {-72, 6, -70} },
    { name = "SWSpire",       size = {10, 28, 10},   pos = {-76, 14, -80} },

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

            local lightness = math.floor(38 + rockRandom() * 25)
            local rock = Instance.new("Part")
            rock.Name = "Rock_" .. rockIndex
            rock.Size = Vector3.new(sx, sy, sz)
            rock.Position = Vector3.new(cluster.cx + ox, sy / 2 + 0.5, cluster.cz + oz)
            rock.Anchored = false
            rock.Color = Color3.fromRGB(lightness, lightness - 3, lightness - 8)
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

--------------------------------------------------------------------------------
-- FLORA
--------------------------------------------------------------------------------

local CONSUME_RANGE = 3
local GLOWBULB_REGROW_SECONDS = 180
local GLOWBULB_EFFECT_DELAY = 120   -- 2 minutes before immobilization kicks in
local GLOWBULB_EFFECT_DURATION = 300 -- 5 minutes immobilized

local GLOWBULB_COLOR = Color3.fromRGB(0, 220, 200)

local function createGlowBulb(name, position)
    local bulb = Instance.new("Part")
    bulb.Name = name
    bulb.Shape = Enum.PartType.Ball
    bulb.Size = Vector3.new(0.7, 0.7, 0.7)
    bulb.Position = position
    bulb.Anchored = true
    bulb.CanCollide = false
    bulb.Color = GLOWBULB_COLOR
    bulb.Material = Enum.Material.Neon
    bulb:SetAttribute("RenderRole", "Flora")
    bulb:SetAttribute("FloraSpecies", "GlowBulb")
    bulb:SetAttribute("IsEdible", true)
    bulb.Parent = Workspace

    local floraState = getOrCreateChild(serverFloraState, "Folder", name)
    local partRef = getOrCreateChild(floraState, "ObjectValue", "Part")
    partRef.Value = bulb
    floraState:SetAttribute("Species", "GlowBulb")
    floraState:SetAttribute("Age", math.random() * 6.28)
    floraState:SetAttribute("Harvested", false)
    floraState:SetAttribute("RegrowTimer", 0)
    floraState:SetAttribute("SpawnPos", position)
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
    -- Outside SWPlateau (edges at x=-80..-50, z=-68..-42)
    { cx = -48, cz = -55, n = 2 },  -- east edge
    { cx = -65, cz = -40, n = 3 },  -- north edge
    -- Outside EastRidge (edges at x=92..108, z=-20..40)
    { cx = 90, cz = -25, n = 2 },   -- south-west corner
    { cx = 90, cz = 45, n = 2 },    -- north-west corner
    -- Outside NW stones (NWStone1 edges at x=-57..-43, z=64..76)
    { cx = -40, cz = 70, n = 2 },   -- east edge
    { cx = -58, cz = 78, n = 3 },   -- north edge
}

if not serverGlobals:GetAttribute("GlowBulbsInitialized") then
    local glowBulbIndex = 0
    for _, cluster in ipairs(glowBulbClusters) do
        for j = 1, cluster.n do
            glowBulbIndex = glowBulbIndex + 1
            local ox = (math.random() - 0.5) * 6
            local oz = (math.random() - 0.5) * 6
            createGlowBulb(
                "GlowBulb_" .. glowBulbIndex,
                Vector3.new(cluster.cx + ox, 0.5, cluster.cz + oz)
            )
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

    floraState:SetAttribute("Harvested", true)
    floraState:SetAttribute("RegrowTimer", GLOWBULB_REGROW_SECONDS)
    plant.Transparency = 1
    plant.CanQuery = false

    local species = floraState:GetAttribute("Species") or "Unknown"
    player:SetAttribute("LastActionResult", "ConsumeSuccess:" .. species)

    if species == "GlowBulb" then
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
    },
    Skuttler = {
        shape = Enum.PartType.Block,
        size = Vector3.new(0.6, 0.55, 0.8),
        color = Color3.fromRGB(180, 100, 60),
        speed = 5.0,
        fleeRange = 5,
        hoverHeight = 0, -- walks on ground
        attractedTo = nil,
        attractRange = 0,
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

local function createCreature(speciesName, position)
    local def = speciesDefs[speciesName]
    if not def then
        warn("Unknown fauna species: " .. tostring(speciesName))
        return nil
    end

    local part = Instance.new("Part")
    part.Name = speciesName
    part.Shape = def.shape
    part.Size = def.size
    part.Position = Vector3.new(position.X, creatureGroundY(def), position.Z)
    part.Anchored = true
    part.CanCollide = false
    part.Color = def.color
    part.Material = Enum.Material.SmoothPlastic
    part:SetAttribute("RenderRole", "Fauna")
    part:SetAttribute("FaunaSpecies", speciesName)
    part.Parent = Workspace

    local creatureId = nextServerCounter("NextCreatureId")
    local creatureState = getOrCreateChild(serverCreatureState, "Folder", "Creature_" .. creatureId)
    local partRef = getOrCreateChild(creatureState, "ObjectValue", "Part")
    partRef.Value = part
    creatureState:SetAttribute("Species", speciesName)
    creatureState:SetAttribute("WanderDir", randomWanderDir())
    creatureState:SetAttribute("WanderTimer", math.random() * FAUNA_WANDER_INTERVAL)
    creatureState:SetAttribute("HomePos", position)
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
                local pos = part.Position
                local flatPos = Vector3.new(pos.X, 0, pos.Z)
                local moveDir = Vector3.new(0, 0, 0)
                local moveSpeed = def.speed
                local fleeing = false

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
                if not fleeing and def.attractedTo and def.attractRange > 0 then
                    local foodPos, foodDist = nearestFloraPosition(pos, def.attractedTo, def.attractRange)
                    if foodPos then
                        local toward = Vector3.new(foodPos.X, 0, foodPos.Z) - flatPos
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
                    if isOverLand(newX, newZ) then
                        part.Position = Vector3.new(newX, creatureGroundY(def), newZ)
                    else
                        local wanderDir = creatureState:GetAttribute("WanderDir")
                        if wanderDir then
                            creatureState:SetAttribute("WanderDir", -wanderDir)
                        end
                    end
                end
            end
        end
    end
end

-- Spawn creatures near their ecological niches
local creatureSpawns = {
    -- Orbweavers: hover near GlowBulb clusters (they feed on the glow)
    { species = "Orbweaver", pos = Vector3.new(15, 0, 18) },    -- CenterHill cluster
    { species = "Orbweaver", pos = Vector3.new(55, 0, 50) },    -- NE cluster
    { species = "Orbweaver", pos = Vector3.new(-60, 0, -50) },  -- SW cluster
    { species = "Orbweaver", pos = Vector3.new(-50, 0, 68) },   -- NW stones

    -- Skuttlers: roam open ground between formations
    { species = "Skuttler", pos = Vector3.new(35, 0, -30) },    -- open south-east
    { species = "Skuttler", pos = Vector3.new(-30, 0, 30) },    -- open west
    { species = "Skuttler", pos = Vector3.new(80, 0, -20) },    -- east ridge area
    { species = "Skuttler", pos = Vector3.new(-20, 0, -70) },   -- south open
    { species = "Skuttler", pos = Vector3.new(30, 0, 90) },     -- north

    -- Drifters: glide slowly across flat lowlands
    { species = "Drifter", pos = Vector3.new(40, 0, 30) },      -- central plain
    { species = "Drifter", pos = Vector3.new(-40, 0, 40) },     -- west lowland
    { species = "Drifter", pos = Vector3.new(10, 0, -50) },     -- south lowland
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

local function setBlockMetadata(block, colorName)
    block:SetAttribute("RenderRole", "PickupObject")
    block:SetAttribute("ItemColorName", colorName)
    block:SetAttribute("IsPickupObject", true)
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
    setBlockMetadata(block, colorName)
    block.Parent = Workspace
    trackedBlocks[block] = true
    return block
end

local function rebuildTrackedBlocks()
    trackedBlocks = {}
    for _, inst in ipairs(Workspace:GetDescendants()) do
        if inst:IsA("Part") and inst:GetAttribute("IsPickupObject") then
            trackedBlocks[inst] = true
        end
    end
end

-- Block clusters scattered across the island, each themed by dominant color
local blockClusters = {
    -- === Sand deposits (warm, near center & south) ===
    { cx = -12, cz = -6, baseY = 6, colors = {"Sand", "Sand", "Sand", "Clay", "Sand"} },
    { cx = 35, cz = -95, baseY = 6, colors = {"Sand", "Sand", "Clay", "Sand"} },
    { cx = 25, cz = -40, baseY = 6, colors = {"Sand", "Clay", "Sand"} },

    -- === Slate quarries (grey, near ridges & highlands) ===
    { cx = 92, cz = -20, baseY = 6, colors = {"Slate", "Slate", "Obsidian", "Slate", "Slate"} },
    { cx = -58, cz = -48, baseY = 6, colors = {"Slate", "Slate", "Slate", "Obsidian"} },
    { cx = 90, cz = 45, baseY = 6, colors = {"Slate", "Obsidian", "Slate"} },

    -- === Coral fragments (warm pink, near coast) ===
    { cx = 115, cz = 25, baseY = 6, colors = {"Coral", "Coral", "Coral", "Sand", "Coral"} },
    { cx = 40, cz = -100, baseY = 6, colors = {"Coral", "Coral", "Sand"} },
    { cx = -100, cz = -15, baseY = 6, colors = {"Coral", "Coral", "Coral"} },

    -- === Moss stones (green, near vegetation) ===
    { cx = -45, cz = 65, baseY = 6, colors = {"Moss", "Moss", "Moss", "Slate", "Moss"} },
    { cx = 52, cz = 48, baseY = 6, colors = {"Moss", "Moss", "Moss"} },
    { cx = -15, cz = 110, baseY = 6, colors = {"Moss", "Moss", "Clay", "Moss"} },

    -- === Obsidian shards (dark, rare, near SW spire) ===
    { cx = -80, cz = -72, baseY = 6, colors = {"Obsidian", "Obsidian", "Obsidian"} },
    { cx = 80, cz = 85, baseY = 6, colors = {"Obsidian", "Obsidian"} },

    -- === Clay deposits (earthy, scattered) ===
    { cx = -30, cz = 30, baseY = 6, colors = {"Clay", "Clay", "Sand", "Clay"} },
    { cx = 60, cz = -30, baseY = 6, colors = {"Clay", "Clay", "Clay"} },

    -- === Second wave: more deposits across the island ===
    -- Sand
    { cx = 10, cz = -70, baseY = 6, colors = {"Sand", "Sand", "Sand", "Sand"} },
    { cx = -80, cz = 15, baseY = 6, colors = {"Sand", "Clay", "Sand", "Sand"} },
    { cx = 70, cz = -60, baseY = 6, colors = {"Sand", "Sand", "Slate"} },
    -- Slate
    { cx = -40, cz = -80, baseY = 6, colors = {"Slate", "Slate", "Slate", "Slate"} },
    { cx = 50, cz = -10, baseY = 6, colors = {"Slate", "Obsidian", "Slate", "Slate"} },
    { cx = -90, cz = 50, baseY = 6, colors = {"Slate", "Slate", "Slate"} },
    -- Coral
    { cx = -70, cz = -90, baseY = 6, colors = {"Coral", "Coral", "Coral", "Coral"} },
    { cx = 100, cz = -50, baseY = 6, colors = {"Coral", "Sand", "Coral"} },
    { cx = -20, cz = -110, baseY = 6, colors = {"Coral", "Coral", "Coral"} },
    -- Moss
    { cx = 30, cz = 60, baseY = 6, colors = {"Moss", "Moss", "Moss"} },
    { cx = -60, cz = 30, baseY = 6, colors = {"Moss", "Clay", "Moss", "Moss"} },
    { cx = 70, cz = 70, baseY = 6, colors = {"Moss", "Moss", "Slate"} },
    -- Obsidian
    { cx = -50, cz = -100, baseY = 6, colors = {"Obsidian", "Obsidian", "Obsidian"} },
    { cx = 120, cz = -10, baseY = 6, colors = {"Obsidian", "Obsidian"} },
    -- Clay
    { cx = -110, cz = -40, baseY = 6, colors = {"Clay", "Clay", "Clay", "Clay"} },
    { cx = 20, cz = 110, baseY = 6, colors = {"Clay", "Sand", "Clay"} },
    { cx = 80, cz = 30, baseY = 6, colors = {"Clay", "Clay", "Coral"} },
}

if not serverGlobals:GetAttribute("BlocksInitialized") then
    local blockIndex = 0
    for _, cluster in ipairs(blockClusters) do
        for j, color in ipairs(cluster.colors) do
            blockIndex = blockIndex + 1
            local ox = (j - 1) * 0.7 - (#cluster.colors - 1) * 0.35
            local oz = ((j % 2) - 0.5) * 0.6
            local oy = (j - 1) * 0.5
            createBlock(
                "Block_" .. blockIndex,
                Vector3.new(cluster.cx + ox, cluster.baseY + oy, cluster.cz + oz),
                color
            )
        end
    end
    serverGlobals:SetAttribute("BlocksInitialized", true)
end

rebuildTrackedBlocks()

--------------------------------------------------------------------------------
-- PLAYER
--------------------------------------------------------------------------------

local function setPlayerBlockState(player, block)
    local _, heldBlock = getOrCreatePlayerState(player)
    heldBlock.Value = block
end

local function createPlacementState(block, startPos, targetPos, duration)
    local placementId = nextServerCounter("NextPlacementId")
    local folder = getOrCreateChild(serverPlacementState, "Folder", "Placement_" .. placementId)
    local blockRef = getOrCreateChild(folder, "ObjectValue", "Block")
    blockRef.Value = block
    folder:SetAttribute("StartPos", startPos)
    folder:SetAttribute("TargetPos", targetPos)
    folder:SetAttribute("Elapsed", 0)
    folder:SetAttribute("Duration", duration)
    return folder
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
    end

    if character then
        character:SetAttribute("ModelUrl", "asset://player.glb")

        local hrp = character:FindFirstChild("HumanoidRootPart")
        if hrp then
            hrp:SetAttribute("ModelYawOffsetDeg", 180)
            hrp.Position = getNextSpawnPosition()
        end
    end

    player:SetAttribute("LastActionResult", "Ready")
    applyDayCycleAttributes(player, getDayCycleState(serverGlobals:GetAttribute("DayCycleElapsedSeconds") or 0))
    playerState:SetAttribute("Initialized", true)
end

local function cleanupPlayer(player)
    local heldBlock = getHeldBlockForPlayer(player)
    if heldBlock then
        heldBlock:Destroy()
        setPlayerBlockState(player, nil)
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
        hrp.Position = getNextSpawnPosition()
        hrp.AssemblyLinearVelocity = Vector3.new(0, 0, 0)
        hrp.AssemblyAngularVelocity = Vector3.new(0, 0, 0)
    end
    if humanoid then
        humanoid:CancelMoveTo()
    end
end

local function releaseBlockReferences(block)
    for _, player in ipairs(Players:GetPlayers()) do
        local heldBlock = getHeldBlockForPlayer(player)
        if heldBlock == block then
            setPlayerBlockState(player, nil)
            player:SetAttribute("LastActionResult", "ResetAfterFall")
        end
    end

    for _, placement in ipairs(serverPlacementState:GetChildren()) do
        local blockRef = placement:FindFirstChild("Block")
        if blockRef and blockRef.Value == block then
            placement:Destroy()
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

local function pickupBlock(player, inputData)
    if getHeldBlockForPlayer(player) then
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

    local block = findNearestBlock(targetPosition, 1.5)
    if not block then
        player:SetAttribute("LastActionResult", "PickupFailed:NoBlock")
        return
    end

    block.CanCollide = false
    setPlayerBlockState(player, block)
    player:SetAttribute("LastActionResult", "PickupSuccess")
end

local function positionOccupied(player, targetPosition)
    local params = OverlapParams.new()
    params.FilterType = Enum.RaycastFilterType.Exclude
    params.FilterDescendantsInstances = { player.Character, getHeldBlockForPlayer(player) }

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
    local block = getHeldBlockForPlayer(player)
    if not block then
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

    if positionOccupied(player, targetPosition) then
        player:SetAttribute("LastActionResult", "PlaceFailed:Occupied")
        return
    end

    local startPos = block.Position
    block.Anchored = true
    block.CanCollide = false
    setPlayerBlockState(player, nil)

    createPlacementState(block, startPos, targetPosition, 0.3)

    player:SetAttribute("LastActionResult", "PlaceSuccess")
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
            pickupBlock(player, inputData)
        elseif inputType == "Place" then
            placeBlock(player, inputData)
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

    -- Update held blocks
    for _, player in ipairs(Players:GetPlayers()) do
        local heldBlock = getHeldBlockForPlayer(player)
        local hrp = getHumanoidRootPart(player)
        if hrp and heldBlock then
            heldBlock.Position = hrp.Position + Vector3.new(0, 3.5, 0)
        end
    end

    -- Animate placing blocks
    for _, placement in ipairs(serverPlacementState:GetChildren()) do
        local blockRef = placement:FindFirstChild("Block")
        local block = blockRef and blockRef.Value or nil
        if not block or not block.Parent then
            placement:Destroy()
        else
        local elapsed = (placement:GetAttribute("Elapsed") or 0) + dt
        local duration = placement:GetAttribute("Duration") or 0.3
        local startPos = placement:GetAttribute("StartPos") or block.Position
        local targetPos = placement:GetAttribute("TargetPos") or block.Position
        local alpha = math.min(elapsed / duration, 1)
        -- Smooth ease-out
        local t = 1 - (1 - alpha) * (1 - alpha)
        block.Position = startPos:Lerp(targetPos, t)

        if alpha >= 1 then
            block.Anchored = false
            block.CanCollide = true
            setBlockMetadata(block, block:GetAttribute("ItemColorName"))
            placement:Destroy()
        else
            placement:SetAttribute("Elapsed", elapsed)
        end
        end
    end

    -- Update player effects (delayed immobilization)
    for _, player in ipairs(Players:GetPlayers()) do
        local playerState = getPlayerState(player)
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
                floraState:SetAttribute("Harvested", false)
                part.Transparency = 0
                part.CanQuery = true
            end
        else
            local age = (floraState:GetAttribute("Age") or 0) + dt
            floraState:SetAttribute("Age", age)
            part.Transparency = 0.15 + 0.15 * math.sin(age * 2.0)
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

print("=== Mesa Island ===")
