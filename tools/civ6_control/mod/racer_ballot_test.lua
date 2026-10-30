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

-- Run: lua5.1 tools/civ6_control/mod/racer_ballot_test.lua
-- The production ballot on a seat assigned the Diplomatic lane
-- (`CivvisDiploRacer`): participation votes first, then the A claim on us.
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
        "WC_RES_PUBLIC_RELATIONS", "WC_RES_TRADE_TREATY", "WC_RES_MIGRATION_TREATY" }) do
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

-- The pure pieces.
check("lane gate on", CivvisDiploRacer({ VictoryLane = "diplomatic" }), true)
check("lane gate other lane", CivvisDiploRacer({ VictoryLane = "culture" }), false)
check("lane gate off switch", CivvisDiploRacer({ VictoryLane = "diplomatic", DiploRacer = false }), false)
check("lane gate no config", CivvisDiploRacer(nil), false)
local v, p = CivvisRacerResolutionVotes("WC_RES_BORDER_CONTROL", 100, costs, {})
check("border control buys 2", v, 2); check("border control costs 4", p, 4)
v, p = CivvisRacerResolutionVotes("WC_RES_TRADE_TREATY", 200, costs, {})
check("trade treaty is over the default cap of 60", v, 1); check("trade treaty costs nothing at the default", p, 0)
v, p = CivvisRacerResolutionVotes("WC_RES_TRADE_TREATY", 200, costs, { DiploRacerResolutionCap = 112 })
check("trade treaty buys 8 at a 112 cap", v, 8); check("trade treaty costs 112", p, 112)
v, p = CivvisRacerResolutionVotes("WC_RES_LUXURY", 1000, costs, {})
check("luxury over the cap keeps the free vote", v, 1); check("luxury costs nothing", p, 0)
v, p = CivvisRacerResolutionVotes("WC_RES_TRADE_TREATY", 100, costs, {})
check("a bank short of the price keeps the free vote", v, 1)
check("reserve funds only the cheap votes",
    CivvisRacerReserve({ "WC_RES_BORDER_CONTROL", { rtype = "WC_RES_TRADE_TREATY" },
        "WC_RES_PUBLIC_RELATIONS" }, costs, {}), 8)
check("a session with the claim on it reserves nothing",
    CivvisRacerReserve({ "WC_RES_DIPLOVICTORY", "WC_RES_BORDER_CONTROL",
        "WC_RES_PUBLIC_RELATIONS" }, costs, {}), 0)
check("claim least from block 5", CivvisRacerClaimVotes(120, 6, 5, costs, 20, {}), 8)
check("claim tops up a quarter", CivvisRacerClaimVotes(300, 6, 5, costs, 20, {}), 10)
check("short of least but over the block, all in", CivvisRacerClaimVotes(100, 6, 5, costs, 20, {}), 7)
check("live t181: 10 votes beat a block of 9", CivvisRacerClaimVotes(190, 10, 9, costs, 20, {}), 10)
check("no more than the block, no claim", CivvisRacerClaimVotes(30, 6, 5, costs, 20, {}), nil)
check("claim first block 8", CivvisRacerClaimVotes(1000, 6, nil, costs, 20, {}), 13)
check("claim all in at match point", CivvisRacerClaimVotes(300, 18, 9, costs, 20, {}), 12)

-- A session at t143: rival leader on 8, its largest A block last time 5,
-- 300 Favor. Reserve 8 for Border Control and Public Relations; the claim
-- takes 10 votes (180) from 292; both participation votes are bought.
local spent, mode = run(300, 8, 6, 5)
check("race mode", mode, "race")
check("race claims option A", requests[1].option, 1)
check("race names us", requests[1].selection, 0)
check("race votes", requests[1].votes, 10)
check("border control A on us", requests[2].option, 1)
check("border control target us", requests[2].selection, 0)
check("border control bought", requests[2].votes, 2)
check("public relations A on us (not the ban)", requests[3].option, 1)
check("public relations bought", requests[3].votes, 2)
check("race spend", spent, 180 + 4 + 4)

-- A small bank cannot reach the claim: the probe stands down to the free
-- vote and the participation votes are still bought.
spent, mode = run(50, 8, 6, 5)
check("small bank mode", mode, "free")
check("small bank free vote", requests[1].votes, 1)
check("small bank still buys border control", requests[2].votes, 2)
check("small bank spend", spent, 8)

-- A rival on the gang threshold keeps the denial: B on the leader.
spent, mode = run(300, 16, 6, 5)
check("gang keeps the deny", mode, "deny")
check("gang votes B", requests[1].option, 2)
check("gang names the leader", requests[1].selection, 3)

