-- Exercise the shipped request, combat callback and settleTurn boundary.
-- No Firaxis damage/legality emulation: the fake host controls those facts.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local cfg = { ReplanFrames = 2, StrikePreview = false, OrderQueueGraceTicks = 3 }
local events, exports, turn, accepted, synchronous, throwing = {}, 0, 20, true, false, false
local attacker = { player = 0, id = 524291, type = "unit", x = 41, y = 11, hp = 100 }
local defender = { player = 63, id = 2555938, type = "unit", x = 42, y = 9, hp = 69 }
local attacks, activity = 1, "operation"
local env = setmetatable({ cfg = cfg, awaiting = { turn = turn, done = true },
    emit = function(kind, row) events[#events + 1] = { kind = kind, row = row } end,
    try = function(f, fallback) local ok, value = pcall(f); if ok then return value end; return fallback end,
    exportState = function() exports = exports + 1 end,
    CivvisQueue = { pendingCount = function() return 0 end },
    Game = { GetCurrentGameTurn = function() return turn end, GetLocalPlayer = function() return 0 end },
    CombatVisType = { ATTACKER = 1, DEFENDER = 2 },
    UnitOperationTypes = { PARAM_X = "x", PARAM_Y = "y" },
    OP = { UNITOPERATION_RANGE_ATTACK = "ranged" },
    GameInfo = { Units = { UNIT_ARCHER = { UnitType = "UNIT_ARCHER" } } },
    unitTypeName = function() return "UNIT_ARCHER" end,
    refusalReason = function() return "can_start=false,no_reasons [p4r]" end,
    UnitManager = { GetActivityType = function() return activity end },
}, { __index = _G })
env.CivvisLedger = {
    pending = {}, open = {}, damage = {},
    componentKey = function(component) return component.player .. ":" .. component.id end,
    describe = function(component)
        local desc = {}; for k, v in pairs(component) do desc[k] = v end; return desc
    end,
    refuseWarStarter = function() return nil end,
    refuseLethalPreview = function() return nil end,
}
local function load(text)
    local chunk = assert(loadstring(text)); setfenv(chunk, env); chunk()
end
load(assert(source:match("(CivvisFrames = {.-)\nlocal function applyOrders")))
load(assert(source:match("(CivvisLedger.strike = function.-)\n%-%- One of OUR units left")))
-- This is the actual RANGE_ATTACK branch, not a copy of its logic.
local branch = assert(source:match('(\t\tif verb == "RANGE_ATTACK" then.-)\n\t\t%-%- %★'))
load("apply = function(player, pid, row, turn)\nlocal verb, subject, x, y = row.verb, row.subject, row.x, row.y\nlocal unit = row.actor\n" .. branch .. "\nend")
load(assert(source:match("(local function settleTurn.-CivvisSettleTurn = settleTurn;)")))
local unit = {
    GetUnitType = function() return "UNIT_ARCHER" end, GetDamage = function() return 0 end,
    GetMovesRemaining = function() return 1 end, GetAttacksRemaining = function() return attacks end,
    GetX = function() return 41 end, GetY = function() return 11 end,
}
local row = { kind = "unit", subject = attacker.id, verb = "RANGE_ATTACK", x = 42, y = 9, actor = unit }
local combat = { attacker, defender }
env.operate = function()
    if synchronous then
        env.CivvisLedger.onCombatVisBegin(combat)
        env.CivvisLedger.onCombatVisEnd(combat)
    end
    if throwing then error("host request raised") end
    return accepted
end
local frames = env.CivvisFrames
-- Discovery/export are unrelated to settlement, so keep the exact frame
-- begin and settleTurn, but replace the expensive board census only.
frames.observe = function() end
local failures, checks = 0, 0
local function check(name, got, want)
    checks = checks + 1
    if got ~= want then failures = failures + 1; print("FAIL " .. name .. ": " .. tostring(got) .. " != " .. tostring(want))
    else print("ok " .. name) end
end
local function reset()
    frames.reset(); events = {}; exports = 0; accepted = true; synchronous = false; throwing = false
    activity = "operation"; attacks = 1
    env.awaiting = { turn = turn, done = true }
    env.CivvisLedger.pending = {}; env.CivvisLedger.open = {}; env.CivvisLedger.damage = {}
end
local function issue() return env.apply({}, 0, row, turn) end
local function settle()
    -- In a regressed implementation an early frame already opened. Model its
    -- answered handshake too, so every subsequent guard can report its result.
    env.awaiting.done = true
    return env.CivvisSettleTurn({}, 0, turn, function() error("fallback") end)
end
local function finish(a, d)
    env.CivvisLedger.onCombatVisBegin({ a or attacker, d or defender })
    env.CivvisLedger.onCombatVisEnd({ a or attacker, d or defender })
end
local function count(kind)
    local n = 0; for _, e in ipairs(events) do if e.kind == kind then n = n + 1 end end; return n
end

reset(); check("accepted shot", issue(), true)
check("accepted shot holds settlement", settle(), false)
check("no premature native frame", frames.current, 0)
check("no premature snapshot", exports, 0)
activity = "awake" -- G66 also exported awake with the attack still available.
check("awake is not combat completion", settle(), false)
check("awake still exports no snapshot", exports, 0)
finish()
check("combat completion permits replan", settle(), false)
check("completed board opens exactly one frame", frames.current, 1)
check("completed board exported", exports, 1)
check("wait was recorded once", count("strike_frame_wait"), 1)
check("completion does not claim timeout", count("strike_frame_timeout"), 0)

reset(); issue(); finish({ player = 9, id = attacker.id, type = "unit" }, defender)
settle(); check("foreign same-ID attacker cannot release our shot", exports, 0)
reset(); issue(); finish(attacker, { player = 63, id = 17, type = "unit", x = 41, y = 9, hp = 20 })
settle(); check("different target combat cannot release our shot", exports, 0)
reset(); issue(); turn = 21; finish(); turn = 20
settle(); check("different turn combat cannot release our shot", exports, 0)

reset(); accepted = false; check("refused shot stays refused", issue(), false)
settle(); check("refusal adds no settlement latency", exports, 1)
reset(); synchronous = true; issue(); settle()
check("synchronous combat is not lost after RequestOperation returns", exports, 1)

reset(); issue(); accepted = false; issue(); settle()
check("a second refused request does not erase the earlier accepted shot", exports, 0)
finish(); settle(); check("earlier accepted shot still resolves", exports, 1)

reset(); issue(); issue(); finish(); settle()
check("one combat cannot release two accepted tickets", exports, 0)
finish(); settle(); check("two completed combats release two tickets", exports, 1)
reset(); issue()
row.subject = 1114114; issue(); row.subject = attacker.id
finish(); settle(); check("one actor cannot release another actor's shot", exports, 0)
finish({ player = 0, id = 1114114, type = "unit", hp = 100 }, defender)
settle(); check("all actors settled before the one replan export", exports, 1)

reset(); issue(); settle(); settle()
check("unanswered shot waits within existing grace", exports, 0)
settle(); check("unanswered shot releases at existing grace", exports, 1)
check("grace release is named, not execution proof", count("strike_frame_timeout"), 1)

reset(); issue(); frames.reset(); frames.noteStrike(); settle()
check("new turn reset clears old pending shot", exports, 1)
reset(); cfg.ReplanFrames = 0; issue()
check("frames disabled leaves turn completion unchanged", settle(), true)
check("frames disabled exports nothing", exports, 0)
cfg.ReplanFrames = 2
reset(); frames.noteStrike(); settle()
check("a legacy strike trigger without a tracked request is unchanged", exports, 1)

assert(failures == 0, tostring(failures) .. " failures in " .. tostring(checks) .. " checks")
print("native strike frame settlement: " .. tostring(checks) .. " checks passed")
