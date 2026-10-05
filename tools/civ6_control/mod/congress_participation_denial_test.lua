-- Offline regression for the congress participation denial.
--
-- Run: lua5.1 tools/civ6_control/mod/congress_participation_denial_test.lua
--
-- A major gains one Diplomatic Victory point for every resolution whose
-- winning option and target match its vote. Live King civvis-20261005T060002Z
-- (game 104): Greece took four such points across turns 182-242 and won on
-- Diplomacy. Against a leader on the denial floor, the ballot votes the
-- option the leader is predicted not to take.

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end
setmetatable(_G, { __index = function(_, key)
	if key == "CivvisParticipationDenialOption" then
		return rawget(_G, key);
	end
	return stub()
end })

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtimeErr = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtimeErr))

local deny = rawget(_G, "CivvisParticipationDenialOption")
assert(type(deny) == "function",
	"CivvisControlAgent.lua did not export CivvisParticipationDenialOption")

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", name, tostring(got)))
	end
end

local last = { ["WC_RES_WORLD_IDEOLOGY:1"] = 1, ["WC_RES_PUBLIC_WORKS:1"] = 2,
	["WC_RES_HERITAGE_ORG:2"] = 1 }

-- Below the floor, the ballot is left alone.
check("below the floor", deny("WC_RES_WORLD_IDEOLOGY", 1, 0, 11, last, {}), nil)
-- From the floor, the opposite of the leader's last option on that type.
check("A last time votes B", deny("WC_RES_WORLD_IDEOLOGY", 1, 0, 12, last, {}), 2)
check("B last time votes A", deny("WC_RES_PUBLIC_WORKS", 1, 0, 16, last, {}), 1)
-- Border Control and Migration Treaty: the leader votes A on itself.
check("border control", deny("WC_RES_BORDER_CONTROL", 1, 0, 14, {}, {}), 2)
check("migration treaty", deny("WC_RES_MIGRATION_TREATY", 1, 0, 14, nil, {}), 2)
-- No prediction, no change.
check("unknown leader vote", deny("WC_RES_TRADE_TREATY", 1, 0, 14, last, {}), nil)
check("another voter's record", deny("WC_RES_HERITAGE_ORG", 1, 0, 14, last, {}), nil)
-- Never the Diplomatic Victory resolution, never ourselves, never without a leader.
check("diplomatic victory", deny("WC_RES_DIPLOVICTORY", 1, 0, 18, last, {}), nil)
check("we lead", deny("WC_RES_WORLD_IDEOLOGY", 0, 0, 18, { ["WC_RES_WORLD_IDEOLOGY:0"] = 1 }, {}), nil)
check("no leader", deny("WC_RES_WORLD_IDEOLOGY", nil, 0, 18, last, {}), nil)
-- Configurable floor and kill switch.
check("floor 15", deny("WC_RES_WORLD_IDEOLOGY", 1, 0, 14, last, { ParticipationDenialFloor = 15 }), nil)
check("off", deny("WC_RES_WORLD_IDEOLOGY", 1, 0, 18, last, { ParticipationDenial = false }), nil)

if failures > 0 then
	error(string.format("%d participation denial check(s) failed", failures))
end
print("congress participation denial: all checks passed")
