-- `rivals[].person`: on a hotseat install, a rival listed in
-- `HotseatHumanSeats` (the seat the harness gives the person) carries
-- `person = true`; every other rival, and every rival on any install without
-- `Hotseat`, carries nothing, so a single-player or ladder board is unchanged.
-- The decider's opt-in `person-units-threaten-civilians` reads it.
--
-- The field is read from the shipped source and evaluated on its own: the
-- full board export walks the whole host API (see export_timing_test.lua).
--
-- Run: lua5.1 tools/civ6_control/mod/rival_person_export_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()

-- The field sits in the rival's row, the table `rivals[#rivals + 1]` builds,
-- once, between the rival's wars and its border facts.
local row = assert(source:find("rivals[#rivals + 1] = {", 1, true), "the rivals export")
local wars = assert(source:find("at_war_with = CivvisRivalWars(pid, otherId),", row, true))
local first, last, field = source:find("\n\t\t\t\t(person = %(function%(%).-end%)%(%),)", wars)
assert(first, "rivals[].person is exported")
local borders = assert(source:find("open_borders = try(function()", row, true))
assert(first > wars and last < borders, "the field is inside the rival's row")
assert(not source:find("\n\t\t\t\tperson = ", last, true), "exported once")
assert(not field:find("_G", 1, true), "no _G in mod Lua")

local function person(cfg, otherId)
	local chunk = assert(loadstring("return { " .. field .. " }"))
	setfenv(chunk, { cfg = cfg, otherId = otherId, ipairs = ipairs, tonumber = tonumber })
	return chunk().person
end

local hotseat = { Hotseat = true, HotseatHumanSeats = { 0 }, HotseatSeats = { 1, 2 } }
assert(person(hotseat, 0) == true, "the person's seat is a person")
assert(person(hotseat, 1) == nil, "a CIVVIS seat is not")
assert(person(hotseat, 2) == nil)
-- Seat ids as the installer writes them, strings included.
assert(person({ Hotseat = true, HotseatHumanSeats = { "3" } }, 3) == true)
assert(person({ Hotseat = true, HotseatHumanSeats = { 0, 4 } }, 4) == true)
-- `--self-play` lists no person; a stub-only table lists none either.
assert(person({ Hotseat = true, HotseatHumanSeats = {} }, 0) == nil)
assert(person({ Hotseat = true }, 0) == nil)
-- Not a hotseat install: nothing, whatever the list says.
assert(person({}, 0) == nil, "single player / ladder: no field")
assert(person({ Hotseat = false, HotseatHumanSeats = { 0 } }, 0) == nil)
assert(person({ HotseatHumanSeats = { 0 } }, 0) == nil)

print("rival_person_export_test: ok")
