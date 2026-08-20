-- Mid-turn replan frames and the tiles delta: after the opening orders
-- settle, a board with newly revealed ground and movement left to spend on
-- it is exported again and the same turn answered again — and the revealed
-- ground crosses with it instead of waiting for the periodic sweep.
--
-- ⚠ Loads the SHIPPED `CivvisControlAgent.lua` with `ReplanFrames = 2` and
-- drives `CivvisFrames`, `CivvisTiles` and the tile exporter against a fake
-- host with a 4x3 map whose revealed set the test controls.
--
-- What is checked:
--   1. a fresh turn wants no frame: nothing revealed, nobody to move;
--   2. the delta sweep sends only plots revealed since the last board went
--      out (stamped `delta`), sends nothing when nothing changed, and
--      re-sends a plot that changed hands; the full sweep re-primes it;
--   3. revealed ground + a unit with movement opens a `replan_frame` with
--      reason `revealed`, re-arms the queue's tick budget and restarts the
--      counters; the cap holds after `ReplanFrames` frames;
--   4. revealed ground with NO movement left opens nothing;
--   5. a strike still opens a `combat_frame` under `ReplanFrames` alone;
--   6. with `ReplanFrames` unset, `observe` is inert and a reveal opens nothing.
--
-- Run: lua5.1 tools/civ6_control/mod/replan_frame_test.lua

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
local revealed, owner = {}, {}
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
	return { IsVisible = function() return true end,
	         IsRevealed = function(_, x, y) return revealed[key(x, y)] == true end }
end })
Game = { GetLocalPlayer = function() return PID end, GetCurrentGameTurn = function() return 12 end }

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtime_err = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtime_err))

local frames = rawget(_G, "CivvisFrames")
local tiles = rawget(_G, "CivvisTiles")
local queue = rawget(_G, "CivvisQueue")
local ledger = rawget(_G, "CivvisLedger")
local exportTiles = rawget(_G, "CivvisExportTiles")
rawget(_G, "CivvisResolveActions")()
assert(type(frames) == "table", "CivvisFrames is not exported")
assert(type(tiles) == "table", "CivvisTiles is not exported")
assert(type(exportTiles) == "function", "CivvisExportTiles is not exported")

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", name, tostring(got)))
	end
end
local function count(kind)
	local n = 0
	for _, line in ipairs(LOG) do if line:find('"kind":"' .. kind .. '"', 1, true) then n = n + 1 end end
	return n
end
local function lastEvent(kind)
	for i = #LOG, 1, -1 do if LOG[i]:find('"kind":"' .. kind .. '"', 1, true) then return LOG[i] end end
	return nil
end
local function has(line, needle) return line ~= nil and line:find(needle, 1, true) ~= nil end
-- `begin` pcalls the full board export; keep it off around the call.
local function beginFrame(turn)
	CivvisControlConfig.ExportState = false
	frames.begin(player, PID, turn)
	CivvisControlConfig.ExportState = true
end

-- 1. A fresh turn wants nothing.
frames.reset()
frames.observe(player, PID, 12)
check("fresh turn: nothing revealed", frames.revealed, 0)
check("fresh turn: no frame wanted", frames.wanted(), false)

