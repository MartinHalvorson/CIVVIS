local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local section = assert(source:match("(CivvisTiles = .-)\n%-%- ★★★★★ CHANNEL PROBE"))
local visible, pillaged, reads, improvement = true, false, 0, -1
local events = {}
local district = {
 IsComplete = function() return true end,
 IsPillaged = function() reads = reads + 1; return pillaged end,
}
local plot = setmetatable({
 GetDistrictType = function() return 1 end,
 GetImprovementType = function() return improvement end,
 IsImprovementPillaged = function() return pillaged end,
 GetOwner = function() return 2 end,
 IsWater = function() return false end,
 IsImpassable = function() return false end,
 IsFreshWater = function() return false end,
}, { __index = function() return function() return -1 end end })
local env = setmetatable({
 cfg = { ExportState = true, TileYields = false },
 try = function(fn, fallback) local ok, value = pcall(fn); if ok then return value end; return fallback end,
 Map = { GetGridSize = function() return 1, 1 end, GetPlot = function() return plot end },
 CityManager = { GetDistrictAt = function() return district end },
 PlayersVisibility = { [0] = { IsVisible = function() return visible end } },
 GameInfo = { Districts = { [1] = { DistrictType = "DISTRICT_CAMPUS" } } },
 plotRevealed = function() return true end,
 visibleResourceName = function() return nil end,
 riverMask = function() return 0 end,
 typeName = function(group) if group == "Terrains" then return "TERRAIN_GRASS" end end,
 emit = function(kind, row) if kind == "tiles" then events[#events + 1] = row end end,
}, { __index = _G })
local chunk = assert(loadstring(section)); setfenv(chunk, env); chunk()
local function sweep()
 events = {}
 local fresh = env.CivvisTiles.sweep({}, 0, 200, 0)
 return fresh, events[1] and events[1].plots[1]
end
local fresh, row = sweep()
assert(fresh == 1 and row.d == "DISTRICT_CAMPUS" and row.p == false)
pillaged = true
fresh, row = sweep()
assert(fresh == 1 and row.p == true, "pillage alone must trigger a delta and cross with the plot")
assert(sweep() == 0, "unchanged pillage does not resend the plot")
visible = false
local before = reads
fresh, row = sweep()
assert(fresh == 1 and row.p == true)
pillaged = false
assert(sweep() == 0 and reads == before, "repair outside sight must not leak into the board")
visible = true
fresh, row = sweep()
assert(fresh == 1 and row.p == false, "observed repair clears the pillage bit")
improvement = 0; pillaged = true
assert(env.CivvisTiles.pillageState(plot, 0, 0, 0) == true, "improvement pillage remains supported")
improvement = -1; visible = false
assert(env.CivvisTiles.pillageState(plot, 0, 9, 9) == nil, "unseen district has no invented observation")
visible = true; pillaged = true
assert(env.CivvisTiles.pillageState(plot, 0, 0, 0) == true)
district.IsPillaged = nil
assert(env.CivvisTiles.pillageState(plot, 0, 0, 0) == true, "failed read preserves the last observation")
assert(env.CivvisTiles.pillageState(plot, 0, 9, 9) == nil, "missing accessor is tolerated")
print("enemy district pillage export, delta, repair and fog checks passed")
