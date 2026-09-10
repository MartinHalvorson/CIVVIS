-- Native Culture actions use parameterless operations, never Builder improvements.
-- Run: lua5.1 tools/civ6_control/mod/culture_operations_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."
local function stub()
    return setmetatable({}, { __index = function() return stub() end,
        __call = function() return stub() end, __newindex = function() end })
end
setmetatable(_G, { __index = function() return stub() end })
CivvisControlConfig = {}
Automation = { Log = function() end }
local rows = {
    UNITOPERATION_EXCAVATE = { Hash = 77 },
    UNITOPERATION_DESIGNATE_PARK = { Hash = 73 },
    UNITOPERATION_TOURISM_BOMB = { Hash = 211 },
}
GameInfo = setmetatable({ UnitOperations = rows, UnitCommands = {}, Units = {} },
    { __index = function() return stub() end })
UnitOperationTypes = {}
UnitCommandTypes = {}
local unit = { GetID = function() return 92 end, GetUnitType = function() return 1 end,
    GetX = function() return 5 end, GetY = function() return 5 end }
local allow, requests, lastHash, argumentCount = true, 0, nil, nil
UnitManager = {
    GetUnit = function() return unit end,
    CanStartOperation = function() return allow end,
    RequestOperation = function(...)
        argumentCount = select('#', ...)
        local _, hash = ...
        lastHash = hash
        requests = requests + 1
    end,
}
Game = { GetLocalPlayer = function() return 0 end, GetCurrentGameTurn = function() return 156 end }
assert(loadfile(here .. '/CivvisControlAgent.lua'))()
CivvisResolveActions()
for _, verb in ipairs({ 'EXCAVATE', 'DESIGNATE_PARK', 'TOURISM_BOMB' }) do
    allow = true
    local before = requests
    local ok, why = CivvisApplyOrder(stub(), 0, { kind = 'unit', subject = 92, verb = verb }, 156)
    assert(ok, verb .. ': ' .. tostring(why))
    assert(requests == before + 1, verb .. ' must request exactly once')
    assert(lastHash == rows['UNITOPERATION_' .. verb].Hash, verb .. ' wrong operation')
    assert(argumentCount == 2, verb .. ' must omit parameter table like UnitPanel')
    allow = false
    ok, why = CivvisApplyOrder(stub(), 0, { kind = 'unit', subject = 92, verb = verb }, 156)
    assert(not ok and why == 'cannot_' .. verb, verb .. ' must preserve native refusal')
    assert(requests == before + 1, verb .. ' refused action must not request')
end
print('Culture operations: accepted and refused native requests passed')
