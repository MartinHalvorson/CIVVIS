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

-- Run: lua5.1 tools/civ6_control/mod/racer_match_point_test.lua
-- `CivvisRacerMatchPointVotes`: from 18 points a Diplomatic racer braces with
-- the free vote and may spend its whole Favor bank on the session's other
-- resolutions. The pure allocation, then the production ballot.
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
local bank, leaderPoints, ourPoints = 300, 8, 6
local costs = { MaxVotes = 20 }
for k = 0, 20 do costs[k] = 2 * (k + 1) * k end
local session = { "WC_RES_DIPLOVICTORY", "WC_RES_BORDER_CONTROL", "WC_RES_PUBLIC_RELATIONS" }
local wc = {
    GetVotesandFavorCost = function() return costs end,
    GetResolutions = function()
        local list = { Stage = 2147483647 }
        for i, name in ipairs(session) do
            list[i] = { Type = name, TargetType = "PlayerType", PossibleTargets = { 0, 1, 2, 3 } }
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
        GetDiplomaticVictoryPoints = function() return ourPoints end,
    } end,
    GetScore = function() return 900 end,
} }
for id = 1, 3 do
    local player = id
    Players[id] = {
        GetStats = function() return {
            GetDiplomaticVictoryPoints = function() return player == 3 and leaderPoints or 4 end,
        } end,
        GetScore = function() return player == 3 and 1000 or 800 end,
    }
end
PlayerManager = { GetAliveMajorIDs = function() return { 0, 1, 2, 3 } end }
GameInfo = { Resolutions = {} }
for index, name in ipairs({ "WC_RES_DIPLOVICTORY", "WC_RES_BORDER_CONTROL",
        "WC_RES_PUBLIC_RELATIONS", "WC_RES_TRADE_TREATY", "WC_RES_MIGRATION_TREATY",
        "WC_RES_PATRONAGE" }) do
    GameInfo.Resolutions[name] = { ResolutionType = name, Hash = index }
end
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
local failures = 0
local function check(name, got, want)
    if got ~= want then
        failures = failures + 1
        print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
    else
        print("ok " .. name)
    end
end
-- `requests` follow the ballot's order: the Diplomatic Victory resolution
-- first, then the others by name.
local function run(favor, leader, ours, block)
    requests = {}
    bank, leaderPoints, ourPoints = favor, leader, ours
    tally.wc_rival_block = block
    tally.wc_review_favor = nil
    local _, spent, _, _, _, _, mode = vote(0)
    return spent, mode
end


local function mp(rtype, entries, favor, points, cfg, want, price)
    local config = cfg or { VictoryLane = "diplomatic" }
    local v, p = CivvisRacerMatchPointVotes(rtype, entries, rtype, favor, costs, config, points,
        want or 1, price or 0)
    return v .. "x" .. p
end
local TREATY = { "WC_RES_DIPLOVICTORY", "WC_RES_MIGRATION_TREATY", "WC_RES_PUBLIC_RELATIONS" }

-- The pure allocation. Prices on this table are 2k(k-1): 6 votes 60, 8 votes
-- 112, 10 votes 180, 12 votes 264.
-- civvis-20261113T195128Z t192: 19 points, 209 Favor, a Migration Treaty and
-- (here) a Public Relations after it. The treaty keeps 12 back for Public
-- Relations' three votes and takes 10 votes (180) of the other 197.
check("the treaty takes the bank less the later count", mp("WC_RES_MIGRATION_TREATY", TREATY, 209, 19), "10x180")
check("the later resolution takes what is left", mp("WC_RES_PUBLIC_RELATIONS", TREATY, 29, 19), "4x24")
check("alone it is capped at twice its count",
    mp("WC_RES_MIGRATION_TREATY", { "WC_RES_MIGRATION_TREATY" }, 1000, 19), "12x264")
check("a cheap resolution stops at twice its count",
    mp("WC_RES_BORDER_CONTROL", { "WC_RES_BORDER_CONTROL" }, 300, 18), "4x24")
check("a bank short of both funds this count first", mp("WC_RES_MIGRATION_TREATY", TREATY, 65, 19), "6x60")
check("a bank short of the count buys what it can", mp("WC_RES_MIGRATION_TREATY", TREATY, 50, 19), "5x40")
check("a bank that buys nothing keeps the free vote", mp("WC_RES_MIGRATION_TREATY", TREATY, 3, 19), "1x0")
check("a dear resolution is bought at match point",
    mp("WC_RES_PATRONAGE", { "WC_RES_DIPLOVICTORY", "WC_RES_PATRONAGE" }, 400, 19), "14x364")
