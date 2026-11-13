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

-- The pure pieces.
check("lane gate on", CivvisDiploRacer({ VictoryLane = "diplomatic" }), true)
check("lane gate other lane", CivvisDiploRacer({ VictoryLane = "culture" }), false)
check("lane gate off switch", CivvisDiploRacer({ VictoryLane = "diplomatic", DiploRacer = false }), false)
check("lane gate no config", CivvisDiploRacer(nil), false)
-- `science-seat-races-the-congress`: the ballot's gate widens, the lane's
-- does not, so `CivvisPublicWorksVote` still runs on a Science seat.
local science = { VictoryLane = "science", ScienceCongressRacer = true }
check("congress gate reads the Diplomatic lane", CivvisCongressRacer({ VictoryLane = "diplomatic" }), true)
check("congress gate on under the tag", CivvisCongressRacer(science), true)
check("congress gate leaves the lane gate", CivvisDiploRacer(science), false)
check("congress gate off without the tag", CivvisCongressRacer({ VictoryLane = "science" }), false)
check("congress gate is Science only",
    CivvisCongressRacer({ VictoryLane = "culture", ScienceCongressRacer = true }), false)
check("congress gate off switch", CivvisCongressRacer({ VictoryLane = "science",
    ScienceCongressRacer = true, DiploRacer = false }), false)
check("congress gate no config", CivvisCongressRacer(nil), false)
local v, p = CivvisRacerResolutionVotes("WC_RES_BORDER_CONTROL", 100, costs, {})
check("border control buys 2", v, 2); check("border control costs 4", p, 4)
-- The four counts re-read on the Deity lane (106 games, 2026-11-12/13).
v, p = CivvisRacerResolutionVotes("WC_RES_PUBLIC_RELATIONS", 100, costs, {})
check("public relations buys 3", v, 3); check("public relations costs 12", p, 12)
v, p = CivvisRacerResolutionVotes("WC_RES_ARMS_CONTROL", 100, costs, {})
check("arms control buys 4", v, 4); check("arms control costs 24", p, 24)
v, p = CivvisRacerResolutionVotes("WC_RES_TRADE_TREATY", 200, costs, {})
check("trade treaty buys 6 at the default cap of 60", v, 6); check("trade treaty costs 60", p, 60)
v, p = CivvisRacerResolutionVotes("WC_RES_MIGRATION_TREATY", 200, costs, {})
check("migration treaty buys 6 at the default cap", v, 6); check("migration treaty costs 60", p, 60)
v, p = CivvisRacerResolutionVotes("WC_RES_TRADE_TREATY", 200, costs, { DiploRacerResolutionCap = 40 })
check("trade treaty is over a 40 cap", v, 1); check("trade treaty costs nothing over the cap", p, 0)
v, p = CivvisRacerResolutionVotes("WC_RES_PATRONAGE", 200, costs, {})
check("patronage is over the default cap of 60", v, 1); check("patronage costs nothing at the default", p, 0)
v, p = CivvisRacerResolutionVotes("WC_RES_PATRONAGE", 200, costs, { DiploRacerResolutionCap = 180 })
check("patronage buys 10 at a 180 cap", v, 10); check("patronage costs 180", p, 180)
v, p = CivvisRacerResolutionVotes("WC_RES_LUXURY", 1000, costs, {})
check("luxury over the cap keeps the free vote", v, 1); check("luxury costs nothing", p, 0)
v, p = CivvisRacerResolutionVotes("WC_RES_TRADE_TREATY", 50, costs, {})
check("a bank short of the price keeps the free vote", v, 1)
-- Border Control 4 and Public Relations 12; the Trade Treaty's 60 is over
-- the cheap cap of 40 and waits for what the claim leaves.
check("reserve funds only the cheap votes",
    CivvisRacerReserve({ "WC_RES_BORDER_CONTROL", { rtype = "WC_RES_TRADE_TREATY" },
        "WC_RES_PUBLIC_RELATIONS" }, costs, {}), 16)
check("a session with the claim on it reserves nothing",
    CivvisRacerReserve({ "WC_RES_DIPLOVICTORY", "WC_RES_BORDER_CONTROL",
        "WC_RES_PUBLIC_RELATIONS" }, costs, {}), 0)
