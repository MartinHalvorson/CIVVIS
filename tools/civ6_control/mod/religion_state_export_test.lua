-- Exercise the actual read-only exporter, including tuple identities and errors.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local f = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = f:read("*a"); f:close()
local helper = assert(source:match("%-%- BEGIN holy%-city observation(.-)%-%- END holy%-city observation"))
local launched = assert(source:match("inquisition_launched = (try%(function%(%).-end, nil%))"))
local religion = {}
local city = {GetX = function() return 5 end, GetY = function() return 7 end}
local manager = {}
local env = setmetatable({
 playerReligion = religion, CityManager = manager,
 try = function(fn, fallback) local ok, value = pcall(fn); if ok then return value end; return fallback end,
}, {__index = _G})
local function expression(text) local fn = assert(loadstring(text)); setfenv(fn, env); return fn end
expression(helper)()
local holy = function() return env.CivvisReligionState.holyCity(religion) end
launched = expression("return " .. launched)

-- The real API may return owner and local city ID. Preserve BOTH, including
-- owner zero: losing a return makes the lookup fail or resolve another city.
religion.GetHolyCityID = function() return 0, 42 end
manager.GetCity = function(owner, id) assert(owner == 0 and id == 42); return city end
religion.HasLaunchedInquisition = function() return false end
local position, observation = holy()
assert(position[1] == 5 and position[2] == 7 and launched() == false)
assert(observation.status == "observed" and observation.identity_returns == 2)
assert(observation.identity[1].value == 0 and observation.identity[2].value == 42)
religion.HasLaunchedInquisition = function() return true end
assert(launched() == true)

-- Opaque identities pass through without pretending their contents are known.
local identity = {}
religion.GetHolyCityID = function() return identity end
manager.GetCity = function(value) assert(value == identity); return city end
position, observation = holy()
assert(position[1] == 5 and observation.identity[1].type == "table")
assert(observation.identity[1].value == nil)

manager.GetCity = function() return nil end
position, observation = holy()
assert(position == nil and observation.status == "city_missing")
manager.GetCity = function() error("lookup failed") end
position, observation = holy()
assert(position == nil and observation.status == "lookup_error")
assert(observation.error:find("lookup failed", 1, true))
religion.GetHolyCityID = function() error("identity failed") end
position, observation = holy()
assert(position == nil and observation.status == "identity_error")
assert(observation.error:find("identity failed", 1, true))
religion.GetHolyCityID = nil; religion.HasLaunchedInquisition = nil
position, observation = holy()
assert(position == nil and observation.status == "identity_error" and launched() == nil)

-- Preserve a nil in the tuple rather than shortening the lookup argument list.
religion.GetHolyCityID = function() return nil, 42 end
manager.GetCity = function(...)
 assert(select("#", ...) == 2)
 local owner, id = ...; assert(owner == nil and id == 42)
 return city
end
position, observation = holy()
assert(position[2] == 7 and observation.identity[1].type == "nil")
city.GetX = function() error("coordinate unavailable") end
position, observation = holy()
assert(position == nil and observation.status == "coordinate_error")
city.GetX = function() return -1 end
position, observation = holy()
assert(position == nil and observation.status == "invalid_coordinates")

-- The helper must be called by the state exporter, and both outputs published.
assert(source:find("holyCity, holyCityObservation = CivvisReligionState.holyCity(playerReligion)", 1, true))
assert(source:find("holy_city = holyCity,", 1, true))
assert(source:find("holy_city_observation = holyCityObservation,", 1, true))
print("religion state export checks passed")
