-- CIVVIS orders are picked up within `OrdersPeekSeconds` of landing, not at
-- the next `OrdersPollTicks` poll: while a board is out, each game-core
-- publish batch may peek at the channel's `ready` marker, wall-clock bounded.
--
-- Loads the SHIPPED `CivvisControlAgent.lua` on the `step_turn_actions_test`
-- host with `OrdersPollTicks = 2`, a controllable UI clock, and the event
-- handlers captured, then checks:
--   1. no board out (turn settled) -> no peek, no query;
--   2. board out, no answer -> at most one `ready` query per 50 ms of clock,
--      however many publish batches arrive, and the turn is not released;
--   3. answer lands -> the next due batch runs the tick and its poll reads
--      the answer although the 2-tick cadence would have skipped it;
--   4. a forced poll is consumed once; the cadence is otherwise unchanged.
--
-- Run: lua5.1 tools/civ6_control/mod/orders_peek_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end

local EXPORTS = { CivvisApplyOrders = true, CivvisQueue = true, CivvisResolveActions = true,
                  CivvisFrames = true, CivvisTiles = true, CivvisLedger = true, CivvisBoard = true,
                  CivvisExportTiles = true, CivvisOrdersReady = true, CivvisFetchOrders = true,
                  CivvisApplyOrder = true, CivvisBeginTurn = true, CivvisSettleTurn = true,
                  CivvisSurvey = true }
