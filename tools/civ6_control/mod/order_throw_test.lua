-- An order that raises inside the mod names what raised and which order it was.
--
-- Run civvis-20261115T071444Z counted `refused_by.unit.throw` on 12 opening
-- frames by turn 137 and held no `agent_error` event: the two `pcall`s that
-- hand a row to `applyOrder` dropped the message. This drives the shipped
-- `applyOrders` and the shipped queue against a fake host whose unit raises
-- inside the handler, and reads the events the agent writes.
--
-- Run with Lua 5.1. Host fixture follows deferred_build_hint_test.lua.
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
	CivvisApplyOrder = true, CivvisVerify = true, CivvisK = true,
}
CivvisControlConfig = { Play = true, CivvisDecides = false }
local LOG = {}
Automation = { Log = function(line) LOG[#LOG + 1] = line end }
UnitOperationTypes = { PARAM_X = "x", PARAM_Y = "y" }
UnitCommandTypes = {}
Map = {
	GetPlotDistance = function(x1, y1, x2, y2)
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
			return { UnitType = name, Combat = 20, RangedCombat = 0 }
		end })
	end
	return stub()
end })
GameInfo.DistrictReplaces = function() return function() return nil end end
setmetatable(_G, { __index = function(_, k)
	if EXPORTS[k] then return rawget(_G, k) end
	return stub()
end })

-- ---------------------------------------------------------------- fake host
local host = { units = {}, ops = {} }
local PID = 0
local function unitObject(u)
	return {
		GetID = function() return u.id end,
		-- The handler for CONTRIBUTE_PROJECT reads the position bare, the way a
		-- defect in any handler would: nothing between it and the order's pcall.
		GetX = function()
			if u.raises ~= nil then error(u.raises) end
			return u.x
		end,
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
	GetUnit = function(_, id)
		local u = host.units[id]
		if u == nil then return nil end
		return unitObject(u)
	end,
	CanStartOperation = function() return true end,
	RequestOperation = function(...)
		local unit, hash, params = ...
		local u = host.units[unit.GetID()]
		host.ops[#host.ops + 1] = { id = u.id, op = hash }
		if hash == "UNITOPERATION_MOVE_TO" then u.x, u.y, u.moves = params.x, params.y, 1 end
	end,
	CanStartCommand = function() return false end,
	RequestCommand = function() end,
}
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
		for _, u in pairs(host.units) do objs[#objs + 1] = unitObject(u) end
		table.sort(objs, function(a, b) return a.GetID() < b.GetID() end)
		return { Members = members(objs) }
	end,
	GetCities = function() return { Members = members({}) } end,
	GetDiplomacy = function() return { IsAtWarWith = function() return false end } end,
	IsTurnActive = function() return true end,
}, { __index = function() return stub() end })
Players = setmetatable({}, { __index = function(_, pid)
	if pid == PID then return player end
	return setmetatable({
		IsBarbarian = function() return true end,
		GetUnits = function() return { Members = members({}) } end,
		GetCities = function() return { Members = members({}) } end,
	}, { __index = function() return stub() end })
end })
PlayerManager = { GetAliveIDs = function() return { PID, 63 } end,
                  GetAliveMajorIDs = function() return { PID } end }
PlayersVisibility = setmetatable({}, { __index = function()
	return { IsVisible = function() return true end, IsRevealed = function() return true end }
end })
Game = {
	GetLocalPlayer = function() return PID end,
	GetCurrentGameTurn = function() return 124 end,
}

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, err)
assert(pcall(chunk))
CivvisControlConfig.CivvisDecides = true
rawget(_G, "CivvisResolveActions")()
local apply = assert(rawget(_G, "CivvisApplyOrders"), "applyOrders is exported")
local K = assert(rawget(_G, "CivvisK"), "CivvisK is exported")

