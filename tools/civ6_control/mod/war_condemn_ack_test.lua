-- Real shipped applyOrders/queue/ledger with an asynchronous native war host.
-- A successful RequestSession is deliberately NOT IsAtWarWith acknowledgement.
-- Run: lua5.1 tools/civ6_control/mod/war_condemn_ack_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."
local function stub()
	return setmetatable({}, { __index = function() return stub() end,
		__call = function() return stub() end, __newindex = function() end })
end
setmetatable(_G, { __index = function() return stub() end })
local logs = {}
Automation = { Log = function(line) logs[#logs + 1] = line end }
CivvisControlConfig = { Play = false, GreatPeopleUse = false,
	OrderQueueGraceTicks = 3, OrderQueueMaxTicks = 8 }
local host = {}
local turn = 77
local PID, ACTOR, TARGET = 0, 1310733, 1835023
local CONDEMN = "UNITCOMMAND_CONDEMN_HERETIC"
local function key(pid, id) return tostring(pid) .. ":" .. tostring(id) end
local function unitObject(u)
	return {
		GetID = function() return u.id end,
		GetX = function() return u.x end, GetY = function() return u.y end,
		GetUnitType = function() return u.kind end, GetType = function() return u.kind end,
		GetMovesRemaining = function() return u.moves end,
		GetDamage = function() return 0 end, GetGreatPerson = function() return nil end,
		GetFortifyTurns = function() return 0 end,
		GetFormationUnitCount = function() return 1 end,
		GetAttacksRemaining = function() return 1 end,
	}
end
local function members(pid)
	local list = {}
	for _, u in pairs(host.units) do
		if u.owner == pid then list[#list + 1] = unitObject(u) end
	end
	table.sort(list, function(a, b) return a:GetID() < b:GetID() end)
	local i = 0
	return function()
		i = i + 1
		if list[i] then return i, list[i] end
	end
end
local diplomacy = {
	IsAtWarWith = function(_, other)
		if host.unknown then return nil end
		return host.wars[other] == true
	end,
	IsDiplomaticActionValid = function(_, action)
		return host.allowed and action == "DIPLOACTION_DECLARE_PROTECTORATE_WAR"
	end,
}
local player = setmetatable({
	GetDiplomacy = function() return diplomacy end,
	GetUnits = function() return { Members = function() return members(PID) end } end,
	GetCities = function() return { Members = function() return function() end end } end,
	GetTreasury = function() return { GetGoldBalance = function() return 0 end } end,
	GetScore = function() return 0 end, IsTurnActive = function() return true end,
}, { __index = function() return stub() end })
Players = { [PID] = player }
for _, owner in ipairs({ 1, 3 }) do
	local id = owner
	Players[id] = setmetatable({
		IsMajor = function() return true end, IsAlive = function() return true end,
		GetScore = function() return 0 end,
		GetUnits = function()
			host.enemy_reads = host.enemy_reads + 1
			return { Members = function() return members(id) end }
		end,
	}, { __index = function() return stub() end })
end
PlayerManager = { GetAliveMajorIDs = function() return { 0, 1, 3 } end,
	GetAliveIDs = function() return { 0, 1, 3 } end }
PlayersVisibility = { [PID] = { IsVisible = function() return host.visible end } }
Game = { GetLocalPlayer = function() return PID end,
	GetCurrentGameTurn = function() return turn end }
Map = { GetPlot = function() return nil end,
	GetPlotDistance = function(x1, y1, x2, y2)
		return math.max(math.abs(x1 - x2), math.abs(y1 - y2))
	end }
UnitOperationTypes = { PARAM_X = "x", PARAM_Y = "y" }
UnitCommandTypes = {}
ActivityTypes = { ACTIVITY_OPERATION = "operation" }
GameInfo = setmetatable({}, { __index = function(_, name)
	if name == "UnitOperations" or name == "UnitCommands" then
		return setmetatable({}, { __index = function(_, action) return { Hash = action } end })
	end
	if name == "Units" then
		return setmetatable({}, { __index = function(_, kind)
			return { UnitType = kind, Combat = kind == "UNIT_WARRIOR" and 20 or 0,
				ReligiousStrength = kind == "UNIT_MISSIONARY" and 100 or 0 }
		end })
	end
	return stub()
end })
UnitManager = {
	GetUnit = function(pid, id)
		local u = host.units[key(pid, id)]
		return u and unitObject(u) or nil
	end,
	GetActivityType = function() return nil end,
	CanStartOperation = function() return true end,
	RequestOperation = function(unit, hash, params)
		host.ops[#host.ops + 1] = { unit = unit:GetID(), verb = hash }
		if hash == "UNITOPERATION_MOVE_TO" then
			local u = host.units[key(PID, unit:GetID())]
			u.pendingX, u.pendingY = params.x, params.y
		end
	end,
	CanStartCommand = function(unit, hash)
		if hash ~= CONDEMN then return false end
		host.condemn_gates = host.condemn_gates + 1
		if host.force_refusal or unit:GetMovesRemaining() <= 0 then return false end
		for _, target in pairs(host.units) do
			if target.owner ~= PID and host.wars[target.owner] and target.kind == "UNIT_MISSIONARY"
					and target.x == unit:GetX() and target.y == unit:GetY() then return true end
		end
		return false
	end,
	RequestCommand = function(unit, hash, ...)
		assert(hash == CONDEMN and select("#", ...) == 0, "native condemnation is parameterless")
		host.commands[#host.commands + 1] = { unit = unit:GetID(), verb = hash }
		for k, target in pairs(host.units) do
			if target.owner ~= PID and host.wars[target.owner] and target.kind == "UNIT_MISSIONARY"
					and target.x == unit:GetX() and target.y == unit:GetY() then
				host.units[k] = nil
				CivvisLedger.onUnitRemoved(target.owner, target.id)
			end
		end
	end,
}
DiplomacyManager = { RequestSession = function(pid, target, statement)
	assert(pid == PID and target == 3 and statement == "DECLARE_PROTECTORATE_WAR")
	if host.request_throws then error("request failed") end
	host.requests[#host.requests + 1] = target
	if host.synchronous then host.wars[target] = true end
end }
PlayerOperations = { PARAM_PLAYER_ONE = "one", PARAM_PLAYER_TWO = "two" }
assert(loadfile(arg[1] or (here .. "/CivvisControlAgent.lua")))()
local queue = rawget(_G, "CivvisQueue")
local apply = rawget(_G, "CivvisApplyOrders")
local applyOne = rawget(_G, "CivvisApplyOrder")
local settle = rawget(_G, "CivvisSettleTurn")
assert(type(apply) == "function" and type(queue) == "table")
CivvisResolveActions()
local function put(owner, id, kind, x, y, moves)
	host.units[key(owner, id)] = { owner = owner, id = id, kind = kind, x = x, y = y, moves = moves or 3 }
end
local function reset()
	host.units, host.wars, host.requests, host.ops, host.commands = {}, {}, {}, {}, {}
	host.allowed, host.visible = true, true
	host.request_throws, host.synchronous, host.force_refusal, host.unknown = false, false, false, false
	host.condemn_gates, host.enemy_reads = 0, 0
	turn = 77
	CivvisControlConfig.OrderQueue = true
	queue.reset(turn)
	CivvisLedger.expected_condemn = {}
	logs = {}
	put(PID, ACTOR, "UNIT_WARRIOR", 4, 19)
	put(3, TARGET, "UNIT_MISSIONARY", 4, 19)
end
local function row(id, verb, x, y) return { kind = "unit", subject = id, verb = verb, x = x, y = y } end
local war = { kind = "war", subject = 3, verb = "DECLARE_PROTECTORATE_WAR" }
local condemn = row(ACTOR, "CONDEMN_HERETIC")
local checks = 0
local function check(name, got, want)
	assert(got == want, name .. ": got " .. tostring(got) .. ", want " .. tostring(want))
	checks = checks + 1
end
local function eventCount(kind)
	local n = 0
	for _, line in ipairs(logs) do
		if line:find('"kind":"' .. kind .. '"', 1, true) then n = n + 1 end
	end
	return n
end
local function drain() return queue.drain(player, PID, turn) end
local function decisionsDone()
	-- Test the actual settleTurn branch without inventing its queue logic or
	-- starting a board export. Only the existing private awaiting fixture changes.
	for i = 1, 100 do
		local name, value = debug.getupvalue(settle, i)
		if name == nil then break end
		if name == "awaiting" then
			value.turn, value.done = turn, true
			CivvisFrames.settled = true
			return
		end
	end
	error("real settleTurn awaiting fixture unavailable")
end

-- Direct condemn: unrelated units proceed; spurious unit events cannot ack war.
reset()
put(PID, 42, "UNIT_WARRIOR", 8, 8)
apply(player, PID, turn, { war, condemn, row(42, "FORTIFY") })
check("one declaration request", #host.requests, 1)
check("unrelated operation runs", #host.ops, 1)
check("no premature condemnation gate", host.condemn_gates, 0)
check("direct condemnation queued", queue.pendingCount(), 1)
drain()
queue.noteUnitEvent(PID, PID, ACTOR)
drain()
check("unit completion cannot acknowledge war", #host.commands, 0)
check("wait event once", eventCount("queue_war_wait"), 1)
host.wars[3] = true
drain()
check("acknowledged condemnation runs once", #host.commands, 1)
check("native target removal witnessed", eventCount("condemn_removed"), 1)
check("queue drains", queue.pendingCount(), 0)
drain()
check("no duplicate command", #host.commands, 1)

-- Move -> condemn waits for BOTH its real destination and the war readback.
reset()
host.units[key(PID, ACTOR)].x = 3
apply(player, PID, turn, { war, row(ACTOR, "MOVE_TO", 4, 19), condemn })
check("opening walk runs without war wait", #host.ops, 1)
drain()
check("no command before arrival", host.condemn_gates, 0)
host.units[key(PID, ACTOR)].x = 4
drain()
check("arrival does not acknowledge war", host.condemn_gates, 0)
host.wars[3] = true
drain()
check("move plus acknowledged condemnation", #host.commands, 1)

reset()
host.synchronous = true
apply(player, PID, turn, { war, condemn })
check("synchronous war adds no queue latency", queue.pendingCount(), 0)
check("synchronous command immediate", #host.commands, 1)
reset()
host.wars[3] = true
apply(player, PID, turn, { condemn })
check("existing war remains immediate", #host.commands, 1)
check("existing war creates no wait", eventCount("queue_war_wait"), 0)

for _, failure in ipairs({ "refused", "throw" }) do
	reset()
	if failure == "refused" then host.allowed = false else host.request_throws = true end
	apply(player, PID, turn, { war, condemn })
	check(failure .. " request creates no dependency", queue.pendingCount(), 0)
	check(failure .. " request cannot issue command", #host.commands, 0)
	check(failure .. " request not recorded", queue.warRequests[3], nil)
end

-- No acknowledgement: bounded named refusal of the whole dependent sequence.
reset()
apply(player, PID, turn, { war, condemn, row(ACTOR, "FORTIFY") })
for _ = 1, 3 do drain() end
check("timeout releases queue", queue.pendingCount(), 0)
check("timeout names all dependent rows", queue.stats.refusals.queue_war_not_acknowledged, 2)
check("timeout issues no dependent command", #host.commands, 0)
check("timeout issues no dependent fortify", #host.ops, 0)
turn = 78
queue.reset(turn)
check("request dependency cannot leak to next turn", queue.warRequests[3], nil)

for _, outcome in ipairs({ "gone", "moved", "no_moves", "native_refused" }) do
	reset()
	apply(player, PID, turn, { war, condemn })
	drain()
	if outcome == "gone" then host.units[key(PID, ACTOR)] = nil
	elseif outcome == "moved" then host.units[key(3, TARGET)].x = 5
	elseif outcome == "no_moves" then host.units[key(PID, ACTOR)].moves = 0; host.wars[3] = true
	else host.force_refusal = true; host.wars[3] = true end
	drain()
	check(outcome .. " releases queue", queue.pendingCount(), 0)
	check(outcome .. " issues no command", #host.commands, 0)
	check(outcome .. " is a refusal", queue.stats.refused, 1)
end

reset()
apply(player, PID, turn, { war, condemn })
host.unknown = true
for _ = 1, 3 do drain() end
check("unknown war readback cannot acknowledge", #host.commands, 0)
check("unknown readback bounded", queue.stats.refusals.queue_war_not_acknowledged, 1)

reset()
host.wars[1] = true
put(1, 91, "UNIT_MISSIONARY", 4, 19)
apply(player, PID, turn, { war, condemn })
check("available existing-war target stays immediate", #host.commands, 1)
check("peace target is not falsely removed", host.units[key(3, TARGET)] ~= nil, true)
check("only actual war removal witnessed", eventCount("condemn_removed"), 1)

reset()
applyOne(player, PID, war, turn)
host.visible = false
host.enemy_reads = 0
check("invisible plot cannot create dependency", queue.condemnWarPending(player, PID, condemn, turn), nil)
check("invisible plot never enumerates enemies", host.enemy_reads, 0)
host.visible = true
host.units[key(3, TARGET)].kind = "UNIT_WARRIOR"
check("military target cannot create dependency", queue.condemnWarPending(player, PID, condemn, turn), nil)
host.units[key(3, TARGET)].kind = "UNIT_MISSIONARY"
host.units[key(3, TARGET)].x = 5
check("remote target cannot create dependency", queue.condemnWarPending(player, PID, condemn, turn), nil)

reset()
CivvisControlConfig.OrderQueue = false
apply(player, PID, turn, { war, condemn })
check("disabled queue cannot issue premature command", host.condemn_gates, 0)
check("disabled queue remains empty", queue.pendingCount(), 0)
local ok, why = applyOne(player, PID, condemn, turn)
check("direct command respects native dependency", ok, false)
check("direct dependency refusal named", why, "condemn_war_pending")

reset()
apply(player, PID, turn, { war, condemn })
decisionsDone()
check("real settlement holds while war unacknowledged", settle(player, PID, turn), false)
host.wars[3] = true
check("real settlement releases after acknowledgement", settle(player, PID, turn), true)
check("real settlement executes promised command", #host.commands, 1)

reset()
apply(player, PID, turn, { war, condemn })
decisionsDone()
queue.ticks = 8
check("real global cap releases turn", settle(player, PID, turn), true)
check("global cap still releases queue", queue.pendingCount(), 0)
check("global cap remains named", queue.stats.refusals.queue_stalled, 1)
print("war/condemn acknowledgement: " .. checks .. " assertions passed against real agent/queue/ledger")
