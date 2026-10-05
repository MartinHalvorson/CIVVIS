-- ⚠ lupa's Lua 5.1.5 (the local runner) barriers the CLOSURE in
-- lua_setupvalue, not the shared UpVal, so a value written into an upvalue the
-- collector has already marked can be freed while still referenced: the merge
-- 7bd809bdb crashed order_queue_test 6/6 (SIGSEGV/BUS/ABRT/TRAP, varying) right
-- after its agent reload, and heap layout decided which tree crashed (-4f
-- root-caused it, 2026-10-05). A full collection first leaves nothing marked.
-- Harmless under the real lua5.1 CI runs.
do
	local setupvalue = debug.setupvalue
	debug.setupvalue = function(...)
		collectgarbage("collect")
		return setupvalue(...)
	end
end

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
--   10. a MOVE_TO -> FORTIFY handoff gets one settlement pass so an accepted
--      host operation cannot be cancelled before fortification lands.
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

-- A host deactivation event can precede its delayed movement callbacks. The
-- turn-204 capital attack was discarded while its tank was still on the
-- origin, even though the tank reached the attack tile later that tick.
reset()
host.units[144] = { id = 144, kind = "UNIT_TANK", x = 5, y = 5, moves = 5 }
applyOrders(player, PID, 7, { row(144, "MOVE_TO", 6, 5), row(144, "ATTACK", 7, 5) })
queue.noteUnitEvent(PID, player, 144)
queue.drain(player, PID, 7)
check("early host event keeps the attack queued", queue.pendingCount(), 1)
check("early host event does not fire the attack", ops(144), "UNITOPERATION_MOVE_TO")
host.arrive(144)
queue.drain(player, PID, 7)
check("late move callback releases the attack", ops(144),
	"UNITOPERATION_MOVE_TO,UNITOPERATION_MOVE_TO")
check("late arrival drains the queue", queue.pendingCount(), 0)
check("late arrival lands the queued strike", field(lastEvent("orders_queue"), "strikes_landed"), 1)

