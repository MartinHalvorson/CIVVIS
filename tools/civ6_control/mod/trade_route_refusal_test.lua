-- A refused trade route says whether its Trader could have started any route.
--
-- A Trader bought this turn, or one that changed its origin city this turn,
-- has no movement left and the host refuses whatever it asks for. The mirror
-- retires an origin/destination pair the host refused twice, and a replan
-- frame re-sends the order, so on civvis-20261115T071444Z two frames of one
-- turn retired pairs the host itself listed in `route_options`. The event
-- now carries `moves`, read the way Base/Assets/UI/WorldInput.lua:971 reads
-- it, and the mirror does not count a refusal recorded at zero.
--
-- The request itself is the shipped chooser's, unchanged:
-- Base/Assets/UI/Choosers/TradeRouteChooser.lua:826-838 (`RequestTradeRoute`).
local here = arg[0]:match('(.*)/[^/]*$') or '.'
local function stub()
    return setmetatable({}, { __index = function() return stub() end,
        __call = function() return stub() end, __newindex = function() end })
end
setmetatable(_G, { __index = function() return stub() end })
CivvisControlConfig = {}
local LOG = {}
Automation = { Log = function(line) LOG[#LOG + 1] = line end }
GameInfo = setmetatable({ UnitOperations = {}, UnitCommands = {},
    Units = { [1] = { UnitType = 'UNIT_TRADER' } } },
    { __index = function() return stub() end })
UnitOperationTypes = { MAKE_TRADE_ROUTE = 55, PARAM_X0 = 'x0', PARAM_Y0 = 'y0',
    PARAM_X1 = 'x1', PARAM_Y1 = 'y1', PARAM_X = 'x', PARAM_Y = 'y' }
UnitCommandTypes = {}
local allow, moves, movesRaises, requests, asked = true, 2, false, 0, nil
local unit = { GetUnitType = function() return 1 end, GetID = function() return 393220 end,
    GetX = function() return 13 end, GetY = function() return 26 end,
    GetMovesRemaining = function()
        if movesRaises then error('no such method on this build') end
        return moves
    end }
UnitManager = { GetUnit = function() return unit end,
    CanStartOperation = function(_, op, _, p)
        asked = { op = op, x0 = p.x0, y0 = p.y0, x1 = p.x1, y1 = p.y1 }
        return allow
    end,
    RequestOperation = function() requests = requests + 1 end }
Game = { GetLocalPlayer = function() return 0 end, GetCurrentGameTurn = function() return 16 end }
assert(loadfile(here .. '/CivvisControlAgent.lua'))()
CivvisResolveActions()
local player = { GetCities = function() return { Members = function() return function() return nil end end } end }
local function route() return { kind = 'unit', subject = 393220, verb = 'TRADE_ROUTE', x = 19, y = 31 } end
local function refusals()
    local found = {}
    for _, line in ipairs(LOG) do
        if line:find('"kind":"trade_route_refused"', 1, true) then found[#found + 1] = line end
    end
    return found
end
local function number(line, name) return tonumber(line:match('"' .. name .. '":(-?[%d%.]+)')) end

-- 1. A route the host starts: the shipped four-parameter request, and no event.
local ok, why = CivvisApplyOrder(player, 0, route(), 16)
assert(ok and why == 'TRADE_ROUTE' and requests == 1, tostring(why))
assert(asked.op == 55 and asked.x0 == 19 and asked.y0 == 31 and asked.x1 == 13 and asked.y1 == 26,
    'destination as X0/Y0 and the Trader\'s own plot as X1/Y1')
assert(#refusals() == 0, 'an accepted route reports no refusal')

-- 2. Refused with no movement left: the event says so, and nothing is requested.
allow, moves = false, 0
ok = CivvisApplyOrder(player, 0, route(), 16)
assert(not ok and requests == 1, 'a refused route is not requested')
local events = refusals()
assert(#events == 1, 'one event per refusal')
assert(number(events[1], 'moves') == 0, 'the event carries the Trader\'s movement: ' .. events[1])
assert(number(events[1], 'unit') == 393220 and number(events[1], 'turn') == 16)
assert(number(events[1], 'from_x') == 13 and number(events[1], 'from_y') == 26
    and number(events[1], 'x') == 19 and number(events[1], 'y') == 31,
    'both endpoints still ride on the event')

-- 3. Refused with movement to spend: a reading the mirror counts.
LOG = {}
moves = 2
ok = CivvisApplyOrder(player, 0, route(), 17)
assert(not ok)
assert(number(refusals()[1], 'moves') == 2, 'movement left is reported as it is')

-- 4. A build without the read reports -1, never a zero the mirror would discount.
LOG = {}
movesRaises = true
ok = CivvisApplyOrder(player, 0, route(), 18)
assert(not ok)
assert(number(refusals()[1], 'moves') == -1, 'an unreadable value is not passed off as zero')

print('trade route refusal: movement on the event, request unchanged')
