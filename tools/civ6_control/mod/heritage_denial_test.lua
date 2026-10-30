-- ⚠ lupa's Lua 5.1.5 (the local runner) barriers the CLOSURE in
-- lua_setupvalue, not the shared UpVal; a full collection first leaves
-- nothing marked (see congress_claim_denial_test.lua). Harmless under the
-- real lua5.1 CI runs.
do
	local setupvalue = debug.setupvalue
	debug.setupvalue = function(...)
		collectgarbage("collect")
		return setupvalue(...)
	end
end

-- Run: lua5.1 tools/civ6_control/mod/heritage_denial_test.lua
-- `CivvisHeritageDenial`: a seat whose lane is not Culture votes Heritage
-- Organization B on the type the culture leader draws the most tourism from.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local function stub()
    return setmetatable({}, {
        __index = function() return stub() end,
        __call = function() return stub() end,
        __newindex = function() end,
    })
end
setmetatable(_G, { __index = function() return stub() end })
assert(pcall(assert(loadfile(here .. "/CivvisControlAgent.lua"))))
local function upvalue(fn, key, replacement)
    for i = 1, 100 do
        local name, value = debug.getupvalue(fn, i)
        if name == nil then break end
        if name == key then
            if replacement ~= nil then debug.setupvalue(fn, i, replacement) end
            return value
        end
    end
    error("missing upvalue " .. key)
end
local failures = 0
local function check(name, got, want)
    if got ~= want then
        failures = failures + 1
        print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
    else
        print("ok " .. name)
    end
end

local SCULPTURE = "LOC_GREAT_WORK_OBJECT_SCULPTURE_NAME"
local WRITING = "LOC_GREAT_WORK_OBJECT_WRITING_NAME"
local RELIC = "LOC_GREAT_WORK_OBJECT_RELIC_NAME"
local costs = { MaxVotes = 20 }
for k = 0, 20 do costs[k] = 2 * (k + 1) * k end

-- The pure piece, with the leader's tourism injected.
local targets = { SCULPTURE, WRITING, RELIC }
local cands = { { id = 1, culture = 3 }, { id = 2, culture = 14 }, { id = 3, culture = 10 } }
local rome = function(player)
    if player == 2 then return { [WRITING] = 12, [RELIC] = 24, [SCULPTURE] = 6 } end
    return { [SCULPTURE] = 30 }
end
local diplo = { VictoryLane = "diplomatic" }
local idx, votes, price, leader = CivvisHeritageDenial("WC_RES_HERITAGE_ORG", targets, cands, 0,
    300, costs, { WC_RES_HERITAGE_ORG = 3 }, diplo, rome)
check("the culture leader is the rival with the highest race", leader, 2)
check("B on the leader's biggest tourism type (3 relics outweigh 6 writings)", targets[idx], RELIC)
check("votes are last A plus one", votes, 4)
check("priced from the host table", price, costs[3])
idx, votes = CivvisHeritageDenial("WC_RES_HERITAGE_ORG", targets, cands, 0, 300, costs, nil, diplo, rome)
check("four votes when no A total is recorded", votes, 4)
idx, votes = CivvisHeritageDenial("WC_RES_HERITAGE_ORG", targets, cands, 0, 300, costs,
    { WC_RES_HERITAGE_ORG = 11 }, diplo, rome)
check("at most eight votes", votes, 8)
idx, votes, price = CivvisHeritageDenial("WC_RES_HERITAGE_ORG", targets, cands, 0, 20, costs,
    { WC_RES_HERITAGE_ORG = 3 }, diplo, rome)
check("a small bank buys what it can", votes, 3)
check("and pays for it", price, costs[2])
idx, votes, price = CivvisHeritageDenial("WC_RES_HERITAGE_ORG", targets, cands, 0, 0, costs,
    { WC_RES_HERITAGE_ORG = 3 }, diplo, rome)
check("an empty bank still names the type with the free vote", targets[idx], RELIC)
check("the free vote", votes, 1)
check("costs nothing", price, 0)
check("a Culture seat is left alone", CivvisHeritageDenial("WC_RES_HERITAGE_ORG", targets, cands,
    0, 300, costs, nil, { VictoryLane = "culture" }, rome), nil)
