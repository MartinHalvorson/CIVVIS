local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local expression = assert(source:match("district_capacity = (try%(function%(%).-\n\t\t\tend, nil%)),"))
local value = 7
local districts = { GetNumAllowedDistrictsRequiringPopulation = function() return value end }
local env = setmetatable({
 try = function(fn, fallback) local ok, result = pcall(fn); if ok then return result end; return fallback end,
 city = { GetDistricts = function() return districts end },
}, { __index = _G })
local export = assert(loadstring("return " .. expression)); setfenv(export, env)
assert(export() == 7, "native bonuses must survive export")
value = 0; assert(export() == 0, "zero is an authoritative limit")
for _, invalid in ipairs({-1, 2.5, "7", math.huge, 0/0}) do
 value = invalid; assert(export() == nil, "invalid capacity must retain legacy fallback")
end
districts.GetNumAllowedDistrictsRequiringPopulation = nil
assert(export() == nil, "missing API must not invent capacity")
districts.GetNumAllowedDistrictsRequiringPopulation = function() error("unavailable") end
assert(export() == nil)
print("district capacity export checks passed")
