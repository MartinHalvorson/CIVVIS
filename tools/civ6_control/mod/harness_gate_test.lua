-- The person-facing install (`RequireHarness`, `tools/civ6_hotseat.py
-- --manual-lobby`) is stock Civ VI whenever its harness is not running. Each
-- claim "The person-facing install" in docs/CIV6_COMPUTER_CONTROL.md makes is
-- checked here, against the scripts as they load in the game:
--   1. the agent, the screen closer and the HUD carry one identical
--      `CivvisHarness` block;
--   2. `CivvisHarness`: unset it changes nothing; outside a hotseat game it is
--      dormant and reads nothing; in one it is live only on a fresh beat, read
--      at most every 3 s, never re-entered, and an error reads as no harness;
--   3. the agent in a single-player game registers no listener and writes no
--      line; in a hotseat game with no harness every listener it registered
--      stops at the seat gate -- the environment refuses every engine global
--      past it -- and writes nothing; live, it plays its own seat only;
--   4. the screen closer in a single-player game is the shipped screen: no
--      clock, `UI` unwrapped, no listener, no line. In a hotseat game with no
--      harness it closes nothing, presses no Start Turn and writes nothing;
--      live, it acts on CIVVIS's seats and on no other;
--   5. the HUD in a single-player game is the shipped HUD; in a hotseat game
--      with no harness it writes nothing and ends no turn, even with a stale
--      controller stamp left from an earlier game; live, it reports the lobby
--      exactly once.
-- Run: lua5.1 tools/civ6_control/mod/harness_gate_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."
local function read(name)
	local handle = assert(io.open(here .. "/" .. name, "r"))
	local text = handle:read("*a")
	handle:close()
	return text
end
local STD = {}
for _, k in ipairs({ "string", "table", "math", "pairs", "ipairs", "type", "tostring", "tonumber",
		"pcall", "select", "unpack", "next", "setmetatable", "getmetatable", "error", "assert" }) do
	STD[k] = _G[k]
end

