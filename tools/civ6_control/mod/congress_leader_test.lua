-- Offline regression for the World Congress diplomatic-victory target.
--
-- Run: lua5.1 tools/civ6_control/mod/congress_leader_test.lua
--
-- At turn 221 of civvis-20260816T045316Z, Sweden and America were tied at
-- 17 DVP.  The old loop selected Sweden solely because it appeared first in
-- PlayerManager's list, although America had the higher score and then won by
-- advancing 17 -> 20.  Score is now the deterministic second tie-breaker.

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end
setmetatable(_G, { __index = function(_, key)
	if key == "CivvisSelectCongressLeader" then return rawget(_G, key); end
	return stub()
end })

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtimeErr = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtimeErr))

local selectLeader = rawget(_G, "CivvisSelectCongressLeader")
assert(type(selectLeader) == "function",
	"CivvisControlAgent.lua did not export CivvisSelectCongressLeader")

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", name, tostring(got)))
	end
end

local function choose(candidates)
	return selectLeader(candidates)
end

-- This is the live loss: America must win the equal-DVP tie, even when the
-- engine lists Sweden first, because America has the higher score.
local leader, points, score = choose({
	{ id = 1, points = 17, score = 968 },
	{ id = 3, points = 17, score = 995 },
})
check("equal DVP chooses higher score", leader, 3)
check("equal DVP keeps the tied points", points, 17)
check("equal DVP reports selected score", score, 995)

-- DVP remains the primary target: a lower-score rival on 18 points is closer
-- to victory than a higher-score rival on 17 points.
leader, points, score = choose({
	{ id = 3, points = 17, score = 2000 },
	{ id = 1, points = 18, score = 100 },
})
check("higher DVP beats score", leader, 1)
check("higher DVP is retained", points, 18)
check("higher DVP reports its score", score, 100)

-- A complete tie must stay deterministic across PlayerManager order, so replay
-- analysis and repeated sessions agree on the same target.
leader, points, score = choose({
	{ id = 7, points = 11, score = 999 },
	{ id = 5, points = 12, score = 500 },
	{ id = 3, points = 12, score = 500 },
})
check("full tie chooses lower player id", leader, 3)
check("full tie keeps DVP", points, 12)
check("full tie keeps score", score, 500)

-- The pure selector must be wired into the real World Congress handler and
-- leave the chosen score in telemetry, rather than becoming a dead test hook.
local src = assert(io.open(here .. "/CivvisControlAgent.lua")):read("*a")
check("world congress calls tested selector",
	src:find("CivvisSelectCongressLeader(candidates);", 1, true) ~= nil, true)
check("world congress samples DVP in the vote handler",
	src:find("GetDiplomaticVictoryPoints()", 1, true) ~= nil, true)
check("world congress returns selected score",
	src:find("return cast, spent, nil, leader, leaderPoints, leaderScore, mode;", 1, true) ~= nil,
	true)
check("world congress telemetry records score",
	src:find("leader_score = leaderScore", 1, true) ~= nil, true)

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("\nall World Congress leader-selection checks passed")
