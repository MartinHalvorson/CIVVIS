-- Run with Lua 5.1. Exercise the real order handler and exported permission
-- helper against Canada's no-surprise-war shape, not an invented war API.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local function stub()
    return setmetatable({}, {
        __index = function() return stub() end,
        __call = function() return stub() end,
        __newindex = function() end,
    })
end
setmetatable(_G, { __index = function() return stub() end })
Automation = { Log = function() end }
CivvisControlConfig = { Play = false }
Game = { GetCurrentGameTurn = function() return 83 end }
assert(loadfile(here .. "/CivvisControlAgent.lua"))()
local apply = rawget(_G, "CivvisApplyOrder")
assert(type(apply) == "function")

local requests, operations, allowed = {}, {}, {}
local major, atWar, genericAllowed, throws = true, false, false, false
local diplomacy = {
    IsAtWarWith = function() return atWar end,
    CanDeclareWarOn = function() return genericAllowed end,
    IsDiplomaticActionValid = function(_, action, target, testVisible)
        assert(target == 3 and testVisible == true)
        if throws then error("permission unavailable") end
        return allowed[action] == true
    end,
}
local player = { GetDiplomacy = function() return diplomacy end }
Players = { [3] = {
    IsMajor = function() return major end,
    IsAlive = function() return true end,
} }
DiplomacyManager = { RequestSession = function(pid, target, statement)
    requests[#requests + 1] = {pid, target, statement}
end }
PlayerOperations = {
    PARAM_PLAYER_ONE = "one", PARAM_PLAYER_TWO = "two",
    DIPLOMACY_DECLARE_WAR = "war",
}
UI = { RequestPlayerOperation = function(pid, operation, params)
    operations[#operations + 1] = {pid, operation, params}
end }
local function order(verb, subject)
    return apply(player, 0, {kind="war", subject=subject or 3, verb=verb}, 83)
end

allowed.DIPLOACTION_DECLARE_FORMAL_WAR = true
assert(order("DECLARE_FORMAL_WAR"), "legal formal war must survive the generic Canada veto")
assert(#requests == 1 and requests[1][3] == "DECLARE_FORMAL_WAR")
assert(#operations == 0, "major wars use the named native diplomacy session")
assert(CivvisWarDeclarations.canDeclareAny(diplomacy, 3) == true)
assert(not order("DECLARE_SURPRISE_WAR"), "formal permission cannot authorize surprise war")
assert(not order("DECLARE"), "legacy major DECLARE still means surprise, not any allowed war")
assert(not order("DECLARE_UNKNOWN_WAR"), "unknown war types cannot become surprise wars")
assert(#requests == 1)

allowed = {}
assert(CivvisWarDeclarations.canDeclareAny(diplomacy, 3) == false)
assert(not order("DECLARE_FORMAL_WAR"), "a treaty/refused action stays refused")
throws = true
assert(CivvisWarDeclarations.canDeclareAny(diplomacy, 3) == nil)
assert(not order("DECLARE_FORMAL_WAR"), "an unreadable permission cannot authorize a war")
throws = false
allowed.DIPLOACTION_DECLARE_WAR_OF_RETRIBUTION = true
assert(order("DECLARE_WAR_OF_RETRIBUTION"))
assert(requests[2][3] == "DECLARE_WAR_OF_RETRIBUTION")
allowed.DIPLOACTION_DECLARE_SURPRISE_WAR = true
assert(order("DECLARE"))
assert(requests[3][3] == "DECLARE_SURPRISE_WAR")
allowed.DIPLOACTION_DECLARE_GOLDEN_AGE_WAR = true
assert(order("DECLARE_GOLDEN_AGE_WAR"))
assert(requests[4][3] == "DECLARE_GOLDEN_WAR", "the native Golden Age session has its own name")
atWar = true
assert(not order("DECLARE_FORMAL_WAR"))
atWar = false
assert(not order("DECLARE_FORMAL_WAR", -1))
assert(#requests == 4)

major, genericAllowed = false, true
assert(order("DECLARE"), "city-state wars retain the native player operation")
assert(#operations == 1 and operations[1][3].two == 3)
assert(not order("DECLARE_FORMAL_WAR"), "city states have no formal-war session")
genericAllowed = false
assert(not order("DECLARE"))
assert(#operations == 1)
print("typed war declarations: exact permission/session, Canada, refusal, unknown API and minors passed")
