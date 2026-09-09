-- The receiving side of the bridge, as the mod re-emits it: verdict rows on an
-- earlier turn's orders come back through the orders channel and become
-- `order_verified` / `order_failed` / `turn_verified` ledger events, and they
-- are kept out of every count that describes THIS turn's orders.
--
-- ⚠ Loads the SHIPPED `CivvisControlAgent.lua` and drives its own
-- `applyOrders` against the same fake host `order_queue_test.lua` uses, so the
-- accounting the ladder sums (`civ6_ladder.orders_ledger`) is the accounting
-- the agent performs.
--
-- What is checked:
--   1. a `turn` record names its return-code count twice: `orders_applied`
--      (the old name, for the readers that clock on it) and `orders_reported`;
--   2. a verdict row is re-emitted as an event carrying the verified turn, the
--      order's kind (`order_kind`), verb, subject and (when failed) reason, and the turn it
--      was checked on;
--   3. `turn_verified` lays the mod's remembered counts for the verified turn
--      beside the decider's tally;
--   4. verdict rows are neither seen nor applied in the turn they arrive on,
--      in both the `turn` and the `orders` record, and are counted as
--      `verdicts` there;
--   5. a verdict row never reaches the host as an operation.
--
-- Run: lua5.1 tools/civ6_control/mod/order_verdict_test.lua

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

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtime_err = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtime_err))

local applyOrders = rawget(_G, "CivvisApplyOrders")
local queue = rawget(_G, "CivvisQueue")
local resolveActions = rawget(_G, "CivvisResolveActions")
assert(type(applyOrders) == "function", "CivvisApplyOrders is not exported")
assert(type(queue) == "table", "CivvisQueue is not exported")
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
		if LOG[i]:find('"kind":"' .. kind .. '"', 1, true) then return LOG[i] end
	end
	return nil
end
local function field(line, name)
	if line == nil then return nil end
	local v = line:match('"' .. name .. '":(%-?%d+)')
	return v and tonumber(v) or line:match('"' .. name .. '":"([^"]*)"')
end
local function row(subject, verb, x, y)
	return { kind = "unit", subject = subject, verb = verb, x = x, y = y }
end

local verify = rawget(_G, "CivvisVerify")
assert(type(verify) == "table", "CivvisVerify is not exported")
host.units[10] = { id = 10, kind = "UNIT_WARRIOR", x = 1, y = 1, moves = 2 }

