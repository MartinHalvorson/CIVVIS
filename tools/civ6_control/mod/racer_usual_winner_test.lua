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

-- Run: lua5.1 tools/civ6_control/mod/racer_usual_winner_test.lua
-- `CivvisRacerUsualWinner`: on Military Advisory, Patronage and World Ideology
-- a Diplomatic racer names the target the field usually picks (Melee, the
-- Great Scientist, Fascism) instead of `PossibleTargets[1]`, and the
-- production ballot sends that selection with the racer's own vote count.
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
local tick = upvalue(CivvisQueue.onUiPulse, "tick")
local config = { Play = true, CivvisDecides = false, CounterResolutions = false,
    VictoryLane = "diplomatic" }
upvalue(tick, "cfg", config)
local hooks = {}
Events = setmetatable({ WorldCongressStage1 = { Add = function(fn) hooks.stage = fn end } },
    { __index = function() return stub() end })
LuaEvents = setmetatable({ CivvisCongressBallot = { Add = function(fn) hooks.popup = fn end } },
    { __index = function() return stub() end })
Automation = { Log = function() end }

-- The offered targets as the live `wc_ballot_context` rows list them
-- (civvis-20261113T18-19: unlocalized names, in this order).
local CLASSES = { "LOC_PROMOTION_CLASS_RECON_NAME", "LOC_PROMOTION_CLASS_MELEE_NAME",
    "LOC_PROMOTION_CLASS_RANGED_NAME", "LOC_PROMOTION_CLASS_SIEGE_NAME" }
local PEOPLE = { "LOC_GREAT_PERSON_CLASS_GENERAL_NAME", "LOC_GREAT_PERSON_CLASS_ADMIRAL_NAME",
    "LOC_GREAT_PERSON_CLASS_ENGINEER_NAME", "LOC_GREAT_PERSON_CLASS_MERCHANT_NAME",
    "LOC_GREAT_PERSON_CLASS_SCIENTIST_NAME", "LOC_GREAT_PERSON_CLASS_WRITER_NAME" }
local GOVERNMENTS = { "LOC_GOVERNMENT_AUTOCRACY_NAME", "LOC_GOVERNMENT_OLIGARCHY_NAME",
    "LOC_GOVERNMENT_CLASSICAL_REPUBLIC_NAME", "LOC_GOVERNMENT_MONARCHY_NAME",
    "LOC_GOVERNMENT_THEOCRACY_NAME", "LOC_GOVERNMENT_MERCHANT_REPUBLIC_NAME",
    "LOC_GOVERNMENT_FASCISM_NAME", "LOC_GOVERNMENT_COMMUNISM_NAME" }
local DISTRICTS = { "LOC_DISTRICT_CITY_CENTER_NAME", "LOC_DISTRICT_CAMPUS_NAME" }

local failures = 0
local function check(name, got, want)
    if got ~= want then
        failures = failures + 1
        print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
    else
        print("ok " .. name)
    end
end
local function pick(rtype, targets, cfg, option, selection)
    local o, s = CivvisRacerUsualWinner(rtype, targets, cfg, option or 1, selection or 1)
    return tostring(o) .. "/" .. tostring(s)
end

-- The pure choice.
local racer = { VictoryLane = "diplomatic" }
check("military advisory names Melee", pick("WC_RES_MILITARY_ADVISORY", CLASSES, racer), "1/2")
check("patronage names the Great Scientist", pick("WC_RES_PATRONAGE", PEOPLE, racer), "1/5")
check("world ideology names Fascism", pick("WC_RES_WORLD_IDEOLOGY", GOVERNMENTS, racer), "1/7")
check("it is option A even from a B", pick("WC_RES_MILITARY_ADVISORY", CLASSES, racer, 2, 3), "1/2")
check("a target not offered keeps the ballot's choice",
    pick("WC_RES_MILITARY_ADVISORY", { "LOC_PROMOTION_CLASS_RECON_NAME",
        "LOC_PROMOTION_CLASS_RANGED_NAME" }, racer), "1/1")
check("no targets keeps the ballot's choice", pick("WC_RES_PATRONAGE", nil, racer, 2, 4), "2/4")
check("another resolution keeps the ballot's choice",
    pick("WC_RES_URBAN_DEVELOPMENT", DISTRICTS, racer), "1/1")
check("another resolution keeps a B too", pick("WC_RES_MERCENARY_COMPANIES", DISTRICTS, racer, 2, 1), "2/1")
check("a Science seat keeps the first target",
    pick("WC_RES_MILITARY_ADVISORY", CLASSES, { VictoryLane = "science" }), "1/1")
-- `science-seat-races-the-congress`: a Science seat racing the Congress is a
-- racer here too; without the tag, or on another lane with it, it is not.
check("a Science seat racing the Congress names Melee", pick("WC_RES_MILITARY_ADVISORY", CLASSES,
    { VictoryLane = "science", ScienceCongressRacer = true }), "1/2")
check("a Culture seat with the tag keeps the first target", pick("WC_RES_MILITARY_ADVISORY", CLASSES,
    { VictoryLane = "culture", ScienceCongressRacer = true }), "1/1")
check("no lane keeps the first target", pick("WC_RES_WORLD_IDEOLOGY", GOVERNMENTS, {}), "1/1")
check("no config keeps the first target", pick("WC_RES_WORLD_IDEOLOGY", GOVERNMENTS, nil), "1/1")
check("the racer switched off keeps the first target", pick("WC_RES_PATRONAGE", PEOPLE,
    { VictoryLane = "diplomatic", DiploRacer = false }), "1/1")
