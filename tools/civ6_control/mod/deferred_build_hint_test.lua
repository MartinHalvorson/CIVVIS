-- Run with Lua 5.1. Host fixture follows order_verdict_test.lua.
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
	CivvisApplyOrder = true, CivvisVerify = true,
}
-- Real tables the agent indexes with real keys.
CivvisControlConfig = { Play = true, CivvisDecides = false, EmergencyWallRadius = 3 }
local LOG = {}
Automation = { Log = function(line) LOG[#LOG + 1] = line end }
UnitOperationTypes = { PARAM_X = "x", PARAM_Y = "y" }
UnitCommandTypes = {}
Map = {
	GetPlotDistance = function(x1, y1, x2, y2)
		-- Manhattan-ish is enough for a radius test on an offset grid.
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
			if name == "UNIT_SETTLER" then return { UnitType = name, Combat = 0, RangedCombat = 0 } end
			if name == "UNIT_SCOUT" then return { UnitType = name, Combat = 10, PromotionClass = "PROMOTION_CLASS_RECON" } end
			if name == "UNIT_ARCHER" then return { UnitType = name, Combat = 15, RangedCombat = 25 } end
			return { UnitType = name, Combat = 20, RangedCombat = 0 }
		end })
	end
	return stub()
end })
-- The production path asks whether a requested type replaces a Holy Site.
-- This fixture has no unique districts; the database iterator must terminate.
-- The catch-all callable stub above returns another stub forever.
GameInfo.DistrictReplaces = function() return function() return nil end end
GameInfo.Types = {
	UNIT_SETTLER = { Hash = 101, Kind = "KIND_UNIT" },
	UNIT_SCOUT = { Hash = 102, Kind = "KIND_UNIT" },
	UNIT_ARCHER = { Hash = 103, Kind = "KIND_UNIT" },
}
GameInfo.CommemorationTypes = {
	[17] = { CommemorationType = "COMMEMORATION_INFRASTRUCTURE" },
	[19] = { CommemorationType = "COMMEMORATION_SCIENTIFIC" },
}
CityOperationTypes = {
	BUILD = "BUILD",
	VALUE_REPLACE_AT = "REPLACE_AT",
	PARAM_INSERT_MODE = "insert_mode",
	PARAM_QUEUE_DESTINATION_LOCATION = "queue_destination",
	PARAM_UNIT_TYPE = "unit_type",
	PARAM_BUILDING_TYPE = "building_type",
}
setmetatable(_G, { __index = function(_, k)
	if EXPORTS[k] then return rawget(_G, k) end
	return stub()
end })

