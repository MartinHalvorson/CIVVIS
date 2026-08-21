-- A unit spends EVERY action it has in the turn, end to end through the
-- shipped mod: the opening walk cut at the edge of the known is followed by
-- a second step on the replan frame; an archer steps and then shoots from
-- the per-unit queue; a settler steps and settles on the hex the brain
-- chose — and ONLY on that hex, whether the found ran first (the mod runs
-- every FOUND_CITY row before the settler's walk) or behind a walk the host
-- capped short of the site.
--
-- ⚠ Loads the SHIPPED `CivvisControlAgent.lua` with `ReplanFrames = 2` and
-- drives `beginTurn` / `settleTurn` / `applyOrders` against a fake host
-- whose MOVE_TO actually walks the unit, spends its movement and reveals
-- the ring around where it lands, and whose orders channel the test writes.
--
-- What is checked:
--   1. the seat event advertises `replan_frames_max`;
--   2. step, then step: opening `MOVE_TO` to the edge of the known → the
--      unit arrives with movement left → `settleTurn` opens a replan frame
--      (`revealed` + a mover) → the frame's answer moves the SAME unit again
--      and the host takes it (no refusal, no explore hand-off, frame stamped);
--   3. step, then shoot: `[MOVE_TO, RANGE_ATTACK]` → the strike is issued
--      from the queue once the step has landed, with movement left;
--   4. step, then settle: `[MOVE_TO site, FOUND_CITY @site]` → the found
--      that ran FIRST (settler not yet on the site) is refused by name
--      (`found_off_site`), does NOT found where the settler stood, emits no
--      `found_refused` (which would block that hex forever), and the
--      re-queued found lands once the settler stands on the site;
--   5. capped walk: the host stops the settler one hex short with movement
--      to spare → the re-queued found is refused `found_off_site`, not
--      founded on the wrong hex; a row without a site (older brain) keeps
--      the old behaviour and founds where the settler stands.
--
-- Run: lua5.1 tools/civ6_control/mod/step_turn_actions_test.lua

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
                        OrdersPollTicks = 1, CombatFramePolls = 20, OrdersWaitPolls = 40,
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
DB = { Query = function(sql)
	if sql:find("civvis.ready", 1, true) then return channel.ready end
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
			local plots, turns = line(u, params.x, params.y)
			for i = 2, #plots do
				if turns[i] <= 1 then
					u.x, u.y = plots[i] % W, math.floor(plots[i] / W)
					u.moves = u.moves - 1
					reveal(u.x, u.y)
				end
			end
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

-- 1. The seat advertises the frame cap.
pcall(rawget(_G, "CivvisSurvey"))
check("seat: replan_frames_max", has(lastEvent("seat"), '"replan_frames_max":2'), true)
check("seat: replan_frames", has(lastEvent("seat"), '"replan_frames":true'), true)

-- 2. Step, then step. A scout at (1,1) with 3 movement; the brain cut its
-- walk at (3,1), the edge of the known. Host: lands at (3,1) with 1 left,
-- reveals the ring → frame 1 → the brain answers MOVE_TO (4,1).
revealed = {}; reveal(1, 1); reveal(2, 1)
host.units = { [5] = { id = 5, kind = "UNIT_SCOUT", x = 1, y = 1, moves = 3 } }
host.ops = {}
openTurn(12)
answer(12, 0, { row(0, 5, "MOVE_TO", 3, 1, 0) })
-- The apply tick must NOT release the turn: the queue and the frame come
-- after it. One tick applies; the next observes and opens the frame.
check("the apply tick holds the turn", settleTurn(player, PID, 12, function() end), false)
check("opening MOVE_TO applied", ops(5, "UNITOPERATION_MOVE_TO"), 1)
check("the scout stands at the edge of the known", host.units[5].x, 3)
check("…with movement left", host.units[5].moves, 1)
check("the next tick opens the frame and holds the turn", settleTurn(player, PID, 12, function() end), false)
check("replan frame opened", frames.current, 1)
check("…for revealed ground", has(lastEvent("replan_frame"), '"reason":"revealed"'), true)
check("…with the scout counted as a mover", has(lastEvent("replan_frame"), '"movers":1'), true)
local beforeExplore = ops(5, "UNITOPERATION_AUTOMATE_EXPLORE")
answer(12, 1, { row(10000, 5, "MOVE_TO", 4, 1, 1) })
check("the frame's apply tick holds the turn too", settleTurn(player, PID, 12, function() end), false)
check("the frame's MOVE_TO moved the SAME unit again", ops(5, "UNITOPERATION_MOVE_TO"), 2)
check("the scout took its second step", host.units[5].x, 4)
check("…and spent the rest of its movement", host.units[5].moves, 0)
check("the frame's orders event is stamped", has(lastEvent("orders"), '"frame":1'), true)
check("…and applied the step", has(lastEvent("orders"), '"applied":1'), true)
check("no explore hand-off on the frame", ops(5, "UNITOPERATION_AUTOMATE_EXPLORE"), beforeExplore)
-- Nothing left to move on: the second frame is not wanted and the turn ends.
check("the turn settles once nobody can move", settle(12, 5), true)
check("no second frame for a board with no mover", frames.current, 1)

-- 3. Step, then shoot. `[MOVE_TO (2,2), RANGE_ATTACK (4,2)]` for an archer
-- with 2 movement: the step lands with 1 left and the queue fires the shot
-- before the turn is released; the strike then wants a frame.
host.units = { [7] = { id = 7, kind = "UNIT_ARCHER", x = 1, y = 2, moves = 2 } }
host.ops = {}
openTurn(13)
answer(13, 0, { row(0, 7, "MOVE_TO", 2, 2, 0), row(1, 7, "RANGE_ATTACK", 4, 2, 0) })
check("archer: the apply tick holds the turn", settleTurn(player, PID, 13, function() end), false)
check("archer: the step ran at once", ops(7, "UNITOPERATION_MOVE_TO"), 1)
check("archer: the shot waits on the queue", queue.pendingCount(), 1)
check("archer: the queue drains before the turn is released", settleTurn(player, PID, 13, function() end), false)
check("archer: the shot rode the queue and landed", ops(7, "UNITOPERATION_RANGE_ATTACK"), 1)
check("archer: shot issued AFTER the step", host.ops[#host.ops].op, "UNITOPERATION_RANGE_ATTACK")
check("archer: nothing refused", has(lastEvent("orders_queue"), '"refused":0'), true)
check("archer: the strike opens a frame", settleTurn(player, PID, 13, function() end), false)
check("archer: …a combat frame", has(lastEvent("combat_frame"), '"reason":"strike"'), true)
answer(13, 1, {})
check("archer: an empty frame answer settles the turn", settle(13, 5), true)

-- 4. Step, then settle on the site. `[MOVE_TO (2,0), FOUND_CITY @(2,0)]`
-- for a settler at (1,0) with 2 movement. The mod runs the FOUND row FIRST:
-- off the site it is refused by name, founds nothing where the settler
-- stands, emits no `found_refused`; re-queued behind the walk, it founds
-- on the site once the settler is there with movement to spare.
host.units = { [9] = { id = 9, kind = "UNIT_SETTLER", x = 1, y = 0, moves = 2 } }
host.ops = {}; host.founded = {}
local refusedBefore = count("found_refused")
openTurn(14)
answer(14, 0, { row(0, 9, "MOVE_TO", 2, 0, 0), row(1, 9, "FOUND_CITY", 2, 0, 0) })
check("settler: the apply tick holds the turn", settleTurn(player, PID, 14, function() end), false)
check("settler: the off-site found is named", has(lastEvent("orders"), 'found_off_site'), true)
check("settler: no found op was requested off-site", ops(9, "UNITOPERATION_FOUND_CITY"), 0)
check("settler: no found_refused (that hex is not condemned)", count("found_refused"), refusedBefore)
check("settler: the found waits behind the walk", queue.pendingCount(), 1)
settle(14, 5)
check("settler: one city founded", #host.founded, 1)
check("settler: …on the site, not where it stood", host.founded[1] and host.founded[1].x, 2)
check("settler: the found ran after the step", host.ops[#host.ops].op, "UNITOPERATION_FOUND_CITY")

-- 5. A walk the host caps short of the site must not settle short. Settler
-- at (4,0) with 2 movement ordered to (7,0): the host walks it to (6,0)
-- (`move_capped`); give it a point back, as a unit that stopped before a
-- hill would keep, and the re-queued found is refused by name rather than
-- founding on (6,0).
host.units = { [10] = { id = 10, kind = "UNIT_SETTLER", x = 4, y = 0, moves = 2 } }
host.ops = {}; host.founded = {}
openTurn(15)
answer(15, 0, { row(0, 10, "MOVE_TO", 7, 0, 0), row(1, 10, "FOUND_CITY", 7, 0, 0) })
settleTurn(player, PID, 15, function() end)
check("capped: the host sent this turn's leg", has(lastEvent("move_capped"), '"sent":[6,0]'), true)
check("capped: settler stands one hex short", host.units[10].x, 6)
host.units[10].moves = 1
settle(15, 5)
check("capped: NO city founded short of the site", #host.founded, 0)
check("capped: the found was refused by name", has(lastEvent("orders_queue"), 'found_off_site'), true)

-- 5b. A row without a site (an older brain) keeps the old behaviour.
host.units = { [11] = { id = 11, kind = "UNIT_SETTLER", x = 4, y = 2, moves = 2 } }
host.ops = {}; host.founded = {}
openTurn(16)
answer(16, 0, { row(0, 11, "MOVE_TO", 7, 2, 0), row(1, 11, "FOUND_CITY", nil, nil, 0) })
settleTurn(player, PID, 16, function() end)
host.units[11].moves = 1
settle(16, 5)
check("no site on the row: founds where the settler stands (old behaviour)", #host.founded, 1)

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("\nall step-turn-actions checks passed")
