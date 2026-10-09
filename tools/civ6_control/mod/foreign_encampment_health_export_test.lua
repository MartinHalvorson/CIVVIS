local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local section = assert(source:match("(CivvisTiles = .-)\n%-%- ★★★★★ CHANNEL PROBE"))

local function fixture()
 local state = { visible = true, complete = true, owner = 2, kind = 1,
                 damage = 0, max_damage = 100, wall_damage = 0, max_wall_damage = 200,
                 reads = 0, fail = false, id = 42, events = {} }
 local function reading(key)
  state.reads = state.reads + 1
  if state.fail or state.fail_key == key then error("unavailable district health") end
  return state[key]
 end
 local district = {
  GetID = function() return state.id end,
  IsComplete = function() return state.complete end,
  IsPillaged = function() return false end,
  GetDamage = function(_, pool) return reading(pool == 0 and "damage" or "wall_damage") end,
  GetMaxDamage = function(_, pool) return reading(pool == 0 and "max_damage" or "max_wall_damage") end,
 }
 local plot = setmetatable({
  GetDistrictType = function() return state.kind end,
  GetImprovementType = function() return -1 end,
  GetOwner = function() return state.owner end,
  IsWater = function() return false end,
  IsImpassable = function() return false end,
  IsFreshWater = function() return false end,
 }, { __index = function() return function() return -1 end end })
 local env = setmetatable({
  cfg = { ExportState = true, TileYields = false },
  try = function(fn, fallback) local ok, value = pcall(fn); if ok then return value end; return fallback end,
  Map = { GetGridSize = function() return 1, 1 end, GetPlot = function() return plot end },
  CityManager = { GetDistrictAt = function() return district end },
  PlayersVisibility = { [0] = { IsVisible = function() return state.visible end } },
  DefenseTypes = { DISTRICT_GARRISON = 0, DISTRICT_OUTER = 1 },
  GameInfo = { Districts = {
    [0] = { DistrictType = "DISTRICT_CITY_CENTER" },
    [1] = { DistrictType = "DISTRICT_ENCAMPMENT" },
    [2] = { DistrictType = "DISTRICT_CAMPUS" },
    [3] = { DistrictType = "DISTRICT_IKANDA" },
    [4] = { DistrictType = "DISTRICT_THANH" },
  }, Districts_XP2 = {
    DISTRICT_CITY_CENTER = { AttackRange = 2 },
    DISTRICT_ENCAMPMENT = { AttackRange = 2 },
    DISTRICT_IKANDA = { AttackRange = 2 },
    DISTRICT_THANH = { AttackRange = 2 },
  } },
  plotRevealed = function() return true end,
  visibleResourceName = function() return nil end,
  riverMask = function() return 0 end,
  typeName = function(group) if group == "Terrains" then return "TERRAIN_GRASS" end end,
  emit = function(kind, row) if kind == "tiles" then state.events[#state.events + 1] = row end end,
 }, { __index = _G })
 local chunk = assert(loadstring(section)); setfenv(chunk, env); chunk()
 local function sweep()
  state.events = {}
  local fresh = env.CivvisTiles.sweep({}, 0, 180, 0)
  return fresh, state.events[1] and state.events[1].plots[1]
 end
 return state, sweep
end

local failures, cases = {}, 0
local function test(name, callback)
 cases = cases + 1
 local ok, err = pcall(callback)
 if ok then print("PASS " .. name)
 else failures[#failures + 1] = name; print("FAIL " .. name .. ": " .. tostring(err)) end
end

test("visible raw defense pools", function()
 local s, sweep = fixture(); s.damage = 60; s.max_damage = 200; s.wall_damage = 125; s.max_wall_damage = 400
 local fresh, row = sweep()
 assert(fresh == 1 and row.d == "DISTRICT_ENCAMPMENT" and row.dc == true)
 assert(row.dh and row.dh.damage == 60 and row.dh.max_damage == 200 and row.dh.wall_damage == 125 and row.dh.max_wall_damage == 400,
        "visible district defense pools must cross with the plot")
end)
test("health alone triggers a delta", function()
 local s, sweep = fixture(); sweep(); s.damage = 25
 local fresh, row = sweep()
 assert(fresh == 1 and row.dh and row.dh.damage == 25, "damage alone must refresh a mid-turn plot")
end)
test("walls alone trigger a delta", function()
 local s, sweep = fixture(); sweep(); s.wall_damage = 200
 local fresh, row = sweep()
 assert(fresh == 1 and row.dh and row.dh.wall_damage == 200, "outer damage alone must refresh a mid-turn plot")
end)
test("capacity alone triggers a delta", function()
 local s, sweep = fixture(); sweep(); s.max_wall_damage = 400
 local fresh, row = sweep()
 assert(fresh == 1 and row.dh and row.dh.max_wall_damage == 400, "outer capacity alone must refresh a plot")
end)
test("unchanged health has no extra delta", function()
 local _, sweep = fixture(); sweep(); assert(sweep() == 0)
end)
test("zero outer capacity is observed", function()
 local s, sweep = fixture(); s.max_wall_damage = 0
 local _, row = sweep()
 assert(row.dh and row.dh.max_wall_damage == 0 and row.dh.wall_damage == 0, "measured zero must survive export")
end)
test("fog retains the last observation without reads", function()
 local s, sweep = fixture(); sweep(); s.visible = false; s.damage = 100; s.wall_damage = 200
 local before = s.reads; local fresh, row = sweep()
 assert(fresh == 1 and row.vis ~= true)
 assert(row.dh and row.dh.damage == 0 and row.dh.wall_damage == 0 and s.reads == before,
        "fog must keep observed health without accessing hidden district health")
 assert(sweep() == 0 and s.reads == before)
 s.visible = true; fresh, row = sweep()
 assert(fresh == 1 and row.dh.damage == 100 and row.dh.wall_damage == 200)
end)
test("fresh fog has no invented health", function()
 local s, sweep = fixture(); s.visible = false
 local _, row = sweep(); assert(row.dh == nil and s.reads == 0)
end)
test("failed reads retain a previous observation", function()
 local s, sweep = fixture(); sweep(); s.fail = true; s.visible = false; sweep(); s.visible = true
 local _, row = sweep()
 assert(row.dh and row.dh.damage == 0 and row.dh.max_damage == 100, "failed getter must preserve earlier observed health")
end)
test("failed initial reads remain unknown", function()
 local s, sweep = fixture(); s.fail = true
 local _, row = sweep(); assert(row.dh == nil, "failed getters must not invent defense damage")
end)
test("changed ownership cannot reuse fog health", function()
 local s, sweep = fixture(); sweep(); s.visible = false; s.owner = 3
 local before = s.reads; local _, row = sweep()
 assert(row.dh == nil and s.reads == before)
end)
test("a different district cannot reuse fort health", function()
 local s, sweep = fixture(); sweep(); s.visible = false; s.kind = 2
 local before = s.reads; local _, row = sweep()
 assert(row.d == "DISTRICT_CAMPUS" and row.dh == nil and s.reads == before)
end)
test("unbuilt fort has no defense observation", function()
 local s, sweep = fixture(); s.complete = false
 local _, row = sweep(); assert(row.dc == false and row.dh == nil and s.reads == 0)
end)
test("city center health remains in the city record", function()
 local s, sweep = fixture(); s.kind = 0
 local _, row = sweep(); assert(row.d == "DISTRICT_CITY_CENTER" and row.dh == nil and s.reads == 0)
end)
test("Encampment replacements export their own defenses", function()
 for _, kind in ipairs({ 3, 4 }) do
  local s, sweep = fixture(); s.kind = kind; s.damage = 40
  local _, row = sweep()
  assert(row.dh and row.dh.damage == 40 and row.dh.max_damage == 100)
 end
end)
test("a failed pool retains the previous pair", function()
 local s, sweep = fixture(); sweep(); s.fail_key = "max_damage"; s.damage = 60; s.wall_damage = 50
 local fresh, row = sweep()
 assert(fresh == 1 and row.dh.damage == 0 and row.dh.max_damage == 100 and row.dh.wall_damage == 50)
end)
test("an initially missing pool remains unknown", function()
 local s, sweep = fixture(); s.fail_key = "max_damage"
 local _, row = sweep()
 assert(row.dh and row.dh.damage == nil and row.dh.max_damage == nil and row.dh.max_wall_damage == 200)
end)
test("invalid initial pools are not observations", function()
 local s, sweep = fixture(); s.damage = 0/0; s.max_damage = math.huge; s.wall_damage = -1; s.max_wall_damage = 200
 local _, row = sweep(); assert(row.dh == nil)
end)
test("a rebuilt district cannot inherit the former fort in fog", function()
 local s, sweep = fixture(); sweep(); s.id = 43; s.fail = true
 local fresh, row = sweep(); assert(fresh == 1 and row.dh == nil)
 s.visible = false; _, row = sweep()
 assert(row.dh == nil, "a changed visible district identity must discard its old health cache")
end)
print("RESULT " .. cases .. " cases; " .. #failures .. " failed")
assert(#failures == 0, "foreign Encampment health failures: " .. table.concat(failures, ", "))
