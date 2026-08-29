-- The host-grounded board: a MOVE_TO is this turn's leg, and queued paths on
-- combat units are cancelled at turn start.
--
-- ⚠ Loads the SHIPPED `CivvisControlAgent.lua` and drives its own `applyOrders`,
-- `CivvisBoard` and `CivvisQueue` against a fake host whose pathfinder answers
-- `GetMoveToPathEx` with plots and per-plot turns the way WorldInput.lua reads
-- them.
--
-- What is checked:
--   1. a path that lands this turn is not capped;
--   2. a two-turn path is sent as its first turn's leg, the row is rewritten
--      so the queue expects the capped plot, and `move_capped` is emitted;
--   3. a path whose first step is already next turn is refused by name;
--   4. a melee ATTACK is never capped;
--   5. an explicit opt-out leaves the two capped paths alone;
--   6. a matching explicit guard keeps pace with the setter;
--   7. a missing guard row is synthesized before a one-step setter move;
--   8. an unreachable or differently-targeted guard remains untouched;
--   9. queued paths: combat units are cancelled, civilians are not, and the
--      count is reported;
--  10. a refused settler move cannot draw its guard into a synthetic move;
--  11. the `orders` event carries the cap and shadow counters.
--  13. an unguarded settler will not step into the measured two-plot reach of
--      a visible barbarian scout; any settler leg reachable by a visible
--      barbarian combat unit is held even with a synchronized single escort,
--      while invisible, distant, and proven-scout-escort cases retain ordinary
--      movement.
--  14. A co-located combat escort queued in an earlier frame stays with an
--      exposed Settler until the later Settler safety row is actuated.
--
-- Run: lua5.1 tools/civ6_control/mod/host_board_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end

local EXPORTS = { CivvisApplyOrders = true, CivvisQueue = true, CivvisResolveActions = true,
                  CivvisBoard = true, CivvisApplyOrder = true }
