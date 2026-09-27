-- Exercise the shipped menu exporter against the host's city/tier prices.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end
setmetatable(_G, { __index = function() return stub() end })
assert(loadfile(here .. "/CivvisControlAgent.lua"))()

local function rows(values)
	return setmetatable({}, { __call = function()
		local n = 0
		return function() n = n + 1; return values[n] end
	end })
end
GameInfo = {
	Districts = rows({}), Buildings = rows({}), Projects = rows({}),
	Units = rows({ { Index = 7, Hash = 701, UnitType = "UNIT_BOMBARD" } }),
}
CityOperationResults = { CAN_TRAIN_CORPS = "corps", CAN_TRAIN_ARMY = "army" }
MilitaryFormationTypes = {
	STANDARD_MILITARY_FORMATION = 0, CORPS_MILITARY_FORMATION = 1,
	ARMY_MILITARY_FORMATION = 2,
}
local called = {}
local queue = {
	CanProduce = function() return true, { corps = true, army = true } end,
	GetUnitCost = function() return 140 end,
	GetUnitCorpsCost = function() return 210 end,
	GetUnitArmyCost = function() return 280 end,
	GetTurnsLeft = function() return 4 end,
	GetUnitResourceCost = function(_, index, tier)
		assert(index == 7, "resource accessor needs row.Index, not its hash")
		called[#called + 1] = tier
		return ({ [0] = 10, [1] = 18, [2] = 25 })[tier]
	end,
}
local menu = CivvisMenus.buildable({ GetBuildQueue = function() return queue end })
assert(#menu == 3 and #called == 3)
for i, tier in ipairs({ 0, 1, 2 }) do
	assert(called[i] == tier and menu[i].t == "UNIT_BOMBARD")
	assert(menu[i].r == ({ [0] = 10, [1] = 18, [2] = 25 })[tier])
end
assert(menu[1].f == nil and menu[2].f == 1 and menu[3].f == 2)

-- Unknown prices stay absent; free prices must survive as an explicit zero.
assert(CivvisMenus.resource_cost({}, 7, 0) == nil)
assert(CivvisMenus.resource_cost({ GetUnitResourceCost = function() error("missing") end }, 7, 0) == nil)
for _, invalid in ipairs({ -1, math.huge, 0 / 0, "10", {} }) do
	assert(CivvisMenus.resource_cost({ GetUnitResourceCost = function() return invalid end }, 7, 0) == nil)
end
assert(CivvisMenus.resource_cost({ GetUnitResourceCost = function() return 0 end }, 7, 0) == 0)

-- The gameplay VM may expose the alternate enum spellings.
MilitaryFormationTypes = { STANDARD_FORMATION = 0, CORPS_FORMATION = 1, ARMY_FORMATION = 2 }
called = {}
menu = CivvisMenus.buildable({ GetBuildQueue = function() return queue end })
assert(#menu == 3 and called[1] == 0 and called[2] == 1 and called[3] == 2)
print("native unit resource menu: PASS")
