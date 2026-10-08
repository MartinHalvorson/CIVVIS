-- ⚠ lupa's Lua 5.1.5 (the local runner) barriers the CLOSURE in
-- lua_setupvalue, not the shared UpVal, so a value written into an upvalue the
-- collector has already marked can be freed while still referenced: the merge
-- 7bd809bdb crashed order_queue_test 6/6 (SIGSEGV/BUS/ABRT/TRAP, varying) right
-- after its agent reload, and heap layout decided which tree crashed (-4f
-- root-caused it, 2026-10-05). A full collection first leaves nothing marked.
-- Harmless under the real lua5.1 CI runs.
do
	local setupvalue = debug.setupvalue
	debug.setupvalue = function(...)
		collectgarbage("collect")
		return setupvalue(...)
	end
end

-- Run: lua5.1 tools/civ6_control/mod/congress_claim_denial_test.lua
-- Exercise the production ballot with the losing native t201 bank and leader.
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
-- `ParticipationDenial` is off for the Diplomatic Victory checks below and
-- exercised by its own case at the end.
local config = { Play = true, CivvisDecides = false, CounterResolutions = false,
    ParticipationDenial = false }
upvalue(tick, "cfg", config)
local hooks = {}
Events = setmetatable({ WorldCongressStage1 = { Add = function(fn) hooks.stage = fn end } },
    { __index = function() return stub() end })
LuaEvents = setmetatable({ CivvisCongressBallot = { Add = function(fn) hooks.popup = fn end } },
    { __index = function() return stub() end })
Automation = { Log = function() end }
local bank, leaderPoints = 314, 17
local costs = { MaxVotes = 20 }
for k = 0, 20 do costs[k] = 2 * (k + 1) * k end
local wc = {
    GetVotesandFavorCost = function() return costs end,
    GetResolutions = function() return {
        Stage = 2147483647,
        { Type = "WC_RES_DIPLOVICTORY", TargetType = "PlayerType", PossibleTargets = { 0, 1, 2, 3 } },
        { Type = "WC_RES_MIGRATION_TREATY", TargetType = "PlayerType", PossibleTargets = { 0, 1, 2, 3 } },
        { Type = "WC_RES_TRADE_TREATY", TargetType = "PlayerType", PossibleTargets = { 0, 1, 2, 3 } },
    } end,
}
Game = {
    GetLocalPlayer = function() return 0 end,
    GetCurrentGameTurn = function() return 201 end,
    GetCurrentTurnSegment = function() return "TURNSEG_WORLDCONGRESS_1" end,
    GetWorldCongress = function() return wc end,
}
DB = { MakeHash = function(name) return name end }
Players = { [0] = {
    IsTurnActive = function() return false end,
    GetFavor = function() return bank end,
    GetFavorEnteringCongress = function() return bank end,
} }
for id = 1, 3 do
    local player = id
    Players[id] = {
        GetStats = function() return {
            GetDiplomaticVictoryPoints = function() return player == 3 and leaderPoints or 8 end,
        } end,
        GetScore = function() return player == 3 and 1275 or 800 end,
    }
end
PlayerManager = { GetAliveMajorIDs = function() return { 0, 1, 2, 3 } end }
GameInfo = { Resolutions = {} }
for index, name in ipairs({ "WC_RES_DIPLOVICTORY", "WC_RES_MIGRATION_TREATY", "WC_RES_TRADE_TREATY" }) do
    GameInfo.Resolutions[name] = { ResolutionType = name, Hash = index }
