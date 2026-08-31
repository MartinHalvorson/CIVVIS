-- A host explore operation is not a queued destination: Civilization VI reports
-- it as ACTIVITY_OPERATION.  Drive the shipped agent against a small fake host
-- so a stale auto-explorer cannot override CIVVIS's tactical order.
--
-- Run: lua5.1 tools/civ6_control/mod/auto_explore_cancellation_test.lua

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
	CivvisResolveActions = true,
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

-- 1. An active auto-explore operation has no queued destination, but must be
-- cancelled at turn start before it can spend another movement point.
reset()
host.units[11] = { id = 11, kind = "UNIT_SLINGER", x = 1, y = 1, moves = 2,
	activity = "operation" }
board.cancelQueuedPaths(player, PID, 7)
check("active operation is cancelled without a queued destination", calls(11),
	"command:UNITCOMMAND_CANCEL")
check("active operation count is reported", field(lastEvent("queued_paths"), "active_operations"), 1)

-- 2. A replan's explicit MOVE_TO preempts the lingering explorer before its
-- own request, so the host cannot accept the order then keep walking elsewhere.
reset()
host.units[12] = { id = 12, kind = "UNIT_SLINGER", x = 1, y = 1, moves = 2,
	activity = "operation" }
applyOrders(player, PID, 7, { row(12, "MOVE_TO", 2, 1) })
check("CIVVIS order preempts host explorer first", calls(12),
	"command:UNITCOMMAND_CANCEL,operation:UNITOPERATION_MOVE_TO")

-- 3. An unmentioned ranged unit is held, not handed to the host's generic
-- explorer; a Warrior still retains the established exploration fallback.
reset()
host.units[13] = { id = 13, kind = "UNIT_SLINGER", x = 1, y = 1, moves = 2,
	activity = "awake" }
applyOrders(player, PID, 7, {})
check("unmentioned ranged unit is held", calls(13), "operation:UNITOPERATION_FORTIFY")
check("ranged protection is counted", field(lastEvent("orders"), "explore_ranged_protected"), 1)

reset()
host.units[14] = { id = 14, kind = "UNIT_WARRIOR", x = 1, y = 1, moves = 2,
	activity = "awake" }
applyOrders(player, PID, 7, {})
check("unmentioned melee unit still explores", calls(14),
	"operation:UNITOPERATION_AUTOMATE_EXPLORE")

if failures > 0 then
	print(string.format("%d failure(s)", failures))
	os.exit(1)
end
print("all auto-explore cancellation tests passed")
