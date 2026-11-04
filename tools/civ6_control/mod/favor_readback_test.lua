-- Execute the shipped state entries: unknown income must remain unknown,
-- and a missing tooltip must not erase a usable signed Favor rate.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local entries = assert(source:match("(\n\t\tdvp = .-)\n\t\t%-%- The World Congress standing"))
local function snapshot(player)
 local chunk = assert(loadstring("return {" .. entries .. "\n}"))
 setfenv(chunk, setmetatable({
  player = player,
  try = function(fn, fallback)
   local ok, value = pcall(fn)
   if ok then return value end
   return fallback
  end,
 }, { __index = _G }))
 return chunk()
end
local function player(rate, text)
 return {
  GetFavor = function() return 227 end,
  GetFavorPerTurn = function() return rate end,
  GetFavorPerTurnToolTip = function() return text end,
 }
end
for _, rate in ipairs({ -20.5, 0, 6.25 }) do
 local row = snapshot(player(rate, "  +6 [ICON_FAVOR] Government[NEWLINE]-26.5  Emissions  "))
 assert(row.favor == 227, "the balance survives independent readbacks")
 assert(row.favor_per_turn == rate, "export the signed native Favor rate, including zero")
 assert(row.favor_sources == "+6 Government\n-26.5 Emissions", "retain the named signed sources")
end
local row = snapshot({ GetFavor = function() return 0 end })
assert(row.favor == 0 and row.favor_per_turn == nil and row.favor_sources == nil,
 "an unavailable API is unknown, never a synthetic zero rate")
local p = player(-17, "unused")
p.GetFavorPerTurnToolTip = function() error("ruleset has no tooltip") end
row = snapshot(p)
assert(row.favor_per_turn == -17 and row.favor_sources == nil,
 "a failed tooltip does not erase the native rate")
p = player(8, "+8 Government")
p.GetFavorPerTurn = function() error("ruleset has no rate") end
row = snapshot(p)
assert(row.favor_per_turn == nil and row.favor_sources == "+8 Government",
 "a failed rate does not erase the native breakdown")
row = snapshot(player(4, nil))
assert(row.favor_per_turn == 4 and row.favor_sources == nil)
row = snapshot(player(4, string.rep("x", 2000)))
assert(#row.favor_sources == 1600, "bound diagnostic text in every exported frame")
print("native Favor readback: signed/zero/unknown rates, independent failures and bounded sources passed")