-- Events as the agent wrote them: one line each, the payload JSON after the prefix.
local function events(kind)
	local found = {}
	for _, line in ipairs(LOG) do
		if line:find('"kind":"' .. kind .. '"', 1, true) then found[#found + 1] = line end
	end
	return found
end
local function field(line, name)
	return line:match('"' .. name .. '":"([^"]*)"') or line:match('"' .. name .. '":(-?[%d%.]+)')
end

-- 1. A healthy order writes neither event.
host.units[7] = { id = 7, kind = "UNIT_WARRIOR", x = 5, y = 5, moves = 2 }
apply(player, PID, 124, { { kind = "unit", subject = 7, verb = "MOVE_TO", x = 6, y = 5 } })
assert(#events("order_threw") == 0, "an order that ran reports no throw")
assert(#host.ops == 1 and host.ops[1].op == "UNITOPERATION_MOVE_TO", "the healthy move reached the host")

-- 2. A handler that raises: the order is refused as `throw`, as before, and
--    now the message and the order are both on the log.
host.units[9] = { id = 9, kind = "UNIT_SCOUT", x = 39, y = 27, moves = 5,
	raises = "CivvisControlAgent.lua:17777: attempt to index a boolean value" }
LOG = {}
apply(player, PID, 124, {
	{ kind = "unit", subject = 9, verb = "CONTRIBUTE_PROJECT", x = 39, y = 30 },
	{ kind = "unit", subject = 7, verb = "MOVE_TO", x = 7, y = 5 },
})
local threw = events("order_threw")
assert(#threw == 1, "one event for the one order that raised, got " .. #threw)
assert(field(threw[1], "order_kind") == "unit", "the event names the order kind")
assert(field(threw[1], "verb") == "CONTRIBUTE_PROJECT", "the event names the verb")
assert(tonumber(field(threw[1], "subject")) == 9, "the event names the unit")
assert(tonumber(field(threw[1], "x")) == 39 and tonumber(field(threw[1], "y")) == 30,
	"the event carries the order's plot")
assert(tonumber(field(threw[1], "turn")) == 124, "the event carries the turn")
assert(field(threw[1], "where") == "order", "the event says which caller caught it")
assert(threw[1]:find("attempt to index a boolean value", 1, true),
	"the event carries the raised message")
local orders = events("orders")
assert(#orders == 1 and orders[1]:find('"throw":1', 1, true),
	"the refusal is still counted under `throw`")
assert(host.ops[#host.ops].id == 7 and #host.ops == 2,
	"the order after the one that raised still reached the host")
-- The message is also reported the way every other guard reports one.
local reported = false
for _, line in ipairs(events("agent_error")) do
	if line:find("attempt to index a boolean value", 1, true) then reported = true end
end
assert(reported, "the message reaches agent_error, where civ6_agent_errors.py adds it up")

-- 3. The same defect every turn stays bounded: the event stops at the limit
--    and the order is still refused.
LOG = {}
K.orderThrows = K.ORDER_THROW_LIMIT
apply(player, PID, 125, { { kind = "unit", subject = 9, verb = "CONTRIBUTE_PROJECT", x = 39, y = 30 } })
assert(#events("order_threw") == 0, "past the limit the event is not written")
assert(events("orders")[1]:find('"throw":1', 1, true), "and the refusal is still counted")

-- 4. The reporter itself cannot raise into the order loop, whatever it is handed.
K.orderThrows = 0
LOG = {}
K.orderThrew(nil, {}, 126, "order")
K.orderThrew({ "not a string" }, { kind = "unit", verb = "MOVE_TO" }, 126, "queued_order")
assert(#events("order_threw") == 2, "a missing message or field still reports")

-- 5. Both callers of `applyOrder` report: the opening pass and the queue.
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local callers = 0
for _ in source:gmatch('else CivvisK%.orderThrew%(res1, row, turn, "[%w_]+"%); end') do
	callers = callers + 1
end
assert(callers == 2, "the opening pass and the queue both name a thrown order, found " .. callers)

print("order throw: passed")
