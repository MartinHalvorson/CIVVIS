-- Native turn horizon readback. Loads the shipped agent and exercises its
-- startup and seat survey with a configuration setter that cannot change the
-- already-running game's limit. WorldRankings.lua:1053 reads GetMaxGameTurns.
-- Run: lua5.1 tools/civ6_control/mod/turn_limit_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."
local function stub()
    return setmetatable({}, {
        __index = function() return stub() end,
        __call = function() return stub() end,
        __newindex = function() end,
    })
end
setmetatable(_G, { __index = function() return stub() end })
CivvisControlConfig = { MaxTurns = 650, StartAfterTicks = 0, Survey = true }
local configured, native = 250, 250
GameConfiguration = {
    GetMaxTurns = function() return configured end,
    SetMaxTurns = function(n) configured = n end,
    SetTurnLimitType = function() end,
    GetValue = function() return nil end,
}
Game = {
    GetLocalPlayer = function() return 0 end,
    GetCurrentGameTurn = function() return 99 end,
    GetMaxGameTurns = function() return native end,
}
PlayerManager = { GetAliveMajorIDs = function() return { 0, 1 } end }
GameInfo = setmetatable({}, { __index = function()
    return setmetatable({}, { __call = function() return function() return nil end end })
end })
assert(loadfile(here .. "/CivvisControlAgent.lua"))()
local function upvalue(fn, key, replacement)
    for i = 1, 100 do
        local name, value = debug.getupvalue(fn, i)
        if name == nil then break end
        if name == key then
            if replacement ~= nil then debug.setupvalue(fn, i, replacement) end
            return value
        end
    end
    error("missing upvalue " .. key)
end
local start = upvalue(CivvisQueue.onUnitSettled, "ensureStarted")
local survey = upvalue(start, "survey")
local records = {}
upvalue(start, "emit", function(kind, value) records[kind] = value end)
start()
assert(configured == 650, "fixture must reproduce the successful config setter")
assert(records.turn_limit and records.turn_limit.config == 650, "requested configuration stays diagnostic")
assert(records.turn_limit.game == 250, "diagnostic must read the native limit using the shipped API")
assert(records.seat and records.seat.max_turns == 250, "planner seat must use native 250, not requested 650")

native = 300
survey()
assert(records.seat.max_turns == 300, "a later survey must read the current host limit")
native = 0
survey()
assert(records.seat.max_turns == 0, "native zero must not become the configured cap")
Game.GetMaxGameTurns = function() error("unavailable") end
survey()
assert(records.seat.max_turns == -1, "unreadable native limit must stay unknown")
Game.GetMaxGameTurns = function() return nil end
survey()
assert(records.seat.max_turns == -1, "nil native limit must stay unknown")
Game.GetMaxGameTurns = nil
survey()
assert(records.seat.max_turns == -1, "missing native API must stay unknown")
print("all native turn limit checks passed")
