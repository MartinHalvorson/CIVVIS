-- ⚠ lupa's Lua 5.1.5 barriers the CLOSURE in lua_setupvalue; a full
-- collection first leaves nothing marked (see congress_submission_test.lua).
do
	local setupvalue = debug.setupvalue
	debug.setupvalue = function(...)
		collectgarbage("collect")
		return setupvalue(...)
	end
end

-- The hand-written government ladder must not switch governments while
-- CIVVIS decides. CIVVIS owns the change (`government` orders); when it means
-- to stay it sends nothing, the prompt survives `civvis_complete`, and the
-- forfeit escape asks the ladder for one answer. "Newest unlocked" is the last
-- Governments row, so the seat landed in Democracy right after Suffrage in 11
-- of the October 5 live runs, and in Digital Democracy in 7.
--
-- Run: lua5.1 tools/civ6_control/mod/government_ladder_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."
local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end
setmetatable(_G, { __index = function() return stub() end })
CivvisControlConfig = { Play = true, CivvisDecides = true, MaxGovernmentPasses = 99 }
Automation = { Log = function() end }
local rows = {
	{ Index = 0, Hash = "H_CHIEFDOM", GovernmentType = "GOVERNMENT_CHIEFDOM" },
	{ Index = 1, Hash = "H_COMMUNISM", GovernmentType = "GOVERNMENT_COMMUNISM" },
	{ Index = 2, Hash = "H_DEMOCRACY", GovernmentType = "GOVERNMENT_DEMOCRACY" },
}
GameInfo = setmetatable({
	Governments = function()
		local i = 0
		return function() i = i + 1; return rows[i] end
	end,
}, { __index = function() return stub() end })

assert(loadfile(here .. "/CivvisControlAgent.lua"))()
local ladder = rawget(_G, "CivvisAnswerBlockerLadder")
assert(type(ladder) == "function", "CivvisControlAgent.lua did not export CivvisAnswerBlockerLadder")

local failures = 0
local function check(name, got, want)
	if got == want then
		print("ok   " .. name)
	else
		failures = failures + 1
		print(string.format("FAIL %s: got %s, want %s", name, tostring(got), tostring(want)))
	end
end

-- A seat under Communism (row 1) that has just unlocked Democracy (row 2).
local function seat()
	local state = { requested = nil, considered = false }
	local culture = {
		CanChangeGovernmentAtAll = function() return true end,
		GetCurrentGovernment = function() return 1 end,
		IsGovernmentUnlocked = function(_, hash) return hash ~= nil end,
		RequestChangeGovernment = function(_, hash) state.requested = hash end,
		SetGovernmentChangeConsidered = function(_, value) state.considered = value end,
	}
	local player = { GetCulture = function() return culture end }
	return state, player
end
local PROMPT = "ENDTURN_BLOCKING_CONSIDER_GOVERNMENT_CHANGE"

-- Under CIVVIS: the prompt is marked considered and nothing switches.
local s1, p1 = seat()
check("under CIVVIS the ladder answers the prompt", ladder(p1, 0, PROMPT, 180), "government considered")
check("under CIVVIS no government is requested", s1.requested, nil)
check("under CIVVIS the change is marked considered", s1.considered, true)

-- The escape hatch restores the old newest-unlocked pick.
CivvisControlConfig.GovernmentLadderSwitches = true
local s2, p2 = seat()
check("with the switch flag the ladder names the newest", ladder(p2, 0, PROMPT, 181), "GOVERNMENT_DEMOCRACY")
check("with the switch flag Democracy is requested", s2.requested, "H_DEMOCRACY")
CivvisControlConfig.GovernmentLadderSwitches = nil

-- Without CIVVIS deciding, the ladder still governs the seat itself.
CivvisControlConfig.CivvisDecides = false
local s3, p3 = seat()
check("without CIVVIS the ladder still switches", ladder(p3, 0, PROMPT, 182), "GOVERNMENT_DEMOCRACY")
check("without CIVVIS Democracy is requested", s3.requested, "H_DEMOCRACY")

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("\nall government-ladder checks passed")
