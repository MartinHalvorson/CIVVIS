-- Execute the seat's per-major Production export, the operator's
-- turn-150 restart rule's only input (`production_rank_reading` in
-- tools/civ6_play.py): every alive major's summed city Production, keyed by
-- player id as a string, read from the build queue first and the city yield
-- second; a major none of whose cities can be read is absent, never 0.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local body = assert(source:match("\n(CivvisMajorProduction = function%(%).-\nend;)\n"))
local _, sites = source:gsub("major_production = CivvisMajorProduction%(%),\n%s*local_player = pid,", "")
assert(sites == 2, "both turn records carry the map and our seat: " .. sites)

local function city(queue, yield)
 return {
  GetBuildQueue = function()
   if queue == "throw" then error("no queue") end
   return { GetProductionYield = function() return queue end }
  end,
  GetYield = function(_, kind)
   assert(kind == "PRODUCTION")
   if yield == "throw" then error("no yield") end
   return yield
  end,
 }
end
local function cities(list)
 return { Members = function()
  local i = 0
  return function() i = i + 1; if list[i] then return i, list[i] end end
 end }
end
local players = {
 [0] = { GetCities = function() return cities({ city(12.5, 99), city(7.25, 99) }) end },
 [1] = { GetCities = function() return cities({ city(-1, 30), city("throw", 4) }) end },
 [2] = { GetCities = function() return cities({ city("throw", "throw") }) end },
 [3] = { GetCities = function() error("gone") end },
 [4] = { GetCities = function() return cities({}) end },
 -- A stub whose iterator never ends (as the war/condemn test's rivals do).
 [5] = { GetCities = function()
  return { Members = function() return function() return 1, city(1, 1) end end }
 end },
 -- An endless iterator yielding only its control value (a callable stub).
 [6] = { GetCities = function()
  return { Members = function() return function() return 1 end end }
 end },
}
local env = setmetatable({
 try = function(fn, fallback) local ok, value = pcall(fn); if ok then return value end; return fallback end,
 PlayerManager = { GetAliveMajorIDs = function() return { 0, 1, 2, 3, 4, 5, 6 } end },
 Players = players,
 YieldTypes = { PRODUCTION = "PRODUCTION" },
}, { __index = _G })
local define = assert(loadstring(body)); setfenv(define, env); define()
local export = env.CivvisMajorProduction
local observed = export()
assert(observed["0"] == 19.8, "the build queue's Production, summed, to a tenth: " .. tostring(observed["0"]))
assert(observed["1"] == 34, "the city yield when the queue reads -1 or throws")
assert(observed["2"] == nil, "no readable city is absent, not 0")
assert(observed["3"] == nil, "an unreadable major is absent")
assert(observed["4"] == nil, "a major with no cities is absent")
assert(observed["5"] == 200, "an endless city list is capped, not a hang: " .. tostring(observed["5"]))
assert(observed["6"] == nil, "an endless list of nothing is capped and absent")
assert(observed[0] == nil, "keys are strings, so the record encodes as an object")
env.PlayerManager = { GetAliveMajorIDs = function() error("no manager") end }
local empty = export()
assert(next(empty) == nil, "no majors readable is an empty map")
print("major production export checks passed")
