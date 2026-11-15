-- A Rock Band's concert raises a popup that locks the game core, and a deal
-- session's leader screen keeps that popup hidden: live 2026-11-15 the two
-- together cost a 30 s ask and a hung rival turn (071444Z t159, t175), and
-- closing the hidden popup was followed once in eleven by a native crash
-- (110627Z t165). So the agent keeps them apart: a `TOURISM_BOMB` order is
-- refused for the frame while a deal session is open, its answer holds the
-- turn, or the frame's own orders can open one. This drives the shipped agent
-- against the small fake host of auto_explore_cancellation_test.lua.
--
-- Run: lua5.1 tools/civ6_control/mod/concert_deal_session_order_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end

local EXPORTS = {
	CivvisApplyOrders = true, CivvisBoard = true, CivvisQueue = true,
	CivvisResolveActions = true, CivvisTrade = true,
}
local LOG = {}
Automation = { Log = function(line) LOG[#LOG + 1] = line end }
ActivityTypes = { ACTIVITY_OPERATION = "operation" }
UnitOperationTypes = { PARAM_X = "x", PARAM_Y = "y" }
UnitCommandTypes = {}
Map = {
	GetPlotDistance = function(x1, y1, x2, y2)
		return math.max(math.abs(x1 - x2), math.abs(y1 - y2))
	end,
	GetPlotIndex = function(x, y) return x * 100 + y end,
	GetPlot = function() return nil end,
}
GameInfo = setmetatable({}, { __index = function(_, key)
	if key == "UnitOperations" or key == "UnitCommands" then
		return setmetatable({}, { __index = function(_, name) return { Hash = name } end })
	end
	if key == "Units" then
		return setmetatable({}, { __index = function(_, name)
			if name == "UNIT_SLINGER" then
				return { UnitType = name, Combat = 5, RangedCombat = 15 }
			end
			if name == "UNIT_SETTLER" then
				return { UnitType = name, Combat = 0, RangedCombat = 0 }
			end
			return { UnitType = name, Combat = 20, RangedCombat = 0 }
		end })
	end
	return stub()
end })
GameConfiguration = { GetValue = function() return nil end }
setmetatable(_G, { __index = function(_, key)
	if EXPORTS[key] then return rawget(_G, key) end
	return stub()
end })

-- ---------------------------------------------------------------- fake host
local host = { units = {}, calls = {} }
local PID = 0
local function unitObject(unit)
	return setmetatable({
		GetID = function() return unit.id end,
		GetX = function() return unit.x end,
		GetY = function() return unit.y end,
		GetMovesRemaining = function() return unit.moves end,
		GetUnitType = function() return unit.kind end,
		GetType = function() return unit.kind end,
		GetDamage = function() return 0 end,
		GetGreatPerson = function() return nil end,
		GetFortifyTurns = function() return 0 end,
		GetFormationUnitCount = function() return 1 end,
		GetAttacksRemaining = function() return 1 end,
	}, { __index = function() return function() return nil end end })
end
UnitManager = {
	GetUnit = function(_, id)
		local unit = host.units[id]
		return unit ~= nil and not unit.gone and unitObject(unit) or nil
	end,
	GetQueuedDestination = function(unit)
		return host.units[unit:GetID()].queued
	end,
	GetActivityType = function(unit)
		return host.units[unit:GetID()].activity
	end,
	CanStartCommand = function(_, hash) return hash == "UNITCOMMAND_CANCEL" end,
	RequestCommand = function(unit, hash)
		local live = host.units[unit:GetID()]
		host.calls[#host.calls + 1] = { id = live.id, kind = "command", action = hash }
		if hash == "UNITCOMMAND_CANCEL" then
			live.activity, live.queued = "awake", nil
		end
	end,
	CanStartOperation = function() return true end,
	RequestOperation = function(unit, hash, params)
		local live = host.units[unit:GetID()]
		host.calls[#host.calls + 1] = {
			id = live.id, kind = "operation", action = hash,
			x = params and params.x or nil, y = params and params.y or nil,
		}
	end,
	GetMoveToPathEx = function() return nil end,
}
local function members(list)
	return function()
		local index = 0
		return function()
			index = index + 1
			if list[index] == nil then return nil end
			return index, list[index]
		end
	end
end
local player = setmetatable({
	GetUnits = function()
		local units = {}
		for _, unit in pairs(host.units) do
			if not unit.gone then units[#units + 1] = unitObject(unit) end
		end
		table.sort(units, function(a, b) return a:GetID() < b:GetID() end)
		return { Members = members(units) }
	end,
	GetCities = function() return { Members = members({}) } end,
	GetDiplomacy = function() return { IsAtWarWith = function() return false end } end,
	GetScore = function() return 0 end,
	GetTreasury = function() return { GetGoldBalance = function() return 0 end } end,
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
PlayerManager = {
	GetAliveIDs = function() return { PID, 63 } end,
	GetAliveMajorIDs = function() return { PID } end,
}
PlayersVisibility = setmetatable({}, { __index = function()
	return { IsVisible = function() return true end, IsRevealed = function() return true end }
end })
Game = { GetLocalPlayer = function() return PID end, GetCurrentGameTurn = function() return 7 end }

local chunk, loadErr = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(loadErr))
local ran, runtimeErr = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtimeErr))

local applyOrders = rawget(_G, "CivvisApplyOrders")
local board = rawget(_G, "CivvisBoard")
local queue = rawget(_G, "CivvisQueue")
local resolveActions = rawget(_G, "CivvisResolveActions")
assert(type(applyOrders) == "function", "CivvisApplyOrders is not exported")
assert(type(board) == "table", "CivvisBoard is not exported")
assert(type(queue) == "table", "CivvisQueue is not exported")
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
local function calls(id)
	local out = {}
	for _, call in ipairs(host.calls) do
		if id == nil or call.id == id then
			out[#out + 1] = call.kind .. ":" .. call.action
		end
	end
	return table.concat(out, ",")
end
local function lastEvent(kind)
	for index = #LOG, 1, -1 do
		if LOG[index]:find('"kind":"' .. kind .. '"', 1, true) then return LOG[index] end
	end
	return nil
end
local function field(line, name)
	if line == nil then return nil end
	local value = line:match('"' .. name .. '":(%-?%d+)')
	return value and tonumber(value) or line:match('"' .. name .. '":"([^"]*)"')
end
local function reset()
	host.units, host.calls, LOG = {}, {}, {}
	queue.reset(7)
	board.reset()
end
local function row(subject, verb, x, y)
	return { kind = "unit", subject = subject, verb = verb, x = x, y = y }
end


local trade = rawget(_G, "CivvisTrade")
assert(type(trade) == "table", "CivvisTrade is not exported")
local BAND = 21
local function band()
	host.units[BAND] = { id = BAND, kind = "UNIT_ROCK_BAND", x = 5, y = 5, moves = 2,
		activity = "awake" }
end
local function concert() return row(BAND, "TOURISM_BOMB") end
local function played() return calls(BAND):find("UNITOPERATION_TOURISM_BOMB", 1, true) ~= nil end
local function clear()
	reset()
	for key in pairs(trade.sessions) do trade.sessions[key] = nil end
	trade.turnHold = nil
	band()
end
local function refusedWhy()
	local line = lastEvent("orders")
	return line ~= nil and line:find("concert_waits_for_the_deal_session", 1, true) ~= nil
end

-- 1. With no session anywhere the concert is requested as it always was.
clear()
applyOrders(player, PID, 7, { concert() })
check("a concert with no deal session is played", played(), true)
check("…and is not reported as waiting", refusedWhy(), false)
check("the predicate agrees", trade.concertWaits({ concert() }), false)

-- 2. A session opened in an earlier frame and still waiting for its answer.
clear()
trade.sessions[4] = { kind = "buy", action = "EQUALIZE", turn = 7, sent = true }
applyOrders(player, PID, 7, { concert() })
check("a concert behind an open session is not requested", played(), false)
check("…and is refused with its own reason", refusedWhy(), true)
check("…counted as one refusal", field(lastEvent("orders"), "refused"), 1)

-- 3. The session has closed: the next frame's order plays the concert.
trade.sessions[4] = nil
host.calls = {}
applyOrders(player, PID, 7, { concert() })
check("the same order in the next frame is played", played(), true)

-- 4. An unanswered ask that still holds our turn for the rival's verdict.
clear()
trade.turnHold = { turn = 7, target = 4, at = 0 }
applyOrders(player, PID, 7, { concert() })
check("a concert behind a held turn is not requested", played(), false)
check("…with the same reason", refusedWhy(), true)

-- 5. A frame whose own orders can open a session, whichever comes first.
for _, kind in ipairs({ "peace", "friendship", "alliance", "sell", "buy" }) do
	clear()
	check("a frame with a " .. kind .. " order makes the concert wait",
		trade.concertWaits({ concert(), { kind = kind, subject = "4" } }), true)
	check("…also when the " .. kind .. " order comes first",
		trade.concertWaits({ { kind = kind, subject = "4" }, concert() }), true)
end
clear()
-- The ask itself is stubbed out here: this frame is about the order beside it.
local realAsk = trade.ask
trade.ask = function() return "session" end
applyOrders(player, PID, 7, { concert(), { kind = "peace", subject = "4" } })
trade.ask = realAsk
check("a concert in the same frame as a peace ask is not requested", played(), false)
check("…and says why", refusedWhy(), true)

-- 6. Orders that open no session leave the concert alone, and so do other
--    units' orders in a frame that holds one.
clear()
host.units[22] = { id = 22, kind = "UNIT_WARRIOR", x = 1, y = 1, moves = 2, activity = "awake" }
check("unit, research and production orders do not make it wait",
	trade.concertWaits({ concert(), row(22, "MOVE_TO", 2, 1),
		{ kind = "research", verb = "TECH_POTTERY" }, { kind = "produce", subject = "1" } }), false)
applyOrders(player, PID, 7, { concert(), row(22, "MOVE_TO", 2, 1) })
check("a concert beside a move is played", played(), true)
clear()
host.units[22] = { id = 22, kind = "UNIT_WARRIOR", x = 1, y = 1, moves = 2, activity = "awake" }
trade.sessions[4] = { kind = "sell", action = "EQUALIZE", turn = 7, sent = true }
applyOrders(player, PID, 7, { concert(), row(22, "MOVE_TO", 2, 1) })
check("behind a session only the concert waits", played(), false)
check("…the move beside it is still requested",
	calls(22):find("UNITOPERATION_MOVE_TO", 1, true) ~= nil, true)

-- 7. Malformed input never throws.
clear()
check("no rows", trade.concertWaits(nil), false)
check("rows that are not orders", trade.concertWaits({ 5, "x", {}, { kind = 7 } }), false)

if failures > 0 then
	print(string.format("%d failure(s)", failures))
	os.exit(1)
end
print("all concert / deal-session order checks passed")
