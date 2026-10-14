-- A sortie at a city holds its strike frame until its combat ends, exactly as
-- a ranged shot does; a sortie at a plot with no city (a pillage) does not.
-- Exercises the shipped AIR_ATTACK branch, combat callback and settleTurn.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local cfg = { ReplanFrames = 2, StrikePreview = false, OrderQueueGraceTicks = 3 }
local events, exports, turn, accepted, startable = {}, 0, 183, true, true
local bomber = { player = 0, id = 10485765, type = "unit", x = 18, y = 20, hp = 100 }
local scone = { player = 1, id = 3735604, type = "district", x = 23, y = 18, hp = 60 }
local cityPlots = { ["23,18"] = true }
local env = setmetatable({ cfg = cfg, awaiting = { turn = turn, done = true },
    emit = function(kind, row) events[#events + 1] = { kind = kind, row = row } end,
    try = function(f, fallback) local ok, value = pcall(f); if ok then return value end; return fallback end,
    exportState = function() exports = exports + 1 end,
    CivvisQueue = { pendingCount = function() return 0 end },
    Game = { GetCurrentGameTurn = function() return turn end, GetLocalPlayer = function() return 0 end },
    Map = { GetPlot = function(x, y)
        return { IsCity = function() return cityPlots[x .. "," .. y] == true end }
    end },
    CombatVisType = { ATTACKER = 1, DEFENDER = 2 },
    UnitOperationTypes = { PARAM_X = "x", PARAM_Y = "y" },
    OP = { UNITOPERATION_AIR_ATTACK = "air", UNITOPERATION_REBASE = "rebase", UNITOPERATION_DEPLOY = "deploy" },
    GameInfo = { Units = { UNIT_BOMBER = { UnitType = "UNIT_BOMBER" } } },
    unitTypeName = function() return "UNIT_BOMBER" end,
    refusalReason = function() return "can_start=false,no_reasons [p4r]" end,
    canOperate = function() return startable end,
    UnitManager = { GetActivityType = function() return "operation" end },
    eachCity = function(_, visit)
        visit({ GetBuildQueue = function()
            return { GetCurrentProductionTypeHash = function() return 0 end }
        end })
    end,
}, { __index = _G })
env.CivvisLedger = {
    pending = {}, open = {}, damage = {},
    componentKey = function(component) return component.player .. ":" .. component.id end,
    describe = function(component)
        local desc = {}; for k, v in pairs(component) do desc[k] = v end; return desc
    end,
    refuseWarStarter = function() return nil end,
    annotateStrikeOffer = function() return {} end,
}
local function load(text)
    local chunk = assert(loadstring(text)); setfenv(chunk, env); chunk()
end
load(assert(source:match("(CivvisClock = {.-)\nCivvisExportClock = ")))
load(assert(source:match("(CivvisFrames = {.-)\nlocal function applyOrders")))
load(assert(source:match("(CivvisLedger.strike = function.-)\n%-%- One of OUR units left")))
-- The actual air-verb branch, not a copy of its logic.
local branch = assert(source:match('(\t\tif verb == "AIR_ATTACK" or verb == "REBASE" or verb == "PATROL" then.-)\n\t\t%-%- %★'))
load("apply = function(player, pid, row, turn)\nlocal verb, subject, x, y = row.verb, row.subject, row.x, row.y\nlocal unit = row.actor\n" .. branch .. "\nend")
load(assert(source:match("(local function settleTurn.-CivvisSettleTurn = settleTurn;)")))
local unit = {
    GetUnitType = function() return "UNIT_BOMBER" end, GetDamage = function() return 0 end,
    GetMovesRemaining = function() return 1 end, GetAttacksRemaining = function() return 1 end,
    GetX = function() return 18 end, GetY = function() return 20 end,
}
local row = { kind = "unit", subject = bomber.id, verb = "AIR_ATTACK", x = 23, y = 18, actor = unit }
env.operate = function() return accepted end
local frames = env.CivvisFrames
frames.observe = function() end
local failures, checks = 0, 0
local function check(name, got, want)
    checks = checks + 1
    if got ~= want then failures = failures + 1; print("FAIL " .. name .. ": " .. tostring(got) .. " != " .. tostring(want))
    else print("ok " .. name) end
end
local function reset()
    frames.reset(); events = {}; exports = 0; accepted = true; startable = true
    row.verb, row.x, row.y = "AIR_ATTACK", 23, 18
    env.awaiting = { turn = turn, done = true }
    env.CivvisLedger.pending = {}; env.CivvisLedger.open = {}; env.CivvisLedger.damage = {}
end
local function issue() return env.apply({}, 0, row, turn) end
local function settle()
    env.awaiting.done = true
    return env.CivvisSettleTurn({}, 0, turn, function() error("fallback") end)
end
local function finish(d)
    env.CivvisLedger.onCombatVisBegin({ bomber, d or scone })
    env.CivvisLedger.onCombatVisEnd({ bomber, d or scone })
end
local function count(kind)
    local n = 0; for _, e in ipairs(events) do if e.kind == kind then n = n + 1 end end; return n
end

reset(); check("sortie at a city is accepted", issue(), true)
check("sortie at a city holds the strike frame", settle(), false)
check("no frame opens before the bomb lands", frames.current, 0)
check("no stale board exported", exports, 0)
finish()
settle()
check("the landed bomb opens exactly one frame", frames.current, 1)
check("the post-strike board is exported once", exports, 1)
check("no timeout claimed for a landed bomb", count("strike_frame_timeout"), 0)

reset(); row.x, row.y = 30, 30; issue(); settle()
check("a pillage sortie (no city) does not hold the frame", exports, 1)
check("a pillage sortie waits out no grace", count("strike_frame_timeout"), 0)

reset(); accepted = false; issue(); settle()
check("a refused sortie releases its ticket at once", exports, 1)

reset(); startable = false
check("a sortie the host will not start is a named refusal", select(2, issue()), "cannot_air_attack")
check("an unstarted sortie holds nothing", settle(), true)
check("an unstarted sortie asks for no strike frame", exports, 0)

reset(); row.verb = "REBASE"; issue(); settle()
check("a rebase is not a strike", exports, 0)
check("a rebase opens no strike frame", frames.current, 0)

reset(); issue(); settle(); settle()
check("an unanswered sortie waits within the grace", exports, 0)
settle(); check("an unanswered sortie releases at the grace", exports, 1)
check("the grace release is named", count("strike_frame_timeout"), 1)

assert(failures == 0, tostring(failures) .. " failures in " .. tostring(checks) .. " checks")
print("air strike frame: " .. tostring(checks) .. " checks passed")
