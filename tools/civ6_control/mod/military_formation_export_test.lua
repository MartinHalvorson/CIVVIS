-- The military formation tier the mod exports, and the two ways it could have
-- been silently wrong.
--
-- ⚠ Loads the SHIPPED `CivvisControlAgent.lua` and calls the agent's own
-- `militaryFormation`. A test that re-implemented the mapping would pass while
-- the agent kept reading the wrong enum.
--
-- What is checked:
--   1. STANDARD/CORPS/ARMY map to CIVVIS's 0/1/2;
--   2. the mapping goes through the ENUM, not through the raw integer, so a
--      build that numbers the tiers differently still lands correctly;
--   3. ⚠ EITHER SPELLING of the enum works. Civilization VI registers a table
--      called `MilitaryFormationTypes` in BOTH of its Lua VMs with DIFFERENT
--      members -- `*_FORMATION` in `GameCore_Base.dll`, `*_MILITARY_FORMATION`
--      in `Civ6_Exe_Child` -- and this script sees globals from both. Betting on
--      one and losing would classify every Corps as unknown forever;
--   4. a missing accessor, a nil answer, a raise, an out-of-range tier and an
--      absent enum table all export -1, which the mirror refuses; none of them
--      may read as STANDARD, because 0 is a legal tier and a fallback that
--      claims it is the `GetDefenseStrength` sentinel all over again.
--
-- Run: lua5.1 tools/civ6_control/mod/military_formation_export_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end

-- The enum table the agent will read. The test owns it so the "this build has no
-- such table" case is reachable at all -- a blanket stub would answer every key.
local ENUM = nil
setmetatable(_G, { __index = function(_, k)
	if k == "MilitaryFormationTypes" then return ENUM end
	if k == "CivvisMilitaryFormation" then return rawget(_G, k) end
	return stub()
end })

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
-- ⚠ REPORT THE PCALL RESULT. A chunk that dies at load must fail this test, not
-- pass because the export happened first.
local ran, runtime_err = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtime_err))

local formationOf = rawget(_G, "CivvisMilitaryFormation")
assert(type(formationOf) == "function",
	"CivvisControlAgent.lua did not export CivvisMilitaryFormation")

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", name, tostring(got)))
	end
end

-- A unit whose accessor answers exactly what the host would.
local function unitReporting(tier)
	return { GetMilitaryFormation = function() return tier end }
end

-- ---------------------------------------------------------------- the mapping
--
-- The three members registered by `GameCore_Base.dll`, in the order the binary
-- lists them immediately after the `MilitaryFormationTypes` string itself.
ENUM = { STANDARD_FORMATION = 0, CORPS_FORMATION = 1, ARMY_FORMATION = 2 }
check("standard is tier 0", formationOf(unitReporting(0)), 0)
check("corps is tier 1", formationOf(unitReporting(1)), 1)
check("army is tier 2", formationOf(unitReporting(2)), 2)

-- ⚠ THE ENUM IS THE AUTHORITY, NOT THE INTEGER. Nothing shipped promises these
-- ordinals, and the whole point of comparing against the table -- exactly as
-- `WorldTracker.lua:512-520` does -- is that a build which numbers them
-- differently still lands on the right tier instead of on a plausible wrong one.
ENUM = { STANDARD_FORMATION = 40, CORPS_FORMATION = 41, ARMY_FORMATION = 42 }
check("renumbered standard still maps", formationOf(unitReporting(40)), 0)
check("renumbered corps still maps", formationOf(unitReporting(41)), 1)
check("renumbered army still maps", formationOf(unitReporting(42)), 2)
check("a value outside the renumbered enum is unknown", formationOf(unitReporting(2)), -1)

-- ------------------------------------------------------------- the two traps
--
-- ⚠ TRAP 1: THE OTHER SPELLING, WHICH IS ALSO A REAL ONE. `Civ6_Exe_Child`
-- registers `MilitaryFormationTypes` with `*_MILITARY_FORMATION` members and
-- `GameCore_Base.dll` registers the same table name with `*_FORMATION` members;
-- this script is an `AddUserInterfaces` context and sees globals from both VMs.
-- Firaxis' own UI uses both families -- `WorldTracker.lua:512-520` the short
-- one, `ProductionPanel.lua:314-456` the long one -- so at least one of them is
-- comparing against nil somewhere. Reading a Corps must work through EITHER.
ENUM = {
	STANDARD_MILITARY_FORMATION = 0,
	CORPS_MILITARY_FORMATION = 1,
	ARMY_MILITARY_FORMATION = 2,
}
check("the long spelling maps standard", formationOf(unitReporting(0)), 0)
check("the long spelling maps corps", formationOf(unitReporting(1)), 1)
check("the long spelling maps army", formationOf(unitReporting(2)), 2)

-- Both registered at once, which is what a VM carrying the two tables merged
-- would look like. The tiers agree, so nothing is ambiguous.
ENUM = {
	STANDARD_FORMATION = 0, CORPS_FORMATION = 1, ARMY_FORMATION = 2,
	STANDARD_MILITARY_FORMATION = 0, CORPS_MILITARY_FORMATION = 1,
	ARMY_MILITARY_FORMATION = 2,
}
check("both spellings present still maps corps", formationOf(unitReporting(1)), 1)
check("both spellings present still maps army", formationOf(unitReporting(2)), 2)

-- ⚠ And a table with NEITHER family is unknown, not standard.
ENUM = { SOME_OTHER_ENUM = 0 }
check("neither spelling yields unknown, not standard", formationOf(unitReporting(0)), -1)

-- ⚠ TRAP 2: THE SENTINEL THAT READS AS AN ANSWER. `GetDefenseStrength` returned
-- its -1 fallback for the project's whole life and nobody could tell. 0 is a
-- legal tier here, so a fallback of 0 would be worse: it would assert that every
-- unit is a plain unit on a build where nothing could be read.
ENUM = { STANDARD_FORMATION = 0, CORPS_FORMATION = 1, ARMY_FORMATION = 2 }
check("no accessor is unknown", formationOf({}), -1)
check("a nil answer is unknown", formationOf(unitReporting(nil)), -1)
check("a tier we do not model is unknown", formationOf(unitReporting(7)), -1)
check("an accessor that raises is unknown", formationOf({
	GetMilitaryFormation = function() error("no such method") end,
}), -1)

ENUM = nil
check("no enum table at all is unknown, not standard", formationOf(unitReporting(0)), -1)

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("\nall military-formation export checks passed")
