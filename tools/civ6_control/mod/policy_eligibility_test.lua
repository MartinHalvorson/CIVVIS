local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua", "r"))
local source = file:read("*a"); file:close()
local start = assert(source:find("CivvisPolicy.isAvailable = function", 1, true))
local finish = assert(source:find("-- Per-city production", start, true))
CivvisPolicy = {}
assert(loadstring(source:sub(start, finish - 1)))()
local rows = {
	{ Hash = 1, PolicyType = "POLICY_Z" },
	{ Hash = 2, PolicyType = "POLICY_NEW_DEAL" },
	{ Hash = 3, PolicyType = "POLICY_A" },
}
GameInfo = { Policies = function()
	local i = 0
	return function() i = i + 1; return rows[i] end
end }
local culture = {
	IsPolicyBanned = function(_, h) return h == 1 end,
	CanPolicyBeSlotted = function(_, h) return h ~= 2 end,
	IsPolicyObsolete = function() return false end,
}
local names = CivvisPolicy.availableNames(culture)
assert(#names == 1 and names[1] == "POLICY_A", "export must exclude banned and unslottable cards")
culture.IsPolicyBanned = function() return false end
culture.CanPolicyBeSlotted = function() return true end
names = CivvisPolicy.availableNames(culture)
assert(table.concat(names, ",") == "POLICY_A,POLICY_NEW_DEAL,POLICY_Z", "eligibility updates and is sorted")
culture.IsPolicyObsolete = function() return true end
assert(#CivvisPolicy.availableNames(culture) == 0, "known empty is an empty list")
culture.CanPolicyBeSlotted = nil
assert(CivvisPolicy.isAvailable(culture, 1) == nil, "missing API is unknown")
assert(CivvisPolicy.availableNames(culture) == nil, "do not publish a partial slate")
culture.CanPolicyBeSlotted = function(_, h) if h == 2 then error("unavailable") end; return true end
assert(CivvisPolicy.availableNames(culture) == nil, "mid-enumeration failure is unknown")
assert(source:find("available_policies = CivvisPolicy.availableNames(pcult)", 1, true), "wire eligibility into state export")
print("ok native policy eligibility export")
