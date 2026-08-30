-- Offline regression for Great Person movement targets.
--
-- `GetActivationHighlightPlots` is the host's activation-eligibility list. It
-- is not a movement path, and Civ6 can accept MOVE_TO for a highlighted plot
-- that is behind a closed border without moving the unit. The driver must
-- skip that plot when the host pathfinder explicitly says it is unreachable.
--
-- Run: lua5.1 tools/civ6_control/mod/great_person_path_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end

local EXPORTS = {
	CivvisApplyOrders = true, CivvisResolveActions = true,
}
local LOG = {}
Automation = { Log = function(line) LOG[#LOG + 1] = line end }
UnitOperationTypes = { PARAM_X = "x", PARAM_Y = "y" }
UnitCommandTypes = {}

local function plotIndex(x, y)
	return y * 100 + x
end

local function emptyIterator()
	return function() return nil end
end

local function plot(x, y)
	return {
		GetX = function() return x end,
		GetY = function() return y end,
	}
end

local plots = {
	[plotIndex(2, 1)] = plot(2, 1), -- nearest highlight, but unreachable
	[plotIndex(4, 1)] = plot(4, 1), -- farther highlight, reachable
}
Map = {
	GetPlotDistance = function(x1, y1, x2, y2)
		return math.max(math.abs(x1 - x2), math.abs(y1 - y2))
	end,
	GetPlot = function() return nil end,
	GetPlotIndex = plotIndex,
	GetPlotByIndex = function(index) return plots[index] end,
}

local GP_INDIVIDUAL = 1001
local GP_CLASS = 2001
GameInfo = setmetatable({
	GreatPersonIndividuals = {
		[GP_INDIVIDUAL] = {
			GreatPersonIndividualType = "GREAT_PERSON_INDIVIDUAL_CHARLES_DARWIN",
		},
	},
	GreatPersonClasses = {
		[GP_CLASS] = {
			GreatPersonClassType = "GREAT_PERSON_CLASS_SCIENTIST",
		},
	},
	GreatWorks = emptyIterator,
	GreatWork_ValidSubTypes = emptyIterator,
	DistrictReplaces = emptyIterator,
	Buildings = emptyIterator,
	Units = {
		UNIT_GREAT_SCIENTIST = {
			UnitType = "UNIT_GREAT_SCIENTIST", Combat = 0, RangedCombat = 0,
		},
	},
}, { __index = function(_, key)
	if key == "UnitOperations" or key == "UnitCommands" then
		return setmetatable({}, {
			__index = function(_, name) return { Hash = name } end,
		})
	end
	return stub()
end })

rawset(_G, "CivvisControlConfig", {
	GreatPeopleUse = true,
	ExploreUnassigned = false,
	OrderQueue = false,
})
setmetatable(_G, { __index = function(_, key)
	if EXPORTS[key] then return rawget(_G, key) end
	return stub()
end })

local host = { units = {}, ops = {}, paths = {} }
local PID = 0
local function greatPerson()
	return {
		IsGreatPerson = function() return true end,
		GetIndividual = function() return GP_INDIVIDUAL end,
		GetClass = function() return GP_CLASS end,
		GetActionCharges = function() return 1 end,
		GetActivationHighlightPlots = function()
			return { plotIndex(2, 1), plotIndex(4, 1) }
		end,
	}
end
local function unitObject(u)
	return {
		GetID = function() return u.id end,
		GetX = function() return u.x end,
		GetY = function() return u.y end,
		GetMovesRemaining = function() return 2 end,
		GetUnitType = function() return u.kind end,
		GetType = function() return u.kind end,
		GetDamage = function() return 0 end,
		GetGreatPerson = function() return u.gp end,
		GetFortifyTurns = function() return 0 end,
		GetFormationUnitCount = function() return 1 end,
		GetBuildCharges = function() return 0 end,
		GetSpreadCharges = function() return 0 end,
		GetReligionType = function() return -1 end,
	}
end

UnitManager = {
	GetUnit = function(_, id)
		local u = host.units[id]
		return u ~= nil and unitObject(u) or nil
	end,
	CanStartOperation = function() return true end,
	RequestOperation = function(unit, operation, params)
		local u = host.units[unit:GetID()]
		host.ops[#host.ops + 1] = {
			id = u.id, operation = operation,
			x = params and params.x, y = params and params.y,
		}
	end,
	CanStartCommand = function() return false end,
	RequestCommand = function() end,
}
local pathFinder = function(unit, destination)
	return host.paths[unit:GetID() .. ":" .. destination]
end
UnitManager.GetMoveToPathEx = pathFinder

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
		local units = {}
		for _, u in pairs(host.units) do units[#units + 1] = unitObject(u) end
		table.sort(units, function(a, b) return a:GetID() < b:GetID() end)
		return { Members = members(units) }
	end,
	GetCities = function() return { Members = members({}) } end,
	GetDiplomacy = function() return { IsAtWarWith = function() return false end } end,
	GetScore = function() return 0 end,
	GetTreasury = function()
		return { GetGoldBalance = function() return 0 end }
	end,
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
Game = {
	GetLocalPlayer = function() return PID end,
	GetCurrentGameTurn = function() return 7 end,
}

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtime_err = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtime_err))
local applyOrders = rawget(_G, "CivvisApplyOrders")
local resolveActions = rawget(_G, "CivvisResolveActions")
assert(type(applyOrders) == "function", "CivvisApplyOrders is not exported")
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
local function lastEvent(kind)
	for i = #LOG, 1, -1 do
		if LOG[i]:find('"kind":"' .. kind .. '"', 1, true) then
			return LOG[i]
		end
	end
	return nil
end
local function reset(id)
	host.units, host.ops, host.paths = {}, {}, {}
	host.units[id] = {
		id = id, kind = "UNIT_GREAT_SCIENTIST", x = 1, y = 1,
		gp = greatPerson(),
	}
	end

-- The nearest highlighted plot has no host route. The driver must choose the
-- farther plot whose path terminates at the requested destination.
reset(10)
local origin = plotIndex(1, 1)
local reachable = plotIndex(4, 1)
host.paths["10:" .. reachable] = { plots = { origin, reachable }, turns = { 0, 1 } }
applyOrders(player, PID, 7, {})
check("unreachable nearest highlight is skipped", host.ops[1] ~= nil and host.ops[1].x, 4)
check("reachable activation plot is selected", host.ops[1] ~= nil and host.ops[1].y, 1)
check("move telemetry names the reachable plot",
	(lastEvent("gp") or ""):find('"action":"moving"', 1, true) ~= nil
		and (lastEvent("gp") or ""):find('"x":4', 1, true) ~= nil, true)

-- A path that exists but terminates short of the requested highlight is not a
-- route to that activation plot either, so it must not be handed to the host.
reset(11)
host.paths["11:" .. plotIndex(2, 1)] = { plots = { origin }, turns = { 0 } }
host.paths["11:" .. reachable] = { plots = { origin }, turns = { 0 } }
applyOrders(player, PID, 7, {})
check("partial path is rejected", #host.ops, 0)
check("unreachable highlights become idle",
	(lastEvent("gp") or ""):find('"action":"idle"', 1, true) ~= nil, true)

-- If an older host does not expose the pathfinder, retain the prior
-- highlight-based behavior rather than treating an unknown API as a hard no.
reset(12)
UnitManager.GetMoveToPathEx = nil
applyOrders(player, PID, 7, {})
check("missing path API keeps compatibility", host.ops[1] ~= nil and host.ops[1].x, 2)
UnitManager.GetMoveToPathEx = pathFinder

if failures > 0 then
	print(string.format("%d failure(s)", failures))
	os.exit(1)
end
print("all checks passed")
