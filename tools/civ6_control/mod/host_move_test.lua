-- The reverse movement-fidelity witness records a host-observed edge, not a
-- guessed route.  Exercise the shipped control mod directly: `UnitMoved` is a
-- Civ VI callback and an otherwise-valid reimplementation could easily use the
-- wrong argument order or emit a first sighting with no source coordinate.
--
-- Run: lua5.1 tools/civ6_control/mod/host_move_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end

local log = {}
Automation = { Log = function(line) log[#log + 1] = line end }
CivvisControlConfig = { Play = false }
Game = {
	GetLocalPlayer = function() return 4 end,
	GetCurrentGameTurn = function() return 77 end,
}

setmetatable(_G, { __index = function(_, key)
	if key == "CivvisLedger" then return rawget(_G, key) end
	return stub()
end })

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtime_err = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtime_err))

local ledger = rawget(_G, "CivvisLedger")
assert(type(ledger) == "table", "CivvisLedger is not exported")
assert(type(ledger.onUnitMoved) == "function", "host-move observer is not exported")

local function countMoves()
	local count = 0
	for _, line in ipairs(log) do
		if line:find('"kind":"host_move"', 1, true) then count = count + 1 end
	end
	return count
end

ledger.positions["7"] = { x = 2, y = 3 }
ledger.kinds["7"] = "UNIT_WARRIOR"
ledger.onUnitMoved(4, 7, 3, 3, true, false)
assert(countMoves() == 1, "a local move with an exported source must be recorded")
local first = log[#log]
assert(first:find('"turn":77', 1, true), "the record carries the host turn")
assert(first:find('"unit":7', 1, true), "the record carries the host unit id")
assert(first:find('"unit_kind":"UNIT_WARRIOR"', 1, true), "the record carries the last exported type")
assert(first:find('"from_x":2', 1, true) and first:find('"from_y":3', 1, true),
	"the record carries the previous host coordinate")
assert(first:find('"x":3', 1, true) and first:find('"y":3', 1, true),
	"the record carries the destination in UnitMoved's x/y positions")

-- Repeated callbacks and other players are not new local movement evidence.
ledger.onUnitMoved(4, 7, 3, 3, true, false)
ledger.onUnitMoved(2, 7, 4, 3, true, false)
assert(countMoves() == 1, "duplicates and foreign movement stay out of the local audit")

-- A first observation is only a baseline. The next host move then has both
-- endpoints and becomes comparable.
ledger.onUnitMoved(4, 8, 5, 6, false, true)
assert(countMoves() == 1, "a source-less first sighting is not fabricated into a move")
ledger.onUnitMoved(4, 8, 6, 6, false, true)
assert(countMoves() == 2, "the next move from a seeded unit is recorded")
local second = log[#log]
assert(second:find('"from_x":5', 1, true) and second:find('"from_y":6', 1, true),
	"the observer advances its source coordinate after each event")
assert(second:find('"state_change":true', 1, true), "the host callback metadata survives")

print("host move observer checks passed")
