-- Offline regression for the declared-friendship session.
--
-- `civvis_orders` sends a friendship-only deal as a `friendship` order with
-- the shipped session name `DECLARE_FRIEND` (DiplomacyActionView.lua:473).
-- The agent must ask only where the host would let a human ask — the action
-- is valid and the rival is Friendly — and at most once per cooldown window,
-- because every ask opens a leader scene the autoclose must dismiss.
--
-- Run: lua5.1 tools/civ6_control/mod/friendship_session_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

-- CivvisControlAgent.lua is a Civ 6 UI script.  Answer unrelated globals with
-- a permissive dummy while preserving the bare global exported for this test.
local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end
setmetatable(_G, { __index = function(_, key)
	if key == "CivvisApplyOrder" then return rawget(_G, key); end
	return stub()
end })

-- A real numeric cooldown, so the permissive dummy cannot mask that path.
CivvisControlConfig = { PeaceRetryTurns = 5, DealSessions = false }

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtimeErr = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtimeErr))

local applyOrder = rawget(_G, "CivvisApplyOrder")
assert(type(applyOrder) == "function",
	"CivvisControlAgent.lua did not export CivvisApplyOrder")

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", name, tostring(got)))
	end
end

-- The rival's view of us is a DiplomaticStates index, read back to its type.
GameInfo = { DiplomaticStates = {
	[0] = { StateType = "DIPLO_STATE_NEUTRAL" },
	[1] = { StateType = "DIPLO_STATE_FRIENDLY" },
	[2] = { StateType = "DIPLO_STATE_UNFRIENDLY" },
} }

local function fixture(opts)
	opts = opts or {}
	local state = { sessions = {} }
	DiplomacyManager = {
		RequestSession = function(from, to, name)
			state.sessions[#state.sessions + 1] = { from = from, to = to, name = name }
			if opts.sessionThrows then error("no session") end
		end,
	}
	Players = setmetatable({}, { __index = function()
		return { GetDiplomaticAI = function()
			if opts.stateUnreadable then error("no diplomatic AI") end
			return { GetDiplomaticStateIndex = function(_, toward)
				state.stateToward = toward
				return opts.stateIndex or 1
			end }
		end }
	end })
	local player = { GetDiplomacy = function()
		return {
			IsAtWarWith = function() return opts.atWar == true end,
			IsDiplomaticActionValid = function(_, action, target, testVisible)
				state.validity = { action = action, target = target, testVisible = testVisible }
				if opts.valid == "throw" then error("unavailable") end
				if opts.valid == nil then return true end
				return opts.valid
			end,
		}
	end }
	return state, player
end

local function ask(player, subject, turn)
	return applyOrder(player, 7, {
		kind = "friendship", subject = tostring(subject), verb = "DECLARE_FRIEND",
	}, turn)
end

-- A Friendly rival the host would let a human ask is asked, by the shipped
-- session name and not the action type's.
local friendly, friendlyPlayer = fixture()
local asked, reason = ask(friendlyPlayer, 11, 30)
check("a friendly rival is asked", asked, true)
check("the ask says asked", reason, "friendship_asked")
check("one session opened", #friendly.sessions, 1)
check("the session is DECLARE_FRIEND", friendly.sessions[1].name, "DECLARE_FRIEND")
check("the session runs from our seat", friendly.sessions[1].from, 7)
check("the session names the rival", friendly.sessions[1].to, 11)
check("validity asks the action type", friendly.validity.action, "DIPLOACTION_DECLARE_FRIENDSHIP")
check("validity names the rival", friendly.validity.target, 11)
check("validity asks as the view does", friendly.validity.testVisible, true)
check("the rival's view is of us", friendly.stateToward, 7)

-- A second ask inside the cooldown window opens nothing; after it, one more.
local again, againPlayer = fixture()
asked, reason = ask(againPlayer, 11, 33)
check("a repeat inside the window is held", asked, false)
check("the hold says cooldown", reason, "friendship_cooldown")
check("the hold opens no session", #again.sessions, 0)
asked = ask(againPlayer, 11, 35)
check("the window reopens after the cooldown", asked, true)

-- Anything the host would refuse opens no leader scene.
for _, case in ipairs({
	{ "at war", { atWar = true }, "friendship_at_war" },
	{ "an ask the host does not offer", { valid = false }, "friendship_not_offered" },
	{ "a neutral rival", { stateIndex = 0 }, "friendship_not_friendly" },
	{ "an unfriendly rival", { stateIndex = 2 }, "friendship_not_friendly" },
}) do
	local held, heldPlayer = fixture(case[2])
	asked, reason = ask(heldPlayer, 12, 40)
	check(case[1] .. " is not asked", asked, false)
	check(case[1] .. " says why", reason, case[3])
	check(case[1] .. " opens no session", #held.sessions, 0)
end

-- An unmapped rival is named, not guessed.
local unmapped, unmappedPlayer = fixture()
asked, reason = ask(unmappedPlayer, -1, 40)
check("an unmapped rival is not asked", asked, false)
check("the unmapped rival says why", reason, "friendship_target_unmapped")
check("the unmapped rival opens no session", #unmapped.sessions, 0)

-- A host that cannot answer a gate does not veto the ask; the verdict reads
-- the host's own friendship turn on the next frame either way.
local unknown, unknownPlayer = fixture({ valid = "throw", stateUnreadable = true })
asked = ask(unknownPlayer, 13, 40)
check("unknown gates still ask", asked, true)
check("unknown gates open one session", #unknown.sessions, 1)

-- A session that throws is reported and does not start the cooldown.
local thrown, thrownPlayer = fixture({ sessionThrows = true })
asked, reason = ask(thrownPlayer, 14, 40)
check("a throwing session is not an ask", asked, false)
check("a throwing session says throw", reason, "throw")
local retry, retryPlayer = fixture()
asked = ask(retryPlayer, 14, 41)
check("a thrown ask may retry at once", asked, true)
check("the retry opens a session", #retry.sessions, 1)

if failures > 0 then
	print(string.format("%d friendship session check(s) failed", failures))
	os.exit(1)
end
print("friendship session: shipped session name, host gates, cooldown and throw passed")
