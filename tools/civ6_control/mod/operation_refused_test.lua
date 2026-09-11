-- An in-place operation the host refuses must be SAID, not silently skipped.
--
-- The bridge otherwise sees only the consequence — the unit is not fortified —
-- and cannot tell a host refusal from an order the mod never issued. That
-- ambiguity is the whole of the largest unexplained failure on the ledger:
-- 82.5% of FORTIFY orders fail across the 42 live runs of 2026-09-10/11.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()

local body = assert(source:match("(local function firstOperation.-\nend)"),
    "firstOperation moved or changed shape")
local chunk = assert(loadstring((body:gsub("^local function firstOperation",
    "firstOperation = function", 1))))

local emitted, started = {}, {}
-- `setfenv` replaces the WHOLE environment, so the standard library the
-- function relies on has to be handed back explicitly.
local env = {
    ipairs = ipairs, pairs = pairs, pcall = pcall, error = error, type = type,
    OP = { UNITOPERATION_FORTIFY = 11, UNITOPERATION_ALERT = 12 },
    operate = function(_, hash) return started[hash] == true end,
    emit = function(kind, payload) emitted[#emitted + 1] = { kind, payload } end,
    try = function(fn, fallback)
        local ok, value = pcall(fn)
        if ok and value ~= nil then return value end
        return fallback
    end,
    unitTypeName = function() return "UNIT_WARRIOR" end,
    Game = { GetCurrentGameTurn = function() return 47 end },
}
setfenv(chunk, env); chunk()

local unit = { GetID = function() return 4242 end }
local names = { "UNITOPERATION_FORTIFY", "UNITOPERATION_ALERT" }

-- 1. Everything refused: one event, naming what was tried.
assert(env.firstOperation(unit, names) == nil, "nothing started, so nothing is claimed")
assert(#emitted == 1, "exactly one event per call, not one per candidate")
assert(emitted[1][1] == "operation_refused", "the event names the refusal")
local payload = emitted[1][2]
assert(payload.unit == 4242, "the event names the unit")
assert(payload.turn == 47, "the event carries the turn")
assert(payload.unit_kind == "UNIT_WARRIOR", "the event carries the unit kind")
assert(#payload.tried == 2 and payload.tried[1] == "UNITOPERATION_FORTIFY",
    "the event names every operation that was refused, in order")

-- 2. ⭐ A FORTIFY that fell through to ALERT and STARTED is not a refusal.
--    Reporting it would recreate the ambiguity this exists to remove.
emitted = {}
started[12] = true
assert(env.firstOperation(unit, names) == "UNITOPERATION_ALERT", "the fallback started")
assert(#emitted == 0, "a call that started something reports nothing")

-- 3. The first candidate starting reports nothing either.
emitted, started = {}, { [11] = true, [12] = true }
assert(env.firstOperation(unit, names) == "UNITOPERATION_FORTIFY")
assert(#emitted == 0, "the common path stays silent")

-- 4. A unit whose id cannot be read still produces a usable event.
emitted, started = {}, {}
local broken = { GetID = function() error("gone") end }
assert(env.firstOperation(broken, names) == nil)
assert(#emitted == 1 and emitted[1][2].unit == -1,
    "an unreadable id falls back rather than throwing inside the emit")

print("operation refused: passed")
