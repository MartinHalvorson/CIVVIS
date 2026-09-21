-- Exercise the actual state-export expressions, including false and unknown.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local f = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = f:read("*a"); f:close()
local holy = assert(source:match("holy_city = (try%(function%(%)%s+local holy =.-end, nil%))"))
local launched = assert(source:match("inquisition_launched = (try%(function%(%).-end, nil%))"))
local religion = {}
local city = {GetX = function() return 5 end, GetY = function() return 7 end}
local env = setmetatable({
 playerReligion = religion,
 CityManager = {GetCity = function(identity) assert(identity == 42); return city end},
 try = function(fn, fallback) local ok, value = pcall(fn); if ok then return value end; return fallback end,
}, {__index = _G})
local function expression(text) local fn = assert(loadstring("return " .. text)); setfenv(fn, env); return fn end
holy = expression(holy); launched = expression(launched)
religion.GetHolyCityID = function() return 42 end
religion.HasLaunchedInquisition = function() return false end
assert(holy()[1] == 5 and holy()[2] == 7 and launched() == false)
religion.HasLaunchedInquisition = function() return true end
assert(launched() == true)
religion.GetHolyCityID = nil; religion.HasLaunchedInquisition = nil
assert(holy() == nil and launched() == nil, "unavailable APIs remain unknown")
print("religion state export checks passed")