check("no lane, nothing", CivvisHeritageDenial("WC_RES_HERITAGE_ORG", targets, cands,
    0, 300, costs, nil, {}, rome), nil)
check("the kill switch", CivvisHeritageDenial("WC_RES_HERITAGE_ORG", targets, cands,
    0, 300, costs, nil, { VictoryLane = "diplomatic", HeritageDenial = false }, rome), nil)
check("another resolution, nothing", CivvisHeritageDenial("WC_RES_TRADE_TREATY", targets, cands,
    0, 300, costs, nil, diplo, rome), nil)
check("below the floor, nothing", CivvisHeritageDenial("WC_RES_HERITAGE_ORG", targets,
    { { id = 1, culture = 9 } }, 0, 300, costs, nil, diplo, rome), nil)
check("a leader with no great works on the ballot, nothing", CivvisHeritageDenial(
    "WC_RES_HERITAGE_ORG", { WRITING }, { { id = 1, culture = 50 } }, 0, 300, costs, nil, diplo,
    function() return { [RELIC] = 8 } end), nil)
check("our own race is not the leader's", CivvisHeritageDenial("WC_RES_HERITAGE_ORG", targets,
    { { id = 0, culture = 90 } }, 0, 300, costs, nil, diplo, rome), nil)

-- The racer buys no Heritage votes of its own.
local v, p = CivvisRacerResolutionVotes("WC_RES_HERITAGE_ORG", 300, costs, {})
check("racer Heritage keeps the free vote", v, 1); check("racer Heritage costs nothing", p, 0)
v = CivvisRacerResolutionVotes("WC_RES_HERITAGE_ORG", 300, costs, { HeritageDenial = false })
check("with the kill switch the racer buys as before", v, 4)

-- The tourism reader, against stubbed buildings: player 2 holds a museum
-- with two relics and an amphitheater with one writing.
local function rows(list)
    return setmetatable({}, { __call = function()
        local i = 0
        return function() i = i + 1; return list[i] end
    end })
end
GameInfo = {
    -- Two slot rows for one building must not count its works twice.
    Building_GreatWorks = rows({
        { BuildingType = "BUILDING_MUSEUM_ARTIFACT" },
        { BuildingType = "BUILDING_MUSEUM_ARTIFACT" },
        { BuildingType = "BUILDING_AMPHITHEATER" },
    }),
    Buildings = {
        BUILDING_MUSEUM_ARTIFACT = { Index = 10 },
        BUILDING_AMPHITHEATER = { Index = 11 },
    },
    GreatWorks = {
        [100] = { GreatWorkObjectType = "GREATWORKOBJECT_RELIC", Tourism = 8 },
        [101] = { GreatWorkObjectType = "GREATWORKOBJECT_WRITING", Tourism = 2 },
    },
    GreatWorkObjectTypes = {
        GREATWORKOBJECT_RELIC = { Name = RELIC },
        GREATWORKOBJECT_WRITING = { Name = WRITING },
    },
}
local slots = { [10] = { 1000, 1001 }, [11] = { 1002 } }
local kinds = { [1000] = 100, [1001] = 100, [1002] = 101 }
local city = { GetBuildings = function() return {
    HasBuilding = function(_, index) return slots[index] ~= nil end,
    GetNumGreatWorkSlots = function(_, index) return #(slots[index] or {}) end,
    GetGreatWorkInSlot = function(_, index, slot) return slots[index][slot + 1] end,
    GetGreatWorkTypeFromIndex = function(_, work) return kinds[work] end,
} end }
Players = { [2] = { GetCities = function() return {
    Members = function() local done = false
        return function() if done then return nil end; done = true; return 1, city end
    end,
} end } }
local tourism = CivvisHeritageTourismByType(2)
check("relic tourism summed once per building", tourism[RELIC], 16)
check("writing tourism", tourism[WRITING], 2)
check("a player with no cities reads empty", next(CivvisHeritageTourismByType(7)), nil)

-- The ballot itself: a Diplomatic seat, Heritage on the ballot, a rival
-- culture leader at 20% holding relics.
local tick = upvalue(CivvisQueue.onUiPulse, "tick")
local config = { Play = true, CivvisDecides = false, VictoryLane = "diplomatic" }
upvalue(tick, "cfg", config)
local hooks = {}
Events = setmetatable({ WorldCongressStage1 = { Add = function(fn) hooks.stage = fn end } },
    { __index = function() return stub() end })
