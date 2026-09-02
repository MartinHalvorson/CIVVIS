-- The per-unit order queue: a unit's later orders wait for its earlier ones.
--
-- ⚠ Loads the SHIPPED `CivvisControlAgent.lua` and drives its own
-- `applyOrders` / `CivvisQueue` against a fake host, so the sequencing the
-- live seat relies on is the sequencing the agent performs — not a
-- re-implementation that would pass while the agent kept the old
-- one-order-per-unit behaviour.
--
-- What is checked:
--   1. the FIRST order per unit is issued at once, every later one is queued;
--   2. a queued strike waits until the walk before it has arrived, then fires;
--   3. a unit whose first order was refused gets no follow-up (named);
--   4. a queued unit the host reports gone is refused by name, not dereferenced;
--   5. a settler's refused FOUND_CITY is retried behind its walk;
--   6. the turn is held while a queue is pending and released when it drains;
--   7. the stall cap gives up by name;
--   8. every unmentioned combat unit is given a holding order, regardless of
--      location. A held unit with no order at all blocks the end of the turn;
--   8b. an unmentioned civilian is told to skip for the same reason:
--      exclusion is not a disposition.
--   9. an asynchronous Governor assignment is submitted once per turn, not
--      once per replan frame, while still retrying on the next turn.
--
-- Run: lua5.1 tools/civ6_control/mod/order_queue_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end

local EXPORTS = {
	CivvisApplyOrders = true, CivvisQueue = true, CivvisResolveActions = true,
	CivvisApplyOrder = true,
}
-- Real tables the agent indexes with real keys.
local LOG = {}
Automation = { Log = function(line) LOG[#LOG + 1] = line end }
UnitOperationTypes = { PARAM_X = "x", PARAM_Y = "y" }
ActivityTypes = { ACTIVITY_OPERATION = "operation" }
UnitCommandTypes = {}
Map = {
	GetPlotDistance = function(x1, y1, x2, y2)
		-- Manhattan-ish is enough for a radius test on an offset grid.
		return math.max(math.abs(x1 - x2), math.abs(y1 - y2))
	end,
	GetPlot = function() return nil end,
}
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
	return stub()
end })
setmetatable(_G, { __index = function(_, k)
	if EXPORTS[k] then return rawget(_G, k) end
	return stub()
end })

-- ---------------------------------------------------------------- fake host
local host = { units = {}, ops = {}, commands = {}, allow_cancel = false,
	defer_cancel = false,
	refuse = {}, contacts = {}, governor_requests = {}, cities = {} }
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
		GetAttacksRemaining = function() return u.attacks or 1 end,
		GetActivityType = function() return u.activity end,
	}
