-- Execute the actual tile sweep: creating a park alone must publish its flag.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local section = assert(source:match("(CivvisTiles = .-)\n%-%- ★★★★★ CHANNEL PROBE"))
local park, revealed, unreadable = false, true, false
local events = {}
local plot = setmetatable({
 GetX = function() return 0 end, GetY = function() return 0 end,
 GetDistrictType = function() return -1 end,
 GetImprovementType = function() return -1 end,
 GetOwner = function() return 0 end,
 GetAppeal = function() return 4 end,
 IsWater = function() return false end,
 IsNationalPark = function()
  if unreadable then error("park accessor unavailable") end
  return park
 end,
}, { __index = function() return function() return -1 end end })
local env = setmetatable({
 cfg = { ExportState = true, TileYields = false },
 try = function(fn, fallback) local ok, value = pcall(fn); if ok then return value end; return fallback end,
 Map = { GetGridSize = function() return 1, 1 end, GetPlot = function() return plot end },
 PlayersVisibility = { [0] = { IsVisible = function() return true end } },
 plotRevealed = function() return revealed end,
 visibleResourceName = function() return nil end,
 riverMask = function() return 0 end,
 typeName = function(group) if group == "Terrains" then return "TERRAIN_GRASS" end end,
 emit = function(kind, row) if kind == "tiles" then events[#events+1] = row end end,
}, { __index = _G })
local chunk = assert(loadstring(section)); setfenv(chunk, env); chunk()
local function sweep(frame)
 events = {}
 local fresh = env.CivvisTiles.sweep({}, 0, 136, frame or 0)
 return fresh, events[1] and events[1].plots[1]
end
local fresh, row = sweep()
assert(fresh == 1 and row.np == nil and row.im == nil and row.ap == 4)
assert(sweep() == 0, "unchanged ground does not resend")
park = true
fresh, row = sweep(1)
assert(fresh == 1 and row.np == true, "park creation alone must publish a same-turn delta")
assert(row.im == nil and row.o == 0 and row.ap == 4, "a park is independent of improvement, owner and appeal")
assert(sweep(2) == 0, "an unchanged park does not resend")
park = false
fresh, row = sweep(3)
assert(fresh == 1 and row.np == nil, "a cleared park flag must invalidate too")

revealed = false; park = true
assert(sweep() == 0, "unrevealed ground is never published")
revealed = true
fresh, row = sweep()
assert(fresh == 1 and row.np == true, "revealed park state reaches the next sweep")
unreadable = true
fresh, row = sweep()
assert(fresh == 1 and row.np == nil, "failed reads do not invent a park")
assert(sweep() == 0, "repeated failed reads do not flood the export")
print("national park delta: creation, removal, same-turn, unchanged, reveal and failed-read checks passed")
