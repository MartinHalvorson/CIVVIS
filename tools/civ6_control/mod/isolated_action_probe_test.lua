local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local helper = assert(source:match("(CivvisActionProbe = {.-\nend;)"))
local x, moves, requests, exports = 1, 2, 0, {}
local unit = { GetX = function() return x end, GetY = function() return 1 end,
    GetMovesRemaining = function() return moves end, GetID = function() return 9 end }
local plot = { GetX = function() return 2 end, GetY = function() return 1 end,
    IsWater = function() return false end, IsImpassable = function() return false end,
    GetOwner = function() return -1 end, GetUnitCount = function() return 0 end }
local cfg = { IsolatedActionProbes = true }
local env = setmetatable({ cfg = cfg,
    liveUnit = function() return unit end, eachUnit = function(_, fn) fn(unit) end,
    unitTypeName = function() return "UNIT_WARRIOR" end,
    GameInfo = { Units = { UNIT_WARRIOR = { Combat = 20 } } },
    PlayersVisibility = { [0] = { IsVisible = function() return true end } },
    Map = { GetAdjacentPlot = function() return plot end },
    CivvisBoard = { cancelQueuedPaths = function() end },
    CivvisTransitions = { sequence = 0, apply = function() requests = requests + 1; return true end },
    exportTiles = function() end,
    exportState = function(_, _, _, _, kind) exports[#exports + 1] = { kind = kind } end,
    emit = function(kind, value) value.kind = kind; exports[#exports + 1] = value end,
    try = function(fn, fallback) local ok, value = pcall(fn); if ok then return value end; return fallback end,
}, { __index = _G })
local chunk = assert(loadstring(helper)); setfenv(chunk, env); chunk()
assert(env.CivvisActionProbe.tick({}, 0, 1), "preparation holds normal planner")
assert(requests == 0)
assert(env.CivvisActionProbe.tick({}, 0, 1), "request holds normal planner")
assert(requests == 1 and #exports == 2)
assert(env.CivvisActionProbe.tick({}, 0, 1), "acknowledgement is not arrival")
assert(#exports == 2)
x, moves = 2, 1
assert(not env.CivvisActionProbe.tick({}, 0, 1), "observed arrival releases normal planner")
assert(#exports == 4 and exports[4].settled == true and exports[4].isolated == true)
assert(not env.CivvisActionProbe.tick({}, 0, 1) and requests == 1, "one probe per turn")
assert(not env.CivvisActionProbe.tick({}, 0, 11), "no late-game probes")
x, moves = 1, 2
assert(env.CivvisActionProbe.tick({}, 0, 2))
assert(env.CivvisActionProbe.tick({}, 0, 2))
for _ = 1, 119 do assert(env.CivvisActionProbe.tick({}, 0, 2)) end
assert(not env.CivvisActionProbe.tick({}, 0, 2), "timeout releases planner")
assert(exports[#exports].settled == false, "timeout is not success")
env.CivvisTransitions.apply = function() return false end
assert(env.CivvisActionProbe.tick({}, 0, 3))
assert(not env.CivvisActionProbe.tick({}, 0, 3), "refusal releases planner")
assert(exports[#exports].settled == false and exports[#exports].after_export == false)
cfg.IsolatedActionProbes = false
assert(not env.CivvisActionProbe.tick({}, 0, 4) and requests == 2)
print("isolated action probe: passed")
