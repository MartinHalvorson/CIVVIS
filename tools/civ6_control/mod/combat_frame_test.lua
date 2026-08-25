-- The mid-turn combat frame: after the opening orders settle on a turn that
-- struck, the board goes out again and the same turn is answered again.
--
-- ⚠ Loads the SHIPPED `CivvisControlAgent.lua` with `CombatFrames = 1` and
-- drives its own `CivvisFrames`, `applyOrders` and the frame-aware order
-- readers against a fake host and a fake order channel.
--
-- What is checked:
--   1. no frame is wanted until a strike was issued; a strike arms one;
--   2. opening a frame emits `combat_frame`, re-arms the handshake and
--      restarts the strike counter; the frame cap holds;
--   3. on a frame, `applyOrders` hands NO unit to explore automation and
--      writes NO `turn` record — the opening board did both;
--   4. the order readers select the answer by frame, and a channel written by
--      a brain that predates the `frame` column reads as frame 0;
--   5. with `CombatFrames` unset nothing is ever wanted (the default is off).
--
-- Run: lua5.1 tools/civ6_control/mod/combat_frame_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end

local EXPORTS = { CivvisApplyOrders = true, CivvisQueue = true, CivvisResolveActions = true,
                  CivvisFrames = true, CivvisLedger = true, CivvisOrdersReady = true,
                  CivvisFetchOrders = true, CivvisApplyOrder = true }
local LOG = {}
Automation = { Log = function(line) LOG[#LOG + 1] = line end }
-- The mod reads its configuration from this global at load.
-- `ExportState` is left OFF: the full board export walks the whole host API,
-- which no stub can stand in for; the frame stamp on the state event is one
-- pass-through argument (`exportState(player, pid, turn, frame)`) and the
-- frame handshake is what this test drives.
CivvisControlConfig = { CombatFrames = 1, OrdersDb = "/tmp/fake.sqlite", RunTag = "test-run" }
UnitOperationTypes = { PARAM_X = "x", PARAM_Y = "y" }
UnitCommandTypes = {}
Map = { GetPlotDistance = function(x1, y1, x2, y2) return math.max(math.abs(x1 - x2), math.abs(y1 - y2)) end,
        GetPlot = function() return nil end }
-- Any other GameInfo table is empty: indexing a row gives a stub, and CALLING
-- the table (the `for row in GameInfo.X() do` iteration the export uses)
-- gives an iterator that ends at once, so the export loops terminate.
local function emptyTable()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return function() return nil end end,
	})
end
GameInfo = setmetatable({}, { __index = function(_, k)
	if k == "UnitOperations" or k == "UnitCommands" then
		return setmetatable({}, { __index = function(_, name) return { Hash = name } end })
	end
	if k == "Units" then
		return setmetatable({}, { __index = function(_, name) return { UnitType = name, Combat = 20, RangedCombat = 0 } end })
	end
	return emptyTable()
end })
-- The fake order channel: rows the "brain" wrote, with or without a frame column.
local channel = { ready = {}, orders = {} }
DB = { Query = function(sql)
	if sql:find("civvis.ready", 1, true) then return channel.ready end
	if sql:find("civvis.orders", 1, true) then return channel.orders end
	return {}
end }
setmetatable(_G, { __index = function(_, k)
	if EXPORTS[k] then return rawget(_G, k) end
	return stub()
end })

local host = { units = {}, ops = {} }
local PID = 0
local function unitObject(u)
	return {
		GetID = function() return u.id end, GetX = function() return u.x end, GetY = function() return u.y end,
		GetMovesRemaining = function() return u.moves end, GetUnitType = function() return u.kind end,
		GetType = function() return u.kind end, GetDamage = function() return 0 end,
		GetGreatPerson = function() return nil end, GetFortifyTurns = function() return 0 end,
		GetFormationUnitCount = function() return 1 end,
		GetComponentID = function() return { player = 0, id = u.id } end,
	}
end
UnitManager = {
	GetUnit = function(pid, id) local u = host.units[id]; return u and unitObject(u) or nil end,
	CanStartOperation = function() return true end,
	RequestOperation = function(unit, hash, params)
		host.ops[#host.ops + 1] = { id = unit.GetID(), op = hash }
	end,
	CanStartCommand = function() return false end, RequestCommand = function() end,
}
local function members(list)
	return function()
		local i = 0
		return function() i = i + 1; if list[i] == nil then return nil end; return i, list[i] end
	end
end
local player = setmetatable({
	GetUnits = function()
		local objs = {}
		for _, u in pairs(host.units) do objs[#objs + 1] = unitObject(u) end
		table.sort(objs, function(a, b) return a.GetID() < b.GetID() end)
		return { Members = members(objs) }
	end,
	GetCities = function() return { Members = members({}) } end,
	GetDiplomacy = function() return { IsAtWarWith = function() return false end } end,
	GetScore = function() return 0 end,
	GetTreasury = function() return { GetGoldBalance = function() return 0 end } end,
	IsTurnActive = function() return true end,
}, { __index = function() return stub() end })
Players = setmetatable({}, { __index = function(_, pid)
	if pid == PID then return player end
	return setmetatable({ IsBarbarian = function() return true end,
		GetUnits = function() return { Members = members({}) } end,
		GetCities = function() return { Members = members({}) } end }, { __index = function() return stub() end })
end })
PlayerManager = { GetAliveIDs = function() return { PID, 63 } end, GetAliveMajorIDs = function() return { PID } end }
PlayersVisibility = setmetatable({}, { __index = function() return { IsVisible = function() return true end } end })
Game = { GetLocalPlayer = function() return PID end, GetCurrentGameTurn = function() return 12 end }

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtime_err = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtime_err))

