-- Read both host API returns through the actual shipped control agent.
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
GameInfo = { Resources = { [7] = { ResourceType = "RESOURCE_IRON" } } }
local function quote(resource, cost)
	return CivvisMenus.upgrade_resource_cost({ GetUpgradeResourceCost = function()
		return resource, cost
	end })
end
local bill = quote(7, 10)
assert(bill.resource == "RESOURCE_IRON" and bill.cost == 10,
	"the first return is a resource Index; the second is the complete bill")
bill = quote(7, 18)
assert(bill.cost == 18, "do not multiply an already priced formation")
bill = quote(7, 0)
assert(bill.resource == "RESOURCE_IRON" and bill.cost == 0)
bill = quote(-1, 0)
assert(bill.resource == nil and bill.cost == 0, "explicitly no material is free")
for _, invalid in ipairs({ -1, math.huge, 0 / 0, "10", {} }) do
	assert(quote(7, invalid) == nil)
end
for _, invalid in ipairs({ -2, math.huge, 0 / 0, 7.5, "7", {}, 701 }) do
	assert(quote(invalid, 10) == nil)
end
assert(quote(-1, 10) == nil)
assert(quote(nil, 0) == nil)
assert(quote(701, 0) == nil, "an unknown resource does not mean free")
assert(CivvisMenus.upgrade_resource_cost({}) == nil)
assert(CivvisMenus.upgrade_resource_cost({ GetUpgradeResourceCost = function()
	error("unavailable")
end }) == nil)
GameInfo.Resources[7].ResourceType = ""
assert(quote(7, 10) == nil)
print("native upgrade resource bill: PASS")
