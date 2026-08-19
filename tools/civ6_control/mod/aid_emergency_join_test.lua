-- Offline regression for automatic Aid Request membership.
--
-- The host fires Events.EmergencyAvailable before an emergency begins. CIVVIS
-- should press the same ACCEPT_EMERGENCY operation as Firaxis's World Crisis
-- popup, but only for Aid Requests it can later score. This loads the actual
-- agent and invokes the callback registered with the actual event table.
--
-- Run: lua5.1 tools/civ6_control/mod/aid_emergency_join_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end

-- The full agent names a broad Civ VI surface while it loads. Keep every
-- unrelated name harmless, but retain the handler and the live-style event
-- registration this test exercises.
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
CivvisControlConfig = { Play = true, AutoJoinAidRequests = true }

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtimeErr = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtimeErr))

local handler = rawget(_G, "CivvisOnAidEmergencyAvailable")
assert(type(handler) == "function",
	"CivvisControlAgent.lua did not export CivvisOnAidEmergencyAvailable")
assert(registrations.EmergencyAvailable == handler,
	"CivvisControlAgent.lua did not register the Aid callback on EmergencyAvailable")

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", name, tostring(got)))
	end
end

local PID, TARGET, TYPE = 7, 11, 41
local nextTurn = 100

local function lastJoin()
	for i = #logs, 1, -1 do
		if logs[i]:find('"kind":"aid_emergency_join"', 1, true) then return logs[i] end
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
	local state = { requests = {}, trackerReads = 0, turn = opts.turn or nextTurn,
		throws = opts.throws or false }
	local crisis = opts.crisis
	if crisis == nil and opts.tracker ~= false then
		crisis = {
			EmergencyType = opts.eventType or TYPE,
			TargetID = opts.target or TARGET,
			HasBegun = opts.begun or false,
			MemberIDs = opts.members or {},
		}
	end

	CivvisControlConfig.Play = opts.play ~= false
	CivvisControlConfig.AutoJoinAidRequests = opts.enabled ~= false
	GameInfo = { EmergencyAlliances = {} }
	if not opts.noDefinition then
		GameInfo.EmergencyAlliances[opts.eventType or TYPE] = {
			EmergencyType = opts.kind or "EMERGENCY_SEND_AID",
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
	local diplomacy = {
		HasMet = function(_, target)
			state.metTarget = target
			return opts.met ~= false
		end,
		IsAtWarWith = function(_, target)
			state.warTarget = target
			return opts.war or false
		end,
	}
	Players = {
		[PID] = { GetDiplomacy = function() return diplomacy end },
		[TARGET] = { IsMajor = function() return opts.major ~= false end },
	}
	PlayerOperations = {
		PARAM_OTHER_PLAYER = "other",
		PARAM_EMERGENCY_TYPE = "emergency",
		ACCEPT_EMERGENCY = "accept",
	}
	if opts.noApi then PlayerOperations.PARAM_EMERGENCY_TYPE = nil end
	UI.RequestPlayerOperation = function(pid, operation, parameters)
		state.requests[#state.requests + 1] = {
			pid = pid, operation = operation, other = parameters.other,
			emergency = parameters.emergency,
		}
		if opts.reenter then handler(opts.target or TARGET, opts.eventType or TYPE) end
		if state.throws then error("host refused Lua call") end
	end
	return state
end

-- A qualifying disaster Aid Request reaches the exact host operation. A
-- synchronous re-publish of the event sees the guard first, so it cannot send
-- a duplicate accept before MemberIDs has refreshed.
local good = fixture({ reenter = true })
registrations.EmergencyAvailable(TARGET, TYPE)
check("Aid request submits once", #good.requests, 1)
check("Aid request uses local player", good.requests[1].pid, PID)
check("Aid request uses accept operation", good.requests[1].operation, "accept")
check("Aid request keeps target", good.requests[1].other, TARGET)
check("Aid request keeps emergency type", good.requests[1].emergency, TYPE)
check("Aid request checked exact tracker player", good.trackerPlayer, PID)
check("Aid request reports submission, not membership", submitted(), true)
check("Aid request submission reason", reason(), "submitted")

-- Both score-capable Aid Request variants qualify. Nothing else is allowed to
-- turn an availability notification into an unpriced strategic commitment.
local military = fixture({ kind = "EMERGENCY_SEND_MILITARY_AID" })
registrations.EmergencyAvailable(TARGET, TYPE)
check("military Aid submits", #military.requests, 1)
check("military Aid reports submitted", submitted(), true)

local other = fixture({ kind = "EMERGENCY_WORLD_GAMES" })
registrations.EmergencyAvailable(TARGET, TYPE)
check("non-Aid never submits", #other.requests, 0)
check("non-Aid does not touch tracker", other.trackerReads, 0)
check("non-Aid is named", reason(), "not_aid_request")

-- The event alone is not authority: the current emergency must match the
-- target and type, still require input, and not already include our seat.
local absent = fixture({ tracker = false })
registrations.EmergencyAvailable(TARGET, TYPE)
check("missing tracker emergency does not submit", #absent.requests, 0)
check("missing tracker emergency is named", reason(), "missing_emergency")

local begun = fixture({ begun = true })
registrations.EmergencyAvailable(TARGET, TYPE)
check("begun emergency does not submit", #begun.requests, 0)
check("begun emergency is named", reason(), "already_begun")

local member = fixture({ members = { PID } })
registrations.EmergencyAvailable(TARGET, TYPE)
check("existing member does not submit", #member.requests, 0)
check("existing member is named", reason(), "already_member")

local mismatch = fixture({ crisis = {
	EmergencyType = TYPE + 1, TargetID = TARGET, HasBegun = false, MemberIDs = {},
} })
registrations.EmergencyAvailable(TARGET, TYPE)
check("mismatched tracker row does not submit", #mismatch.requests, 0)
check("mismatched tracker row is named", reason(), "missing_emergency")

-- Aid membership is broader than the final Gold-deal fallback: an unmet or
-- city-state recipient can still make PROJECT_SEND_AID available. Only a live
-- war is actively harmful to score and must stop the acceptance.
local unknown = fixture({ met = false, major = false })
registrations.EmergencyAvailable(TARGET, TYPE)
check("unmet non-major still unlocks project route", #unknown.requests, 1)
check("unmet non-major reports submitted", submitted(), true)

for _, blocked in ipairs({
	{ name = "war", opts = { war = true }, why = "at_war" },
	{ name = "missing API", opts = { noApi = true }, why = "api_unavailable" },
}) do
	local state = fixture(blocked.opts)
	registrations.EmergencyAvailable(TARGET, TYPE)
	check(blocked.name .. " does not submit", #state.requests, 0)
	check(blocked.name .. " is named", reason(), blocked.why)
end

-- A disabled or observer controller does not mutate the game. A throwing host
-- call clears the in-turn guard, so it may be retried rather than being marked
-- as an imagined accept.
local disabled = fixture({ enabled = false })
registrations.EmergencyAvailable(TARGET, TYPE)
check("disabled controller does not submit", #disabled.requests, 0)
check("disabled controller does not emit an action", lastJoin(), nil)

local observer = fixture({ play = false })
registrations.EmergencyAvailable(TARGET, TYPE)
check("observer controller does not submit", #observer.requests, 0)

local failed = fixture({ throws = true, turn = 999 })
registrations.EmergencyAvailable(TARGET, TYPE)
check("throwing host call is not submitted", submitted(), false)
check("throwing host call is named", reason(), "throw")
failed.throws = false
registrations.EmergencyAvailable(TARGET, TYPE)
check("throwing host call can retry", #failed.requests, 2)
check("retry after throw submits", submitted(), true)

local src = assert(io.open(here .. "/CivvisControlAgent.lua")):read("*a")
check("handler is wired to the real host event", src:find(
	"EmergencyAvailable = CivvisOnAidEmergencyAvailable", 1, true) ~= nil, true)
check("handler uses Firaxis accept operation", src:find(
	"PlayerOperations.ACCEPT_EMERGENCY", 1, true) ~= nil, true)

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("all Aid Request join checks passed")
