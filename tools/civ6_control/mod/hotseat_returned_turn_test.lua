-- A hotseat turn the engine hands back (CivvisControlAutoClose.lua
-- `CivvisHotseatScreen.onBuild`, CivvisHotseatCover.lua `handedBack`).
--
-- The engine gives a seat its turn back by itself when a unit still has
-- movement once the walk it was on is finished at the turn complete
-- (live-6p-20261128T0819Zg turn 6, net_message_debug.log: `Player 0 Unit
-- 262146 has movement remaining after completing automation.  Attempting to
-- turn active player.`, `AppRequestTurnUnready: Player 0`). The same seat's
-- turn then begins again; PlayerChange.lua built its hand-over again
-- (BuildTurnControls :121-140, `SetPause(true)` :125), the hand-over was
-- pressed a second time (`again:true`, `via:"cover"`), and Civ VI died, in all
-- three games of 2026-11-28 that had one. Checked here:
--   1. an ordinary seat change and the World Congress's second round are no
--      hand-back; the shipped hand-over is built as before;
--   2. a hand-back (end and begin of the same seat and turn, no other begin
--      between, whichever of the two turn-begin handlers runs first): the
--      shipped hand-over is NOT built, nothing pauses, the "please wait"
--      screen is closed once by the shipped Start Turn (`OnOk`), the agent is
--      told (`ExposedMembers.CivvisTurnReturned`, counting hand-backs), and
--      the cover's backstop cannot press it (no second press);
--   3. the diagnostic reactions a crash check game can ask for
--      (`HotseatReturnedTurn`): `legacy` is the old hand-over, `press` keeps
--      the pause and presses it as a first press;
--   4. outside hotseat nothing is wrapped;
--   5. the cover lifts for a person's turn handed back, with no hand-over,
--      no camera move and no backstop press; a CIVVIS seat's stays covered.
--
-- Run: lua5.1 tools/civ6_control/mod/hotseat_returned_turn_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."
local function read(name)
	local handle = assert(io.open(here .. "/" .. name, "r"))
	local text = handle:read("*a")
	handle:close()
	return text
end
local closer = read("CivvisControlAutoClose.lua")
local screenBlock = assert(closer:match("(CivvisHotseatScreen = { .-\n)if NAME == \"PlayerChange\" then"),
	"AutoClose defines CivvisHotseatScreen before the PlayerChange arm")

