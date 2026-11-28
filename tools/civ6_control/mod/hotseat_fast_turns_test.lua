-- `HotseatFastTurns` (`tools/civ6_hotseat.py --fast-turns`, off unless asked)
-- and the UI clock on the records that time a hotseat seat's turn.
--
--   1. The covered hand-over is pressed on its first frame up, for a CIVVIS
--      seat and for the person's own; without the key, without the cover, or
--      with an explicit `HotseatPressSeconds`, the press waits as it did. A
--      retry still waits two seconds.
--   2. The hand-over's records carry `"ui"` where the game has the clock.
--   3. The Heartbeat offers the landed-orders peek every 20 ms with the key.
--   4. The agent's peek bound follows the same rule; its timing records carry
--      `ui` in a hotseat install only.
--
-- Run: lua5.1 tools/civ6_control/mod/hotseat_fast_turns_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."
local function read(name)
	local handle = assert(io.open(here .. "/" .. name, "r"))
	local text = handle:read("*a")
	handle:close()
	return text
end
local closer, agent = read("CivvisControlAutoClose.lua"), read("CivvisControlAgent.lua")

-- 1-2. The hand-over screen. ------------------------------------------------
local screenBlock = assert(closer:match("(CivvisHotseatScreen = { .-\n)if NAME == \"PlayerChange\" then"),
	"AutoClose defines CivvisHotseatScreen before the PlayerChange arm")