check("a dear later resolution reserves nothing",
    mp("WC_RES_MIGRATION_TREATY", { "WC_RES_MIGRATION_TREATY", "WC_RES_PATRONAGE" }, 209, 19), "10x180")
check("the Diplomatic Victory resolution reserves nothing",
    mp("WC_RES_MIGRATION_TREATY", { "WC_RES_MIGRATION_TREATY", "WC_RES_DIPLOVICTORY" }, 209, 19), "10x180")
check("entries may be the ballot's rows", (function()
    local rows = { { rtype = "WC_RES_MIGRATION_TREATY" }, { rtype = "WC_RES_PUBLIC_RELATIONS" } }
    local v, p = CivvisRacerMatchPointVotes("WC_RES_MIGRATION_TREATY", rows, rows[1], 209, costs,
        { VictoryLane = "diplomatic" }, 19, 1, 0)
    return v .. "x" .. p
end)(), "10x180")
-- Where it does not apply the ballot's own answer comes back.
check("below match point: unchanged", mp("WC_RES_MIGRATION_TREATY", TREATY, 209, 17, nil, 6, 60), "6x60")
check("a raised match point is honoured",
    mp("WC_RES_MIGRATION_TREATY", TREATY, 209, 18, { VictoryLane = "diplomatic", DiploVictoryMatchPoint = 19 }, 6, 60), "6x60")
check("a Science seat: unchanged",
    mp("WC_RES_MIGRATION_TREATY", TREATY, 209, 19, { VictoryLane = "science", ScienceCongressRacer = true }, 1, 0), "1x0")
check("switched off: unchanged",
    mp("WC_RES_MIGRATION_TREATY", TREATY, 209, 19, { VictoryLane = "diplomatic", RacerMatchPointBank = false }, 6, 60), "6x60")
check("the Heritage vote stays the denial's",
    mp("WC_RES_HERITAGE_ORG", { "WC_RES_HERITAGE_ORG" }, 300, 19), "1x0")
check("never fewer than the ballot already bought",
    mp("WC_RES_MIGRATION_TREATY", TREATY, 50, 19, nil, 6, 60), "6x60")
check("no points read: unchanged", mp("WC_RES_MIGRATION_TREATY", TREATY, 209, nil, nil, 6, 60), "6x60")

-- The production ballot. `run(favor, rival leader's points, ours, last block)`.
session = { "WC_RES_DIPLOVICTORY", "WC_RES_MIGRATION_TREATY", "WC_RES_PUBLIC_RELATIONS" }
local spent, mode = run(209, 14, 19, 9)
check("19 points: the ballot braces", mode, "brace")
check("19 points: the brace is the free vote", requests[1].votes, 1)
check("19 points: the Migration Treaty takes ten votes", requests[2].votes, 10)
check("19 points: as A on us", requests[2].option .. "/" .. requests[2].selection, "1/0")
check("19 points: Public Relations takes four", requests[3].votes, 4)
check("19 points: 204 of the 209 Favor is spent", spent, 180 + 24)

-- One point lower the ordinary counts stand (six and three), whatever the
-- Diplomatic Victory resolution took.
spent, mode = run(209, 14, 17, 9)
check("17 points: the treaty keeps its six or the free vote",
    requests[2].votes == 6 or requests[2].votes == 1, true)
check("17 points: Public Relations is never above three", requests[3].votes <= 3, true)

-- Switched off at 19 points: the ordinary counts.
config.RacerMatchPointBank = false
spent, mode = run(209, 14, 19, 9)
check("switch off: the treaty buys six", requests[2].votes, 6)
check("switch off: Public Relations buys three", requests[3].votes, 3)
check("switch off: 72 Favor", spent, 60 + 12)
config.RacerMatchPointBank = nil

-- A small bank at match point: five votes for the treaty (40 of 50), and the
-- 10 left buy Public Relations two.
spent, mode = run(50, 14, 19, 9)
check("small bank: five for the treaty", requests[2].votes, 5)
check("small bank: two for Public Relations", requests[3].votes, 2)
check("small bank spend", spent, 40 + 4)

if failures > 0 then
    print(string.format("%d FAILED", failures))
    os.exit(1)
end
print("all racer match-point checks passed")
