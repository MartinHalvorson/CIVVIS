-- Offline test for the Great Work slot knowledge (`CivvisGreatWorks`).
--
-- ⚠ It loads the SHIPPED `CivvisControlAgent.lua` and calls the functions the
-- agent itself calls, following `envoy_spend_test.lua`. The regression this
-- pins: the old exporter's class->object constant spelt object types a way
-- Firaxis's database does not (`GREAT_WORK_OBJECT_WRITING` for
-- `GREATWORKOBJECT_WRITING`, and a `..._ART` that does not exist — Artists
-- create SCULPTURE/PORTRAIT/LANDSCAPE/RELIGIOUS works), so every cultural
-- person exported `empty_slots = 0` forever and the brain, which stands still
-- on 0 by design, froze them all. A test that re-implemented the mapping
-- would have passed while the agent kept the bad spelling, so this asserts
-- through the agent's own tables against slot rows spelt exactly as
-- `GreatWork_ValidSubTypes` ships them.
--
-- Run: lua tools/civ6_control/mod/great_work_slot_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

-- Any unknown global returns a table that is callable, indexable, and
-- comparable, so top-level code in the agent neither errors nor branches on it.
local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end
setmetatable(_G, { __index = function() return stub() end })

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))

-- ⚠⚠ REPORT THE PCALL RESULT — a chunk that dies at load must fail here, not
-- pass because the export happened first. See envoy_spend_test.lua.
local ran, runtime_err = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtime_err))

-- ⚠ A permissive stub CANNOT stand in for `GameInfo` here: the agent's lazy
-- individual->object build iterates `for row in GameInfo.GreatWorks() do`,
-- and a callable stub that never returns nil turns that into an infinite
-- loop inside `try` — pcall does not stop loops. Seed a terminating table
-- with one real row, which also exercises the per-individual path.
rawset(_G, "GameInfo", setmetatable({
	GreatWorks = function()
		local rows = {
			{ GreatPersonIndividualType = "GREAT_PERSON_INDIVIDUAL_QIU_YING",
			  GreatWorkObjectType = "GREATWORKOBJECT_LANDSCAPE" },
		}
		local i = 0
		return function() i = i + 1; return rows[i] end
	end,
}, { __index = function() return stub() end }))

-- ⚠ `rawget`, not `_G.x`: the agent must never be able to pass this test by
-- doing something the game's sandbox forbids.
local gw = rawget(_G, "CivvisGreatWorks")
assert(type(gw) == "table",
	"CivvisControlAgent.lua did not export CivvisGreatWorks")
assert(type(gw.objectsFor) == "function", "objectsFor missing")
assert(type(gw.matches) == "function", "matches missing")

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", name, tostring(got)))
	end
end

-- The individual row outranks the class fallback: Qiu Ying paints landscapes,
-- whatever the Artist class could otherwise produce.
local ying = gw.objectsFor("GREAT_PERSON_INDIVIDUAL_QIU_YING",
	"GREAT_PERSON_CLASS_ARTIST")
check("a known individual answers exactly",
	ying ~= nil and ying.GREATWORKOBJECT_LANDSCAPE == true
		and ying.GREATWORKOBJECT_SCULPTURE == nil, true)

-- An individual the table does not know falls back to the class — the path a
-- DLC person under a foreign ruleset takes live.
local writer = gw.objectsFor("GREAT_PERSON_INDIVIDUAL_UNKNOWN_TO_THE_TABLE",
	"GREAT_PERSON_CLASS_WRITER")
check("writer creates WRITING",
	writer ~= nil and writer.GREATWORKOBJECT_WRITING == true, true)
local artist = gw.objectsFor(nil, "GREAT_PERSON_CLASS_ARTIST")
check("artist creates LANDSCAPE",
	artist ~= nil and artist.GREATWORKOBJECT_LANDSCAPE == true, true)
check("artist creates SCULPTURE",
	artist ~= nil and artist.GREATWORKOBJECT_SCULPTURE == true, true)
check("no invented ART object",
	artist ~= nil and artist.GREATWORKOBJECT_ART == nil, true)
local merchant = gw.objectsFor(nil, "GREAT_PERSON_CLASS_MERCHANT")
check("a merchant consumes no slot", merchant == nil, true)

-- Slot rows spelt exactly as `GreatWork_ValidSubTypes` ships them: a writing
-- slot on a known tile, an art slot on a known tile, a palace-kind slot in a
-- wonder (no tile). The survey shape is the one `CivvisGreatWorks.survey`
-- builds; `matches` must count the compatible ones and name only the tiles it
-- knows.
local survey = {
	slots = {
		{ accepts = { GREATWORKOBJECT_WRITING = true }, plot = 101 },
		{ accepts = {
			GREATWORKOBJECT_SCULPTURE = true,
			GREATWORKOBJECT_PORTRAIT = true,
			GREATWORKOBJECT_LANDSCAPE = true,
			GREATWORKOBJECT_RELIGIOUS = true,
		}, plot = 202 },
		{ accepts = { GREATWORKOBJECT_MUSIC = true, GREATWORKOBJECT_WRITING = true },
			plot = nil },
	},
	district_plots = { [101] = true, [202] = true },
}

