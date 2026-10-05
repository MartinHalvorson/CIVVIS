-- Where a board export spends its time: `CivvisExportClock` marks the
-- sections of `exportState` and emits one `export_timing` per export with
-- milliseconds per section, timed on `os.rawclock()/os.clockpersecond()`.
--
-- The full export walks the whole host API (replan_frame_test switches it
-- off for that reason), so the clock is checked directly and the marks'
-- placement is read from the shipped source.
--
-- Run: lua5.1 tools/civ6_control/mod/export_timing_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end

local EXPORTS = { CivvisApplyOrders = true, CivvisQueue = true, CivvisResolveActions = true,
                  CivvisFrames = true, CivvisTiles = true, CivvisLedger = true, CivvisBoard = true,
                  CivvisExportTiles = true, CivvisOrdersReady = true, CivvisFetchOrders = true,
                  CivvisApplyOrder = true, CivvisBeginTurn = true, CivvisSettleTurn = true,
                  CivvisSurvey = true }
local LOG = {}
Automation = { Log = function(line) LOG[#LOG + 1] = line end }
-- `ExportState` OFF: the full board export walks the whole host API and
-- this stub does not model it. The TILE exporter — the frame trigger — is
-- the one export the test needs, so `CivvisTiles.sweep` is wrapped below to
-- switch the flag on around itself.
CivvisControlConfig = { ReplanFrames = 2, ExportState = false, TileExportEvery = 25,
                        OrdersPollTicks = 1, CombatFramePolls = 20, OrdersWaitPolls = 40,
                        OrderQueueGraceTicks = 30, OrderQueueMaxTicks = 240,
                        CivvisDecides = true, OrdersDb = "/tmp/fake.sqlite", RunTag = "test-run" }
UnitOperationTypes = { PARAM_X = "x", PARAM_Y = "y", FOUND_CITY = "UNITOPERATION_FOUND_CITY" }
UnitCommandTypes = {}
OperationResultsTypes = { ALL = 1 }
UnitOperationResults = { FAILURE_REASONS = "reasons" }

-- An 8x4 map. `revealed[key]` is what the host answers; a unit that lands
-- on a hex reveals the ring around it.
local W, H = 8, 4
local revealed, owner = {}, {}
local function key(x, y) return y * W + x end
local function plotObject(x, y)
	return {
		GetX = function() return x end, GetY = function() return y end,
		GetOwner = function() return owner[key(x, y)] or -1 end,
		GetTerrainType = function() return -1 end, GetFeatureType = function() return -1 end,
		GetResourceType = function() return -1 end, GetImprovementType = function() return -1 end,
		GetDistrictType = function() return -1 end, GetWonderType = function() return -1 end,
		GetRouteType = function() return -1 end, GetContinentType = function() return -1 end,
		IsWater = function() return false end, IsImpassable = function() return false end,
		IsFreshWater = function() return false end, IsRiver = function() return false end,
		IsWOfRiver = function() return false end, IsNWOfRiver = function() return false end,
		IsNEOfRiver = function() return false end, IsRoutePillaged = function() return false end,
		IsImprovementPillaged = function() return false end,
	}
end
local function dist(x1, y1, x2, y2) return math.max(math.abs(x1 - x2), math.abs(y1 - y2)) end
local function reveal(x, y)
	for dy = -1, 1 do for dx = -1, 1 do
		local px, py = x + dx, y + dy
		if px >= 0 and px < W and py >= 0 and py < H then revealed[key(px, py)] = true end
	end end
end
Map = { GetPlotDistance = function(x1, y1, x2, y2) return dist(x1, y1, x2, y2) end,
        GetGridSize = function() return W, H end,
        GetPlot = function(x, y) return plotObject(x, y) end,
        GetPlotIndex = function(x, y) return key(x, y) end,
        GetPlotByIndex = function(index) return plotObject(index % W, math.floor(index / W)) end,
        GetAdjacentPlot = function() return nil end }
TerrainManager = { GetCoastalLowlandType = function() return -1 end }
local function emptyTable()
	return setmetatable({}, {
		__index = function() return nil end,
		__call = function() return function() return nil end end,
	})
end
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
	return emptyTable()
end })
-- The orders channel: the test writes what the brain would have answered.
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