local applyOrders = rawget(_G, "CivvisApplyOrders")
local frames = rawget(_G, "CivvisFrames")
local ledger = rawget(_G, "CivvisLedger")
local ordersReady = rawget(_G, "CivvisOrdersReady")
local fetchOrders = rawget(_G, "CivvisFetchOrders")
rawget(_G, "CivvisResolveActions")()
assert(type(frames) == "table", "CivvisFrames is not exported")

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", name, tostring(got)))
	end
end
local function count(kind)
	local n = 0
	for _, line in ipairs(LOG) do if line:find('"kind":"' .. kind .. '"', 1, true) then n = n + 1 end end
	return n
end
local function lastEvent(kind)
	for i = #LOG, 1, -1 do if LOG[i]:find('"kind":"' .. kind .. '"', 1, true) then return LOG[i] end end
	return nil
end
local function has(line, needle) return line ~= nil and line:find(needle, 1, true) ~= nil end
local function row(subject, verb, x, y) return { kind = "unit", subject = subject, verb = verb, x = x, y = y } end

-- 1. Nothing wanted until a strike; a strike arms a frame.
frames.reset()
check("no frame wanted before any strike", frames.wanted(), false)
host.units[5] = { id = 5, kind = "UNIT_ARCHER", x = 2, y = 2, moves = 2 }
ledger.strike(UnitManager.GetUnit(0, 5), 5, "RANGE_ATTACK", 4, 2, 12)
check("a strike arms a frame", frames.wanted(), true)

-- 2. Opening a frame emits combat_frame and re-arms the handshake; the cap holds.
frames.begin(player, PID, 12)
check("frame 1 opened", frames.current, 1)
check("combat_frame emitted with the strike count", has(lastEvent("combat_frame"), '"strikes":1'), true)
check("the frame's strike counter starts again", frames.strikes, 0)
check("no second frame beyond the cap", frames.wanted(), false)
ledger.strike(UnitManager.GetUnit(0, 5), 5, "RANGE_ATTACK", 4, 2, 12)
check("even after another strike the cap holds", frames.wanted(), false)

-- 3. On a frame, applyOrders neither explores unmentioned units nor writes a turn record.
host.units[6] = { id = 6, kind = "UNIT_WARRIOR", x = 9, y = 9, moves = 2 } -- unmentioned combat unit
local turnsBefore = count("turn")
host.ops = {}
applyOrders(player, PID, 12, { row(5, "FORTIFY") })
local explored = 0
for _, o in ipairs(host.ops) do if o.op == "UNITOPERATION_AUTOMATE_EXPLORE" then explored = explored + 1 end end
check("no explore hand-off on a frame", explored, 0)
check("no turn record on a frame", count("turn"), turnsBefore)
check("orders event names the frame", has(lastEvent("orders"), '"frame":1'), true)

-- 4. Frame-aware readers, and a pre-frame channel reads as frame 0.
channel.ready = { { run = "test-run", turn = 12, count = 2, frame = 1 } }
channel.orders = {
	{ seq = 0, kind = "unit", subject = 5, verb = "MOVE_TO", x = 3, y = 2, frame = 0 },
	{ seq = 10000, kind = "unit", subject = 5, verb = "RANGE_ATTACK", x = 4, y = 2, frame = 1 },
	{ seq = 10001, kind = "unit", subject = 6, verb = "FORTIFY", frame = 1 },
}
check("ready answers for frame 1", ordersReady(12, 1), 2)
check("ready does not answer frame 0 once frame 1 was written", ordersReady(12, 0), nil)
check("fetch filters frame 1 rows", #fetchOrders(12, 1), 2)
check("fetch filters frame 0 rows", #fetchOrders(12, 0), 1)
channel.ready = { { run = "test-run", turn = 12, count = 1 } }              -- old brain: no frame column
channel.orders = { { seq = 0, kind = "unit", subject = 5, verb = "FORTIFY" } }
check("a channel without the column reads as frame 0", ordersReady(12, 0), 1)
check("…and answers no frame", ordersReady(12, 1), nil)
check("…and its rows are frame 0's", #fetchOrders(12, 0), 1)

-- 5. Off by default: with the flag unset nothing is ever wanted.
CivvisControlConfig.CombatFrames = nil
frames.reset()
ledger.strike(UnitManager.GetUnit(0, 5), 5, "RANGE_ATTACK", 4, 2, 12)
check("default off: no frame wanted after a strike", frames.wanted(), false)

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("\nall combat-frame checks passed")
