-- Offline regression for the World Congress vote budget.
--
-- Run: lua5.1 tools/civ6_control/mod/congress_vote_budget_test.lua
--
-- Seventeen multi-vote ballots across four runs were refused whole while
-- ninety-five one-vote ballots registered, and every refused ask saturated
-- the bank priced by the host's own `GetVotesandFavorCost` table -- the
-- ONLINE curve, cumulative `2n(n-1)`.  The Standard curve charges
-- `5n(n-1)`; a core that charges Standard while the accessor reports Online
-- refuses every ask this seat ever made, and no recorded ballot could tell,
-- because none ever asked a count that fits both tables.
-- `CivvisCongressVoteBudget` caps the ask by both walks; these are the
-- recorded sessions it must reprice.

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

-- Run civvis-20260819T004405Z turn 222: 791 Favor, MaxVotes 20.  The old
-- walk asked 20 (Online price 760) and the host recorded one; 13 votes is
-- the largest ask that also fits the Standard table (5*13*12 = 780).
local votes, host, standard = voteBudget(791, onlineCosts(20), 20)
check("t222 ask fits both tables", votes, 13)
check("t222 host walk still reads the bank", host, 20)
check("t222 standard walk stops at 780", standard, 13)

-- Run civvis-20260818T175125Z turn 162: 352 Favor, MaxVotes 13.  The old
-- walk asked 13 (Online 312, refused); 8 votes cost 280 Standard.
votes, host, standard = voteBudget(352, onlineCosts(13), 13)
check("t162 ask", votes, 8)
check("t162 host walk", host, 13)
check("t162 standard walk", standard, 8)

-- A first-session probe bank: 52 Favor buys 5 votes Online but only 3
-- Standard, so the probe's three votes fit both tables from turn 61 on.
votes, host, standard = voteBudget(52, onlineCosts(10), 10)
check("first-session ask", votes, 3)
check("first-session host walk", host, 5)
check("first-session standard walk", standard, 3)

-- Exact Standard boundary: 780 affords the 13th vote, 779 does not.
votes = voteBudget(780, onlineCosts(20), 20)
check("standard boundary at 780", votes, 13)
votes = voteBudget(779, onlineCosts(20), 20)
check("standard boundary at 779", votes, 12)

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

if failures > 0 then
	print(string.format("%d failure(s)", failures))
	os.exit(1)
end
print("all congress vote budget checks passed")