local function screenHost(cfg, clock)
	local h = { presses = 0, reports = {}, player = 1, hidden = false, paused = true }
	local env = {
		cfg = cfg, RUN = "fast-test",
		tonumber = tonumber, tostring = tostring, type = type, pcall = pcall,
		ipairs = ipairs, string = string,
		report = function(kind, fields) h.reports[#h.reports + 1] = { kind = kind, fields = fields or "" } end,
		Game = { GetLocalPlayer = function() return h.player end,
		         GetCurrentGameTurn = function() return 4 end },
		Controls = {
			PopupAlphaIn = { IsHidden = function() return h.hidden end,
			                 SetHide = function(_, hide) h.hidden = hide end },
			PlayerChangingText = { IsHidden = function() return true end, SetHide = function() end },
		},
		PlayerConfigurations = setmetatable({}, { __index = function()
			return { GetWantsPause = function() return h.paused end,
			         GetHotseatPassword = function() return "" end }
		end }),
		OnOk = function() h.presses = h.presses + 1 end,
		SaveLocations = { LOCAL_STORAGE = 7 },
		Network = { GetGameConfigurationSaveType = function() return 3 end, SaveGame = function() end },
		Events = { LocalPlayerTurnBegin = { Add = function() end },
		           LocalPlayerTurnEnd = { Add = function() end } },
		ContextPtr = { SetUpdate = function() end },
	}
	if clock ~= nil then env.UI = { GetElapsedTime = function() return clock end } end
	local chunk = assert(loadstring(screenBlock))
	setfenv(chunk, env)
	chunk()
	h.screen = env.CivvisHotseatScreen
	-- The hand-over shows: the dialog up, its pause standing.
	function h.show(player)
		h.player, h.hidden, h.paused = player or h.player, false, true
	end
	return h
end
local covered = function(extra)
	local cfg = { Hotseat = true, HotseatCover = true, HotseatHumanSeats = { 0 },
	              HotseatSeats = { 1, 2 }, HotseatAutoStart = true }
	for k, v in pairs(extra or {}) do cfg[k] = v end
	return cfg
end
local FRAME = 1 / 60

-- As shipped: a covered hand-over waits 0.3 s of frames.
local h = screenHost(covered())
h.show(1)
h.screen.frame(FRAME)
assert(h.presses == 0, "without the key the first frame does not press")
for _ = 1, 16 do h.screen.frame(FRAME) end
assert(h.presses == 0, "nor before 0.3 s")
for _ = 1, 3 do h.screen.frame(FRAME) end
assert(h.presses == 1, "pressed at 0.3 s, as shipped: " .. h.presses)

-- With the key, under the cover: the first frame presses, for a CIVVIS seat...
h = screenHost(covered({ HotseatFastTurns = true }))
h.show(1)
h.screen.frame(FRAME)
assert(h.presses == 1, "the key presses a covered CIVVIS hand-over on its first frame")
assert(h.reports[#h.reports].kind == "hotseat_start_turn")
-- ...still up (its pause stands): the retry waits its two seconds.
for _ = 1, 110 do h.screen.frame(FRAME) end
assert(h.presses == 1, "a retry is not sooner")
for _ = 1, 12 do h.screen.frame(FRAME) end
assert(h.presses == 2, "the retry comes at two seconds")
assert(h.reports[#h.reports].fields:find('"again":true', 1, true))
-- ...and for the person's own hand-over, which the cover lifts on.
h.show(0)
h.screen.frame(FRAME)
assert(h.presses == 3, "the person's covered hand-over is pressed on its first frame too")
-- The next seat in the same turn: its own first press, at once.
h.show(2)
h.screen.frame(FRAME)
assert(h.presses == 4)

-- The key without the cover: the hand-over shows to whoever watches, so it
-- keeps its slide-in.
h = screenHost(covered({ HotseatFastTurns = true, HotseatCover = false }))
h.show(1)
h.screen.frame(FRAME)
assert(h.presses == 0, "an uncovered hand-over keeps its 0.3 s")
for _ = 1, 20 do h.screen.frame(FRAME) end
assert(h.presses == 1)

-- An explicit `HotseatPressSeconds` wins over the key.
h = screenHost(covered({ HotseatFastTurns = true, HotseatPressSeconds = 0.5 }))
h.show(1)
for _ = 1, 25 do h.screen.frame(FRAME) end
assert(h.presses == 0, "a configured press time is honoured")
for _ = 1, 8 do h.screen.frame(FRAME) end
assert(h.presses == 1)

-- A seat that is not CIVVIS's, and a person's seat without auto start, are
-- never pressed, key or not.
h = screenHost(covered({ HotseatFastTurns = true, HotseatAutoStart = false }))
h.show(0)
for _ = 1, 30 do h.screen.frame(FRAME) end
h.show(5)
for _ = 1, 30 do h.screen.frame(FRAME) end
assert(h.presses == 0, "the key presses nobody else's hand-over")

-- 2. The clock on the hand-over's records.
h = screenHost(covered({ HotseatFastTurns = true }), 1409.93456)
h.show(1)
h.screen.frame(FRAME)
local pressed = h.reports[#h.reports]
assert(pressed.kind == "hotseat_start_turn" and pressed.fields:sub(-14) == ',"ui":1409.935',
	pressed.fields)
h.screen.announce("begin")
assert(h.reports[#h.reports].fields ==
	',"phase":"begin","player":1,"turn":4,"human":false,"civvis":true,"ui":1409.935',
	h.reports[#h.reports].fields)
-- No such clock (and nothing unreadable written in its place).
h = screenHost(covered())
h.screen.announce("end")
assert(not h.reports[#h.reports].fields:find('"ui"', 1, true), "no clock, no field")
h = screenHost(covered(), 0 / 0)
h.screen.announce("end")
assert(not h.reports[#h.reports].fields:find('"ui"', 1, true), "NaN is not a clock")
print("hand-over checks passed")

-- 3. The Heartbeat's peek pulse. ---------------------------------------------
local function peekHost(cfg)
	local p = { peeks = 0 }
	local env = setmetatable({ CivvisControlConfig = cfg }, { __index = _G })
	env.include = function(name)
		if name ~= "TopPanel" then error("unavailable expansion") end
		env.LateInitialize = function() end
	end
	env.ContextPtr = { SetUpdate = function(_, callback) p.update = callback end }
	env.LuaEvents = { CivvisControlPulse = function() end,
	                  CivvisControlPeek = function() p.peeks = p.peeks + 1 end }
	local chunk = assert(loadfile(here .. "/CivvisControlHeartbeat.lua"))
	setfenv(chunk, env); chunk()
	return p
end
local fast = peekHost({ CivvisDecides = true, HotseatFastTurns = true })
fast.update(0.015); assert(fast.peeks == 0)
fast.update(0.006); assert(fast.peeks == 1, "20 ms of frames offer a peek with the key")
for _ = 1, 10 do fast.update(0.0201) end
assert(fast.peeks == 11, "one peek per 20 ms: " .. fast.peeks)
local shipped = peekHost({ CivvisDecides = true })
for _ = 1, 2 do shipped.update(0.021) end
assert(shipped.peeks == 0, "without the key the peek keeps its 50 ms")
shipped.update(0.01); assert(shipped.peeks == 1)
local set = peekHost({ CivvisDecides = true, HotseatFastTurns = true, OrdersPeekSeconds = 0.1 })
for _ = 1, 4 do set.update(0.021) end
assert(set.peeks == 0, "an explicit OrdersPeekSeconds wins over the key")
set.update(0.021); assert(set.peeks == 1)
print("peek pulse checks passed")

-- 4. The agent. -------------------------------------------------------------
local function extract(pattern, env)
	local source = assert(agent:match(pattern), "the agent defines " .. pattern)
	local chunk = assert(loadstring(source))
	setfenv(chunk, env)
	chunk()
end
local function peekSeconds(cfg)
	local env = { cfg = cfg, tonumber = tonumber, CivvisQueue = {} }
	extract("(CivvisQueue%.peekSeconds = function%(%).-\nend;)", env)
	return env.CivvisQueue.peekSeconds()
end
assert(peekSeconds({}) == 0.05, "as shipped")
assert(peekSeconds({ HotseatFastTurns = true }) == 0.02)
assert(peekSeconds({ HotseatFastTurns = false }) == 0.05)
assert(peekSeconds({ HotseatFastTurns = true, OrdersPeekSeconds = 0.1 }) == 0.1)
assert(peekSeconds({ OrdersPeekSeconds = 0.02 }) == 0.02, "the ladder's poll-20ms arm unchanged")

-- `ordersLanded` asks the channel at most once per bound.
local function landedHost(cfg)
	local a = { clock = 100, queries = 0 }
	local env = {
		cfg = cfg, tonumber = tonumber, type = type,
		awaiting = { turn = 4, frame = 0, done = false },
		CivvisClock = { now = function() return a.clock end },
		CivvisQueue = {},
		ordersReady = function() a.queries = a.queries + 1; return nil end,
	}
	extract("(CivvisQueue%.peekSeconds = function%(%).-\nend;)", env)
	extract("(CivvisQueue%.ordersLanded = function%(%).-\nend;)", env)
	a.landed = env.CivvisQueue.ordersLanded
	return a
end
local function queriesAt(a, times)
	for _, t in ipairs(times) do a.clock = t; a.landed() end
	return a.queries
end
assert(queriesAt(landedHost({ CivvisDecides = true, HotseatFastTurns = true }),
	{ 100, 100.019, 100.021, 100.03, 100.042 }) == 3, "one query per 20 ms with the key")
assert(queriesAt(landedHost({ CivvisDecides = true }),
	{ 100, 100.021, 100.042, 100.049, 100.051 }) == 2, "one query per 50 ms without it")

-- The UI clock on the timing records: hotseat installs only.
local function stamp(cfg, raw)
	local env = { cfg = cfg, math = math, CivvisClock = { raw = function() return raw end } }
	extract("(CivvisClock%.stamp = function%(%).-\nend;)", env)
	return env.CivvisClock.stamp()
end
assert(stamp({ Hotseat = true }, 1409.93456) == 1409.935, "to the millisecond")
assert(stamp({}, 1409.93456) == nil, "a single-player install's records are unchanged")
assert(stamp({ Hotseat = true }, nil) == nil, "no clock, no field")
for _, site in ipairs({
	'emit("export_timing", {\n\t\tturn = turn, frame = frame, ms = sections, city_ms = cityMs, clock = CivvisExportClock.kind,\n\t\tui = CivvisClock.stamp(),',
	'movers = CivvisFrames.movers,\n\t\tui = CivvisClock.stamp(),',
	'emit("orders", {\n\t\tturn = turn, frame = awaiting.frame or 0, source = "civvis", ui = CivvisClock.stamp(),',
	'emit("await", { turn = turn, polls = awaiting.polls, ui = CivvisClock.stamp() });',
	'emit("orders_peek_wake", {\n\t\tturn = awaiting.turn, frame = awaiting.frame or 0, ui = CivvisClock.stamp(),',
}) do
	assert(agent:find(site, 1, true), "the record carries the UI clock: " .. site:sub(1, 40))
end
print("agent checks passed")
print("all fast-turns checks passed")
