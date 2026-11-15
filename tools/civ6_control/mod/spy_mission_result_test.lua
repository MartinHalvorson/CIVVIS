-- The shipped espionage ledger against a faked SpyMissionCompleted host.
-- One `spy_mission_result` row per finished mission of our seat (and of a
-- rival's against our city), read back the way EspionagePopup.lua:383 does.
-- Run: lua5.1 tools/civ6_control/mod/spy_mission_result_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."
local function stub()
	return setmetatable({}, { __index = function() return stub() end,
		__call = function() return stub() end, __newindex = function() end })
end
setmetatable(_G, { __index = function() return stub() end })
local logs = {}
Automation = { Log = function(line) logs[#logs + 1] = line end }
local registered = {}
Events = setmetatable({}, { __index = function(t, name)
	local event = setmetatable({ Add = function(handler)
		registered[name] = registered[name] or {}
		table.insert(registered[name], handler)
	end }, { __call = function() end })
	rawset(t, name, event)
	return event
end })
assert(loadfile(arg[1] or (here .. "/CivvisControlAgent.lua")))()
local ledger = rawget(_G, "CivvisLedger")
assert(type(ledger) == "table" and type(ledger.onSpyMissionCompleted) == "function")

-- Registered on the engine events the shipped popup and banners use.
assert(registered.SpyMissionCompleted and registered.SpyMissionCompleted[1] == ledger.onSpyMissionCompleted,
	"SpyMissionCompleted is not registered")
assert(registered.SpyMissionUpdated and registered.SpyMissionUpdated[1] == ledger.flushSpyMissions,
	"SpyMissionUpdated is not registered")
local turnBegin = false
for _, handler in ipairs(registered.LocalPlayerTurnBegin or {}) do
	if handler == ledger.flushSpyMissions then turnBegin = true end
end
assert(turnBegin, "an open escape is not re-read at turn begin")

-- The host.
local US, RIVAL, OTHER = 0, 2, 3
local turn = 151
Game = { GetLocalPlayer = function() return US end, GetCurrentGameTurn = function() return turn end }
Locale = { Lookup = function(s)
	return ({ LOC_CITY_NAME_BABYLON = "Babylon", LOC_CITY_NAME_ROMA = "Roma",
		LOC_CITY_NAME_ATHENS = "Athens", LOC_SPY_7 = "Agent Seven", LOC_SPY_8 = "Agent Eight" })[s] or s
end }
EspionageResultTypes = { NO_RESULT = 0, SUCCESS_UNDETECTED = 1, SUCCESS_MUST_ESCAPE = 2,
	FAIL_UNDETECTED = 3, FAIL_MUST_ESCAPE = 4, CAPTURED = 5, KILLED = 6 }
local R = EspionageResultTypes
local OPS = {
	{ OperationType = "UNITOPERATION_SPY_DISRUPT_ROCKETRY", Index = 11, Hash = -1111,
		TargetDistrict = "DISTRICT_SPACEPORT" },
	{ OperationType = "UNITOPERATION_SPY_STEAL_TECH_BOOST", Index = 5, Hash = -555,
		TargetDistrict = "DISTRICT_CAMPUS" },
	{ OperationType = "UNITOPERATION_SPY_SIPHON_FUNDS", Index = 6, Hash = -666,
		TargetDistrict = "DISTRICT_COMMERCIAL_HUB" },
}
local DISTRICTS = { [0] = { DistrictType = "DISTRICT_CITY_CENTER" }, [2] = { DistrictType = "DISTRICT_CAMPUS" },
	[4] = { DistrictType = "DISTRICT_COMMERCIAL_HUB" }, [5] = { DistrictType = "DISTRICT_SPACEPORT" } }
GameInfo = {
	UnitOperations = setmetatable({}, {
		__index = function(_, k)
			for _, op in ipairs(OPS) do
				if op.Index == k or op.OperationType == k then return op end
			end
			return nil
		end,
		__call = function()
			local i = 0
			return function() i = i + 1; return OPS[i] end
		end,
	}),
	Districts = DISTRICTS,
	Projects = { [-9090] = { ProjectType = "PROJECT_LAUNCH_EXOPLANET_EXPEDITION" } },
}

-- Plots by index: (x, y, district). Babylon (RIVAL) has a Spaceport, Roma (US)
-- a Campus, Athens (OTHER) a Commercial Hub.
local PLOTS = {
	[500] = { x = 10, y = 7, district = 0, city = "babylon" },
	[501] = { x = 11, y = 7, district = 5, city = "babylon" },
	[600] = { x = 30, y = 12, district = 0, city = "roma" },
	[601] = { x = 31, y = 12, district = 2, city = "roma" },
	[700] = { x = 50, y = 20, district = 0, city = "athens" },
	[701] = { x = 51, y = 20, district = 4, city = "athens" },
}
local host = { pillaged = {} }
local function plotObject(index)
	local p = PLOTS[index]
	if p == nil then return nil end
	return { GetX = function() return p.x end, GetY = function() return p.y end,
		GetIndex = function() return index end, GetDistrictType = function() return p.district end,
		city = p.city }
end
local function indexAt(x, y)
	for index, p in pairs(PLOTS) do
		if p.x == x and p.y == y then return index end
	end
end
local CITIES = {
	babylon = { owner = RIVAL, id = 65536, name = "LOC_CITY_NAME_BABYLON", center = 500, project = -9090 },
	roma = { owner = US, id = 131072, name = "LOC_CITY_NAME_ROMA", center = 600, project = 0 },
	athens = { owner = OTHER, id = 196608, name = "LOC_CITY_NAME_ATHENS", center = 700, project = 0 },
}
local function cityObject(key)
	local c = CITIES[key]
	local center = PLOTS[c.center]
	local districts = {}
	for index, p in pairs(PLOTS) do
		if p.city == key then
			districts[#districts + 1] = { GetType = function() return p.district end,
				GetX = function() return p.x end, GetY = function() return p.y end }
		end
	end
	return {
		GetOwner = function() return c.owner end, GetID = function() return c.id end,
		GetX = function() return center.x end, GetY = function() return center.y end,
		GetName = function() return c.name end,
		GetDistricts = function() return {
			Members = function()
				local i = 0
				return function() i = i + 1; if districts[i] then return i, districts[i] end end
			end,
			IsPillaged = function(_, districtType, plotIndex)
				assert(PLOTS[plotIndex].district == districtType, "pillage read on the wrong plot")
				return host.pillaged[plotIndex] == true
			end,
		} end,
		GetBuildQueue = function() return {
			GetCurrentProductionTypeHash = function() return c.project end,
		} end,
	}
end
Map = {
	GetPlotByIndex = plotObject,
	GetPlot = function(x, y) return plotObject(indexAt(x, y)) end,
}
Cities = { GetPlotPurchaseCity = function(plot) return plot and cityObject(plot.city) or nil end }
local missions = { [US] = {}, [RIVAL] = {} }
local units = { [US] = {} }
local function player(id)
	return {
		GetDiplomacy = function() return {
			GetMission = function(_, who, missionId)
				assert(who == id, "a mission is read from its own player's diplomacy")
				return missions[id] and missions[id][missionId] or 0
			end,
		} end,
		GetUnits = function() return { Members = function()
			local list, i = units[id] or {}, 0
			return function() i = i + 1; if list[i] then return i, list[i] end end
		end } end,
	}
end
Players = { [US] = player(US), [RIVAL] = player(RIVAL), [OTHER] = player(OTHER) }
local function spy(id, name, x, y)
	return { GetID = function() return id end, GetName = function() return name end,
		GetX = function() return x end, GetY = function() return y end }
end

local function rows(kind)
	local out = {}
	for _, line in ipairs(logs) do
		if line:find('"kind":"' .. kind .. '"', 1, true) then out[#out + 1] = line end
	end
	return out
end
local checks = 0
local function has(line, fragment)
	assert(line:find(fragment, 1, true), "missing " .. fragment .. " in " .. line)
	checks = checks + 1
end
local function lacks(line, fragment)
	assert(not line:find(fragment, 1, true), "unexpected " .. fragment .. " in " .. line)
	checks = checks + 1
end

-- 1. Our Disrupt Rocketry on Babylon's Spaceport lands undetected.
units[US] = { spy(777, "LOC_SPY_7", 11, 7), spy(778, "LOC_SPY_8", 31, 12) }
host.pillaged[501] = true
missions[US][3] = { Name = "LOC_SPY_7", CityName = "LOC_CITY_NAME_BABYLON", Operation = 11,
	PlotIndex = 501, InitialResult = R.SUCCESS_UNDETECTED, EscapeResult = R.NO_RESULT,
	LootInfo = -1, LevelAfter = 2, CompletionTurn = 151 }
ledger.onSpyMissionCompleted(US, 3)
local got = rows("spy_mission_result")
assert(#got == 1, "one row for one finished mission, got " .. #got)
for _, fragment in ipairs({ '"turn":151', '"completion_turn":151', '"player":0', '"mission_id":3',
		'"ours":true', '"operation":"UNITOPERATION_SPY_DISRUPT_ROCKETRY"',
		'"initial_result":"SUCCESS_UNDETECTED"', '"escape_result":"NO_RESULT"',
		'"succeeded":true', '"spy_fate":"undetected"', '"spy_lost":false',
		'"spy":777', '"spy_name":"Agent Seven"', '"spy_level_after":2', '"loot_info":-1',
		'"target_player":2', '"city_id":65536', '"city_name":"Babylon"', '"city_x":10', '"city_y":7',
		'"x":11', '"y":7', '"plot":501', '"district":"DISTRICT_SPACEPORT"', '"district_x":11',
		'"district_y":7', '"district_pillaged":true',
		'"target_project":"PROJECT_LAUNCH_EXOPLANET_EXPEDITION"' }) do
	has(got[1], fragment)
end
lacks(got[1], '"after_escape"')

-- 2. The same mission raised twice is written once.
ledger.onSpyMissionCompleted(US, 3)
assert(#rows("spy_mission_result") == 1, "a repeated completion must not duplicate the row")

-- 3. Detected: the escape is open, then the host settles it as a capture.
--    The Operation arrives as a hash here; the row still names it.
missions[US][4] = { Name = "LOC_SPY_8", CityName = "LOC_CITY_NAME_BABYLON", Operation = -1111,
	PlotIndex = 501, InitialResult = R.FAIL_MUST_ESCAPE, EscapeResult = R.NO_RESULT,
	LootInfo = -1, LevelAfter = 1, CompletionTurn = 151 }
host.pillaged[501] = false
ledger.onSpyMissionCompleted(US, 4)
local pending = rows("spy_mission_pending")
assert(#pending == 1 and #rows("spy_mission_result") == 1, "an open escape is pending, not a result")
has(pending[1], '"spy_fate":"escape_pending"')
has(pending[1], '"operation":"UNITOPERATION_SPY_DISRUPT_ROCKETRY"')
has(pending[1], '"spy":778')
-- Still open at SpyMissionUpdated: nothing more.
registered.SpyMissionUpdated[1]()
assert(#rows("spy_mission_pending") == 1 and #rows("spy_mission_result") == 1)
-- Captured: the Spy is gone, the escape settled; our next turn reads it.
turn = 152
missions[US][4].EscapeResult = R.CAPTURED
units[US] = { spy(777, "LOC_SPY_7", 11, 7) }
for _, handler in ipairs(registered.LocalPlayerTurnBegin) do
	if handler == ledger.flushSpyMissions then handler() end
end
got = rows("spy_mission_result")
assert(#got == 2, "the settled escape is written, got " .. #got)
for _, fragment in ipairs({ '"mission_id":4', '"turn":152', '"initial_result":"FAIL_MUST_ESCAPE"',
		'"escape_result":"CAPTURED"', '"succeeded":false', '"spy_fate":"captured"',
		'"spy_lost":true', '"after_escape":true', '"district_pillaged":false' }) do
	has(got[2], fragment)
end
lacks(got[2], '"spy":')
registered.SpyMissionUpdated[1]()
assert(#rows("spy_mission_result") == 2, "a settled mission leaves the open list")

-- 4. Success detected, escaped: the effect landed and the Spy came home.
missions[US][5] = { Name = "LOC_SPY_7", CityName = "LOC_CITY_NAME_BABYLON", Operation = 11,
	PlotIndex = 501, InitialResult = R.SUCCESS_MUST_ESCAPE, EscapeResult = R.FAIL_MUST_ESCAPE,
	LootInfo = -1, LevelAfter = 3, CompletionTurn = 152 }
ledger.onSpyMissionCompleted(US, 5)
got = rows("spy_mission_result")
assert(#got == 3)
has(got[3], '"succeeded":true')
has(got[3], '"spy_fate":"escaped"')
has(got[3], '"spy_lost":false')

-- 5. Killed outright.
missions[US][6] = { Name = "LOC_SPY_7", CityName = "LOC_CITY_NAME_BABYLON", Operation = 11,
	PlotIndex = 501, InitialResult = R.KILLED, EscapeResult = R.NO_RESULT,
	LootInfo = -1, LevelAfter = 3, CompletionTurn = 152 }
ledger.onSpyMissionCompleted(US, 6)
got = rows("spy_mission_result")
assert(#got == 4)
has(got[4], '"spy_fate":"killed"')
has(got[4], '"spy_lost":true')
has(got[4], '"succeeded":false')

-- 6. A rival's Steal Tech Boost against our Campus is ours to record...
missions[RIVAL][0] = { Name = "LOC_SPY_RIVAL", CityName = "LOC_CITY_NAME_ROMA", Operation = 5,
	PlotIndex = 601, InitialResult = R.SUCCESS_UNDETECTED, EscapeResult = R.NO_RESULT,
	LootInfo = 14, LevelAfter = 1, CompletionTurn = 152 }
ledger.onSpyMissionCompleted(RIVAL, 0)
got = rows("spy_mission_result")
assert(#got == 5)
for _, fragment in ipairs({ '"player":2', '"ours":false', '"target_player":0',
		'"operation":"UNITOPERATION_SPY_STEAL_TECH_BOOST"', '"district":"DISTRICT_CAMPUS"',
		'"loot_info":14', '"city_name":"Roma"' }) do
	has(got[5], fragment)
end
lacks(got[5], '"spy":')

-- 7. ...its mission against another rival is not.
missions[RIVAL][1] = { Name = "LOC_SPY_RIVAL", CityName = "LOC_CITY_NAME_ATHENS", Operation = 6,
	PlotIndex = 701, InitialResult = R.SUCCESS_UNDETECTED, EscapeResult = R.NO_RESULT,
	LootInfo = 120, LevelAfter = 2, CompletionTurn = 152 }
ledger.onSpyMissionCompleted(RIVAL, 1)
assert(#rows("spy_mission_result") == 5 and #rows("spy_mission_pending") == 1,
	"a rival-on-rival mission is not this seat's")

-- 8. Fail open: an unreadable mission and a host that throws write nothing and raise nothing.
ledger.onSpyMissionCompleted(US, 99)
ledger.onSpyMissionCompleted(9, 1)
registered.SpyMissionUpdated[1]("unexpected", "arguments")
assert(#rows("spy_mission_result") == 5 and #rows("spy_mission_pending") == 1)
assert(#rows("agent_error") >= 1, "a throwing host is reported, not raised")

print("spy_mission_result: all " .. checks .. " checks passed")