local LOG = {}
Automation = { Log = function(line) LOG[#LOG + 1] = line end }
-- `ExportState` OFF: the full board export walks the whole host API and
-- this stub does not model it. The TILE exporter — the frame trigger — is
-- the one export the test needs, so `CivvisTiles.sweep` is wrapped below to
-- switch the flag on around itself.
CivvisControlConfig = { ReplanFrames = 2, ExportState = false, TileExportEvery = 25,
                        OrdersPollTicks = 2, CombatFramePolls = 20, OrdersWaitPolls = 40,
                        OrderQueueGraceTicks = 30, OrderQueueMaxTicks = 240,
                        CivvisDecides = true, OrdersDb = "/tmp/fake.sqlite", RunTag = "test-run" }
UnitOperationTypes = { PARAM_X = "x", PARAM_Y = "y", FOUND_CITY = "UNITOPERATION_FOUND_CITY" }
UnitCommandTypes = {}
OperationResultsTypes = { ALL = 1 }
UnitOperationResults = { FAILURE_REASONS = "reasons" }

-- An 8x4 map. `revealed[key]` is what the host answers; a unit that lands
-- on a hex reveals the ring around it.
local W, H = 8, 4
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
local function dist(x1, y1, x2, y2) return math.max(math.abs(x1 - x2), math.abs(y1 - y2)) end
local function reveal(x, y)
	for dy = -1, 1 do for dx = -1, 1 do
		local px, py = x + dx, y + dy
		if px >= 0 and px < W and py >= 0 and py < H then revealed[key(px, py)] = true end
	end end
end
Map = { GetPlotDistance = function(x1, y1, x2, y2) return dist(x1, y1, x2, y2) end,
        GetGridSize = function() return W, H end,
        GetPlot = function(x, y) return plotObject(x, y) end,
        GetPlotIndex = function(x, y) return key(x, y) end,
        GetPlotByIndex = function(index) return plotObject(index % W, math.floor(index / W)) end,
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
		return setmetatable({}, { __index = function(_, name)
			if name == "UNIT_SETTLER" then return { UnitType = name, Combat = 0, RangedCombat = 0 } end
			if name == "UNIT_ARCHER" then return { UnitType = name, Combat = 15, RangedCombat = 25 } end
			return { UnitType = name, Combat = 20, RangedCombat = 0 }
		end })
	end
	return emptyTable()
end })
-- The orders channel: the test writes what the brain would have answered.
local channel = { ready = {}, orders = {} }
local readyQueries = 0
DB = { Query = function(sql)
	if sql:find("civvis.ready", 1, true) then readyQueries = readyQueries + 1; return channel.ready end
	if sql:find("civvis.orders", 1, true) then return channel.orders end
	return {}
end }
setmetatable(_G, { __index = function(_, k)
	if EXPORTS[k] then return rawget(_G, k) end
	return stub()
end })

-- ---------------------------------------------------------------- fake host
-- MOVE_TO walks the unit as far as its movement allows along a straight
-- line (one hex per movement point), spends it, and reveals the ring where
-- it lands. `GetMoveToPathEx` prices the same line: hexes within the unit's
-- movement land this turn, the rest next turn, so `CivvisBoard.capToTurn`
-- caps exactly as on the real host.
local host = { units = {}, ops = {}, founded = {} }
local PID = 0
local function unitObject(u)
	return {
		GetID = function() return u.id end, GetX = function() return u.x end, GetY = function() return u.y end,
		GetMovesRemaining = function() return u.moves end, GetUnitType = function() return u.kind end,
		GetType = function() return u.kind end, GetDamage = function() return 0 end,
		GetGreatPerson = function() return nil end, GetFortifyTurns = function() return 0 end,
		GetFormationUnitCount = function() return 1 end,
		GetAttacksRemaining = function() return u.attacks or 1 end,
		GetComponentID = function() return { player = 0, id = u.id } end,
	}
end
local function line(u, x, y)
	local plots, turns = { key(u.x, u.y) }, { 0 }
	local cx, cy, steps = u.x, u.y, 0
	while cx ~= x or cy ~= y do
		if cx < x then cx = cx + 1 elseif cx > x then cx = cx - 1 end
		if cy < y then cy = cy + 1 elseif cy > y then cy = cy - 1 end
		steps = steps + 1
		plots[#plots + 1] = key(cx, cy)
		turns[#turns + 1] = (steps <= (u.moves or 0)) and 1 or 2
	end
	return plots, turns
end
function host.walk(u, x, y)
	local plots, turns = line(u, x, y)
	for i = 2, #plots do
		if turns[i] <= 1 then
			u.x, u.y = plots[i] % W, math.floor(plots[i] / W)
			u.moves = u.moves - 1
			reveal(u.x, u.y)
		end
	end
end
function host.land(id)
	local u = host.units[id]
	if u.pendingX ~= nil then
		local x, y = u.pendingX, u.pendingY
		u.pendingX, u.pendingY = nil, nil
		host.walk(u, x, y)
	end
end
UnitManager = {
	GetUnit = function(pid, id)
		local u = host.units[id]
		if u == nil or u.gone then return nil end
		return unitObject(u)
	end,
	CanStartOperation = function(unit, hash, _, params)
		local u = host.units[unit.GetID()]
		if hash == "UNITOPERATION_FOUND_CITY" and (u.moves or 0) <= 0 then return false end
		return true
	end,
	RequestOperation = function(unit, hash, params)
		local u = host.units[unit.GetID()]
		host.ops[#host.ops + 1] = { id = u.id, op = hash,
			x = params and params.x or nil, y = params and params.y or nil }
		if hash == "UNITOPERATION_MOVE_TO" then
			if host.slow then
				-- Asynchronous, like the host: the walk lands when the test
				-- says so (`host.land`).
				u.pendingX, u.pendingY = params.x, params.y
				return
			end
			host.walk(u, params.x, params.y)
		elseif hash == "UNITOPERATION_RANGE_ATTACK" then
			u.moves = 0; u.attacks = 0
		elseif hash == "UNITOPERATION_FOUND_CITY" then
			host.founded[#host.founded + 1] = { id = u.id, x = u.x, y = u.y }
			u.gone = true
		end
	end,
	GetMoveToPathEx = function(unit, plotIndex)
		local u = host.units[unit.GetID()]
		local plots, turns = line(u, plotIndex % W, math.floor(plotIndex / W))
		return { plots = plots, turns = turns }
	end,
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
		for _, u in pairs(host.units) do if not u.gone then objs[#objs + 1] = unitObject(u) end end
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
CombatManager = { SimulateAttackInto = function() return nil end }
Locale = { Lookup = function(s) return tostring(s) end }

local clock = 100.0
UI = setmetatable({ GetElapsedTime = function() return clock end },
	{ __index = function() return stub() end })
local handlers = {}
Events = setmetatable({}, { __index = function(_, name)
	return { Add = function(handler) handlers[name] = handler end }
end })
local luaHandlers = {}
LuaEvents = setmetatable({}, { __index = function(_, name)
	return { Add = function(handler) luaHandlers[name] = handler end }
end })
local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtime_err = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtime_err))

local frames = rawget(_G, "CivvisFrames")
local tiles = rawget(_G, "CivvisTiles")
local queue = rawget(_G, "CivvisQueue")
local applyOrders = rawget(_G, "CivvisApplyOrders")
local beginTurn = rawget(_G, "CivvisBeginTurn")
local settleTurn = rawget(_G, "CivvisSettleTurn")
rawget(_G, "CivvisResolveActions")()
assert(type(frames) == "table", "CivvisFrames is not exported")
assert(type(beginTurn) == "function", "CivvisBeginTurn is not exported")
assert(type(settleTurn) == "function", "CivvisSettleTurn is not exported")
-- The delta sweep runs with the export flag on; everything else sees it off.
local sweep = tiles.sweep
tiles.sweep = function(...)
	CivvisControlConfig.ExportState = true
	local ok, fresh = pcall(sweep, ...)
	CivvisControlConfig.ExportState = false
	assert(ok, tostring(fresh))
	return fresh
end

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
	for _, l in ipairs(LOG) do if l:find('"kind":"' .. kind .. '"', 1, true) then n = n + 1 end end
	return n
end
local function lastEvent(kind)
	for i = #LOG, 1, -1 do if LOG[i]:find('"kind":"' .. kind .. '"', 1, true) then return LOG[i] end end
	return nil
end
local function has(l, needle) return l ~= nil and l:find(needle, 1, true) ~= nil end
local function ops(id, op)
	local n = 0
	for _, o in ipairs(host.ops) do if o.id == id and (op == nil or o.op == op) then n = n + 1 end end
	return n
end
local function row(seq, subject, verb, x, y, frame)
	return { seq = seq, kind = "unit", subject = subject, verb = verb, x = x, y = y, frame = frame }
end
-- Answer the board the mod is waiting on (frame N) and tick `settleTurn`
-- until it returns true or the budget is spent.
local function answer(turn, frame, rows)
	channel.ready = { { run = "test-run", turn = turn, count = #rows, frame = frame } }
	channel.orders = rows
end
local function settle(turn, budget)
	for _ = 1, (budget or 400) do
		if settleTurn(player, PID, turn, function() end) then return true end
	end
	return false
end
-- Open a turn the way the tick does, then prime the tile delta as the
-- turn-start export would (it runs with the flag off here).
local function openTurn(turn)
	beginTurn(player, PID, turn)
	tiles.sweep(player, PID, turn, 0)
end

local publish = handlers.GameCoreEventPublishComplete
check("the publish-batch handler is registered", type(publish), "function")
local landed = queue.ordersLanded
check("CivvisQueue.ordersLanded is exported", type(landed), "function")

-- 0. The HUD clock's peek (`CivvisControlPeek`): a quiet game core publishes
-- no batch, so the same bounded peek is offered from the UI clock. First, on
-- turns 3 and 4: this host has no cities, and past turn 5 the agent reports
-- the seat eliminated and stops ticking (`finished`), which the peek honours.
local uiPeek = luaHandlers.CivvisControlPeek
check("CivvisControlPeek is registered", type(uiPeek), "function")
local function wakes() return count("orders_peek_wake") end
-- 0a. No board out: nothing is asked and nothing wakes.
host.units = { [7] = { id = 7, kind = "UNIT_SCOUT", x = 1, y = 1, moves = 3 } }
host.ops = {}
openTurn(3)
answer(3, 0, { row(0, 7, "FORTIFY", nil, nil, 0) })
check("turn 3 settles", settle(3, 50), true)
readyQueries = 0
clock = clock + 1
uiPeek()
check("a settled turn: the UI peek asks nothing", readyQueries, 0)
check("…and wakes nothing", wakes(), 0)
-- 0b. Board out, no answer: one bounded query, no wake.
host.units = { [8] = { id = 8, kind = "UNIT_SCOUT", x = 1, y = 1, moves = 3 } }
host.ops = {}
channel.ready = {}; channel.orders = {}
openTurn(4)
readyQueries = 0
clock = clock + 1
uiPeek()
check("no answer yet: one query", readyQueries, 1)
check("…no wake", wakes(), 0)
-- 0c. The answer lands; within the shared 50 ms budget nothing is asked.
answer(4, 0, { row(0, 8, "MOVE_TO", 2, 1, 0) })
clock = clock + 0.02
uiPeek()
check("inside the shared 50 ms budget: no query", readyQueries, 1)
check("…no wake", wakes(), 0)
-- 0d. The next due UI peek finds it, says so, and forces the poll.
local beforeForce = queue.forcePoll
clock = clock + 0.05
queue.lastTickAt = clock - 0.75              -- the core went quiet 0.75 s ago
uiPeek()
check("the due UI peek queries once more", readyQueries >= 2, true)
check("…and wakes the controller", wakes(), 1)
check("…naming the turn", has(lastEvent("orders_peek_wake"), '"turn":4'), true)
check("…with the time since the last tick", has(lastEvent("orders_peek_wake"), '"since_tick":0.75'), true)
check("the forced poll was armed (or already consumed by the tick)",
	beforeForce == false and (queue.forcePoll == true or ops(8, "UNITOPERATION_MOVE_TO") == 1), true)
-- 0e. A publish-batch peek and a UI peek share one budget.
host.units = { [9] = { id = 9, kind = "UNIT_SCOUT", x = 1, y = 1, moves = 3 } }
host.ops = {}
channel.ready = {}; channel.orders = {}
openTurn(5)
local before = readyQueries
clock = clock + 1
landed()
uiPeek()
check("batch peek then UI peek within 50 ms: one query", readyQueries - before, 1)

-- 1. A settled turn has no board out: no peek, no query.
revealed = {}; reveal(1, 1)
host.units = { [5] = { id = 5, kind = "UNIT_SCOUT", x = 1, y = 1, moves = 3 } }
host.ops = {}
openTurn(30)
answer(30, 0, { row(0, 5, "FORTIFY", nil, nil, 0) })
check("turn 30 settles", settle(30, 50), true)
readyQueries = 0
clock = clock + 1
check("a settled turn does not peek", landed(), false)
check("…and does not query", readyQueries, 0)

-- 2. Board out, no answer yet: peeks are wall-clock bounded.
host.units = { [5] = { id = 5, kind = "UNIT_SCOUT", x = 1, y = 1, moves = 3 } }
host.ops = {}
channel.ready = {}; channel.orders = {}
openTurn(31)
readyQueries = 0
local hits = 0
for _ = 1, 50 do                 -- 50 publish batches over 0.2 s of clock
	clock = clock + 0.004
	if landed() then hits = hits + 1 end
end
check("no answer, no landing", hits, 0)
check("at most one ready query per 50 ms", readyQueries <= 5 and readyQueries >= 3, true)

-- 3. The answer lands; the next DUE batch sees it, an early one does not.
clock = clock + 0.06              -- a fresh window: this batch peeks and misses
check("the window's first batch peeks and misses", landed(), false)
answer(31, 0, { row(0, 5, "MOVE_TO", 2, 1, 0) })
local before = readyQueries
clock = clock + 0.01
check("a batch inside the 50 ms window does not re-query", landed(), false)
check("…and costs no query", readyQueries, before)
clock = clock + 0.06
check("the next due batch finds the landed answer", landed(), true)

-- 4. The cadence alone skips tick 1 (OrdersPollTicks = 2); a forced poll
-- reads the answer on it.
check("tick 1 without a forced poll does not apply", settleTurn(player, PID, 31, function() end), false)
check("…the move is not issued yet", ops(5, "UNITOPERATION_MOVE_TO"), 0)
check("tick 2 polls on its cadence and applies", (function()
	settleTurn(player, PID, 31, function() end)
	return ops(5, "UNITOPERATION_MOVE_TO")
end)(), 1)

host.units = { [6] = { id = 6, kind = "UNIT_SCOUT", x = 1, y = 1, moves = 3 } }
host.ops = {}
openTurn(32)
answer(32, 0, { row(0, 6, "MOVE_TO", 2, 1, 0) })
queue.forcePoll = true
settleTurn(player, PID, 32, function() end)
check("a forced poll reads the answer on tick 1", ops(6, "UNITOPERATION_MOVE_TO"), 1)
check("…and is consumed", queue.forcePoll, false)

-- 5. The real handler runs over a quiet board without raising.
local ok, errText = pcall(function()
	for _ = 1, 40 do clock = clock + 0.01; publish() end
end)
check("the publish handler runs cleanly", ok, true)
if not ok then print(errText) end

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("\nall orders-peek checks passed")
