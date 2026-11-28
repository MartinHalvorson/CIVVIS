-- What a CIVVIS seat did with the person's deal, written for the cover
-- (CivvisControlAgent.lua `CivvisTrade.seatNote`, called by `seatSend` for an
-- answer sent to the person; CivvisHotseatCover.lua shows it).
-- Run: lua5.1 tools/civ6_control/mod/hotseat_deal_note_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."
local handle = assert(io.open(here .. "/CivvisControlAgent.lua", "r"))
local source = handle:read("*a")
handle:close()
local note = assert(source:match("\n(CivvisTrade%.seatNote = function%(pid, other, session, turn%).-\nend;\n)"),
	"seatNote is where it was")
-- Called only for an answer that was sent, and only to the person.
local send = assert(source:match("\n(CivvisTrade%.seatSend = function.-\nend;\n)"))
assert(send:find('\tif sent and CivvisTrade.seatKind(other) == "person" then\n\t\tpcall(CivvisTrade.seatNote, pid, other, session, turn);\n\tend\n', 1, true),
	"seatSend writes the note for a sent answer to the person")

local env = setmetatable({ CivvisTrade = {}, ExposedMembers = {} }, { __index = _G })
local chunk = assert(loadstring(note, "seatNote"))
setfenv(chunk, env)
chunk()
env.CivvisTrade.seatNote(3, 0, { verb = "ACCEPT", id = 77 }, 12)
local notes = env.ExposedMembers.CivvisDealNotes
assert(type(notes) == "table" and notes.n == 1 and #notes == 1)
assert(notes[1].n == 1 and notes[1].seat == 3 and notes[1].to == 0 and notes[1].verb == "ACCEPT"
	and notes[1].turn == 12 and notes[1].id == 77)
-- The last eight, numbered on.
for i = 1, 9 do env.CivvisTrade.seatNote(4, 0, { verb = "REJECT", id = i }, 13) end
assert(notes.n == 10 and #notes == 8 and notes[1].n == 3 and notes[8].n == 10, notes.n .. " " .. #notes)
-- Another context's table is reused, not replaced.
env.ExposedMembers.CivvisDealNotes = { n = 41 }
env.CivvisTrade.seatNote(5, 0, { verb = "COUNTER" }, 14)
assert(env.ExposedMembers.CivvisDealNotes[1].n == 42)

print("hotseat_deal_note_test: ok")
