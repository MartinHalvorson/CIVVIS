-- Offline regression for the emergency tracker's `type` in the state export.
--
-- `GetEmergencyInfoTable` reports `EmergencyType` as a row index of
-- `GameInfo.EmergencyAlliances`. The export wrote that number as text
-- ("14"), and CIVVIS's mirror matches the name (`EMERGENCY_WORLD_GAMES`), so
-- a competition the seat had joined did not exist for the planner. This
-- loads the actual agent and checks the name the export now writes.
--
-- Run: lua5.1 tools/civ6_control/mod/emergency_type_name_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end

setmetatable(_G, { __index = function(_, key)
	if key == "CivvisEmergencyTypeName" then return rawget(_G, key) end
	return stub()
end })
Automation = { Log = function() end }
UI = { DataError = function() end }
CivvisControlConfig = { Play = true }

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtimeErr = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtimeErr))

local name = rawget(_G, "CivvisEmergencyTypeName")
assert(type(name) == "function", "CivvisControlAgent.lua did not export CivvisEmergencyTypeName")

local failures = 0
local function check(label, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", label, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", label, tostring(got)))
	end
end

-- The rows the live game reported at t115 and t135 of civvis-20261113T204218Z,
-- with the names the shipped data gives them; a row with no name; an index
-- the table does not hold (nil, as the host answers); and one it raises on.
local rows = {
	[11] = { EmergencyType = "EMERGENCY_WORLDS_FAIR" },
	[14] = { EmergencyType = "EMERGENCY_WORLD_GAMES" },
	[3] = { EmergencyType = "EMERGENCY_SEND_AID" },
	[7] = { EmergencyType = "" },
}
GameInfo = { EmergencyAlliances = setmetatable({}, { __index = function(_, key)
	if key == -1 then error("no such row: " .. tostring(key)) end
	return rows[key]
end }) }

check("the World's Fair row", name(11), "EMERGENCY_WORLDS_FAIR")
check("the World Games row", name(14), "EMERGENCY_WORLD_GAMES")
check("an Aid Request row", name(3), "EMERGENCY_SEND_AID")
check("a name passes through", name("EMERGENCY_CLIMATE_ACCORDS"), "EMERGENCY_CLIMATE_ACCORDS")
check("a row the table does not hold stays the number", name(99), "99")
check("a lookup that raises stays the number", name(-1), "-1")
check("a row with an empty name stays the number", name(7), "7")
check("nothing stays readable", name(nil), "nil")
check("text that is not a name is looked up, then kept", name("14"), "14")

-- The state export uses it for the tracker's `type`.
do
	local handle = assert(io.open(here .. "/CivvisControlAgent.lua", "r"))
	local source = handle:read("*a")
	handle:close()
	check("the export writes the name",
		source:find("type = CivvisEmergencyTypeName(crisis.EmergencyType),", 1, true) ~= nil, true)
	check("the export no longer writes the number",
		source:find("type = tostring(crisis.EmergencyType),", 1, true), nil)
end

if failures > 0 then
	print(string.format("%d FAILED", failures))
	os.exit(1)
end
print("all emergency type name checks passed")
