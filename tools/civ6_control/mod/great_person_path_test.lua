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

local host = { units = {}, ops = {}, paths = {}, cities = {}, commands = {} }
local PID = 0
local function greatPerson(individual)
	return {
		IsGreatPerson = function() return true end,
		GetIndividual = function() return individual or host.individual or GP_INDIVIDUAL end,
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
local function lastEvent(kind)
	for i = #LOG, 1, -1 do
		if LOG[i]:find('"kind":"' .. kind .. '"', 1, true) then
			return LOG[i]
		end
	end
	return nil
end
local function reset(id)
	host.units, host.ops, host.paths, host.commands = {}, {}, {}, {}
	host.cities, host.individual, host.charges, host.canActivate, host.launched = {}, nil, nil, false, false
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

-- Exercise the actual order driver, including activation before movement.
-- Standard-speed costs from Projects.xml:29-33 and Expansion2_Projects.xml:32-35;
-- the city queue below prices them at Online speed (half), as the host does.
local projects = {}
for index, entry in ipairs({
	{ "PROJECT_LAUNCH_EARTH_SATELLITE", 900 }, { "PROJECT_LAUNCH_MOON_LANDING", 1500 },
	{ "PROJECT_LAUNCH_MARS_BASE", 1800 }, { "PROJECT_LAUNCH_EXOPLANET_EXPEDITION", 2100 },
	{ "PROJECT_ORBITAL_LASER", 600 }, { "PROJECT_TERRESTRIAL_LASER", 600 },
	{ "PROJECT_LAUNCH_MARS_REACTOR", 1800 } }) do
	projects[index] = { Index = index, Hash = index, ProjectType = entry[1], Cost = entry[2] }
	projects[entry[1]] = projects[index]
end
GameInfo.Projects = projects
local SPACEPORT = { Index = 77, DistrictType = "DISTRICT_SPACEPORT" }
GameInfo.Districts = setmetatable({ [77] = SPACEPORT, DISTRICT_SPACEPORT = SPACEPORT },
	{ __call = function() return function() return nil end end })
GameInfo.GreatPersonIndividuals[1002] = { GreatPersonIndividualType = "GREAT_PERSON_INDIVIDUAL_CARL_SAGAN" }
GameInfo.GreatPersonIndividuals[1003] = { GreatPersonIndividualType = "GREAT_PERSON_INDIVIDUAL_SERGEI_KOROLEV" }
GameInfo.GreatPersonIndividuals[1004] = { GreatPersonIndividualType = "GREAT_PERSON_INDIVIDUAL_STEPHANIE_KWOLEK" }
Map.GetPlot = function(x, y) return { x = x, y = y } end
Cities = { GetPlotPurchaseCity = function(p) return host.cityAt and host.cityAt[p.x] end }
local function pad(x, y)
	return {
		GetType = function() return SPACEPORT.Index end,
		GetX = function() return x end, GetY = function() return y end,
		IsComplete = function() return true end,
		IsPillaged = function() return false end,
	}
end
-- One of our cities. `opts.progress` is the host's progress on the current
-- project; its cost is the Online price unless `opts.cost` names one.
local function spaceCity(id, project, opts)
	opts = opts or {}
	return {
		GetID = function() return id end, GetOwner = function() return PID end,
		GetBuildQueue = function() return {
			GetCurrentProductionTypeHash = function() return project end,
			GetTurnsLeft = function() return opts.turns or 10 end,
			GetProjectProgress = function() return opts.progress or 0 end,
			GetProjectCost = function(_, index)
				return opts.cost or (projects[index] and projects[index].Cost / 2) or -1
			end,
		} end,
		GetYield = function() return opts.production or 20 end,
		GetDistricts = function() return { Members = members(opts.pads or {}) } end,
	}
end
local function spaceCase(id, project, charges, turns, progress)
	reset(id)
	host.individual, host.charges, host.canActivate = 1002, charges, true
	local city = spaceCity(id, project, { turns = turns, progress = progress })
	host.cities, host.cityAt = { city }, { [1] = city, [2] = city, [4] = city }
end
-- The launches are serial, so a fresh link of any stage takes the charge
-- (old policy: reserved for Exoplanet, 6 of 12 live people never used).
for _, project in ipairs({1, 2, 3}) do
	spaceCase(20 + project, project, 1)
	applyOrders(player, PID, 7, {})
	check("fresh launch link takes the charge at stage " .. project, #host.commands, 1)
	check("activation reported at stage " .. project,
		(lastEvent("gp") or ""):find('"action":"activated"', 1, true) ~= nil, true)
end
spaceCase(30, 3, 2)
host.individual = 1003
applyOrders(player, PID, 7, {})
check("Korolev accelerates Mars", #host.commands, 1)
spaceCase(31, 4, 1)
applyOrders(player, PID, 7, {})
check("last charge launches Exoplanet", #host.commands, 1)
applyOrders(player, PID, 7, {})
check("asynchronous activation cannot spend twice on one city", #host.commands, 1)
spaceCase(32, 4, 1, 1)
applyOrders(player, PID, 7, {})
check("one-turn project does not consume boost", #host.commands, 0)
spaceCase(33, 5, 1)
applyOrders(player, PID, 7, {})
check("leftover grant accelerates a laser", #host.commands, 1)
spaceCase(34, 1, 1)
host.individual = 1004
applyOrders(player, PID, 7, {})
check("permanent Kwolek bonus activates early", #host.commands, 1)
spaceCase(35, 7, 1)
projects.PROJECT_LAUNCH_EXOPLANET_EXPEDITION = nil
applyOrders(player, PID, 7, {})
check("vanilla charge can finish Mars", #host.commands, 1)
projects.PROJECT_LAUNCH_EXOPLANET_EXPEDITION = projects[4]
spaceCase(36, 4, 1)
host.canActivate = false
host.cityAt[1], host.cityAt[2] = nil, nil
host.paths["36:" .. reachable] = { plots = { origin, reachable }, turns = { 0, 1 } }
applyOrders(player, PID, 7, {})
check("walk to the spending city instead of nearest highlight", host.ops[1] and host.ops[1].x, 4)
spaceCase(37, 4, 1)
host.cityAt = {}
applyOrders(player, PID, 7, {})
check("unknown city cannot consume finite grant", #host.commands, 0)

-- Hold only when the next link would take at least twice what this one still
-- can. Live 204504Z: Sagan born beside a Mars Base 709/900 done (4 turns to
-- save) spent on the Exoplanet four turns later (10 turns).
spaceCase(50, 3, 1, 3, 709)
applyOrders(player, PID, 7, {})
check("nearly built Mars waits for the Exoplanet", #host.commands, 0)
check("the wait is reported", (lastEvent("gp") or ""):find('"action":"reserved_space_boost"', 1, true) ~= nil, true)
spaceCase(51, 3, 1, 3, 709)
host.individual = 1003
applyOrders(player, PID, 7, {})
check("nearly built Mars waits for Korolev too", #host.commands, 0)
-- Live 211309Z: Korolev passed a Moon 158/750 with 12 turns left at t178 and
-- was spent on the t196 Exoplanet (about 4 turns).
spaceCase(52, 2, 1, 12, 158)
host.individual = 1003
applyOrders(player, PID, 7, {})
check("a part-built Moon with most of its build left takes Korolev", #host.commands, 1)
spaceCase(53, 1, 1, 3, 300)
host.individual = 1003
applyOrders(player, PID, 7, {})
check("a Satellite two-thirds built waits for the Moon", #host.commands, 0)

spaceCase(38, 1, 1)
applyOrders(player, PID, 7, { { kind = "unit", subject = 38, verb = "ACTIVATE_GREAT_PERSON" } })
check("explicit order spends on the first link", #host.commands, 1)
local civvisActivation = nil
for _, line in ipairs(LOG) do
	if line:find('"action":"activated_by_civvis"', 1, true) and line:find('"unit":38', 1, true) then
		civvisActivation = line
	end
end
check("CIVVIS's activation is reported as a gp event", civvisActivation ~= nil, true)
check("the report names the individual", civvisActivation ~= nil
	and civvisActivation:find("GREAT_PERSON_INDIVIDUAL_CARL_SAGAN", 1, true) ~= nil, true)
spaceCase(54, 3, 1, 3, 709)
applyOrders(player, PID, 7, { { kind = "unit", subject = 54, verb = "ACTIVATE_GREAT_PERSON" } })
check("explicit order still reserves a nearly built link", #host.commands, 0)
spaceCase(39, 4, 1)
applyOrders(player, PID, 7, { { kind = "unit", subject = 39, verb = "ACTIVATE_GREAT_PERSON" } })
check("explicit Exoplanet activation order consumes charge once", #host.commands, 1)
spaceCase(40, 1, 1)
local policy = CivvisSpaceBoost.forUnit(player, unitObject(host.units[40]), 7)
check("bridge exports first-link movement targets", #CivvisSpaceBoost.filterPlots(player, policy,
	{ { x = 2, y = 1 }, { x = 4, y = 1 } }), 2)
spaceCase(41, 4, 1)
host.cityAt[2] = nil
policy = CivvisSpaceBoost.forUnit(player, unitObject(host.units[41]), 7)
local filtered = CivvisSpaceBoost.filterPlots(player, policy, { { x = 2, y = 1 }, { x = 4, y = 1 } })
check("bridge exports eligible target only", #filtered, 1)
check("bridge exports correct city", filtered[1] and filtered[1].x, 4)

-- ★ Waiting happens on a Spaceport, not at the birthplace. No city is
-- launching anything, so the person walks to the pad and holds there.
local function stageCase(id, cities)
	reset(id)
	host.individual, host.charges, host.canActivate = 1002, 1, false
	host.cities, host.cityAt = cities, {}
end
local padA, padB = plotIndex(6, 1), plotIndex(8, 1)
stageCase(60, { spaceCity(60, nil, { pads = { pad(6, 1) } }) })
host.paths["60:" .. padA] = { plots = { origin, plotIndex(5, 1), padA }, turns = { 0, 1, 1 } }
applyOrders(player, PID, 7, {})
check("held space person walks to the Spaceport", host.ops[1] and host.ops[1].x, 6)
check("the walk is reported once", (lastEvent("gp") or ""):find('"action":"moving_to_spaceport"', 1, true) ~= nil, true)
check("no charge spent while nothing launches", #host.commands, 0)
local stagePlots = CivvisSpaceBoost.stagePlots(player, unitObject(host.units[60]))
check("export names the pad for civvis_orders", stagePlots[1] and stagePlots[1].x, 6)
check("export pad distance", stagePlots[1] and stagePlots[1].distance, 5)

stageCase(61, { spaceCity(61, nil, { pads = { pad(6, 1) } }) })
host.units[61].x = 6
applyOrders(player, PID, 7, {})
check("a person on the pad stays", #host.ops, 0)
check("arrival is reported on the pad",
	(lastEvent("gp") or ""):find('"on_spaceport":true', 1, true) ~= nil, true)
check("stage plot at distance 0 is civvis_orders' hold",
	CivvisSpaceBoost.stagePlots(player, unitObject(host.units[61]))[1].distance, 0)

stageCase(62, { spaceCity(62, nil, { pads = { pad(6, 1) } }) })
applyOrders(player, PID, 7, {})
check("an unreachable pad is not walked to", #host.ops, 0)

-- The city running a launch outranks a richer one; a pad a sibling holds is
-- skipped (one civilian per plot).
local launching = spaceCity(63, 1, { progress = 400, production = 10, pads = { pad(6, 1) } })
local richer = spaceCity(64, nil, { production = 50, pads = { pad(8, 1) } })
stageCase(63, { launching, richer })
host.paths["63:" .. padA] = { plots = { origin, padA }, turns = { 0, 1 } }
host.paths["63:" .. padB] = { plots = { origin, padB }, turns = { 0, 2 } }
applyOrders(player, PID, 7, {})
check("the launching city's pad outranks more production", host.ops[1] and host.ops[1].x, 6)
stageCase(63, { launching, richer })
host.paths["63:" .. padA] = { plots = { origin, padA }, turns = { 0, 1 } }
host.paths["63:" .. padB] = { plots = { origin, padB }, turns = { 0, 2 } }
host.units[65] = { id = 65, kind = "UNIT_GREAT_ENGINEER", x = 6, y = 1, gp = greatPerson(1003) }
applyOrders(player, PID, 7, {})
local sagan = nil
for _, op in ipairs(host.ops) do if op.id == 63 then sagan = op end end
check("a pad a sibling holds is skipped", sagan and sagan.x, 8)

if failures > 0 then
	print(string.format("%d failure(s)", failures))
	os.exit(1)
end
print("all checks passed")
