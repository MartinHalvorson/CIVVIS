-- Hotseat `--fast-turns`: a seat waiting for nothing but its next tick gets
-- it from the HUD's 20 ms pulse (`CivvisQueue.pulseTickDue`), at most twice
-- per wait, and never while anything it started is in flight.
--
-- Loads the SHIPPED `CivvisControlAgent.lua` on the `orders_peek_test` host
-- (a controllable UI clock, turn, pause and turn-active flag), then checks:
--   0. nothing before the controller has started;
--   1. a turn not yet opened ticks on the pulse once the game is unpaused,
--      not while the hand-over's pause stands, and opens there;
--   2. an applied answer that started nothing ticks on the next pulse, and
--      that tick asks for the end of the turn; once asked, no more;
--   3. a watched walk or a ranged shot in flight: no pulse tick until it lands;
--   4. at most `PULSE_TICKS` per wait;
--   5. without `HotseatFastTurns`, or outside hotseat, the pulse ticks nothing.
--
-- Run: lua5.1 tools/civ6_control/mod/hotseat_pulse_tick_test.lua

-- The host state the test moves: the game turn, the hand-over's pause, and
-- whether this seat's turn is active.
world = { turn = 3, paused = true, active = true }
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
-- A hotseat seat's context (`HotseatSeat`), so the agent claims its seat
-- rather than re-hosting the game after its third tick.
CivvisControlConfig = { Hotseat = true, HotseatFastTurns = true, HotseatSeat = 0, Rehost = false,
                        ReplanFrames = 2, ExportState = false, TileExportEvery = 25,
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
	-- No retire row: the real tick polls for one (`retireAsked`).
	if sql:find("kind = 'retire'", 1, true) then return {} end
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
	IsTurnActive = function() return world.active end,
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
Game = { GetLocalPlayer = function() return PID end, GetCurrentGameTurn = function() return world.turn end }
GameConfiguration = { IsPaused = function() return world.paused end }
-- Nothing blocks the end of a turn here, so the tick that settles asks for it.
EndTurnBlockingTypes = { NO_ENDTURN_BLOCKING = 0 }
NotificationManager = { GetFirstEndTurnBlocking = function() return 0 end }
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


local queue = rawget(_G, "CivvisQueue")
local frames = rawget(_G, "CivvisFrames")
rawget(_G, "CivvisResolveActions")()
local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", name, tostring(got)))
	end
end
local function count(kind, needle)
	local n = 0
	for _, l in ipairs(LOG) do
		if l:find('"kind":"' .. kind .. '"', 1, true) and (needle == nil or l:find(needle, 1, true)) then
			n = n + 1
		end
	end
	return n
end
local function lastEvent(kind)
	for i = #LOG, 1, -1 do if LOG[i]:find('"kind":"' .. kind .. '"', 1, true) then return LOG[i] end end
	return nil
end
local function row(seq, subject, verb, x, y, frame)
	return { seq = seq, kind = "unit", subject = subject, verb = verb, x = x, y = y, frame = frame }
end
local function answer(turn, frame, rows)
	channel.ready = { { run = "test-run", turn = turn, count = #rows, frame = frame } }
	channel.orders = rows
end
local uiPeek = luaHandlers.CivvisControlPeek
local publish = handlers.GameCoreEventPublishComplete
check("CivvisControlPeek is registered", type(uiPeek), "function")
check("CivvisQueue.pulseTickDue is defined", type(queue.pulseTickDue), "function")
local function ticks() return queue.controllerTicks or 0 end
local function pulses(why) return count("pulse_tick", '"why":"' .. why .. '"') end
local function pulse(seconds) clock = clock + (seconds or 0.02); uiPeek() end
-- A board is out when the peek asks the channel for it.
local function boardOut()
	local before = readyQueries
	pulse(0.1)
	return readyQueries > before
end
-- Land the answer and let the peek apply it: pulse until its `orders` record
-- is written, and no further.
local function applied(turn, rows)
	answer(turn, 0, rows)
	local before = count("orders", '"turn":' .. turn)
	for _ = 1, 10 do
		pulse(0.1)
		if count("orders", '"turn":' .. turn) > before then return true end
	end
	return false
end

-- 0. Not before the controller has started (its seat claimed, its actions
-- resolved): the first publish batches start it.
host.units = { [5] = { id = 5, kind = "UNIT_SCOUT", x = 1, y = 1, moves = 3 } }
host.ops = {}
channel.ready, channel.orders = {}, {}
world.turn, world.paused, world.active = 3, false, true
check("0. not started yet: not due", queue.pulseTickDue(), false)
for _ = 1, 3 do publish() end
check("0. started by three publish batches: due", queue.pulseTickDue(), true)
-- That ask wrote its record and spent one of the wait's two: start clean.
queue.pulseSpent = nil
for i = #LOG, 1, -1 do LOG[i] = nil end

-- 1. The opening: not while the hand-over's pause stands; once it lifts, the
-- next pulse ticks, and that tick opens the turn.
world.paused = true
local t0 = ticks()
pulse(1)
check("1. paused: the pulse does not tick", ticks() - t0, 0)
check("…and writes no pulse_tick", pulses("open"), 0)
world.paused = false
pulse()
check("1. unpaused: the pulse ticks", ticks() - t0, 1)
check("…once, naming the opening", pulses("open"), 1)
local opened = lastEvent("pulse_tick")
check("…on turn 3", opened ~= nil and opened:find('"turn":3', 1, true) ~= nil, true)
check("…with the UI clock", opened ~= nil and opened:find('"ui":', 1, true) ~= nil, true)
check("the tick opened the turn: its board is out", boardOut(), true)
check("…and the pulse ticks no more for it", pulses("open"), 1)

-- 2. An answer that starts nothing: the next pulse ticks, the turn's end is
-- asked on that tick, and nothing more is ticked after.
check("2. the answer is applied by the peek's own tick",
	applied(3, { row(0, 5, "FORTIFY", nil, nil, 0) }), true)
check("…and the pulse has not ticked the settle yet", pulses("settle"), 0)
local t1 = ticks()
pulse()
check("2. the next pulse ticks the settled frame", pulses("settle"), 1)
check("…one tick", ticks() - t1, 1)
check("…which asks for the end of the turn", queue.endTurnSubmittedTurn, 3)
pulse(); pulse()
check("2. once asked, the pulse ticks nothing more", ticks() - t1, 1)

-- 3a. A walk in flight: no pulse tick until the host's own tick lands it.
CivvisControlConfig.ReplanFrames = 0
host.units = { [6] = { id = 6, kind = "UNIT_SCOUT", x = 1, y = 1, moves = 3 } }
host.ops = {}
host.slow = true
channel.ready, channel.orders = {}, {}
world.turn = 4
pulse()
check("3a. turn 4 opens on the pulse", pulses("open"), 2)
check("3a. its walk is applied", applied(4, { row(0, 6, "MOVE_TO", 3, 1, 0) }), true)
check("…asked of the host", #host.ops >= 1, true)
check("…and is watched", queue.pendingCount() > 0, true)
local t2 = ticks()
pulse(); pulse(); pulse()
check("3a. a watched walk: the pulse ticks nothing", ticks() - t2, 0)
check("…and writes no pulse_tick", pulses("settle"), 1)
host.land(6)
pulse()
check("3a. landed but not yet seen by a tick: still nothing", ticks() - t2, 0)
for _ = 1, 16 * 40 do
	publish()
	if queue.endTurnSubmittedTurn == 4 then break end
end
check("3a. the publish batches' ticks end the turn as before", queue.endTurnSubmittedTurn, 4)
check("…with no pulse tick in the turn", pulses("settle"), 1)
host.slow = false

-- 3b. A ranged shot waiting for its fight: no pulse tick until it is over.
host.units = { [7] = { id = 7, kind = "UNIT_SCOUT", x = 1, y = 1, moves = 3 } }
host.ops = {}
channel.ready, channel.orders = {}, {}
world.turn = 5
pulse()
check("3b. its answer is applied", applied(5, { row(0, 7, "FORTIFY", nil, nil, 0) }), true)
frames.pendingStrikes = frames.pendingStrikes or {}
frames.pendingStrikes[901] = { player = PID, unit = 7, turn = 5, x = 2, y = 2 }
local t3 = ticks()
pulse(); pulse()
check("3b. a shot in flight: the pulse ticks nothing", ticks() - t3, 0)
frames.pendingStrikes[901] = nil
pulse()
check("3b. the fight over: the next pulse ticks", ticks() - t3, 1)
check("…as a settle", pulses("settle"), 2)

-- 4. At most `PULSE_TICKS` per wait; a new wait has its own.
world.turn = 6
check("4. PULSE_TICKS is two", queue.PULSE_TICKS, 2)
check("4. first", queue.pulseTickDue(), true)
check("4. second", queue.pulseTickDue(), true)
check("4. third: spent", queue.pulseTickDue(), false)
world.turn = 7
check("4. the next turn's wait has its own", queue.pulseTickDue(), true)
world.paused = true
check("4. paused again: not due", queue.pulseTickDue(), false)
world.paused, world.active = false, false
check("4. a turn not active: not due", queue.pulseTickDue(), false)
world.active = true

-- 5. Without `--fast-turns`, or outside hotseat: the pulse ticks nothing.
world.turn = 8
CivvisControlConfig.HotseatFastTurns = false
local t4 = ticks()
pulse()
check("5. without HotseatFastTurns: no tick", ticks() - t4, 0)
check("…not due", queue.pulseTickDue(), false)
CivvisControlConfig.HotseatFastTurns, CivvisControlConfig.Hotseat = true, false
check("5. outside hotseat: not due", queue.pulseTickDue(), false)
CivvisControlConfig.Hotseat = true

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("\nall pulse-tick checks passed")
