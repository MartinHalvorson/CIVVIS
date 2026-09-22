-- Drive the shipped actuator and queue, including asynchronous removal.
-- Run: lua5.1 tools/civ6_control/mod/strategic_improvement_replacement_test.lua
local here = arg[0]:match('(.*)/[^/]*$') or '.'
local function stub()
    return setmetatable({}, { __index = function() return stub() end,
        __call = function() return stub() end, __newindex = function() end })
end
setmetatable(_G, { __index = function() return stub() end })
CivvisControlConfig = { AutomateStuckBuilders = false }
local logs, calls = {}, {}
Automation = { Log = function(line) logs[#logs + 1] = line end }
local state = {}
local ops = { UNITOPERATION_BUILD_IMPROVEMENT = { Hash = 1 },
    UNITOPERATION_REMOVE_IMPROVEMENT = { Hash = 2 }, UNITOPERATION_MOVE_TO = { Hash = 3 } }
local mine = { Hash = 10, ImprovementType = 'IMPROVEMENT_MINE' }
local hacienda = { Hash = 11, ImprovementType = 'IMPROVEMENT_HACIENDA' }
local resource = { Hash = 20, ResourceType = 'RESOURCE_ALUMINUM',
    ResourceClassType = 'RESOURCECLASS_STRATEGIC', PrereqTech = 'TECH_RADIO' }
GameInfo = setmetatable({ UnitOperations = ops, UnitCommands = {},
    Units = { [1] = { UnitType = 'UNIT_BUILDER', Combat = 0, RangedCombat = 0 } },
    Improvements = { [10] = mine, [11] = hacienda,
        IMPROVEMENT_MINE = mine, IMPROVEMENT_HACIENDA = hacienda },
    Resources = { [20] = resource, RESOURCE_ALUMINUM = resource },
    Technologies = { TECH_RADIO = { Index = 30 } },
    Improvement_ValidResources = function()
        local links = {}
        if state.wantedConnects ~= false then links[1] = { ImprovementType = 'IMPROVEMENT_MINE', ResourceType = 'RESOURCE_ALUMINUM' } end
        if state.connected then links[#links + 1] = {
            ImprovementType = 'IMPROVEMENT_HACIENDA', ResourceType = 'RESOURCE_ALUMINUM' } end
        local index = 0
        return function() index = index + 1; return links[index] end
    end,
}, { __index = function() return stub() end })
UnitOperationTypes = { PARAM_X = 'x', PARAM_Y = 'y', PARAM_IMPROVEMENT_TYPE = 'im' }
UnitCommandTypes = {}
ActivityTypes = { ACTIVITY_OPERATION = 1 }
local unit = { GetID = function() return 92 end, GetUnitType = function() return 1 end,
    GetX = function() return state.x end, GetY = function() return 5 end,
    GetMovesRemaining = function() return state.moves end,
    GetBuildCharges = function() return state.charges end }
local plot = { GetOwner = function() return state.owner end,
    GetResourceType = function() return 20 end,
    GetImprovementType = function() return state.improvement or -1 end }
Map = { GetPlot = function(x, y) if x == 5 and y == 5 then return plot end end }
local player = {
    GetResources = function() return { IsResourceVisible = function() return state.visible end } end,
    GetTechs = function() return { HasTech = function() return state.revealed end } end,
}
UnitManager = {
    GetUnit = function() return unit end,
    GetActivityType = function() return state.active and 1 or 0 end,
    CanStartOperation = function(u, hash, excluded, params, results)
        assert(u == unit and excluded == nil)
        if hash == 2 then
            assert(params == false and results == false, 'strict parameterless removal gate')
            return state.removeAllowed and state.moves > 0 and state.improvement ~= nil
        end
        if hash == 1 then
            return type(params) == 'table' and params.im == 10
                and not state.active and (state.improvement == nil or state.directBuild) and state.owner == 0
                and state.x == 5 and state.moves > 0 and state.charges > 0
        end
        return false
    end,
    RequestOperation = function(...)
        local u, hash, params = ...
        calls[#calls + 1] = hash
        if hash == 2 then
            assert(select('#', ...) == 2, 'removal must omit params')
            if state.removeThrows then error('native removal request failed') end
            state.active = true -- Does not change the plot synchronously.
        elseif hash == 1 then
            assert(params.im == 10 and not state.active and (state.improvement == nil or state.directBuild))
            state.improvement = 10
            state.charges = state.charges - 1
        else error('unexpected operation') end
    end,
    CanStartCommand = function() return false end,
}
Game = { GetLocalPlayer = function() return 0 end, GetCurrentGameTurn = function() return 187 end }
assert(loadfile(here .. '/CivvisControlAgent.lua'))()
CivvisResolveActions()
local function reset()
    state = { x = 5, moves = 2, charges = 4, owner = 0, improvement = 11,
        visible = true, revealed = true, removeAllowed = true, active = false }
    resource.ResourceClassType = 'RESOURCECLASS_STRATEGIC'
    CivvisControlConfig.OrderQueue = true
    logs, calls = {}, {}
    CivvisQueue.reset(187)
end
local function order()
    return { kind = 'unit', subject = 92, verb = 'IMPROVE:IMPROVEMENT_MINE', x = 5, y = 5 }
end
local function logHas(text)
    for _, line in ipairs(logs) do if line:find(text, 1, true) then return true end end
    return false
end
reset()
local ok, why = CivvisApplyOrder(player, 0, order(), 187)
assert(ok and why == 'IMPROVEMENT_REPLACEMENT_QUEUED', tostring(why))
assert(#calls == 1 and calls[1] == 2 and state.improvement == 11)
assert(logHas('"kind":"improvement_replacement_started"'), 'record the prerequisite')
assert(not logHas('"kind":"improved"'), 'removal is not a completed mine')
assert(CivvisQueue.pendingCount() == 1)
CivvisQueue.drain(player, 0, 187)
assert(#calls == 1, 'wait for native removal to settle')
state.active, state.improvement = false, nil
CivvisQueue.drain(player, 0, 187)
assert(#calls == 2 and calls[2] == 1 and state.improvement == 10)
assert(CivvisQueue.pendingCount() == 0)
assert(logHas('"kind":"improved"'), 'record the accepted named build')

-- A queued IMPROVE inserts its prerequisite retry before later planned walks.
reset()
local original, later = order(), { kind = 'unit', subject = 92, verb = 'SKIP_TURN' }
CivvisQueue.push(92, original, nil)
CivvisQueue.push(92, later, nil)
CivvisQueue.drain(player, 0, 187)
local entry = CivvisQueue.pending[92]
assert(entry.rows[entry.next].resource_replacement_retry)
assert(entry.rows[entry.next + 1] == later)
state.active, state.improvement = false, nil
CivvisQueue.drain(player, 0, 187)
assert(calls[2] == 1, 'mine precedes later Builder orders')

-- Running out of movement after removal must not blacklist the deposit.
reset()
assert(CivvisApplyOrder(player, 0, order(), 187))
state.active, state.improvement, state.moves = false, nil, 0
CivvisQueue.drain(player, 0, 187)
assert(#calls == 1 and not logHas('"kind":"improve_refused"'))
state.moves = 2
assert(CivvisApplyOrder(player, 0, order(), 188))
assert(calls[2] == 1)

local vetoes = {
    function() state.owner = 1 end,
    function() state.x = 6 end,
    function() state.moves = 0 end,
    function() state.charges = 0 end,
    function() state.visible = false end,
    function() state.revealed = false end,
    function() state.connected = true end,
    function() state.wantedConnects = false end,
    function() state.removeAllowed = false end,
    function() resource.ResourceClassType = 'RESOURCECLASS_LUXURY' end,
    function() CivvisControlConfig.OrderQueue = false end,
}
for _, veto in ipairs(vetoes) do
    reset(); veto()
    CivvisApplyOrder(player, 0, order(), 187)
    for _, hash in ipairs(calls) do assert(hash ~= 2, 'vetoed replacement removed an improvement') end
end
reset(); state.improvement = nil
assert(CivvisApplyOrder(player, 0, order(), 187))
assert(#calls == 1 and calls[1] == 1, 'bare deposit still builds directly')
reset(); state.improvement = 10
CivvisApplyOrder(player, 0, order(), 187)
assert(#calls == 0, 'existing mine remains untouched')
reset(); state.directBuild = true
assert(CivvisApplyOrder(player, 0, order(), 187))
assert(#calls == 1 and calls[1] == 1, 'prefer a legal direct build')
reset(); state.removeThrows = true
CivvisApplyOrder(player, 0, order(), 187)
assert(CivvisQueue.pendingCount() == 0 and state.improvement == 11)
assert(not logHas('"kind":"improvement_replacement_started"'))
print('Strategic replacement: native gate, asynchronous sequence, priority, movement and vetoes passed')
