local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local section = assert(source:match("(CivvisTiles = .-)\n%-%- ★★★★★ CHANNEL PROBE"))

-- Expansion2_Schema.sql:180-188 keys Districts_XP2 by DistrictType;
-- AttackRange lives there, independently of the base Districts index.
local function fixture()
 local state = { kind = 1, reads = 0, events = {} }
 local names = { [0] = "DISTRICT_CITY_CENTER", [1] = "DISTRICT_ENCAMPMENT",
                 [2] = "DISTRICT_CAMPUS", [3] = "DISTRICT_IKANDA",
                 [4] = "DISTRICT_THANH", [5] = "DISTRICT_OPPIDUM" }
 local base, xp2 = {}, {}
 for kind, name in pairs(names) do
  base[kind] = { DistrictType = name }
  if kind ~= 2 then xp2[name] = { DistrictType = name, AttackRange = 2 } end
 end
 -- A numeric index is not the DistrictType primary key.
 xp2[1] = { DistrictType = "DISTRICT_CAMPUS", AttackRange = 0 }
 local function read(pool, maximum)
  state.reads = state.reads + 1
  if maximum then return pool == 0 and 100 or 200 end
  return pool == 0 and 60 or 125
 end
 local district = {
  GetID = function() return 42 end,
  IsComplete = function() return true end,
  IsPillaged = function() return false end,
  GetDamage = function(_, pool) return read(pool, false) end,
  GetMaxDamage = function(_, pool) return read(pool, true) end,
 }
 local plot = setmetatable({
  GetDistrictType = function() return state.kind end,
  GetImprovementType = function() return -1 end,
  GetOwner = function() return 2 end,
  IsWater = function() return false end,
  IsImpassable = function() return false end,
  IsFreshWater = function() return false end,
 }, { __index = function() return function() return -1 end end })
 local info = { Districts = base, Districts_XP2 = xp2 }
 local env = setmetatable({
  cfg = { ExportState = true, TileYields = false }, GameInfo = info,
  try = function(fn, fallback) local ok, value = pcall(fn); if ok then return value end; return fallback end,
  Map = { GetGridSize = function() return 1, 1 end, GetPlot = function() return plot end },
  CityManager = { GetDistrictAt = function() return district end },
  PlayersVisibility = { [0] = { IsVisible = function() return true end } },
  DefenseTypes = { DISTRICT_GARRISON = 0, DISTRICT_OUTER = 1 },
  plotRevealed = function() return true end,
  visibleResourceName = function() return nil end,
  riverMask = function() return 0 end,
  typeName = function(group) if group == "Terrains" then return "TERRAIN_GRASS" end end,
  emit = function(kind, row) if kind == "tiles" then state.events[#state.events + 1] = row end end,
 }, { __index = _G })
 local chunk = assert(loadstring(section)); setfenv(chunk, env); chunk()
 local function sweep()
  state.events = {}
  env.CivvisTiles.sweep({}, 0, 180, 0)
  return assert(state.events[1]).plots[1]
 end
 return state, info, sweep
end

local failures, cases = {}, 0
local function test(name, callback)
 cases = cases + 1
 local ok, err = pcall(callback)
 if ok then print("PASS " .. name)
 else failures[#failures + 1] = name; print("FAIL " .. name .. ": " .. tostring(err)) end
end

for _, item in ipairs({ { 1, "Encampment" }, { 3, "Ikanda" }, { 4, "Thanh" }, { 5, "Oppidum" } }) do
 test(item[2] .. " exports defense pools from the XP2 range row", function()
  local s, info, sweep = fixture(); s.kind = item[1]
  assert(info.Districts[s.kind].AttackRange == nil)
  local row = sweep()
  assert(row.dh and row.dh.damage == 60 and row.dh.max_damage == 100
         and row.dh.wall_damage == 125 and row.dh.max_wall_damage == 200,
         "a visible completed defending district needs its observed pools")
  assert(s.reads == 4)
 end)
end
test("XP2 rows use DistrictType rather than the base numeric index", function()
 local s, info, sweep = fixture()
 assert(info.Districts_XP2[s.kind].AttackRange == 0)
 local row = sweep(); assert(row.dh and row.dh.damage == 60 and s.reads == 4)
end)
test("a zero XP2 range overrides an obsolete base range", function()
 local s, info, sweep = fixture()
 info.Districts[1].AttackRange = 2; info.Districts_XP2.DISTRICT_ENCAMPMENT.AttackRange = 0
 local row = sweep(); assert(row.dh == nil and s.reads == 0)
end)
test("a sparse XP2 table does not revive an obsolete base range", function()
 local s, info, sweep = fixture()
 info.Districts[1].AttackRange = 2; info.Districts_XP2.DISTRICT_ENCAMPMENT = nil
 local row = sweep(); assert(row.dh == nil and s.reads == 0)
end)
test("a positive XP2 range overrides an obsolete base zero", function()
 local s, info, sweep = fixture(); info.Districts[1].AttackRange = 0
 local row = sweep(); assert(row.dh and row.dh.damage == 60 and s.reads == 4)
end)
test("an environment without the XP2 table retains its flat range", function()
 local s, info, sweep = fixture(); info.Districts_XP2 = nil; info.Districts[1].AttackRange = 2
 local row = sweep(); assert(row.dh and row.dh.damage == 60 and s.reads == 4)
end)
test("a Campus without a defense range never reads health", function()
 local s, _, sweep = fixture(); s.kind = 2
 local row = sweep(); assert(row.d == "DISTRICT_CAMPUS" and row.dh == nil and s.reads == 0)
end)
test("city-center range does not duplicate city health", function()
 local s, _, sweep = fixture(); s.kind = 0
 local row = sweep(); assert(row.d == "DISTRICT_CITY_CENTER" and row.dh == nil and s.reads == 0)
end)
print("RESULT " .. cases .. " cases; " .. #failures .. " failed")
assert(#failures == 0, "district defense range failures: " .. table.concat(failures, ", "))
