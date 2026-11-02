-- Execute the actual read-only exporter with partial and unavailable host APIs.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local f = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = f:read("*a"); f:close()
local helper = assert(source:match("%-%- BEGIN city power observation(.-)%-%- END city power observation"))
local env = setmetatable({
 try = function(fn, fallback)
  local ok, value = pcall(fn); if ok then return value; end; return fallback
 end,
}, {__index = _G})
local chunk = assert(loadstring(helper)); setfenv(chunk, env); chunk()
local power = {
 GetFreePower = function() return 0 end,
 GetTemporaryPower = function() return 1.5 end,
 GetRequiredPower = function() return 6 end,
 IsFullyPowered = function() return false end,
 IsFullyPoweredByActiveProject = function() return false end,
}
local calls = 0
local city = {GetPower = function() calls = calls + 1; return power end}
local function read() return env.CivvisCityPower(city) end
local out = read()
assert(calls == 1 and out.free == 0 and out.temporary == 1.5 and out.required == 6)
assert(out.fully_powered == false and out.fully_powered_by_active_project == false)

-- A project can power demand despite zero supplied power. Preserve the native
-- bool rather than recomputing it from the three numerical readings.
power.GetTemporaryPower = function() return 0 end
power.IsFullyPowered = function() return true end
power.IsFullyPoweredByActiveProject = function() return true end
out = read()
assert(out.free == 0 and out.temporary == 0 and out.required == 6)
assert(out.fully_powered == true and out.fully_powered_by_active_project == true)

-- One absent/failed API must not erase the other readings or create a default.
power.GetFreePower = nil
power.GetTemporaryPower = function() error("unavailable") end
power.IsFullyPowered = function() return 1 end
out = read()
assert(out.free == nil and out.temporary == nil and out.fully_powered == nil)
assert(out.required == 6 and out.fully_powered_by_active_project == true)

-- Reject values that cannot be represented faithfully in JSON.
for _, invalid in ipairs({0/0, math.huge, -math.huge, "6", true}) do
 power.GetRequiredPower = function() return invalid end
 assert(read().required == nil)
end
power.IsFullyPoweredByActiveProject = nil
assert(read() == nil)
city.GetPower = function() return nil end
assert(read() == nil)
city.GetPower = function() error("power system absent") end
assert(read() == nil)
city.GetPower = nil
assert(read() == nil)

-- A fabricated state field would not exercise this function; require its
-- actual call in the exported city record as well as testing the function.
assert(source:find("power = CivvisCityPower(city),", 1, true))
print("city power export checks passed")