local FRAME = 1 / 60
-- The PlayerChange context: the shipped script's globals (PlayerChange.lua),
-- as far as they matter here, and this file's block on top.
local function handover(cfg)
	local h = { player = 0, turn = 6, paused = false, active = true, hidden = true,
		built = 0, oks = 0, reports = {}, handlers = { begin = {}, ["end"] = {} } }
	local env
	env = {
		cfg = cfg, RUN = "returned-test",
		tonumber = tonumber, tostring = tostring, type = type, pcall = pcall,
		ipairs = ipairs, string = string,
		report = function(kind, fields) h.reports[#h.reports + 1] = { kind = kind, fields = fields or "" } end,
		Game = { GetLocalPlayer = function() return h.player end,
		         GetCurrentGameTurn = function() return h.turn end },
		Controls = {
			PopupAlphaIn = { IsHidden = function() return h.hidden end,
			                 SetHide = function(_, hide) h.hidden = hide end },
			PlayerChangingText = { IsHidden = function() return true end, SetHide = function() end },
		},
		PlayerConfigurations = setmetatable({}, { __index = function()
			return { GetWantsPause = function() return h.paused end,
			         GetHotseatPassword = function() return "" end }
		end }),
		Players = setmetatable({}, { __index = function()
			return { IsTurnActive = function() return h.active end }
		end }),
		ExposedMembers = {},
		-- PlayerChange.lua:54-64: unpause, close, dequeue, hide the dialog.
		OnOk = function() h.oks = h.oks + 1; h.paused, h.hidden = false, true end,
		-- :121-140: pause, then the dialog shows.
		BuildTurnControls = function() h.built = h.built + 1; h.paused, h.hidden = true, false end,
		SaveLocations = { LOCAL_STORAGE = 7 },
		Network = { GetGameConfigurationSaveType = function() return 3 end, SaveGame = function() end },
		Events = {
			LocalPlayerTurnBegin = { Add = function(fn) table.insert(h.handlers.begin, fn) end },
			LocalPlayerTurnEnd = { Add = function(fn) table.insert(h.handlers["end"], fn) end },
		},
		LuaEvents = { CivvisHotseatStartTurn = { Add = function(fn) h.backstop = fn end } },
		ContextPtr = { SetUpdate = function(_, fn) h.update = fn end },
		table = table,
	}
	local chunk = assert(loadstring(screenBlock))
	setfenv(chunk, env)
	chunk()
	h.env, h.screen = env, env.CivvisHotseatScreen
	h.screen.arm()
	-- The shipped OnLocalPlayerTurnBegin (:83-91) was registered first, at the
	-- shipped Initialize; it calls the global BuildTurnControls by name.
	local shippedBegin = function() env.BuildTurnControls() end
	table.insert(h.handlers.begin, 1, shippedBegin)
	function h.finish() h.active = false; for _, fn in ipairs(h.handlers["end"]) do fn() end end
	function h.begin(player, turn, oursFirst)
		h.player, h.turn, h.active = player, turn or h.turn, true
		local order = h.handlers.begin
		if oursFirst then order = { order[2], order[1] } end
		for _, fn in ipairs(order) do fn() end
	end
	function h.kinds(kind)
		local out = {}
		for _, r in ipairs(h.reports) do if r.kind == kind then out[#out + 1] = r end end
		return out
	end
	return h
end
local function covered(extra)
	local cfg = { Hotseat = true, HotseatCover = true, HotseatHumanSeats = { 0 }, HotseatStubSeats = { 0 },
	              HotseatSeats = { 1, 2 }, HotseatAutoStart = true, HotseatFastTurns = true }
	for k, v in pairs(extra or {}) do cfg[k] = v end
	return cfg
end

-- 1. Ordinary seat changes, and the World Congress's second round. -----------
do
	local h = handover(covered())
	assert(h.screen.wrapBuild ~= nil and h.env.BuildTurnControls ~= nil)
	h.begin(0, 6); h.update(FRAME)
	assert(h.built == 1 and h.oks == 1, "the person's own turn: the hand-over built and pressed")
	h.finish(); h.begin(1, 6); h.update(FRAME)
	assert(h.built == 2 and h.oks == 2, "the next seat's: built and pressed")
	h.finish(); h.begin(2, 6); h.update(FRAME)
	h.finish(); h.begin(0, 7); h.update(FRAME)
	assert(h.built == 4 and #h.kinds("hotseat_turn_returned") == 0, "a new turn is no hand-back")
	-- selfplay-duel-20261127T0654Z turn 61: end(0), begin(1), end(1), begin(0)
	-- on the same game turn, the congress's round: every seat's hand-over again.
	h.turn = 61
	h.finish(); h.begin(1, 61); h.update(FRAME)
	h.finish(); h.begin(0, 61); h.update(FRAME)
	assert(h.built == 6 and #h.kinds("hotseat_turn_returned") == 0, "another seat began between: no hand-back")
end

-- 2. A hand-back, as live-6p-20261128T0819Zg turn 6 had it. ------------------
for _, oursFirst in ipairs({ false, true }) do
	local h = handover(covered())
	h.begin(0, 6); h.update(FRAME)
	assert(h.oks == 1 and h.built == 1)
	h.finish()
	h.begin(0, 6, oursFirst)                       -- the same seat and turn, again
	assert(h.built == 1, "the shipped hand-over is not built again (handler order flipped: "
		.. tostring(oursFirst) .. ")")
	assert(not h.paused, "nothing pauses")
	assert(h.oks == 2, "the please-wait screen is closed once, the way Start Turn closes it")
	local r = h.kinds("hotseat_turn_returned")
	assert(#r == 1 and r[1].fields:find('"player":0,"turn":6,"returned":1,"reaction":"safe","closed":true', 1, true),
		r[1] and r[1].fields)
	assert(r[1].fields:find('"pressed_before":true', 1, true) and r[1].fields:find('"person":true', 1, true))
	local told = h.env.ExposedMembers.CivvisTurnReturned
	assert(told and told.player == 0 and told.turn == 6 and told.n == 1, "the agent is told")
	for _ = 1, 300 do h.update(FRAME) end
	assert(h.oks == 2 and #h.kinds("hotseat_start_turn") == 1, "no press of the hand-back, at any time")
	-- The cover's backstop on a hand-over that stands paused anyway: refused.
	h.paused = true
	h.backstop(0, 6)
	assert(h.oks == 2, "the backstop does not press a turn handed back")
	h.paused = false
	-- Handed back a second time in the same turn: counted.
	h.finish(); h.begin(0, 6)
	assert(h.built == 1 and h.oks == 3 and h.env.ExposedMembers.CivvisTurnReturned.n == 2)
	-- The next seat is ordinary again.
	h.finish(); h.begin(1, 6); h.update(FRAME)
	assert(h.built == 2 and h.oks == 4)
	h.paused = true
	h.backstop(1, 6)
	assert(h.oks == 5, "and its backstop is as it was")
end

-- 3. The diagnostic reactions a crash check game can ask for. ----------------
do
	-- `legacy`: the hand-over as it was built, its press a retry at 2 s.
	local h = handover(covered({ HotseatReturnedTurn = "legacy" }))
	h.begin(0, 6); h.update(FRAME)
	h.finish(); h.begin(0, 6)
	assert(h.built == 2 and h.paused and h.oks == 1, "legacy: built again, paused")
	assert(h.kinds("hotseat_turn_returned")[1].fields:find('"reaction":"legacy","closed":false', 1, true))
	for _ = 1, 100 do h.update(FRAME) end
	assert(h.oks == 1, "legacy: no press before the retry's 2 s")
	h.backstop(0, 6)
	assert(h.oks == 2 and h.kinds("hotseat_start_turn")[2].fields:find('"again":true,"person":true,"via":"cover"', 1, true),
		"legacy: the backstop's second press, as in the three crashes")
	-- `press`: the pause kept, pressed on its first frame as a first press.
	local p = handover(covered({ HotseatReturnedTurn = "press" }))
	p.begin(0, 6); p.update(FRAME)
	p.finish(); p.begin(0, 6)
	assert(p.built == 2 and p.paused and p.oks == 1)
	p.update(FRAME)
	assert(p.oks == 2 and not p.paused, "press: pressed on the first frame")
	p.hidden, p.paused = false, true            -- still up: a later retry waits its 2 s
	for _ = 1, 60 do p.update(FRAME) end
	assert(p.oks == 2, "press: one first press only")
end

-- 4. Outside hotseat nothing is wrapped. -------------------------------------
do
	local h = handover({ HotseatCover = true })
	h.begin(0, 6); h.finish(); h.begin(0, 6)
	assert(h.built == 2 and #h.kinds("hotseat_turn_returned") == 0, "no hotseat, no wrap")
end
print("hand-over checks passed")

-- 5. The cover. ---------------------------------------------------------------
local coverText = read("CivvisHotseatCover.lua")
local function cover(cfg)
	local w = { pid = 0, turn = 6, active = { [0] = true }, paused = {}, did = {}, queued = 0, looked = 0, started = 0 }
	local env = setmetatable({}, { __index = function(_, k) return rawget(_G, k) end })
	env.CivvisControlConfig = cfg
	env.include = function() end
	env.PopupPriority = { Utmost = 100 }
	env.Game = { GetLocalPlayer = function() return w.pid end, GetCurrentGameTurn = function() return w.turn end }
	env.Players = setmetatable({}, { __index = function(_, pid)
		return { IsTurnActive = function() return w.active[pid] == true end }
	end })
	env.PlayerConfigurations = setmetatable({}, { __index = function(_, pid)
		return { GetWantsPause = function() return w.paused[pid] == true end,
		         GetHotseatPassword = function() return "" end,
		         GetPlayerName = function() return "seat" .. pid end }
	end })
	env.UIManager = {
		QueuePopup = function() w.queued = w.queued + 1 end,
		DequeuePopup = function() w.queued = w.queued - 1 end,
		IsInPopupQueue = function() return false end,
	}
	env.UI = { GetElapsedTime = function() return 100 end,
		LookAtPlot = function() w.looked = w.looked + 1 end, SetMapZoom = function() end,
		GetMapLookAtWorldTarget = function() return 0, 0 end, GetPlotCoordFromWorld = function() return 3, 4 end,
		GetMapZoom = function() return 1 end, GetHeadSelectedUnit = function() return nil end,
		GetHeadSelectedCity = function() return nil end, GetInterfaceMode = function() return 1 end }
	env.InterfaceModeTypes = { SELECTION = 1 }
	env.Automation = { Log = function(line) w.did[#w.did + 1] = line end }
	env.Options = { GetAudioOption = function() return 100 end, SetAudioOption = function() end }
	local handlers = {}
	local function events(name)
		return { Add = function(fn) handlers[name] = fn end }
	end
	env.Events = setmetatable({}, { __index = function(_, name) return events(name) end })
	env.LuaEvents = setmetatable({}, { __index = function(_, name)
		if name == "CivvisHotseatStartTurn" then
			return setmetatable({}, { __call = function() w.started = w.started + 1 end })
		end
		return setmetatable({ Add = function(fn) handlers[name] = fn end }, { __call = function() end })
	end })
	env.ContextPtr = { SetUpdate = function(_, fn) handlers.update = fn end, SetHide = function() end,
		SetInputHandler = function() end, SetShutdown = function() end,
		LookUpControl = function() return nil end }
	env.Controls = setmetatable({}, { __index = function()
		return setmetatable({}, { __index = function() return function() end end })
	end })
	local chunk = assert(loadstring(coverText))
	setfenv(chunk, env)
	chunk()
	w.cover, w.handlers = env.CivvisHotseatCover, handlers
	return w
end
local function personCfg(extra)
	local cfg = { Hotseat = true, HotseatCover = true, HotseatHumanSeats = { 0 }, HotseatSeats = { 1, 2 },
	              HotseatAutoStart = true, RunTag = "returned-test" }
	for k, v in pairs(extra or {}) do cfg[k] = v end
	return cfg
end
local function lastCover(w)
	for i = #w.did, 1, -1 do
		if w.did[i]:find('"kind":"hotseat_cover"', 1, true) then return w.did[i] end
	end
end
do
	local w = cover(personCfg())
	-- The person's turn ends: the cover comes up.
	w.handlers.LocalPlayerTurnEnd()
	assert(w.cover.up, "up at the person's end of turn")
	-- Handed back: the same seat's turn begins again.
	w.handlers.LocalPlayerTurnBegin()
	assert(not w.cover.up and lastCover(w):find('"reason":"person_returned"', 1, true),
		"the cover lifts for their turn handed back")
	assert(w.looked == 0, "the camera is left where they had it")
	-- The please-wait screen closing (PlayerChange.lua:59) is no hand-over.
	w.handlers.PlayerChange_Close(0)
	assert(w.looked == 0 and not w.cover.up)
	-- No backstop press, whatever the pause reads.
	w.cover.up, w.paused[0] = true, true
	for _ = 1, 200 do w.handlers.update(FRAME) end
	assert(w.started == 0, "the backstop never presses a turn handed back")
	-- A CIVVIS seat's turn handed back stays covered.
	local c = cover(personCfg())
	c.pid = 1
	c.handlers.LocalPlayerTurnBegin()
	c.handlers.LocalPlayerTurnEnd()
	c.handlers.LocalPlayerTurnBegin()
	assert(c.cover.up, "a CIVVIS seat's turn handed back stays covered")
	-- An ordinary person turn: the hand-over, then the camera put back.
	local o = cover(personCfg())
	o.pid = 1
	o.handlers.LocalPlayerTurnBegin()
	o.handlers.LocalPlayerTurnEnd()
	o.pid = 0
	o.paused[0] = true
	o.handlers.LocalPlayerTurnBegin()
	assert(o.cover.up, "over the person's own hand-over")
	o.paused[0] = false
	o.handlers.PlayerChange_Close(0)
	assert(not o.cover.up and lastCover(o):find('"reason":"person_start"', 1, true))
	-- The old reaction, asked for by a crash check game: covered over the
	-- hand-over again, the backstop free to press.
	local l = cover(personCfg({ HotseatReturnedTurn = "legacy" }))
	l.handlers.LocalPlayerTurnEnd()
	l.paused[0] = true
	l.handlers.LocalPlayerTurnBegin()
	assert(l.cover.up, "legacy: up over the hand-over that came back")
	for _ = 1, 100 do l.handlers.update(FRAME) end
	assert(l.started == 1, "legacy: the backstop presses at 1.5 s")
end
print("cover checks passed")
print("all returned-turn checks passed")
