-- Exercise the actual opening-turn closure at the event that wakes the
-- Python worker. Instrument only its exporters, including multi-chunk,
-- empty-delta and failed-terrain cases. Fake host follows replan_frame_test.
-- Run with Lua 5.1.
local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end

local EXPORTS = { CivvisApplyOrders = true, CivvisQueue = true, CivvisResolveActions = true,
                  CivvisFrames = true, CivvisTiles = true, CivvisLedger = true,
                  CivvisExportTiles = true, CivvisOrdersReady = true, CivvisFetchOrders = true }
local LOG = {}
Automation = { Log = function(line) LOG[#LOG + 1] = line end }
-- `ExportState` is ON so the tile exporter runs; the full board export is
-- never invoked here (CivvisFrames.begin pcalls it and we switch the flag
-- off around `begin`, because that export walks the whole host API).
CivvisControlConfig = { ReplanFrames = 2, ExportState = true, TileExportEvery = 25,
                        OrdersDb = "/tmp/fake.sqlite", RunTag = "test-run" }
UnitOperationTypes = { PARAM_X = "x", PARAM_Y = "y" }
UnitCommandTypes = {}

-- A 4x3 map. `revealed[key]` and `owner[key]` are what the host would answer.
local W, H = 4, 3
local revealed, owner, visible = {}, {}, {}
local function key(x, y) return y * W + x end
local function plotObject(x, y)
	return {
		GetX = function() return x end, GetY = function() return y end,
		GetOwner = function() return owner[key(x, y)] or -1 end,
		GetTerrainType = function() return -1 end, GetFeatureType = function() return -1 end,
		GetResourceType = function() return -1 end, GetImprovementType = function() return -1 end,
		GetDistrictType = function() return -1 end, GetWonderType = function() return -1 end,
		GetRouteType = function() return -1 end, GetContinentType = function() return -1 end,
		IsWater = function() return false end, IsImpassable = function() return false end,
		IsFreshWater = function() return false end, IsRiver = function() return false end,
		IsWOfRiver = function() return false end, IsNWOfRiver = function() return false end,
		IsNEOfRiver = function() return false end, IsRoutePillaged = function() return false end,
		IsImprovementPillaged = function() return false end,
	}
end
Map = { GetPlotDistance = function(x1, y1, x2, y2) return math.max(math.abs(x1 - x2), math.abs(y1 - y2)) end,
        GetGridSize = function() return W, H end,
        GetPlot = function(x, y) return plotObject(x, y) end,
        GetAdjacentPlot = function() return nil end }
TerrainManager = { GetCoastalLowlandType = function() return -1 end }
local function emptyTable()
	return setmetatable({}, {
		__index = function() return nil end,
		__call = function() return function() return nil end end,
	})
end
GameInfo = setmetatable({}, { __index = function(_, k)
	if k == "UnitOperations" or k == "UnitCommands" then
		return setmetatable({}, { __index = function(_, name) return { Hash = name } end })
	end
	if k == "Units" then
		return setmetatable({}, { __index = function(_, name) return { UnitType = name, Combat = 20, RangedCombat = 0 } end })
	end
	return emptyTable()
end })
local channel = { ready = {}, orders = {} }
DB = { Query = function(sql)
	if sql:find("civvis.ready", 1, true) then return channel.ready end
	if sql:find("civvis.orders", 1, true) then return channel.orders end
	return {}
end }
setmetatable(_G, { __index = function(_, k)
	if EXPORTS[k] then return rawget(_G, k) end
	return stub()
end })

local host = { units = {}, ops = {} }
local PID = 0
local function unitObject(u)
	return {
		GetID = function() return u.id end, GetX = function() return u.x end, GetY = function() return u.y end,
		GetMovesRemaining = function() return u.moves end, GetUnitType = function() return u.kind end,
		GetType = function() return u.kind end, GetDamage = function() return 0 end,
		GetGreatPerson = function() return nil end, GetFortifyTurns = function() return 0 end,
		GetFormationUnitCount = function() return 1 end,
		GetComponentID = function() return { player = 0, id = u.id } end,
	}
end
UnitManager = {
	GetUnit = function(pid, id) local u = host.units[id]; return u and unitObject(u) or nil end,
	CanStartOperation = function() return true end,
	RequestOperation = function(unit, hash, params) host.ops[#host.ops + 1] = { id = unit.GetID(), op = hash } end,
	CanStartCommand = function() return false end, RequestCommand = function() end,
}
local function members(list)
	return function()
		local i = 0
		return function() i = i + 1; if list[i] == nil then return nil end; return i, list[i] end
	end
end
local player = setmetatable({
	GetUnits = function()
		local objs = {}
		for _, u in pairs(host.units) do objs[#objs + 1] = unitObject(u) end
		table.sort(objs, function(a, b) return a.GetID() < b.GetID() end)
		return { Members = members(objs) }
	end,
	GetCities = function() return { Members = members({}) } end,
	GetDiplomacy = function() return { IsAtWarWith = function() return false end } end,
	GetResources = function() return { IsResourceVisible = function() return false end } end,
	GetScore = function() return 0 end,
	GetTreasury = function() return { GetGoldBalance = function() return 0 end } end,
	IsTurnActive = function() return true end,
}, { __index = function() return stub() end })
Players = setmetatable({}, { __index = function(_, pid)
	if pid == PID then return player end
	return setmetatable({ IsBarbarian = function() return true end,
		GetUnits = function() return { Members = members({}) } end,
		GetCities = function() return { Members = members({}) } end }, { __index = function() return stub() end })
end })
PlayerManager = { GetAliveIDs = function() return { PID, 63 } end, GetAliveMajorIDs = function() return { PID } end }
PlayersVisibility = setmetatable({}, { __index = function()
	return { IsVisible = function(_, x, y) return visible[key(x, y)] ~= false end,
	         IsRevealed = function(_, x, y) return revealed[key(x, y)] == true end }
end })
Game = { GetLocalPlayer = function() return PID end, GetCurrentGameTurn = function() return 12 end }

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, err); assert(pcall(chunk))
local begin = assert(rawget(_G, "CivvisBeginTurn"))
local published = {}
local function replace(name, replacement)
 for i = 1, 250 do
  local n = debug.getupvalue(begin, i)
  if n == nil then break end
  if n == name then debug.setupvalue(begin, i, replacement); return end
 end
 error("missing actual exporter upvalue: " .. name)
end
replace("exportTiles", function()
 published[#published + 1] = "tiles-1"
 published[#published + 1] = "tiles-2"
 published[#published + 1] = "tiles-complete"
end)
replace("exportState", function()
 published[#published + 1] = "state"
 -- Stop at the exact event that wakes the real Python worker. Everything
 -- after this point is too late for an already-captured native decision.
 error("consumer-wakes-on-state")
end)
local ok, why = pcall(begin, player, PID, 5)
assert(not ok and tostring(why):find("consumer-wakes-on-state", 1, true), tostring(why))
assert(published[1] == "tiles-1", "consumer woke on state before the new terrain export")
assert(published[2] == "tiles-2" and published[3] == "tiles-complete" and published[4] == "state", "the state is published after the complete tile exporter returns")
published = {}
replace("exportTiles", function()
 published[#published + 1] = "partial-tiles"
 error("terrain-export-failed")
end)
ok, why = pcall(begin, player, PID, 6)
assert(not ok and tostring(why):find("terrain-export-failed", 1, true), tostring(why))
assert(#published == 1 and published[1] == "partial-tiles",
 "a terrain export failure cannot publish a decision-triggering state")
published = {}
replace("exportTiles", function()
 published[#published + 1] = "empty-delta-complete"
 return 0
end)
ok, why = pcall(begin, player, PID, 7)
assert(not ok and tostring(why):find("consumer-wakes-on-state", 1, true), tostring(why))
assert(published[1] == "empty-delta-complete" and published[2] == "state",
 "an unchanged map still publishes its new city/unit board")
print("opening board publishes all terrain before state, including empty and failed exports")
