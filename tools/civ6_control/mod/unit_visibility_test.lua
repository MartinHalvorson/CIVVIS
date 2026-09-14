-- Terrain visibility cannot reveal an undetected submarine or spy.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local helper = assert(source:match("(CivvisUnitVisible = function.-\nend;)"))
local visible, detected = true, true
local visibility = {
    IsVisible = function() return visible end,
    IsUnitVisible = function() return detected end,
}
local env = { PlayersVisibility = { [0] = visibility },
    try = function(fn, fallback)
        local ok, value = pcall(fn)
        if ok then return value end
        return fallback
    end,
}
local chunk = assert(loadstring(helper)); setfenv(chunk, env); chunk()
local unit = { GetX = function() return 2 end, GetY = function() return 3 end }
assert(env.CivvisUnitVisible(0, unit))
detected = false
assert(not env.CivvisUnitVisible(0, unit), "visible terrain does not detect a unit")
detected, visible = true, false
assert(not env.CivvisUnitVisible(0, unit), "remembered terrain does not reveal current occupants")
visible, visibility.IsUnitVisible = true, nil
assert(not env.CivvisUnitVisible(0, unit), "missing detection evidence fails closed")
local _, consumers = source:gsub("CivvisUnitVisible%(pid, unit%)", "")
assert(consumers == 3, "all three foreign roster exports must use detection")
print("foreign unit visibility: passed")
