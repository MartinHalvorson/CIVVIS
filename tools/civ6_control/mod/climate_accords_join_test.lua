-- Offline regression for automatic Climate Accords membership.
--
-- CIVVIS must accept the target-free World Crisis before it begins, because
-- only members can score the bridge's 100-point power-plant decommission
-- projects. This invokes the actual callback registered by the control mod.
--
-- Run: lua5.1 tools/civ6_control/mod/climate_accords_join_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end

setmetatable(_G, { __index = function(_, key)
	if key == "CivvisOnAidEmergencyAvailable" then return rawget(_G, key) end
	return stub()
end })

local registrations, logs = {}, {}
Events = setmetatable({}, { __index = function(_, name)
	return { Add = function(handler) registrations[name] = handler end }
end })
Automation = { Log = function(line) logs[#logs + 1] = line end }
UI = { DataError = function() end }
CivvisControlConfig = {
	Play = true, AutoJoinAidRequests = true, AutoJoinClimateAccords = true,
}

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtimeErr = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtimeErr))

local handler = rawget(_G, "CivvisOnAidEmergencyAvailable")
assert(type(handler) == "function",
	"CivvisControlAgent.lua did not export the emergency callback")
assert(registrations.EmergencyAvailable == handler,
	"CivvisControlAgent.lua did not register its callback on EmergencyAvailable")

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", name, tostring(got)))
	end
end

local PID, TYPE = 7, 83
local nextTurn = 300

local function lastJoin()
	for i = #logs, 1, -1 do
		if logs[i]:find('"kind":"climate_accords_join"', 1, true) then
			return logs[i]
		end
	end
	return nil
end

local function reason()
	local line = lastJoin()
	return line and line:match('"reason":"([^"]*)"') or nil
end

local function submitted()
	local line = lastJoin()
	return line ~= nil and line:find('"submitted":true', 1, true) ~= nil
end

local function fixture(opts)
	opts = opts or {}
	nextTurn = nextTurn + 1
	logs = {}
	local state = {
		requests = {}, trackerReads = 0, turn = opts.turn or nextTurn,
		throws = opts.throws or false,
	}
	local crisis = opts.crisis
	if crisis == nil and opts.tracker ~= false then
		crisis = {
			EmergencyType = opts.eventType or TYPE,
			TargetID = opts.target == nil and -1 or opts.target,
			HasBegun = opts.begun or false,
			MemberIDs = opts.members or {},
		}
	end

	CivvisControlConfig.Play = opts.play ~= false
	CivvisControlConfig.AutoJoinAidRequests = opts.aidEnabled ~= false
	if opts.defaultEnabled then
		CivvisControlConfig.AutoJoinClimateAccords = nil
	else
		CivvisControlConfig.AutoJoinClimateAccords = opts.enabled ~= false
	end
	GameInfo = { EmergencyAlliances = {} }
	if not opts.noDefinition then
		GameInfo.EmergencyAlliances[opts.eventType or TYPE] = {
			EmergencyType = opts.kind or "EMERGENCY_CLIMATE_ACCORDS",
		}
	end
	Game = {
		GetLocalPlayer = function() return PID end,
		GetCurrentGameTurn = function() return state.turn end,
		GetEmergencyManager = function()
			return {
				GetEmergencyInfoTable = function(_, requested)
					state.trackerReads = state.trackerReads + 1
					state.trackerPlayer = requested
					return crisis and { crisis } or {}
				end,
			}
		end,
	}
	-- Climate Accords has no player target. An empty Players table proves that
	-- joining it does not accidentally try to inspect Players[-1] or diplomacy.
	Players = {}
	PlayerOperations = {
		PARAM_OTHER_PLAYER = "other",
		PARAM_EMERGENCY_TYPE = "emergency",
		ACCEPT_EMERGENCY = "accept",
	}
	if opts.noApi then PlayerOperations.ACCEPT_EMERGENCY = nil end
	UI.RequestPlayerOperation = function(pid, operation, parameters)
		state.requests[#state.requests + 1] = {
			pid = pid, operation = operation, other = parameters.other,
			emergency = parameters.emergency,
		}
		if opts.reenter then handler(opts.target == nil and -1 or opts.target,
			opts.eventType or TYPE) end
		if state.throws then error("host refused Lua call") end
	end
	return state
