-- Execute the seat's luxury-count export: the host's own count of each
-- luxury, net of copies traded away, the barter's last-copy reading.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local expression = assert(source:match("luxury_counts = (try%(function%(%).-\n\t\tend, nil%)),"))
local rows = {
 { ResourceType = "RESOURCE_MARBLE", ResourceClassType = "RESOURCECLASS_LUXURY" },
 { ResourceType = "RESOURCE_COFFEE", ResourceClassType = "RESOURCECLASS_LUXURY" },
 { ResourceType = "RESOURCE_JADE", ResourceClassType = "RESOURCECLASS_LUXURY" },
 { ResourceType = "RESOURCE_IRON", ResourceClassType = "RESOURCECLASS_STRATEGIC" },
}
-- Two Marble tiles with one copy traded away: the host counts one.
local held = { RESOURCE_MARBLE = 1, RESOURCE_COFFEE = 2, RESOURCE_JADE = 0, RESOURCE_IRON = 30 }
local resources = {}
resources.GetResourceAmount = function(_, name) return held[name] end
local env = setmetatable({
 try = function(fn, fallback) local ok, value = pcall(fn); if ok then return value end; return fallback end,
 player = { GetResources = function() return resources end },
 GameInfo = { Resources = function()
  local i = 0; return function() i = i + 1; return rows[i] end
 end },
}, { __index = _G })
local export = assert(loadstring("return " .. expression)); setfenv(export, env)
local observed = export()
assert(observed.RESOURCE_MARBLE == 1, "the host's net count, not the tiles")
assert(observed.RESOURCE_COFFEE == 2)
assert(observed.RESOURCE_JADE == nil, "none held is absent")
assert(observed.RESOURCE_IRON == nil, "strategics are exported elsewhere")
held = { RESOURCE_IRON = 30 }
assert(export() == nil, "no luxury held is nil, not an empty table")
resources = nil
assert(export() == nil)
print("luxury count export checks passed")