-- 1. One block, three copies. ----------------------------------------------
local blocks = {}
for _, name in ipairs({ "CivvisControlAgent.lua", "CivvisControlAutoClose.lua",
		"CivvisControlHeartbeat.lua" }) do
	local block = read(name):match("(%-%- >>> CivvisHarness.-%-%- <<< CivvisHarness\n)")
	assert(block, name .. " carries the CivvisHarness block")
	blocks[#blocks + 1] = block
end
assert(blocks[1] == blocks[2] and blocks[2] == blocks[3],
	"the agent, the closer and the HUD must judge the harness the same way")

-- 2. The gate itself. ------------------------------------------------------
local function gate(cfg, opts)
	opts = opts or {}
	local h = { attaches = {}, queries = {}, now = 100 }
	local env = setmetatable({
		cfg = cfg,
		GameConfiguration = { IsHotseat = function() return opts.hotseat == true end },
		Automation = { GetTime = function() return h.now end },
		DB = { Query = function(sql)
			if sql:find("^ATTACH") then
				h.attaches[#h.attaches + 1] = sql
				if opts.attachFails then error("database civvis is already in use") end
				return {}
			end
			h.queries[#h.queries + 1] = sql
			if opts.query then return opts.query(sql, h) end
			return {}
		end },
	}, { __index = STD })
	local chunk = assert(loadstring(blocks[1]))
	setfenv(chunk, env)
	chunk()
	h.H, h.env = env.CivvisHarness, env
	return h
end
local PERSON = { RequireHarness = true, HarnessRun = "hs-test", OrdersDb = "/runs/hs-test/orders.sqlite",
	HotseatSeats = { 1, 2 }, HotseatStubSeats = { 0 } }
local function rows(age) return function() return { { age = age } } end end

-- Unset: every existing install. Live, never dormant, and nothing is read.
local unset = gate({ HotseatSeats = { 1 } }, { hotseat = true })
assert(unset.H.live() == true and unset.H.dormant() == false)
assert(#unset.queries == 0 and #unset.attaches == 0, "an install without the key reads nothing")

-- The person's own single-player game: dormant, and the database is never touched.
local solo = gate(PERSON, { hotseat = false, query = rows(1) })
assert(solo.H.dormant() == true, "outside hotseat the person-facing install is dormant")
assert(solo.H.live() == false and solo.H.mine(1) == false)
assert(#solo.queries == 0 and #solo.attaches == 0, "a single-player game never reads the channel")

-- A hotseat game and no harness: not live. One ATTACH, the run's own row.
local none = gate(PERSON, { hotseat = true })
assert(none.H.dormant() == false)
assert(none.H.live() == false, "no beat, no harness")
assert(#none.attaches == 1 and none.attaches[1]:find("/runs/hs-test/orders.sqlite", 1, true))
assert(none.queries[1]:find("strftime('%s','now')", 1, true), "the age is SQLite's own clock")
assert(none.queries[1]:find("FROM civvis.harness WHERE run = 'hs-test'", 1, true))
assert(none.H.mine(1) == false and none.H.mine(0) == false, "no harness, no CIVVIS turn")

-- A fresh beat is live; stale, from the future or unreadable is not.
local fresh = gate(PERSON, { hotseat = true, query = rows(3) })
assert(fresh.H.live() == true, "a beat 3 s old is a live harness")
assert(fresh.H.mine(1) and fresh.H.mine(2) and fresh.H.mine(0), "its seats and the stub are CIVVIS's")
assert(not fresh.H.mine(3) and not fresh.H.mine(-1), "every other seat is a person's")
for _, age in ipairs({ 21, 60, -30 }) do
	assert(gate(PERSON, { hotseat = true, query = rows(age) }).H.live() == false,
		"a beat aged " .. age .. " s is no harness")
end
assert(gate(PERSON, { hotseat = true, query = function() return { { age = nil } } end }).H.live() == false)
assert(gate(PERSON, { hotseat = true, query = function() error("no such table: civvis.harness") end })
	.H.live() == false, "an unreadable channel is no harness")
assert(gate(PERSON, { hotseat = true, attachFails = true, query = rows(1) }).H.live() == true,
	"an alias another context attached first is read through")

-- Read at most every 3 s of the UI clock, then again.
local cached = gate(PERSON, { hotseat = true, query = rows(1) })
assert(cached.H.live() and #cached.queries == 1)
cached.now = 102.9
assert(cached.H.live() and #cached.queries == 1, "inside 3 s the last reading stands")
cached.now = 103
assert(cached.H.live() and #cached.queries == 2, "after 3 s it is read again")
assert(#cached.attaches == 1, "ATTACH once per context")
-- ...once a read has gone through it; until then each reading tries again.
local flaky = gate(PERSON, { hotseat = true, query = function(_, h)
	if not h.ready then error("no such table: civvis.harness") end
	return { { age = 1 } }
end })
assert(flaky.H.live() == false and #flaky.attaches == 1)
flaky.ready, flaky.now = true, 104
assert(flaky.H.live() == true and #flaky.attaches == 2, "a failed first ATTACH is retried")
flaky.now = 108
assert(flaky.H.live() == true and #flaky.attaches == 2, "and a working one is not repeated")
-- A harness stopping is seen on the next reading.
local stopping = gate(PERSON, { hotseat = true, query = function(_, h)
	return h.stopped and {} or { { age = 1 } }
end })
assert(stopping.H.live() == true)
stopping.stopped, stopping.now = true, 104
assert(stopping.H.live() == false and stopping.H.mine(1) == false, "a deleted row stands it down")

-- Never re-entered: a read that asks again gets the standing answer.
local nested = 0
local reentry
reentry = gate(PERSON, { hotseat = true, query = function()
	nested = nested + 1
	assert(reentry.H.live() == false, "a nested ask returns the standing answer")
	return { { age = 1 } }
end })
assert(reentry.H.live() == true and nested == 1)

-- No clock at all: read on the first call, then once every 200 calls.
local clockless = gate(PERSON, { hotseat = true, query = rows(1) })
clockless.env.Automation.GetTime = function() return nil end
for _ = 1, 199 do clockless.H.live() end
assert(#clockless.queries == 1, "without a clock it does not query per call")
clockless.H.live()
assert(#clockless.queries == 2)

-- A quote or backslash cannot leave the SQL literal.
local quoted = gate({ RequireHarness = true, HarnessRun = "a'b\\c", OrdersDb = "/x'y" }, { hotseat = true })
quoted.H.live()
assert(quoted.attaches[1] == "ATTACH DATABASE '/xy' AS civvis")
assert(quoted.queries[1]:find("run = 'abc'", 1, true))
print("ok   CivvisHarness")

-- 3. The agent, whole. -----------------------------------------------------
-- Past load the environment refuses every engine global but the four the
-- seat gate reads, so a listener that got past the gate fails this test.
local function agent(cfg, opts)
	-- `past` records each refusal too: the agent's handlers guard their own
	-- bodies, so the error alone could be swallowed.
	local h = { listeners = {}, logs = {}, strict = false, past = {} }
	local function events(kind)
		return setmetatable({}, { __index = function(_, name)
			return { Add = function(fn) h.listeners[#h.listeners + 1] = { kind = kind, name = name, fn = fn } end,
				Remove = function() end }
		end })
	end
	local env = setmetatable({
		CivvisControlConfig = cfg, Events = events("Events"), LuaEvents = events("LuaEvents"),
		ExposedMembers = {},
		GameConfiguration = { IsHotseat = function() return opts.hotseat end },
		Game = { GetLocalPlayer = function() return h.localPlayer end },
		PlayerConfigurations = setmetatable({}, { __index = function()
			return { GetWantsPause = function() return false end }
		end }),
		Automation = { Log = function(line) h.logs[#h.logs + 1] = line end, GetTime = function() return 50 end },
		DB = { Query = function(sql)
			if sql:find("civvis.harness", 1, true) then return opts.beat and { { age = 1 } } or {} end
			return {}
		end },
	}, { __index = function(_, key)
		if STD[key] ~= nil then return STD[key] end
		if h.strict then
			h.past[#h.past + 1] = tostring(key)
			error("reached " .. tostring(key) .. " past the seat gate", 2)
		end
		return nil
	end })
	local chunk = assert(loadfile(here .. "/CivvisControlAgent.lua"))
	setfenv(chunk, env)
	chunk()
	h.strict, h.env = true, env
	return h
end
local SEAT = { RequireHarness = true, Hotseat = true, HotseatSeat = 1, HotseatSeats = { 1, 2 },
	HarnessRun = "hs-test", OrdersDb = "/runs/hs-test/orders.sqlite", RunTag = "hs-test-p1",
	CivvisDecides = true, Play = true }

local spAgent = agent(SEAT, { hotseat = false })
assert(#spAgent.listeners == 0, "a single-player game gets no listener from a seat context")
assert(#spAgent.logs == 0, "and not one line")

local idle = agent(SEAT, { hotseat = true })
assert(#idle.listeners > 20, "a hotseat game registers the seat's (gated) listeners")
assert(#idle.logs == 0, "with no harness the load writes nothing")
for _, seat in ipairs({ 1, 0, 2 }) do
	idle.localPlayer = seat
	for _, listener in ipairs(idle.listeners) do
		local ok, err = pcall(listener.fn, 1, 2, 3, 4, 5)
		assert(ok, listener.kind .. "." .. listener.name .. " went past the gate: " .. tostring(err))
	end
end
assert(#idle.past == 0, "a listener reached " .. tostring(idle.past[1]) .. " past the seat gate")
assert(#idle.logs == 0, "no listener writes a line without a harness")
idle.localPlayer = 1
assert(idle.env.CivvisSeat.mine() == false, "its own seat is a person's without a harness")

local playing = agent(SEAT, { hotseat = true, beat = true })
playing.localPlayer = 1
assert(playing.env.CivvisSeat.mine() == true, "live, the seat plays its own turn")
playing.localPlayer = 2
assert(playing.env.CivvisSeat.mine() == false, "and no other seat's")
-- Not the person's Exit either: their quit keeps the game's own question.
local quit = playing.env.Events
local confirmed = false
playing.env.Events = setmetatable({ UserConfirmedClose = function() confirmed = true end }, { __index = quit })
for _, listener in ipairs(playing.listeners) do
	if listener.name == "UserRequestClose" then
		playing.localPlayer = 1
		listener.fn()
	end
end
assert(not confirmed, "the person-facing install never confirms a quit")
print("ok   agent")

-- 4. The screen closer, whole. ---------------------------------------------
local function closer(name, cfg, opts)
	local h = { logs = {}, closes = 0, presses = 0, hidden = false, listeners = 0, includes = {},
		localPlayer = opts.localPlayer, now = 10 }
	local ui = { ReferenceCurrentEvent = function() return 7 end, ReleaseEventID = function() end }
	h.ui, h.reference = ui, ui.ReferenceCurrentEvent
	local function events()
		return setmetatable({}, { __index = function()
			return setmetatable({ Add = function() h.listeners = h.listeners + 1 end, Remove = function() end },
				{ __call = function() end })
		end })
	end
	local env = setmetatable({ CivvisControlConfig = cfg, ExposedMembers = {}, UI = ui,
		Events = events(), LuaEvents = events() }, { __index = _G })
	env.include = function(script)
		h.includes[#h.includes + 1] = script
		if script == "PlayerChange" then
			env.OnOk = function() h.presses = h.presses + 1 end
		else
			env.OnClose = function() h.closes = h.closes + 1; h.hidden = true end
		end
	end
	env.Controls = { PopupAlphaIn = { IsHidden = function() return false end } }
	env.ContextPtr = {
		GetID = function() return name end, IsHidden = function() return h.hidden end,
		SetUpdate = function(_, f) h.tick = f end, SetShowHandler = function(_, f) h.show = f end,
	}
	env.Automation = { Log = function(line) h.logs[#h.logs + 1] = line end, GetTime = function() return h.now end }
	env.GameConfiguration = { IsHotseat = function() return opts.hotseat end }
	env.Game = { GetLocalPlayer = function() return h.localPlayer end, GetCurrentGameTurn = function() return 5 end }
	env.DB = { Query = function(sql)
		if sql:find("civvis.harness", 1, true) then return opts.beat and { { age = 1 } } or {} end
		return {}
	end }
	local chunk = assert(loadfile(here .. "/CivvisControlAutoClose.lua"))
	setfenv(chunk, env)
	chunk()
	h.env = env
	return h
end
local SCREENS = { RequireHarness = true, Hotseat = true, HotseatSeats = { 1, 2, 3, 4, 5 },
	HotseatHumanSeats = { 0 }, HarnessRun = "hs-test", OrdersDb = "/runs/hs-test/orders.sqlite",
	RunTag = "hs-test", CivvisDecides = true, Play = true, AnnouncementSeconds = 0.25 }
local function play(h, frames)
	for _ = 1, frames do
		h.now = h.now + 0.25
		if h.tick then h.tick(0.25) end
	end
end

for _, name in ipairs({ "TechCivicCompletedPopup", "PlayerChange", "DiplomacyActionView" }) do
	local sp = closer(name, SCREENS, { hotseat = false, localPlayer = 0 })
	assert(sp.includes[#sp.includes] == name or sp.includes[1] ~= nil, name .. ": the shipped screen loads")
	assert(sp.tick == nil, name .. ": no clock of ours in a single-player game")
	assert(sp.env.UI == sp.ui and sp.ui.ReferenceCurrentEvent == sp.reference, name .. ": UI is the shipped one")
	assert(sp.listeners == 0 and #sp.logs == 0, name .. ": no listener and no line")
end

-- A hotseat game with no harness: on a CIVVIS seat's turn nothing is closed or pressed.
for _, seat in ipairs({ 1, 0 }) do
	local popup = closer("TechCivicCompletedPopup", SCREENS, { hotseat = true, localPlayer = seat })
	play(popup, 400)
	assert(popup.closes == 0 and not popup.hidden, "no harness: the announcement is the person's")
	assert(#popup.logs == 0, "no harness: not one line")
	local handover = closer("PlayerChange", SCREENS, { hotseat = true, localPlayer = seat })
	play(handover, 400)
	assert(handover.presses == 0, "no harness: Start Turn waits for a person on every seat")
	assert(#handover.logs == 0)
end

-- Live: CIVVIS's own seats only.
for seat, mine in pairs({ [1] = true, [5] = true, [0] = false, [6] = false }) do
	local popup = closer("TechCivicCompletedPopup", SCREENS, { hotseat = true, beat = true, localPlayer = seat })
	play(popup, 40)
	assert((popup.closes > 0) == mine, "live: seat " .. seat .. (mine and " closes" or " does not close"))
	local handover = closer("PlayerChange", SCREENS, { hotseat = true, beat = true, localPlayer = seat })
	play(handover, 40)
	assert((handover.presses > 0) == mine, "live: seat " .. seat .. (mine and " presses" or " waits"))
end
-- The same seats with the key unset (`tools/civ6_hotseat.py` without the flag) are unchanged.
local operator = { Hotseat = true, HotseatSeats = { 1, 2, 3 }, HotseatHumanSeats = { 0 }, RunTag = "hs",
	CivvisDecides = true, Play = true, AnnouncementSeconds = 0.25 }
local legacy = closer("PlayerChange", operator, { hotseat = true, localPlayer = 1 })
play(legacy, 40)
assert(legacy.presses > 0, "without the key a jack seat's Start Turn is still pressed")
print("ok   screen closer")

-- 5. The HUD, whole. -------------------------------------------------------
local function hud(cfg, opts)
	local h = { logs = {}, ends = 0, pulses = 0, now = 1000, localPlayer = opts.localPlayer }
	local env = setmetatable({ CivvisControlConfig = cfg }, { __index = _G })
	env.include = function(name)
		if name ~= "TopPanel" then error("unavailable") end
		env.LateInitialize = function() end
		env.OnRefreshTimeTick = function() end
		h.stock, h.stockTick = true, env.OnRefreshTimeTick
	end
	env.ContextPtr = {
		SetUpdate = function(_, f) h.update = f end,
		LookUpControl = function() return { IsHidden = function() return true end } end,
	}
	env.LuaEvents = { CivvisControlPulse = function() h.pulses = h.pulses + 1 end, CivvisControlPeek = function() end }
	env.Events = { TeamVictory = { Add = function() end } }
	env.Automation = { Log = function(line) h.logs[#h.logs + 1] = line end, GetTime = function() return h.now end }
	env.ExposedMembers = { CivvisSeatBeat = { [1] = 5 } }   -- a stamp left by an earlier game
	env.Game = { GetLocalPlayer = function() return h.localPlayer end, GetCurrentGameTurn = function() return 3 end,
		GetMaxGameTurns = function() return 250 end }
	local configs = {
		[0] = { slot = 1, human = true, name = "Ada", handicap = 10 },
		[1] = { slot = 1, human = true, name = "CIVVIS jack 1", handicap = 10 },
		[2] = { slot = 2, human = false, name = "AI", handicap = 12 },
	}
	env.PlayerConfigurations = setmetatable({}, { __index = function(_, id)
		local c = configs[id]
		if c == nil then return nil end
		return {
			GetWantsPause = function() return false end, GetSlotStatus = function() return c.slot end,
			GetCivilizationLevelTypeID = function() return 7 end, IsHuman = function() return c.human end,
			GetPlayerName = function() return c.name end, GetHandicapTypeID = function() return c.handicap end,
			GetCivilizationTypeName = function() return "CIVILIZATION_X" .. id end,
			GetLeaderTypeName = function() return "LEADER_X" .. id end,
		}
	end })
	env.Players = setmetatable({}, { __index = function()
		return { IsTurnActive = function() return true end, IsAlive = function() return true end }
	end })
	env.UI = {
		RequestAction = function() h.ends = h.ends + 1 end, HasSentTurnComplete = function() return false end,
		CanEndTurn = function() return true end, IsGameCoreBusy = function() return false end,
		GetElapsedTime = function() return h.now end,
	}
	env.ActionTypes = { ACTION_ENDTURN = 1 }
	env.NotificationManager = { GetFirstEndTurnBlocking = function() return 0 end }
	env.EndTurnBlockingTypes = { NO_ENDTURN_BLOCKING = 0 }
	env.Network = { BroadcastPlayerInfo = function() end }
	env.GameConfiguration = {
		IsHotseat = function() return opts.hotseat end,
		GetParticipatingPlayerIDs = function() return { 0, 1, 2 } end,
		GetGameSpeedType = function() return "speed#" end, GetRuleSet = function() return "RULESET_EXPANSION_2" end,
	}
	env.MapConfiguration = { GetScript = function() return "Pangaea.lua" end, GetMapSize = function() return "size#" end }
	env.GameInfo = { Difficulties = { [10] = { DifficultyType = "DIFFICULTY_KING" }, [12] = { DifficultyType = "DIFFICULTY_DEITY" } },
		Maps = { ["size#"] = { MapSizeType = "MAPSIZE_TINY" } },
		GameSpeeds = { ["speed#"] = { GameSpeedType = "GAMESPEED_ONLINE" } } }
	env.SlotStatus = { SS_TAKEN = 1, SS_COMPUTER = 2 }
	env.CivilizationLevelTypes = { CIVILIZATION_LEVEL_FULL_CIV = 7 }
	env.DB = { Query = function(sql)
		if sql:find("civvis.harness", 1, true) then return h.beat and { { age = 1 } } or {} end
		return {}
	end }
	h.beat = opts.beat
	local chunk = assert(loadfile(here .. "/CivvisControlHeartbeat.lua"))
	setfenv(chunk, env)
	chunk()
	h.env = env
	return h
end
local function run(h, seconds)
	for _ = 1, seconds do
		h.now = h.now + 1
		h.update(1)
	end
end
local function lines(h, kind)
	local found = {}
	for _, line in ipairs(h.logs) do
		if line:find('"kind":"' .. kind .. '"', 1, true) then found[#found + 1] = line end
	end
	return found
end
local HUD = { RequireHarness = true, Hotseat = true, HotseatSeats = { 1, 2, 3, 4, 5 }, HotseatHumanSeats = { 0 },
	HarnessRun = "hs-test", OrdersDb = "/runs/hs-test/orders.sqlite", RunTag = "hs-test",
	CivvisDecides = true, Play = true }

local spHud = hud(HUD, { hotseat = false, localPlayer = 0 })
assert(spHud.stock, "the shipped HUD loads")
assert(spHud.update == nil, "no clock of ours in a single-player game")
assert(spHud.env.OnRefreshTimeTick == spHud.stockTick, "the shipped clock label is not wrapped")
assert(#spHud.logs == 0)

local idleHud = hud(HUD, { hotseat = true, localPlayer = 1 })
run(idleHud, 120)
assert(idleHud.ends == 0, "no harness: a quiet CIVVIS seat's turn is never ended for it")
assert(#idleHud.logs == 0, "no harness: not one line, no lobby")

local liveHud = hud(HUD, { hotseat = true, beat = true, localPlayer = 1 })
run(liveHud, 120)
local lobby = lines(liveHud, "hotseat_lobby")
assert(#lobby == 1, "the lobby is reported once a game")
assert(#lines(liveHud, "hotseat_lobby_flush") == 1, "and written out by the line behind it")
for _, want in ipairs({ '"run":"hs-test"', '"hotseat":true', '"map":"Pangaea.lua"', '"size":"MAPSIZE_TINY"',
		'"speed":"GAMESPEED_ONLINE"', '"ruleset":"RULESET_EXPANSION_2"', '"max_turns":250', '"local_player":1',
		'{"id":0,"slot":"human","human":true,', '{"id":1,"slot":"human","human":true,',
		'{"id":2,"slot":"computer","human":false,', '"difficulty":"DIFFICULTY_DEITY"' }) do
	assert(lobby[1]:find(want, 1, true), "the lobby reads " .. want .. ": " .. lobby[1])
end
local _, opened = lobby[1]:gsub("[{%[]", "")
local _, closed = lobby[1]:gsub("[}%]]", "")
assert(opened == closed, "the lobby line is balanced JSON")
assert(liveHud.ends >= 1, "live, the watch still ends a quiet CIVVIS turn")

local personHud = hud(HUD, { hotseat = true, beat = true, localPlayer = 0 })
run(personHud, 120)
assert(personHud.ends == 0, "live, the person's own turn is never ended")
print("ok   HUD")
print("all harness-gate checks passed")