end

-- Firaxis's target-free popup sends PARAM_OTHER_PLAYER=-1. A synchronous
-- re-publish sees the turn guard before MemberIDs has been refreshed.
local good = fixture({ reenter = true })
registrations.EmergencyAvailable(-1, TYPE)
check("Climate Accords submits once", #good.requests, 1)
check("Climate Accords uses local player", good.requests[1].pid, PID)
check("Climate Accords uses accept operation", good.requests[1].operation, "accept")
check("Climate Accords keeps no-target sentinel", good.requests[1].other, -1)
check("Climate Accords keeps emergency type", good.requests[1].emergency, TYPE)
check("Climate Accords checked exact tracker player", good.trackerPlayer, PID)
check("Climate Accords reports submission, not membership", submitted(), true)
check("Climate Accords submission reason", reason(), "submitted")

-- Climate is independently default-on: an existing Aid opt-out cannot strand
-- its two-victory-point contest, and an absent new setting is equivalent to on.
local defaultOn = fixture({ aidEnabled = false, defaultEnabled = true })
registrations.EmergencyAvailable(-1, TYPE)
check("Climate defaults on independently of Aid", #defaultOn.requests, 1)
check("Climate default-on reports submitted", submitted(), true)

-- The notification is only a hint. The live row must be present, unbegun, and
-- not already contain our membership; a player target is not Climate Accords.
for _, blocked in ipairs({
	{ name = "missing tracker emergency", opts = { tracker = false }, why = "missing_emergency" },
	{ name = "begun emergency", opts = { begun = true }, why = "already_begun" },
	{ name = "existing member", opts = { members = { PID } }, why = "already_member" },
	{ name = "mismatched tracker row", opts = { crisis = {
		EmergencyType = TYPE + 1, TargetID = -1, HasBegun = false, MemberIDs = {},
	} }, why = "missing_emergency" },
	{ name = "player target", opts = { target = 11 }, why = "unexpected_target" },
	{ name = "missing host API", opts = { noApi = true }, why = "api_unavailable" },
}) do
	local state = fixture(blocked.opts)
	registrations.EmergencyAvailable(blocked.opts.target == nil and -1 or blocked.opts.target,
		TYPE)
	check(blocked.name .. " does not submit", #state.requests, 0)
	check(blocked.name .. " is named", reason(), blocked.why)
end

-- The feature defaults on but remains separately suppressible. A throwing
-- engine call clears the guard, making the next notification a real retry.
local disabled = fixture({ enabled = false })
registrations.EmergencyAvailable(-1, TYPE)
check("disabled controller does not submit", #disabled.requests, 0)
check("disabled controller does not emit an action", lastJoin(), nil)

local observer = fixture({ play = false })
registrations.EmergencyAvailable(-1, TYPE)
check("observer controller does not submit", #observer.requests, 0)

local failed = fixture({ throws = true, turn = 999 })
registrations.EmergencyAvailable(-1, TYPE)
check("throwing host call is not submitted", submitted(), false)
check("throwing host call is named", reason(), "throw")
failed.throws = false
registrations.EmergencyAvailable(-1, TYPE)
check("throwing host call can retry", #failed.requests, 2)
check("retry after throw submits", submitted(), true)

local other = fixture({ kind = "EMERGENCY_CONTROL_TEST_UNSUPPORTED" })
registrations.EmergencyAvailable(-1, TYPE)
check("other competition never submits", #other.requests, 0)
check("other competition does not touch tracker", other.trackerReads, 0)

local src = assert(io.open(here .. "/CivvisControlAgent.lua")):read("*a")
check("handler recognizes Climate Accords", src:find(
	'kind == "EMERGENCY_CLIMATE_ACCORDS"', 1, true) ~= nil, true)
check("handler retains Firaxis accept operation", src:find(
	"PlayerOperations.ACCEPT_EMERGENCY", 1, true) ~= nil, true)

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("all Climate Accords join checks passed")
