-- Execute both shipped rival-roster records, including unavailable API reads.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local allowance, unavailable = 6, false
local unit = {
 GetID = function() return 39 end,
 GetDamage = function() return 0 end,
 GetMovesRemaining = function() return 0 end,
 GetMaxMoves = function() if unavailable then error("unavailable") end; return allowance end,
 GetFortifyTurns = function() return 0 end,
 GetAttacksRemaining = function() return 1 end,
 IsEmbarked = function() return true end,
}
local env = {
 unit = unit, ux = 7, uy = 26, name = "UNIT_CUIRASSIER",
 row = { Combat = 64, RangedCombat = 0 }, progress = { promotions = {} },
 unitBaseType = function() return nil end,
 unitClass = function() return "PROMOTION_CLASS_HEAVY_CAVALRY" end,
 CivvisMilitaryFormation = function() return 0 end,
 try = function(fn, fallback)
  local ok, value = pcall(fn); if ok then return value end; return fallback
 end,
}
local count = 0
for record in source:gmatch("theirUnits%[#theirUnits %+ 1%] = (%b{});") do
 count = count + 1
 local chunk = assert(loadstring("return " .. record)); setfenv(chunk, env)
 for _, maximum in ipairs({ 6, 2.5, 0 }) do
  allowance, unavailable = maximum, false
  local row = chunk()
  assert(row.moves == 0, "spent movement is a separate observation")
  assert(row.max_moves == maximum, "foreign roster omitted the native allowance")
 end
 allowance = nil
 assert(chunk().max_moves == nil, "unknown must not be replaced by spent movement")
 unavailable = true
 assert(chunk().max_moves == nil, "unavailable API must remain unknown")
 unit.GetMaxMoves = nil
 assert(chunk().max_moves == nil, "older API absence must remain unknown")
 unit.GetMaxMoves = function() if unavailable then error("unavailable") end; return allowance end
end
assert(count == 2, "execute the major and city-state roster records")
print("visible foreign movement: both rosters preserve actual, fractional, zero and unknown allowance")