-- ---------------------------------------------------------------- fake host
local host = { units = {}, ops = {}, refuse = {}, contacts = {}, cities = {}, cityOps = {} }
local PID = 0
local function unitObject(u)
	return {
		GetID = function() return u.id end,
		GetX = function() return u.x end,
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
local function cityObject(c)
	return {
		GetID = function() return c.id end,
		GetX = function() return c.x or 0 end,
		GetY = function() return c.y or 0 end,
		GetBuildQueue = function()
			return {
				GetCurrentProductionTypeHash = function() return c.current or 0 end,
				GetTurnsLeft = function() return c.turns or -1 end,
				CanProduce = function(_, hash, exclusion)
					if exclusion then return not (c.excluded and c.excluded[hash]) end
					return not (c.unstartable and c.unstartable[hash])
				end,
				HasBeenPlaced = function() return false end,
			}
		end,
	}
end
UnitManager = {
	GetUnit = function(pid, id)
		local u = host.units[id]
		if u == nil or u.gone then return nil end
		return unitObject(u)
	end,
	CanStartOperation = function(unit, hash, _, params)
		local id = unit.GetID()
		if host.refuse[id] and host.refuse[id][hash] then return false end
		return true
	end,
	RequestOperation = function(...)
		local argc = select("#", ...)
		local unit, hash, params = ...
		local u = host.units[unit.GetID()]
		host.ops[#host.ops + 1] = { id = u.id, op = hash,
			argc = argc, x = params and params.x or nil,
			y = params and params.y or nil }
		if hash == "UNITOPERATION_MOVE_TO" then
			-- Asynchronous, like the host: the unit arrives only when the
			-- test says so (`host.arrive`), unless the walk was priced dead.
			u.pendingX, u.pendingY = params.x, params.y
		end
	end,
	CanStartCommand = function() return false end,
	RequestCommand = function() end,
}
CityManager = {
	GetDistrictAt = function() return nil end,
	RequestOperation = function(city, op, params)
		local c = host.cities[city:GetID()]
		host.cityOps[#host.cityOps + 1] = { city = city:GetID(), op = op,
			item = (params[CityOperationTypes.PARAM_UNIT_TYPE] or params[CityOperationTypes.PARAM_BUILDING_TYPE]) }
		if c ~= nil then c.current = (params[CityOperationTypes.PARAM_UNIT_TYPE] or params[CityOperationTypes.PARAM_BUILDING_TYPE]) end
	end,
}
function host.arrive(id)
	local u = host.units[id]
	if u.pendingX ~= nil then
		u.x, u.y = u.pendingX, u.pendingY
		u.pendingX, u.pendingY = nil, nil
		u.moves = math.max(0, (u.moves or 0) - 1)
	end
end
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
		for _, u in pairs(host.units) do
			if not u.gone then objs[#objs + 1] = unitObject(u) end
		end
		table.sort(objs, function(a, b) return a.GetID() < b.GetID() end)
		return { Members = members(objs) }
	end,
	GetCities = function()
		local cities = {}
		for _, c in pairs(host.cities) do cities[#cities + 1] = cityObject(c) end
		table.sort(cities, function(a, b) return a:GetID() < b:GetID() end)
		return {
			Members = members(cities),
			FindID = function(_, id)
				local c = host.cities[id]
				return c ~= nil and cityObject(c) or nil
			end,
		}
	end,
	GetDiplomacy = function() return { IsAtWarWith = function() return false end } end,
	GetScore = function() return 0 end,
	GetTreasury = function() return { GetGoldBalance = function() return 0 end } end,
	IsTurnActive = function() return true end,
}, { __index = function() return stub() end })
Players = setmetatable({}, { __index = function(_, pid)
	if pid == PID then return player end
	-- One barbarian seat with the hostile units the test plants.
	return setmetatable({
		IsBarbarian = function() return true end,
		GetUnits = function()
			local objs = {}
			for _, u in ipairs(host.contacts) do objs[#objs + 1] = unitObject(u) end
			return { Members = members(objs) }
		end,
		GetCities = function() return { Members = members({}) } end,
	}, { __index = function() return stub() end })
end })
PlayerManager = { GetAliveIDs = function() return { PID, 63 } end,
                  GetAliveMajorIDs = function() return { PID } end }
PlayersVisibility = setmetatable({}, { __index = function()
	return { IsVisible = function() return true end, IsRevealed = function() return true end }
end })
local playerOperations = {}
PlayerOperations = {
	PARAM_COMMEMORATION_TYPE = "commemoration_type",
	COMMEMORATE = "COMMEMORATE",
}
UI = {
	RequestPlayerOperation = function(pid, operation, parameters)
		playerOperations[#playerOperations + 1] = {
			pid = pid, operation = operation, parameters = parameters,
		}
	end,
}
Game = {
	GetLocalPlayer = function() return PID end,
	GetCurrentGameTurn = function() return 7 end,
	GetEras = function()
		return {
			GetPlayerNumAllowedCommemorations = function() return 1 end,
			GetPlayerCommemorateChoices = function() return { 17, 19 } end,
		}
	end,
}

-- Exercise the shipped actuator through its public hint intake and real
-- production closure; the fake host controls completion and legal targets.
GameInfo.Types.DISTRICT_CAMPUS = { Hash = 201, Kind = "KIND_DISTRICT", Type = "DISTRICT_CAMPUS" }
CityOperationTypes.PARAM_DISTRICT_TYPE = "district_type"
CityOperationTypes.PARAM_X, CityOperationTypes.PARAM_Y = "x", "y"
CityOperationResults = { PLOTS = "plots" }
local offered = { 100, 200 }
Map.GetPlotByIndex = function(index)
 local x = index == 100 and 1 or 2
 return { GetX = function() return x end, GetY = function() return 0 end }
end
CityManager.GetOperationTargets = function() return { plots = offered } end
CityManager.RequestOperation = function(city, op, params)
 local item = params.unit_type or params.building_type or params.district_type
 host.cityOps[#host.cityOps + 1] = { city = city:GetID(), item = item, x = params.x, y = params.y }
 host.cities[city:GetID()].current = item
end
local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, err)
assert(pcall(chunk))
CivvisControlConfig.CivvisDecides = true
local apply = rawget(_G, "CivvisApplyOrders")
local function upvalue(fn, wanted, seen)
 seen = seen or {}; if seen[fn] then return nil end; seen[fn] = true
 for i = 1, 250 do
  local name, value = debug.getupvalue(fn, i); if not name then break end
  if name == wanted then return value end
  if type(value) == "function" then
   local found = upvalue(value, wanted, seen); if found ~= nil then return found end
  end
 end
end
local drive = assert(upvalue(rawget(_G, "CivvisAnswerBlockerLadder"), "driveProduction"), "real production closure is reachable")
local memory = assert(upvalue(drive, "civvisBuild"), "real remembered directives are reachable")
local function hint(turn, item, x, y)
 apply(player, PID, turn, {{ kind = "produce_next", subject = 42, verb = item, x = x, y = y }})
end
local function equal(name, actual, expected)
 assert(actual == expected, name .. ": expected " .. tostring(expected) .. ", got " .. tostring(actual))
end
host.cities[42] = { id = 42, current = 103, turns = 1 }
-- A deferred lease cannot disturb the live queue.
hint(7, "DISTRICT_CAMPUS", 2, 0)
equal("hint intake issues no host build", #host.cityOps, 0)
drive(player, 7, true)
equal("running queue survives", host.cities[42].current, 103)
equal("blocker with running queue issues no host build", #host.cityOps, 0)
-- The host offers two legal sites; preserve the selected second one.
host.cities[42].current = 0
drive(player, 8, true)
equal("Campus starts", host.cityOps[1].item, 201)
equal("selected x survives deferred handoff", host.cityOps[1].x, 2)
equal("selected y survives deferred handoff", host.cityOps[1].y, 0)
-- The lease is one-shot, including after another completion.
host.cities[42].current = 0
drive(player, 9, true)
equal("consumed lease does not replay", #host.cityOps, 1)
-- A stale requested site must not turn into an arbitrary still-legal site.
offered = { 100 }; hint(10, "DISTRICT_CAMPUS", 2, 0)
drive(player, 10, true)
equal("expired selected site causes no substitute build", #host.cityOps, 1)
-- A next build takes over after the old direct directive completes.
offered = { 100, 200 }; memory[42] = "UNIT_ARCHER"
hint(11, "DISTRICT_CAMPUS", 2, 0)
drive(player, 11, true)
equal("next lease supersedes old repeatable directive", host.cityOps[2].item, 201)
host.cities[42].current = 0
drive(player, 12, true)
equal("old repeatable directive does not resurrect", #host.cityOps, 2)
-- Updating a lease replaces its coordinates; missing coordinates retain
-- legacy host selection rather than inheriting the old lease's selected site.
hint(13, "DISTRICT_CAMPUS", 2, 0)
hint(13, "DISTRICT_CAMPUS", nil, nil)
drive(player, 13, true)
equal("replacement without coordinates clears old site", host.cityOps[3].x, 1)
host.cities[42].current = 0
hint(14, "DISTRICT_CAMPUS", 2, nil)
drive(player, 14, true)
equal("partial coordinates cannot silently choose a site", #host.cityOps, 3)
-- A failed request keeps the lease, including its selected site, for retry.
hint(15, "DISTRICT_CAMPUS", 2, 0)
local request = CityManager.RequestOperation
CityManager.RequestOperation = function() error("host request failed") end
drive(player, 15, true)
equal("throw does not consume lease", memory["42:next"], "DISTRICT_CAMPUS")
CityManager.RequestOperation = request
drive(player, 16, true)
equal("retry retains selected site", host.cityOps[4].x, 2)
-- A newer direct directive removes both halves of an older lease.
host.cities[42].current = 0
hint(17, "DISTRICT_CAMPUS", 2, 0)
apply(player, PID, 17, {{ kind = "produce", subject = 42, verb = "UNIT_ARCHER" }})
equal("new direct order clears old lease", memory["42:next"], nil)
equal("new direct order clears old plot", memory["42:next-plot"], nil)
print("deferred build hint preserves queue, selected site, legality and one-shot handoff")
