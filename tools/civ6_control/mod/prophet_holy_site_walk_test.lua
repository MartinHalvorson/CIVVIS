-- Offline regression for a Great Prophet still to found.
--
-- The `religion` order founds only where the host's CanStartOperation
-- (FOUND_RELIGION) holds, a Holy Site; the Prophet appears in the city
-- centre. Run civvis-20261030T062126Z left John the Baptist two tiles from
-- Athens' Holy Site from t66 to Spain's religious victory at t85, refused
-- `cannot_found_religion_here` every turn. The driver must walk it there.
--
-- Run: lua5.1 tools/civ6_control/mod/prophet_holy_site_walk_test.lua

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
			GreatPersonIndividualType = "GREAT_PERSON_INDIVIDUAL_JOHN_THE_BAPTIST",
		},
	},
	GreatPersonClasses = {
		[GP_CLASS] = {
			GreatPersonClassType = "GREAT_PERSON_CLASS_PROPHET",
		},
	},
	GreatWorks = emptyIterator,
	GreatWork_ValidSubTypes = emptyIterator,
	DistrictReplaces = emptyIterator,
	Districts = {
		[1] = { DistrictType = "DISTRICT_CITY_CENTER" },
		[2] = { DistrictType = "DISTRICT_HOLY_SITE" },
	},
	Buildings = emptyIterator,
	Units = {
		UNIT_GREAT_PROPHET = {
			UnitType = "UNIT_GREAT_PROPHET", Combat = 0, RangedCombat = 0,
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

local host = { units = {}, ops = {}, paths = {}, cities = {}, commands = {} }
local PID = 0
local function greatPerson()
	return {
		IsGreatPerson = function() return true end,
		GetIndividual = function() return host.individual or GP_INDIVIDUAL end,
		GetClass = function() return GP_CLASS end,
		GetActionCharges = function() return host.charges or 1 end,
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
	CanStartOperation = function(unit, operation)
		if operation == "UNITOPERATION_FOUND_RELIGION" then
			local u = host.units[unit:GetID()]
			return host.foundAt ~= nil and u.x == host.foundAt.x and u.y == host.foundAt.y
		end
		return true
	end,
	RequestOperation = function(unit, operation, params)
		local u = host.units[unit:GetID()]
		host.ops[#host.ops + 1] = {
			id = u.id, operation = operation,
			x = params and params.x, y = params and params.y,
		}
	end,
	CanStartCommand = function(_, command)
		return host.canActivate and command == "UNITCOMMAND_ACTIVATE_GREAT_PERSON"
	end,
	RequestCommand = function(_, command) host.commands[#host.commands + 1] = command end,
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
	GetID = function() return PID end,
	GetStats = function() return { GetNumProjectsAdvanced = function() return host.launched and 1 or 0 end } end,
	GetCities = function() return { Members = members(host.cities) } end,
	GetReligion = function()
		return { GetReligionTypeCreated = function() return host.religion or -1 end }
	end,
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
local function district(kind, x, y, complete)
	return {
		GetType = function() return kind end,
		GetX = function() return x end,
		GetY = function() return y end,
		IsComplete = function() return complete end,
		IsPillaged = function() return false end,
	}
end
local function city(districts)
	return { GetDistricts = function() return { Members = members(districts) } end }
end
local function reset(id)
	host.units, host.ops, host.paths, host.commands = {}, {}, {}, {}
	host.cities, host.charges, host.religion, host.foundAt = {}, nil, nil, { x = 3, y = 1 }
	host.units[id] = { id = id, kind = "UNIT_GREAT_PROPHET", x = 1, y = 1, gp = greatPerson() }
	host.cities[1] = city({ district(1, 1, 1, true), district(2, 3, 1, true) })
end

-- In the centre, with the Holy Site two tiles away and a route to it: walk.
reset(20)
local origin = plotIndex(1, 1)
local site = plotIndex(3, 1)
host.paths["20:" .. site] = { plots = { origin, plotIndex(2, 1), site }, turns = { 0, 1, 1 } }
applyOrders(player, PID, 7, {})
check("the unfounded Prophet walks to the Holy Site", host.ops[1] ~= nil and host.ops[1].x, 3)
check("the walk names the Holy Site's row", host.ops[1] ~= nil and host.ops[1].y, 1)
check("the walk is reported as a founding walk",
	(lastEvent("gp") or ""):find('"action":"moving_to_found"', 1, true) ~= nil, true)

-- The live host reads an unfounded Prophet's action charges as 0
-- (civvis-20261030T090715Z): the walk must not depend on them.
reset(25)
host.charges = 0
host.paths["25:" .. site] = { plots = { origin, plotIndex(2, 1), site }, turns = { 0, 1, 1 } }
applyOrders(player, PID, 7, {})
check("a zero-charge unfounded Prophet still walks", host.ops[1] ~= nil and host.ops[1].x, 3)

-- A Holy Site whose pillage state cannot be read is not refused for it.
reset(26)
host.cities[1] = city({ district(1, 1, 1, true), {
	GetType = function() return 2 end,
	GetX = function() return 3 end,
	GetY = function() return 1 end,
	IsComplete = function() return true end,
	IsPillaged = function() error("no accessor") end,
} })
host.paths["26:" .. site] = { plots = { origin, plotIndex(2, 1), site }, turns = { 0, 1, 1 } }
applyOrders(player, PID, 7, {})
check("an unreadable pillage state does not block the walk", host.ops[1] ~= nil and host.ops[1].x, 3)

-- With no route the refusal is reported, by name.
reset(27)
host.paths["27:" .. site] = { plots = { origin }, turns = { 0 } }
applyOrders(player, PID, 7, {})
check("an unwalked Prophet names its reason",
	table.concat(LOG, "\n"):find('"reason":"no_reachable_holy_site"', 1, true) ~= nil, true)

-- On the Holy Site the host takes FOUND_RELIGION: the Prophet stays for the
-- `religion` order.
reset(21)
host.units[21].x = 3
applyOrders(player, PID, 7, {})
check("a Prophet that can found here does not walk", #host.ops, 0)

-- No route to the Holy Site: no MOVE_TO the host would leave unmoved.
reset(22)
host.paths["22:" .. site] = { plots = { origin }, turns = { 0 } }
applyOrders(player, PID, 7, {})
check("an unreachable Holy Site is not walked to", #host.ops, 0)

-- A Holy Site still under construction cannot take the founding.
reset(23)
host.cities[1] = city({ district(1, 1, 1, true), district(2, 3, 1, false) })
host.paths["23:" .. site] = { plots = { origin, plotIndex(2, 1), site }, turns = { 0, 1, 1 } }
applyOrders(player, PID, 7, {})
check("an unfinished Holy Site is not walked to", #host.ops, 0)

-- The religion founded: the Prophet is no longer walked to found.
reset(24)
host.religion = 3
host.charges = 1
host.paths["24:" .. site] = { plots = { origin, plotIndex(2, 1), site }, turns = { 0, 1, 1 } }
applyOrders(player, PID, 7, {})
check("a founded seat's Prophet is not walked to found", #host.ops, 0)

if failures > 0 then
	print(string.format("%d failure(s)", failures))
	os.exit(1)
end
print("all prophet holy site walk checks passed")
