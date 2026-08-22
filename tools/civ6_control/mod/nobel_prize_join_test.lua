-- Offline regression for automatic Nobel Prize competition membership.
--
-- Firaxis makes Literature, Peace, and Physics target-free, universal
-- competitions. CIVVIS already prices Literature and Physics Great Person
-- scores, and Peace can award a Diplomatic Victory Point, so exercise the
-- actual emergency callback before any of those contests begins.
--
-- Run: lua5.1 tools/civ6_control/mod/nobel_prize_join_test.lua

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
	AutoJoinWorldsFair = true, AutoJoinWorldGames = true,
	AutoJoinSpaceStation = true, AutoJoinNobelPrizes = true,
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

local PID = 7
local prizes = {
	{ label = "Literature", kind = "EMERGENCY_NOBEL_PRIZE_LITERATURE", type = 101 },
	{ label = "Peace", kind = "EMERGENCY_NOBEL_PRIZE_PEACE", type = 102 },
	{ label = "Physics", kind = "EMERGENCY_NOBEL_PRIZE_PHYSICS", type = 103 },
}
local nextTurn = 700

local function lastJoin()
	for i = #logs, 1, -1 do
		if logs[i]:find('"kind":"nobel_prize_join"', 1, true) then
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
	local prize = prizes[1]
	local eventType = opts.eventType or prize.type
	local kind = opts.kind or prize.kind
	local state = {
		requests = {}, trackerReads = 0, turn = opts.turn or nextTurn,
		throws = opts.throws or false,
	}
	local crisis = opts.crisis
	if crisis == nil and opts.tracker ~= false then
		crisis = {
			EmergencyType = eventType,
			TargetID = opts.target == nil and -1 or opts.target,
			HasBegun = opts.begun or false,
			MemberIDs = opts.members or {},
		}
	end

	CivvisControlConfig.Play = opts.play ~= false
	CivvisControlConfig.AutoJoinAidRequests = opts.aidEnabled ~= false
	CivvisControlConfig.AutoJoinClimateAccords = opts.climateEnabled ~= false
	CivvisControlConfig.AutoJoinWorldsFair = opts.fairEnabled ~= false
	CivvisControlConfig.AutoJoinWorldGames = opts.gamesEnabled ~= false
	CivvisControlConfig.AutoJoinSpaceStation = opts.spaceEnabled ~= false
	if opts.defaultEnabled then
		CivvisControlConfig.AutoJoinNobelPrizes = nil
	else
		CivvisControlConfig.AutoJoinNobelPrizes = opts.enabled ~= false
	end
	GameInfo = { EmergencyAlliances = {} }
	if not opts.noDefinition then
		GameInfo.EmergencyAlliances[eventType] = { EmergencyType = kind }
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
	-- Nobel competitions are target-free. They must not inspect Players[-1] or diplomacy.
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
		if opts.reenter then handler(opts.target == nil and -1 or opts.target, eventType) end
		if state.throws then error("host refused Lua call") end
	end
	return state
end

-- The native popup uses the same accept operation and no-target sentinel. A
-- synchronous re-publish cannot submit a second request in the same turn.
for _, prize in ipairs(prizes) do
	local good = fixture({ eventType = prize.type, kind = prize.kind, reenter = true })
	registrations.EmergencyAvailable(-1, prize.type)
	check(prize.label .. " submits once", #good.requests, 1)
	check(prize.label .. " uses local player", good.requests[1].pid, PID)
	check(prize.label .. " uses accept operation", good.requests[1].operation, "accept")
	check(prize.label .. " keeps no-target sentinel", good.requests[1].other, -1)
	check(prize.label .. " keeps emergency type", good.requests[1].emergency, prize.type)
	check(prize.label .. " checked exact tracker player", good.trackerPlayer, PID)
	check(prize.label .. " reports submission, not membership", submitted(), true)
	check(prize.label .. " reports exact competition", lastJoin():find(
		'"emergency":"' .. prize.kind .. '"', 1, true) ~= nil, true)
end

-- One default-on switch covers all three contests and stays independent from
-- every earlier competition setting, so a previous opt-out cannot strand them.
for _, prize in ipairs(prizes) do
	local defaultOn = fixture({
		eventType = prize.type, kind = prize.kind, aidEnabled = false,
		climateEnabled = false, fairEnabled = false, gamesEnabled = false,
		spaceEnabled = false, defaultEnabled = true,
	})
	registrations.EmergencyAvailable(-1, prize.type)
	check(prize.label .. " defaults on independently", #defaultOn.requests, 1)
	check(prize.label .. " default-on reports submitted", submitted(), true)
end

-- An availability event alone is never authority. The current tracker row
-- must match, still require input, and not already contain our membership.
for _, blocked in ipairs({
	{ name = "missing tracker emergency", opts = { tracker = false }, why = "missing_emergency" },
	{ name = "begun emergency", opts = { begun = true }, why = "already_begun" },
	{ name = "existing member", opts = { members = { PID } }, why = "already_member" },
	{ name = "mismatched tracker row", opts = { crisis = {
		EmergencyType = prizes[1].type + 1, TargetID = -1, HasBegun = false, MemberIDs = {},
	} }, why = "missing_emergency" },
	{ name = "player target", opts = { target = 11 }, why = "unexpected_target" },
	{ name = "missing host API", opts = { noApi = true }, why = "api_unavailable" },
}) do
	local state = fixture(blocked.opts)
	registrations.EmergencyAvailable(blocked.opts.target == nil and -1 or blocked.opts.target,
		prizes[1].type)
	check(blocked.name .. " does not submit", #state.requests, 0)
	check(blocked.name .. " is named", reason(), blocked.why)
end

-- An explicit opt-out and observer mode do not mutate the host. A thrown host
-- request clears the turn guard so the next valid notification can retry.
for _, prize in ipairs(prizes) do
	local disabled = fixture({ eventType = prize.type, kind = prize.kind, enabled = false })
	registrations.EmergencyAvailable(-1, prize.type)
	check(prize.label .. " opt-out does not submit", #disabled.requests, 0)
	check(prize.label .. " opt-out does not emit an action", lastJoin(), nil)
end

local observer = fixture({ play = false })
registrations.EmergencyAvailable(-1, prizes[1].type)
check("observer controller does not submit", #observer.requests, 0)

local failed = fixture({ throws = true, turn = 999 })
registrations.EmergencyAvailable(-1, prizes[1].type)
check("throwing host call is not submitted", submitted(), false)
check("throwing host call is named", reason(), "throw")
failed.throws = false
registrations.EmergencyAvailable(-1, prizes[1].type)
check("throwing host call can retry", #failed.requests, 2)
check("retry after throw submits", submitted(), true)

local other = fixture({ kind = "EMERGENCY_CONTROL_TEST_UNSUPPORTED" })
registrations.EmergencyAvailable(-1, prizes[1].type)
check("other competition never submits", #other.requests, 0)
check("other competition does not touch tracker", other.trackerReads, 0)

local src = assert(io.open(here .. "/CivvisControlAgent.lua")):read("*a")
for _, prize in ipairs(prizes) do
	check("handler recognizes " .. prize.label, src:find(
		'kind == "' .. prize.kind .. '"', 1, true) ~= nil, true)
end
check("handler retains Firaxis accept operation", src:find(
	"PlayerOperations.ACCEPT_EMERGENCY", 1, true) ~= nil, true)

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("all Nobel Prize join checks passed")
