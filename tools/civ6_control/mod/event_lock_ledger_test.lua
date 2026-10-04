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

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("\nall event-lock ledger checks passed")
