-- Execute the shipped tile sweep: flooding alone must trigger a delta.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local section = assert(source:match("(CivvisTiles = .-)\n%-%- ★★★★★ CHANNEL PROBE"))
local flooded, submerged, revealed = false, false, true
local events = {}
local plot = setmetatable({
 GetDistrictType = function() return -1 end,
 GetImprovementType = function() return -1 end,
 GetOwner = function() return 0 end,
 IsWater = function() return false end,
}, { __index = function() return function() return -1 end end })
local env = setmetatable({
 cfg = { ExportState = true, TileYields = false },
 try = function(fn, fallback) local ok, value = pcall(fn); if ok then return value end; return fallback end,
 Map = { GetGridSize = function() return 1, 1 end, GetPlot = function() return plot end },
 TerrainManager = {
  GetCoastalLowlandType = function() return 1 end,
  IsFlooded = function(p) assert(p == plot); return flooded end,
  IsSubmerged = function(p) assert(p == plot); return submerged end,
 },
 PlayersVisibility = { [0] = { IsVisible = function() return true end } },
 plotRevealed = function() return revealed end,
 visibleResourceName = function() return nil end,
 riverMask = function() return 0 end,
 typeName = function(group) if group == "Terrains" then return "TERRAIN_GRASS" end end,
 emit = function(kind, row) if kind == "tiles" then events[#events + 1] = row end end,
}, { __index = _G })
local chunk = assert(loadstring(section)); setfenv(chunk, env); chunk()
local function sweep()
 events = {}
 local fresh = env.CivvisTiles.sweep({}, 0, 203, 0)
 return fresh, events[1] and events[1].plots[1]
end
local fresh, row = sweep()
assert(fresh == 1 and row.flooded == false and row.submerged == false)
assert(sweep() == 0, "unchanged dry ground does not resend")
flooded = true
fresh, row = sweep()
assert(fresh == 1 and row.flooded == true and row.submerged == false)
assert(sweep() == 0)
submerged = true
fresh, row = sweep()
assert(fresh == 1 and row.submerged == true, "submergence alone triggers a delta")
flooded, submerged = false, false
fresh, row = sweep()
assert(fresh == 1 and row.flooded == false and row.submerged == false, "dry flags are explicit")
for _, unknown in ipairs({ 0, "false", {} }) do
 flooded = unknown
 assert(env.CivvisTiles.floodState(plot, "IsFlooded") == nil)
end
env.TerrainManager.IsFlooded = function() error("unavailable") end
fresh, row = sweep()
assert(fresh == 1 and row.flooded == nil and row.submerged == false)
env.TerrainManager.IsSubmerged = nil
assert(env.CivvisTiles.floodState(plot, "IsSubmerged") == nil)
env.TerrainManager = nil
assert(env.CivvisTiles.floodState(plot, "IsFlooded") == nil)
revealed = false
assert(sweep() == 0, "unrevealed plots are never exported")
print("native flood state: dry, flooded, submerged, delta, unknown and reveal checks passed")