local count, plots = gw.matches(survey, writer)
check("writer matches two slots", count, 2)
check("writer knows the writing tile", plots ~= nil and plots[101] == true, true)
check("writer does not claim the art tile", plots ~= nil and plots[202], nil)

count, plots = gw.matches(survey, artist)
check("artist matches one slot", count, 1)
check("artist knows the art tile", plots ~= nil and plots[202] == true, true)

count = gw.matches(survey, gw.objectsFor(nil, "GREAT_PERSON_CLASS_MUSICIAN"))
check("musician matches the wonder slot it cannot map", count, 1)

-- The starved case the brain stands still on: a full board matches nothing.
count, plots = gw.matches({ slots = {}, district_plots = { [101] = true } }, writer)
check("no empty slots is an honest zero", count, 0)

-- And the two unknowns stay unknowable, never zero: 0 is a claim.
check("no survey is nil", gw.matches(nil, writer), nil)
check("no objects is nil", gw.matches(survey, nil), nil)

-- Exercise the survey itself against the shipped column names, not a
-- hand-built survey. Buildings.xml:113 names PrereqDistrict="DISTRICT_THEATER"
-- for the Amphitheater. Reading PrerequisiteDistrict silently counted empty
-- slots while locating none, so every highlighted Acropolis was called full.
local function rows(values)
	return function()
		local i = 0
		return function() i = i + 1; return values[i] end
	end
end
local occupied = false
local districtType = 1
local blds = {
	HasBuilding = function() return true end,
	GetNumGreatWorkSlots = function(_, index) return index == 1 and 2 or 1 end,
	GetGreatWorkInSlot = function(_, index)
		return (occupied or index == 2) and 42 or -1
	end,
	GetGreatWorkSlotType = function() return 0 end,
}
local city = { GetBuildings = function() return blds end }
local player = { GetCities = function()
	return { Members = function() return ipairs({city}) end }
end }
rawset(_G, "GameInfo", {
	GreatWork_ValidSubTypes = rows({{
		GreatWorkSlotType = "GREATWORKSLOT_WRITING",
		GreatWorkObjectType = "GREATWORKOBJECT_WRITING",
	}}),
	DistrictReplaces = rows({{
		CivUniqueDistrictType = "DISTRICT_ACROPOLIS",
		ReplacesDistrictType = "DISTRICT_THEATER",
	}}),
	Districts = {
		[0] = {DistrictType = "DISTRICT_CITY_CENTER"},
		[1] = {DistrictType = "DISTRICT_ACROPOLIS"},
		[2] = {DistrictType = "DISTRICT_THEATER"},
	},
	Buildings = rows({
		{Index = 1, BuildingType = "BUILDING_AMPHITHEATER", PrereqDistrict = "DISTRICT_THEATER"},
		{Index = 2, BuildingType = "BUILDING_PALACE", PrereqDistrict = "DISTRICT_CITY_CENTER"},
		-- A building with no district remains an unknown-location slot.
		{Index = 3, BuildingType = "TEST_WONDER"},
	}),
	GreatWorkSlotTypes = {[0] = {GreatWorkSlotType = "GREATWORKSLOT_WRITING"}},
})
rawset(_G, "Map", {
	GetCityPlots = function()
		return {GetPurchasedPlots = function() return {101, 202} end}
	end,
	GetPlotByIndex = function(index)
		return {
			GetDistrictType = function() return index == 101 and 0 or districtType end,
			GetOwner = function() return 0 end,
			GetX = function() return index end,
			GetY = function() return 0 end,
		}
	end,
})
for _, kind in ipairs({1, 2}) do
	districtType = kind
	local actual = gw.survey(player, 100 + kind)
	count, plots = gw.matches(actual, writer)
	check("survey counts real slots for district " .. kind, count, 3)
	check("survey maps Amphitheater to district " .. kind, plots[202], true)
	check("full Palace stays unavailable for district " .. kind, plots[101], nil)
	check("unknown wonder location stays unknown", actual.slots[3].plot, nil)
	local exported = rawget(_G, "CivvisGreatPersonActivationPlots")({}, {
		GetActivationHighlightPlots = function() return {101, 202} end,
	}, 0, actual, plots)
	check("export marks full Palace closed", exported[1].slot_open, false)
	check("export marks matching theater open", exported[2].slot_open, true)
end
occupied = true
count, plots = gw.matches(gw.survey(player, 103), writer)
check("survey of occupied buildings has no free slot", count, 0)
check("occupied theater is not an activation destination", plots[202], nil)

if failures > 0 then
	print(string.format("%d failure(s)", failures))
	os.exit(1)
end
print("all checks passed")
