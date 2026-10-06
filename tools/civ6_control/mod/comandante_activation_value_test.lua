-- Offline regression for useful Urdaneta retirement.
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
			GreatPersonClassType = "GREAT_PERSON_CLASS_COMANDANTE_GENERAL",
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
		GetMovesRemaining = function() return u.moves or 4 end,
		GetMaxMoves = function() if host.missingMoves then error("unknown") end; return u.max_moves or 4 end,
		GetAttacksRemaining = function() return u.attacks == nil and 1 or u.attacks end,
		GetUnitType = function() return u.kind end,
		GetType = function() return u.kind end,
		GetDamage = function() return u.damage or 0 end,
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

GameInfo.GreatPersonIndividuals[1005] = { GreatPersonIndividualType = "GREAT_PERSON_INDIVIDUAL_COMMANDANTE_URDANETA" }
GameInfo.Units.UNIT_COMANDANTE_GENERAL = { UnitType = "UNIT_COMANDANTE_GENERAL", Domain = "DOMAIN_LAND", Combat = 0, RangedCombat = 0 }
GameInfo.Units.UNIT_ROCKET_ARTILLERY = { UnitType = "UNIT_ROCKET_ARTILLERY", Domain = "DOMAIN_LAND", Combat = 70, RangedCombat = 0 }
GameInfo.Units.UNIT_COLOMBIAN_LLANERO = { UnitType = "UNIT_COLOMBIAN_LLANERO", Domain = "DOMAIN_LAND", Combat = 62, RangedCombat = 0 }
GameInfo.Units.UNIT_DESTROYER = { UnitType = "UNIT_DESTROYER", Domain = "DOMAIN_SEA", Combat = 85, RangedCombat = 0 }
local function reset()
	host.units, host.ops, host.paths, host.commands = {}, {}, {}, {}
	host.individual, host.canActivate, host.missingMoves = 1005, true, false
	host.units[1] = { id = 1, kind = "UNIT_COMANDANTE_GENERAL", x = 1, y = 1, gp = greatPerson() }
	host.units[2] = { id = 2, kind = "UNIT_ROCKET_ARTILLERY", x = 2, y = 1, moves = 4, max_moves = 4, attacks = 1 }
end
local function run(rows)
	applyOrders(player, PID, 7, rows or {})
end
reset(); run()
check("fallback preserves charge beside fully ready artillery", #host.commands, 0)
reset(); run({ { kind = "unit", subject = 1, verb = "ACTIVATE_GREAT_PERSON" } })
check("explicit activation cannot bypass useful-retirement guard", #host.commands, 0)
reset(); host.units[2].moves = 1; run()
check("spent movement can consume reset", #host.commands, 1)
reset(); host.units[2].attacks = 0; run()
check("spent attack with full movement can consume reset", #host.commands, 1)
reset(); host.units[2].kind, host.units[2].damage = "UNIT_COLOMBIAN_LLANERO", 25; run()
check("wounded Llanero can consume retirement heal", #host.commands, 1)
reset(); host.units[2].kind = "UNIT_COLOMBIAN_LLANERO"; run()
check("healthy ready Llanero does not spend charge", #host.commands, 0)
reset(); host.units[2].kind, host.units[2].moves = "UNIT_DESTROYER", 0; run()
check("naval movement cannot consume land reset", #host.commands, 0)
reset(); host.units[2] = nil; run()
check("empty army does not consume charge", #host.commands, 0)
reset(); host.units[2] = nil; host.units[1].moves = 1; run()
check("Comandante's own walk does not justify consuming him", #host.commands, 0)
reset(); host.units[2].x, host.units[2].moves = 4, 1; run()
check("spent movement outside two tiles cannot consume charge here", #host.commands, 0)
reset(); host.units[2].x, host.units[2].moves = 3, 1; run()
check("exactly two tiles receives movement reset", #host.commands, 1)
reset(); GameInfo.Units.UNIT_ROCKET_ARTILLERY.Domain = nil; run()
check("unknown domain keeps compatibility", #host.commands, 1)
GameInfo.Units.UNIT_ROCKET_ARTILLERY.Domain = "DOMAIN_LAND"
reset(); host.units[2].moves = -1; run()
check("negative movement observation keeps compatibility", #host.commands, 1)
reset(); host.units[2].x, host.units[2].y, host.units[2].moves = -1, -1, 0; run()
check("off-map unit cannot justify consuming local charge", #host.commands, 0)
reset(); host.missingMoves = true; run()
check("unknown movement authority keeps compatibility", #host.commands, 1)
reset(); host.individual = GP_INDIVIDUAL; run()
check("unrelated Great Person keeps activation behavior", #host.commands, 1)

-- The highlighted nearer tile offers no benefit; a reachable farther tile
-- can reset this unit. The real fallback driver must choose the useful tile.
reset(); host.canActivate = false; host.units[2].x, host.units[2].moves = 5, 1
host.paths["1:" .. plotIndex(2, 1)] = { plots = { plotIndex(1, 1), plotIndex(2, 1) }, turns = { 0, 1 } }
host.paths["1:" .. plotIndex(4, 1)] = { plots = { plotIndex(1, 1), plotIndex(4, 1) }, turns = { 0, 1 } }
run()
check("movement targets useful farther highlight", host.ops[1] and host.ops[1].x, 4)

if rawget(_G, "CivvisComandante") then
	local targets = CivvisComandante.forUnit(player, unitObject(host.units[1]))
	local filtered = CivvisComandante.filterPlots(targets, { { x = 2, y = 1 }, { x = 4, y = 1 } })
	check("bridge exports only useful highlight", #filtered, 1)
	check("bridge useful highlight keeps coordinates", filtered[1] and filtered[1].x, 4)
else
	check("bridge receives useful-retirement policy", false, true)
end
if failures > 0 then print(string.format("%d failure(s)", failures)); os.exit(1) end
print("all checks passed")
