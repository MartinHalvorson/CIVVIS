-- Firaxis self-play (`FiraxisSelfPlay`): the control mod hands our seat to the
-- game's own autoplay on its first turn and plays nothing, so one real game
-- with every seat played by Firaxis's AI can be timed (`CivvisSelfPlay` in
-- CivvisControlAgent.lua; `civ6_play.py --firaxis-self-play`).
--
-- What this holds, on a faked host:
--   * the hand-over is the four calls of the shipped "PlayGame" test
--     (Base/Assets/UI/Automation/Automation_StandardTests.lua:330-334), made
--     once, on our seat's first turn, after the seat is read back and after
--     the lane's timescale is applied;
--   * the rows the harness reads are written (`autoplay`, a `turn` a game
--     turn, the stock `victory`, `autoplay_end`);
--   * no order reaches the host;
--   * with the key absent the mod listens, starts and reports as it did.
--
-- It cannot hold what only a live game can: that `AutoplayManager` exists in
-- this context of this mod, and what the engine does once it has the seat.
--
-- Loads the SHIPPED `CivvisControlAgent.lua`.
--
-- Run: lua5.1 tools/civ6_control/mod/firaxis_self_play_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."
-- The agent prints every row it logs; this test's own lines are what is read.
local say = print
print = function() end

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end

-- Globals that read as nil, the way an undefined global does in the game.
local ABSENT = {}
setmetatable(_G, { __index = function(_, k)
	if ABSENT[k] then return nil end
	return stub()
end })

-- ---------------------------------------------------------------- fake host
local host
local function listeners(into)
	return setmetatable({}, { __index = function(_, name)
		return setmetatable({
			Add = function(handler)
				host[into][name] = host[into][name] or {}
				table.insert(host[into][name], handler)
			end,
		}, { __call = function()
			host.raised[name] = (host.raised[name] or 0) + 1
		end })
	end })
end
local function order()
	host.orders = host.orders + 1
end
local function ordering(fields)
	for _, name in ipairs({ "RequestOperation", "RequestCommand", "RequestAction",
	                        "RequestPlayerOperation" }) do
		fields[name] = order
	end
	return setmetatable(fields, { __index = function() return stub() end })
end