-- ---------------------------------------------------------------- fake host
-- MOVE_TO walks the unit as far as its movement allows along a straight
-- line (one hex per movement point), spends it, and reveals the ring where
-- it lands. `GetMoveToPathEx` prices the same line: hexes within the unit's
-- movement land this turn, the rest next turn, so `CivvisBoard.capToTurn`
-- caps exactly as on the real host.
local host = { units = {}, ops = {}, founded = {} }
local PID = 0
local function unitObject(u)
	return {
		GetID = function() return u.id end, GetX = function() return u.x end, GetY = function() return u.y end,
		GetMovesRemaining = function() return u.moves end, GetUnitType = function() return u.kind end,
		GetType = function() return u.kind end, GetDamage = function() return 0 end,
		GetGreatPerson = function() return nil end, GetFortifyTurns = function() return 0 end,
		GetFormationUnitCount = function() return 1 end,
		GetAttacksRemaining = function() return u.attacks or 1 end,
		GetComponentID = function() return { player = 0, id = u.id } end,
	}
end
local function line(u, x, y)
	local plots, turns = { key(u.x, u.y) }, { 0 }
	local cx, cy, steps = u.x, u.y, 0
	while cx ~= x or cy ~= y do
		if cx < x then cx = cx + 1 elseif cx > x then cx = cx - 1 end
		if cy < y then cy = cy + 1 elseif cy > y then cy = cy - 1 end
		steps = steps + 1
		plots[#plots + 1] = key(cx, cy)
		turns[#turns + 1] = (steps <= (u.moves or 0)) and 1 or 2
	end
	return plots, turns
end
function host.walk(u, x, y)
	local plots, turns = line(u, x, y)
	for i = 2, #plots do
		if turns[i] <= 1 then
			u.x, u.y = plots[i] % W, math.floor(plots[i] / W)
			u.moves = u.moves - 1
			reveal(u.x, u.y)
		end
	end
end
function host.land(id)
	local u = host.units[id]
	if u.pendingX ~= nil then
		local x, y = u.pendingX, u.pendingY
		u.pendingX, u.pendingY = nil, nil
		host.walk(u, x, y)
	end
end
UnitManager = {
	GetUnit = function(pid, id)
		local u = host.units[id]
		if u == nil or u.gone then return nil end
		return unitObject(u)
	end,
	CanStartOperation = function(unit, hash, _, params)
		local u = host.units[unit.GetID()]
		if hash == "UNITOPERATION_FOUND_CITY" and (u.moves or 0) <= 0 then return false end
		return true
	end,
	RequestOperation = function(unit, hash, params)
		local u = host.units[unit.GetID()]
		host.ops[#host.ops + 1] = { id = u.id, op = hash,
			x = params and params.x or nil, y = params and params.y or nil }
		if hash == "UNITOPERATION_MOVE_TO" then
			if host.slow then
				-- Asynchronous, like the host: the walk lands when the test
				-- says so (`host.land`).
				u.pendingX, u.pendingY = params.x, params.y
				return
			end
			host.walk(u, params.x, params.y)
		elseif hash == "UNITOPERATION_RANGE_ATTACK" then
			u.moves = 0; u.attacks = 0
		elseif hash == "UNITOPERATION_FOUND_CITY" then
			host.founded[#host.founded + 1] = { id = u.id, x = u.x, y = u.y }
			u.gone = true
		end
	end,
	GetMoveToPathEx = function(unit, plotIndex)
		local u = host.units[unit.GetID()]
		local plots, turns = line(u, plotIndex % W, math.floor(plotIndex / W))
		return { plots = plots, turns = turns }
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
		for _, u in pairs(host.units) do if not u.gone then objs[#objs + 1] = unitObject(u) end end
		table.sort(objs, function(a, b) return a.GetID() < b.GetID() end)
		return { Members = members(objs) }
	end,
	GetCities = function() return { Members = members({}) } end,
	GetDiplomacy = function() return { IsAtWarWith = function() return false end } end,
	GetResources = function() return { IsResourceVisible = function() return false end } end,
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
PlayersVisibility = setmetatable({}, { __index = function()
	return { IsVisible = function() return true end,
	         IsRevealed = function(_, x, y) return revealed[key(x, y)] == true end }
end })
Game = { GetLocalPlayer = function() return PID end, GetCurrentGameTurn = function() return 12 end }
CombatManager = { SimulateAttackInto = function() return nil end }
Locale = { Lookup = function(s) return tostring(s) end }

local raw, per, auto, cpu = 1000, 1000, 50.0, 7.0
os.rawclock = function() return raw end
os.clockpersecond = function() return per end
os.clock = function() return cpu end
Automation.GetTime = function() return auto end
local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtime_err = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtime_err))
local clock = rawget(_G, "CivvisExportClock")

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

check("CivvisExportClock is exported", type(clock), "table")

-- 1. Sections are the differences between consecutive marks.
clock.begin()
raw = raw + 12; clock.mark("cities")         -- 12 ms
raw = raw + 150; clock.mark("rivals")        -- 150 ms
raw = raw + 3; auto = auto + 0.165; cpu = cpu + 0.4
clock.mark("state_table_and_emit")           -- 3 ms
clock.report(40, 2)
local e = events("export_timing")
check("one export_timing per export", #e, 1)
check("…per-section cities", has(e[1], '"cities":12'), true)
check("…per-section rivals", has(e[1], '"rivals":150'), true)
check("…per-section emit", has(e[1], '"state_table_and_emit":3'), true)
check("…total", has(e[1], '"total_ms":165'), true)
check("…Automation.GetTime cross-check", has(e[1], '"auto_total":165'), true)
check("…os.clock cross-check", has(e[1], '"cpu_total_ms":400'), true)
check("…turn and frame", has(e[1], '"turn":40') and has(e[1], '"frame":2'), true)

-- 2. A report without a begin, or a second report, says nothing.
clock.report(40, 2)
check("report is once per begin", #events("export_timing"), 1)
clock.mark("stray")
check("a mark outside an export is ignored", clock.marks, nil)

-- 3. A host without the raw clock still reports the cross-checks.
os.rawclock = nil
clock.begin(); auto = auto + 0.2; clock.mark("cities"); clock.report(41, 0)
e = events("export_timing")
check("no raw clock: still one event", #e, 2)
check("…sections fall back to Automation.GetTime", has(e[2], '"cities":200'), true)
check("…saying which clock timed them", has(e[2], '"clock":"auto"'), true)
check("…and the Automation total", has(e[2], '"auto_total":200'), true)
check("the raw clock says so too", has(e[1], '"clock":"raw"'), true)

-- 4. The marks sit in `exportState`, in its order, around the state emit.
local source = assert(io.open(here .. "/CivvisControlAgent.lua")):read("*a")
local first = source:find("local function exportState(player, pid, turn, frame, eventKind)", 1, true)
local last = source:find("\nend\n", source:find('CivvisExportClock.report(turn, frame);', first, true), true)
local body = source:sub(first, last)
local order = { "CivvisExportClock.begin();", 'mark("prelude")', 'mark("cities")', 'mark("units")',
	'mark("public_stats")', 'mark("rivals")', 'mark("minors")', 'mark("techs_civics_projects")',
	'mark("government_religion_policies")', 'emit(eventKind or "state", {',
	'mark("state_table_and_emit")', "CivvisExportClock.report(turn, frame);" }
local at = 0
for _, needle in ipairs(order) do
	local i = body:find(needle, at + 1, true)
	check("exportState: " .. needle .. " in order", i ~= nil, true)
	at = i or at
end
-- 5. The record after the state is LOAD-BEARING: the game writes its latest
-- log record only when the next one is logged, so the report must follow the
-- state unconditionally -- only the final mark and comments between them.
local stateAt = body:find('emit(eventKind or "state", {', 1, true)
local closeAt = body:find("\n\t});\n", stateAt, true)
local reportAt = body:find("CivvisExportClock.report(turn, frame);", closeAt, true)
local between = body:sub(closeAt + 5, reportAt - 1)
local code = {}
for line in between:gmatch("[^\n]+") do
	local trimmed = line:match("^%s*(.-)%s*$")
	if trimmed ~= "" and trimmed:sub(1, 2) ~= "--" then code[#code + 1] = trimmed end
end
check("only the final mark sits between the state and its releasing record",
	#code == 1 and code[1] == 'CivvisExportClock.mark("state_table_and_emit");', true)
check("…and the report is the export's last statement",
	body:sub(reportAt):match("^CivvisExportClock%.report%(turn, frame%);%s*$") ~= nil, true)
check("…begin after the ExportState guard",
	body:find("if cfg.ExportState ~= true then return; end", 1, true)
		< body:find("CivvisExportClock.begin();", 1, true), true)

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("\nall export-timing checks passed")
