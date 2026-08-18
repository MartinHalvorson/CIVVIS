-- Offline regression for the World Congress penalty-ballot target.
--
-- Run: lua5.1 tools/civ6_control/mod/victory_threat_test.lua
--
-- Every player-targeted resolution except the Diplomatic Victory one used to
-- select THIS SEAT with option 1 -- the option that buffs its target -- so the
-- three ballots that carry a real penalty were spent on a small bonus for us
-- while the civilization about to end the game took nothing. Over the 39 live
-- games of 2026-08-16/17 that is 232 wasted ballots, about six a game, against
-- a record in which diplomatic (32) and culture (27) victories are 83% of
-- every game a rival ended before the turn cap.
--
-- `CivvisSelectVictoryThreat` is the pure half of the repair: given each
-- rival's progress toward whichever victory it is closest to, name the one a
-- penalty ballot should punish.

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end
setmetatable(_G, { __index = function(_, key)
	if key == "CivvisSelectVictoryThreat" then return rawget(_G, key); end
	return stub()
end })

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtimeErr = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtimeErr))

local selectThreat = rawget(_G, "CivvisSelectVictoryThreat")
assert(type(selectThreat) == "function",
	"CivvisControlAgent.lua did not export CivvisSelectVictoryThreat")

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", name, tostring(got)))
	end
end

-- The culture leader outranks a diplomatic one that is further from winning.
-- This is the case the whole change exists for: culture ends more of our games
-- than anything but diplomacy, and racing it is not available when a third
-- civilization holds the domestic-tourist bar.
local id, progress = selectThreat({
	{ id = 3, progress = 40, score = 900 },   -- diplomatic, 8 of 20 points
	{ id = 5, progress = 85, score = 700 },   -- culture, 85% of the bar
})
check("culture leader is named over a distant diplomat", id, 5)
check("and its progress is reported", progress, 85)

-- A tie on progress breaks on score, exactly as the diplomatic selector does:
-- player-manager order is not a strategic signal.
check("equal progress breaks on score", (selectThreat({
	{ id = 2, progress = 70, score = 500 },
	{ id = 7, progress = 70, score = 950 },
})), 7)

-- Equal on both, the lower id wins, so the choice is deterministic across
-- turns rather than dependent on table order.
check("equal progress and score break on id", (selectThreat({
	{ id = 9, progress = 70, score = 500 },
	{ id = 4, progress = 70, score = 500 },
})), 4)

-- No candidates is not a crash and not a target: the caller's bar check must
-- see something it can refuse.
check("an empty field names nobody", (selectThreat({})), -1)
check("a nil field names nobody", (selectThreat(nil)), -1)

-- Malformed rows are skipped rather than voted on. The congress table arrives
-- from the host and a resolution the DB does not know is left alone elsewhere
-- in this file for the same reason.
check("garbage rows are skipped", (selectThreat({
	"not a table",
	{ id = nil, progress = 99 },
	{ id = 6, progress = 65, score = 100 },
})), 6)

-- ⚠ The selector ranks; it does not decide. A rival at 5% is still "closest",
-- and the caller is what refuses to spend a ballot on it (CounterResolutionBar,
-- default 60). Asserted so nobody moves the bar into here by mistake.
check("a trivial leader is still ranked, and left to the caller", (selectThreat({
	{ id = 8, progress = 5, score = 10 },
})), 8)

if failures > 0 then
	print(string.format("%d failure(s)", failures))
	os.exit(1)
end
print("all victory-threat checks passed")