-- The gang falls on the leader of all the majors. A rival on the threshold
-- BEHIND us is no denial target: we claim (17 > 16)...
spent, mode = run(300, 16, 17, 5)
check("leading a ganged rival races", mode, "race")
check("leading a ganged rival claims A", requests[1].option, 1)
check("leading a ganged rival names us", requests[1].selection, 0)
check("leading a ganged rival claims sized", requests[1].votes, 10)
-- ...brace from our own match point...
spent, mode = run(300, 16, 18, 5)
check("leading a ganged rival at 18 braces", mode, "brace")
-- ...and, short of a claim, take the gang's free vote on ourselves.
spent, mode = run(50, 16, 17, 5)
check("a small bank leading the gang braces", mode, "brace")
check("the brace votes B", requests[1].option, 2)
check("the brace names us", requests[1].selection, 0)
check("the brace is free", requests[1].votes, 1)
-- A tie is not a lead: the denial stands.
spent, mode = run(300, 17, 17, 5)
check("a tied ganged rival keeps the deny", mode, "deny")
check("the tie denial names the rival", requests[1].selection, 3)

-- Our own match point: the rivals gang on us, so B on ourselves with the
-- free vote takes the participation point; the bank buys the others.
spent, mode = run(300, 8, 18, 9)
check("match point braces", mode, "brace")
check("brace votes B", requests[1].option, 2)
check("brace names us", requests[1].selection, 0)
check("brace is the free vote", requests[1].votes, 1)
check("brace still buys border control", requests[2].votes, 2)
check("brace spend", spent, 8)
check("brace below 18", CivvisRacerBraces(17, {}), false)
check("brace at 18", CivvisRacerBraces(18, {}), true)
check("warned at 15 braces", CivvisRacerBraces(15, {}, 6), true)
check("unwarned at 15 does not", CivvisRacerBraces(15, {}, 0), false)
check("warned below 14 does not", CivvisRacerBraces(13, {}, 6), false)
check("warned by a B that passed does not", CivvisRacerBraces(15, {}, 6, 1), false)
check("warned by a B that failed still braces", CivvisRacerBraces(15, {}, 6, 0), true)
check("our own 18 braces whatever passed", CivvisRacerBraces(18, {}, 6, 1), true)
check("the switch restores the old warning",
    CivvisRacerBraces(15, { DiploRacerBraceAfterPassedB = true }, 6, 1), true)
-- Live King 074500Z t181: 15 points, Georgia's 6 B on us at t162 -- the
-- brace, not a 9-vote claim into a 26-vote gang.
tally.wc_b_on_us = 6
spent, mode = run(324, 8, 15, 5)
check("warned at 15: brace", mode, "brace")
check("warned brace votes B on us", requests[1].option, 2)
check("warned brace names us", requests[1].selection, 0)
check("warned brace is free", requests[1].votes, 1)
tally.wc_b_on_us = 0
spent, mode = run(324, 8, 15, 5)
check("unwarned at 15 still claims", mode, "race")
-- Emperor 113137Z t218: warned by a B that CARRIED on us (14 votes at
-- t198) -- the rivals relaxed to 4 B votes, so the claim, not the brace.
tally.wc_b_on_us = 14
tally.wc_b_passed_on_us = 1
spent, mode = run(324, 8, 15, 5)
check("a passed B's warning lapses: claim", mode, "race")
check("the lapsed warning claims A on us", requests[1].option, 1)
check("the lapsed warning names us", requests[1].selection, 0)
tally.wc_b_passed_on_us = 0
spent, mode = run(324, 8, 15, 5)
check("a failed B's warning still braces", mode, "brace")
tally.wc_b_on_us = nil
tally.wc_b_passed_on_us = nil
-- With the brace moved past the win, match point claims all in.
config.DiploRacerGangSelf = 21
spent, mode = run(300, 8, 18, 9)
check("match point race", mode, "race")
check("match point all in", requests[1].votes, 12)
config.DiploRacerGangSelf = nil

-- An expensive resolution is bought from what the claim leaves, under a
-- raised cap; at the default 60 it keeps the free vote.
session = { "WC_RES_DIPLOVICTORY", "WC_RES_TRADE_TREATY" }
spent, mode = run(400, 8, 6, 5)
check("trade session races", mode, "race")
check("trade claim keeps no reserve for a dear vote", requests[1].votes, 10)
check("trade treaty keeps the free vote at the default cap", requests[2].votes, 1)
check("trade session spend at the default cap", spent, 180)
config.DiploRacerResolutionCap = 112
spent, mode = run(400, 8, 6, 5)
check("trade treaty bought from the rest under a 112 cap", requests[2].votes, 8)
check("trade session spend under a 112 cap", spent, 180 + 112)
config.DiploRacerResolutionCap = nil
session = { "WC_RES_DIPLOVICTORY", "WC_RES_BORDER_CONTROL", "WC_RES_PUBLIC_RELATIONS" }

-- Off: the old ballot (a probe below the floor, the ban on Public Relations).
config.DiploRacer = false
-- (A block of 6: twice it is more than the paced 10-vote budget, so no outvote.)
spent, mode = run(300, 8, 6, 6)
check("off probes", mode, "probe")
check("off keeps the PR ban option", requests[3].option, 2)
check("off leaves border control free", requests[2].votes, 1)
config.DiploRacer = nil
-- Another lane: the same.
config.VictoryLane = "culture"
spent, mode = run(300, 8, 6, 6)
check("culture lane probes", mode, "probe")
config.VictoryLane = "diplomatic"

if failures > 0 then
    print(failures .. " failure(s)")
    os.exit(1)
end
print("racer ballot: all ok")
