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
local config = { Play = true, CivvisDecides = false, CounterResolutions = false }
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

if failures > 0 then os.exit(1) end
print("all Congress claim/denial checks passed")