end
UnitManager = {
	GetUnit = function(pid, id)
		local u = host.units[id]
		if u == nil or u.gone then return nil end
		return unitObject(u)
	end,
	GetActivityType = function(unit)
		return host.units[unit.GetID()].activity
	end,
	CanStartOperation = function(unit, hash, _, params)
		local id = unit.GetID()
		if host.refuse[id] and host.refuse[id][hash] then return false end
		return true
	end,
	RequestOperation = function(unit, hash, params)
		local u = host.units[unit.GetID()]
		host.ops[#host.ops + 1] = { id = u.id, op = hash,
			x = params and params.x or nil, y = params and params.y or nil }
		if hash == "UNITOPERATION_MOVE_TO" then
			-- Asynchronous, like the host: the unit arrives only when the
			-- test says so (`host.arrive`), unless the walk was priced dead.
			u.pendingX, u.pendingY = params.x, params.y
			if u.active_operation then u.activity = "operation" end
		end
	end,
	CanStartCommand = function(_, hash)
		return hash == "UNITCOMMAND_CANCEL" and host.allow_cancel
	end,
	RequestCommand = function(unit, hash)
		host.commands[#host.commands + 1] = { id = unit.GetID(), command = hash }
		if hash == "UNITCOMMAND_CANCEL" and not host.defer_cancel then
			host.units[unit.GetID()].activity = nil
		end
	end,
}
PlayerOperations = {
	PARAM_GOVERNOR_TYPE = "governor_type",
	PARAM_PLAYER_ONE = "player_one",
	PARAM_CITY_DEST = "city_dest",
	ASSIGN_GOVERNOR = "assign_governor",
}
GameInfo.Governors = {
	GOVERNOR_THE_MERCHANT = {
		Hash = "GOVERNOR_THE_MERCHANT", Index = 17,
	},
}
CityManager = {
	GetCity = function(owner, id)
		return host.cities[id] and { owner = owner, id = id } or nil
	end,
}
UI = {
	RequestPlayerOperation = function(pid, operation, params)
		host.governor_requests[#host.governor_requests + 1] = {
			pid = pid, operation = operation, params = params,
		}
	end,
}
function host.arrive(id)
	local u = host.units[id]
	if u.pendingX ~= nil then
		u.x, u.y = u.pendingX, u.pendingY
		u.pendingX, u.pendingY = nil, nil
		u.moves = math.max(0, (u.moves or 0) - 1)
	end
end
function host.deactivate(id)
	host.units[id].activity = nil
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
	GetGovernors = function()
		return {
			HasGovernor = function(_, hash) return hash == "GOVERNOR_THE_MERCHANT" end,
			GetGovernor = function() return nil end,
		}
	end,
	GetUnits = function()
		local objs = {}
		for _, u in pairs(host.units) do
			if not u.gone then objs[#objs + 1] = unitObject(u) end
		end
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
	-- One barbarian seat with the hostile units the test plants.
	return setmetatable({
		IsBarbarian = function() return true end,
		GetUnits = function()
			local objs = {}
			for _, u in ipairs(host.contacts) do objs[#objs + 1] = unitObject(u) end
			return { Members = members(objs) }
		end,
		GetCities = function() return { Members = members({}) } end,
	}, { __index = function() return stub() end })
end })
PlayerManager = { GetAliveIDs = function() return { PID, 63 } end,
                  GetAliveMajorIDs = function() return { PID } end }
PlayersVisibility = setmetatable({}, { __index = function()
	return { IsVisible = function() return true end, IsRevealed = function() return true end }
end })
Game = { GetLocalPlayer = function() return PID end, GetCurrentGameTurn = function() return 7 end }

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtime_err = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtime_err))

