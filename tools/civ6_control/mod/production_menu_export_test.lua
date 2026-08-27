-- The host's production and purchase menus the mod exports per city, and the
-- ways each could be silently wrong.
--
-- ⚠ Loads the SHIPPED `CivvisControlAgent.lua` and calls the agent's own
-- `CivvisMenus.buildable` / `purchasable` / `queue`. A test that re-implemented
-- the loops would pass while the agent kept asking the wrong question.
--
-- What is checked, against a fake city whose build queue answers like the
-- engine's (`ProductionPanel.lua`: the exclusion test lists, the start test
-- decides, the typed cost accessors take `row.Index`):
--   1. only what can be STARTED crosses -- a listed-but-disabled Spearman and
--      an excluded Walls do not, a MustPurchase Palace and an InternalOnly
--      district are never asked;
--   2. a Corps row crosses with its own cost and tier only when the results
--      table says the city may train one, and no Army row when it may not;
--   3. a district carries the engine's plot offer and its count;
--   4. purchases carry the engine's price per currency, Faith only where the
--      row or the city enables it;
--   5. the queue behind the head crosses with type, tier and progress;
--   6. a queue that raises exports an EMPTY menu (the mirror treats that as
--      unknown), a missing queue or directive table exports nil -- never a
--      menu that claims the city can build nothing.
--
-- Run: lua5.1 tools/civ6_control/mod/production_menu_export_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end

-- Globals the test hides on purpose read as nil, everything else unknown is a
-- stub -- so "this build has no such table" is reachable at all.
local hidden = {}
setmetatable(_G, { __index = function(_, k)
	if hidden[k] then return nil end
	return stub()
end })

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtime_err = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtime_err))

local menus = rawget(_G, "CivvisMenus")
assert(type(menus) == "table", "CivvisControlAgent.lua did not export CivvisMenus")

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", name, tostring(got)))
	end
end

-- ------------------------------------------------------------------ the host
-- A GameInfo table that iterates like the engine's AND indexes by type or index.
local function gameInfoTable(rows)
	local byKey = {}
	for _, row in ipairs(rows) do
		byKey[row.Index] = row
		byKey[row.Hash] = row
		byKey[row.UnitType or row.BuildingType or row.DistrictType or row.ProjectType] = row
	end
	return setmetatable({}, {
		__call = function()
			local i = 0
			return function()
				i = i + 1
				return rows[i]
			end
		end,
		__index = function(_, k) return byKey[k] end,
	})
end

GameInfo = {
	Districts = gameInfoTable({
		{ DistrictType = "DISTRICT_CAMPUS", Hash = 101, Index = 1 },
		{ DistrictType = "DISTRICT_HOLY_SITE", Hash = 102, Index = 2 },
		{ DistrictType = "DISTRICT_WONDER", Hash = 103, Index = 3, InternalOnly = true },
	}),
	Buildings = gameInfoTable({
		{ BuildingType = "BUILDING_MONUMENT", Hash = 201, Index = 1, PurchaseYield = "YIELD_GOLD" },
		{ BuildingType = "BUILDING_GRANARY", Hash = 202, Index = 2, PurchaseYield = "YIELD_GOLD" },
		{ BuildingType = "BUILDING_WALLS", Hash = 203, Index = 3 },
		{ BuildingType = "BUILDING_PALACE", Hash = 204, Index = 4, MustPurchase = true },
	}),
	Units = gameInfoTable({
		{ UnitType = "UNIT_WARRIOR", Hash = 301, Index = 1, PurchaseYield = "YIELD_GOLD" },
		{ UnitType = "UNIT_SPEARMAN", Hash = 302, Index = 2, PurchaseYield = "YIELD_GOLD" },
		{ UnitType = "UNIT_MISSIONARY", Hash = 303, Index = 3, PurchaseYield = "YIELD_FAITH" },
	}),
	Projects = gameInfoTable({
		{ ProjectType = "PROJECT_ENHANCE_DISTRICT_CAMPUS", Hash = 401, Index = 1 },
	}),
	Yields = { YIELD_GOLD = { Index = 1 }, YIELD_FAITH = { Index = 5 } },
}
MilitaryFormationTypes = {
	STANDARD_MILITARY_FORMATION = 0,
	CORPS_MILITARY_FORMATION = 1,
	ARMY_MILITARY_FORMATION = 2,
}
CityOperationTypes = { BUILD = "BUILD", PARAM_DISTRICT_TYPE = "PARAM_DISTRICT_TYPE" }
CityOperationResults = { PLOTS = "PLOTS", CAN_TRAIN_CORPS = "CAN_TRAIN_CORPS", CAN_TRAIN_ARMY = "CAN_TRAIN_ARMY" }
CityCommandTypes = {
	PURCHASE = "PURCHASE",
	PARAM_UNIT_TYPE = "PARAM_UNIT_TYPE",
	PARAM_BUILDING_TYPE = "PARAM_BUILDING_TYPE",
	PARAM_DISTRICT_TYPE = "PARAM_DISTRICT_TYPE",
	PARAM_YIELD_TYPE = "PARAM_YIELD_TYPE",
}
CityProductionDirectives = { TRAIN = 1, CONSTRUCT = 2, ZONE = 3, PROJECT = 4 }
Map = { GetPlotByIndex = function(i)
	return { GetX = function() return i end, GetY = function() return i + 1 end }
end }
CityManager = {
	GetOperationTargets = function(city, op, probe)
		if op == "BUILD" and probe.PARAM_DISTRICT_TYPE == 101 then
			return { PLOTS = { 11, 12 } }
		end
		return {}
	end,
	CanStartCommand = function(city, command, exclusion, params, wantResults)
		local hash = params.PARAM_UNIT_TYPE or params.PARAM_BUILDING_TYPE or params.PARAM_DISTRICT_TYPE
		if command ~= "PURCHASE" then return false end
		-- The exclusion test lists everything but Walls; the verdict refuses
		-- the Spearman and any district.
		if exclusion == true then return hash ~= 203 end
		if hash == 302 or hash == 101 or hash == 102 then
			return false, { FAILURE_REASONS = { "LOC_TEST_REFUSED" } }
		end
		return true, {}
	end,
}