local LOG = {}
Automation = { Log = function(line) LOG[#LOG + 1] = line end }
UnitOperationTypes = { PARAM_X = "x", PARAM_Y = "y" }
UnitCommandTypes = {}
DirectionTypes = {
	DIRECTION_WEST = "west", DIRECTION_EAST = "east",
	DIRECTION_NORTHWEST = "northwest", DIRECTION_NORTHEAST = "northeast",
	DIRECTION_SOUTHWEST = "southwest", DIRECTION_SOUTHEAST = "southeast",
}
-- Plot index = y * 100 + x on this fake map.
local function plotIndex(x, y) return y * 100 + x end
Map = {
	GetPlotDistance = function(x1, y1, x2, y2) return math.max(math.abs(x1 - x2), math.abs(y1 - y2)) end,
	GetPlot = function() return nil end,
	GetAdjacentPlot = function(x, y, direction)
		local dx, dy = 0, 0
		if direction == DirectionTypes.DIRECTION_WEST then dx = -1
		elseif direction == DirectionTypes.DIRECTION_EAST then dx = 1
		elseif direction == DirectionTypes.DIRECTION_NORTHWEST then dy = -1
		elseif direction == DirectionTypes.DIRECTION_NORTHEAST then dx, dy = 1, -1
		elseif direction == DirectionTypes.DIRECTION_SOUTHWEST then dx, dy = -1, 1
		elseif direction == DirectionTypes.DIRECTION_SOUTHEAST then dy = 1 end
		return { GetX = function() return x + dx end, GetY = function() return y + dy end }
	end,
	GetPlotIndex = function(x, y) return plotIndex(x, y) end,
	GetPlotByIndex = function(index)
		return { GetX = function() return index % 100 end, GetY = function() return math.floor(index / 100) end }
	end,
}
GameInfo = setmetatable({}, { __index = function(_, k)
	if k == "UnitOperations" or k == "UnitCommands" then
		return setmetatable({}, { __index = function(_, name) return { Hash = name } end })
	end
	if k == "Units" then
		return setmetatable({}, { __index = function(_, name)
			if name == "UNIT_SETTLER" then return { UnitType = name, Combat = 0, RangedCombat = 0 } end
			return { UnitType = name, Combat = 20, RangedCombat = 0 }
		end })
	end
	return stub()
end })
rawset(_G, "CivvisControlConfig", {})
setmetatable(_G, { __index = function(_, k)
	if EXPORTS[k] then return rawget(_G, k) end
	return stub()
end })

local host = { units = {}, cities = {}, barbarians = {}, hidden = {}, ops = {}, cmds = {},
               paths = {}, queued = {}, blocked = {} }
local PID = 0
local function unitObject(u)
	return {
		GetID = function() return u.id end,
		GetX = function() return u.x end,
		GetY = function() return u.y end,
		GetMovesRemaining = function() return u.moves end,
		GetUnitType = function() return u.kind end,
		GetType = function() return u.kind end,
		GetDamage = function() return 0 end,
		GetGreatPerson = function() return nil end,
		GetFortifyTurns = function() return 0 end,
		GetFormationUnitCount = function() return 1 end,
	}
end
local function cityObject(c)
	return {
		GetX = function() return c.x end,
		GetY = function() return c.y end,
	}
end
UnitManager = {
	GetUnit = function(pid, id)
		local u = host.units[id]
		if u == nil or u.gone then return nil end
		return unitObject(u)
	end,
	CanStartOperation = function(unit) return not host.blocked[unit.GetID()] end,
	RequestOperation = function(unit, hash, params)
		local u = host.units[unit.GetID()]
		host.ops[#host.ops + 1] = { id = u.id, op = hash, x = params and params.x, y = params and params.y }
		if hash == "UNITOPERATION_MOVE_TO" then u.pendingX, u.pendingY = params.x, params.y end
	end,
	CanStartCommand = function() return true end,
	RequestCommand = function(unit, hash) host.cmds[#host.cmds + 1] = { id = unit.GetID(), cmd = hash } end,
	-- The fake pathfinder: `host.paths[unitId .. ":" .. destIndex]` = { plots = {...}, turns = {...} }.
	GetMoveToPathEx = function(unit, index)
		return host.paths[unit.GetID() .. ":" .. index]
	end,
	GetQueuedDestination = function(unit) return host.queued[unit.GetID()] end,
}
function host.arrive(id)
	local u = host.units[id]
	if u.pendingX ~= nil then
		u.x, u.y = u.pendingX, u.pendingY
		u.pendingX, u.pendingY = nil, nil
	end
end
local function members(list)
	return function()
		local i = 0
		return function()
			i = i + 1
			if list[i] == nil then return nil end
			return i, list[i]
		end
	end
end
local player = setmetatable({
	GetUnits = function()
		local objs = {}
		for _, u in pairs(host.units) do if not u.gone then objs[#objs + 1] = unitObject(u) end end
		table.sort(objs, function(a, b) return a.GetID() < b.GetID() end)
		return { Members = members(objs) }
	end,
	GetCities = function()
		local objs = {}
		for _, c in pairs(host.cities) do objs[#objs + 1] = cityObject(c) end
		return { Members = members(objs) }
	end,
	GetDiplomacy = function() return { IsAtWarWith = function() return false end } end,
	GetScore = function() return 0 end,
	GetTreasury = function() return { GetGoldBalance = function() return 0 end } end,
	IsTurnActive = function() return true end,
}, { __index = function() return stub() end })
Players = setmetatable({}, { __index = function(_, pid)
	if pid == PID then return player end
	return setmetatable({ IsBarbarian = function() return true end,
		GetUnits = function()
			local objs = {}
			for _, u in pairs(host.barbarians) do
				if not u.gone then objs[#objs + 1] = unitObject(u) end
			end
			table.sort(objs, function(a, b) return a.GetID() < b.GetID() end)
			return { Members = members(objs) }
		end,
		GetCities = function() return { Members = members({}) } end },
		{ __index = function() return stub() end })
end })
PlayerManager = { GetAliveIDs = function() return { PID, 63 } end, GetAliveMajorIDs = function() return { PID } end }
PlayersVisibility = setmetatable({}, { __index = function()
	return { IsVisible = function(_, x, y) return not host.hidden[x .. ":" .. y] end,
		IsRevealed = function() return true end }
end })
Game = { GetLocalPlayer = function() return PID end, GetCurrentGameTurn = function() return 7 end }

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtime_err = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtime_err))

local applyOrders = rawget(_G, "CivvisApplyOrders")
local board = rawget(_G, "CivvisBoard")
local queue = rawget(_G, "CivvisQueue")
local config = rawget(_G, "CivvisControlConfig")
rawget(_G, "CivvisResolveActions")()
assert(type(board) == "table", "CivvisBoard is not exported")

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", name, tostring(got)))
	end
end
local function ops(id)
	local out = {}
	for _, o in ipairs(host.ops) do
		if o.id == id then out[#out + 1] = o.op .. "@" .. tostring(o.x) .. "," .. tostring(o.y) end
	end
	return table.concat(out, ";")
end
local function lastEvent(kind)
	for i = #LOG, 1, -1 do
		if LOG[i]:find('"kind":"' .. kind .. '"', 1, true) then return LOG[i] end
	end
	return nil
end
local function has(line, needle) return line ~= nil and line:find(needle, 1, true) ~= nil end
local function row(subject, verb, x, y) return { kind = "unit", subject = subject, verb = verb, x = x, y = y } end
local function reset()
	host.units, host.cities, host.barbarians, host.hidden, host.ops, host.cmds, host.paths,
		host.queued, host.blocked, LOG = {}, {}, {}, {}, {}, {}, {}, {}, {}, {}
	config.SettlerEscortCapSync = nil
	queue.reset(7); board.reset()
end

-- 1. A same-turn path is not capped.
reset()
host.units[10] = { id = 10, kind = "UNIT_WARRIOR", x = 1, y = 1, moves = 2 }
host.paths["10:" .. plotIndex(3, 1)] = { plots = { plotIndex(1, 1), plotIndex(2, 1), plotIndex(3, 1) }, turns = { 0, 1, 1 } }
applyOrders(player, PID, 7, { row(10, "MOVE_TO", 3, 1) })
check("same-turn walk sent whole", ops(10), "UNITOPERATION_MOVE_TO@3,1")
check("nothing capped", board.stats.capped, 0)

-- 2. A two-turn path is sent as this turn's leg; the queue expects the capped plot.
reset()
host.units[11] = { id = 11, kind = "UNIT_WARRIOR", x = 1, y = 1, moves = 2 }
host.paths["11:" .. plotIndex(5, 1)] = {
	plots = { plotIndex(1, 1), plotIndex(2, 1), plotIndex(3, 1), plotIndex(4, 1), plotIndex(5, 1) },
	turns = { 0, 1, 1, 2, 2 } }
applyOrders(player, PID, 7, { row(11, "MOVE_TO", 5, 1), row(11, "FORTIFY") })
check("two-turn walk capped to its first leg", ops(11), "UNITOPERATION_MOVE_TO@3,1")
check("move_capped emitted with both plots", has(lastEvent("move_capped"), '"sent":[3,1]') and has(lastEvent("move_capped"), '"want":[5,1]'), true)
check("orders event counts the cap", has(lastEvent("orders"), '"move_capped":1'), true)
host.units[11].x, host.units[11].y = 3, 1   -- the host walked it to the capped plot
queue.drain(player, PID, 7)
check("queued fortify fires on arrival at the CAPPED plot", ops(11), "UNITOPERATION_MOVE_TO@3,1;UNITOPERATION_FORTIFY@nil,nil")

-- 3. A path whose first step is next turn is refused by name.
reset()
host.units[12] = { id = 12, kind = "UNIT_WARRIOR", x = 1, y = 1, moves = 0 }
host.paths["12:" .. plotIndex(2, 1)] = { plots = { plotIndex(1, 1), plotIndex(2, 1) }, turns = { 0, 2 } }
applyOrders(player, PID, 7, { row(12, "MOVE_TO", 2, 1) })
check("no-reach move issues nothing", ops(12), "")
check("refused as move_no_moves_this_turn", has(lastEvent("orders"), "move_no_moves_this_turn"), true)
check("orders event counts no_reach", has(lastEvent("orders"), '"move_no_reach":1'), true)

-- 4. A melee ATTACK is never capped, whatever the path says.
reset()
host.units[13] = { id = 13, kind = "UNIT_WARRIOR", x = 1, y = 1, moves = 2 }
host.paths["13:" .. plotIndex(4, 1)] = { plots = { plotIndex(1, 1), plotIndex(2, 1), plotIndex(3, 1), plotIndex(4, 1) }, turns = { 0, 1, 2, 2 } }
applyOrders(player, PID, 7, { row(13, "ATTACK", 4, 1) })
check("attack sent uncapped", ops(13), "UNITOPERATION_MOVE_TO@4,1")

-- 5. The explicit opt-out keeps the ordinary per-row cap unchanged.
reset()
config.SettlerEscortCapSync = false
host.units[17] = { id = 17, kind = "UNIT_SETTLER", x = 1, y = 1, moves = 2 }
host.units[18] = { id = 18, kind = "UNIT_WARRIOR", x = 1, y = 1, moves = 2 }
host.paths["17:" .. plotIndex(5, 1)] = {
	plots = { plotIndex(1, 1), plotIndex(2, 1), plotIndex(3, 1), plotIndex(4, 1), plotIndex(5, 1) },
	turns = { 0, 1, 1, 2, 2 } }
host.paths["17:" .. plotIndex(3, 1)] = {
	plots = { plotIndex(1, 1), plotIndex(2, 1), plotIndex(3, 1) }, turns = { 0, 1, 1 } }
host.paths["18:" .. plotIndex(5, 1)] = {
	plots = { plotIndex(1, 1), plotIndex(2, 1), plotIndex(3, 1), plotIndex(4, 1), plotIndex(5, 1) },
	turns = { 0, 1, 1, 1, 2 } }
host.paths["18:" .. plotIndex(3, 1)] = {
	plots = { plotIndex(1, 1), plotIndex(2, 1), plotIndex(3, 1) }, turns = { 0, 1, 1 } }
applyOrders(player, PID, 7, { row(17, "MOVE_TO", 5, 1), row(18, "MOVE_TO", 5, 1) })
check("opt-out: setter uses its cap", ops(17), "UNITOPERATION_MOVE_TO@3,1")
check("opt-out: guard keeps its own cap", ops(18), "UNITOPERATION_MOVE_TO@4,1")
check("opt-out: no escort sync event", lastEvent("escort_cap_synced"), nil)

-- 6. A matching explicit row is rewritten to the setter's actual leg.
reset()
config.SettlerEscortCapSync = true
host.units[17] = { id = 17, kind = "UNIT_SETTLER", x = 1, y = 1, moves = 2 }
host.units[18] = { id = 18, kind = "UNIT_WARRIOR", x = 1, y = 1, moves = 2 }
host.paths["17:" .. plotIndex(5, 1)] = {
	plots = { plotIndex(1, 1), plotIndex(2, 1), plotIndex(3, 1), plotIndex(4, 1), plotIndex(5, 1) },
	turns = { 0, 1, 1, 2, 2 } }
host.paths["17:" .. plotIndex(3, 1)] = {
	plots = { plotIndex(1, 1), plotIndex(2, 1), plotIndex(3, 1) }, turns = { 0, 1, 1 } }
host.paths["18:" .. plotIndex(5, 1)] = {
	plots = { plotIndex(1, 1), plotIndex(2, 1), plotIndex(3, 1), plotIndex(4, 1), plotIndex(5, 1) },
	turns = { 0, 1, 1, 1, 2 } }
host.paths["18:" .. plotIndex(3, 1)] = {
	plots = { plotIndex(1, 1), plotIndex(2, 1), plotIndex(3, 1) }, turns = { 0, 1, 1 } }
local setterRow, guardRow = row(17, "MOVE_TO", 5, 1), row(18, "MOVE_TO", 5, 1)
applyOrders(player, PID, 7, { setterRow, guardRow })
check("matching: setter uses its cap", ops(17), "UNITOPERATION_MOVE_TO@3,1")
check("matching: guard follows setter cap", ops(18), "UNITOPERATION_MOVE_TO@3,1")
check("matching: guard row was rewritten", guardRow.x, 3)
check("matching: sync named both units", has(lastEvent("escort_cap_synced"), '"settler":17')
	and has(lastEvent("escort_cap_synced"), '"guard":18'), true)
check("matching: orders count the sync", has(lastEvent("orders"), '"escort_cap_synced":1'), true)

-- 7. The safe default supplies the row the planner omitted and applies it first.
reset()
host.units[19] = { id = 19, kind = "UNIT_SETTLER", x = 1, y = 1, moves = 2 }
host.units[20] = { id = 20, kind = "UNIT_WARRIOR", x = 1, y = 1, moves = 2 }
host.paths["19:" .. plotIndex(2, 1)] = {
	plots = { plotIndex(1, 1), plotIndex(2, 1) }, turns = { 0, 1 } }
host.paths["20:" .. plotIndex(2, 1)] = {
	plots = { plotIndex(1, 1), plotIndex(2, 1) }, turns = { 0, 1 } }
local missingGuard = { row(19, "MOVE_TO", 2, 1) }
applyOrders(player, PID, 7, missingGuard)
check("shadow: guard moved before settler", ops(20), "UNITOPERATION_MOVE_TO@2,1")
check("shadow: setter still moved", ops(19), "UNITOPERATION_MOVE_TO@2,1")
check("shadow: row was inserted", #missingGuard, 2)
check("shadow: named both units", has(lastEvent("escort_shadow_injected"), '"settler":19')
	and has(lastEvent("escort_shadow_injected"), '"guard":20'), true)
check("shadow: not counted as CIVVIS order", has(lastEvent("orders"), '"seen":1')
	and has(lastEvent("orders"), '"applied":1'), true)
check("shadow: orders carry host outcome", has(lastEvent("orders"), '"escort_shadow_injected":1')
	and has(lastEvent("orders"), '"escort_shadow_applied":1')
	and has(lastEvent("orders"), '"escort_shadow_refused":0'), true)

-- 8. A guard that cannot reach the setter's leg retains its previous order.
reset()
config.SettlerEscortCapSync = true
host.units[17] = { id = 17, kind = "UNIT_SETTLER", x = 1, y = 1, moves = 2 }
host.units[18] = { id = 18, kind = "UNIT_WARRIOR", x = 1, y = 1, moves = 0 }
host.paths["17:" .. plotIndex(5, 1)] = {
	plots = { plotIndex(1, 1), plotIndex(2, 1), plotIndex(3, 1), plotIndex(4, 1), plotIndex(5, 1) },
	turns = { 0, 1, 1, 2, 2 } }
host.paths["17:" .. plotIndex(3, 1)] = {
	plots = { plotIndex(1, 1), plotIndex(2, 1), plotIndex(3, 1) }, turns = { 0, 1, 1 } }
host.paths["18:" .. plotIndex(5, 1)] = {
	plots = { plotIndex(1, 1), plotIndex(2, 1), plotIndex(3, 1), plotIndex(4, 1), plotIndex(5, 1) },
	turns = { 0, 1, 1, 1, 2 } }
host.paths["18:" .. plotIndex(3, 1)] = {
	plots = { plotIndex(1, 1), plotIndex(2, 1), plotIndex(3, 1) }, turns = { 0, 2, 2 } }
applyOrders(player, PID, 7, { row(17, "MOVE_TO", 5, 1), row(18, "MOVE_TO", 5, 1) })
check("unreachable guard keeps its cap", ops(18), "UNITOPERATION_MOVE_TO@4,1")
check("unreachable guard is named", has(lastEvent("escort_cap_unresolved"), '"reason":"guard_still_capped"'), true)
check("orders count the unresolved guard", has(lastEvent("orders"), '"escort_cap_unresolved":1'), true)

-- 9. A missing but too-slow guard is not synthesized.
reset()
host.units[21] = { id = 21, kind = "UNIT_SETTLER", x = 1, y = 1, moves = 2 }
host.units[22] = { id = 22, kind = "UNIT_WARRIOR", x = 1, y = 1, moves = 0 }
host.paths["21:" .. plotIndex(2, 1)] = {
	plots = { plotIndex(1, 1), plotIndex(2, 1) }, turns = { 0, 1 } }
host.paths["22:" .. plotIndex(2, 1)] = {
	plots = { plotIndex(1, 1), plotIndex(2, 1) }, turns = { 0, 2 } }
applyOrders(player, PID, 7, { row(21, "MOVE_TO", 2, 1) })
check("too-slow missing guard does not move", ops(22), "")
check("too-slow missing guard is named", has(lastEvent("escort_cap_unresolved"), '"reason":"guard_still_capped"'), true)
check("too-slow missing guard is held, not injected", has(lastEvent("orders"), '"escort_shadow_injected":0')
	and has(lastEvent("orders"), '"escort_shadow_held":1'), true)

-- 10. Different original goals are not an escort contract.
reset()
config.SettlerEscortCapSync = true
host.units[17] = { id = 17, kind = "UNIT_SETTLER", x = 1, y = 1, moves = 2 }
host.units[18] = { id = 18, kind = "UNIT_WARRIOR", x = 1, y = 1, moves = 2 }
host.paths["17:" .. plotIndex(5, 1)] = {
	plots = { plotIndex(1, 1), plotIndex(2, 1), plotIndex(3, 1), plotIndex(4, 1), plotIndex(5, 1) },
	turns = { 0, 1, 1, 2, 2 } }
host.paths["17:" .. plotIndex(3, 1)] = {
	plots = { plotIndex(1, 1), plotIndex(2, 1), plotIndex(3, 1) }, turns = { 0, 1, 1 } }
host.paths["18:" .. plotIndex(6, 1)] = {
	plots = { plotIndex(1, 1), plotIndex(2, 1), plotIndex(3, 1), plotIndex(4, 1), plotIndex(5, 1), plotIndex(6, 1) },
	turns = { 0, 1, 1, 1, 2, 2 } }
applyOrders(player, PID, 7, { row(17, "MOVE_TO", 5, 1), row(18, "MOVE_TO", 6, 1) })
check("different goal keeps guard route", ops(18), "UNITOPERATION_MOVE_TO@4,1")
check("different goal emits no sync", lastEvent("escort_cap_synced"), nil)

-- 11. A host-refused settler cannot pull its guard into a synthetic move.
reset()
host.units[23] = { id = 23, kind = "UNIT_SETTLER", x = 1, y = 1, moves = 2 }
host.units[24] = { id = 24, kind = "UNIT_WARRIOR", x = 1, y = 1, moves = 2 }
host.paths["23:" .. plotIndex(2, 1)] = {
	plots = { plotIndex(1, 1), plotIndex(2, 1) }, turns = { 0, 1 } }
host.paths["24:" .. plotIndex(2, 1)] = {
	plots = { plotIndex(1, 1), plotIndex(2, 1) }, turns = { 0, 1 } }
host.blocked[23] = true
applyOrders(player, PID, 7, { row(23, "MOVE_TO", 2, 1) })
check("blocked setter has no shadow injection", lastEvent("escort_shadow_injected"), nil)
check("blocked setter does not synthesize a guard move", has(ops(24), "UNITOPERATION_MOVE_TO"), false)

-- 12. Queued paths: combat units cancelled, civilians kept, count reported.
reset()
host.units[14] = { id = 14, kind = "UNIT_WARRIOR", x = 1, y = 1, moves = 2 }
host.units[15] = { id = 15, kind = "UNIT_SETTLER", x = 2, y = 2, moves = 2 }
host.units[16] = { id = 16, kind = "UNIT_ARCHER", x = 3, y = 3, moves = 2 }
host.queued[14] = plotIndex(9, 9)
host.queued[15] = plotIndex(8, 8)
board.cancelQueuedPaths(player, PID, 7)
check("one combat unit cancelled", #host.cmds, 1)
check("it was the warrior, not the settler", host.cmds[1] and host.cmds[1].id, 14)
check("cancel used UNITCOMMAND_CANCEL", host.cmds[1] and host.cmds[1].cmd, "UNITCOMMAND_CANCEL")
check("queued_paths reported", has(lastEvent("queued_paths"), '"found":2') and has(lastEvent("queued_paths"), '"cancelled":1'), true)

-- 13. The direct live-loss geometry is held: a new settler leaving a city for
-- a tile within a barbarian scout's measured two-plot reach cannot spend its
-- opening turn there without a guard.
reset()
host.cities[1] = { x = 1, y = 1 }
host.units[30] = { id = 30, kind = "UNIT_SETTLER", x = 1, y = 1, moves = 2 }
host.barbarians[90] = { id = 90, kind = "UNIT_SCOUT", x = 2, y = 3, moves = 0 }
host.paths["30:" .. plotIndex(1, 2)] = {
	plots = { plotIndex(1, 1), plotIndex(1, 2) }, turns = { 0, 1 } }
applyOrders(player, PID, 7, { row(30, "MOVE_TO", 1, 2) })
check("visible scout: setter stays in city", ops(30), "")
check("visible scout: hold names setter and scout", has(lastEvent("settler_scout_capture_hold"), '"settler":30')
	and has(lastEvent("settler_scout_capture_hold"), '"scout":90'), true)
check("visible scout: order is an explicit held refusal", has(lastEvent("orders"), "settler_scout_capture_hold"), true)
check("visible scout: orders count the held capture leg", has(lastEvent("orders"), '"settler_scout_capture_held":1'), true)

-- A refusal is not enough when the Settler already stands inside the scout's
-- measured envelope.  The host can prove a one-step retreat, so rewrite the
-- actuation leg to that safe tile instead of leaving the civilian exposed.
reset()
host.units[32] = { id = 32, kind = "UNIT_SETTLER", x = 1, y = 2, moves = 2 }
host.barbarians[92] = { id = 92, kind = "UNIT_SCOUT", x = 1, y = 4, moves = 0 }
host.paths["32:" .. plotIndex(1, 3)] = {
	plots = { plotIndex(1, 2), plotIndex(1, 3) }, turns = { 0, 1 } }
host.paths["32:" .. plotIndex(1, 1)] = {
	plots = { plotIndex(1, 2), plotIndex(1, 1) }, turns = { 0, 1 } }
applyOrders(player, PID, 7, { row(32, "MOVE_TO", 1, 3) })
check("exposed scout leg retreats to a safe neighbour", ops(32), "UNITOPERATION_MOVE_TO@1,1")
check("exposed scout retreat records the original target", has(lastEvent("settler_capture_escape"), '"want":[1,3]')
	and has(lastEvent("settler_capture_escape"), '"sent":[1,1]'), true)
check("exposed scout retreat is not counted as a hold", has(lastEvent("orders"), '"settler_scout_capture_held":0'), true)

-- The safety decision is made against the first host leg, not the far-away
-- planner target.  This catches a queued route whose current cap ends beside
-- the scout even though its intended city site is two turns away.
reset()
host.cities[1] = { x = 1, y = 1 }
host.units[36] = { id = 36, kind = "UNIT_SETTLER", x = 1, y = 1, moves = 2 }
host.barbarians[96] = { id = 96, kind = "UNIT_SCOUT", x = 2, y = 3, moves = 0 }
host.paths["36:" .. plotIndex(1, 4)] = {
	plots = { plotIndex(1, 1), plotIndex(1, 2), plotIndex(1, 3), plotIndex(1, 4) },
	turns = { 0, 1, 1, 2 } }
host.paths["36:" .. plotIndex(1, 3)] = {
	plots = { plotIndex(1, 1), plotIndex(1, 2), plotIndex(1, 3) }, turns = { 0, 1, 1 } }
applyOrders(player, PID, 7, { row(36, "MOVE_TO", 1, 4) })
check("capped scout leg: setter stays in city", ops(36), "")
check("capped scout leg: event distinguishes want from sent", has(lastEvent("settler_scout_capture_hold"), '"want":[1,4]')
	and has(lastEvent("settler_scout_capture_hold"), '"sent":[1,3]'), true)

-- The scout floor protects the travelling leg as well as a first city
-- departure.  A lone settler must not enter the measured two-plot capture
-- geometry merely because it began this turn outside a city.
reset()
host.units[31] = { id = 31, kind = "UNIT_SETTLER", x = 1, y = 1, moves = 2 }
host.barbarians[91] = { id = 91, kind = "UNIT_SCOUT", x = 3, y = 4, moves = 0 }
host.paths["31:" .. plotIndex(1, 2)] = {
	plots = { plotIndex(1, 1), plotIndex(1, 2) }, turns = { 0, 1 } }
applyOrders(player, PID, 7, { row(31, "MOVE_TO", 1, 2) })
check("travelling setter stays out of two-step scout capture leg", ops(31), "")
check("travelling setter hold names the scout", has(lastEvent("settler_scout_capture_hold"), '"settler":31')
	and has(lastEvent("settler_scout_capture_hold"), '"scout":91'), true)

-- A scout hold protects the current tile too.  The live loss at turn 47 of
-- civvis-20260827T183146Z held the Settler's unsafe leg but let a warrior
-- sharing its current tile take an unrelated route; the scout then captured
-- the now-lone Settler.  Keep that guard put while the scout covers both
-- the rejected leg and the tile the Settler must remain on.
reset()
host.units[42] = { id = 42, kind = "UNIT_SETTLER", x = 1, y = 1, moves = 2 }
host.units[43] = { id = 43, kind = "UNIT_WARRIOR", x = 1, y = 1, moves = 2 }
host.barbarians[100] = { id = 100, kind = "UNIT_SCOUT", x = 3, y = 3, moves = 0 }
host.paths["42:" .. plotIndex(2, 1)] = {
	plots = { plotIndex(1, 1), plotIndex(2, 1) }, turns = { 0, 1 } }
host.paths["43:" .. plotIndex(1, 2)] = {
	plots = { plotIndex(1, 1), plotIndex(1, 2) }, turns = { 0, 1 } }
applyOrders(player, PID, 7, { row(42, "MOVE_TO", 2, 1), row(43, "MOVE_TO", 1, 2) })
check("scout-held setter stays put", ops(42), "")
check("scout-held co-located guard does not leave", ops(43), "")
check("scout-held guard is held off fallback", board.escortHolds[43], true)
check("scout-held guard event identifies both units", has(lastEvent("settler_scout_guard_hold"), '"settler":42')
	and has(lastEvent("settler_scout_guard_hold"), '"guard":43'), true)

-- A scout that covers only the rejected destination leaves a co-located guard
-- mobile: the Settler is safe on its current tile, so retaining the guard
-- would turn an actuation floor into needless army paralysis.
reset()
host.units[44] = { id = 44, kind = "UNIT_SETTLER", x = 1, y = 1, moves = 2 }
host.units[45] = { id = 45, kind = "UNIT_WARRIOR", x = 1, y = 1, moves = 2 }
host.barbarians[101] = { id = 101, kind = "UNIT_SCOUT", x = 1, y = 4, moves = 0 }
host.paths["44:" .. plotIndex(1, 2)] = {
	plots = { plotIndex(1, 1), plotIndex(1, 2) }, turns = { 0, 1 } }
host.paths["45:" .. plotIndex(2, 1)] = {
	plots = { plotIndex(1, 1), plotIndex(2, 1) }, turns = { 0, 1 } }
applyOrders(player, PID, 7, { row(44, "MOVE_TO", 1, 2), row(45, "MOVE_TO", 2, 1) })
check("destination-only scout still holds setter", ops(44), "")
check("destination-only scout leaves guard mobile", ops(45), "UNITOPERATION_MOVE_TO@2,1")
check("destination-only scout emits no guard hold", lastEvent("settler_scout_guard_hold"), nil)

-- The narrowed radius preserves a normal travel leg once the known scout is
-- outside the measured two-step capture reach.
reset()
host.units[41] = { id = 41, kind = "UNIT_SETTLER", x = 1, y = 1, moves = 2 }
host.barbarians[99] = { id = 99, kind = "UNIT_SCOUT", x = 4, y = 5, moves = 0 }
host.paths["41:" .. plotIndex(1, 2)] = {
	plots = { plotIndex(1, 1), plotIndex(1, 2) }, turns = { 0, 1 } }
applyOrders(player, PID, 7, { row(41, "MOVE_TO", 1, 2) })
check("distant scout leaves travelling setter mobile", ops(41), "UNITOPERATION_MOVE_TO@1,2")

reset()
host.cities[1] = { x = 1, y = 1 }
host.units[32] = { id = 32, kind = "UNIT_SETTLER", x = 1, y = 1, moves = 2 }
host.barbarians[92] = { id = 92, kind = "UNIT_SCOUT", x = 2, y = 3, moves = 0 }
host.hidden["2:3"] = true
host.paths["32:" .. plotIndex(1, 2)] = {
	plots = { plotIndex(1, 1), plotIndex(1, 2) }, turns = { 0, 1 } }
applyOrders(player, PID, 7, { row(32, "MOVE_TO", 1, 2) })
check("invisible scout: setter still moves", ops(32), "UNITOPERATION_MOVE_TO@1,2")

-- The live loss geometry: a visible non-scout combat unit beside the actual
-- leg holds both a travelling settler and the explicit escort that the host
-- synchronized to it.  This is intentionally not limited to a city origin.
reset()
host.units[33] = { id = 33, kind = "UNIT_SETTLER", x = 1, y = 1, moves = 2 }
host.units[34] = { id = 34, kind = "UNIT_WARRIOR", x = 1, y = 1, moves = 2 }
host.barbarians[93] = { id = 93, kind = "UNIT_MAN_AT_ARMS", x = 2, y = 3, moves = 2 }
host.paths["33:" .. plotIndex(1, 2)] = {
	plots = { plotIndex(1, 1), plotIndex(1, 2) }, turns = { 0, 1 } }
host.paths["34:" .. plotIndex(1, 2)] = {
	plots = { plotIndex(1, 1), plotIndex(1, 2) }, turns = { 0, 1 } }
applyOrders(player, PID, 7, { row(33, "MOVE_TO", 1, 2), row(34, "MOVE_TO", 1, 2) })
check("visible combat: setter stays out of capture leg", ops(33), "")
check("visible combat: synced guard stays with setter", ops(34), "")
check("visible combat: hold names hostile and exact setter", has(lastEvent("settler_barbarian_combat_capture_hold"), '"settler":33')
	and has(lastEvent("settler_barbarian_combat_capture_hold"), '"hostile":93'), true)
check("visible combat: order reports combat hold", has(lastEvent("orders"), '"settler_barbarian_combat_capture_held":1'), true)
check("visible combat: guard is held off automation", board.escortHolds[34], true)

-- The same safety applies when the host had to synthesize the matching guard
-- row.  Refusing the settler must not send that host-only escort ahead alone.
reset()
host.units[37] = { id = 37, kind = "UNIT_SETTLER", x = 1, y = 1, moves = 2 }
host.units[38] = { id = 38, kind = "UNIT_WARRIOR", x = 1, y = 1, moves = 2 }
host.barbarians[97] = { id = 97, kind = "UNIT_MUSKETMAN", x = 2, y = 3, moves = 2 }
host.paths["37:" .. plotIndex(1, 2)] = {
	plots = { plotIndex(1, 1), plotIndex(1, 2) }, turns = { 0, 1 } }
host.paths["38:" .. plotIndex(1, 2)] = {
	plots = { plotIndex(1, 1), plotIndex(1, 2) }, turns = { 0, 1 } }
applyOrders(player, PID, 7, { row(37, "MOVE_TO", 1, 2) })
check("visible combat shadow: setter stays out of capture leg", ops(37), "")
check("visible combat shadow: synthetic guard stays with setter", ops(38), "")
check("visible combat shadow: host row was held, not applied", has(lastEvent("orders"), '"escort_shadow_injected":1')
	and has(lastEvent("orders"), '"escort_shadow_refused":1'), true)

-- A nearby guard with no existing order is rescued onto the settler's current
-- tile when the settler's move is held.  This is the live t73 geometry: the
-- warrior is adjacent rather than co-located, so refusing the settler alone
-- would leave it capturable before the next export.
reset()
host.units[47] = { id = 47, kind = "UNIT_SETTLER", x = 1, y = 1, moves = 2 }
host.units[48] = { id = 48, kind = "UNIT_WARRIOR", x = 2, y = 1, moves = 2 }
host.barbarians[95] = { id = 95, kind = "UNIT_MAN_AT_ARMS", x = 2, y = 2, moves = 2 }
host.paths["47:" .. plotIndex(1, 2)] = {
	plots = { plotIndex(1, 1), plotIndex(1, 2) }, turns = { 0, 1 } }
host.paths["48:" .. plotIndex(1, 1)] = {
	plots = { plotIndex(2, 1), plotIndex(1, 1) }, turns = { 0, 1 } }
local rescueRows = { row(47, "MOVE_TO", 1, 2) }
applyOrders(player, PID, 7, rescueRows)
check("visible combat rescue: setter stays out of capture leg", ops(47), "")
check("visible combat rescue: nearby guard joins current tile", ops(48), "UNITOPERATION_MOVE_TO@1,1")
check("visible combat rescue: row is inserted before held setter", rescueRows[1].subject, 48)
check("visible combat rescue: event names both units", has(lastEvent("settler_barbarian_combat_guard_rescue"), '"settler":47')
	and has(lastEvent("settler_barbarian_combat_guard_rescue"), '"guard":48'), true)
check("visible combat rescue: shadow is applied, not counted as CIVVIS work",
	has(lastEvent("orders"), '"settler_barbarian_combat_guard_rescued":1')
	and has(lastEvent("orders"), '"escort_shadow_applied":1'), true)

-- When no guard is available, an exposed Settler still gets a proven retreat
-- rather than waiting on the combat threat's current tile.  This mirrors the
-- live turn-20 loss where the held destination was safe by the mirror but the
-- stationary Settler was already inside the host's BaseMoves envelope.
reset()
host.units[53] = { id = 53, kind = "UNIT_SETTLER", x = 1, y = 2, moves = 2 }
host.barbarians[104] = { id = 104, kind = "UNIT_WARRIOR", x = 1, y = 4, moves = 0 }
host.paths["53:" .. plotIndex(1, 3)] = {
	plots = { plotIndex(1, 2), plotIndex(1, 3) }, turns = { 0, 1 } }
host.paths["53:" .. plotIndex(1, 1)] = {
	plots = { plotIndex(1, 2), plotIndex(1, 1) }, turns = { 0, 1 } }
applyOrders(player, PID, 7, { row(53, "MOVE_TO", 1, 3) })
check("exposed combat leg retreats without a guard", ops(53), "UNITOPERATION_MOVE_TO@1,1")
check("exposed combat retreat identifies the threat", has(lastEvent("settler_capture_escape"), '"settler":53')
	and has(lastEvent("settler_capture_escape"), '"sent":[1,1]'), true)
check("exposed combat retreat is not counted as a hold", has(lastEvent("orders"), '"settler_barbarian_combat_capture_held":0'), true)

-- A co-located guard can be queued in an earlier combat frame than the
-- Settler's later safety row.  Keep that guard on the exposed current tile;
-- otherwise it leaves first and the Settler is captured before its held row
-- is even visible to the host bridge.
reset()
host.units[54] = { id = 54, kind = "UNIT_SETTLER", x = 1, y = 1, moves = 2 }
host.units[55] = { id = 55, kind = "UNIT_WARRIOR", x = 1, y = 1, moves = 2 }
host.barbarians[105] = { id = 105, kind = "UNIT_WARRIOR", x = 1, y = 3, moves = 0 }
host.paths["55:" .. plotIndex(2, 1)] = {
	plots = { plotIndex(1, 1), plotIndex(2, 1) }, turns = { 0, 1 } }
applyOrders(player, PID, 7, { row(55, "MOVE_TO", 2, 1) })
check("earlier-frame guard stays with exposed settler", ops(55), "")
check("earlier-frame guard hold names both units",
	has(lastEvent("settler_barbarian_combat_guard_hold"), '"settler":54')
	and has(lastEvent("settler_barbarian_combat_guard_hold"), '"guard":55'), true)
check("earlier-frame guard hold is counted", has(lastEvent("orders"),
	'"settler_barbarian_combat_guard_held":1'), true)

-- A nearby guard that already has an unrelated order is deliberately not
-- overwritten.  The conservative result is observable: no rescue event or
-- shadow row, and the guard keeps the route the planner supplied.
reset()
host.units[49] = { id = 49, kind = "UNIT_SETTLER", x = 1, y = 1, moves = 2 }
host.units[50] = { id = 50, kind = "UNIT_WARRIOR", x = 2, y = 1, moves = 2 }
host.barbarians[96] = { id = 96, kind = "UNIT_MAN_AT_ARMS", x = 2, y = 2, moves = 2 }
host.paths["49:" .. plotIndex(1, 2)] = {
	plots = { plotIndex(1, 1), plotIndex(1, 2) }, turns = { 0, 1 } }
host.paths["50:" .. plotIndex(3, 1)] = {
	plots = { plotIndex(2, 1), plotIndex(3, 1) }, turns = { 0, 1 } }
local noOverwriteRows = { row(49, "MOVE_TO", 1, 2), row(50, "MOVE_TO", 3, 1) }
applyOrders(player, PID, 7, noOverwriteRows)
check("visible combat no-overwrite: setter stays out of capture leg", ops(49), "")
check("visible combat no-overwrite: guard keeps its supplied route", ops(50), "UNITOPERATION_MOVE_TO@3,1")
check("visible combat no-overwrite: no rescue is synthesized", lastEvent("settler_barbarian_combat_guard_rescue"), nil)
check("visible combat no-overwrite: rescue counter stays zero", has(lastEvent("orders"), '"settler_barbarian_combat_guard_rescued":0'), true)

-- A combat unit can capture from more than one plot away on its next turn.
-- The live horse-archer loss at t43 used this geometry: the staggered-hex
-- distance was two, so an adjacency-only check let the Settler step onto the
-- exact tile the horse archer could enter.  Prefer the host path proof even
-- when the hostile has already spent its current-turn movement.
reset()
host.units[51] = { id = 51, kind = "UNIT_SETTLER", x = 1, y = 1, moves = 2 }
host.barbarians[102] = { id = 102, kind = "UNIT_HORSEMAN", x = 4, y = 1, moves = 0 }
host.paths["51:" .. plotIndex(2, 1)] = {
	plots = { plotIndex(1, 1), plotIndex(2, 1) }, turns = { 0, 1 } }
host.paths["102:" .. plotIndex(2, 1)] = {
	plots = { plotIndex(4, 1), plotIndex(3, 1), plotIndex(2, 1) }, turns = { 0, 1, 1 } }
applyOrders(player, PID, 7, { row(51, "MOVE_TO", 2, 1) })
check("two-step combat reach: setter stays out of capture leg", ops(51), "")
check("two-step combat reach: host path names the threat", has(lastEvent("settler_barbarian_combat_capture_hold"), '"hostile":102')
	and has(lastEvent("settler_barbarian_combat_capture_hold"), '"hostile_reach":"path"'), true)

-- If the host cannot answer an enemy path to a civilian-occupied plot, the
-- conservative BaseMoves/distance fallback still holds a normal two-move
-- barbarian rather than accepting a false-safe leg.
reset()
host.units[52] = { id = 52, kind = "UNIT_SETTLER", x = 1, y = 1, moves = 2 }
host.barbarians[103] = { id = 103, kind = "UNIT_HORSEMAN", x = 4, y = 1, moves = 0 }
host.paths["52:" .. plotIndex(2, 1)] = {
	plots = { plotIndex(1, 1), plotIndex(2, 1) }, turns = { 0, 1 } }
applyOrders(player, PID, 7, { row(52, "MOVE_TO", 2, 1) })
check("two-step fallback reach: setter stays out of capture leg", ops(52), "")
check("two-step fallback reach: distance names the threat", has(lastEvent("settler_barbarian_combat_capture_hold"), '"hostile":103')
	and has(lastEvent("settler_barbarian_combat_capture_hold"), '"hostile_reach":"base_moves"'), true)

-- A visible combat unit outside the adjacent-capture geometry does not freeze
-- normal expansion movement.
reset()
host.units[39] = { id = 39, kind = "UNIT_SETTLER", x = 1, y = 1, moves = 2 }
host.barbarians[98] = { id = 98, kind = "UNIT_WARRIOR", x = 4, y = 4, moves = 2 }
host.paths["39:" .. plotIndex(1, 2)] = {
	plots = { plotIndex(1, 1), plotIndex(1, 2) }, turns = { 0, 1 } }
applyOrders(player, PID, 7, { row(39, "MOVE_TO", 1, 2) })
check("distant combat: setter still moves", ops(39), "UNITOPERATION_MOVE_TO@1,2")

-- A proven co-located escort may make the exact same leg, including the
-- synthesized row from the existing host escort reconciliation.
reset()
host.cities[1] = { x = 1, y = 1 }
host.units[34] = { id = 34, kind = "UNIT_SETTLER", x = 1, y = 1, moves = 2 }
host.units[35] = { id = 35, kind = "UNIT_WARRIOR", x = 1, y = 1, moves = 2 }
host.barbarians[94] = { id = 94, kind = "UNIT_SCOUT", x = 2, y = 3, moves = 0 }
host.paths["34:" .. plotIndex(1, 2)] = {
	plots = { plotIndex(1, 1), plotIndex(1, 2) }, turns = { 0, 1 } }
host.paths["35:" .. plotIndex(1, 2)] = {
	plots = { plotIndex(1, 1), plotIndex(1, 2) }, turns = { 0, 1 } }
applyOrders(player, PID, 7, { row(34, "MOVE_TO", 1, 2) })
check("proven escort: guard shares exposed leg", ops(35), "UNITOPERATION_MOVE_TO@1,2")
check("proven escort: setter still moves", ops(34), "UNITOPERATION_MOVE_TO@1,2")
check("proven escort: no scout capture hold", lastEvent("settler_scout_capture_hold"), nil)

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("\nall host-board checks passed")
