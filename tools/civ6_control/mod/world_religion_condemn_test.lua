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

-- Run: lua5.1 tools/civ6_control/mod/world_religion_condemn_test.lua
-- `CivvisWorldReligionCondemn`: a seat with no faith votes World Religion B on
-- the leading faith when the bank carries one vote more than the A total.
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
local function upvalue(fn, key)
    for i = 1, 100 do
        local name, value = debug.getupvalue(fn, i)
        if name == nil then break end
        if name == key then return value end
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

local CATHOLIC, ORTHODOX, TAOIST = "LOC_RELIGION_CATHOLICISM", "LOC_RELIGION_ORTHODOXY", "LOC_RELIGION_TAOISM"
local targets = { CATHOLIC, ORTHODOX, TAOIST }
local counts = { [CATHOLIC] = 3, [ORTHODOX] = 19, [TAOIST] = 7 }
local costs = { MaxVotes = 20 }
for k = 0, 20 do costs[k] = 2 * (k + 1) * k end
local RTYPE = "WC_RES_WORLD_RELIGION"
local on = { WorldReligionCondemns = true }
local vote = CivvisWorldReligionCondemn

local idx, votes, price = vote(RTYPE, targets, 0, 100, costs, nil, on, counts, false)
check("the leading faith", targets[idx], ORTHODOX)
check("one more than the five rival free votes", votes, 6)
check("six votes cost 60 Favor", price, 60)

idx, votes, price = vote(RTYPE, targets, 0, 200, costs, { [RTYPE] = 7 }, on, counts, false)
check("sized from last session's A total", votes, 8)
check("eight votes cost 112 Favor", price, 112)
idx = vote(RTYPE, targets, 0, 100, costs, { [RTYPE] = 7 }, on, counts, false)
check("too dear returns nothing", idx, nil)
check("Favor below the price stands aside", vote(RTYPE, targets, 0, 59, costs, nil, on, counts, false), nil)
check("exactly the price is enough", targets[(vote(RTYPE, targets, 0, 60, costs, nil, on, counts, false))], ORTHODOX)

check("a founder keeps its option A", vote(RTYPE, targets, 0, 400, costs, nil, on, counts, true), nil)
check("the lease is required", vote(RTYPE, targets, 0, 400, costs, nil, {}, counts, false), nil)
check("another resolution, nothing", vote("WC_RES_HERITAGE_ORG", targets, 0, 400, costs, nil, on, counts, false), nil)
check("a Diplomatic racer keeps its vote", vote(RTYPE, targets, 0, 400, costs, nil,
    { WorldReligionCondemns = true, VictoryLane = "diplomatic" }, counts, false), nil)
check("no faith holding a city, nothing", vote(RTYPE, targets, 0, 400, costs, nil, on, {}, false), nil)
check("the vote cap reads its setting", vote(RTYPE, targets, 0, 400, costs, nil,
    { WorldReligionCondemns = true, WorldReligionCondemnMaxVotes = 5 }, counts, false), nil)

-- The ballot itself: a Science seat with no faith, 100 Favor, Orthodoxy leading.
local function cityOf(rid)
    return { GetReligion = function() return { GetMajorityReligion = function() return rid end } end }
end
local function citiesOf(list)
    return { Members = function()
        local i = 0
        return function() i = i + 1; if list[i] then return i, list[i] end end
    end }
end
GameInfo = {
    Religions = { [3] = { Name = CATHOLIC }, [5] = { Name = ORTHODOX }, [7] = { Name = TAOIST } },
    Resolutions = { WC_RES_WORLD_RELIGION = { ResolutionType = "WC_RES_WORLD_RELIGION", Hash = 1 } },
}
local holdings = { [0] = { 5, 5, 5 }, [1] = { 5, 5 }, [2] = { 3, -1 } }
local bank = 100
Players = {}
for id, list in pairs(holdings) do
    local cities = {}
    for _, rid in ipairs(list) do cities[#cities + 1] = cityOf(rid) end
    Players[id] = {
        GetCities = function() return citiesOf(cities) end,
        GetReligion = function() return { GetReligionTypeCreated = function()
            return id == 1 and 5 or -1
        end } end,
        IsTurnActive = function() return false end,
        GetFavor = function() return bank end,
        GetFavorEnteringCongress = function() return bank end,
        GetScore = function() return 800 + id end,
        GetStats = function() return { GetDiplomaticVictoryPoints = function() return 5 end } end,
    }
end
PlayerManager = { GetAliveMajorIDs = function() return { 0, 1, 2 } end }
local tick = upvalue(CivvisQueue.onUiPulse, "tick")
local config = { Play = true, CivvisDecides = false, VictoryLane = "science" }
for i = 1, 100 do
    local name = debug.getupvalue(tick, i)
    if name == nil then break end
    if name == "cfg" then debug.setupvalue(tick, i, config) end
end
local hooks = {}
Events = setmetatable({ WorldCongressStage1 = { Add = function(fn) hooks.stage = fn end } },
    { __index = function() return stub() end })
LuaEvents = setmetatable({ CivvisCongressBallot = { Add = function(fn) hooks.popup = fn end } },
    { __index = function() return stub() end })
Automation = { Log = function() end }
local wc = {
    GetVotesandFavorCost = function() return costs end,
    GetResolutions = function()
        return { Stage = 2147483647, { Type = RTYPE, TargetType = "ReligionType",
            PossibleTargets = targets } }
    end,
}
Game = {
    GetLocalPlayer = function() return 0 end,
    GetCurrentGameTurn = function() return 72 end,
    GetCurrentTurnSegment = function() return "TURNSEG_WORLDCONGRESS_1" end,
    GetWorldCongress = function() return wc end,
}
DB = { MakeHash = function(name) return name end }
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
local ballot = upvalue(cast, "voteWorldCongress")

config.WorldReligionCondemns = true
ballot(0)
check("one World Religion request", #requests, 1)
check("the ballot takes option B", requests[1].option, 2)
check("on Orthodoxy", targets[requests[1].selection + 1], ORTHODOX)
check("with six votes", requests[1].votes, 6)

bank = 10
requests = {}
ballot(0)
check("a short bank keeps option A", requests[1].option, 1)
check("on the weakest faith, as before", targets[requests[1].selection + 1], TAOIST)

config.WorldReligionCondemns = nil
bank = 100
requests = {}
ballot(0)
check("without the lease, option A", requests[1].option, 1)

if failures > 0 then
    error(failures .. " failure(s)")
end
print("world_religion_condemn_test OK")