local function fakeQueue(behaviour)
	behaviour = behaviour or {}
	local queue = {}
	function queue:CanProduce(arg, exclusion, wantResults)
		if behaviour.raise then error("no such method") end
		local hash = type(arg) == "table" and arg.UnitType or arg
		local tier = type(arg) == "table" and arg.MilitaryFormationType or 0
		if exclusion == true then return hash ~= 203 end
		if hash == 302 then return false, { FAILURE_REASONS = { "LOC_TEST_NEEDS_IRON" } } end
		if tier == 2 then return false, nil end
		if hash == 301 and tier == 0 then
			return true, { CAN_TRAIN_CORPS = true, CAN_TRAIN_ARMY = true }
		end
		return true, {}
	end
	function queue:GetDistrictCost(index) return 50 + index end
	function queue:GetBuildingCost(index) return 60 * index end
	function queue:GetUnitCost(index) return 40 * index end
	function queue:GetUnitCorpsCost(index) return 100 + index end
	function queue:GetUnitArmyCost(index) return 200 + index end
	function queue:GetProjectCost(index) return 15 * index end
	function queue:GetTurnsLeft(key, tier) if tier ~= nil then return 9 end return 4 end
	function queue:GetSize() return 3 end
	function queue:GetAt(i)
		if i == 1 then return { Directive = 1, UnitType = 2, MilitaryFormationType = 1 } end
		if i == 2 then return { Directive = 2, BuildingType = 2 } end
		return nil
	end
	function queue:GetUnitProgress(index) return 7 end
	function queue:GetBuildingProgress(index) return 0 end
	function queue:GetDistrictProgress(index) return 0 end
	function queue:GetProjectProgress(index) return 0 end
	return queue
end

local wallet = {
	GetPurchaseCost = function(self, currency, hash)
		if currency == 1 then return hash * 2 end
		return hash
	end,
	IsUnitFaithPurchaseEnabled = function(self, hash) return hash == 301 end,
	IsBuildingFaithPurchaseEnabled = function(self, hash) return false end,
}

local function fakeCity(queue)
	return {
		GetBuildQueue = function() return queue end,
		GetGold = function() return wallet end,
		GetID = function() return 7 end,
	}
end

-- Buildable rows: by type AND tier (`f` is the formation there).
local function find(list, t, f)
	for _, row in ipairs(list or {}) do
		if row.t == t and row.f == f then return row end
	end
	return nil
end

-- Purchase rows: by type only (`f` is the Faith price there).
local function findType(list, t)
	for _, row in ipairs(list or {}) do
		if row.t == t then return row end
	end
	return nil
