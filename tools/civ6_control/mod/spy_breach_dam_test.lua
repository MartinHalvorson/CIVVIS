-- Gathering Storm dam breaches must use the native database hash and district target.
-- Run: lua5.1 tools/civ6_control/mod/spy_breach_dam_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."
local function stub()
    return setmetatable({}, { __index = function() return stub() end,
        __call = function() return stub() end, __newindex = function() end })
end
setmetatable(_G, { __index = function() return stub() end })
CivvisControlConfig = {}
Automation = { Log = function() end }
local operation = 'UNITOPERATION_SPY_BREACH_DAM'
local verb = 'SPY_BREACH_DAM'
local rows = { [operation] = { Hash = -514 } }
GameInfo = setmetatable({ UnitOperations = rows, UnitCommands = {}, Units = {} },
    { __index = function() return stub() end })
-- The enum deliberately omits the operation: resolve through GameInfo instead.
UnitOperationTypes = { PARAM_X = 'x', PARAM_Y = 'y' }
UnitCommandTypes = {}
local unit = { GetID = function() return 15073298 end, GetUnitType = function() return 1 end,
    GetOwner = function() return 0 end, GetX = function() return 36 end,
    GetY = function() return 31 end }
local allow, gates, requests, gatedParams = true, 0, 0, nil
UnitManager = {
    GetUnit = function(pid, id)
        assert(pid == 0 and id == 15073298)
        return unit
    end,
    CanStartOperation = function(...)
        assert(select('#', ...) == 4, 'preserve the native parameterized gate')
        local actor, hash, visible, params = ...
        assert(actor == unit and hash == rows[operation].Hash and visible == nil)
        assert(params.x == 38 and params.y == 29, 'preserve the native dam district target')
        gates = gates + 1
        gatedParams = params
        return allow
    end,
    RequestOperation = function(...)
        assert(select('#', ...) == 3, 'preserve the native parameterized request')
        local actor, hash, params = ...
        assert(actor == unit and hash == rows[operation].Hash)
        assert(params == gatedParams, 'request exactly the parameters that passed the gate')
        requests = requests + 1
    end,
}
Game = { GetLocalPlayer = function() return 0 end, GetCurrentGameTurn = function() return 154 end }
assert(loadfile(here .. '/CivvisControlAgent.lua'))()
CivvisResolveActions()
local function apply(x, y)
    return CivvisApplyOrder(stub(), 0,
        { kind = 'unit', subject = 15073298, verb = verb, x = x, y = y }, 154)
end
local ok, why = apply(38, 29)
assert(ok, 'registered dam breach must reach the host: ' .. tostring(why))
assert(why == verb and gates == 1 and requests == 1)

allow = false
ok, why = apply(38, 29)
assert(not ok and why == verb, 'preserve the existing spy refusal result')
assert(gates == 2 and requests == 1, 'a native refusal must not request')

for _, target in ipairs({ {38}, {false, 29}, {} }) do
    local x = target[1] or nil
    ok, why = apply(x, target[2])
    assert(not ok and why == 'no_spy_target:' .. verb)
    assert(gates == 2 and requests == 1, 'incomplete targets must not reach the host')
end

rows[operation] = nil
CivvisResolveActions()
ok, why = apply(38, 29)
assert(not ok and why == 'unknown_op_' .. verb)
assert(gates == 2 and requests == 1, 'non-Gathering Storm rules must not invent a hash')

-- Re-resolution must use a changed native hash, not a hard-coded fallback.
rows[operation] = { Hash = 915 }
allow = true
CivvisResolveActions()
ok, why = apply(38, 29)
assert(ok and why == verb and gates == 3 and requests == 2)
print('Spy dam breach: database resolution, district target, strict gate and absent expansion passed')
