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

-- Run: lua5.1 tools/civ6_control/mod/world_religion_vote_test.lua
-- `CivvisWorldReligionTarget`: World Religion's A goes to our own faith, or
-- to the weakest one, never to the faith holding the most cities.
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

local CATHOLIC, SIKH, BUDDHA = "LOC_RELIGION_CATHOLICISM", "LOC_RELIGION_SIKHISM", "LOC_RELIGION_BUDDHISM"
local targets = { CATHOLIC, SIKH, BUDDHA }
-- civvis-20261030T105026Z t137: Catholicism held all six of ours.
local counts = { [CATHOLIC] = 19, [SIKH] = 7, [BUDDHA] = 2 }
check("our own faith when it is on the ballot",
    targets[CivvisWorldReligionTarget("WC_RES_WORLD_RELIGION", targets, 0, {}, counts, BUDDHA)], BUDDHA)
check("our faith even when it is the leader",
    targets[CivvisWorldReligionTarget("WC_RES_WORLD_RELIGION", targets, 0, {}, counts, CATHOLIC)], CATHOLIC)
check("no faith of ours: the weakest, never the leader",
    targets[CivvisWorldReligionTarget("WC_RES_WORLD_RELIGION", { CATHOLIC, SIKH }, 0, {}, counts, false)], SIKH)
check("a faith with no cities is weaker still",
    targets[CivvisWorldReligionTarget("WC_RES_WORLD_RELIGION", targets, 0, {},
        { [CATHOLIC] = 19, [SIKH] = 7 }, false)], BUDDHA)
check("ties go to the first listed",
    CivvisWorldReligionTarget("WC_RES_WORLD_RELIGION", { SIKH, BUDDHA }, 0, {},
        { [SIKH] = 3, [BUDDHA] = 3 }, false), 1)
check("another resolution, nothing",
    CivvisWorldReligionTarget("WC_RES_HERITAGE_ORG", targets, 0, {}, counts, BUDDHA), nil)
check("the kill switch",
    CivvisWorldReligionTarget("WC_RES_WORLD_RELIGION", targets, 0, { WorldReligionGuard = false }, counts, BUDDHA), nil)

-- The majority counter and our own faith, against stubbed players.
local function cityOf(rid)
    return { GetReligion = function() return { GetMajorityReligion = function() return rid end } end }
end
local function citiesOf(list)
    return { Members = function()
        local i = 0
        return function() i = i + 1; if list[i] then return i, list[i] end end
    end }
end
GameInfo = { Religions = { [3] = { Name = CATHOLIC }, [5] = { Name = SIKH }, [7] = { Name = BUDDHA } } }
local holdings = { [0] = { 3, 3, 3 }, [1] = { 3, 3 }, [2] = { 5, -1 } }
Players = {}
for id, list in pairs(holdings) do
    local cities = {}
    for _, rid in ipairs(list) do cities[#cities + 1] = cityOf(rid) end
    Players[id] = {
        GetCities = function() return citiesOf(cities) end,
        GetReligion = function() return { GetReligionTypeCreated = function()
            return id == 0 and 7 or (id == 1 and 3 or -1)
        end } end,
    }
end
PlayerManager = { GetAliveMajorIDs = function() return { 0, 1, 2 } end }
local tally = CivvisReligionMajorityCounts()
check("majority cities counted", tally[CATHOLIC], 5)
check("one Sikh city", tally[SIKH], 1)
check("a city with no majority is not counted", tally[BUDDHA], nil)
check("our own faith read from the host",
    targets[CivvisWorldReligionTarget("WC_RES_WORLD_RELIGION", targets, 0, {})], BUDDHA)
check("a seat with no faith takes the weakest from the host",
    targets[CivvisWorldReligionTarget("WC_RES_WORLD_RELIGION", { CATHOLIC, SIKH }, 2, {})], SIKH)

-- The ballot itself: a Diplomatic racer, World Religion on the ballot,
-- Catholicism the leader and Buddhism ours.
local tick = upvalue(CivvisQueue.onUiPulse, "tick")
local config = { Play = true, CivvisDecides = false, VictoryLane = "diplomatic" }
upvalue(tick, "cfg", config)
local hooks = {}
Events = setmetatable({ WorldCongressStage1 = { Add = function(fn) hooks.stage = fn end } },
    { __index = function() return stub() end })
LuaEvents = setmetatable({ CivvisCongressBallot = { Add = function(fn) hooks.popup = fn end } },
    { __index = function() return stub() end })
Automation = { Log = function() end }
local costs = { MaxVotes = 20 }
for k = 0, 20 do costs[k] = 2 * (k + 1) * k end
local wc = {
    GetVotesandFavorCost = function() return costs end,
    GetResolutions = function()
        return { Stage = 2147483647, { Type = "WC_RES_WORLD_RELIGION",
            TargetType = "ReligionType", PossibleTargets = { CATHOLIC, SIKH, BUDDHA } } }
    end,
}
Game = {
    GetLocalPlayer = function() return 0 end,
    GetCurrentGameTurn = function() return 137 end,
    GetCurrentTurnSegment = function() return "TURNSEG_WORLDCONGRESS_1" end,
    GetWorldCongress = function() return wc end,
}
DB = { MakeHash = function(name) return name end }
for id = 0, 2 do
    local p = Players[id]
    p.IsTurnActive = function() return false end
    p.GetFavor = function() return 300 end
    p.GetFavorEnteringCongress = function() return 300 end
    p.GetStats = function() return { GetDiplomaticVictoryPoints = function() return 5 end } end
    p.GetScore = function() return 800 + id end
end
GameInfo.Resolutions = { WC_RES_WORLD_RELIGION = { ResolutionType = "WC_RES_WORLD_RELIGION", Hash = 1 } }
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
vote(0)
check("one World Religion request", #requests, 1)
check("World Religion keeps option A", requests[1].option, 1)
check("on our own Buddhism, not the leader's Catholicism", requests[1].selection, 2)
check("the racer still buys its two", requests[1].votes, 2)

config.WorldReligionGuard = false
requests = {}
vote(0)
check("with the guard off, the first target as before", requests[1].selection, 0)

if failures > 0 then
    print(failures .. " FAILURES")
    os.exit(1)
end
print("all world religion checks passed")
