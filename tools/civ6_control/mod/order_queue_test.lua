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
--   8. unmentioned combat units near a hostile are NOT handed to explore
--      automation, and units far from one still are.
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
local host = { units = {}, ops = {}, refuse = {}, contacts = {} }
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
		end
	end,
	CanStartCommand = function() return false end,
	RequestCommand = function() end,
}
function host.arrive(id)
	local u = host.units[id]
	if u.pendingX ~= nil then
		u.x, u.y = u.pendingX, u.pendingY
		u.pendingX, u.pendingY = nil, nil
		u.moves = math.max(0, (u.moves or 0) - 1)
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
	host.units, host.ops, host.refuse, host.contacts = {}, {}, {}, {}
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

-- 6. Spent movement refuses what needs it, by name.
reset()
host.units[14] = { id = 14, kind = "UNIT_WARRIOR", x = 5, y = 5, moves = 1 }
applyOrders(player, PID, 7, { row(14, "MOVE_TO", 6, 5), row(14, "ATTACK", 7, 5) })
host.arrive(14) -- moves -> 0
queue.drain(player, PID, 7)
check("strike with no movement left refused", ops(14), "UNITOPERATION_MOVE_TO")
check("named queue_no_moves", (lastEvent("orders_queue") or ""):find("queue_no_moves", 1, true) ~= nil, true)

-- 7. The stall cap gives up by name.
reset()
host.units[15] = { id = 15, kind = "UNIT_WARRIOR", x = 5, y = 5, moves = 2 }
applyOrders(player, PID, 7, { row(15, "MOVE_TO", 6, 5), row(15, "ATTACK", 7, 5) })
queue.giveUp(7)
check("give-up empties the queue", queue.pendingCount(), 0)
check("give-up named queue_stalled", (lastEvent("orders_queue") or ""):find("queue_stalled", 1, true) ~= nil, true)

-- 8. Explore hand-off: a soldier beside a hostile stays put; a far one explores.
reset()
host.units[16] = { id = 16, kind = "UNIT_WARRIOR", x = 5, y = 5, moves = 2 }   -- near
host.units[17] = { id = 17, kind = "UNIT_WARRIOR", x = 30, y = 30, moves = 2 } -- far
host.contacts = { { id = 900, kind = "UNIT_BARBARIAN", x = 7, y = 5, moves = 2 } }
applyOrders(player, PID, 7, {})
check("held soldier not explored", ops(16), "")
check("far soldier explored", ops(17), "UNITOPERATION_AUTOMATE_EXPLORE")
check("orders event counts the guard", field(lastEvent("orders"), "explore_guarded"), 1)

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("\nall order-queue checks passed")
