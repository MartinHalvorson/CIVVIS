-- Exercise the production actuator against the two shipped build operations.
-- WorldInput_Expansion2.lua:36-43 uses BUILD_IMPROVEMENT_ADJACENT with the
-- target's coordinates and the named improvement, not BUILD_IMPROVEMENT.
local here = arg[0]:match('(.*)/[^/]*$') or '.'
local function stub()
    return setmetatable({}, { __index = function() return stub() end,
        __call = function() return stub() end, __newindex = function() end })
end
setmetatable(_G, { __index = function() return stub() end })
CivvisControlConfig = { AutomateStuckBuilders = false }
local state, requests, probes, logs = {}, {}, {}, {}
Automation = { Log = function(line) logs[#logs + 1] = line end }
local ordinary, adjacent = 101, 102
local names = { 'IMPROVEMENT_FARM', 'IMPROVEMENT_MINE', 'IMPROVEMENT_SKI_RESORT',
    'IMPROVEMENT_MOUNTAIN_TUNNEL', 'IMPROVEMENT_MOUNTAIN_ROAD' }
local improvements = {}
for i, name in ipairs(names) do
    local row = { Hash = 200 + i, ImprovementType = name }
    improvements[name], improvements[row.Hash] = row, row
end
GameInfo = setmetatable({
    UnitOperations = {
        UNITOPERATION_BUILD_IMPROVEMENT = { Hash = ordinary },
        UNITOPERATION_BUILD_IMPROVEMENT_ADJACENT = { Hash = adjacent },
    },
    UnitCommands = {},
    Units = { [1] = { UnitType = 'UNIT_BUILDER', Combat = 0, RangedCombat = 0 },
        [2] = { UnitType = 'UNIT_MILITARY_ENGINEER', Combat = 0, RangedCombat = 0 } },
    Improvements = improvements,
    Improvements_XP2 = {},
}, { __index = function() return stub() end })
UnitOperationTypes = { PARAM_X = 'x', PARAM_Y = 'y', PARAM_IMPROVEMENT_TYPE = 'im' }
UnitCommandTypes = {}
ActivityTypes = { ACTIVITY_OPERATION = 1 }
local unit = { GetID = function() return 7274528 end,
    GetUnitType = function() return state.unitType end,
    GetX = function() return 20 end, GetY = function() return 11 end,
    GetMovesRemaining = function() return state.moves end,
    GetBuildCharges = function() return state.charges end }
local plot = { GetOwner = function() return 0 end,
    GetResourceType = function() return -1 end,
    GetImprovementType = function() return state.existing or -1 end }
Map = { GetPlot = function(x, y)
    assert(x == state.x and y == state.y, 'use the ordered target, not the unit plot')
    return plot
end }
UnitManager = {
    GetUnit = function() return unit end,
    GetActivityType = function() return 0 end,
    CanStartCommand = function() return false end,
    CanStartOperation = function(u, hash, excluded, params)
        assert(u == unit and excluded == nil)
        if type(params) ~= 'table' then return false end
        probes[#probes + 1] = { operation = hash, im = params.im }
        return hash == state.operation and params.x == state.x and params.y == state.y
            and params.im == state.im and state.allowed and state.moves > 0 and state.charges > 0
    end,
    RequestOperation = function(...)
        local u, hash, params = ...
        assert(select('#', ...) == 3 and u == unit and hash == state.operation)
        assert(params.x == state.x and params.y == state.y and params.im == state.im)
        requests[#requests + 1] = { operation = hash, x = params.x, y = params.y, im = params.im }
        if state.requestThrows then error('native request failed') end
    end,
}
Game = { GetLocalPlayer = function() return 0 end, GetCurrentGameTurn = function() return 173 end }
assert(loadfile(here .. '/CivvisControlAgent.lua'))()
CivvisResolveActions()
local checks = 0
local function check(label, condition)
    assert(condition, label)
    checks = checks + 1
end
local function reset(wanted, flag, operation, onOwnTile)
    requests, probes, logs = {}, {}, {}
    state = { unitType = 1, moves = 2, charges = 1, allowed = true,
        x = onOwnTile and 20 or 21, y = onOwnTile and 11 or 10,
        operation = operation, im = wanted and improvements[wanted].Hash or nil }
    GameInfo.Improvements_XP2 = {}
    if wanted and flag ~= nil then
        GameInfo.Improvements_XP2[wanted] = { ImprovementType = wanted, BuildOnAdjacentPlot = flag }
    end
    CivvisQueue.reset(173)
    return { kind = 'unit', subject = 7274528,
        verb = wanted and ('IMPROVE:' .. wanted) or 'IMPROVE', x = state.x, y = state.y }
end
local function hasLog(kind)
    for _, line in ipairs(logs) do
        if line:find('"kind":"' .. kind .. '"', 1, true) then return true end
    end
    return false
end

-- Ordinary controls pass before the regression: their operation stays the same.
for _, flag in ipairs({ false, 0 }) do
    local row = reset('IMPROVEMENT_FARM', flag, ordinary, true)
    check('ordinary Farm remains buildable', CivvisApplyOrder({}, 0, row, 173))
    check('ordinary Farm sends exactly its normal request', #requests == 1 and requests[1].operation == ordinary)
end
local row = reset('IMPROVEMENT_MINE', nil, ordinary, true)
GameInfo.Improvements_XP2 = nil
check('absent expansion table preserves ordinary build', CivvisApplyOrder({}, 0, row, 173))
check('absent expansion table sends normal operation', #requests == 1 and requests[1].operation == ordinary)
row = reset(nil, nil, ordinary, true)
check('unnamed ordinary build still works', CivvisApplyOrder({}, 0, row, 173))

-- Actual refused Pericles 043232Z order: unit (20,11), target (21,10), t173.
-- Expansion2_Improvements.xml:88-89 and Improvements_Major.xml:20 mark these
-- three rows BuildOnAdjacentPlot=true. Both Boolean representations are valid.
for _, wanted in ipairs({ 'IMPROVEMENT_SKI_RESORT', 'IMPROVEMENT_MOUNTAIN_TUNNEL',
        'IMPROVEMENT_MOUNTAIN_ROAD' }) do
    for _, flag in ipairs({ true, 1 }) do
        row = reset(wanted, flag, adjacent, false)
        if wanted ~= 'IMPROVEMENT_SKI_RESORT' then state.unitType = 2 end
        check(wanted .. ' uses its adjacent operation', CivvisApplyOrder({}, 0, row, 173))
        check('one adjacent request preserves target and name', #requests == 1
            and requests[1].operation == adjacent and requests[1].x == 21
            and requests[1].y == 10 and requests[1].im == improvements[wanted].Hash)
        check('accepted named request is reported', hasLog('improved'))
    end
end

-- A refused adjacent operation must not try an ordinary or unnamed build.
for _, veto in ipairs({
    function() state.allowed = false end,
    function() state.moves = 0 end,
    function() state.charges = 0 end,
}) do
    row = reset('IMPROVEMENT_SKI_RESORT', true, adjacent, false)
    veto()
    check('native refusal stays refused', not CivvisApplyOrder({}, 0, row, 173))
    check('refused target sends no request', #requests == 0)
    for _, probe in ipairs(probes) do
        check('refusal diagnostics and retries retain adjacent operation and name',
            probe.operation == adjacent and probe.im == state.im)
    end
    check('refused request is not reported improved', not hasLog('improved'))
end
row = reset('IMPROVEMENT_SKI_RESORT', true, adjacent, false)
state.requestThrows = true
check('throwing request is not accepted', not CivvisApplyOrder({}, 0, row, 173))
check('throwing request is not reported improved', not hasLog('improved'))
row = reset('IMPROVEMENT_SKI_RESORT', true, adjacent, false)
state.existing = state.im
check('already built target is not reissued', not CivvisApplyOrder({}, 0, row, 173) and #requests == 0)
check('already built target is reported', hasLog('improve_already'))
print(string.format('All %d adjacent-improvement actuator checks passed', checks))
