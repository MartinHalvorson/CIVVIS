-- Exercise the shipped stage-event / popup-callback submission ordering.
-- Native WorldCongressPopup.lua:2222-2271 sends votes from OnAccept, after
-- popup setup. This test does not model native acceptance: ballot verdicts
-- in a subsequent live Congress remain the proof that votes registered.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local function stub()
    return setmetatable({}, {
        __index = function() return stub() end,
        __call = function() return stub() end,
        __newindex = function() end,
    })
end
setmetatable(_G, { __index = function() return stub() end })
CivvisControlConfig = { Play = true, CivvisDecides = false }
local hooks = {}
Events = setmetatable({ WorldCongressStage1 = {
    Add = function(fn) hooks.stage = fn end,
} }, { __index = function() return stub() end })
LuaEvents = setmetatable({ CivvisCongressBallot = {
    Add = function(fn) hooks.popup = fn end,
} }, { __index = function() return stub() end })
Automation = { Log = function() end }
local turn = 194
Game = {
    GetLocalPlayer = function() return 0 end,
    GetCurrentGameTurn = function() return turn end,
    GetCurrentTurnSegment = function() return "TURNSEG_WORLDCONGRESS_1" end,
    GetWorldCongress = function() return { GetResolutions = function() return { Stage = 2147483647 } end } end,
}
DB = { MakeHash = function(name) return name end }
Players = { [0] = {
    IsTurnActive = function() return false end,
    GetFavor = function() return 1020 end,
} }
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
local tick = upvalue(CivvisQueue.onUiPulse, "tick")
tick()
assert(type(hooks.stage) == "function" and type(hooks.popup) == "function", "both real event handlers must register")
local cast = upvalue(hooks.popup, "castBallot")
local calls, readable = 0, true
upvalue(cast, "voteWorldCongress", function(pid)
    assert(pid == 0, "ballot must belong to the local seat")
    calls = calls + 1
    if not readable then return 0, 0, "not_readable" end
    return 3, 364, nil, 4, 15, 1108, "claim"
end)
hooks.stage(1)
assert(calls == 0, "another player's stage cannot send our ballot")
hooks.stage(0)
assert(calls == 0, "the early stage event must not send or consume a ballot")
hooks.popup()
assert(calls == 1, "the popup gets the first submission attempt")
hooks.popup()
assert(calls == 1, "one submitted ballot must not be sent twice")
turn = 214
hooks.stage(0)
assert(calls == 1, "the next stage must also wait for its popup")
readable = false
hooks.popup()
assert(calls == 2, "a later Congress can try its own ballot")
readable = true
hooks.popup()
assert(calls == 3, "an unreadable attempt must leave a later retry available")
hooks.popup()
assert(calls == 3, "a retried ballot must not be sent twice")
print("all Congress submission ordering checks passed")
