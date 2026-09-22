-- Offline regression for the World Congress vote budget.
--
-- Run: lua5.1 tools/civ6_control/mod/congress_vote_budget_test.lua
--
-- Native civvis-20260922T151652Z t221 had 340 Favor and a host-priced
-- allowance of 13 votes. A speculative Standard-speed cap asked only eight
-- and prevented the existing twelve-vote claim policy from activating.
-- The host subsequently verified all eight votes, option and target exactly.
-- Price the actual request from the same table as WorldCongressPopup.lua.

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end
setmetatable(_G, { __index = function(_, key)
	if key == "CivvisCongressVoteBudget" then return rawget(_G, key); end
	return stub()
end })

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtimeErr = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtimeErr))

local voteBudget = rawget(_G, "CivvisCongressVoteBudget")
assert(type(voteBudget) == "function",
	"CivvisControlAgent.lua did not export CivvisCongressVoteBudget")

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", name, tostring(got)))
	end
end

-- The Online table as the host reports it: `costs[k]` is the cumulative
-- price of k+1 votes, `2n(n-1)` with the first vote free.
local function onlineCosts(entries)
	local costs = {}
	for k = 0, entries do
		costs[k] = 2 * (k + 1) * k
	end
	return costs
end

-- Recorded Online banks use the host quote, not a hypothetical Standard cap.
local votes, host, standard = voteBudget(791, onlineCosts(20), 20)
check("791 Favor Online ask", votes, 20)
check("host walk", host, 20)
check("Standard comparison stays observable", standard, 13)
votes, host, standard = voteBudget(340, onlineCosts(13), 13)
check("native t221 ask", votes, 13)
check("native t221 host allowance", host, 13)
check("native t221 diagnostic Standard allowance", standard, 8)
check("Online exact boundary", voteBudget(312, onlineCosts(20), 20), 13)
check("Online below boundary", voteBudget(311, onlineCosts(20), 20), 12)
check("small Online bank", voteBudget(52, onlineCosts(10), 10), 5)
local standardCosts = {}
for k = 0, 20 do standardCosts[k] = 5 * (k + 1) * k end
check("Standard exact boundary", voteBudget(780, standardCosts, 20), 13)
check("Standard below boundary", voteBudget(779, standardCosts, 20), 12)

-- MaxVotes caps both walks even when the bank is deep.
votes, host, standard = voteBudget(10000, onlineCosts(20), 20)
check("cap holds the ask", votes, 20)
check("cap holds the host walk", host, 20)
check("cap holds the standard walk", standard, 20)

-- The free vote survives every degenerate input.
check("empty bank", voteBudget(0, onlineCosts(20), 20), 1)
check("nil bank", voteBudget(nil, onlineCosts(20), 20), 1)
check("missing cost table", voteBudget(791, nil, 20), 1)
check("MaxVotes one", voteBudget(791, onlineCosts(20), 1), 1)
check("missing MaxVotes", voteBudget(791, onlineCosts(20), nil), 1)

-- Exercise the real ballot selector and request, not a duplicate claim rule.
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
upvalue(tick, "cfg", { Play = true, CivvisDecides = false, CounterResolutions = false })
local hooks = {}
Events = setmetatable({ WorldCongressStage1 = { Add = function(fn) hooks.stage = fn end } },
    { __index = function() return stub() end })
LuaEvents = setmetatable({ CivvisCongressBallot = { Add = function(fn) hooks.popup = fn end } },
    { __index = function() return stub() end })
Automation = { Log = function() end }
local costs = onlineCosts(13)
costs.MaxVotes = 13
local wc = {
    GetVotesandFavorCost = function() return costs end,
    GetResolutions = function() return {
        Stage = 2147483647,
        { Type = "WC_RES_DIPLOVICTORY", TargetType = "PlayerType", PossibleTargets = { 0, 1, 2, 3 } },
    } end,
}
Game = {
    GetLocalPlayer = function() return 0 end,
    GetCurrentGameTurn = function() return 221 end,
    GetCurrentTurnSegment = function() return "TURNSEG_WORLDCONGRESS_1" end,
    GetWorldCongress = function() return wc end,
}
DB = { MakeHash = function(name) return name end }
Players = { [0] = {
    IsTurnActive = function() return false end,
    GetFavor = function() return 340 end,
    GetFavorEnteringCongress = function() return 340 end,
} }
for id = 1, 3 do
    local points = id == 3 and 15 or 11
    Players[id] = {
        GetStats = function() return { GetDiplomaticVictoryPoints = function() return points end } end,
        GetScore = function() return 1000 end,
    }
end
PlayerManager = { GetAliveMajorIDs = function() return { 0, 1, 2, 3 } end }
GameInfo = { Resolutions = { WC_RES_DIPLOVICTORY = { ResolutionType = "WC_RES_DIPLOVICTORY", Hash = 99 } } }
PlayerOperations = {
    PARAM_RESOLUTION_TYPE = "type", PARAM_WORLD_CONGRESS_VOTES = "votes",
    PARAM_RESOLUTION_OPTION = "option", PARAM_RESOLUTION_SELECTION = "selection",
    WORLD_CONGRESS_RESOLUTION_VOTE = "vote", WORLD_CONGRESS_SUBMIT_TURN = "submit",
}
local requested, submitted = nil, 0
UI = { RequestPlayerOperation = function(pid, operation, params)
    assert(pid == 0)
    if operation == "vote" then requested = params end
    if operation == "submit" then submitted = submitted + 1 end
end }
tick()
assert(type(hooks.popup) == "function", "the production popup hook must register")
local cast = upvalue(hooks.popup, "castBallot")
local vote = upvalue(cast, "voteWorldCongress")
local count, spent, _, leader, points, _, mode = vote(0)
check("actual ballot cast", count, 1)
check("actual ballot modeled cost", spent, 312)
check("actual ballot leading rival", leader, 3)
check("actual ballot rival points", points, 15)
check("actual ballot mode", mode, "claim")
check("actual request votes", requested and requested.votes, 13)
check("actual request option", requested and requested.option, 1)
check("actual request target index is our seat", requested and requested.selection, 0)
check("actual ballot submitted once", submitted, 1)

if failures > 0 then
	print(string.format("%d failure(s)", failures))
	os.exit(1)
end
print("all congress vote budget checks passed")