reset()
host.units[145] = { id = 145, kind = "UNIT_TANK", x = 5, y = 5, moves = 5 }
applyOrders(player, PID, 7, { row(145, "MOVE_TO", 6, 5), row(145, "ATTACK", 7, 5) })
queue.noteUnitEvent(PID, player, 145)
for _ = 1, 29 do queue.drain(player, PID, 7) end
check("a truly stalled walk is held only through grace", queue.pendingCount(), 1)
queue.drain(player, PID, 7)
check("stalled attack is refused after grace", queue.pendingCount(), 0)
check("stalled refusal remains named", (lastEvent("orders_queue") or ""):
	find("queue_prior_not_arrived", 1, true) ~= nil, true)

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
check("cancelled path gets a settlement pass", ops(142), "UNITOPERATION_MOVE_TO")
check("cancelled path keeps follow-up pending", queue.pendingCount(), 1)
queue.drain(player, PID, 7)
check("follow-up runs after landed operation settles", ops(142),
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
check("asynchronous cancellation gets a settlement pass", ops(143), "UNITOPERATION_MOVE_TO")
check("asynchronous cancellation keeps follow-up pending", queue.pendingCount(), 1)
queue.drain(player, PID, 7)
check("follow-up runs after cancellation settles", ops(143),
	"UNITOPERATION_MOVE_TO,UNITOPERATION_FORTIFY")
check("asynchronous cancellation queue drains", queue.pendingCount(), 0)

-- 2c. A path that cannot land this turn holds its follow-up only for the
-- grace period. The host keeps a multi-turn (or leftover) MOVE_TO active until
-- the next turn; the follow-up would be refused as not arrived anyway, so
-- waiting longer only ran the turn into the stall cap (civvis-20261004T083931Z
-- t84: a Scout's queued PILLAGE held the turn 37 s).
reset()
host.units[144] = {
	id = 144, kind = "UNIT_SCOUT", x = 1, y = 1, moves = 2,
	active_operation = true,
}
applyOrders(player, PID, 7, { row(144, "MOVE_TO", 9, 1), row(144, "PILLAGE") })
local graceTicks = 30 -- the agent's OrderQueueGraceTicks default; no config is installed here
for _ = 1, graceTicks - 1 do queue.drain(player, PID, 7) end
check("en-route operation still holds within the grace", queue.pendingCount(), 1)
queue.drain(player, PID, 7)
check("past the grace an unlanded operation releases its follow-up", queue.pendingCount(), 0)
check("released follow-up is never issued", ops(144), "UNITOPERATION_MOVE_TO")
check("released follow-up is named not-arrived",
	(lastEvent("orders_queue") or ""):find("queue_prior_not_arrived", 1, true) ~= nil, true)

-- 2d. An opening walk the host cannot path is answered a few ticks in, not
-- at the grace: one plot (or none) from `GetMoveToPathEx` means the leg will
-- not happen (civvis-20261004T083931Z: 564 such no-ops held their frame to the
-- 30-tick grace). A walk with a real path keeps waiting, and a host that
-- cannot be asked decides nothing.
reset()
-- Every leg has a path when it is issued (`capToTurn` refuses one without);
-- 145's is gone afterwards, which is what the live no-ops reported.
host.paths = { [145] = 4, [146] = 4 }
Map.GetPlotIndex = function(x, y) return y * 100 + x end
UnitManager.GetMoveToPathEx = function(unit)
	local n = host.paths[unit.GetID()];
	if n == nil then return nil end
	local plots = {};
	for i = 1, n do plots[i] = i end
	return { plots = plots, turns = {} };
end
host.units[145] = { id = 145, kind = "UNIT_WARRIOR", x = 1, y = 1, moves = 2 }
host.units[146] = { id = 146, kind = "UNIT_WARRIOR", x = 4, y = 4, moves = 2 }
host.units[147] = { id = 147, kind = "UNIT_WARRIOR", x = 7, y = 7, moves = 2 }
applyOrders(player, PID, 7, {
	row(145, "MOVE_TO", 3, 1), row(146, "MOVE_TO", 6, 4), row(147, "MOVE_TO", 9, 7),
})
host.paths[145] = 1
for _ = 1, 7 do queue.drain(player, PID, 7) end
check("no probe before the probe tick", queue.pendingCount(), 3)
queue.drain(player, PID, 7)
check("an unpathed walk is answered at the probe tick", queue.pendingCount(), 2)
local noop = lastEvent("move_noop") or ""
check("the early no-op is named for its unit", noop:find('"unit":145', 1, true) ~= nil, true)
check("the early no-op reports its tick", noop:find('"ticks":8', 1, true) ~= nil, true)
for _ = 1, 20 do queue.drain(player, PID, 7) end
check("a pathed walk and an unaskable host keep waiting", queue.pendingCount(), 2)
UnitManager.GetMoveToPathEx = nil
Map.GetPlotIndex = nil
host.paths = nil

-- 2d'. RECORD-ONLY stalled-operation probe (G77: 22 of 23 full-grace
-- `unknown` no-ops sat in ACTIVITY_OPERATION on their origin). At the probe
-- tick, a pathed opening walk still on its origin with its movement intact and
-- an active operation is marked `stall_probe`; it then resolves `stepped` the
-- moment it leaves the origin, or carries `stall_probe` on its grace no-op.
-- A walk that is not in an operation is never marked. Nothing changes the queue.
reset()
host.paths = { [150] = 2, [151] = 2, [152] = 2 }
Map.GetPlotIndex = function(x, y) return y * 100 + x end
UnitManager.GetMoveToPathEx = function(unit)
	local n = host.paths[unit.GetID()];
	if n == nil then return nil end
	local plots = {};
	for i = 1, n do plots[i] = i end
	return { plots = plots, turns = { 0, 1 } };
end
host.units[150] = { id = 150, kind = "UNIT_MUSKETMAN", x = 1, y = 1, moves = 2, active_operation = true }
host.units[151] = { id = 151, kind = "UNIT_MUSKETMAN", x = 4, y = 4, moves = 2, active_operation = true }
host.units[152] = { id = 152, kind = "UNIT_WARRIOR", x = 7, y = 7, moves = 2 }
applyOrders(player, PID, 7, {
	row(150, "MOVE_TO", 2, 1), row(151, "MOVE_TO", 5, 4), row(152, "MOVE_TO", 8, 7),
})
local function countEvents(kind)
	local n = 0
	for _, line in ipairs(LOG) do
		if line:find('"kind":"' .. kind .. '"', 1, true) then n = n + 1 end
	end
	return n
end
local probesBefore = countEvents("stall_probe")
for _ = 1, 8 do queue.drain(player, PID, 7) end
check("both stalled operations are marked at the probe tick", countEvents("stall_probe") - probesBefore, 2)
check("the mark names its tick", (lastEvent("stall_probe") or ""):find('"tick":8', 1, true) ~= nil, true)
check("the mark says it was not released", (lastEvent("stall_probe") or ""):find('"released":false', 1, true) ~= nil, true)
check("the probe changes nothing in the queue", queue.pendingCount(), 3)
host.units[150].x = 2
host.units[150].moves = 1
queue.drain(player, PID, 7)
local resolved = lastEvent("stall_probe_resolved") or ""
check("a stalled operation that steps is resolved stepped", resolved:find('"outcome":"stepped"', 1, true) ~= nil
	and resolved:find('"unit":150', 1, true) ~= nil, true)
check("…with the probe tick and the step tick", resolved:find('"probe_tick":8', 1, true) ~= nil
	and resolved:find('"tick":9', 1, true) ~= nil, true)
for _ = 1, 25 do queue.drain(player, PID, 7) end
local graceNoop
for i = #LOG, 1, -1 do
	if LOG[i]:find('"kind":"move_noop"', 1, true) and LOG[i]:find('"unit":151', 1, true) then graceNoop = LOG[i]; break end
end
check("one that never steps carries its probe on the grace no-op",
	graceNoop ~= nil and graceNoop:find('"stall_probe":8', 1, true) ~= nil, true)
check("a walk not in an operation is never marked",
	(function() for _, l in ipairs(LOG) do if l:find('"kind":"stall_probe"', 1, true) and l:find('"unit":152', 1, true) then return true end end return false end)(), false)
check("each mark resolves once", countEvents("stall_probe_resolved"), 1)
UnitManager.GetMoveToPathEx = nil
Map.GetPlotIndex = nil
host.paths = nil

-- 2e. WorldInput.lua:884 does not ask for a movement path while the game
-- core is busy. An accepted request can transiently have no queryable path
-- before the host actually walks it; that is not an early no-op verdict.
reset()
host.units[148] = { id = 148, kind = "UNIT_BOMBARD", x = 1, y = 1, moves = 3 }
host.busy, host.path_queries = false, 0
UI.IsGameCoreBusy = function() return host.busy end
Map.GetPlotIndex = function(x, y) return y * 100 + x end
UnitManager.GetMoveToPathEx = function()
	host.path_queries = host.path_queries + 1
	local plots = host.busy and {} or { 101, 102, 103 }
	return { plots = plots, turns = {} }
end
applyOrders(player, PID, 7, { row(148, "MOVE_TO", 3, 1) })
host.busy = true
local beforeProbe = host.path_queries
local beforeNoop = lastEvent("move_noop")
for _ = 1, 8 do queue.drain(player, PID, 7) end
check("a busy core does not answer the early path probe", host.path_queries, beforeProbe)
check("a busy core keeps the opening watch", queue.pendingCount(), 1)
check("a busy core is not labelled an early no-op", lastEvent("move_noop"), beforeNoop)
host.busy = false
queue.drain(player, PID, 7)
check("the probe is retried when the core is idle", host.path_queries > beforeProbe, true)
check("a restored path keeps waiting for arrival", queue.pendingCount(), 1)
host.arrive(148)
queue.drain(player, PID, 7)
check("the deferred opening watch releases on arrival", queue.pendingCount(), 0)

-- A core that stays busy does not create an unbounded queue: the existing
-- grace/turn cap still owns the terminal answer. Record busy in its evidence
-- so the verdict is not mistaken for an idle host's authoritative no-path.
reset()
host.units[149] = { id = 149, kind = "UNIT_BOMBARD", x = 1, y = 1, moves = 3 }
host.busy = false
applyOrders(player, PID, 7, { row(149, "MOVE_TO", 3, 1) })
host.busy = true
for _ = 1, 30 do queue.drain(player, PID, 7) end
check("a permanently busy opening watch remains bounded", queue.pendingCount(), 0)
check("the bounded verdict records a busy game core",
	(lastEvent("move_noop") or ""):find('"core_busy":true', 1, true) ~= nil, true)
host.busy = false
UnitManager.GetMoveToPathEx = nil
Map.GetPlotIndex = nil
UI.IsGameCoreBusy = nil

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

-- A completed final move must wake a previously requested end turn even
-- after all queued follow-ups were drained. No game-core publish is required.
local settledTickCalls = 0
for i = 1, 20 do
    local name = debug.getupvalue(queue.onUnitSettled, i)
    if name == "tick" then
        debug.setupvalue(queue.onUnitSettled, i, function() settledTickCalls = settledTickCalls + 1 end)
    elseif name == "ensureStarted" then
        debug.setupvalue(queue.onUnitSettled, i, function() end)
    elseif name == nil then break end
end
queue.count = 0
local requested = {}
UI.RequestAction = function(...)
    local action, parameters = ...
    requested[#requested + 1] = { action = action, parameters = parameters, argc = select("#", ...) }
end
ActionTypes = { ACTION_ENDTURN = "end_turn" }
local parameters = { REASON = "UserForced" }
queue.requestEndTurn(7, parameters)
check("end-turn submission arms settlement retry", queue.endTurnRetryTurn, 7)
check("end-turn action reaches host", requested[1].action, "end_turn")
check("forced request parameters survive", requested[1].parameters, parameters)
queue.requestEndTurn(7)
check("ordinary request keeps the single-argument host signature", requested[2].argc, 1)

-- A completed request is still pending in the host while movement callbacks
-- and other UI ticks arrive. Match ActionPanel's automatic-end-turn guard.
UI.HasSentTurnComplete = function() return true end
queue.requestEndTurn(8)
queue.requestEndTurn(8, parameters)
check("pending completion suppresses ordinary and forced duplicates", #requested, 2)
check("suppressed requests do not replace the retry turn", queue.endTurnRetryTurn, 7)
UI.HasSentTurnComplete = function() return false end
queue.requestEndTurn(7)
check("host rejection or cleared completion permits retry", #requested, 3)
check("retry preserves ordinary host signature", requested[3].argc, 1)
UI.HasSentTurnComplete = nil
queue.requestEndTurn(7, parameters)
check("older hosts without the completion query retain submission", #requested, 4)
check("fallback preserves forced parameters", requested[4].parameters, parameters)
-- A host that repeatedly clears its completion flag must not be flooded by
-- settlement/blocker callbacks in the same small wall-clock interval.
local now = 100
UI.GetElapsedTime = function() return now end
UI.HasSentTurnComplete = function() return false end
local before = #requested
queue.requestEndTurn(7)
for i = 1, 100 do queue.requestEndTurn(7, parameters) end
check("one native request during a callback burst", #requested - before, 1)
local deferredLogs = 0
for _, line in ipairs(LOG) do
    if line:find('"kind":"end_turn_retry_deferred"', 1, true) then deferredLogs = deferredLogs + 1 end
end
check("callback burst emits one diagnostic", deferredLogs, 1)
now = 100.24
queue.requestEndTurn(7)
check("retry waits through the bounded interval", #requested - before, 1)
now = 100.25
queue.requestEndTurn(7, parameters)
check("rejected completion can retry after quarter second", #requested - before, 2)
check("delayed forced request keeps parameters", requested[#requested].parameters, parameters)
queue.requestEndTurn(8)
check("a new turn does not inherit retry delay", #requested - before, 3)
now = 10
queue.requestEndTurn(8)
check("a reset native clock cannot strand a turn", #requested - before, 4)
UI.GetElapsedTime = function() error("clock unavailable") end
queue.requestEndTurn(8)
check("missing native clock keeps the existing fallback", #requested - before, 5)
UI.GetElapsedTime = function() return 0/0 end
queue.requestEndTurn(8)
check("invalid clock keeps fallback", #requested - before, 6)
UI.GetElapsedTime = nil
queue.requestEndTurn(7)
queue.onUnitSettled(PID, 10)
check("final settled move retries requested turn with empty queue", settledTickCalls, 1)
queue.onUnitSettled(PID + 1, 10)
check("rival movement cannot retry our turn", settledTickCalls, 1)
queue.endTurnRetryTurn = 6
queue.onUnitSettled(PID, 10)
check("old-turn request cannot trigger a new-turn tick", settledTickCalls, 1)
queue.endTurnRetryTurn = nil
queue.onUnitSettled(PID, 10)
check("movement before any end-turn request adds no tick", settledTickCalls, 1)
queue.count = 1
queue.pending[10] = { ready = false }
queue.onUnitSettled(PID, 10)
check("queued follow-up still ticks without an end-turn request", settledTickCalls, 2)
check("queued follow-up is marked ready", queue.pending[10].ready, true)

-- The visible HUD can wake the actual controller entry point even when no
-- game-core or unit-completion callback arrives. Normal ticks suppress it.
local pulseCalls = 0
local pulseCfg = { CivvisDecides = true }
local pulseUpvalues = {}
for i = 1, 30 do
    local name = debug.getupvalue(queue.onUiPulse, i)
    if name == nil then break end
    pulseUpvalues[name] = i
    if name == "tick" then
        debug.setupvalue(queue.onUiPulse, i, function()
            pulseCalls = pulseCalls + 1
            queue.controllerTicks = (queue.controllerTicks or 0) + 1
        end)
    elseif name == "cfg" then
        debug.setupvalue(queue.onUiPulse, i, pulseCfg)
    end
end
queue.controllerTicks = 5; queue.lastUiTick = nil
queue.onUiPulse("TopPanel")
check("first pulse observes normal controller activity", pulseCalls, 0)
check("first HUD clock is attributed", (lastEvent("controller_clock") or ""):find('"source":"TopPanel"', 1, true) ~= nil, true)
queue.controllerTicks = 6
queue.onUiPulse()
check("normal controller activity suppresses fallback", pulseCalls, 0)
queue.onUiPulse("TopPanel")
check("quiet interval wakes controller without game-core events", pulseCalls, 1)
check("wakeup records its clock", (lastEvent("controller_wake") or ""):find('"source":"TopPanel"', 1, true) ~= nil, true)
queue.onUiPulse()
check("continued quiet intervals remain recoverable", pulseCalls, 2)
pulseCfg.Play = false
queue.onUiPulse()
check("disabled play prevents fallback", pulseCalls, 2)
pulseCfg.Play = true; pulseCfg.CivvisDecides = false
queue.onUiPulse()
check("standalone harness has no CivVis heartbeat", pulseCalls, 2)
pulseCfg.CivvisDecides = true
debug.setupvalue(queue.onUiPulse, pulseUpvalues.inTick, true)
queue.onUiPulse("DiplomacyActionView")
check("reentrant pulse cannot wake controller", pulseCalls, 2)
check("reentrant rejection is observable", (lastEvent("controller_clock") or ""):find('"disposition":"in_tick"', 1, true) ~= nil, true)
local rejectedLogCount = #LOG
for i = 1, 120 do queue.onUiPulse("DiplomacyActionView") end
check("rejected clock cannot keep the watchdog alive with repeated logs", #LOG, rejectedLogCount)
debug.setupvalue(queue.onUiPulse, pulseUpvalues.inTick, false)
debug.setupvalue(queue.onUiPulse, pulseUpvalues.finished, true)
queue.onUiPulse()
check("finished game cannot be woken", pulseCalls, 2)
debug.setupvalue(queue.onUiPulse, pulseUpvalues.finished, false)
Game.GetLocalPlayer = function() return -1 end
queue.onUiPulse()
check("no local seat cannot be woken", pulseCalls, 2)

-- 11. `StalledOperationRelease` (off everywhere above). With the switch on,
-- a leg the probe marks as a stalled operation takes the early no-op answer at
-- the probe tick instead of waiting out the grace. A walk not in an operation
-- is untouched. The agent binds `cfg` at load, so it is reloaded here.
CivvisControlConfig = { StalledOperationRelease = true }
local releaseChunk = assert(loadfile(here .. "/CivvisControlAgent.lua"))
local releaseRan, releaseErr = pcall(releaseChunk)
assert(releaseRan, "CivvisControlAgent.lua raised on reload: " .. tostring(releaseErr))
local applyOn, queueOn = rawget(_G, "CivvisApplyOrders"), rawget(_G, "CivvisQueue")
rawget(_G, "CivvisResolveActions")()
host.units, host.ops, host.commands, host.refuse, host.contacts = {}, {}, {}, {}, {}
host.governor_requests, host.cities = {}, {}
Game.GetLocalPlayer = function() return PID end
queueOn.reset(7)
host.paths = { [160] = 2, [161] = 2 }
Map.GetPlotIndex = function(x, y) return y * 100 + x end
UnitManager.GetMoveToPathEx = function(unit)
	local n = host.paths[unit.GetID()];
	if n == nil then return nil end
	local plots = {};
	for i = 1, n do plots[i] = i end
	return { plots = plots, turns = { 0, 1 } };
end
host.units[160] = { id = 160, kind = "UNIT_SKIRMISHER", x = 1, y = 1, moves = 2, active_operation = true }
host.units[161] = { id = 161, kind = "UNIT_WARRIOR", x = 4, y = 4, moves = 2 }
applyOn(player, PID, 7, { row(160, "MOVE_TO", 2, 1), row(161, "MOVE_TO", 5, 4) })
for _ = 1, 7 do queueOn.drain(player, PID, 7) end
check("release: nothing is answered before the probe tick", queueOn.pendingCount(), 2)
queueOn.drain(player, PID, 7)
check("release: the stalled operation is answered at the probe tick", queueOn.pendingCount(), 1)
local released = lastEvent("move_noop") or ""
check("release: the answer is the no-op for that unit, at tick 8",
	released:find('"unit":160', 1, true) ~= nil and released:find('"ticks":8', 1, true) ~= nil, true)
check("release: it carries its probe mark", released:find('"stall_probe":8', 1, true) ~= nil, true)
check("release: the mark says it was released",
	(lastEvent("stall_probe") or ""):find('"released":true', 1, true) ~= nil, true)
for _ = 1, 20 do queueOn.drain(player, PID, 7) end
check("release: a walk not in an operation still waits for the grace", queueOn.pendingCount(), 1)
UnitManager.GetMoveToPathEx = nil
Map.GetPlotIndex = nil
host.paths = nil

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("\nall order-queue checks passed")