LuaEvents = setmetatable({ CivvisCongressBallot = { Add = function(fn) hooks.popup = fn end } },
    { __index = function() return stub() end })
Automation = { Log = function() end }
local wc = {
    GetVotesandFavorCost = function() return costs end,
    GetResolutions = function()
        return { Stage = 2147483647, { Type = "WC_RES_HERITAGE_ORG",
            TargetType = "GreatWorkObjectType", PossibleTargets = { SCULPTURE, WRITING, RELIC } } }
    end,
}
Game = {
    GetLocalPlayer = function() return 0 end,
    GetCurrentGameTurn = function() return 139 end,
    GetCurrentTurnSegment = function() return "TURNSEG_WORLDCONGRESS_1" end,
    GetWorldCongress = function() return wc end,
}
DB = { MakeHash = function(name) return name end }
local culture = { [0] = { 45, 0 }, [1] = { 56, 3 }, [2] = { 100, 20 }, [3] = { 90, 12 } }
local function player(id)
    return {
        IsTurnActive = function() return false end,
        GetFavor = function() return 300 end,
        GetFavorEnteringCongress = function() return 300 end,
        GetStats = function() return { GetDiplomaticVictoryPoints = function() return 6 end } end,
        GetScore = function() return 800 + id end,
        GetCulture = function() return {
            GetStaycationers = function() return culture[id][1] end,
            GetTouristsTo = function() return culture[id][2] end,
        } end,
        GetCities = function() return {
            Members = function() local done = id ~= 2
                return function() if done then return nil end; done = true; return 1, city end
            end,
        } end,
    }
end
Players = {}
for id = 0, 3 do Players[id] = player(id) end
PlayerManager = { GetAliveMajorIDs = function() return { 0, 1, 2, 3 } end }
GameInfo.Resolutions = { WC_RES_HERITAGE_ORG = { ResolutionType = "WC_RES_HERITAGE_ORG", Hash = 1 } }
PlayerOperations = {
    PARAM_RESOLUTION_TYPE = "type", PARAM_WORLD_CONGRESS_VOTES = "votes",
    PARAM_RESOLUTION_OPTION = "option", PARAM_RESOLUTION_SELECTION = "selection",
    WORLD_CONGRESS_RESOLUTION_VOTE = "vote", WORLD_CONGRESS_SUBMIT_TURN = "submit",
}
local requests = {}
UI = { RequestPlayerOperation = function(pid, operation, params)
    assert(pid == 0)
    if operation == "vote" then requests[#requests + 1] = params end
end }
tick()
local cast = upvalue(hooks.popup, "castBallot")
local vote = upvalue(cast, "voteWorldCongress")
local tally = upvalue(vote, "envoyTally")
tally.wc_last_a = { WC_RES_HERITAGE_ORG = 3 }
local _, spent = vote(0)
check("one Heritage vote request", #requests, 1)
check("Heritage votes B", requests[1].option, 2)
check("on the leader's relics", requests[1].selection, 2)
check("with last A plus one", requests[1].votes, 4)
check("and pays for them", spent, costs[3])

-- A Culture seat votes the type that widens its lead
-- (`CivvisHeritageCultureSeat`): we hold no Great Works and the leader holds
-- relics, so B on Relic, with the denial's vote count.
config.VictoryLane = "culture"
requests = {}
_, spent = vote(0)
check("culture seat votes B on the leader's lead", requests[1].option, 2)
check("culture seat names the relics", requests[1].selection, 2)
check("culture seat votes last A plus one", requests[1].votes, 4)

-- With its kill switch a Culture seat keeps today's ballot: A on the first
-- target, free.
config.HeritageCultureSeat = false
requests = {}
_, spent = vote(0)
check("culture seat votes A", requests[1].option, 1)
check("culture seat first target", requests[1].selection, 0)
check("culture seat free vote", requests[1].votes, 1)
config.HeritageCultureSeat = nil

if failures > 0 then
    print(failures .. " FAILURES")
    os.exit(1)
end
print("all heritage denial checks passed")
