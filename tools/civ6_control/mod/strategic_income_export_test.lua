-- Execute the production export against the shipped TopPanel income contract.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local expression = assert(source:match("strategic_resource_income = (try%(function%(%).-\n\t\tend, nil%)),"))
local rows = {
 { ResourceType = "RESOURCE_ALUMINUM", ResourceClassType = "RESOURCECLASS_STRATEGIC" },
 { ResourceType = "RESOURCE_IRON", ResourceClassType = "RESOURCECLASS_STRATEGIC" },
 { ResourceType = "RESOURCE_SILK", ResourceClassType = "RESOURCECLASS_LUXURY" },
}
local resources, imports, bonus = {}, 2, 1
resources.GetResourceAccumulationPerTurn = function(_, name)
 assert(name ~= "RESOURCE_SILK")
 return name == "RESOURCE_ALUMINUM" and 1 or 0
end
resources.GetResourceImportPerTurn = function(_, name) return name == "RESOURCE_ALUMINUM" and imports or 0 end
resources.GetBonusResourcePerTurn = function(_, name) return name == "RESOURCE_ALUMINUM" and bonus or 0 end
resources.GetUnitResourceDemandPerTurn = function() error("gross income must not subtract demand") end
resources.GetPowerResourceDemandPerTurn = resources.GetUnitResourceDemandPerTurn
local env = setmetatable({
 try = function(fn, fallback) local ok, value = pcall(fn); if ok then return value end; return fallback end,
 player = { GetResources = function() return resources end },
 GameInfo = { Resources = function()
  local i = 0; return function() i = i + 1; return rows[i] end
 end },
}, { __index = _G })
local export = assert(loadstring("return " .. expression)); setfenv(export, env)
local observed = export()
assert(observed.RESOURCE_ALUMINUM == 4, "income includes imports and bonuses")
assert(observed.RESOURCE_IRON == 0, "zero is authoritative, not absent")
assert(observed.RESOURCE_SILK == nil)
resources.GetBonusResourcePerTurn = nil
assert(export() == nil, "unsupported API must retain legacy fallback")
resources.GetBonusResourcePerTurn = function(_, name) if name == "RESOURCE_ALUMINUM" then error("unavailable") end; return 0 end
observed = export()
assert(observed.RESOURCE_ALUMINUM == nil and observed.RESOURCE_IRON == 0, "partial API failure is not zero income")
resources = nil
assert(export() == nil)
print("strategic income export checks passed")
