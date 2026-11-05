-- Offline regression for `CivvisRivalWars`: each met rival's export names the
-- other majors it is at war with, among those our seat has met, and never our
-- own seat (its war with us crosses as `at_war`).
--
-- Run: lua5.1 tools/civ6_control/mod/rival_wars_test.lua
--
-- A Military Alliance's +5 holds against "units of players at war with you
-- and your ally" (LOC_ALLIANCE_LV1_MILITARY_EFFECT_1), so the ally that pays
-- at once is a rival already fighting our enemy. Before this export the
-- mirror read every rival as at peace with every other major, and no desk
-- could find one.

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end
setmetatable(_G, { __index = function(_, key)
	if key == "CivvisRivalWars" or key == "Players" or key == "PlayerManager" then
		return rawget(_G, key);
	end
	return stub()
end })

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtimeErr = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtimeErr))

local rivalWars = rawget(_G, "CivvisRivalWars")
assert(type(rivalWars) == "function", "CivvisControlAgent.lua did not export CivvisRivalWars")

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", name, tostring(got)))
	end
end

-- Seat 0 is ours. We have met 1, 2 and 3 but not 4. Player 1 is at war with
-- us, with 2 and with 4.
local met = { [1] = true, [2] = true, [3] = true }
local wars = { [1] = { [0] = true, [2] = true, [4] = true }, [2] = { [1] = true } }
local function diplomacyOf(id)
	return {
		HasMet = function(_, other) return id == 0 and met[other] == true end,
		IsAtWarWith = function(_, other) return wars[id] ~= nil and wars[id][other] == true end,
	}
end
Players = setmetatable({}, { __index = function(_, id)
	return { GetDiplomacy = function() return diplomacyOf(id) end }
end })
PlayerManager = { GetAliveMajorIDs = function() return { 0, 1, 2, 3, 4 } end }

local one = rivalWars(0, 1)
check("player 1 is at war with one other met major", #one, 1)
check("and it is player 2", one[1], 2)
local two = rivalWars(0, 2)
check("player 2's war with player 1 reads from its side too", two[1], 1)
check("player 3 fights nobody: an empty list, not nil", #rivalWars(0, 3), 0)
check("an unmet major's war is not exported", (function()
	for _, id in ipairs(one) do if id == 4 then return true end end
	return false
end)(), false)
check("our own war with them is not in the list", (function()
	for _, id in ipairs(one) do if id == 0 then return true end end
	return false
end)(), false)

-- An unreadable diplomacy is unknown, not peace.
Players = setmetatable({}, { __index = function(_, id)
	if id == 3 then return { GetDiplomacy = function() return nil end } end
	return { GetDiplomacy = function() return diplomacyOf(id) end }
end })
check("unreadable diplomacy exports nil", rivalWars(0, 3), nil)

if failures > 0 then
	print(string.format("%d failure(s)", failures))
	os.exit(1)
end
print("all rival-wars checks passed")
