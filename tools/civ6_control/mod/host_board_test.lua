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
-- Plot index = y * 100 + x on this fake map.
local function plotIndex(x, y) return y * 100 + x end
Map = {
	GetPlotDistance = function(x1, y1, x2, y2) return math.max(math.abs(x1 - x2), math.abs(y1 - y2)) end,
	GetPlot = function() return nil end,
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

local host = { units = {}, ops = {}, cmds = {}, paths = {}, queued = {}, blocked = {} }
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
	GetCities = function() return { Members = members({}) } end,
	GetDiplomacy = function() return { IsAtWarWith = function() return false end } end,
	GetScore = function() return 0 end,
	GetTreasury = function() return { GetGoldBalance = function() return 0 end } end,
	IsTurnActive = function() return true end,
}, { __index = function() return stub() end })
Players = setmetatable({}, { __index = function(_, pid)
	if pid == PID then return player end
	return setmetatable({ IsBarbarian = function() return true end,
		GetUnits = function() return { Members = members({}) } end,
		GetCities = function() return { Members = members({}) } end },
		{ __index = function() return stub() end })
end })
PlayerManager = { GetAliveIDs = function() return { PID, 63 } end, GetAliveMajorIDs = function() return { PID } end }
PlayersVisibility = setmetatable({}, { __index = function()
	return { IsVisible = function() return true end, IsRevealed = function() return true end }
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
	host.units, host.ops, host.cmds, host.paths, host.queued, host.blocked, LOG = {}, {}, {}, {}, {}, {}, {}
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

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("\nall host-board checks passed")
