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

local highlighted = { 17, 23 }
unit.GetRockBand = function() return {
    GetActivationHighlightPlots = function() return highlighted end,
} end
Map = { GetPlotByIndex = function(index)
    return { GetX = function() return index end, GetY = function() return 3 end }
end }
local sites = CivvisRockBandConcertPlots(unit, 'UNIT_ROCK_BAND')
assert(#sites == 2 and sites[1].x == 17 and sites[2].x == 23 and sites[2].y == 3)
assert(CivvisRockBandConcertPlots(unit, 'UNIT_BUILDER') == nil)
highlighted = {}
assert(#CivvisRockBandConcertPlots(unit, 'UNIT_ROCK_BAND') == 0)
highlighted = nil
assert(CivvisRockBandConcertPlots(unit, 'UNIT_ROCK_BAND') == nil)
unit.GetRockBand = function() error('API unavailable') end
assert(CivvisRockBandConcertPlots(unit, 'UNIT_ROCK_BAND') == nil)
print('Rock Band highlights: coordinates, empty and unavailable readings passed')

-- Naming is independent of a concert order, since an unnamed band may export
-- no activation destinations. Preserve custom names and bound async retries.
GameInfo.Units[1] = { UnitType = 'UNIT_ROCK_BAND', Name = 'LOC_UNIT_ROCK_BAND_NAME' }
local bandName = 'LOC_UNIT_ROCK_BAND_NAME'
unit.GetName = function() return bandName end
UnitCommandTypes.NAME_UNIT = 501
UnitCommandTypes.PARAM_NAME = 'name'
local nameRequests, canName = 0, true
UnitManager.CanStartCommand = function(u, command, testVisible, params)
    assert(u == unit and command == 501 and testVisible == false)
    assert(params.name == 'Civvis Band 92')
    return canName
end
UnitManager.RequestCommand = function(u, command, params)
    assert(u == unit and command == 501 and params.name == 'Civvis Band 92')
    nameRequests = nameRequests + 1
end
local player = { GetUnits = function() return { Members = function() return ipairs({unit}) end } end }
CivvisNameRockBands(player, 0, 199)
assert(nameRequests == 1)
CivvisNameRockBands(player, 0, 199)
assert(nameRequests == 1, 'pending name requests must not repeat in one turn')
CivvisNameRockBands(player, 0, 200)
assert(nameRequests == 2, 'a name absent on the next turn may retry')
bandName = 'Existing Band'
CivvisNameRockBands(player, 0, 201)
assert(nameRequests == 2, 'preserve names already assigned by the host or user')
bandName = 'LOC_UNIT_ROCK_BAND_NAME'
canName = false
CivvisNameRockBands(player, 0, 202)
assert(nameRequests == 2, 'respect native command denial')
canName = true
GameInfo.Units[1].UnitType = 'UNIT_BUILDER'
CivvisNameRockBands(player, 0, 203)
assert(nameRequests == 2, 'only name Rock Bands')
GameInfo.Units[1].UnitType = 'UNIT_ROCK_BAND'
unit.GetName = function() error('unreadable') end
CivvisNameRockBands(player, 0, 204)
assert(nameRequests == 2, 'unknown names must not be overwritten')
print('Rock Band naming: prerequisite, pending retry, existing names and refusal passed')
