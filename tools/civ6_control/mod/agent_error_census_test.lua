-- The agent reports what its guards swallow: each distinct message once, again
-- at powers of ten, and never more than a fixed number of messages per guard
-- kind, without any report being able to break the guard that made it.
-- Run: lua5.1 tools/civ6_control/mod/agent_error_census_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."
local handle = assert(io.open(here .. "/CivvisControlAgent.lua", "r"))
local source = handle:read("*a"); handle:close()

local first = assert(source:find("CivvisK.ERROR_LIMIT = ", 1, true), "the error census is gone")
local last = assert(source:find("-- Every read of the game API is guarded", first, true),
	"the census no longer sits before the guards that use it")
local block = source:sub(first, last - 1)

local emitted = {}
local env = setmetatable({
	CivvisK = {},
	emit = function(kind, payload)
		payload.kind = kind
		emitted[#emitted + 1] = payload
	end,
	Game = { GetCurrentGameTurn = function() return 17 end },
}, { __index = _G })
local chunk = assert(loadstring(block))
setfenv(chunk, env)
chunk()
local K = env.CivvisK

-- First sighting is reported with its site, kind and turn.
K.noteError("Agent:12: attempt to index a nil value", "guard")
assert(#emitted == 1, "the first sighting is reported")
assert(emitted[1].kind == "agent_error")
assert(emitted[1].message == "Agent:12: attempt to index a nil value")
assert(emitted[1].where == "guard" and emitted[1].count == 1 and emitted[1].turn == 17)

-- Repeats stay quiet until a power of ten.
for _ = 2, 9 do K.noteError("Agent:12: attempt to index a nil value", "guard") end
assert(#emitted == 1, "repeats below ten are counted, not reported")
K.noteError("Agent:12: attempt to index a nil value", "guard")
assert(#emitted == 2 and emitted[2].count == 10, "the tenth occurrence is reported")
for _ = 11, 100 do K.noteError("Agent:12: attempt to index a nil value", "guard") end
assert(#emitted == 3 and emitted[3].count == 100, "the hundredth occurrence is reported")

-- A noisy kind cannot spend another kind's budget.
emitted = {}
env.emit = function(kind, payload) emitted[#emitted + 1] = payload end
setfenv(chunk, env)
chunk()
K = env.CivvisK
for i = 1, 55 do K.noteError("Agent:" .. i .. ": noisy", "try") end
assert(#emitted == K.ERROR_LIMIT, "distinct messages stop at the limit")
K.noteError("Agent:99: rare", "guard")
assert(#emitted == K.ERROR_LIMIT + 1, "the other guard kind still reports")

-- A message already seen is still counted after the limit, and reported at ten.
for _ = 2, 10 do K.noteError("Agent:1: noisy", "try") end
assert(emitted[#emitted].count == 10, "a known message keeps its count past the limit")

-- Long messages are cut; non-string errors are read.
emitted = {}
K.noteError(string.rep("x", 900), "guard")
assert(#emitted[1].message == 240, "a long message is cut to a bounded size")
emitted = {}
K.noteError({}, "guard")
assert(type(emitted[1].message) == "string", "a table thrown as an error still reports")

-- A traceback is not part of the site: two callers are one message.
emitted = {}
K.noteError("Agent:60: boom\nstack traceback:\n\tfrom one caller", "guard")
K.noteError("Agent:60: boom\nstack traceback:\n\tfrom another caller", "guard")
assert(#emitted == 1 and emitted[1].message == "Agent:60: boom", "only the first line is the key")

-- guard: failure is reported, success is silent, neither escapes.
emitted = {}
K.guard(function() error("Agent:7: boom") end)
assert(#emitted == 1 and emitted[1].where == "guard", "guard reports a failure")
emitted = {}
K.guard(function() return 1 end)
assert(#emitted == 0, "guard says nothing when the work succeeds")

-- attempt: the same boolean as pcall, and the failure is reported.
emitted = {}
assert(K.attempt(function() return 1 end) == true and #emitted == 0, "attempt reports success silently")
assert(K.attempt(function() error("Agent:8: nope") end) == false, "attempt answers false on failure")
assert(#emitted == 1 and emitted[1].where == "attempt", "attempt reports what it caught")

-- A report that itself fails must not take the turn down.
env.emit = function() error("the log is gone") end
assert(pcall(K.noteError, "Agent:50: a new message", "guard"), "reporting never throws")
assert(pcall(K.guard, function() error("Agent:51: boom") end), "guard never throws")

-- The guards in the file actually report.
assert(source:find('CivvisK.noteError(result, "try");', 1, true), "try() reports what it catches")
assert(source:find("CivvisK.guard(function() fn(unit); end);", 1, true), "eachUnit reports per unit")
assert(source:find("CivvisK.guard(function() fn(city); end);", 1, true), "eachCity reports per city")
for _, name in ipairs({ "resolveActions", "survey", "rehost" }) do
	assert(source:find("CivvisK.guard(" .. name .. ");", 1, true), name .. " reports what it swallows")
end
assert(source:find("CivvisK.guard(function() exportState(", 1, true), "a lost state export is reported")
print("agent error census checks passed")
