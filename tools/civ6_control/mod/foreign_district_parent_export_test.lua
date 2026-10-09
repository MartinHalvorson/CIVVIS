local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local section = assert(source:match("(CivvisTiles = .-)\n%-%- ★★★★★ CHANNEL PROBE"))

local function fixture()
 local s = { visible = true, complete = true, owner = 2, kind = 1, id = 42,
             parent = 31, parent_owner = 2, parent_known = true, reads = 0, events = {} }
 local city = {
  GetID = function() return s.parent end,
  GetOwner = function() return s.parent_owner end,
  GetX = function() return 9 end, GetY = function() return 10 end,
 }
 local district = {
  GetID = function() return s.id end,
  IsComplete = function() return s.complete end,
  IsPillaged = function() return false end,
  GetCity = function()
   s.reads = s.reads + 1
   if s.fail then error("district parent unavailable") end
   if s.no_city then return nil end
   return city
  end,
 }
 local plot = setmetatable({
  GetX = function() return 0 end, GetY = function() return 0 end,
  GetDistrictType = function() return s.kind end,
  GetOwner = function() return s.owner end,
  IsWater = function() return false end,
  IsImpassable = function() return false end,
  IsFreshWater = function() return false end,
 }, { __index = function() return function() return -1 end end })
 local env = setmetatable({
  cfg = { ExportState = true, TileYields = false },
  try = function(fn, fallback) local ok, value = pcall(fn); if ok then return value end; return fallback end,
  Map = { GetGridSize = function() return 1, 1 end, GetPlot = function() return plot end },
  CityManager = { GetDistrictAt = function() return district end },
  Cities = { GetPlotPurchaseCity = function() return city end },
  PlayersVisibility = { [0] = {
   IsVisible = function() return s.visible end,
   IsRevealed = function(_, x, y) assert(x == 9 and y == 10); return s.parent_known end,
  } },
  GameInfo = { Districts = {
   [0] = { DistrictType = "DISTRICT_CITY_CENTER" },
   [1] = { DistrictType = "DISTRICT_ENCAMPMENT", AttackRange = 2 },
   [2] = { DistrictType = "DISTRICT_CAMPUS" },
  } },
  plotRevealed = function() return true end,
  visibleResourceName = function() return nil end,
  riverMask = function() return 0 end,
  typeName = function(group) if group == "Terrains" then return "TERRAIN_GRASS" end end,
  emit = function(kind, row) if kind == "tiles" then s.events[#s.events + 1] = row end end,
 }, { __index = _G })
 local chunk = assert(loadstring(section)); setfenv(chunk, env); chunk()
 local function sweep()
  s.events = {}
  local fresh = env.CivvisTiles.sweep({}, 0, 180, 0)
  return fresh, s.events[1] and s.events[1].plots[1]
 end
 return s, sweep
end

local failures, cases = {}, 0
local function test(name, callback)
 cases = cases + 1
 local ok, err = pcall(callback)
 if ok then print("PASS " .. name)
 else failures[#failures + 1] = name; print("FAIL " .. name .. ": " .. tostring(err)) end
end

test("visible completed district names its known parent", function()
 local _, sweep = fixture(); local fresh, row = sweep()
 assert(fresh == 1 and row.d == "DISTRICT_ENCAMPMENT" and row.dc == true)
 assert(row.oc == 31, "actual district parent must cross with the plot")
end)
test("parent alone triggers a delta", function()
 local s, sweep = fixture(); sweep(); s.parent = 32
 local fresh, row = sweep()
 assert(fresh == 1 and row.oc == 32, "a changed parent must refresh the plot")
end)
test("unchanged parent has no extra delta", function()
 local _, sweep = fixture(); sweep(); assert(sweep() == 0)
end)
test("fog retains the last parent without reading it", function()
 local s, sweep = fixture(); sweep(); s.visible = false; s.parent = 32
 local before = s.reads; local fresh, row = sweep()
 assert(fresh == 1 and row.vis ~= true)
 assert(row.oc == 31 and s.reads == before, "fog must preserve only the observed parent")
 assert(sweep() == 0 and s.reads == before)
 s.visible = true; fresh, row = sweep()
 assert(fresh == 1 and row.oc == 32)
end)
test("initial fog has no parent observation", function()
 local s, sweep = fixture(); s.visible = false
 local _, row = sweep(); assert(row.oc == nil and s.reads == 0)
end)
test("failed getter retains an observed parent", function()
 local s, sweep = fixture(); sweep(); s.fail = true; s.visible = false; sweep(); s.visible = true
 local _, row = sweep(); assert(row.oc == 31)
end)
test("initial failed getter remains unknown", function()
 local s, sweep = fixture(); s.fail = true
 local _, row = sweep(); assert(row.oc == nil)
end)
test("missing parent remains unknown", function()
 local s, sweep = fixture(); s.no_city = true
 local _, row = sweep(); assert(row.oc == nil)
end)
test("changed owner clears fog parent", function()
 local s, sweep = fixture(); sweep(); s.visible = false; s.owner = 3
 local before = s.reads; local _, row = sweep(); assert(row.oc == nil and s.reads == before)
end)
test("changed district type clears fog parent", function()
 local s, sweep = fixture(); sweep(); s.visible = false; s.kind = 2
 local before = s.reads; local _, row = sweep(); assert(row.oc == nil and s.reads == before)
end)
test("visible district replacement clears cached parent", function()
 local s, sweep = fixture(); sweep(); s.id = 43; s.fail = true
 local fresh, row = sweep(); assert(fresh == 1 and row.oc == nil)
 s.visible = false; _, row = sweep(); assert(row.oc == nil)
end)
test("unrevealed city is not named by its district", function()
 local s, sweep = fixture(); s.parent_known = false
 local _, row = sweep(); assert(row.oc == nil, "district sight must not expose an unseen city")
end)
test("parent owner must match the plot owner", function()
 local s, sweep = fixture(); s.parent_owner = 3
 local _, row = sweep(); assert(row.oc == nil)
end)
test("unbuilt district does not read a parent", function()
 local s, sweep = fixture(); s.complete = false
 local _, row = sweep(); assert(row.oc == nil and s.reads == 0)
end)
test("city center uses its city record", function()
 local s, sweep = fixture(); s.kind = 0
 local _, row = sweep(); assert(row.oc == nil and s.reads == 0)
end)
test("ordinary ground does not expose a purchase city", function()
 local s, sweep = fixture(); s.kind = -1
 local _, row = sweep(); assert(row.oc == nil and s.reads == 0)
end)
test("our plot keeps its purchase city", function()
 local s, sweep = fixture(); s.owner = 0; s.parent_owner = 0; s.kind = -1
 local _, row = sweep(); assert(row.oc == 31 and s.reads == 0)
end)
test("native city zero is a valid parent", function()
 local s, sweep = fixture(); s.parent = 0
 local _, row = sweep(); assert(row.oc == 0)
end)
test("nondefending districts also name their parent", function()
 local s, sweep = fixture(); s.kind = 2
 local _, row = sweep(); assert(row.d == "DISTRICT_CAMPUS" and row.oc == 31)
end)
test("changed parent cannot expose an unrevealed city", function()
 local s, sweep = fixture(); local _, row = sweep(); assert(row.oc == 31)
 s.parent = 32; s.parent_known = false
 local fresh; fresh, row = sweep()
 assert(fresh == 1 and row.oc == nil, "a new unseen parent must clear the old assignment")
end)
test("capture by us clears the former foreign parent", function()
 local s, sweep = fixture(); local _, row = sweep(); assert(row.oc == 31)
 local before = s.reads; s.owner = 0; s.parent_owner = 0
 _, row = sweep(); assert(row.oc == 31 and s.reads == before)
 s.owner = 2; s.parent_owner = 2; s.fail = true
 _, row = sweep(); assert(row.oc == nil, "former ownership must not revive a cached district parent")
end)
print("RESULT " .. cases .. " cases; " .. #failures .. " failed")
assert(#failures == 0, "foreign district parent failures: " .. table.concat(failures, ", "))
