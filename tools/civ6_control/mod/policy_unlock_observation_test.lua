local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua", "r"))
local source = file:read("*a"); file:close()
local start = assert(source:find("CivvisPolicy.unlockObservation = function", 1, true))
local finish = assert(source:find("-- Per-city production", start, true))
CivvisPolicy = {}
assert(loadstring(source:sub(start, finish - 1)))()

local cost, made = 0, false
local culture = {
	GetCostToUnlockPolicies = function() return cost end,
	PolicyChangeMade = function() return made end,
}
GlobalParameters = { GOVERNMENT_UNLOCK_WITH_FAITH = 0 }
local observed = CivvisPolicy.unlockObservation(culture)
assert(observed.unlock_cost == 0 and observed.made == false and observed.currency == "gold",
	"observe a zero cost and false flag, rather than treating them as missing")

cost, made = 125, true
GlobalParameters.GOVERNMENT_UNLOCK_WITH_FAITH = 1
observed = CivvisPolicy.unlockObservation(culture)
assert(observed.unlock_cost == 125 and observed.made == true and observed.currency == "faith",
	"each export reads current values and the shipped currency parameter")

GlobalParameters = nil
culture.GetCostToUnlockPolicies = function() error("missing cost API") end
observed = CivvisPolicy.unlockObservation(culture)
assert(observed.unlock_cost == nil and observed.made == true and observed.currency == nil,
	"a failed cost read must not invent a free swap or erase the known flag")
culture.PolicyChangeMade = nil
assert(CivvisPolicy.unlockObservation(culture) == nil, "all missing observations stay unknown")
assert(CivvisPolicy.unlockObservation(nil) == nil, "missing culture is unknown")

culture.GetCostToUnlockPolicies = function() return -1 end
observed = CivvisPolicy.unlockObservation(culture)
assert(observed.unlock_cost == -1, "retain a finite raw sentinel without interpreting it")
for _, bad in ipairs({ "0", math.huge, -math.huge, 0/0 }) do
	culture.GetCostToUnlockPolicies = function() return bad end
	assert(CivvisPolicy.unlockObservation(culture) == nil, "invalid numeric cost is unknown")
end
culture.PolicyChangeMade = function() return 0 end
assert(CivvisPolicy.unlockObservation(culture) == nil, "numeric zero is not a native false flag")
GlobalParameters = { GOVERNMENT_UNLOCK_WITH_FAITH = "0" }
assert(CivvisPolicy.unlockObservation(culture) == nil, "unknown currency is not guessed")
GlobalParameters.GOVERNMENT_UNLOCK_WITH_FAITH = 0
observed = CivvisPolicy.unlockObservation(culture)
assert(observed.currency == "gold" and observed.unlock_cost == nil and observed.made == nil,
	"independent observations may be partial")
assert(source:find("policy_change = CivvisPolicy.unlockObservation(pcult)", 1, true),
	"wire the observation into the native state export")
assert(source:find("unit_maintenance_discount = try(function()", 1, true)
	and source:find("return player:GetTreasury():GetMaintDiscountPerUnit();", 1, true),
	"export the native per-unit discount alongside base maintenance bills")
print("ok native policy unlock observations")
