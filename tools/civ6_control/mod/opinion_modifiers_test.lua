-- Offline regression for `CivvisOpinionModifiers`: a rival's opinion of us,
-- reason by reason, as the shipped Intel tab reads it
-- (DiplomacyActionView.lua:895 `GetDiplomaticModifiers(ms_LocalPlayerID)`).
--
-- The export keeps each non-zero `{Text, Score}` row as `{text, score}` with
-- the LOC key untouched, drops zero rows as the screen does, and answers nil
-- (never an invented empty list) when the host cannot be asked.
--
-- Run: lua5.1 tools/civ6_control/mod/opinion_modifiers_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end
setmetatable(_G, { __index = function(_, key)
	if key == "CivvisOpinionModifiers" then
		return rawget(_G, key);
	end
	return stub()
end })

local realPrint = print
print = function() end

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtimeErr = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtimeErr))

local opinion = rawget(_G, "CivvisOpinionModifiers")

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		realPrint(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		realPrint(string.format("ok   %s = %s", name, tostring(got)))
	end
end

check("the opinion breakdown is exported", type(opinion), "function")

-- A rival whose AI answers `GetDiplomaticModifiers(asked)` with `rows`, and
-- records whom it was asked about.
local function rival(rows, noAi)
	local seen = {}
	local ai = {
		GetDiplomaticModifiers = function(_, about)
			seen.about = about
			return rows
		end,
	}
	return {
		GetDiplomaticAI = function()
			if noAi then return nil end
			return ai
		end,
	}, seen
end

local other, seen = rival({
	{ Text = "LOC_DIPLO_MODIFIER_DENOUNCED", Score = -9 },
	{ Text = "LOC_DIPLO_MODIFIER_DELEGATION", Score = 3 },
	{ Text = "LOC_TOOLTIP_DIPLOMACY_UNKNOWN_REASON", Score = 0 },
	{ Text = "LOC_DIPLO_MODIFIER_FIRST_IMPRESSION", Score = 1.5 },
})
local rows = opinion(other, 7)
check("the rival is asked about us", seen.about, 7)
check("zero rows are dropped, as the screen drops them", rows and #rows, 3)
check("the LOC key crosses untouched", rows and rows[1].text, "LOC_DIPLO_MODIFIER_DENOUNCED")
check("a negative score crosses", rows and rows[1].score, -9)
check("a positive score crosses", rows and rows[2].score, 3)
check("a fractional score crosses", rows and rows[3].score, 1.5)

local quiet = opinion((rival({})), 7)
check("no reasons is a known empty list", quiet and #quiet, 0)
check("an unreadable answer is unknown", opinion((rival(nil)), 7), nil)
check("no diplomatic AI is unknown", opinion((rival({}, true)), 7), nil)
local odd = opinion((rival({ { Text = nil, Score = "-4" } })), 7)
check("a numeric string score is read", odd and odd[1].score, -4)
check("a missing text is an empty key", odd and odd[1].text, "")

if failures > 0 then
	realPrint(string.format("%d failure(s)", failures))
	os.exit(1)
end
realPrint("all opinion_modifiers checks passed")
