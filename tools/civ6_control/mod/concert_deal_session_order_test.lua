-- A Rock Band's concert raises a popup that locks the game core, and a deal
-- session's leader screen keeps that popup hidden: live 2026-11-15 the two
-- together cost a 30 s ask and a hung rival turn (071444Z t159, t175). So the
-- agent keeps them apart without losing the concert: a frame's deal asks go
-- out after its unit orders, and a concert that finds a deal of this turn
-- holding our end turn waits in the order queue and plays the same turn as
-- soon as that hold ends. #5336/#5341 refused it for the frame instead, while
-- ANY session sat in the table: in civvis-20261115T151156Z a session opened at
-- t184 was never closed, and a level-4 band on its stage never played again.
-- This drives the shipped agent against the small fake host of
-- auto_explore_cancellation_test.lua.
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
-- The agent's clock (`CivvisClock.now`) reads `UI.GetElapsedTime`.
NOW = 100
UI = setmetatable({ GetElapsedTime = function() return NOW end },
	{ __index = function() return stub() end })

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
local transitions = rawget(_G, "CivvisTransitions")
assert(type(transitions) == "table", "CivvisTransitions is not exported")
local BAND = 21
local function band(id)
	host.units[id or BAND] = { id = id or BAND, kind = "UNIT_ROCK_BAND", x = 5, y = 5, moves = 4,
		activity = "awake" }
end
local function concert(id) return row(id or BAND, "TOURISM_BOMB") end
local function played(id)
	return calls(id or BAND):find("UNITOPERATION_TOURISM_BOMB", 1, true) ~= nil
end
local function clear()
	reset()
	NOW = 100
	for key in pairs(trade.sessions) do trade.sessions[key] = nil end
	trade.turnHold = nil
	band()
end
local function refusedWhy()
	for _, line in ipairs(LOG) do
		if line:find("concert_waits_for_the_deal_session", 1, true) then return true end
	end
	return false
end
local function count(kind)
	local n = 0
	for _, line in ipairs(LOG) do
		if line:find('"kind":"' .. kind .. '"', 1, true) then n = n + 1 end
	end
	return n
end
-- A session of this turn as `CivvisTrade.ask` leaves it: opening, then asked.
local function session(target, asked, turn)
	trade.sessions[target] = { kind = "buy", action = "EQUALIZE", turn = turn or 7,
		sent = asked == true, opened_at = NOW, reply_at = asked and NOW or nil }
end
local function ticks(n, step)
	for _ = 1, n do
		NOW = NOW + (step or 0.1)
		queue.drain(player, PID, 7)
	end
end

