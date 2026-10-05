-- The AutoClose shim's event-lock ledger: every context wraps
-- `UI.ReferenceCurrentEvent` / `UI.ReleaseEventID` once per UI table, records
-- the held set in `ExposedMembers.CivvisEventLocks`, and never changes or
-- blocks the lock itself (G84 t117: a UI-side pause with no way to name the
-- context holding the game).
--
-- Run: lua5.1 tools/civ6_control/mod/event_lock_ledger_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."
local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", name, tostring(got)))
	end
end

local lines = {}
Game = { GetLocalPlayer = function() return 0 end, GetCurrentGameTurn = function() return 117 end }
CivvisControlConfig = { DialogueSeconds = 2, Play = false }
ContextPtr = {
	GetID = function() return "DiplomacyActionView" end,
	IsHidden = function() return true end,
	SetUpdate = function() end,
}
Controls = {}
LuaEvents = { CivvisDealSession = { Add = function() end } }
Automation = { Log = function(line) lines[#lines + 1] = line end }
include = function() end
ExposedMembers = {}
local refCalls, releaseCalls, nextId = 0, 0, 41
UI = {
	GetElapsedTime = function() return 12.5 end,
	ReferenceCurrentEvent = function()
		refCalls = refCalls + 1
		nextId = nextId + 1
		return nextId, "extra", nil
	end,
	ReleaseEventID = function(id)
		releaseCalls = releaseCalls + 1
		return "released:" .. tostring(id)
	end,
}
local function load()
	assert(loadfile(here .. "/CivvisControlAutoClose.lua"))()
end
local function rows(op)
	local n = 0
	for _, l in ipairs(lines) do
		if l:find('"kind":"event_lock"', 1, true) and l:find('"op":"' .. op .. '"', 1, true) then n = n + 1 end
	end
	return n
end

load()
check("the ledger is installed on the UI table", UI.CivvisEventLedger, true)
local a, b, c = UI.ReferenceCurrentEvent()
check("the original runs first and its id is returned", a, 42)
check("…with every other result untouched", b, "extra")
check("…including a trailing nil", select("#", UI.ReferenceCurrentEvent()), 3)
local ledger = ExposedMembers.CivvisEventLocks
check("two refs are held", ledger.count, 2)
check("…named by the context that took them", ledger.held[42].ctx, "DiplomacyActionView")
check("…with the turn", ledger.held[42].turn, 117)
check("a ref row per lock", rows("ref"), 2)
check("release returns the original's result", UI.ReleaseEventID(42), "released:42")
check("…and drops the entry", ledger.held[42], nil)
check("…leaving one held", ledger.count, 1)
check("a release row", rows("release"), 1)
UI.ReleaseEventID(999)
check("releasing an id never seen changes nothing", ledger.count, 1)

-- Reloading the shim (every popup context runs it) must not wrap again.
load()
local before = refCalls
UI.ReferenceCurrentEvent()
check("a second install leaves one layer", refCalls - before, 1)
check("…and one ref row per call", rows("ref"), 3)

-- Fail-open: a broken ledger never changes the lock.
ExposedMembers.CivvisEventLocks = { held = nil, count = "x" }
local ok, id = pcall(UI.ReferenceCurrentEvent)
check("a broken ledger still returns the original's id", ok and id == nextId, true)
check("…and still releases", UI.ReleaseEventID(5), "released:5")
local brokenLog = Automation.Log
Automation.Log = function() error("log unavailable") end
ExposedMembers.CivvisEventLocks = nil
check("a broken log still returns the id", UI.ReferenceCurrentEvent(), nextId)
Automation.Log = brokenLog

-- Bounded: at most 32 held, the rest counted as overflow.
ExposedMembers.CivvisEventLocks = nil
for _ = 1, 40 do UI.ReferenceCurrentEvent() end
ledger = ExposedMembers.CivvisEventLocks
check("the held set is capped", ledger.count, 32)
check("…and the excess counted", ledger.overflow, 8)

-- A UI without the pair is left alone.
UI = { GetElapsedTime = function() return 1 end }
local loaded = pcall(load)
check("a UI without the lock functions loads cleanly", loaded, true)
check("…and is not marked", UI.CivvisEventLedger, nil)

local function lastRow(kind)
	for i = #lines, 1, -1 do
		if lines[i]:find('"kind":"' .. kind .. '"', 1, true) then return lines[i] end
	end
	return ""
end

-- `UI` as the engine hands it over on this build: not a plain table (G90 had
-- 19 DiplomacyActionView closes and no row). The context's global is then
-- shadowed by a proxy that wraps only the two lock calls.
local backing = {
	GetElapsedTime = function() return 3.0 end,
	ReferenceCurrentEvent = function() refCalls = refCalls + 1; return 77 end,
	ReleaseEventID = function(id) return "freed:" .. tostring(id) end,
}
local real = newproxy(true)
getmetatable(real).__index = backing
getmetatable(real).__newindex = function(_, k, v) backing[k] = v end
UI = real
ExposedMembers = {}
load()
check("a userdata UI is shadowed by a table proxy", type(UI), "table")
check("…and the install says how", lastRow("event_ledger"):find('"installed":true,"how":"proxy","ui_type":"userdata"', 1, true) ~= nil, true)
check("other keys return the real function object", rawequal(UI.GetElapsedTime, backing.GetElapsedTime), true)
local before = refCalls
check("the lock call reaches the engine through the proxy", UI.ReferenceCurrentEvent(), 77)
check("…exactly once", refCalls - before, 1)
check("…and is recorded", ExposedMembers.CivvisEventLocks.held[77] ~= nil, true)
check("release passes through", UI.ReleaseEventID(77), "freed:77")
UI.SomeFlag = 5
check("a write goes through to the real UI", backing.SomeFlag, 5)
load()
check("a second shim run in the context leaves one proxy", lastRow("event_ledger"):find('"how":"already"', 1, true) ~= nil, true)
before = refCalls
UI.ReferenceCurrentEvent()
check("…still one call per lock", refCalls - before, 1)

-- A read-only table refuses the in-place wrap; the proxy takes over.
local frozen = { GetElapsedTime = function() return 1 end,
	ReferenceCurrentEvent = function() return 5 end, ReleaseEventID = function() return true end }
UI = setmetatable({}, { __index = frozen, __newindex = function() error("read-only") end })
local readonly = UI
load()
check("a read-only UI falls back to the proxy", lastRow("event_ledger"):find('"how":"proxy","ui_type":"table"', 1, true) ~= nil, true)
check("…which replaced the global", rawequal(UI, readonly), false)
check("…and still hands back the real function", rawequal(UI.GetElapsedTime, frozen.GetElapsedTime), true)

-- No UI at all: nothing installed, said so, global untouched.
UI = nil
load()
check("no UI reports installed=false", lastRow("event_ledger"):find('"installed":false,"how":"none","ui_type":"nil"', 1, true) ~= nil, true)
check("…and leaves the global alone", UI, nil)

-- No shared table in a context: the row is still written.
UI = { ReferenceCurrentEvent = function() return 9 end, ReleaseEventID = function() return true end }
ExposedMembers = nil
load()
UI.ReferenceCurrentEvent()
check("without ExposedMembers the ref row still appears", lastRow("event_lock"):find('"op":"ref","id":9,"held":-1', 1, true) ~= nil, true)
check("…and the install names the missing table", lastRow("event_ledger"):find('"exposed":"nil"', 1, true) ~= nil, true)

-- Havok Script's sandbox has no rawequal (G89, 2026-10-05: every context
-- reported `threw` after a swap that had worked). The install must neither
-- need it nor misreport a working proxy.
local savedRawequal = rawequal
rawequal = nil
local sandboxBacking = {
	ReferenceCurrentEvent = function() return 81 end,
	ReleaseEventID = function() return true end,
	GetElapsedTime = function() return 2 end,
}
local sandboxUI = newproxy(true)
getmetatable(sandboxUI).__index = sandboxBacking
UI = sandboxUI
ExposedMembers = {}
load()
check("without rawequal a userdata UI still installs the proxy",
	lastRow("event_ledger"):find('"installed":true,"how":"proxy","ui_type":"userdata"', 1, true) ~= nil, true)
check("…and records locks", (UI.ReferenceCurrentEvent() == 81) and ExposedMembers.CivvisEventLocks.held[81] ~= nil, true)
rawequal = savedRawequal

-- A raising install reports the error text and the real UI type.
UI = { ReferenceCurrentEvent = function() return 1 end, ReleaseEventID = function() return true end }
local brokenType = type
local typeCalls = 0
type = function(v)
	if v == UI then
		typeCalls = typeCalls + 1
		if typeCalls == 1 then error("probe \"quoted\" failure") end
	end
	return brokenType(v)
end
load()
type = brokenType
local threw = lastRow("event_ledger")
check("a raising install says it threw, with the error text", threw:find('"why":"threw: ', 1, true) ~= nil, true)
check("…with quotes made JSON-safe", threw:find("probe 'quoted' failure", 1, true) ~= nil, true)
check("…and the real UI type", threw:find('"ui_type":"table"', 1, true) ~= nil, true)

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("\nall event-lock ledger checks passed")
