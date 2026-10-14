-- Offline regression for `peace-asks-a-city`.
--
-- Run: lua5.1 tools/civ6_control/mod/peace_city_ask_test.lua
--
-- A peace offered from strength asks the rival to cede its nearest cedable
-- town: its own (SubType 0), valid in the host's list, not its Holy City and
-- not building a wonder. The deal calls mirror DiplomacyDealView.lua.

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end
setmetatable(_G, { __index = function(_, key)
	if key == "CivvisPeaceCityAsk" then return rawget(_G, key); end
	return stub()
end })

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtimeErr = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtimeErr))
local ask = rawget(_G, "CivvisPeaceCityAsk")
assert(type(ask) == "function", "CivvisControlAgent.lua did not export CivvisPeaceCityAsk")

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", name, tostring(got)))
	end
end

local function city(id, x, y, name, producing)
	return {
		GetID = function() return id end, GetX = function() return x end,
		GetY = function() return y end, GetName = function() return name end,
		GetOwner = function() return 1 end,
		GetBuildQueue = function() return {
			GetCurrentProductionTypeHash = function() return producing or 0 end } end,
	}
end
local ours = { city(1, 0, 0, "Bogota") }
local theirs = { [11] = city(11, 9, 0, "Far"), [12] = city(12, 4, 0, "Near"),
	[13] = city(13, 3, 0, "Holy"), [14] = city(14, 2, 0, "Wonder", 777),
	[15] = city(15, 1, 0, "Occupied") }
local possible
DealManager = { GetPossibleDealItems = function() return possible end }
DealItemTypes = { CITIES = 5 }
Players = {
	[0] = { GetCities = function() return { Members = function()
		local i = 0
		return function() i = i + 1; if ours[i] then return i, ours[i] end end
	end } end },
	[1] = { GetCities = function() return { FindID = function(_, id) return theirs[id] end } end,
		GetReligion = function() return { GetHolyCityID = function() return 13 end } end },
}
CityManager = { GetCity = function(id) return theirs[id] end }
GameInfo = { Buildings = { [777] = { IsWonder = true } } }
Map = { GetPlotDistance = function(ax, ay, bx, by) return math.abs(ax - bx) + math.abs(ay - by) end }
local added
local function deal(valid)
	added = nil
	return {
		AddItemOfType = function(_, kind, from, to, sub, value)
			added = { kind = kind, from = from, to = to, sub = sub, value = value }
			return { SetSubType = function() end, SetValueType = function() end,
				IsValid = function() return valid end, GetID = function() return 1 end }
		end,
		RemoveItemByID = function() added = nil end,
	}
end

possible = {
	{ ForType = 11, SubType = 0, IsValid = true }, { ForType = 12, SubType = 0, IsValid = true },
	{ ForType = 13, SubType = 0, IsValid = true }, { ForType = 14, SubType = 0, IsValid = true },
	{ ForType = 15, SubType = 1, IsValid = true }, { ForType = 16, SubType = 0, IsValid = false },
}
local got = ask(deal(true), 0, 1)
check("nearest cedable town", got and got.name, "Near")
check("asked from the rival", added and added.from, 1)
check("asked to us", added and added.to, 0)
check("own-town subtype", added and added.sub, 0)
check("by city id", added and added.value, 12)
check("distance", got and got.distance, 4)
possible = { { ForType = 13, SubType = 0, IsValid = true }, { ForType = 14, SubType = 0, IsValid = true } }
check("holy city and wonder skipped", ask(deal(true), 0, 1), nil)
possible = { { ForType = 12, SubType = 0, IsValid = true } }
check("an invalid item is withdrawn", ask(deal(false), 0, 1), nil)
check("nothing left in the deal", added, nil)
possible = nil
check("no list, no ask", ask(deal(true), 0, 1), nil)

if failures > 0 then error(string.format("%d peace city ask check(s) failed", failures)) end
print("peace city ask: all checks passed")
