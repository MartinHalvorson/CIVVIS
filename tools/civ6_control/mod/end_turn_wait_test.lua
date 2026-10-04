-- Why the host refuses our end turn: `CivvisQueue.noteEndTurnRequest` samples
-- the shipped predicates on every request that passes the rate guard, and
-- `LocalPlayerTurnEnd` emits one `end_turn_wait` per turn with the counts, the
-- first-request -> turn-end time, and the live Quick Movement/Combat.
--
-- Loads the SHIPPED `CivvisControlAgent.lua` on the `order_queue_test` host.
--
-- Run: lua5.1 tools/civ6_control/mod/end_turn_wait_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end

local EXPORTS = {
	CivvisApplyOrders = true, CivvisQueue = true, CivvisResolveActions = true,
	CivvisApplyOrder = true,
}
-- Real tables the agent indexes with real keys.
local LOG = {}
Automation = { Log = function(line) LOG[#LOG + 1] = line end }
UnitOperationTypes = { PARAM_X = "x", PARAM_Y = "y" }
ActivityTypes = { ACTIVITY_OPERATION = "operation" }
UnitCommandTypes = {}
Map = {
	GetPlotDistance = function(x1, y1, x2, y2)
		-- Manhattan-ish is enough for a radius test on an offset grid.
		return math.max(math.abs(x1 - x2), math.abs(y1 - y2))
	end,
	GetPlot = function() return nil end,
}
GameInfo = setmetatable({}, { __index = function(_, k)
	if k == "UnitOperations" or k == "UnitCommands" then
		return setmetatable({}, { __index = function(_, name) return { Hash = name } end })
	end
	if k == "Units" then
		return setmetatable({}, { __index = function(_, name)
			if name == "UNIT_SETTLER" then return { UnitType = name, Combat = 0, RangedCombat = 0 } end
			if name == "UNIT_ARCHER" then return { UnitType = name, Combat = 15, RangedCombat = 25 } end
			return { UnitType = name, Combat = 20, RangedCombat = 0 }
		end })
	end
	return stub()
end })
setmetatable(_G, { __index = function(_, k)
	if EXPORTS[k] then return rawget(_G, k) end
	return stub()
end })

-- ---------------------------------------------------------------- fake host
local host = { units = {}, ops = {}, commands = {}, allow_cancel = false,
	defer_cancel = false,
	refuse = {}, contacts = {}, governor_requests = {}, cities = {} }
local PID = 0
local function unitObject(u)
	return {
		GetID = function() return u.id end,
		GetX = function() return u.x end,
		GetY = function() return u.y end,
		GetMovesRemaining = function() return u.moves end,
		GetUnitType = function() return u.kind end,
		GetType = function() return u.kind end,
		GetDamage = function() return 0 end,
		GetGreatPerson = function() return nil end,
		GetFortifyTurns = function() return 0 end,
		GetFormationUnitCount = function() return 1 end,
		GetAttacksRemaining = function() return u.attacks or 1 end,
		GetActivityType = function() return u.activity end,
	}
end
UnitManager = {
	GetUnit = function(pid, id)
		local u = host.units[id]
		if u == nil or u.gone then return nil end
		return unitObject(u)
	end,
	GetActivityType = function(unit)
		return host.units[unit.GetID()].activity
	end,
	CanStartOperation = function(unit, hash, _, params)
		local id = unit.GetID()
		if host.refuse[id] and host.refuse[id][hash] then return false end
		return true
	end,
	RequestOperation = function(unit, hash, params)
		local u = host.units[unit.GetID()]
		host.ops[#host.ops + 1] = { id = u.id, op = hash,
			x = params and params.x or nil, y = params and params.y or nil }
		if hash == "UNITOPERATION_MOVE_TO" then
			-- Asynchronous, like the host: the unit arrives only when the
			-- test says so (`host.arrive`), unless the walk was priced dead.
			u.pendingX, u.pendingY = params.x, params.y
			if u.active_operation then u.activity = "operation" end
		end
	end,
	CanStartCommand = function(_, hash)
		return hash == "UNITCOMMAND_CANCEL" and host.allow_cancel
	end,
	RequestCommand = function(unit, hash)
		host.commands[#host.commands + 1] = { id = unit.GetID(), command = hash }
		if hash == "UNITCOMMAND_CANCEL" and not host.defer_cancel then
			host.units[unit.GetID()].activity = nil
		end
	end,
}
PlayerOperations = {
	PARAM_GOVERNOR_TYPE = "governor_type",
	PARAM_PLAYER_ONE = "player_one",
	PARAM_CITY_DEST = "city_dest",
	ASSIGN_GOVERNOR = "assign_governor",
}
GameInfo.Governors = {
	GOVERNOR_THE_MERCHANT = {
		Hash = "GOVERNOR_THE_MERCHANT", Index = 17,
	},
}
CityManager = {
	GetCity = function(owner, id)
		return host.cities[id] and { owner = owner, id = id } or nil
	end,
}
UI = {
	RequestPlayerOperation = function(pid, operation, params)
		host.governor_requests[#host.governor_requests + 1] = {
			pid = pid, operation = operation, params = params,
		}
	end,
}
function host.arrive(id)
	local u = host.units[id]
	if u.pendingX ~= nil then
		u.x, u.y = u.pendingX, u.pendingY
		u.pendingX, u.pendingY = nil, nil
		u.moves = math.max(0, (u.moves or 0) - 1)
	end
end
function host.deactivate(id)
	host.units[id].activity = nil
end
local function members(list)
	return function()
		local i = 0
		return function()
			i = i + 1
			if list[i] == nil then return nil end
			return i, list[i]
		end
	end
end
local player = setmetatable({
	GetGovernors = function()
		return {
			HasGovernor = function(_, hash) return hash == "GOVERNOR_THE_MERCHANT" end,
			GetGovernor = function() return nil end,
		}
	end,
	GetUnits = function()
		local objs = {}
		for _, u in pairs(host.units) do
			if not u.gone then objs[#objs + 1] = unitObject(u) end
		end
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
	-- One barbarian seat with the hostile units the test plants.
	return setmetatable({
		IsBarbarian = function() return true end,
		GetUnits = function()
			local objs = {}
			for _, u in ipairs(host.contacts) do objs[#objs + 1] = unitObject(u) end
			return { Members = members(objs) }
		end,
		GetCities = function() return { Members = members({}) } end,
	}, { __index = function() return stub() end })
end })
PlayerManager = { GetAliveIDs = function() return { PID, 63 } end,
                  GetAliveMajorIDs = function() return { PID } end }
PlayersVisibility = setmetatable({}, { __index = function()
	return { IsVisible = function() return true end, IsRevealed = function() return true end }
end })
Game = { GetLocalPlayer = function() return PID end, GetCurrentGameTurn = function() return 7 end }

local handlers = {}
Events = setmetatable({}, { __index = function(_, name)
	return { Add = function(handler) handlers[name] = handler end }
end })
local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtime_err = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtime_err))
local queue = rawget(_G, "CivvisQueue")
assert(type(queue) == "table", "CivvisQueue is not exported")

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", name, tostring(got)))
	end
end
local function events(kind)
	local out = {}
	for _, l in ipairs(LOG) do
		if l:find('"kind":"' .. kind .. '"', 1, true) then out[#out + 1] = l end
	end
	return out
end
local function has(l, needle) return l ~= nil and l:find(needle, 1, true) ~= nil end

-- The host this probe reads: a controllable UI clock and the four shipped
-- predicates, plus the live Quick Movement/Combat answers.
local now = 10.0
local host_state = { busy = true, can = false, processing = false, blocker = 0 }
local requested = 0
ActionTypes = { ACTION_ENDTURN = "end_turn" }
UI.GetElapsedTime = function() return now end
UI.HasSentTurnComplete = function() return false end
UI.RequestAction = function() requested = requested + 1 end
UI.IsGameCoreBusy = function() return host_state.busy end
UI.CanEndTurn = function() return host_state.can end
UI.IsProcessingMessages = function() return host_state.processing end
EndTurnBlockingTypes = { NO_ENDTURN_BLOCKING = 0 }
NotificationManager = { GetFirstEndTurnBlocking = function() return host_state.blocker end }
UserConfiguration = { IsQuickMovement = function() return false end,
                      IsQuickCombat = function() return true end }

check("LocalPlayerTurnEnd is registered", type(handlers.LocalPlayerTurnEnd), "function")

-- 1. Turn 7: two refused requests while the core is busy, one inside the
-- 0.25 s rate guard (not sent, not sampled), one with a blocker, one forced.
queue.requestEndTurn(7)                       -- t=10.0 busy, cannot
now = 10.1; queue.requestEndTurn(7)           -- deferred by the rate guard
now = 10.4; queue.requestEndTurn(7)           -- busy, cannot
host_state.busy, host_state.blocker = false, 5
now = 10.7; queue.requestEndTurn(7)           -- cannot, blocker 5
host_state.can, host_state.blocker, host_state.processing = true, 0, true
now = 11.0; queue.requestEndTurn(7, { REASON = "UserForced" })
check("three requests and the forced one reached the host", requested, 4)

now = 11.25
handlers.LocalPlayerTurnEnd()
local waits = events("end_turn_wait")
check("one end_turn_wait for the turn", #waits, 1)
local w = waits[1]
check("…counts the sent requests", has(w, '"requests":4'), true)
check("…and the forced one", has(w, '"forced":1'), true)
check("…first request to turn end", has(w, '"wait":1.25'), true)
check("…last request to turn end", has(w, '"after_last":0.25'), true)
check("…busy samples", has(w, '"busy":2'), true)
check("…cannot-end samples", has(w, '"cannot":3'), true)
check("…processing samples", has(w, '"processing":1'), true)
check("…the blocker by type", has(w, '"blockers":{"5":1}'), true)
check("…the live Quick Movement", has(w, '"quick_movement":false'), true)
check("…the live Quick Combat", has(w, '"quick_combat":true'), true)

-- 2. Once per turn: a second turn-end callback for the same turn is silent.
handlers.LocalPlayerTurnEnd()
check("no second event for the same turn", #events("end_turn_wait"), 1)

-- 3. A turn whose end was never requested by us reports nothing.
Game.GetCurrentGameTurn = function() return 8 end
handlers.LocalPlayerTurnEnd()
check("no event for a turn we never asked to end", #events("end_turn_wait"), 1)

-- 4. A host without the predicates (older build) still ends the turn.
UI.IsGameCoreBusy, UI.CanEndTurn, UI.IsProcessingMessages = nil, nil, nil
NotificationManager, UserConfiguration = nil, nil
now = 20.0
local before = requested
local ok = pcall(function() queue.requestEndTurn(8) end)
check("missing predicates do not stop the request", ok and requested == before + 1, true)
now = 20.5
handlers.LocalPlayerTurnEnd()
check("…and the turn still reports", #events("end_turn_wait"), 2)
check("…with nothing counted", has(events("end_turn_wait")[2], '"busy":0'), true)

-- 5. An AI phase that never hands the turn back (G84 t117): 30 s after our
-- turn ended with the turn number unchanged, one `ai_phase_stall` with what
-- the UI shows. A turn that advances, or a second pulse, says nothing more.
check("the HUD pulse asks for the stall check first",
	has(io.open(here .. "/CivvisControlAgent.lua"):read("*a"),
		"if rejected then CivvisQueue.noteUiPulse(source, rejected); return; end\n\tpcall(CivvisQueue.checkAiPhaseStall);"), true)
local turnNow = 8
Game.GetCurrentGameTurn = function() return turnNow end
UI.IsGameCoreBusy = function() return false end
local liveContext = rawget(_G, "ContextPtr")
ContextPtr = { LookUpControl = function(_, path)
	return { IsHidden = function() return path ~= "/InGame/DiplomacyActionView" end }
end }
now = 40.0
check("not before AiPhaseStallSeconds", queue.checkAiPhaseStall(), false)
now = 50.6
check("a stalled AI phase is named", queue.checkAiPhaseStall(), true)
local stall = events("ai_phase_stall")[1]
check("…for our turn", has(stall, '"turn":8'), true)
check("…with how long since our turn ended", has(stall, '"waited":30.1'), true)
check("…and which views were up", has(stall, '"visible":["DiplomacyActionView"]'), true)
check("…and the core's own busy flag", has(stall, '"core_busy":false'), true)
now = 70.0
check("once per turn", queue.checkAiPhaseStall(), false)
check("…one event", #events("ai_phase_stall"), 1)
turnNow = 9
now = 90.0
check("a turn that advanced is never a stall", queue.checkAiPhaseStall(), false)
ContextPtr = liveContext

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("\nall end-turn-wait checks passed")
