-- Execute the shipped Spy purchase-city reader, including district operation.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local section = assert(source:match("(CivvisSpyCity = function.-)\nlocal function exportState"))
assert(source:find("spy_city = CivvisSpyCity(unit, name, pid),", 1, true), "reader must be wired into actual unit payload")
local city = { GetID = function() return 9 end, GetOwner = function() return 2 end,
 GetX = function() return 29 end, GetY = function() return 11 end }
local owner, located, reads = 0, true, 0
local unit = { GetOwner = function() return owner end,
 GetX = function() return 31 end, GetY = function() return 8 end }
local plot = {}
local env = setmetatable({
 try = function(fn, fallback) local ok, value = pcall(fn); if ok then return value end; return fallback end,
 Map = { GetPlot = function(x, y) assert(x == 31 and y == 8); if located then return plot end end },
 Cities = { GetPlotPurchaseCity = function(p) assert(p == plot); reads = reads + 1; return city end },
}, { __index = _G })
local chunk = assert(loadstring(section)); setfenv(chunk, env); chunk()
local row = env.CivvisSpyCity(unit, "UNIT_SPY", 0)
assert(row.id == 9 and row.player == 2 and row.x == 29 and row.y == 11,
 "district31,8 belongs to city29,11, not to a center at the unit position")
assert(env.CivvisSpyCity(unit, "UNIT_WARRIOR", 0) == nil)
owner = 2
local before = reads
assert(env.CivvisSpyCity(unit, "UNIT_SPY", 0) == nil and reads == before,
 "foreign spy location must never be looked up")
owner = 0; located = false
assert(env.CivvisSpyCity(unit, "UNIT_SPY", 0) == nil)
located = true; city = nil
assert(env.CivvisSpyCity(unit, "UNIT_SPY", 0) == nil)
city = { GetID = function() return 0 end, GetOwner = function() return 0 end,
 GetX = function() return 0 end, GetY = function() return 0 end }
row = env.CivvisSpyCity(unit, "UNIT_SPY", 0)
assert(row.id == 0 and row.player == 0 and row.x == 0 and row.y == 0, "zero identities are valid")
city.GetID = nil
assert(env.CivvisSpyCity(unit, "UNIT_SPY", 0) == nil, "failed city read returns no partial payload")
env.Cities.GetPlotPurchaseCity = nil
assert(env.CivvisSpyCity(unit, "UNIT_SPY", 0) == nil, "older API failure is tolerated")
print("native spy purchase-city: district, owner, identity, unknown and wiring checks passed")