-- 2. The delta sweep.
revealed[key(0, 0)] = true; revealed[key(1, 0)] = true; revealed[key(1, 1)] = true
local before = count("tiles")
check("first delta sends every revealed plot", tiles.sweep(player, PID, 12, 1), 3)
check("…as one tiles chunk", count("tiles"), before + 1)
check("…stamped delta", has(lastEvent("tiles"), '"delta":true'), true)
check("…with the frame", has(lastEvent("tiles"), '"frame":1'), true)
check("…and a tiles_delta summary", has(lastEvent("tiles_delta"), '"plots":3'), true)
before = count("tiles")
check("nothing new: nothing sent", tiles.sweep(player, PID, 12, 1), 0)
check("…no tiles chunk", count("tiles"), before)
revealed[key(2, 2)] = true
check("one new plot: one plot sent", tiles.sweep(player, PID, 12, 2), 1)
check("…and it is that plot", has(lastEvent("tiles"), '"x":2,"y":2'), true)
owner[key(0, 0)] = 3
check("a plot that changed hands is re-sent", tiles.sweep(player, PID, 13, 0), 1)
check("…with its new owner", has(lastEvent("tiles"), '"o":3'), true)
-- The full sweep (turn 25 on the cadence) re-primes `known`: the next delta
-- is empty, and the full chunks carry no delta stamp.
before = count("tiles_done")
exportTiles(player, PID, 25)
check("the full sweep still runs on its cadence", count("tiles_done"), before + 1)
check("…unstamped", has(lastEvent("tiles"), '"delta"'), false)
check("…and re-primes the delta", tiles.sweep(player, PID, 25, 1), 0)
-- Between sweeps, the ordinary turn-start call is a delta.
revealed[key(3, 2)] = true
before = count("tiles")
check("turn 26's exporter call is the delta", exportTiles(player, PID, 26), 1)
check("…one chunk", count("tiles"), before + 1)
check("…stamped delta, frame 0", has(lastEvent("tiles"), '"delta":true,"frame":0'), true)
CivvisControlConfig.TileDelta = false
revealed[key(0, 2)] = true
check("TileDelta=false withholds the delta", exportTiles(player, PID, 27), 0)
CivvisControlConfig.TileDelta = nil

-- 3. Revealed ground plus movement opens a replan frame; the cap holds.
frames.reset()
host.units[5] = { id = 5, kind = "UNIT_SCOUT", x = 1, y = 1, moves = 2 }
revealed[key(3, 0)] = true
queue.ticks = 99
frames.observe(player, PID, 12)
-- Two: the plot revealed while the delta was withheld was deferred, not lost.
check("observe counts the revealed plots (a withheld one included)", frames.revealed, 2)
check("observe counts the mover", frames.movers, 1)
check("revealed ground with a mover: reason", frames.why(), "revealed")
beginFrame(12)
check("frame 1 opened", frames.current, 1)
check("replan_frame emitted", has(lastEvent("replan_frame"), '"reason":"revealed"'), true)
check("…naming what it saw", has(lastEvent("replan_frame"), '"revealed":2'), true)
check("the queue's tick budget is re-armed", queue.ticks, 0)
check("the revealed counter starts again", frames.revealed, 0)
frames.observe(player, PID, 12)
check("nothing new after the frame: nothing wanted", frames.wanted(), false)
revealed[key(3, 1)] = true
frames.observe(player, PID, 12)
check("a second reveal wants frame 2", frames.wanted(), true)
beginFrame(12)
check("frame 2 opened", frames.current, 2)
revealed[key(2, 0)] = true
frames.observe(player, PID, 12)
check("the cap holds at ReplanFrames", frames.wanted(), false)

-- 4. Revealed ground with nobody to move on it opens nothing.
frames.reset()
host.units[5].moves = 0
revealed[key(2, 1)] = true
frames.observe(player, PID, 12)
check("no movement left: revealed but not wanted", frames.revealed > 0 and not frames.wanted(), true)
host.units[5].moves = 2

-- 5. A strike still opens a combat frame under ReplanFrames alone.
frames.reset()
ledger.strike(UnitManager.GetUnit(0, 5), 5, "RANGE_ATTACK", 2, 1, 12)
check("a strike under ReplanFrames: reason", frames.why(), "strike")
beginFrame(12)
check("…opens a combat_frame", has(lastEvent("combat_frame"), '"reason":"strike"'), true)

-- 6. With ReplanFrames unset, observe is inert and a reveal opens nothing.
CivvisControlConfig.ReplanFrames = nil
frames.reset()
revealed[key(0, 1)] = true
frames.observe(player, PID, 12)
check("default off: observe counts nothing", frames.revealed, 0)
check("default off: no frame wanted", frames.wanted(), false)

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("\nall replan-frame checks passed")