local applyOrders = rawget(_G, "CivvisApplyOrders")
local queue = rawget(_G, "CivvisQueue")
local resolveActions = rawget(_G, "CivvisResolveActions")
assert(type(applyOrders) == "function", "CivvisApplyOrders is not exported")
assert(type(queue) == "table", "CivvisQueue is not exported")
assert(type(resolveActions) == "function", "CivvisResolveActions is not exported")
resolveActions()

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
		if id == nil or o.id == id then out[#out + 1] = o.op end
	end
	return table.concat(out, ",")
end
local function lastEvent(kind)
	for i = #LOG, 1, -1 do
		if LOG[i]:find('"kind":"' .. kind .. '"', 1, true) then return LOG[i] end
	end
	return nil
end
local function field(line, name)
	if line == nil then return nil end
	local v = line:match('"' .. name .. '":(%-?%d+)')
	return v and tonumber(v) or line:match('"' .. name .. '":"([^"]*)"')
end
local function row(subject, verb, x, y)
	return { kind = "unit", subject = subject, verb = verb, x = x, y = y }
end
local function reset()
	host.units, host.ops, host.commands, host.refuse, host.contacts = {}, {}, {}, {}, {}
	host.governor_requests, host.cities = {}, {}
	host.allow_cancel, host.defer_cancel = false, false
	queue.reset(7)
end

-- 1 + 2. Walk then strike: the strike waits for the walk, then fires.
reset()
host.units[10] = { id = 10, kind = "UNIT_ARCHER", x = 1, y = 1, moves = 2 }
applyOrders(player, PID, 7, { row(10, "MOVE_TO", 2, 1), row(10, "RANGE_ATTACK", 4, 1) })
check("first order issued at once", ops(10), "UNITOPERATION_MOVE_TO")
check("strike queued behind the walk", queue.pendingCount(), 1)
check("orders event reports queued", field(lastEvent("orders"), "queued"), 1)
queue.drain(player, PID, 7)
check("strike does not fire before arrival", ops(10), "UNITOPERATION_MOVE_TO")
host.arrive(10)
queue.drain(player, PID, 7)
check("strike fires once the walk arrived", ops(10), "UNITOPERATION_MOVE_TO,UNITOPERATION_RANGE_ATTACK")
check("queue drained", queue.pendingCount(), 0)
check("orders_queue reports the landed strike", field(lastEvent("orders_queue"), "strikes_landed"), 1)

-- 2b. Reaching the requested plot while the host's operation is still active
-- is not settled. Civilization VI can expose the unit at its destination and
-- still ignore a follow-up operation until the path deactivates. Once the
-- destination is reached, the bridge may cancel that landed path through the
-- host's own cancel command and run the dependent order in the same turn.
reset()
host.units[142] = {
	id = 142, kind = "UNIT_WARRIOR", x = 1, y = 1, moves = 2,
	active_operation = true,
}
applyOrders(player, PID, 7, { row(142, "MOVE_TO", 2, 1), row(142, "FORTIFY") })
queue.drain(player, PID, 7)
check("active operation protects an en-route follow-up", ops(142), "UNITOPERATION_MOVE_TO")
host.arrive(142)
host.allow_cancel = false
queue.drain(player, PID, 7)
check("landed operation waits when cancellation is unavailable", ops(142),
	"UNITOPERATION_MOVE_TO")
check("uncancellable landed operation keeps the queue pending", queue.pendingCount(), 1)
host.allow_cancel = true
queue.drain(player, PID, 7)
check("landed operation is cancelled before the follow-up", host.commands[1]
	and host.commands[1].command, "UNITCOMMAND_CANCEL")
check("follow-up runs after landed operation cancellation", ops(142),
	"UNITOPERATION_MOVE_TO,UNITOPERATION_FORTIFY")
check("landed-operation queue drains", queue.pendingCount(), 0)

-- A cancel request may be asynchronous too.  Do not turn its successful
-- return into permission to race the still-active operation.
reset()
host.units[143] = {
	id = 143, kind = "UNIT_WARRIOR", x = 1, y = 1, moves = 2,
	active_operation = true,
}
applyOrders(player, PID, 7, { row(143, "MOVE_TO", 2, 1), row(143, "FORTIFY") })
host.arrive(143)
host.allow_cancel, host.defer_cancel = true, true
queue.drain(player, PID, 7)
check("asynchronous cancellation keeps the follow-up pending", ops(143),
	"UNITOPERATION_MOVE_TO")
check("asynchronous cancellation does not race the follow-up", queue.pendingCount(), 1)
host.deactivate(143)
queue.drain(player, PID, 7)
check("follow-up runs after cancellation settles", ops(143),
	"UNITOPERATION_MOVE_TO,UNITOPERATION_FORTIFY")
check("asynchronous cancellation queue drains", queue.pendingCount(), 0)

-- 3. A refused first order takes its follow-ups with it, by name.
reset()
host.units[11] = { id = 11, kind = "UNIT_WARRIOR", x = 5, y = 5, moves = 2 }
host.refuse[11] = { UNITOPERATION_MOVE_TO = true }
applyOrders(player, PID, 7, { row(11, "MOVE_TO", 6, 5), row(11, "ATTACK", 7, 5) })
check("refused walk issues nothing", ops(11), "")
check("no follow-up queued after a refused first order", queue.pendingCount(), 0)
check("the dropped follow-up is named", (lastEvent("orders") or ""):find("queue_prior_refused", 1, true) ~= nil, true)

-- 4. A queued unit that dies is a named refusal, never a dereference.
reset()
host.units[12] = { id = 12, kind = "UNIT_WARRIOR", x = 5, y = 5, moves = 2 }
applyOrders(player, PID, 7, { row(12, "MOVE_TO", 6, 5), row(12, "FORTIFY") })
host.units[12].gone = true
queue.drain(player, PID, 7)
check("gone unit's follow-up refused", queue.pendingCount(), 0)
check("gone unit named", (lastEvent("orders_queue") or ""):find("unit_gone:12", 1, true) ~= nil, true)

-- 5. A settler's refused found is retried behind its walk and lands.
reset()
host.units[13] = { id = 13, kind = "UNIT_SETTLER", x = 3, y = 3, moves = 2 }
host.refuse[13] = { UNITOPERATION_FOUND_CITY = true }
applyOrders(player, PID, 7, { row(13, "MOVE_TO", 4, 3), row(13, "FOUND_CITY") })
check("found tried first, refused; walk issued", ops(13), "UNITOPERATION_MOVE_TO")
check("found queued behind the walk", queue.pendingCount(), 1)
host.refuse[13] = nil
host.arrive(13)
queue.drain(player, PID, 7)
check("found retried on arrival", ops(13), "UNITOPERATION_MOVE_TO,UNITOPERATION_FOUND_CITY")

-- 5b. A target-specific ranged refusal is named with the host's probe and
-- the unit state read at the same instant. A generic RANGE_ATTACK counter
-- cannot distinguish a stale target/LOS decision from an actuation mismatch.
reset()
host.units[20] = { id = 20, kind = "UNIT_ARCHER", x = 8, y = 8, moves = 2, attacks = 1 }
host.refuse[20] = { UNITOPERATION_RANGE_ATTACK = true }
applyOrders(player, PID, 7, { row(20, "RANGE_ATTACK", 10, 8) })
check("refused ranged shot issues nothing", ops(20), "")
local refusedRange = lastEvent("range_attack_refused") or ""
check("ranged refusal names unit", refusedRange:find('"unit":20', 1, true) ~= nil, true)
check("ranged refusal names target", refusedRange:find('"x":10', 1, true) ~= nil, true)
check("ranged refusal samples moves", refusedRange:find('"moves":2', 1, true) ~= nil, true)
check("ranged refusal carries host probe", refusedRange:find('"why":"', 1, true) ~= nil, true)

-- 6. Spent movement refuses what needs it, by name.
reset()
host.units[14] = { id = 14, kind = "UNIT_WARRIOR", x = 5, y = 5, moves = 1 }
applyOrders(player, PID, 7, { row(14, "MOVE_TO", 6, 5), row(14, "ATTACK", 7, 5) })
host.arrive(14) -- moves -> 0
queue.drain(player, PID, 7)
check("strike with no movement left refused", ops(14), "UNITOPERATION_MOVE_TO")
check("named queue_no_moves", (lastEvent("orders_queue") or ""):find("queue_no_moves", 1, true) ~= nil, true)

-- 6b. A move that never reaches its target refuses every dependent follow-up.
-- This is the builder failure seen in the live Civ VI trace: the host ended the
-- MOVE_TO at the origin, then the queued IMPROVE was evaluated on the city
-- centre instead of the farm tile CIVVIS had planned.
reset()
host.units[141] = { id = 141, kind = "UNIT_BUILDER", x = 5, y = 5, moves = 1 }
applyOrders(player, PID, 7, { row(141, "MOVE_TO", 6, 5), row(141, "IMPROVE:IMPROVEMENT_FARM") })
check("stop-short move issues only the opening walk", ops(141), "UNITOPERATION_MOVE_TO")
host.units[141].moves = 0 -- the host ended the move without reaching (6, 5)
queue.drain(player, PID, 7)
check("stop-short move never improves the origin", ops(141), "UNITOPERATION_MOVE_TO")
check("stop-short follow-up is named", (lastEvent("orders_queue") or ""):find("queue_prior_not_arrived", 1, true) ~= nil, true)
check("stop-short follow-up is removed", queue.pendingCount(), 0)

-- 7. The stall cap gives up by name.
reset()
host.units[15] = { id = 15, kind = "UNIT_WARRIOR", x = 5, y = 5, moves = 2 }
applyOrders(player, PID, 7, { row(15, "MOVE_TO", 6, 5), row(15, "ATTACK", 7, 5) })
queue.giveUp(7)
check("give-up empties the queue", queue.pendingCount(), 0)
check("give-up named queue_stalled", (lastEvent("orders_queue") or ""):find("queue_stalled", 1, true) ~= nil, true)

-- 8. CIVVIS owns movement: both a soldier beside a hostile and one far away
-- hold until the planner gives either one an explicit destination.
--
-- ⚠⚠⚠ THIS ONCE ASSERTED THE HELD SOLDIER GOT NOTHING, AND NOTHING IS WHAT
-- BLOCKED THE TURN. Civilization VI will not end a turn while a unit still
-- awaits orders, so a soldier CIVVIS did not mention had no disposition at
-- all. Measured 2026-08-28, run
-- civvis-20260828T161408Z at turn 105 with five such units:
-- blocked(ENDTURN_BLOCKING_UNITS) -> dismissed(forced) -> residual_unblock ->
-- blocked, repeating until the wedge watchdog killed a game that had reached
-- seven cities. Held means HELD, which is an order the engine accepts.
reset()
host.units[16] = { id = 16, kind = "UNIT_WARRIOR", x = 5, y = 5, moves = 2 }   -- near
host.units[17] = { id = 17, kind = "UNIT_WARRIOR", x = 30, y = 30, moves = 2 } -- far
applyOrders(player, PID, 7, {})
check("near soldier is given a holding order", ops(16), "UNITOPERATION_FORTIFY")
check("far soldier is given a holding order", ops(17), "UNITOPERATION_FORTIFY")
check("orders event counts every unmentioned hold",
      field(lastEvent("orders"), "unmentioned_held"), 2)
check("every unmentioned hold is accepted",
      field(lastEvent("orders"), "unmentioned_held_applied"), 2)

-- 8b. An unmentioned CIVILIAN is held with SKIP_TURN — a settler that wanders
-- never founds — but exclusion is not a disposition. Civilization
-- VI will not end a turn while any unit awaits orders, civilian included, so it
-- has to be told to skip. Seven of the nineteen ENDTURN_BLOCKING_UNITS turns in
-- run civvis-20260828T165926Z had an unordered civilian on them.
reset()
host.units[18] = { id = 18, kind = "UNIT_SETTLER", x = 40, y = 40, moves = 2 }
host.units[19] = { id = 19, kind = "UNIT_BUILDER", x = 41, y = 41, moves = 2 }
applyOrders(player, PID, 7, {})
check("idle settler is told to skip", ops(18), "UNITOPERATION_SKIP_TURN")
check("idle builder is told to skip", ops(19), "UNITOPERATION_SKIP_TURN")
check("the orders event counts both", field(lastEvent("orders"), "civilians_skipped"), 2)

-- 9. Governor assignment is a player operation, not a synchronous mutation.
-- Replan frames see the old roster until the host exports the next turn; the
-- bridge must avoid stacking identical requests while preserving that retry.
reset()
host.cities[42] = true
local applyOrder = rawget(_G, "CivvisApplyOrder")
local governorRow = { kind = "governor_assign", subject = 42,
	verb = "GOVERNOR_THE_MERCHANT", x = PID }
local assigned, assignedWhy = applyOrder(player, PID, governorRow, 7)
check("first governor assignment is submitted", assigned, true)
check("first governor assignment has no refusal", assignedWhy, "GOVERNOR_THE_MERCHANT")
local duplicate, duplicateWhy = applyOrder(player, PID, governorRow, 7)
check("same-turn governor assignment is held", duplicate, false)
check("same-turn governor refusal is named", duplicateWhy, "governor_assign_pending")
check("same-turn governor request is submitted once", #host.governor_requests, 1)
local retried = applyOrder(player, PID, governorRow, 8)
check("unassigned governor retries next turn", retried, true)
check("next-turn governor request is submitted", #host.governor_requests, 2)

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("\nall order-queue checks passed")
