-- Offline regression for automatic World's Fair membership.
--
-- Firaxis scores every Great Person point at the target-free World's Fair.
-- CIVVIS already prices those project awards, but only members can score them,
-- so this invokes the actual event callback before the contest begins.
--
-- Run: lua5.1 tools/civ6_control/mod/worlds_fair_join_test.lua

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
	AutoJoinWorldsFair = true,
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

local PID, TYPE = 7, 91
local nextTurn = 400

local function lastJoin()
	for i = #logs, 1, -1 do
		if logs[i]:find('"kind":"worlds_fair_join"', 1, true) then
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
	CivvisControlConfig.AutoJoinClimateAccords = opts.climateEnabled ~= false
	if opts.defaultEnabled then
		CivvisControlConfig.AutoJoinWorldsFair = nil
	else
		CivvisControlConfig.AutoJoinWorldsFair = opts.enabled ~= false
	end
	GameInfo = { EmergencyAlliances = {} }
	if not opts.noDefinition then
		GameInfo.EmergencyAlliances[opts.eventType or TYPE] = {
			EmergencyType = opts.kind or "EMERGENCY_WORLDS_FAIR",
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
	-- World's Fair is target-free. It must not look up Players[-1] or diplomacy.
	Players = {}
	PlayerOperations = {
		PARAM_OTHER_PLAYER = "other",
		PARAM_EMERGENCY_TYPE = "emergency",
		ACCEPT_EMERGENCY = "accept",
	}
	if opts.noApi then PlayerOperations.PARAM_OTHER_PLAYER = nil end
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

-- The native popup uses the same accept operation and no-target sentinel.
-- A synchronous re-publish cannot submit a second request in the same turn.
local good = fixture({ reenter = true })
registrations.EmergencyAvailable(-1, TYPE)
check("World's Fair submits once", #good.requests, 1)
check("World's Fair uses local player", good.requests[1].pid, PID)
check("World's Fair uses accept operation", good.requests[1].operation, "accept")
check("World's Fair keeps no-target sentinel", good.requests[1].other, -1)
check("World's Fair keeps emergency type", good.requests[1].emergency, TYPE)
check("World's Fair checked exact tracker player", good.trackerPlayer, PID)
check("World's Fair reports submission, not membership", submitted(), true)
check("World's Fair submission reason", reason(), "submitted")

-- The new setting defaults on and is independent of the other competition
-- switches, so an existing opt-out cannot suppress this GPP score race.
local defaultOn = fixture({ aidEnabled = false, climateEnabled = false, defaultEnabled = true })
registrations.EmergencyAvailable(-1, TYPE)
check("World's Fair defaults on independently", #defaultOn.requests, 1)
check("World's Fair default-on reports submitted", submitted(), true)

-- An availability event alone is never authority. The current tracker row
-- must match, still require input, and not already contain our membership.
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

-- An explicit opt-out and observer mode do not mutate the host. A thrown host
-- request clears the turn guard so the next valid notification can retry.
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

local other = fixture({ kind = "EMERGENCY_SPACE_STATION" })
registrations.EmergencyAvailable(-1, TYPE)
check("other competition never submits", #other.requests, 0)
check("other competition does not touch tracker", other.trackerReads, 0)

local src = assert(io.open(here .. "/CivvisControlAgent.lua")):read("*a")
check("handler recognizes World's Fair", src:find(
	'kind == "EMERGENCY_WORLDS_FAIR"', 1, true) ~= nil, true)
check("handler retains Firaxis accept operation", src:find(
	"PlayerOperations.ACCEPT_EMERGENCY", 1, true) ~= nil, true)

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("all World's Fair join checks passed")
