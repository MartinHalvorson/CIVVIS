-- Exercise the actual order driver with native API stubs.
-- Run: lua5.1 tools/civ6_control/mod/comandante_aura_support_test.lua
-- Horizontal paths have exact native hex distances; no aura application is simulated.

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

local host
local function plot(x, y)
	return {
		GetX = function() return x end,
		GetY = function() return y end,
		GetOwner = function() if host.unknownOwner then error("unknown") end; return host.owners[y * 100 + x] or 0 end,
		IsWater = function() return host.water[y * 100 + x] == true end,
		IsImpassable = function() return false end,
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
	GetPlot = function(x,y) return plots[plotIndex(x,y)] end,
	GetPlotIndex = plotIndex,
	GetPlotByIndex = function(index) return plots[index] end,
}

local GP_INDIVIDUAL = 1001
local GP_CLASS = 2001
GameInfo = setmetatable({
	GreatPersonIndividuals = {
		[GP_INDIVIDUAL] = {
			GreatPersonIndividualType = "GREAT_PERSON_INDIVIDUAL_COMMANDANTE_ANTONIO_PAEZ",
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
	CapMovesToReach = false,
})
setmetatable(_G, { __index = function(_, key)
	if EXPORTS[key] then return rawget(_G, key) end
	return stub()
end })

host = { units = {}, ops = {}, paths = {}, cities = {}, commands = {} }
local PID = 0
local function greatPerson()
	return {
		IsGreatPerson = function() return true end,
		GetIndividual = function() return host.individual or GP_INDIVIDUAL end,
		GetClass = function() return host.class or GP_CLASS end,
		GetActionCharges = function() return host.charges == nil and 1 or host.charges end,
		GetActivationHighlightPlots = function()
			return host.highlights or {}
		end,
	}
end
local function unitObject(u)
	return {
		GetID = function() return u.id end,
		GetOwner = function() return PID end,
		IsEmbarked = function() return u.embarked or false end,
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
	CanStartOperation = function(_,op) return not (host.refuseMove and op == "UNITOPERATION_MOVE_TO") end,
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

local failures, checks = 0, 0
local function check(name, got, want)
 checks = checks + 1
 if got ~= want then failures = failures + 1; print("FAIL " .. name .. ": got " .. tostring(got) .. " want " .. tostring(want))
 else print("ok   " .. name) end
end
local tagRows = {}
local function tagIterator()
 local i = 0
 return function() i = i + 1; return tagRows[i] end
end
GameInfo.TypeTags = tagIterator
GameInfo.GreatPersonIndividuals[1005] = { GreatPersonIndividualType = "GREAT_PERSON_INDIVIDUAL_COMMANDANTE_URDANETA" }
GameInfo.GreatPersonClasses[2002] = { GreatPersonClassType = "GREAT_PERSON_CLASS_SCIENTIST" }
GameInfo.Units.UNIT_COMANDANTE_GENERAL = { UnitType = "UNIT_COMANDANTE_GENERAL", Domain = "DOMAIN_LAND", Combat = 0, RangedCombat = 0 }
GameInfo.Units.UNIT_WARRIOR = { UnitType = "UNIT_WARRIOR", Domain = "DOMAIN_LAND", Combat = 20, RangedCombat = 0 }
GameInfo.Units.UNIT_CUSTOM_CAVALRY = { UnitType = "UNIT_CUSTOM_CAVALRY", Domain = "DOMAIN_LAND", Combat = 60, RangedCombat = 0 }
GameInfo.Units.UNIT_DESTROYER = { UnitType = "UNIT_DESTROYER", Domain = "DOMAIN_SEA", Combat = 80, RangedCombat = 0 }
GameInfo.Units.UNIT_BUILDER = { UnitType = "UNIT_BUILDER", Domain = "DOMAIN_LAND", Combat = 0, RangedCombat = 0 }
GameInfo.Units.UNIT_UNTAGGED_LAND = { UnitType = "UNIT_UNTAGGED_LAND", Domain = "DOMAIN_LAND", Combat = 130, RangedCombat = 0 }
local function pathTo(x)
 local path = {}
 for col = 1,x do path[#path + 1] = plotIndex(col,1) end
 local turns = {}; for i=1,#path do turns[i]=i==1 and 0 or 1 end
 return { plots = path, turns = turns }
end
local function reset()
 host.units, host.ops, host.paths, host.commands = {}, {}, {}, {}
 host.owners, host.water, host.highlights = {}, {}, {}
 host.individual, host.class, host.charges, host.canActivate = GP_INDIVIDUAL, GP_CLASS, 1, false
 host.unknownOwner, host.refuseMove, host.missingMoves = false, false, false
 UnitManager.GetMoveToPathEx = pathFinder
 GameInfo.TypeTags = tagIterator
 tagRows = {
  { Type = "ABILITY_COMANDANTE_AOE_STRENGTH", Tag = "CLASS_MELEE" },
  { Type = "ABILITY_COMANDANTE_AOE_STRENGTH", Tag = "CLASS_LIGHT_CAVALRY" },
  { Type = "UNIT_WARRIOR", Tag = "CLASS_MELEE" },
  { Type = "UNIT_CUSTOM_CAVALRY", Tag = "CLASS_LIGHT_CAVALRY" },
  { Type = "UNIT_DESTROYER", Tag = "CLASS_MELEE" },
  { Type = "UNIT_BUILDER", Tag = "CLASS_MELEE" },
 }
 for x=1,12 do plots[plotIndex(x,1)] = plot(x,1) end
 host.units[1] = { id = 1, kind = "UNIT_COMANDANTE_GENERAL", x = 1, y = 1, gp = greatPerson(), moves = 4 }
 host.units[2] = { id = 2, kind = "UNIT_WARRIOR", x = 6, y = 1, moves = 4, max_moves = 4, attacks = 1 }
 host.paths["1:" .. plotIndex(6,1)] = pathTo(6)
 host.paths["1:" .. plotIndex(3,1)] = pathTo(3)
 CivvisComandante.auraTags, CivvisComandante.auraChecked = nil, {}
end
local function run(rows) applyOrders(player, PID, 7, rows or {}) end
local function moves()
 local result = {}
 for _,op in ipairs(host.ops) do if op.id==1 and op.operation=="UNITOPERATION_MOVE_TO" then result[#result+1] = op end end
 return result
end
local function scenario(name, configure, expectedX, expectedCommands)
 reset(); if configure then configure() end; run()
 local result = moves()
 check(name .. " movement count", #result, expectedX and 1 or 0)
 if expectedX then check(name .. " destination", result[1] and result[1].x, expectedX) end
 check(name .. " activation count", #host.commands, expectedCommands or 0)
end
scenario("idle Paez moves to owned reachable recipient", nil, 6)
scenario("tag discovery includes custom cavalry", function() host.units[2].kind="UNIT_CUSTOM_CAVALRY" end,6)
scenario("Urdaneta reserves ready army and follows for aura", function() host.individual=1005 end,6)
scenario("existing aura recipient preserves position", function() host.units[2].x=3 end,nil)
scenario("naval unit is not an aura recipient", function() host.units[2].kind="UNIT_DESTROYER" end,nil)
scenario("civilian with a matching tag is not an army recipient", function() host.units[2].kind="UNIT_BUILDER" end,nil)
scenario("untagged land military is ineligible", function() host.units[2].kind="UNIT_UNTAGGED_LAND" end,nil)
scenario("embarked recipient is not a destination", function() host.units[2].embarked=true end,nil)
scenario("off-map recipient is ignored", function() host.units[2].x=-1 end,nil)
scenario("empty army holds", function() host.units[2]=nil end,nil)
scenario("spent charge holds", function() host.charges=0 end,nil)
scenario("zero remaining movement holds", function() host.units[1].moves=0 end,nil)
scenario("unreachable recipient holds", function() host.paths={} end,nil)
scenario("future-turn path holds", function() host.paths["1:"..plotIndex(6,1)].turns[6]=2 end,nil)
scenario("missing turn allowance holds", function() host.paths["1:"..plotIndex(6,1)].turns=nil end,nil)
scenario("partial path holds", function() host.paths["1:"..plotIndex(6,1)]=pathTo(3) end,nil)
scenario("one-entry no-progress path holds", function() host.paths["1:"..plotIndex(6,1)]={plots={plotIndex(6,1)}} end,nil)
scenario("path through foreign land holds", function() host.owners[plotIndex(4,1)]=1 end,nil)
scenario("path through water holds", function() host.water[plotIndex(4,1)]=true end,nil)
scenario("unknown ownership holds", function() host.unknownOwner=true end,nil)
scenario("missing path authority holds", function() UnitManager.GetMoveToPathEx=nil end,nil)
scenario("missing rule authority holds", function() GameInfo.TypeTags=nil end,nil)
scenario("refused movement stays idle", function() host.refuseMove=true end,nil)
scenario("occupied civilian destination holds", function() host.units[3]={id=3,kind="UNIT_BUILDER",x=6,y=1} end,nil)
scenario("another Comandante already covers recipient", function() host.units[3]={id=3,kind="UNIT_COMANDANTE_GENERAL",x=5,y=1,gp=greatPerson()} end,nil)
scenario("unrelated Great Person remains idle", function() host.class=2002 end,nil)
scenario("activation retains first priority", function() host.canActivate=true end,nil,1)
scenario("activation destination retains priority", function() host.highlights={plotIndex(4,1)};host.paths["1:"..plotIndex(4,1)]=pathTo(4) end,4)
reset();run();run()
check("asynchronous support requested once per turn",#moves(),1)
reset();run({{kind="unit",subject=1,verb="MOVE_TO",x=3,y=1}})
check("explicit controller movement owns this batch",#moves(),1)
check("explicit controller destination preserved",moves()[1] and moves()[1].x,3)
run();check("earlier explicit movement owns later frame",#moves(),1)
if failures>0 then print(failures .. " failure(s) / " .. checks .. " checks");os.exit(1) end
print("all " .. checks .. " checks passed")