end

-- --------------------------------------------------------------- buildable
local buildable = menus.buildable(fakeCity(fakeQueue()))
check("buildable is a list", type(buildable), "table")
check("a startable unit crosses", find(buildable, "UNIT_WARRIOR") ~= nil, true)
check("…with the engine's cost", (find(buildable, "UNIT_WARRIOR") or {}).c, 40)
check("…and its turns", (find(buildable, "UNIT_WARRIOR") or {}).p, 4)
check("a listed-but-disabled Spearman does not", find(buildable, "UNIT_SPEARMAN"), nil)
check("an excluded Walls does not", find(buildable, "BUILDING_WALLS"), nil)
check("a MustPurchase Palace does not", find(buildable, "BUILDING_PALACE"), nil)
check("an InternalOnly district does not", find(buildable, "DISTRICT_WONDER"), nil)
check("a Corps row crosses when the results say so", find(buildable, "UNIT_WARRIOR", 1) ~= nil, true)
check("…with the corps cost", (find(buildable, "UNIT_WARRIOR", 1) or {}).c, 101)
check("…and the tier's turns", (find(buildable, "UNIT_WARRIOR", 1) or {}).p, 9)
check("no Army row when the tier cannot start", find(buildable, "UNIT_WARRIOR", 2), nil)
local campus = find(buildable, "DISTRICT_CAMPUS") or {}
check("a district crosses", campus.t, "DISTRICT_CAMPUS")
check("…with the plot count", campus.n, 2)
check("…and the plots", campus.s and #campus.s, 2)
check("…as offset x", campus.s and campus.s[1].x, 11)
check("…and offset y", campus.s and campus.s[1].y, 12)
check("a project crosses", (find(buildable, "PROJECT_ENHANCE_DISTRICT_CAMPUS") or {}).c, 15)
check("a building crosses", (find(buildable, "BUILDING_GRANARY") or {}).c, 120)
local count = 0
for _ in ipairs(buildable) do count = count + 1 end
check("nothing else crosses", count, 8)

-- ------------------------------------------------------------- purchasable
local purchasable = menus.purchasable(fakeCity(fakeQueue()))
check("purchasable is a list", type(purchasable), "table")
local granary = findType(purchasable, "BUILDING_GRANARY") or {}
check("a purchasable building carries the Gold price", granary.g, 404)
check("…and no Faith price when the row does not enable it", granary.f, nil)
local warrior = findType(purchasable, "UNIT_WARRIOR") or {}
check("a unit carries the Gold price", warrior.g, 602)
check("…and the Faith price when the city enables it", warrior.f, 301)
local missionary = findType(purchasable, "UNIT_MISSIONARY") or {}
check("a Faith-only unit carries no Gold price", missionary.g, nil)
check("…and its Faith price", missionary.f, 303)
check("a refused purchase does not cross", findType(purchasable, "UNIT_SPEARMAN"), nil)
check("an excluded one does not either", findType(purchasable, "BUILDING_WALLS"), nil)
check("a refused district does not", findType(purchasable, "DISTRICT_CAMPUS"), nil)

-- ------------------------------------------------------------------- queue
local queued = menus.queue(fakeCity(fakeQueue()))
check("the queue behind the head crosses", queued and #queued, 2)
check("…first the Spearman Corps", queued and queued[1].t, "UNIT_SPEARMAN")
check("…with its tier", queued and queued[1].f, 1)
check("…and its progress", queued and queued[1].pr, 7)
check("…then the Granary", queued and queued[2].t, "BUILDING_GRANARY")
check("…untiered", queued and queued[2].f, nil)

-- ------------------------------------------------------------- robustness
local raising = menus.buildable(fakeCity(fakeQueue({ raise = true })))
check("a queue that raises exports an empty menu, not a claim", type(raising), "table")
check("…empty", raising and #raising, 0)
check("no build queue exports nil", menus.buildable(fakeCity(nil)), nil)
check("no build queue exports nil for the queue either", menus.queue(fakeCity(nil)), nil)
local savedDirectives = CityProductionDirectives
hidden.CityProductionDirectives = true
CityProductionDirectives = nil
check("no directive table exports nil for the queue", menus.queue(fakeCity(fakeQueue())), nil)
hidden.CityProductionDirectives = nil
CityProductionDirectives = savedDirectives

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("\nall production-menu export checks passed")
