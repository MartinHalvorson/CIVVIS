-- Execute the actual stockpile expression, including its guarded host calls.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local begin = assert(source:find("\t\tstrategic_resources = ", 1, true))
local finish = assert(source:find("\t\t-- Our luxury copies", begin, true))
local expression = assert(source:sub(begin, finish - 1):match("strategic_resources%s*=%s*(.-),%s*$"))
local rows = {
 { ResourceType = "RESOURCE_COAL", Index = 41, ResourceClassType = "RESOURCECLASS_STRATEGIC" },
 { ResourceType = "RESOURCE_IRON", Index = 42, ResourceClassType = "RESOURCECLASS_STRATEGIC" },
 { ResourceType = "RESOURCE_SILK", Index = 43, ResourceClassType = "RESOURCECLASS_LUXURY" },
}
local values, faults, calls = {}, {}, {}
local resources = {
 GetResourceAmount = function(_, resourceType)
  calls[#calls + 1] = resourceType;
  if faults[resourceType] then error("resource query unavailable"); end
  return values[resourceType];
 end,
}
local player = { GetResources = function() return resources; end }
local env = setmetatable({
 player = player,
 GameInfo = { Resources = function()
  local index = 0;
  return function() index = index + 1; return rows[index]; end;
 end },
 try = function(fn, fallback)
  local ok, value = pcall(fn); if ok then return value; end; return fallback;
 end,
}, { __index = _G })
local chunk = assert(loadstring("return " .. expression)); setfenv(chunk, env)
local passed, failed = 0, 0
local function check(ok, label)
 if ok then passed = passed + 1; else failed = failed + 1; print("FAIL: " .. label); end
end
local function read()
 calls = {};
 return chunk();
end

values = { RESOURCE_COAL = 0, RESOURCE_IRON = 12, RESOURCE_SILK = 99 }
local out = read()
check(out ~= nil and out.RESOURCE_COAL == 0, "a successful zero survives beside a positive stock")
check(out ~= nil and out.RESOURCE_IRON == 12, "positive stock is unchanged")
check(out ~= nil and out.RESOURCE_SILK == nil, "only strategic resources are exported")
check(#calls == 2 and calls[1] == "RESOURCE_COAL" and calls[2] == "RESOURCE_IRON",
      "preserve the existing getter arguments and one call per strategic row")

values = { RESOURCE_COAL = 0, RESOURCE_IRON = 0 }
out = read()
check(out ~= nil and out.RESOURCE_COAL == 0 and out.RESOURCE_IRON == 0,
      "all-zero successful reads form a nonempty keyed map")

faults = { RESOURCE_COAL = true }
out = read()
check(out ~= nil and out.RESOURCE_COAL == nil and out.RESOURCE_IRON == 0,
      "one failed query remains unknown without erasing another known zero")
faults.RESOURCE_IRON = true
check(read() == nil, "all failed queries remain nil rather than an empty array")
faults = {}

for _, invalid in ipairs({0/0, math.huge, -math.huge, -1, "12", true}) do
 values = { RESOURCE_COAL = invalid }
 check(read() == nil, "invalid stock stays unknown: " .. tostring(invalid))
end
values = {}
check(read() == nil, "nil stock is unknown")
player.GetResources = function() return nil; end
check(read() == nil, "absent resource object is unknown")
player.GetResources = function() error("resource subsystem unavailable"); end
check(read() == nil, "failed resource object lookup is unknown")
print(tostring(passed) .. " stockpile readback checks passed; " .. tostring(failed) .. " failed")
assert(failed == 0, "stockpile readback regressions")
