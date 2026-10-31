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

-- Run: lua5.1 tools/civ6_control/mod/congress_unmet_target_test.lua
-- `CivvisCongressTargetIndex` / `CivvisCongressDenialTarget`: a ballot names
-- only a target the ballot offers, and never falls back to option B on us.
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
local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print("ok " .. name)
	end
end

-- The live 003455Z t137 ballot: we are 0 and have met 2 and 3; the leader,
-- 1, is not offered. The ballot's targets are 1-indexed, us first.
local met = { "0", "2", "3" }
local everyone = { "0", "1", "2", "3", "4", "5" }
local candidates = {
	{ id = 1, points = 6 }, { id = 2, points = 3 }, { id = 3, points = 2 },
	{ id = 4, points = 1 }, { id = 5, points = 6 },
}

check("an offered player's index", CivvisCongressTargetIndex(met, 3), 3)
check("an unoffered player has none", CivvisCongressTargetIndex(met, 1), nil)
check("no player, no index", CivvisCongressTargetIndex(met, nil), nil)
check("no ballot, no index", CivvisCongressTargetIndex(nil, 2), nil)

local option, selection = CivvisCongressDenialTarget(everyone, 1, candidates, 0, 2, 1)
check("an offered leader keeps option B", option, 2)
check("an offered leader is named", everyone[selection], "1")

option, selection = CivvisCongressDenialTarget(met, 1, candidates, 0, 2, 1)
check("an unoffered leader: B on the offered rival with the most points", option, 2)
check("that rival is 2", met[selection], "2")
check("never the default, us", met[selection] ~= "0", true)

option, selection = CivvisCongressDenialTarget({ "0" }, 1, candidates, 0, 2, 1)
check("no rival offered: option A", option, 1)
check("on us", ({ "0" })[selection], "0")

option, selection = CivvisCongressDenialTarget({}, 1, candidates, 0, 2, 1)
check("an empty ballot changes nothing", option, 2)
check("nor the selection", selection, 1)

-- The ballot uses the guards on all three player-target paths.
local source = assert(io.open(here .. "/CivvisControlAgent.lua")):read("*a")
check("the Diplomatic Victory denial uses the guard",
	source:find("option, selection = CivvisCongressDenialTarget(targets, leader,", 1, true) ~= nil, true)
check("the counter path drops an unoffered threat",
	source:find("if counter and CivvisCongressTargetIndex(targets, threat) == nil then", 1, true) ~= nil, true)
check("the participation denial drops an unoffered leader",
	source:find("and CivvisCongressTargetIndex(targets, leader) == nil then\n\t\t\t\t\t\tdeny = nil;", 1, true) ~= nil, true)

if failures > 0 then
	print(failures .. " FAILURES")
	os.exit(1)
end
print("all congress unmet target checks passed")
