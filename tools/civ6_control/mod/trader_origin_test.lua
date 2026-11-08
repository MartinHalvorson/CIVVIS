-- The shipped TradeOriginChooser sends PARAM_X/Y through the native operation.
local here = arg[0]:match('(.*)/[^/]*$') or '.'
local function stub()
    return setmetatable({}, { __index = function() return stub() end,
        __call = function() return stub() end, __newindex = function() end })
end
setmetatable(_G, { __index = function() return stub() end })
CivvisControlConfig = {}
Automation = { Log = function() end }
GameInfo = setmetatable({ UnitOperations = { UNITOPERATION_TELEPORT_TO_CITY = { Hash = 77 } },
    UnitCommands = {}, Units = { [1] = { UnitType = 'UNIT_TRADER' }, [2] = { UnitType = 'UNIT_WARRIOR' } } },
    { __index = function() return stub() end })
UnitOperationTypes = { PARAM_X = 'x', PARAM_Y = 'y' }
UnitCommandTypes = {}
local kind, allow, throw, requests, gateArguments, requestArguments, got = 1, true, false, 0
local unit = { GetUnitType = function() return kind end, GetID = function() return 92 end,
    GetX = function() return 5 end, GetY = function() return 5 end }
UnitManager = { GetUnit = function() return unit end,
    CanStartOperation = function(...)
        gateArguments = select('#', ...)
        local _, op, _, p = ...
        assert(op == 77 and p.x == 10 and p.y == 10)
        return allow
    end,
    RequestOperation = function(...)
        requestArguments = select('#', ...)
        local _, op, p = ...
        got = { op = op, x = p.x, y = p.y }; requests = requests + 1
        if throw then error('native refusal') end
    end }
Game = { GetLocalPlayer = function() return 0 end, GetCurrentGameTurn = function() return 175 end }
assert(loadfile(here .. '/CivvisControlAgent.lua'))()
CivvisResolveActions()
local player = { GetCities = function() return { Members = function()
    local done = false
    return function()
        if done then return nil end
        done = true
        return 1, { GetX = function() return 10 end, GetY = function() return 10 end }
    end
end } end }
local function order(x, y) return { kind = 'unit', subject = 92, verb = 'TELEPORT_TO_CITY', x = x, y = y } end
local ok, why = CivvisApplyOrder(player, 0, order(10, 10), 175)
assert(ok and requests == 1 and got.op == 77 and got.x == 10 and got.y == 10, tostring(why))
assert(gateArguments == 4 and requestArguments == 3, 'native chooser signatures')
allow = false
ok, why = CivvisApplyOrder(player, 0, order(10, 10), 175)
assert(not ok and why == 'cannot_trade_origin' and requests == 1)
allow = true; kind = 2
ok, why = CivvisApplyOrder(player, 0, order(10, 10), 175)
assert(not ok and why == 'not_trader' and requests == 1)
kind = 1
ok, why = CivvisApplyOrder(player, 0, order(20, 20), 175)
assert(not ok and why == 'trade_origin_not_ours' and requests == 1)
ok, why = CivvisApplyOrder(player, 0, order(nil, nil), 175)
assert(not ok and why == 'no_trade_origin' and requests == 1)
throw = true
ok, why = CivvisApplyOrder(player, 0, order(10, 10), 175)
assert(not ok and requests == 2, 'a thrown request is not success')
print('Trader origin: native request, refusal, ownership and signature checks passed')