check("claim least from block 5", CivvisRacerClaimVotes(120, 6, 5, costs, 20, {}), 8)
check("claim tops up a quarter", CivvisRacerClaimVotes(300, 6, 5, costs, 20, {}), 10)
check("short of least but over the block, all in", CivvisRacerClaimVotes(100, 6, 5, costs, 20, {}), 7)
check("live t181: 10 votes beat a block of 9", CivvisRacerClaimVotes(190, 10, 9, costs, 20, {}), 10)
-- The claim floor (Deity lane, 2026-11-13): a bank that buys nine votes
-- claims with all of it though it is not over the last block. 182344Z t134
-- (151 Favor, last block 9; the rivals' largest that session was 8), 193612Z
-- t134 (144, 9; 5) and 204218Z t134 (249, 12; 6) each sat out a session that
-- was theirs.
check("151 Favor claims nine against a last block of 9", CivvisRacerClaimVotes(151, 8, 9, costs, 20, {}), 9)
check("144 Favor claims nine against a last block of 9", CivvisRacerClaimVotes(144, 8, 9, costs, 20, {}), 9)
check("249 Favor claims eleven against a last block of 12", CivvisRacerClaimVotes(249, 4, 12, costs, 20, {}), 11)
check("eight votes are under the floor", CivvisRacerClaimVotes(143, 8, 9, costs, 20, {}), nil)
check("a raised floor sits the session out",
    CivvisRacerClaimVotes(249, 4, 12, costs, 20, { DiploRacerClaimFloor = 12 }), nil)
check("the floor does not shrink a claim that reaches its count",
    CivvisRacerClaimVotes(300, 6, 5, costs, 20, {}), 10)
check("no more than the block, no claim", CivvisRacerClaimVotes(30, 6, 5, costs, 20, {}), nil)
check("claim first block 8", CivvisRacerClaimVotes(1000, 6, nil, costs, 20, {}), 13)
check("claim all in at match point", CivvisRacerClaimVotes(300, 18, 9, costs, 20, {}), 12)
-- `CivvisRacerClaimBlock`: against a contender, the larger of the last two
-- sessions' blocks; otherwise the last.
check("contender: the bigger block of two sessions", CivvisRacerClaimBlock(5, 9, 15, 15, {}), 9)
check("contender ahead: still the bigger", CivvisRacerClaimBlock(5, 9, 15, 17, {}), 9)
check("within two behind is a contender", CivvisRacerClaimBlock(5, 9, 15, 13, {}), 9)
check("three behind is not", CivvisRacerClaimBlock(5, 9, 15, 12, {}), 5)
check("a smaller block before keeps the last", CivvisRacerClaimBlock(9, 5, 15, 15, {}), 9)
check("no block before keeps the last", CivvisRacerClaimBlock(5, nil, 15, 15, {}), 5)
check("no last block takes the one before", CivvisRacerClaimBlock(nil, 9, 15, 15, {}), 9)
check("the switch keeps the last", CivvisRacerClaimBlock(5, 9, 15, 15, { DiploRacerContenderPeak = false }), 5)

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
check("public relations bought", requests[3].votes, 3)
check("race spend", spent, 180 + 4 + 12)

-- The claim floor in the ballot: 151 Favor after a contested session, the
-- rivals' last block 9. Nine votes (144) claim A on us; the 7 left buy
-- Border Control its two and Public Relations nothing.
spent, mode = run(151, 8, 6, 9)
check("floor claim mode", mode, "race")
check("floor claim is A on us", requests[1].option .. "/" .. requests[1].selection, "1/0")
check("floor claim votes", requests[1].votes, 9)
check("floor claim leaves border control its two", requests[2].votes, 2)
check("floor claim leaves public relations the free vote", requests[3].votes, 1)
check("floor claim spend", spent, 144 + 4)
-- One Favor less buys eight votes: under the floor, the session is sat out.
spent, mode = run(143, 8, 6, 9)
check("under the floor mode", mode, "free")
check("under the floor the Diplomatic Victory vote is free", requests[1].votes, 1)

-- A small bank cannot reach the claim: the probe stands down to the free
-- vote and the participation votes are still bought.
spent, mode = run(50, 8, 6, 5)
check("small bank mode", mode, "free")
check("small bank free vote", requests[1].votes, 1)
check("small bank still buys border control", requests[2].votes, 2)
check("small bank spend", spent, 4 + 12)

-- Immortal 173341Z t174: both on 15, the rival's last block 5 but 9 the
-- session before, about 351 Favor: 13 votes (from the 9), not 10.
session = { "WC_RES_DIPLOVICTORY" }
tally.wc_rival_block_prev = 9
spent, mode = run(351, 15, 15, 5)
check("contender claim races", mode, "race")
check("contender claim sized from the bigger block", requests[1].votes, 13)
tally.wc_rival_block_prev = nil
spent, mode = run(351, 15, 15, 5)
check("without the block before, sized from the last", requests[1].votes, 10)
session = { "WC_RES_DIPLOVICTORY", "WC_RES_BORDER_CONTROL", "WC_RES_PUBLIC_RELATIONS" }

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
-- From match point the bank is spent (`CivvisRacerMatchPointVotes`): twice
-- the ordinary count on each, 4 votes (24 Favor) and 6 (60).
check("brace buys border control with twice its count", requests[2].votes, 4)
check("brace buys public relations with twice its count", requests[3].votes, 6)
check("brace spend", spent, 24 + 60)
config.RacerMatchPointBank = false
spent, mode = run(300, 8, 18, 9)
check("with the match-point bank off the ordinary counts stand",
    requests[2].votes .. "/" .. requests[3].votes, "2/3")
check("and the ordinary spend", spent, 4 + 12)
config.RacerMatchPointBank = nil
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
session = { "WC_RES_DIPLOVICTORY", "WC_RES_PATRONAGE" }
spent, mode = run(400, 8, 6, 5)
check("dear session races", mode, "race")
check("the claim keeps no reserve for a dear vote", requests[1].votes, 10)
check("patronage keeps the free vote at the default cap", requests[2].votes, 1)
check("dear session spend at the default cap", spent, 180)
config.DiploRacerResolutionCap = 180
spent, mode = run(400, 8, 6, 5)
check("patronage bought from the rest under a 180 cap", requests[2].votes, 10)
check("dear session spend under a 180 cap", spent, 180 + 180)
config.DiploRacerResolutionCap = nil
-- The Trade Treaty and the Migration Treaty are 6 votes (60 Favor) now, so
-- they are bought at the default cap from what the claim leaves, and keep
-- the free vote when it leaves less.
session = { "WC_RES_DIPLOVICTORY", "WC_RES_TRADE_TREATY" }
spent, mode = run(400, 8, 6, 5)
check("trade treaty bought at the default cap", requests[2].votes, 6)
check("trade treaty is A on us", requests[2].option .. "/" .. requests[2].selection, "1/0")
check("trade session spend", spent, 180 + 60)
session = { "WC_RES_DIPLOVICTORY", "WC_RES_MIGRATION_TREATY" }
spent, mode = run(250, 8, 6, 5)
check("migration treaty bought from the 70 the claim leaves", requests[2].votes, 6)
check("migration session spend", spent, 180 + 60)
spent, mode = run(230, 8, 6, 5)
check("migration treaty keeps the free vote when the claim leaves 50", requests[2].votes, 1)
check("migration session spend on the claim alone", spent, 180)
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

-- `science-seat-races-the-congress`: a Science seat keeps its old ballot...
config.VictoryLane = "science"
spent, mode = run(300, 8, 6, 6)
check("science lane probes without the tag", mode, "probe")
check("science lane keeps the PR ban option", requests[3].option, 2)
check("science lane leaves border control free", requests[2].votes, 1)
-- ...and under the tag casts the same ballot the Diplomatic lane casts.
config.ScienceCongressRacer = true
spent, mode = run(300, 8, 6, 6)
local raced = { spent = spent, mode = mode, votes = requests[1].votes, option = requests[1].option,
    selection = requests[1].selection, border = requests[2].votes, pr = requests[3].option }
check("science seat races under the tag", mode, "race")
check("science seat claims option A", raced.option, 1)
config.VictoryLane = "diplomatic"
config.ScienceCongressRacer = nil
spent, mode = run(300, 8, 6, 6)
check("the same claim as the lane", raced.votes, requests[1].votes)
check("the same target as the lane", raced.selection, requests[1].selection)
check("the same spend as the lane", raced.spent, spent)
check("the same border votes as the lane", raced.border, requests[2].votes)
check("the same PR option as the lane", raced.pr, requests[3].option)
-- Short of a claim above the denial floor, a Science racer banks: a paid B
-- never decided a session. The Diplomatic lane keeps its denial.
config.VictoryLane = "science"
config.ScienceCongressRacer = true
spent, mode = run(120, 14, 6, 12)
check("science racer short of a claim casts the free vote", mode, "free")
check("science racer banks the Favor", requests[1].votes, 1)
check("science racer's free vote stays on B", requests[1].option, 2)
-- With the rivals' gang on a leader past 16: the same free vote beside them.
spent, mode = run(300, 17, 6, 8)
check("science racer joins the gang free", mode, "free")
check("science racer spends nothing on the gang's vote", requests[1].votes, 1)
check("science racer votes with the gang", requests[1].option, 2)
config.VictoryLane = "science"
config.ScienceCongressRacer = nil
spent, mode = run(120, 14, 6, 12)
check("without the tag the denial still pays", mode, "deny")
check("without the tag it casts the bank", requests[1].votes > 1, true)
config.VictoryLane = "diplomatic"
spent, mode = run(120, 14, 6, 12)
check("the Diplomatic lane keeps its denial", mode, "deny")
-- A Public Works vote stands: the racer's buying branch leaves its count.
GameInfo.Resolutions.WC_RES_PUBLIC_WORKS = { ResolutionType = "WC_RES_PUBLIC_WORKS", Hash = 6 }
config.VictoryLane = "science"
config.ScienceCongressRacer = true
local spaceStage = CivvisSpaceStage
CivvisSpaceStage = function(id) return id == 3 and 2 or 0 end
local rtypeTargets = wc.GetResolutions
wc.GetResolutions = function()
    local list = rtypeTargets()
    for _, r in ipairs(list) do
        if r.Type == "WC_RES_PUBLIC_WORKS" then
            r.TargetType = "ProjectType"
            r.PossibleTargets = { "LOC_PROJECT_LAUNCH_EARTH_SATELLITE_NAME",
                "LOC_PROJECT_LAUNCH_MOON_LANDING_NAME", "LOC_PROJECT_LAUNCH_MARS_BASE_NAME",
                "LOC_PROJECT_LAUNCH_EXOPLANET_EXPEDITION_NAME" }
        end
    end
    return list
end
session = { "WC_RES_DIPLOVICTORY", "WC_RES_PUBLIC_WORKS", "WC_RES_PUBLIC_RELATIONS" }
spent, mode = run(300, 8, 6, 6)
local pw
for _, q in ipairs(requests) do if q.type == 6 then pw = q end end
check("public works is on the ballot", pw ~= nil, true)
check("public works still slows the racer", pw and pw.option, 2)
check("public works keeps its own vote count", pw and pw.votes > 1, true)
-- With a cap that lets the racer's buying branch reach this resolution, that
-- branch still leaves the count alone.
spent, mode = run(900, 8, 6, 6)
pw = nil
for _, q in ipairs(requests) do if q.type == 6 then pw = q end end
local slowed = pw and pw.votes
config.DiploRacerResolutionCap = 400
spent, mode = run(900, 8, 6, 6)
pw = nil
for _, q in ipairs(requests) do if q.type == 6 then pw = q end end
check("a higher cap leaves the public works votes", pw and pw.votes, slowed)
config.DiploRacerResolutionCap = nil
wc.GetResolutions = rtypeTargets
CivvisSpaceStage = spaceStage
session = { "WC_RES_DIPLOVICTORY", "WC_RES_BORDER_CONTROL", "WC_RES_PUBLIC_RELATIONS" }
config.VictoryLane = "diplomatic"
config.ScienceCongressRacer = nil

if failures > 0 then
    print(failures .. " failure(s)")
    os.exit(1)
end
print("racer ballot: all ok")