-- The order in which the frame reached the host: unit orders through the
-- real request, a deal ask as `CivvisTrade.ask` would open it (the shipped ask
-- needs a whole rival behind it; what matters here is when it goes out).
local SEQ = {}
local realApply = transitions.apply
transitions.apply = function(p, pid, order, turn)
	local kind = tostring(order.kind or "")
	if trade.SESSION_ORDER_KINDS[kind] then
		SEQ[#SEQ + 1] = "ask:" .. kind
		session(tonumber(order.subject), false, turn)
		return true, "session"
	end
	local ok, why = realApply(p, pid, order, turn)
	SEQ[#SEQ + 1] = tostring(order.verb) .. ":" .. tostring(ok)
	return ok, why
end

-- 1. With no session anywhere the concert is requested as it always was.
clear()
applyOrders(player, PID, 7, { concert() })
check("a concert with no deal session is played", played(), true)
check("…and is not reported as waiting", refusedWhy(), false)
check("…nor deferred", count("concert_deferred"), 0)
check("the predicate agrees", trade.concertWaits(7), false)

-- 2. civvis-20261115T151156Z t184/t185: a band on its stage with full moves,
--    in a frame whose own trade ask opens a session. CIVVIS put the ask first;
--    the concert still goes out before it, and plays.
for _, kind in ipairs({ "peace", "friendship", "alliance", "sell", "buy" }) do
	clear()
	SEQ = {}
	applyOrders(player, PID, 7, { { kind = kind, subject = "4" }, concert() })
	check("t184 shape (" .. kind .. "): the concert is played this frame", played(), true)
	check("…before the ask goes out", table.concat(SEQ, ","), "TOURISM_BOMB:true,ask:" .. kind)
	check("…and the ask still goes out", trade.sessions[4] ~= nil, true)
	check("…nothing is refused", refusedWhy(), false)
end

-- 3. A later frame of the same turn finds that session still opening: the
--    concert waits in the queue, the turn is held for it, and it plays the
--    moment the session has closed. Never a lost turn.
clear()
session(4, false)
NOW = NOW + 0.4
applyOrders(player, PID, 7, { concert() })
check("behind an opening session the concert is not requested", played(), false)
check("…not refused", refusedWhy(), false)
check("…but deferred from the frame", count("concert_deferred"), 1)
check("…and the queue holds the turn for it", queue.pendingCount(), 1)
ticks(3)
check("while the session is open it keeps waiting", played(), false)
check("…still holding the turn", queue.pendingCount(), 1)
trade.sessions[4] = nil -- answered and closed, ~0.3-1 s after it opened
ticks(1)
check("the session closed: the concert plays the same turn", played(), true)
check("…released once", count("concert_released"), 1)
check("…the queue lets the turn go", queue.pendingCount(), 0)
check("…with no refusal anywhere", next(queue.stats.refusals) == nil and not refusedWhy(), true)

-- 4. The t184 session itself: opened, never acknowledged, never closed. Its
--    opening hold (3 s) ends and so does the concert's wait, although the
--    session is still in the table.
clear()
session(4, false)
applyOrders(player, PID, 7, { concert() })
check("an unacknowledged opening holds the concert at first", played(), false)
ticks(20, 0.1)
check("…for no more than its 3 s opening hold", played(), false)
ticks(15, 0.1)
check("…after which the concert plays this turn", played(), true)
check("…the session still sitting in the table", trade.sessions[4] ~= nil, true)

-- 5. A session left from an earlier turn holds nothing: this is what refused
--    the 151156Z band three times a turn from t185 to the end of the game.
clear()
session(4, false, 6)
applyOrders(player, PID, 7, { concert() })
check("a session from an earlier turn does not hold the concert", played(), true)
check("…nothing deferred", count("concert_deferred"), 0)

-- 6. An asked session waits for its answer; the answer comes, the band plays.
clear()
session(4, true)
applyOrders(player, PID, 7, { concert() })
ticks(5, 0.2)
check("behind an asked session the concert waits", played(), false)
trade.sessions[4] = nil
ticks(1)
check("…and plays once it is answered", played(), true)

-- 7. An unanswered ask whose verdict still holds our turn: the concert waits
--    while that hold lasts and plays when the rival speaks. One from an
--    earlier turn holds nothing.
clear()
trade.turnHold = { turn = 7, target = 4, at = NOW }
applyOrders(player, PID, 7, { concert() })
ticks(5)
check("behind a held turn the concert waits", played(), false)
trade.turnHold = nil -- the rival spoke
ticks(1)
check("…and plays when the rival speaks", played(), true)
clear()
trade.turnHold = { turn = 6, target = 4, at = NOW }
applyOrders(player, PID, 7, { concert() })
check("a held turn from an earlier turn does not hold it", played(), true)

-- 8. A band that walks first plays from the queue, some ticks after the
--    frame, when the frame's session is up (114552Z t154, t158): it waits
--    there too, and plays the same turn.
clear()
queue.push(BAND, concert(), nil, nil)
session(4, true)
ticks(40, 0.05)
check("a queued concert behind an open session is not requested", played(), false)
check("…not refused", (queue.stats.refusals or {})["concert_waits_for_the_deal_session"], nil)
check("…and still holds the turn", queue.pendingCount(), 1)
check("…deferred once", count("concert_deferred"), 1)
trade.sessions[4] = nil
ticks(1)
check("…and plays as soon as the session closes", played(), true)
check("…the queue drained", queue.pendingCount(), 0)

-- 9. Only the concert waits: other units' orders in the same frame go out.
clear()
host.units[22] = { id = 22, kind = "UNIT_WARRIOR", x = 1, y = 1, moves = 2, activity = "awake" }
session(4, true)
applyOrders(player, PID, 7, { concert(), row(22, "MOVE_TO", 2, 1) })
check("behind a session only the concert waits", played(), false)
check("…the move beside it is still requested",
	calls(22):find("UNITOPERATION_MOVE_TO", 1, true) ~= nil, true)

-- 10. Two bands, one frame, an ask between them: both play, then the ask.
clear()
band(23)
SEQ = {}
applyOrders(player, PID, 7, { concert(), { kind = "sell", subject = "4" }, concert(23) })
check("two bands around an ask both play first", table.concat(SEQ, ","),
	"TOURISM_BOMB:true,TOURISM_BOMB:true,ask:sell")

-- 11. Malformed input never throws.
clear()
check("no turn", trade.concertWaits(nil), false)
check("a turn that is not a number", trade.concertWaits("x"), false)

transitions.apply = realApply
if failures > 0 then
	print(string.format("%d failure(s)", failures))
	os.exit(1)
end
print("all concert / deal-session order checks passed")