check("its own switch keeps the first target", pick("WC_RES_PATRONAGE", PEOPLE,
    { VictoryLane = "diplomatic", RacerUsualWinner = false }), "1/1")

-- The production ballot.
local bank = 300
local costs = { MaxVotes = 20 }
for k = 0, 20 do costs[k] = 2 * (k + 1) * k end
local SESSION = {
    { "WC_RES_MILITARY_ADVISORY", "UnitPromotionClass", CLASSES },
    { "WC_RES_PATRONAGE", "GreatPersonClassType", PEOPLE },
    { "WC_RES_URBAN_DEVELOPMENT", "DistrictType", DISTRICTS },
    { "WC_RES_WORLD_IDEOLOGY", "GovernmentType", GOVERNMENTS },
}
local wc = {
    GetVotesandFavorCost = function() return costs end,
    GetResolutions = function()
        local list = { Stage = 2147483647 }
        for i, row in ipairs(SESSION) do
            list[i] = { Type = row[1], TargetType = row[2], PossibleTargets = row[3] }
        end
        return list
    end,
}
Game = {
    GetLocalPlayer = function() return 0 end,
    GetCurrentGameTurn = function() return 143 end,
    GetCurrentTurnSegment = function() return "TURNSEG_WORLDCONGRESS_1" end,
    GetWorldCongress = function() return wc end,
}
DB = { MakeHash = function(name) return name end }
Players = { [0] = {
    IsTurnActive = function() return false end,
    GetFavor = function() return bank end,
    GetFavorEnteringCongress = function() return bank end,
    GetStats = function() return {
        GetDiplomaticVictoryPoints = function() return 6 end,
    } end,
    GetScore = function() return 900 end,
} }
for id = 1, 3 do
    local player = id
    Players[id] = {
        GetStats = function() return {
            GetDiplomaticVictoryPoints = function() return player == 3 and 8 or 4 end,
        } end,
        GetScore = function() return player == 3 and 1000 or 800 end,
    }
end
PlayerManager = { GetAliveMajorIDs = function() return { 0, 1, 2, 3 } end }
GameInfo = { Resolutions = {} }
for index, row in ipairs(SESSION) do
    GameInfo.Resolutions[row[1]] = { ResolutionType = row[1], Hash = row[1] }
end
PlayerOperations = {
    PARAM_RESOLUTION_TYPE = "type", PARAM_WORLD_CONGRESS_VOTES = "votes",
    PARAM_RESOLUTION_OPTION = "option", PARAM_RESOLUTION_SELECTION = "selection",
    WORLD_CONGRESS_RESOLUTION_VOTE = "vote", WORLD_CONGRESS_SUBMIT_TURN = "submit",
}
local requests = {}
UI = { RequestPlayerOperation = function(pid, operation, params)
    assert(pid == 0)
    if operation == "vote" then requests[params.type] = params end
end }
tick()
local cast = upvalue(hooks.popup, "castBallot")
local vote = upvalue(cast, "voteWorldCongress")
local function ballot(lane, switch)
    requests = {}
    config.VictoryLane = lane
    config.RacerUsualWinner = switch
    vote(0)
    local out = {}
    for _, row in ipairs(SESSION) do
        local r = requests[row[1]]
        out[row[1]] = r and (r.option .. "/" .. r.selection .. "x" .. r.votes) or "none"
    end
    return out
end

-- A Diplomatic racer: `selection` is zero-based, so Melee is 1, the Scientist
-- 4 and Fascism 6. Military Advisory's three votes cost 12 Favor and are
-- bought; Patronage's ten and World Ideology's fifteen are over the 60 cap,
-- so those two carry the free vote. Urban Development is untouched.
local racerBallot = ballot("diplomatic", nil)
check("racer: military advisory is A on Melee with three votes",
    racerBallot.WC_RES_MILITARY_ADVISORY, "1/1x3")
check("racer: patronage is A on the Scientist with the free vote",
    racerBallot.WC_RES_PATRONAGE, "1/4x1")
check("racer: world ideology is A on Fascism with the free vote",
    racerBallot.WC_RES_WORLD_IDEOLOGY, "1/6x1")
check("racer: urban development keeps the first target",
    racerBallot.WC_RES_URBAN_DEVELOPMENT, "1/0x3")

-- Switched off, the racer votes as it did: the first target each time.
local offBallot = ballot("diplomatic", false)
check("switch off: military advisory on the first target",
    offBallot.WC_RES_MILITARY_ADVISORY, "1/0x3")
check("switch off: patronage on the first target", offBallot.WC_RES_PATRONAGE, "1/0x1")
check("switch off: world ideology on the first target", offBallot.WC_RES_WORLD_IDEOLOGY, "1/0x1")

-- A Science seat is not a racer: the first target, the free vote.
local scienceBallot = ballot("science", nil)
check("science: military advisory on the first target",
    scienceBallot.WC_RES_MILITARY_ADVISORY:sub(1, 3), "1/0")
check("science: patronage on the first target", scienceBallot.WC_RES_PATRONAGE:sub(1, 3), "1/0")
check("science: world ideology on the first target",
    scienceBallot.WC_RES_WORLD_IDEOLOGY:sub(1, 3), "1/0")

if failures > 0 then
    print(string.format("%d FAILED", failures))
    os.exit(1)
end
print("all racer usual-winner checks passed")