Automation = {
	Log = function(line) host.log[#host.log + 1] = line end,
	GetTime = function() return host.clock end,
}
Events = listeners("events")
LuaEvents = listeners("lua")
ExposedMembers = {}
-- The rule tables read as empty: walking one ends, and a lookup finds no row.
-- (A stub would be walked for ever by the seat survey's `GameInfo.Victories()`.)
GameInfo = setmetatable({}, { __index = function()
	return setmetatable({}, { __call = function() return function() return nil end end })
end })
Game = setmetatable({
	GetLocalPlayer = function() return host.localPlayer end,
	GetCurrentGameTurn = function() return host.turn end,
	GetMaxGameTurns = function() return 250 end,
}, { __index = function() return stub() end })
PlayerManager = setmetatable({
	GetAliveMajorIDs = function() return { 0, 1, 2, 3, 4, 5 } end,
}, { __index = function() return function() return {} end end })
Players = setmetatable({}, { __index = function(_, id)
	return setmetatable({
		GetTeam = function() return id end,
		GetScore = function() return 100 + id end,
		IsHuman = function() return id == host.localPlayer end,
	}, { __index = function() return stub() end })
end })
PlayerConfigurations = setmetatable({}, { __index = function(_, id)
	return setmetatable({
		GetCivilizationTypeName = function() return "CIVILIZATION_" .. id end,
		GetLeaderTypeName = function() return "LEADER_" .. id end,
	}, { __index = function() return stub() end })
end })
UI = ordering({ GetElapsedTime = function() return host.ui end })
UnitManager = ordering({})
CityManager = ordering({})
-- The engine console the timescale goes through, on the same list as the
-- manager's calls so their order can be read.
AutoProfiler = { RunCommand = function(command)
	host.calls[#host.calls + 1] = "console " .. command
	return true
end }
local function manager()
	local function call(name, effect)
		return function(value)
			host.calls[#host.calls + 1] = name .. " " .. tostring(value)
			if effect then effect(value) end
		end
	end
	return {
		SetTurns = call("SetTurns"),
		SetReturnAsPlayer = call("SetReturnAsPlayer"),
		SetObserveAsPlayer = call("SetObserveAsPlayer"),
		-- GameCapabilities.lua:18 reads the local player as -1 under autoplay.
		SetActive = call("SetActive", function(on)
			if on then host.localPlayer = -1 end
		end),
	}
end

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		say(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		say(string.format("ok   %s = %s", name, tostring(got)))
	end
end
local function rows(kind)
	local out = {}
	for _, line in ipairs(host.log) do
		if line:find('"kind":"' .. kind .. '"', 1, true) then out[#out + 1] = line end
	end
	return out
end
local function at(kind)
	for index, line in ipairs(host.log) do
		if line:find('"kind":"' .. kind .. '"', 1, true) then return index end
	end
	return nil
end
local function has(line, needle)
	return line ~= nil and line:find(needle, 1, true) ~= nil
end
local function called(prefix)
	local n = 0
	for _, call in ipairs(host.calls) do
		if call:sub(1, #prefix) == prefix then n = n + 1 end
	end
	return n
end
local function fire(into, name, ...)
	for _, handler in ipairs(host[into][name] or {}) do handler(...) end
end

-- A fresh game under `config`. `options.manager == false` leaves the game
-- without an autoplay manager.
local function load(config, options)
	options = options or {}
	host = { localPlayer = 0, turn = 1, clock = 1000, ui = 10.0, orders = 0,
	         calls = {}, log = {}, events = {}, lua = {}, raised = {} }
	ABSENT.AutoplayManager = options.manager == false
	rawset(_G, "AutoplayManager", options.manager ~= false and manager() or nil)
	for key in pairs(ExposedMembers) do ExposedMembers[key] = nil end
	config.RunTag = config.RunTag or "firaxis-selfplay-test"
	config.Rehost = false
	CivvisControlConfig = config
	local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
	assert(chunk, "could not load agent: " .. tostring(err))
	local ran, raised = pcall(chunk)
	assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(raised))
end
-- The game core publishes through the load; three publishes in, `ensureStarted`
-- has a seat and a turn to read.
local function publish(times)
	for _ = 1, times or 4 do fire("events", "GameCoreEventPublishComplete") end
end
local function selfPlay(extra)
	local config = { FiraxisSelfPlay = true, Play = false, MaxTurns = 250,
	                 DebugTimeScale = 3 }
	for key, value in pairs(extra or {}) do config[key] = value end
	return config
end

-- ── 1. What a self-play run listens to ──
load(selfPlay())
check("the run says it is armed", #rows("self_play"), 1)
check("…and what the autoplay manager is here", has(rows("self_play")[1], '"manager":"table"'), true)
for _, name in ipairs({ "LocalPlayerTurnBegin", "GameCoreEventPublishComplete",
                        "LoadGameViewStateDone", "TeamVictory", "PlayerDefeat",
                        "TurnBegin", "UserRequestClose" }) do
	check("listens to " .. name, type((host.events[name] or {})[1]), "function")
end
for _, name in ipairs({ "EndTurnBlockingChanged", "LocalPlayerTurnEnd", "UnitMoveComplete",
                        "CityProductionCompleted", "DiplomacyIncomingDeal",
                        "DiplomacyStatement", "EmergencyAvailable", "GovernorAppointed",
                        "CombatVisBegin", "SpyMissionCompleted" }) do
	check("does not listen to " .. name, host.events[name], nil)
end
check("the manager's end is listened for", type((host.lua.AutoPlayEnd or {})[1]), "function")
check("…and its start", type((host.lua.AutoPlayStart or {})[1]), "function")

-- ── 2. The load: the seat is read back, nothing is handed over yet ──
publish()
check("the game core's publishes hand nothing over", #host.calls, 0)
check("no turn row before our seat's turn", #rows("turn"), 0)
check("the seat was read back while it was ours", #rows("seat"), 1)
check("…as the local player", has(rows("seat")[1], '"local_player":0'), true)

-- ── 3. Our seat's first turn: timescale, then the four calls, once ──
fire("events", "LocalPlayerTurnBegin")
check("the lane's timescale, then the shipped test's four calls, in its order",
	table.concat(host.calls, " | "),
	"console timescale 3 | SetTurns 260 | SetReturnAsPlayer 0 | SetObserveAsPlayer 0 | SetActive true")
local autoplay = rows("autoplay")
check("one autoplay row", #autoplay, 1)
check("…started", has(autoplay[1], '"started":true'), true)
check("…ten turns past the game's own limit", has(autoplay[1], '"turns":260'), true)
check("…our seat, observed", has(autoplay[1], '"seat":0') and has(autoplay[1], '"observe":0'), true)
check("…at the lane's scale", has(autoplay[1], '"scale":3'), true)
check("…with every major", has(autoplay[1], '"civ":"CIVILIZATION_5"'), true)
check("the seat row came before the hand-over", at("seat") < at("autoplay"), true)
local turns = rows("turn")
check("the hand-over is the first turn row", #turns, 1)
check("…turn 1", has(turns[1], '"turn":1') and has(turns[1], '"self_play":true'), true)
check("…with the seat's score", has(turns[1], '"score":100'), true)
check("…and the write time the harness times the game by", has(turns[1], '"t":1000'), true)
check("the hand-over row came before it", at("autoplay") < at("turn"), true)
-- The game writes its last record only when the next one is logged.
check("a record behind the first turn, or it waits for the second",
	has(host.log[#host.log], '"kind":"self_play_flush"') and has(host.log[#host.log], '"after":"turn"'), true)

-- ── 4. Under autoplay: a turn row a game turn, nothing handed over twice ──
check("the local player is nobody now", Game.GetLocalPlayer(), -1)
fire("lua", "AutoPlayStart")
check("the engine's own word is recorded", #rows("autoplay_start"), 1)
host.turn, host.clock = 2, 1003
fire("events", "TurnBegin", 2)
fire("events", "TurnBegin", 2)
host.turn, host.clock = 3, 1005
fire("events", "TurnBegin", 3)
publish()
fire("events", "LocalPlayerTurnBegin")
turns = rows("turn")
check("one row a game turn", #turns, 3)
check("…turn 2", has(turns[2], '"turn":2') and has(turns[2], '"t":1003'), true)
check("…turn 3", has(turns[3], '"turn":3'), true)
check("the seat is handed over once", called("SetActive"), 1)
check("no order reached the host", host.orders, 0)

-- ── 5. The game is won: the stock victory row, and the turn rows stop ──
host.turn, host.clock = 187, 1500
fire("events", "TurnBegin", 187)
check("the turn the game ends on names every major", has(rows("turn")[4], '"leader":"LEADER_3"'), true)
fire("events", "TeamVictory", 3, 5, 0)
local victory = rows("victory")
check("one victory row", #victory, 1)
check("…the winning team and turn", has(victory[1], '"team":3') and has(victory[1], '"turn":187'), true)
check("…nobody's, with no local player", has(victory[1], '"won":false'), true)
check("…and its write time", has(victory[1], '"t":1500'), true)
check("the timescale is given back", host.calls[#host.calls], "console timescale 1")
check("a record behind the victory, or the game never writes it to the log",
	has(host.log[#host.log], '"after":"victory"') and at("victory") < #host.log, true)
local flushes = #rows("self_play_flush")
publish(1)
publish(1)
check("…and one more at the next publish, whichever listener ran first",
	#rows("self_play_flush"), flushes + 1)
local before = #rows("turn")
host.turn = 188
fire("events", "TurnBegin", 188)
check("no turn row after the game is over", #rows("turn"), before)
-- The engine may stop its autoplay at the end of the game: said, not an ending.
fire("lua", "AutoPlayEnd")
check("an autoplay end after a victory says so", has(rows("autoplay_end")[1], '"after_victory":true'), true)

-- ── 6. Should the engine name our seat as the local player while it observes ──
-- The stock handlers are untouched, so they would call that seat's defeat
-- ours and its win won; the harness reads a self-play game's rows as
-- nobody's (`civ6_play.firaxis_self_play_unseat`). What the mod owes is that
-- the game goes on being reported after the seat is out of it.
load(selfPlay())
publish()
fire("events", "LocalPlayerTurnBegin")
host.localPlayer = 0
fire("events", "PlayerDefeat", 0, 1, 0)
check("the stock defeat row is written as it always was", has(rows("defeat")[1], '"ours":true'), true)
host.turn = 90
fire("events", "TurnBegin", 90)
check("the game is still reported after the seat is out", has(rows("turn")[#rows("turn")], '"turn":90'), true)
fire("events", "TeamVictory", 4, 5, 0)
check("…to its victory", has(rows("victory")[1], '"team":4'), true)

-- ── 7. The manager hands the seat back with nobody having won ──
load(selfPlay())
publish()
fire("events", "LocalPlayerTurnBegin")
host.turn = 250
fire("events", "TurnBegin", 250)
host.localPlayer = 0
fire("lua", "AutoPlayEnd")
fire("lua", "AutoPlayEnd")
local ended = rows("autoplay_end")
check("one autoplay_end row", #ended, 1)
check("…on the turn it ended", has(ended[1], '"turn":250'), true)
check("…with no victory before it", has(ended[1], '"after_victory":false'), true)
check("…and the timescale given back", host.calls[#host.calls], "console timescale 1")
check("…and a record behind it", has(host.log[#host.log], '"after":"autoplay_end"'), true)
host.turn = 251
fire("events", "TurnBegin", 251)
check("no turn row after the hand-back", has(rows("turn")[#rows("turn")], '"turn":250'), true)
fire("events", "LocalPlayerTurnBegin")
publish()
check("…and the seat is not handed over again", called("SetActive"), 1)
check("…and still no order", host.orders, 0)

-- ── 8. The harness's quit is answered, by the run's own listener ──
fire("events", "UserRequestClose")
check("the quit is confirmed once", host.raised.UserConfirmedClose, 1)

-- ── 9. A game with no autoplay manager says so and plays nothing ──
load(selfPlay(), { manager = false })
check("the armed row names the missing manager", has(rows("self_play")[1], '"manager":"nil"'), true)
publish()
fire("events", "LocalPlayerTurnBegin")
autoplay = rows("autoplay")
check("one autoplay row", #autoplay, 1)
check("…not started", has(autoplay[1], '"started":false'), true)
check("…and why", has(autoplay[1], "AutoplayManager is not available in this context"), true)
-- The board is live all the same, and the harness's setup waits to hear it;
-- the run is ended on the row above.
check("the turn row still says the board is live", #rows("turn"), 1)
check("…after the row that says nobody took the seat", at("autoplay") < at("turn"), true)
check("…with a record behind it", has(host.log[#host.log], '"kind":"self_play_flush"'), true)
check("no order", host.orders, 0)
host.turn = 2
fire("events", "TurnBegin", 2)
check("the game is nobody's, and its turn rows say what it does", #rows("turn"), 2)
fire("events", "LocalPlayerTurnBegin")
check("…and it is not tried again", #rows("autoplay"), 1)

-- ── 10. A manager that refuses a call says which ──
load(selfPlay())
AutoplayManager.SetActive = function() error("refused by the engine") end
publish()
fire("events", "LocalPlayerTurnBegin")
check("a raising call is not a start", has(rows("autoplay")[1], '"started":false'), true)
check("…and its message is kept", has(rows("autoplay")[1], "refused by the engine"), true)

-- ── 11. The two settings of the hand-over ──
load(selfPlay({ FiraxisSelfPlayObserve = "none" }))
publish()
fire("events", "LocalPlayerTurnBegin")
check("observe nobody sends -1", called("SetObserveAsPlayer -1"), 1)
check("…and still returns to our seat", called("SetReturnAsPlayer 0"), 1)
load(selfPlay({ MaxTurns = 0 }))
publish()
fire("events", "LocalPlayerTurnBegin")
check("a game with no turn limit asks for none", called("SetTurns 0"), 1)
load(selfPlay({ DebugTimeScale = false }))
publish()
fire("events", "LocalPlayerTurnBegin")
check("no timescale asked, none run", called("console"), 0)
check("…and the seat is still handed over", called("SetActive true"), 1)

-- ── 12. Our turn can begin before the seat has been read back ──
load(selfPlay())
fire("events", "LocalPlayerTurnBegin")
check("not handed over before the seat is read back", called("SetActive"), 0)
publish()
check("…handed over at the publish that reads it", called("SetActive true"), 1)
check("…after the seat row", at("seat") < at("autoplay"), true)

-- ── 13. Without the key, the mod is what it was ──
load({ Play = false, MaxTurns = 250, DebugTimeScale = 3 })
check("no armed row", #rows("self_play"), 0)
check("no turn hook", host.events.TurnBegin, nil)
check("no autoplay hooks", host.lua.AutoPlayEnd == nil and host.lua.AutoPlayStart == nil, true)
for _, name in ipairs({ "LocalPlayerTurnBegin", "LocalPlayerTurnEnd",
                        "GameCoreEventPublishComplete", "EndTurnBlockingChanged",
                        "UnitMoveComplete", "DiplomacyIncomingDeal", "EmergencyAvailable",
                        "TeamVictory", "PlayerDefeat", "CombatVisBegin" }) do
	check("still listens to " .. name, type((host.events[name] or {})[1]), "function")
end
check("one quit listener, the stock one", #host.events.UserRequestClose, 1)
publish()
fire("events", "LocalPlayerTurnBegin")
host.turn = 2
publish()
check("the manager is never called", called("Set"), 0)
check("no autoplay row", #rows("autoplay"), 0)
check("no self-play turn row", #rows("turn"), 0)
fire("events", "TeamVictory", 0, 5, 0)
check("our win is ours", has(rows("victory")[1], '"won":true'), true)
check("…with no self-play mark", has(rows("victory")[1], "self_play"), false)
check("…and no roster", has(rows("victory")[1], '"majors"'), false)
fire("events", "PlayerDefeat", 0, 1, 0)
check("our defeat is ours", has(rows("defeat")[1], '"ours":true'), true)
fire("events", "PlayerDefeat", 4, 1, 0)
check("a rival's is not", has(rows("defeat")[2], '"ours":false'), true)
-- `false` is off too, not only absent.
load({ Play = false, FiraxisSelfPlay = false })
check("false is off: no armed row", #rows("self_play"), 0)
check("…and the full listener list", type((host.events.EndTurnBlockingChanged or {})[1]), "function")
publish()
fire("events", "LocalPlayerTurnBegin")
check("…and the manager is never called", called("Set"), 0)

-- ── 14. The source: every read of the key is a comparison with `true` ──
local source = assert(io.open(here .. "/CivvisControlAgent.lua")):read("*a")
local reads, compared = 0, 0
for line in source:gmatch("[^\n]+") do
	if not line:match("^%s*%-%-") then
		for _ in line:gmatch("cfg%.FiraxisSelfPlay%f[^%w_]") do reads = reads + 1 end
		for _ in line:gmatch("cfg%.FiraxisSelfPlay [=~]= true") do compared = compared + 1 end
	end
end
check("the key is read", reads > 0, true)
check("…and only ever compared with true", compared, reads)
-- The four calls, as the shipped test makes them.
local beginAt = assert(source:find("function CivvisSelfPlay.begin()", 1, true))
local beginEnd = assert(source:find("\nend\n", beginAt, true))
local body = source:sub(beginAt, beginEnd)
local order = { "manager.SetTurns(turns);", "manager.SetReturnAsPlayer(pid);",
                "manager.SetObserveAsPlayer(observe);", "manager.SetActive(true);" }
local cursor = 1
for _, call in ipairs(order) do
	local found = body:find(call, cursor, true)
	check("the hand-over calls " .. call, found ~= nil, true)
	cursor = found or cursor
end
-- No new top-level local: the main chunk has no register to spare.
check("CivvisSelfPlay is a bare global", source:find("\nlocal CivvisSelfPlay", 1, true), nil)

if failures > 0 then
	say(failures .. " failure(s)")
	os.exit(1)
end
say("firaxis self-play: all ok")