end
PlayerOperations = {
    PARAM_RESOLUTION_TYPE = "type", PARAM_WORLD_CONGRESS_VOTES = "votes",
    PARAM_RESOLUTION_OPTION = "option", PARAM_RESOLUTION_SELECTION = "selection",
    WORLD_CONGRESS_RESOLUTION_VOTE = "vote", WORLD_CONGRESS_SUBMIT_TURN = "submit",
}
local requests, submits = {}, 0
UI = { RequestPlayerOperation = function(pid, operation, params)
    assert(pid == 0)
    if operation == "vote" then requests[#requests + 1] = params end
    if operation == "submit" then submits = submits + 1 end
end }
tick()
local cast = upvalue(hooks.popup, "castBallot")
local vote = upvalue(cast, "voteWorldCongress")
local failures = 0
local function check(name, got, want)
    if got ~= want then
        failures = failures + 1
        print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
    else
        print("ok " .. name)
    end
end
local function ballot(name, favor, points, floor, option, mode, expectedVotes)
    requests, submits = {}, 0
    bank, leaderPoints = favor, points
    config.DiploVictoryVoteFloor = floor
    local count, spent, _, leader, _, _, actualMode = vote(0)
    check(name .. " casts every resolution", count, 3)
    check(name .. " mode", actualMode, mode)
    check(name .. " diplomatic option", requests[1].option, option)
    check(name .. " target", requests[1].selection, option == 2 and 3 or 0)
    check(name .. " host-priced votes", requests[1].votes, expectedVotes)
    check(name .. " cost", spent, costs[expectedVotes - 1])
    check(name .. " other ballots stay free", requests[2].votes + requests[3].votes, 2)
    check(name .. " submits once", submits, 1)
    check(name .. " leader", leader, 3)
end

-- Recorded t201: our 13 A/self votes joined the rival's 14 A votes, beating
-- the other rivals' 18 B/leader votes. A selected player 3, who reached 21 DVP.
-- `CivvisCongressBallotBank` paces the bank: below match point (18) a denial
-- spends half of it (157 of 314 buys 9 votes), keeping the rest for the
-- session a leader can win from.
ballot("native t201", 314, 17, nil, 2, "deny", 9)
-- With the other recorded votes held fixed, the selected denial joins B.
-- This is a ballot counterfactual, not proof of a changed native game outcome.
local ours = requests[1]
local optionA = 14 + (ours.option == 1 and ours.votes or 0)
local optionB = 18 + (ours.option == 2 and ours.votes or 0)
check("recorded rivals plus our denial choose B", optionB > optionA, true)

ballot("native t181", 569, 15, nil, 2, "deny", 12)
ballot("floor boundary", 314, 12, nil, 2, "deny", 9)
-- At match point the whole bank goes.
ballot("match point", 314, 18, nil, 2, "deny", 13)
ballot("match point plus", 569, 19, nil, 2, "deny", 17)
-- A self-claim spends only what is above the 120 reserve (436 buys 15).
ballot("claim below floor", 556, 11, nil, 1, "claim", 15)
-- With the floor moved to 18, 314 less the reserve buys 10 votes: short of a
-- twelve-vote claim, so the session probes with three.
ballot("explicit later floor", 314, 17, 18, 2, "probe", 3)
ballot("explicit later floor, large bank", 556, 17, 18, 1, "claim", 15)
ballot("small denial bank", 12, 17, nil, 2, "deny", 2)
ballot("free denial", 0, 17, nil, 2, "free", 1)

-- From the floor, a claim that outvotes the last session's largest rival A
-- block. Game 99 (T042226Z) t221: 439 Favor at 15 points, the largest rival
-- A block at t201 was 5; half the bank buys 10 votes, twice the block.
local tally = upvalue(vote, "envoyTally")
tally.wc_rival_block = 5
-- From the floor the outvote goes all in above the 60 reserve: 379 buys 14.
ballot("game 99 t221 outvotes the blocks", 439, 15, nil, 1, "outvote", 14)
config.DiploVictoryOutvoteAllIn = false
ballot("outvote all-in off keeps the session share", 439, 15, nil, 1, "outvote", 10)
config.DiploVictoryOutvoteAllIn = nil
-- A block that 10 votes do not double stays a denial (native t201: our 13
-- against the leader's 14 A votes lost the +2 to the leader).
tally.wc_rival_block = 8
ballot("a larger block keeps the denial", 439, 15, nil, 2, "deny", 10)
-- Off by configuration.
tally.wc_rival_block = 5
config.DiploVictoryOutvoteClaim = false
ballot("outvote off", 439, 15, nil, 2, "deny", 10)
config.DiploVictoryOutvoteClaim = nil
-- Match point spends the whole bank, on the denial: from 16 the rivals gang
-- on the leader, and an A claim is cast into their B (game 134 t241).
ballot("match point denies with the gang", 439, 18, nil, 2, "deny", 15)
ballot("a leader at 16 is denied, not outvoted", 439, 16, nil, 2, "deny", 10)
config.DiploVictoryGangCertain = 19
ballot("a later gang threshold keeps the outvote", 439, 18, nil, 1, "outvote", 15)
config.DiploVictoryGangCertain = nil
-- Game 102 (T051413Z) t181: 171 Favor with the leader on 8, below the floor;
-- the largest rival A block at t162 was 2, and the bank had fallen from 190
-- since that session. A draining bank is spent whole (9 votes), and 9 votes
-- outvote the blocks below the floor too.
tally.wc_rival_block = 2
tally.wc_review_favor = 190
ballot("game 102 t181 drains and outvotes", 171, 8, nil, 1, "outvote", 9)
-- Not draining: the reserve stands (51 above it buys 5), still twice the block.
tally.wc_review_favor = 150
ballot("game 102 t181 without the drain", 171, 8, nil, 1, "outvote", 5)
-- A block the bank cannot double keeps the probe below the floor.
tally.wc_rival_block = 6
tally.wc_review_favor = 190
ballot("a block too big below the floor probes", 171, 8, nil, 2, "probe", 3)
config.DiploVictorySpendDraining = false
tally.wc_rival_block = 2
ballot("drain spending off", 171, 8, nil, 1, "outvote", 5)
config.DiploVictorySpendDraining = nil
tally.wc_rival_block = nil
tally.wc_review_favor = nil

-- `CivvisParticipationDenialOption`: with the leader on 19, the Migration
-- Treaty -- whose leader votes A on itself -- draws three B votes on the
-- leader from what the Diplomatic Victory ballot leaves (569 - 544 = 25), and
-- the Diplomatic Victory ballot is unchanged. The Trade Treaty, with no record
-- of the leader's vote, keeps its free vote.
config.ParticipationDenial = nil
requests, submits = {}, 0
bank, leaderPoints = 569, 19
config.DiploVictoryVoteFloor = nil
local _, deniedSpent = vote(0)
check("participation denial leaves the diplomatic ballot", requests[1].votes, 17)
check("participation denial option", requests[2].option, 2)
check("participation denial names the leader", requests[2].selection, 3)
check("participation denial votes", requests[2].votes, 3)
check("participation denial cost", deniedSpent, costs[16] + costs[2])
check("participation denial leaves an unknown vote free", requests[3].votes, 1)
config.ParticipationDenial = false

-- See `CivvisCongressRedirect`. Game 118 (T095215Z) t202: Macedon (here
-- player 3, on 15 points) cast 9 A votes for itself at t182, Gaul (player 1,
-- 8 points) 4 and Portugal (player 2, 8) 5. Half of 371 buys 10 votes: short
-- of the 13 a claim needs over 9 plus a quarter, enough to lift Gaul's block
-- (counted at three of its four) past 12. Ties on points and votes go to the
-- lower id.
local function redirect(name, favor, points, option, selection, mode, votes)
    requests, submits = {}, 0
    bank, leaderPoints = favor, points
    config.DiploVictoryVoteFloor = nil
    local _, spent, _, _, _, _, actualMode = vote(0)
    check(name .. " mode", actualMode, mode)
    check(name .. " option", requests[1].option, option)
    check(name .. " target", requests[1].selection, selection)
    check(name .. " votes", requests[1].votes, votes)
    check(name .. " cost", spent, costs[votes - 1])
end
tally.wc_rival_blocks = { [3] = 9, [1] = 4, [2] = 5 }
tally.wc_dvp_won = 1
redirect("game 118 t202 hands the +2 to Gaul", 371, 15, 1, 1, "redirect", 10)
-- A bank that buys the block plus a quarter claims it for us -- from the
-- floor with every vote above the 60 reserve (379 buys 14), since game 130
-- t181 lost a claim 11-12 with 13 votes in the bank.
tally.wc_rival_blocks = { [3] = 4, [1] = 2 }
redirect("a small block is outvoted for us", 439, 15, 1, 0, "outvote", 14)
-- A session the rivals carried on B is left to the denial.
tally.wc_rival_blocks = { [3] = 9, [1] = 4, [2] = 5 }
tally.wc_dvp_won = 2
redirect("after a B session the denial stands", 371, 15, 2, 3, "deny", 10)
-- Below the gang floor the rivals vote A for themselves again (0 of 15
-- sessions after a B win left a leader at 14 or less ganged on): game 133's
-- t221. Last session only the leader voted A (11); the others' last A blocks
-- are remembered. 11 plus a quarter is 14; half of 600 buys 12 votes, which
-- lift Gaul's 4 (counted at three of four) past it.
tally.wc_rival_blocks = { [3] = 11 }
tally.wc_rival_blocks_seen = { [3] = 11, [1] = 4, [2] = 5 }
redirect("after a B session a leader at 14 is redirected", 600, 14, 1, 1, "redirect", 12)
config.DiploVictoryGangFloor = 14
redirect("a gang floor at 14 keeps the denial", 600, 14, 2, 3, "deny", 12)
config.DiploVictoryGangFloor = nil
tally.wc_rival_blocks_seen = nil
tally.wc_rival_blocks = { [3] = 9, [1] = 4, [2] = 5 }
tally.wc_dvp_won = 1
-- The largest block held by a rival far behind the leader is no contender's.
tally.wc_rival_blocks = { [1] = 9, [3] = 4 }
redirect("a non-contender's block is not bought", 371, 15, 2, 3, "deny", 10)
-- `congress-guards-the-leader` (the bridge sets `DiploVictoryGuardLeader`).
-- Game 402 (T132311Z) t201: last session's largest A block was the
-- Netherlands' (here player 1, 9 points) 7, the leader's (player 3, 15) 3
-- and Scythia's 2; the leader then cast 7 and won 13-10 against our B. With
-- the guard the leader is the block to beat, read at the largest last block
-- (7): 7 plus 2 is 9, which half of 371 outbuys, so the bank claims it for us
-- -- all in from the floor (the 311 above the reserve buys 12).
config.DiploVictoryGuardLeader = true
tally.wc_rival_blocks = { [1] = 7, [3] = 3, [2] = 2 }
redirect("game 402 t201 guards the leader", 371, 15, 1, 0, "outvote", 12)
-- The non-contender's 9 above, read as the leader's: 9 plus 3 is 12, more
-- than half of 371 buys, so player 1's block (counted at six of its nine)
-- is lifted past it with 7.
tally.wc_rival_blocks = { [1] = 9, [3] = 4 }
redirect("a guarded leader redirects past the big block", 371, 15, 1, 1, "redirect", 7)
-- Under the guard floor (14) the old reading stands.
redirect("the guard waits for fourteen", 371, 13, 2, 3, "deny", 10)
config.DiploVictoryGuardLeader = nil
-- Tied blocks: the contender's (player 3, on 12) is the one to beat, 5 plus
-- 2. Half of 227 buys the eight a claim needs; half of 150 buys six, and
-- player 1's block (8 points, counted at three of five) takes the +2 with
-- five. Reading the lower id's block as the top found no contender and
-- denied.
tally.wc_rival_blocks = { [1] = 5, [3] = 5 }
redirect("tied blocks: a claim over the contender", 227, 12, 1, 0, "outvote", 9)
redirect("tied blocks: the contender's is the one to beat", 150, 12, 1, 1, "redirect", 5)
-- Off by configuration.
tally.wc_rival_blocks = { [3] = 9, [1] = 4, [2] = 5 }
config.DiploVictoryRedirect = false
redirect("redirect off", 371, 15, 2, 3, "deny", 10)
config.DiploVictoryRedirect = nil
tally.wc_rival_blocks = nil
tally.wc_dvp_won = nil

-- See `CivvisCultureEmbargoVotes`. The culture leader (player 3) draws 90
-- visitors against a bar of 100: the counter's Trade Policy ballot buys
-- eight B votes on it (no recorded A total: 6 + 2) from what the probe left
-- (314 - 12), 112 Favor; below the bar it keeps the one free vote.
config.CounterResolutions = nil
local function culture(id, visitors, domestic)
    Players[id].GetCulture = function() return {
        GetTouristsTo = function() return visitors end,
        GetStaycationers = function() return domestic end,
    } end
end
culture(1, 0, 100); culture(2, 0, 100); culture(3, 90, 10)
requests, submits = {}, 0
bank, leaderPoints = 314, 8
config.DiploVictoryVoteFloor = nil
local _, embargoSpent = vote(0)
check("culture embargo option", requests[3].option, 2)
check("culture embargo names the culture leader", requests[3].selection, 3)
check("culture embargo votes", requests[3].votes, 8)
check("culture embargo cost", embargoSpent, costs[2] + costs[7])
check("the migration counter keeps its free vote", requests[2].votes, 1)
-- A recorded A total of 3 asks for five.
tally.wc_last_a = { WC_RES_TRADE_TREATY = 3 }
requests, submits = {}, 0
vote(0)
check("culture embargo outvotes the recorded block", requests[3].votes, 5)
tally.wc_last_a = nil
-- Under the bar, the free vote.
culture(3, 60, 10)
requests, submits = {}, 0
vote(0)
check("culture embargo waits for the bar", requests[3].votes, 1)
-- Off by configuration.
culture(3, 90, 10)
config.CultureEmbargo = false
requests, submits = {}, 0
vote(0)
check("culture embargo off", requests[3].votes, 1)
config.CultureEmbargo = nil
for id = 1, 3 do Players[id].GetCulture = nil end
config.CounterResolutions = false

if failures > 0 then os.exit(1) end
print("all Congress claim/denial checks passed")