-- 1. Turn 7: one order, applied by return code, named both ways.
applyOrders(player, PID, 7, { row(10, "FORTIFY") })
local turn7 = lastEvent("turn")
check("t7 parameterless operation uses the UnitPanel signature",
	host.ops[#host.ops].argc, 2)
check("t7 orders_seen", field(turn7, "orders_seen"), 1)
check("t7 orders_applied (return code)", field(turn7, "orders_applied"), 1)
check("t7 orders_reported", field(turn7, "orders_reported"), 1)
check("t7 remembered for its verdict", verify.reported["7"] and verify.reported["7"].applied, 1)

-- 2-5. Turn 8: the decider's verdicts on turn 7 ride in with one real order.
local opsBefore = #host.ops
applyOrders(player, PID, 8, {
	row(10, "FORTIFY"),
	{ kind = "order_verified", subject = 10, verb = "unit:FORTIFY", x = 7, y = -1 },
	{ kind = "order_failed", subject = 65536,
	  verb = "produce:BUILDING_MONUMENT producing=UNIT_WARRIOR", x = 7, y = -1 },
	{ kind = "order_failed", subject = -1, verb = "research:TECH_WRITING research=TECH_MINING", x = 7, y = -1 },
	{ kind = "turn_verified", subject = 7, verb = "issued=3 verified=1 failed=2 unverifiable=0" },
})
local verified = lastEvent("order_verified")
check("order_verified turn", field(verified, "turn"), 7)
check("order_verified checked_on", field(verified, "checked_on"), 8)
check("order_verified order_kind", field(verified, "order_kind"), "unit")
check("order_verified verb", field(verified, "verb"), "FORTIFY")
check("order_verified subject", field(verified, "subject"), 10)
local failed = lastEvent("order_failed")
check("order_failed order_kind", field(failed, "order_kind"), "research")
check("order_failed verb", field(failed, "verb"), "TECH_WRITING")
check("order_failed reason", field(failed, "reason"), "research=TECH_MINING")
check("order_failed without subject carries none", failed:find('"subject"', 1, true), nil)
local firstFailed
for i = 1, #LOG do
	if LOG[i]:find('"kind":"order_failed"', 1, true) then firstFailed = LOG[i]; break end
end
check("order_failed subject", field(firstFailed, "subject"), 65536)
check("order_failed reason with the city's queue", field(firstFailed, "reason"), "producing=UNIT_WARRIOR")
local tally = lastEvent("turn_verified")
check("turn_verified turn", field(tally, "turn"), 7)
check("turn_verified checked_on", field(tally, "checked_on"), 8)
check("turn_verified orders_issued", field(tally, "orders_issued"), 3)
check("turn_verified orders_applied (verified)", field(tally, "orders_applied"), 1)
check("turn_verified orders_failed", field(tally, "orders_failed"), 2)
check("turn_verified orders_unverifiable", field(tally, "orders_unverifiable"), 0)
check("turn_verified orders_seen (mod, t7)", field(tally, "orders_seen"), 1)
check("turn_verified orders_reported (mod, t7)", field(tally, "orders_reported"), 1)
check("t7 memory released", verify.reported["7"], nil)
local turn8 = lastEvent("turn")
check("t8 orders_seen excludes verdicts", field(turn8, "orders_seen"), 1)
check("t8 orders_applied excludes verdicts", field(turn8, "orders_applied"), 1)
check("t8 orders_refused", field(turn8, "orders_refused"), 0)
local orders8 = lastEvent("orders")
check("t8 orders.seen excludes verdicts", field(orders8, "seen"), 1)
check("t8 orders.verdicts", field(orders8, "verdicts"), 4)
check("t8 orders.by has no verdict kind", orders8:find("order_verified", 1, true), nil)
check("a verdict row reaches no host operation", #host.ops - opsBefore, 1)

-- A tally for a turn this file never counted still lands, without the counts.
applyOrders(player, PID, 9, {
	{ kind = "turn_verified", subject = 5, verb = "issued=2 verified=2 failed=0 unverifiable=0" },
})
local orphan = lastEvent("turn_verified")
check("orphan tally turn", field(orphan, "turn"), 5)
check("orphan tally verified", field(orphan, "orders_applied"), 2)
check("orphan tally carries no mod count", orphan:find("orders_reported", 1, true), nil)
check("t9 orders_seen with only verdicts", field(lastEvent("turn"), "orders_seen"), 0)

-- 6. The first expansion Settler is not a disposable production queue. The
-- controller can re-evaluate after it starts; a non-Settler order must not
-- replace it while it is the only city's path to city two.
local applyOrder = rawget(_G, "CivvisApplyOrder")
assert(type(applyOrder) == "function", "CivvisApplyOrder is not exported")
local dedicationOk, dedicationWhy = applyOrder(player, PID, {
	kind = "dedication", verb = "COMMEMORATION_SCIENTIFIC",
}, 10)
assert(dedicationOk and dedicationWhy == "COMMEMORATION_SCIENTIFIC",
	"named dedication was not accepted: " .. tostring(dedicationWhy))
assert(#playerOperations == 1
	and playerOperations[1].operation == "COMMEMORATE"
	and playerOperations[1].parameters.commemoration_type == 19,
	"dedication did not request the offered named commemoration value")
local missingDedication, missingWhy = applyOrder(player, PID, {
	kind = "dedication", verb = "COMMEMORATION_CULTURAL",
}, 10)
assert(not missingDedication
	and missingWhy == "dedication_not_offered_COMMEMORATION_CULTURAL",
	"an unoffered dedication must be refused by name")
host.cities[42] = { id = 42, x = 4, y = 4, current = 101 }
local cityOpsBefore = #host.cityOps
local kept, keepWhy = applyOrder(player, PID,
	{ kind = "produce", subject = 42, verb = "UNIT_SCOUT" }, 10)
check("opening settler rejects a replacement", kept, false)
check("opening settler names the refusal", keepWhy, "opening_settler_in_progress")
check("opening settler leaves the host queue alone", #host.cityOps, cityOpsBefore)
check("opening settler remains queued", host.cities[42].current, 101)
local preserved = lastEvent("opening_settler_preserved")
check("opening settler preservation names the city", field(preserved, "city"), 42)
check("opening settler preservation names the requested item", field(preserved, "requested"), "UNIT_SCOUT")

-- Founding city two while the already-protected pipeline Settler is still in
-- the capital must not release it.  The live board changes city count between
-- the two requests, which used to replace the second Settler with a Warrior.
host.cities[43] = { id = 43, x = 8, y = 8, current = 0 }

local pipelineHeld, pipelineWhy = applyOrder(player, PID,
	{ kind = "produce", subject = 42, verb = "UNIT_SCOUT" }, 11)
check("pipeline settler survives the first founding", pipelineHeld, false)
check("pipeline settler names the refusal", pipelineWhy, "opening_settler_in_progress")
check("pipeline settler leaves the host queue alone", #host.cityOps, cityOpsBefore)
check("pipeline settler remains queued", host.cities[42].current, 101)

-- The lock belongs only to the Settler that was already in progress.  Once
-- the host has changed that queue away, a later two-city Settler can still be
-- replaced normally.
host.cities[42].current = 102
local cleared, clearWhy = applyOrder(player, PID,
	{ kind = "produce", subject = 42, verb = "UNIT_SCOUT" }, 12)
check("completed pipeline clears its lock", cleared, true)
check("completed pipeline uses its current queue", clearWhy, "already_building")

host.cities[42].current = 101
local replaced, replaceWhy = applyOrder(player, PID,
	{ kind = "produce", subject = 42, verb = "UNIT_SCOUT" }, 13)
check("later settler can be replaced", replaced, true)
check("later replacement reaches the host", replaceWhy, "UNIT_SCOUT")
check("later replacement changes the queue", host.cities[42].current, 102)

-- A pipeline Setter can also start on a quiet one-city frame before any
-- competing request has observed it.  That successful host start must seed
-- the same lock for the first-city founding handoff.
host.cities[43] = nil
host.cities[42].current = 0
local started, startedWhy = applyOrder(player, PID,
	{ kind = "produce", subject = 42, verb = "UNIT_SETTLER" }, 14)
check("one-city pipeline settler starts", started, true)
check("one-city pipeline names its start", startedWhy, "UNIT_SETTLER")
check("one-city pipeline reaches the host", host.cities[42].current, 101)

host.cities[43] = { id = 43, x = 8, y = 8, current = 0 }
local startedPipelineHeld, startedPipelineWhy = applyOrder(player, PID,
	{ kind = "produce", subject = 42, verb = "UNIT_SCOUT" }, 15)
check("started pipeline survives the first founding", startedPipelineHeld, false)
check("started pipeline names the refusal", startedPipelineWhy,
	"opening_settler_in_progress")
check("started pipeline remains queued", host.cities[42].current, 101)

-- An explicit military response must reach the host even with a protected
-- opening Settler, including after founding the first expansion city.
local defended, defenseWhy = applyOrder(player, PID,
	{ kind = "produce", subject = 42, verb = "UNIT_ARCHER" }, 16)
check("opening defense reaches host", defended, true)
check("opening defense names its build", defenseWhy, "UNIT_ARCHER")
check("opening defense replaces settler", host.cities[42].current, 103)

-- Reproduce the stalled one-city opening: two live Settlers are already
-- waiting for routes. The bridge must let the governor stop a third queue.
host.cities[43] = nil
host.cities[42].current = 101
host.units[501] = { id = 501, kind = "UNIT_SETTLER", x = 5, y = 5, moves = 2 }
host.units[502] = { id = 502, kind = "UNIT_SETTLER", x = 6, y = 6, moves = 2 }
local backlog, backlogWhy = applyOrder(player, PID,
	{ kind = "produce", subject = 42, verb = "UNIT_SCOUT" }, 17)
check("settler backlog releases queue", backlog, true)
check("backlog replacement names its build", backlogWhy, "UNIT_SCOUT")
check("backlog replacement reaches host", host.cities[42].current, 102)

-- A single walking Settler still permits the quiet second expansion pipeline.
host.units[502] = nil
host.cities[42].current = 101
local smallPipeline, smallWhy = applyOrder(player, PID,
	{ kind = "produce", subject = 42, verb = "UNIT_SCOUT" }, 18)
check("small quiet pipeline stays committed", smallPipeline, false)
check("small quiet pipeline refusal", smallWhy, "opening_settler_in_progress")

-- Host siege evidence also releases the lock when CivVis requested an
-- economic item; the emergency path must be reachable before the lock return.
DefenseTypes = { DISTRICT_GARRISON = 1, DISTRICT_OUTER = 2 }
Map.GetPlot = function() return {} end
CityManager.GetDistrictAt = function()
	return {
		GetDefenseStrength = function() return 13 end,
		GetDamage = function(_, kind) return kind == 1 and 179 or 0 end,
		GetMaxDamage = function(_, kind) return kind == 1 and 200 or 0 end,
	}
end
local siege, siegeWhy = applyOrder(player, PID,
	{ kind = "produce", subject = 42, verb = "UNIT_SCOUT" }, 19)
check("damaged capital releases opening lock", siege, true)
check("unavailable walls preserve CivVis request", siegeWhy, "UNIT_SCOUT")
check("siege request reaches host", host.cities[42].current, 102)

-- Native production lists and start-now checks are separate. Walls before
-- Masonry are excluded even when the second predicate alone says true.
GameInfo.Types.BUILDING_WALLS = { Hash = 104, Type = "BUILDING_WALLS", Kind = "KIND_BUILDING" }
GameInfo.Buildings = { BUILDING_WALLS = { IsWonder = false } }
host.cities[42].current = 101
host.cities[42].excluded = { [104] = true }
local preMasonry, preMasonryWhy = applyOrder(player, PID,
	{ kind = "produce", subject = 42, verb = "UNIT_ARCHER" }, 20)
check("unresearched walls do not replace defense", preMasonry, true)
check("unresearched walls keep requested defense", preMasonryWhy, "UNIT_ARCHER")
check("requested defense reaches host before Masonry", host.cities[42].current, 103)

-- A listed but disabled wall is equally ineligible.
host.cities[42].excluded = nil
host.cities[42].unstartable = { [104] = true }
host.cities[42].current = 101
local disabled, disabledWhy = applyOrder(player, PID,
	{ kind = "produce", subject = 42, verb = "UNIT_ARCHER" }, 21)
check("disabled walls keep requested defense", disabled, true)
check("disabled walls name requested defense", disabledWhy, "UNIT_ARCHER")
check("disabled walls do not reach host", host.cities[42].current, 103)

-- Once both native predicates admit the wall, the existing emergency works.
host.cities[42].unstartable = nil
host.cities[42].current = 101
local wall, wallWhy = applyOrder(player, PID,
	{ kind = "produce", subject = 42, verb = "UNIT_ARCHER" }, 22)
check("eligible emergency walls start", wall, true)
check("eligible emergency names wall", wallWhy, "BUILDING_WALLS")
check("eligible emergency wall reaches host", host.cities[42].current, 104)

-- An explicitly requested excluded item must not receive applied credit.
host.cities[42].current = 103
host.cities[42].excluded = { [104] = true }
local beforeExcluded = #host.cityOps
local excluded, excludedWhy = applyOrder(player, PID,
	{ kind = "produce", subject = 42, verb = "BUILDING_WALLS" }, 23)
check("excluded explicit build refused", excluded, false)
check("excluded explicit build reason", excludedWhy, "cannot_start_BUILDING_WALLS")
check("excluded explicit build never reaches host", #host.cityOps, beforeExcluded)

-- CivVis owns the siege decision in its mode, even when Walls are legal.
CivvisControlConfig.CivvisDecides = true
host.cities[42].excluded = nil
host.cities[42].current = 101
local civvisDefense, civvisDefenseWhy = applyOrder(player, PID,
	{ kind = "produce", subject = 42, verb = "UNIT_ARCHER" }, 24)
check("CivVis siege choice is accepted", civvisDefense, true)
check("CivVis siege choice is not replaced by Walls", civvisDefenseWhy, "UNIT_ARCHER")
check("CivVis siege choice reaches host", host.cities[42].current, 103)

-- The bridge also cannot silently keep a finishing defender when CivVis
-- explicitly chooses Walls. The legacy standalone controller may retain it.
host.cities[42].turns = 1
local civvisWalls, civvisWallsWhy = applyOrder(player, PID,
	{ kind = "produce", subject = 42, verb = "BUILDING_WALLS" }, 25)
check("CivVis can replace a finishing defender", civvisWalls, true)
check("CivVis chosen wall is respected", civvisWallsWhy, "BUILDING_WALLS")
check("CivVis chosen wall reaches host", host.cities[42].current, 104)

CivvisControlConfig.CivvisDecides = false
host.cities[42].current = 103
local legacyDefender, legacyDefenderWhy = applyOrder(player, PID,
	{ kind = "produce", subject = 42, verb = "BUILDING_WALLS" }, 26)
check("standalone controller preserves finishing defender", legacyDefender, true)
check("standalone preservation reason", legacyDefenderWhy, "finishing_defender_preserved")
check("standalone finishing defender remains queued", host.cities[42].current, 103)

if failures > 0 then
	print(string.format("%d failure(s)", failures))
	os.exit(1)
end
print("all order verdict checks passed")
