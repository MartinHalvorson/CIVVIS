-- The native settler lens warning is observable without changing UI state.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local function stub()
 return setmetatable({}, {
  __index = function() return stub() end,
  __call = function() return stub() end,
  __newindex = function() end,
 })
end
setmetatable(_G, { __index = function() return stub() end })
assert(loadfile(here .. "/CivvisControlAgent.lua"))()
local helper = rawget(_G, "CivvisSettlementLoyalty")
assert(type(helper) == "function", "native settlement loyalty helper missing")
setmetatable(_G, nil)
UI = setmetatable({}, { __index = function() error("must not change selection") end })
UILens = setmetatable({}, { __index = function() error("must not change the lens") end })

local calls = 0
local values = { [7] = -20, [3] = "-9.5", [8] = 0, [9] = -12 }
Game = { GetLocalPlayer = function() return 0 end }
Map = {
 GetContinentPlotsLoyalty = function() calls = calls + 1; return values end,
 GetPlotByIndex = function(index)
  return { GetX = function() return index end, GetY = function() return 2 end }
 end,
}
PlayersVisibility = { [0] = { IsRevealed = function(_, x) return x ~= 9 end } }
local settlers = { { kind = "UNIT_SETTLER" } }
local facts = helper(0, 27, 1, settlers)
assert(facts.available and facts.turn == 27 and facts.frame == 1)
assert(calls == 1 and facts.raw_count == 4 and #facts.plots == 3)
assert(facts.plots[1].x == 3 and facts.plots[1].loyalty == -9.5)
assert(facts.plots[2].x == 7 and facts.plots[2].loyalty == -20)
assert(facts.plots[3].x == 8 and facts.plots[3].loyalty == 0)
-- Missing rows are unavailable, not invented zero-pressure predictions.
values = {}
facts = helper(0, 28, 0, settlers)
assert(facts.available and #facts.plots == 0 and facts.raw_count == 0)
-- Never carry old lens warnings through an unavailable or failed query.
Map.GetContinentPlotsLoyalty = function() error("unsupported") end
facts = helper(0, 29, 0, settlers)
assert(not facts.available and #facts.plots == 0 and facts.reason == "api_unavailable")
Map.GetContinentPlotsLoyalty = nil
assert(not helper(0, 30, 0, settlers).available)
-- A different seat or an empire without a Settler must not query this UI API.
Map.GetContinentPlotsLoyalty = function() error("must not be queried") end
facts = helper(0, 31, 0, { { kind = "UNIT_WARRIOR" } })
assert(not facts.available and facts.reason == "no_settler")
facts = helper(1, 31, 0, settlers)
assert(not facts.available and facts.reason == "not_local_player")
-- Skip malformed values, nonfinite numbers, and plots without a safe readback.
values = { [3] = -5, [4] = 0/0, [5] = math.huge, [6] = "unknown", [-1] = -20 }
Map.GetContinentPlotsLoyalty = function() return values end
facts = helper(0, 32, 0, settlers)
assert(facts.available and #facts.plots == 1 and facts.invalid_count == 4)
Map.GetPlotByIndex = function() return nil end
assert(#helper(0, 33, 0, settlers).plots == 0)
Map.GetPlotByIndex = function()
 return { GetX = function() return "unknown" end, GetY = function() return 2 end }
end
assert(#helper(0, 33, 1, settlers).plots == 0)
PlayersVisibility = {}
assert(#helper(0, 34, 0, settlers).plots == 0)

-- Exercise the production call site, including suppression for diagnostic
-- action-transition exports. The event must be distinct from state fields so
-- older deciders can ignore it without changing their schema contract.
local block = assert(source:match("(\tif eventKind == nil then\n\t\temit%(\"settlement_loyalty\".-\n\tend)"),
 "settlement loyalty event is not wired into state export")
local events = {}
local environment = setmetatable({
 eventKind = nil, pid = 0, turn = 35, frame = 2, units = settlers,
 emit = function(kind, payload) events[#events + 1] = { kind = kind, payload = payload } end,
}, { __index = _G })
local export = assert(loadstring(block))
setfenv(export, environment)
export()
assert(#events == 1 and events[1].kind == "settlement_loyalty")
assert(events[1].payload.turn == 35 and events[1].payload.frame == 2)
environment.eventKind = "action_transition_before"
export()
assert(#events == 1)
print("native settlement loyalty export checks passed")
