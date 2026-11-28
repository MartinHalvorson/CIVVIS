-- The hotseat cover (CivvisHotseatCover.lua): up from the person's end of turn
-- to the start of their next, over every CIVVIS seat's turn; lifted on their
-- own hand-over's close with their view put back; never a trap.
-- Run: lua5.1 tools/civ6_control/mod/hotseat_cover_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."
local function read(name)
	local handle = assert(io.open(here .. "/" .. name, "r"))
	local text = handle:read("*a")
	handle:close()
	return text
end
local source = read("CivvisHotseatCover.lua")
local layout = read("CivvisHotseatCover.xml")

-- The shipped order (Loaders/PopupPriorityLoader_Base.lua:11-15), offset.
local PRIORITY = { Utmost = 100, HotSeatBackground = 101, PausePanel = 102, PlayerChange = 103,
                   EndGameMenu = 104, InGameTopOptionsMenu = 105 }

-- A game: whose turn, what is paused, where the camera is, what is selected.
local function newWorld()
	return { pid = 0, turn = 1, active = {}, paused = {}, password = {}, capital = {},
	         camX = 3, camY = 4, zoom = 0.5, unit = nil, city = nil, mode = 1,
	         looks = {}, endGameQueued = false, menuQueued = false,
	         -- The person's own volume sliders (Options.lua:225-229), and the UI
	         -- clock (nil: the cover falls back on each frame's dt).
	         volumes = { ["Master Volume"] = 80, ["Music Volume"] = 60, ["SFX Volume"] = 70,
	                     ["Ambience Volume"] = 50, ["Speech Volume"] = 40 },
	         audioThrows = false, clock = nil }
end

-- Load the cover into a fresh context with `config`. Returns the cover table,
-- the world it reads, and everything it did.
local function load(config)
	local world = newWorld()
	local did = { queued = {}, dequeued = 0, logs = {}, menus = 0, nudges = {}, detail = nil,
	              texture = nil, audio = {}, bar = { hides = {} } }
	local handlers = {}
	local function event(name)
		return setmetatable({ Add = function(f) handlers[name] = f end },
			{ __call = function(_, ...) if name == "PlayerChange_OpenInGameOptionsMenu" then
				did.menus = did.menus + 1
			elseif name == "CivvisHotseatStartTurn" then
				local pid, turn = ...
				did.nudges[#did.nudges + 1] = pid .. ":" .. turn
			end end })
	end
	local endGameMenu, gameMenu = {}, {}
	local env = {
		CivvisControlConfig = config,
		include = function() end,
		tonumber = tonumber, tostring = tostring, type = type, pcall = pcall,
		ipairs = ipairs, pairs = pairs, string = string, table = table, math = math,
		setmetatable = setmetatable,
		Automation = { Log = function(line) did.logs[#did.logs + 1] = line end },
		PopupPriority = PRIORITY,
		UIManager = {
			QueuePopup = function(self, ctx, priority, params)
				did.queued[#did.queued + 1] = { ctx = ctx, priority = priority, params = params }
			end,
			DequeuePopup = function(self, ctx) did.dequeued = did.dequeued + 1 end,
			IsInPopupQueue = function(self, ctx)
				return (ctx == endGameMenu and world.endGameQueued)
					or (ctx == gameMenu and world.menuQueued)
			end,
		},
		Options = {
			GetAudioOption = function(section, name)
				assert(section == "Sound")
				return world.volumes[name]
			end,
			SetAudioOption = function(section, name, value, save)
				assert(section == "Sound")
				if world.audioThrows then error("no audio") end
				did.audio[#did.audio + 1] = { name = name, value = value, save = save }
				world.volumes[name] = value
			end,
		},
		Controls = {
			Detail = { SetText = function(self, text) did.detail = text end },
			Lines = { CalculateSize = function() end },
			Backdrop = { SetTexture = function(self, name) did.texture = name end },
			MenuButton = { RegisterCallback = function(self, kind, f) handlers.menuButton = f end },
			Wait = { SetHide = function(self, hide)
				did.bar.hidden = hide
				did.bar.hides[#did.bar.hides + 1] = hide
			end },
			WaitFill = { SetSizeX = function(self, x) did.bar.width = x end,
			             SetOffsetX = function(self, x) did.bar.offset = x end },
			WaitText = { SetText = function(self, text) did.bar.text = text end },
		},
		Mouse = { eLClick = 1 },
		KeyEvents = { KeyDown = 1, KeyUp = 2 },
		Keys = { VK_ESCAPE = 27, VK_RETURN = 13 },
		InterfaceModeTypes = { SELECTION = 1, CITY_RANGE_ATTACK = 7 },
		ExposedMembers = { CivvisSeatBeat = {} },
		Game = { GetLocalPlayer = function() return world.pid end,
		         GetCurrentGameTurn = function() return world.turn end },
		Players = setmetatable({}, { __index = function(_, pid)
			return {
				IsTurnActive = function() return world.active[pid] == true end,
				GetCities = function()
					return { GetCapitalCity = function() return world.capital[pid] end }
				end,
			}
		end }),
		PlayerConfigurations = setmetatable({}, { __index = function(_, pid)
			return {
				GetWantsPause = function() return world.paused[pid] == true end,
				GetHotseatPassword = function() return world.password[pid] or "" end,
				GetPlayerName = function() return "Player " .. (pid + 1) end,
			}
		end }),
		UI = {
			GetMapLookAtWorldTarget = function() return world.camX * 100, world.camY * 100 end,
			GetPlotCoordFromWorld = function(wx, wy) return wx / 100, wy / 100 end,
			GetMapZoom = function() return world.zoom end,
			LookAtPlot = function(x, y)
				world.camX, world.camY = x, y
				world.looks[#world.looks + 1] = x .. "," .. y
			end,
			SetMapZoom = function(zoom) world.zoom = zoom end,
			GetHeadSelectedUnit = function() return world.unit end,
			DeselectAllUnits = function() world.unit = nil end,
			GetHeadSelectedCity = function() return world.city end,
			DeselectAllCities = function() world.city = nil end,
			GetInterfaceMode = function() return world.mode end,
			SetInterfaceMode = function(mode) world.mode = mode end,
			GetElapsedTime = function() return world.clock end,
		},
		Events = {}, LuaEvents = {},
	}
	for _, name in ipairs({ "LocalPlayerTurnEnd", "LocalPlayerTurnBegin", "LoadScreenClose",
			"TeamVictory", "UserRequestClose", "PlayerDefeat" }) do
		env.Events[name] = event(name)
	end
	for _, name in ipairs({ "PlayerChange_Close", "EndGameMenu_Shown", "EndGameMenu_OneMoreTurn",
			"EndGameMenu_Closed",
			"PlayerChange_OpenInGameOptionsMenu", "CivvisHotseatStartTurn" }) do
		env.LuaEvents[name] = event(name)
	end
	env.ContextPtr = {
		LookUpControl = function(self, path)
			if path == "/InGame/EndGameMenu" then return endGameMenu end
			if path == "/InGame/TopOptionsMenu" then return gameMenu end
		end,
		SetShutdown = function(self, f) handlers.shutdown = f end,
		SetInputHandler = function(self, f) handlers.input = f end,
		SetUpdate = function(self, f) handlers.update = f end,
	}
	local chunk = assert(loadstring(source))
	setfenv(chunk, env)
	chunk()
	return env.CivvisHotseatCover, world, did, handlers, env
end

local function kinds(did, kind)
	local out = {}
	for _, line in ipairs(did.logs) do
		local k = line:match('"kind":"([%w_]+)"')
		if kind == nil or k == kind then out[#out + 1] = line end
	end
	return out
end
local function last(did, kind)
	local rows = kinds(did, kind)
	return rows[#rows] or ""
end
local function ticks(handlers, seconds, each)
	for _ = 1, seconds do
		if each then each() end
		handlers.update(1.0)
	end
end

local HOTSEAT = function(extra)
	local config = { Hotseat = true, RunTag = "hs", HotseatSeats = { 1, 2 },
	                 HotseatHumanSeats = { 0 }, HotseatCover = true, HotseatAutoStart = true,
	                 HotseatNames = { ["1"] = "CIVVIS jack 1", ["2"] = "CIVVIS jack 2" } }
	for k, v in pairs(extra or {}) do config[k] = v end
	return config
end

-- 0. The layout: an opaque, mouse-eating base under everything it draws. ----
do
	local first = layout:match("<Context[^>]*>%s*(<%a+[^>]*>)")
	assert(first and first:find('<Box ID="Shade"', 1, true), "the opaque Box is the first control")
	assert(first:find('Size="parent,parent"', 1, true) and first:find('ConsumeMouse="1"', 1, true))
	assert(first:find('Color="%d+,%d+,%d+,255"'), "the base is fully opaque: " .. first)
	assert(layout:find('String="CIVVIS is taking its turn"', 1, true))
	assert(layout:find('ID="MenuButton"', 1, true), "a mouse way to the game menu")
end

-- 1. Nothing outside a game a person plays. --------------------------------
for _, config in ipairs({
	{},                                                       -- single player
	{ RunTag = "solo", Play = true, CivvisDecides = true },   -- the live ladder
	{ Hotseat = true, HotseatSeats = { 0, 1 }, HotseatHumanSeats = {},
	  HotseatCover = false, HotseatAutoStart = false },       -- an unattended table
	{ Hotseat = true, HotseatSeats = { 1 }, HotseatHumanSeats = { 0 } },  -- an install from before
}) do
	local cover, world, did, handlers = load(config)
	assert(handlers.update == nil and handlers.input == nil and handlers.LocalPlayerTurnEnd == nil,
		"no handler is registered")
	assert(cover.raise("test") == false and #did.queued == 0, "nothing is ever covered")
	assert(last(did, "hotseat_cover_armed"):find('"cover":false', 1, true))
end

-- 2. The person's game, from turn 1. ---------------------------------------
do
	local cover, world, did, handlers = load(HOTSEAT())
	assert(did.texture == "LEADER_HOJO_BACKGROUND")
	assert(last(did, "hotseat_cover_armed"):find('"cover":true', 1, true))

	-- The game loads on the person's turn 1, its hand-over up and paused.
	world.pid, world.turn, world.active[0], world.paused[0] = 0, 1, true, true
	handlers.LoadScreenClose()
	assert(#did.queued == 1 and cover.up, "covered over their own hand-over")
	local q = did.queued[1]
	-- Over the shipped hotseat backdrop and every CIVVIS screen (High and
	-- below), UNDER the hand-over: it is pressed exactly as it ships.
	assert(q.priority > PRIORITY.HotSeatBackground and q.priority < PRIORITY.PlayerChange,
		"between the backdrop and the hand-over: " .. tostring(q.priority))
	assert(q.priority == PRIORITY.PausePanel, "the slot a hotseat game never fills")
	assert(q.params.AlwaysVisibleInQueue == true,
		"stays drawn under the hand-over and the open game menu")
	-- Their hand-over is pressed (CivvisControlAutoClose.lua): it closes, and the
	-- cover lifts. Turn 1: no view saved and no capital yet, so the camera stays.
	world.paused[0] = false
	handlers.PlayerChange_Close(0)
	assert(not cover.up and did.dequeued == 1, "lifted on their own Start Turn")
	assert(#world.looks == 0, "turn 1: the camera is where the game put it")
	assert(last(did, "hotseat_cover"):find('"reason":"person_start"', 1, true))

	-- They play, leave the camera at (10, 12) zoomed out, and end the turn.
	world.camX, world.camY, world.zoom = 10, 12, 0.75
	handlers.LocalPlayerTurnEnd()
	world.active[0] = false
	assert(cover.up and #did.queued == 2, "up the moment their turn ends")
	assert(did.detail == "Turn 1", did.detail)
	assert(cover.views[0].x == 10 and cover.views[0].y == 12 and cover.views[0].zoom == 0.75)
	-- Between turns for a while: still up.
	ticks(handlers, 60)
	assert(cover.up)

	-- Seat 1's turn begins: still the same cover (no second queue, so no gap),
	-- and up before its hand-over is pressed 0.3 s in.
	world.pid, world.active[1], world.paused[1] = 1, true, true
	handlers.LocalPlayerTurnBegin()
	assert(cover.up and #did.queued == 2 and did.dequeued == 1, "never down between seats")
	assert(did.detail == "CIVVIS jack 1, turn 1", did.detail)
	-- Its hand-over still paused 1.5 s in: the press is asked for, once a 1.5 s.
	handlers.update(1.0)
	assert(#did.nudges == 0)
	handlers.update(1.0)
	assert(#did.nudges == 1 and did.nudges[1] == "1:1", tostring(did.nudges[1]))
end

-- A CIVVIS seat's whole turn, its controller stamping a beat every pass.
do
	local cover, world, did, handlers, env = load(HOTSEAT())
	world.pid, world.active[0] = 0, true
	handlers.LocalPlayerTurnEnd()
	world.active[0] = false
	world.pid, world.active[1] = 1, true
	handlers.LocalPlayerTurnBegin()
	local beat = 0
	ticks(handlers, 200, function()
		beat = beat + 1
		env.ExposedMembers.CivvisSeatBeat[1] = beat
	end)
	assert(cover.up, "a seat that keeps playing is not uncovered (short of the long clock, 240 s)")
	-- The seat moves the camera, selects its own unit, leaves a targeting mode.
	world.camX, world.camY = 40, 41
	world.unit = { GetOwner = function() return 1 end }
	world.city = { GetOwner = function() return 1 end }
	world.mode = 7
	-- It ends its turn; seat 2 plays; still covered throughout.
	handlers.LocalPlayerTurnEnd()
	world.active[1] = false
	assert(cover.up and #did.queued == 1)
	world.pid, world.active[2] = 2, true
	handlers.LocalPlayerTurnBegin()
	assert(cover.up and did.detail == "CIVVIS jack 2, turn 1", did.detail)
	assert(not did.detail:find("jack 1", 1, true))
	handlers.LocalPlayerTurnEnd()
	world.active[2] = false
	-- Turn 2, the person's: covered over their hand-over until it is pressed.
	world.turn, world.pid, world.active[0], world.paused[0] = 2, 0, true, true
	handlers.LocalPlayerTurnBegin()
	assert(cover.up and #did.queued == 1)
	ticks(handlers, 2)
	assert(cover.up and did.nudges[#did.nudges] == "0:2", "their press is asked for too")
	world.paused[0] = false
	handlers.PlayerChange_Close(0)
	assert(not cover.up, "lifted")
	-- Their view, as they left it: camera and zoom back, the seat's unit and
	-- city deselected, plain selection mode.
	assert(world.camX == 3 and world.camY == 4, world.camX .. "," .. world.camY)
	assert(world.unit == nil and world.city == nil and world.mode == 1)
	assert(last(did, "hotseat_view"):find('"camera":"saved"', 1, true))
	-- The lift is said, and flushed: the game writes a record only when the next is logged.
	local lines = kinds(did)
	assert(lines[#lines]:find('"kind":"cover_flush"', 1, true))
end

-- 3. The person's own selection is theirs; no saved view falls back to the capital.
do
	local cover, world, did, handlers = load(HOTSEAT())
	world.pid, world.turn, world.active[0], world.paused[0] = 0, 30, true, true
	world.capital[0] = { GetX = function() return 5 end, GetY = function() return 6 end }
	world.unit = { GetOwner = function() return 0 end }
	handlers.LoadScreenClose()      -- a loaded save: no view saved in this session
	handlers.PlayerChange_Close(0)
	assert(world.camX == 5 and world.camY == 6, "their capital")
	assert(world.unit ~= nil, "their own unit stays selected")
	assert(last(did, "hotseat_view"):find('"camera":"capital"', 1, true))
	-- Another seat's hand-over closing lifts nothing.
	handlers.LocalPlayerTurnEnd()
	assert(cover.up)
	handlers.PlayerChange_Close(1)
	assert(cover.up, "a CIVVIS seat's Start Turn never uncovers")
end

-- 4. Every key and click is the cover's; Esc opens the game menu. ------------
do
	local cover, world, did, handlers = load(HOTSEAT())
	assert(handlers.input(1, 27) == true and did.menus == 0, "Esc down: consumed, nothing yet")
	assert(handlers.input(2, 27) == true and did.menus == 1, "Esc up: the game menu")
	assert(handlers.input(2, 13) == true and did.menus == 1, "Enter does not end anyone's turn")
	assert(handlers.input(512, 0) == true, "the mouse is consumed too")
	handlers.menuButton()
	assert(did.menus == 2, "the menu button")
end

-- 5. A seat that stops playing is not hidden for good. ---------------------
do
	local cover, world, did, handlers, env = load(HOTSEAT())
	handlers.LocalPlayerTurnEnd()
	world.pid, world.active[1] = 1, true
	handlers.LocalPlayerTurnBegin()
	env.ExposedMembers.CivvisSeatBeat[1] = 7
	ticks(handlers, 89)
	assert(cover.up, "89 quiet seconds: still covered")
	ticks(handlers, 2)
	assert(not cover.up, "90 quiet seconds: the cover stands aside")
	assert(last(did, "hotseat_cover"):find('"reason":"yield_quiet"', 1, true))
	-- That turn stays uncovered: its own events do not raise it again...
	assert(cover.raise("civvis_turn") == false and not cover.up)
	handlers.LocalPlayerTurnBegin()
	assert(not cover.up)
	-- ...until it ends.
	handlers.LocalPlayerTurnEnd()
	assert(cover.up, "the stalled turn ended: covered again")
	-- A paused seat counts as quiet too (the seat watch owns the pause first).
	world.active[1], world.turn = true, 2
	handlers.LocalPlayerTurnBegin()
	world.paused[1] = true
	ticks(handlers, 91)
	assert(not cover.up and last(did, "hotseat_cover"):find('"reason":"yield_quiet"', 1, true))
end
-- A turn that runs on although its controller beats (a controller past its
-- forfeit ladder still stamps every pass): not hidden for good either.
do
	local cover, world, did, handlers, env = load(HOTSEAT())
	handlers.LocalPlayerTurnEnd()
	world.pid, world.active[1] = 1, true
	handlers.LocalPlayerTurnBegin()
	local beat = 0
	local function stamp()
		beat = beat + 1
		env.ExposedMembers.CivvisSeatBeat[world.pid] = beat
	end
	ticks(handlers, 239, stamp)
	assert(cover.up, "239 s of a beating turn: still covered")
	ticks(handlers, 1, stamp)
	assert(not cover.up and last(did, "hotseat_cover"):find('"reason":"yield_long"', 1, true),
		"240 s: the cover stands aside")
	assert(last(did, "hotseat_cover"):find('"seat_turn":240', 1, true), last(did, "hotseat_cover"))
	-- That turn stays uncovered; the next seat's turn is covered and counted afresh.
	handlers.LocalPlayerTurnBegin()
	assert(not cover.up)
	world.active[1] = false
	handlers.LocalPlayerTurnEnd()
	assert(cover.up)
	world.pid, world.active[2] = 2, true
	handlers.LocalPlayerTurnBegin()
	ticks(handlers, 200, stamp)
	assert(cover.up and cover.turnFor == 200, "a new turn, a new clock: " .. cover.turnFor)
	-- The person's own turn is not on this clock (it has its own, 10 s paused).
	world.active[2] = false
	handlers.LocalPlayerTurnEnd()
	world.pid, world.turn, world.active[0], world.paused[0] = 0, 2, true, false
	handlers.PlayerChange_Close(0)
	world.active[0] = false
	handlers.LocalPlayerTurnEnd()
	assert(cover.up)
	-- A config may set its own length.
	local cover2, world2, did2, handlers2, env2 = load(HOTSEAT({ HotseatCoverLongSeconds = 30 }))
	handlers2.LocalPlayerTurnEnd()
	world2.pid, world2.active[1] = 1, true
	handlers2.LocalPlayerTurnBegin()
	ticks(handlers2, 31, function() beat = beat + 1; env2.ExposedMembers.CivvisSeatBeat[1] = beat end)
	assert(not cover2.up and last(did2, "hotseat_cover"):find('"reason":"yield_long"', 1, true))
end
do
	local cover, world, did, handlers = load(HOTSEAT())
	handlers.LocalPlayerTurnEnd()          -- the person ended; no seat begins
	ticks(handlers, 179)
	assert(cover.up)
	ticks(handlers, 2)
	assert(not cover.up and last(did, "hotseat_cover"):find('"reason":"yield_between"', 1, true))
end
do
	-- The person's turn whose hand-over is never pressed: theirs after 10 s.
	local cover, world, did, handlers = load(HOTSEAT())
	handlers.LocalPlayerTurnEnd()
	world.turn, world.active[0], world.paused[0] = 2, true, true
	handlers.LocalPlayerTurnBegin()
	ticks(handlers, 11)
	assert(not cover.up and last(did, "hotseat_cover"):find('"reason":"person_wait"', 1, true))
	-- And the person's turn running unpaused is theirs at once, however it began.
	local cover2, world2, did2, handlers2 = load(HOTSEAT())
	handlers2.LocalPlayerTurnEnd()
	world2.active[0] = true
	handlers2.update(0.1)
	assert(not cover2.up and last(did2, "hotseat_cover"):find('"reason":"person_turn"', 1, true))
end

-- 6. The end of the game, and a quit, are never covered. ---------------------
do
	local cover, world, did, handlers = load(HOTSEAT())
	handlers.LocalPlayerTurnEnd()
	world.endGameQueued = true
	handlers.update(0.1)
	assert(not cover.up and last(did, "hotseat_cover"):find('"reason":"end_game"', 1, true))
	assert(cover.raise("civvis_turn") == false, "not over the end screen")
	world.endGameQueued = false
	handlers.LocalPlayerTurnEnd()
	assert(cover.up)
	handlers.TeamVictory(1, 2, 3)
	assert(not cover.up)
	handlers.LocalPlayerTurnEnd()
	assert(not cover.up, "the game is won: nothing to cover")
	handlers.EndGameMenu_OneMoreTurn()
	handlers.LocalPlayerTurnEnd()
	assert(cover.up, "one more turn: covered again")
	handlers.EndGameMenu_Shown()
	handlers.update(0.1)
	assert(not cover.up, "an end screen whose defeat is not known: lifted, as it ships")
	handlers.LocalPlayerTurnEnd()
	handlers.UserRequestClose()
	assert(not cover.up and last(did, "hotseat_cover"):find('"reason":"quit_request"', 1, true),
		"a quit's confirmation is on screen")
end

-- A loader without the PausePanel slot still lands over the backdrop.
do
	PRIORITY.PausePanel = nil
	local cover, world, did, handlers = load(HOTSEAT())
	handlers.LocalPlayerTurnEnd()
	assert(did.queued[1].priority == PRIORITY.HotSeatBackground + 1)
	PRIORITY.PausePanel = 102
end

-- 7. The old ways, kept: --keep-handover, --no-cover, a hotseat password. ----
do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatAutoStart = false }))
	handlers.LocalPlayerTurnEnd()
	assert(cover.up, "the cover alone")
	world.turn, world.active[0], world.paused[0] = 2, true, true
	handlers.LocalPlayerTurnBegin()
	assert(not cover.up, "their own hand-over is on screen the moment their turn begins")
	assert(#did.nudges == 0)
	-- A load on their turn leaves their hand-over to them.
	handlers.LoadScreenClose()
	assert(not cover.up)
end
do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatCover = false }))
	world.camX, world.camY = 8, 9
	handlers.LocalPlayerTurnEnd()
	assert(not cover.up and #did.queued == 0, "--no-cover: the turns are on screen")
	world.camX, world.camY = 30, 30
	world.active[0], world.turn = true, 2
	handlers.PlayerChange_Close(0)
	assert(world.camX == 8 and world.camY == 9, "the view is still put back")
end
do
	local cover, world, did, handlers = load(HOTSEAT())
	world.password[0] = "secret"
	handlers.LocalPlayerTurnEnd()
	world.turn, world.active[0], world.paused[0] = 2, true, true
	handlers.LocalPlayerTurnBegin()
	assert(not cover.up, "a password seat types it on its own hand-over")
end

-- 8. Quiet while CIVVIS plays (`HotseatCoverSound`). --------------------------
local QUIETED = { "SFX Volume", "Ambience Volume", "Speech Volume" }
local function volumes(world)
	local v = world.volumes
	return string.format("%d/%d/%d/%d/%d", v["Master Volume"], v["Music Volume"], v["SFX Volume"],
		v["Ambience Volume"], v["Speech Volume"])
end
local OWN = "80/60/70/50/40"
do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatCoverSound = "music" }))
	assert(last(did, "hotseat_cover_armed"):find('"sound":"music"', 1, true))
	-- Their turn 1 is theirs: the cover over their own hand-over mutes nothing.
	world.pid, world.turn, world.active[0], world.paused[0] = 0, 1, true, true
	handlers.LoadScreenClose()
	assert(cover.up and #did.audio == 0 and volumes(world) == OWN, "their own turn is heard")
	ticks(handlers, 2)
	assert(#did.audio == 0)
	world.paused[0] = false
	handlers.PlayerChange_Close(0)
	-- They end it. The local player is still their seat until the next seat's
	-- turn begins: quiet from this moment all the same.
	world.active[0] = false
	handlers.LocalPlayerTurnEnd()
	assert(cover.up and volumes(world) == "80/60/0/0/0",
		"units, fights, chimes and voices are silent; the soundtrack plays on: " .. volumes(world))
	assert(#did.audio == 3)
	for i, set in ipairs(did.audio) do
		assert(set.name == QUIETED[i] and set.value == 0, "one call per bus")
		assert(set.save == 0, "this session only: the .ini is never written (Options.lua:225)")
	end
	assert(last(did, "hotseat_cover_sound"):find('"muted":true', 1, true)
		and last(did, "hotseat_cover_sound"):find('"was":70,"now":0', 1, true))
	-- Through the CIVVIS seats' turns: nothing more is touched.
	world.pid, world.active[1] = 1, true
	handlers.LocalPlayerTurnBegin()
	ticks(handlers, 5)
	world.active[1] = false
	handlers.LocalPlayerTurnEnd()
	world.pid, world.active[2] = 2, true
	handlers.LocalPlayerTurnBegin()
	ticks(handlers, 5)
	assert(#did.audio == 3 and volumes(world) == "80/60/0/0/0", "muted once, held")
	world.active[2] = false
	handlers.LocalPlayerTurnEnd()
	-- Their turn begins: their volumes are back before the hand-over is pressed.
	world.pid, world.turn, world.active[0], world.paused[0] = 0, 2, true, true
	handlers.LocalPlayerTurnBegin()
	assert(cover.up, "the cover still stands over their hand-over")
	assert(volumes(world) == OWN, "their turn's own sounds are theirs: " .. volumes(world))
	assert(#did.audio == 6 and did.audio[6].save == 0)
	ticks(handlers, 2)
	assert(#did.audio == 6, "and stay theirs while the hand-over waits")
	world.paused[0] = false
	handlers.PlayerChange_Close(0)
	assert(not cover.up and #did.audio == 6 and volumes(world) == OWN)
	assert(cover.muted == nil)
end
-- A bus the person keeps at 0 is theirs: never set, never "restored".
do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatCoverSound = "music" }))
	world.volumes["Speech Volume"] = 0
	handlers.LocalPlayerTurnEnd()
	assert(#did.audio == 2 and volumes(world) == "80/60/0/0/0")
	cover.lift("test")
	assert(#did.audio == 4 and volumes(world) == "80/60/70/50/0", volumes(world))
	-- Every bus at 0 already: nothing to do, and nothing logged.
	world.volumes["SFX Volume"], world.volumes["Ambience Volume"] = 0, 0
	local before = #kinds(did, "hotseat_cover_sound")
	handlers.LocalPlayerTurnEnd()
	ticks(handlers, 3)
	assert(#did.audio == 4 and cover.muted == nil)
	assert(#kinds(did, "hotseat_cover_sound") == before)
end
-- `silent`: the master bus alone. `all`, or an install from before the key: nothing.
do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatCoverSound = "silent" }))
	handlers.LocalPlayerTurnEnd()
	assert(#did.audio == 1 and did.audio[1].name == "Master Volume" and did.audio[1].save == 0)
	assert(volumes(world) == "0/60/70/50/40")
	cover.lift("test")
	assert(volumes(world) == OWN)
end
for _, mode in ipairs({ "all", "", false }) do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatCoverSound = mode or nil }))
	handlers.LocalPlayerTurnEnd()
	world.pid, world.active[1] = 1, true
	handlers.LocalPlayerTurnBegin()
	ticks(handlers, 3)
	assert(cover.up and #did.audio == 0 and #kinds(did, "hotseat_cover_sound") == 0,
		"the sound is left alone: " .. tostring(mode))
end
-- The turn events decide, not `IsTurnActive` at the instant they fire: still
-- "active" as their turn ends is quiet, not yet "active" as it begins is heard.
do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatCoverSound = "music" }))
	world.pid, world.active[0] = 0, true
	handlers.LocalPlayerTurnEnd()
	assert(volumes(world) == "80/60/0/0/0", "their turn has ended, whatever the flag says")
	world.pid, world.active[0], world.active[1] = 1, false, true
	handlers.LocalPlayerTurnBegin()
	world.active[1] = false
	handlers.LocalPlayerTurnEnd()
	world.pid, world.turn, world.paused[0] = 0, 2, true
	handlers.LocalPlayerTurnBegin()
	assert(cover.up and volumes(world) == OWN, "their turn has begun, whatever the flag says")
	ticks(handlers, 2)
	assert(cover.up and volumes(world) == OWN and #did.audio == 6, "and no flicker while it waits")
end
-- A game loaded on a CIVVIS seat's turn is quiet without any turn event.
do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatCoverSound = "music" }))
	world.pid, world.turn, world.active[1] = 1, 9, true
	handlers.LoadScreenClose()
	assert(cover.up and volumes(world) == "80/60/0/0/0")
end
-- The game menu over the cover: the person's volumes while it is open (its
-- Options screen reads them and can save them), quiet again once it closes,
-- and a volume changed in it is the one that comes back.
do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatCoverSound = "music" }))
	handlers.LocalPlayerTurnEnd()
	world.pid, world.active[1] = 1, true
	handlers.LocalPlayerTurnBegin()
	assert(volumes(world) == "80/60/0/0/0")
	handlers.input(2, 27)
	assert(did.menus == 1 and volumes(world) == OWN, "theirs before the menu is drawn")
	world.menuQueued = true
	ticks(handlers, 4)
	assert(volumes(world) == OWN and cover.muted == nil, "and for as long as it is open")
	world.volumes["SFX Volume"] = 30     -- changed and confirmed in Options
	world.menuQueued = false
	ticks(handlers, 1)
	assert(volumes(world) == "80/60/0/0/0", "quiet again within a second of its closing")
	world.active[1] = false
	handlers.LocalPlayerTurnEnd()
	world.pid, world.turn, world.active[0], world.paused[0] = 0, 2, true, true
	handlers.LocalPlayerTurnBegin()
	assert(volumes(world) == "80/60/30/50/40", "their new volume, not the old one: " .. volumes(world))
end
-- Every way the cover comes down puts the volumes back.
for _, way in ipairs({ "yield_quiet", "end_game", "quit_request", "victory", "shutdown", "no_harness" }) do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatCoverSound = "music" }))
	handlers.LocalPlayerTurnEnd()
	world.pid, world.active[1] = 1, true
	handlers.LocalPlayerTurnBegin()
	assert(volumes(world) == "80/60/0/0/0")
	if way == "yield_quiet" then
		ticks(handlers, 91)
		assert(last(did, "hotseat_cover"):find('"reason":"yield_quiet"', 1, true))
	elseif way == "end_game" then
		world.endGameQueued = true
		ticks(handlers, 1)
	elseif way == "quit_request" then
		handlers.UserRequestClose()
	elseif way == "victory" then
		handlers.TeamVictory()
	elseif way == "shutdown" then
		assert(handlers.shutdown, "a shutdown handler is registered (PausePanel.lua:75)")
		handlers.shutdown()
	else
		cover.lift(way)
	end
	assert(volumes(world) == OWN, way .. ": " .. volumes(world))
	assert(cover.muted == nil)
end
-- A game whose audio calls fail, or has none: the cover is still a cover.
do
	local cover, world, did, handlers, env = load(HOTSEAT({ HotseatCoverSound = "music" }))
	world.audioThrows = true
	handlers.LocalPlayerTurnEnd()
	assert(cover.up and cover.muted == nil and volumes(world) == OWN)
	env.Options = nil
	ticks(handlers, 2)
	cover.lift("test")
	assert(not cover.up)
end
-- A bus that will not take its volume back is tried again.
do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatCoverSound = "silent" }))
	handlers.LocalPlayerTurnEnd()
	world.audioThrows = true
	cover.lift("test")
	assert(cover.muted ~= nil and volumes(world) == "0/60/70/50/40")
	world.audioThrows = false
	assert(cover.unmute("again") == true and volumes(world) == OWN and cover.muted == nil)
end

-- 8b. A CIVVIS seat's defeat shown on a CIVVIS seat's turn (EndGameMenu.lua:1012-1013):
-- the screen is over the cover and is the person's to read and press; the
-- turn under it stays covered through Next Player and after.
do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatCoverSound = "music",
		HotseatSeats = { 1, 2, 3 } }))
	handlers.LocalPlayerTurnEnd()
	world.pid, world.active[1] = 1, true
	handlers.LocalPlayerTurnBegin()
	assert(cover.up and volumes(world) == "80/60/0/0/0")
	-- Seat 1 takes seat 2's last city: the defeat event, then the screen.
	world.endGameQueued = true
	handlers.EndGameMenu_Shown()            -- raised from inside the defeat's own handler
	handlers.PlayerDefeat(2, 1, 77)
	handlers.update(0.1)
	assert(cover.up and did.dequeued == 0, "still covered under the end screen")
	assert(last(did, "hotseat_cover_kept"):find('"defeated":2', 1, true))
	assert(volumes(world) == OWN, "the end screen is the person's to hear: " .. volumes(world))
	-- Nobody presses Next Player for minutes: nothing is hidden, so no stand-aside.
	ticks(handlers, 300)
	assert(cover.up and #kinds(did, "hotseat_cover_kept") == 1, "said once")
	assert(not last(did, "hotseat_cover"):find('"yield_', 1, true))
	-- Next Player (:304-318): the screen closes and seat 1's turn goes on, covered.
	world.endGameQueued = false
	handlers.EndGameMenu_Closed()
	ticks(handlers, 1)
	assert(cover.up and did.dequeued == 0, "seat 1's map is never on screen")
	assert(volumes(world) == "80/60/0/0/0", "quiet again: " .. volumes(world))
	world.active[1] = false
	handlers.LocalPlayerTurnEnd()
	world.pid, world.active[3] = 3, true
	handlers.LocalPlayerTurnBegin()
	assert(cover.up and did.dequeued == 0 and #did.queued == 1)
	-- A screen that leaves the queue without saying so is closed all the same.
	world.endGameQueued = true
	handlers.PlayerDefeat(2, 1, 78)
	handlers.update(0.1)
	assert(cover.up and cover.kept)
	world.endGameQueued = false
	handlers.update(0.1)
	assert(cover.up and not cover.kept and cover.defeated == nil)
	ticks(handlers, 91)
	assert(not cover.up and last(did, "hotseat_cover"):find('"reason":"yield_quiet"', 1, true),
		"and the stall clock runs again")
	-- Shown and kept, then gone from the queue with no closing word: the same.
	world.active[3], world.turn = false, 2
	handlers.LocalPlayerTurnEnd()
	world.pid, world.active[1] = 1, true
	handlers.LocalPlayerTurnBegin()
	assert(cover.up)
	world.endGameQueued = true
	handlers.EndGameMenu_Shown()
	handlers.PlayerDefeat(3, 1, 79)
	handlers.update(0.1)
	assert(cover.up and cover.kept)
	world.endGameQueued = false
	handlers.update(0.1)
	assert(cover.up and not cover.kept and not cover.endShown)
	ticks(handlers, 91)
	assert(not cover.up, "the stall clock runs again")
end
-- An end screen on a CIVVIS turn whose defeat is not known: lifted, as it
-- ships; and once it has gone (even unannounced) the cover comes back.
do
	local cover, world, did, handlers = load(HOTSEAT())
	handlers.LocalPlayerTurnEnd()
	world.pid, world.active[1] = 1, true
	handlers.LocalPlayerTurnBegin()
	world.endGameQueued = true
	handlers.EndGameMenu_Shown()
	handlers.update(0.1)
	assert(not cover.up and last(did, "hotseat_cover"):find('"reason":"end_game"', 1, true))
	world.endGameQueued = false
	world.active[1] = false
	handlers.LocalPlayerTurnEnd()
	ticks(handlers, 2)
	assert(cover.up, "covered again at the turn's end")
end
-- The person's own defeat, a defeat shown on the person's turn, and a win:
-- lifted as before.
do
	local cover, world, did, handlers = load(HOTSEAT())
	handlers.LocalPlayerTurnEnd()
	world.pid, world.active[1] = 1, true
	handlers.LocalPlayerTurnBegin()
	handlers.PlayerDefeat(0, 1, 5)
	world.endGameQueued = true
	handlers.EndGameMenu_Shown()
	handlers.update(0.1)
	assert(not cover.up and last(did, "hotseat_cover"):find('"reason":"end_game"', 1, true),
		"the person's defeat is theirs to see")
	world.endGameQueued = false
	handlers.EndGameMenu_Closed()
	handlers.LocalPlayerTurnEnd()
	assert(cover.up)
	-- Their own hand-over is up (the cover over it) and a CIVVIS seat is defeated.
	world.pid, world.turn, world.active[0], world.paused[0] = 0, 2, true, true
	handlers.LocalPlayerTurnBegin()
	assert(cover.up)
	handlers.PlayerDefeat(1, 1, 6)
	world.endGameQueued = true
	handlers.update(0.1)
	assert(not cover.up, "on the person's turn the map behind it is theirs")
	world.endGameQueued = false
	handlers.EndGameMenu_Closed()
	-- A CIVVIS seat wins: the game is over, the cover goes.
	world.active[0], world.paused[0] = false, false
	handlers.LocalPlayerTurnEnd()
	world.pid, world.active[1] = 1, true
	handlers.LocalPlayerTurnBegin()
	handlers.PlayerDefeat(2, 1, 7)
	handlers.TeamVictory(1, 2, 3)
	assert(not cover.up and cover.over)
	world.endGameQueued = true
	handlers.EndGameMenu_Shown()
	handlers.update(0.1)
	assert(not cover.up)
end

-- 9. How long (`HotseatCoverTimer`). ------------------------------------------
do
	local track = layout:match('<Box ID="WaitTrack"[^>]*>')
	local cover = load(HOTSEAT())
	assert(track and track:find('Size="' .. cover.BAR .. ',', 1, true),
		"the script's bar width is the layout's track: " .. tostring(track))
	assert(layout:find('<Container ID="Wait"[^>]*Hidden="1"'), "hidden until a wait is on")
	assert(layout:find('ID="WaitFill"', 1, true) and layout:find('ID="WaitText"', 1, true))

	-- About how long one of these takes: nothing without any on record; a mean,
	-- the newer weighted more; nothing counted at more than twice the median.
	local near = function(a, b) return math.abs(a - b) < 1e-9 end
	assert(cover.typical(nil) == nil and cover.typical({}) == nil)
	assert(cover.typical({ 2 }) == 2)
	assert(near(cover.typical({ 1, 2 }), (1 + 2 * 2) / 3), "the newer weighs more")
	assert(near(cover.typical({ 1, 1, 1, 1, 9 }), (1 + 2 + 3 + 4 + 2 * 5) / 15),
		"a 9 s turn among 1 s ones counts as 2 s: " .. cover.typical({ 1, 1, 1, 1, 9 }))
	assert(cover.typical({ 30, 1, 1, 1, 1, 1 }) == 1, "the last five only")
	assert(near(cover.typical({ 3, 1, 2 }, true), 2), "`all`: every one, alike")
	assert(near(cover.typical({ 0.2, 0.3 }), (0.2 + 0.6) / 3), "a fast seat stays fast")

	-- Which seats play before the person's turn is back, in turn order.
	local function same(a, b)
		if #a ~= #b then return false end
		for i = 1, #a do if a[i] ~= b[i] then return false end end
		return true
	end
	assert(same(cover.lineup(0), { 1, 2 }))
	local other = load(HOTSEAT({ HotseatSeats = { 0, 1, 3 }, HotseatHumanSeats = { 2 } }))
	assert(same(other.lineup(2), { 3, 0, 1 }), "after the person's seat, round to the start")
	local two = load(HOTSEAT({ HotseatSeats = { 1, 3 }, HotseatHumanSeats = { 0, 2 } }))
	assert(same(two.lineup(0), { 1 }) and same(two.lineup(2), { 3 }), "up to the next person")

	assert(cover.timeLeft(nil, false) == "")
	assert(cover.timeLeft(12, false) == "About 12 s" and cover.timeLeft(11.2, false) == "About 12 s")
	assert(cover.timeLeft(0.4, true) == "Any moment now", "nothing but the last stretch left")
	assert(cover.timeLeft(0.4, false) == "About 1 s" and cover.timeLeft(0, false) == "About 1 s",
		"never 'any moment' while a seat still plays")
	assert(cover.timeLeft(59.5, false) == "About 1 min 00 s")
	assert(cover.timeLeft(125, false) == "About 2 min 05 s")
end

-- Frames of 0.1 s; `each` after every one.
local function frames(handlers, seconds, each)
	for _ = 1, math.floor(seconds / 0.1 + 0.5) do
		handlers.update(0.1)
		if each then each() end
	end
end
-- One wait: the person (seat 0) ends turn `turn`; each { seat, gap, took }
-- plays in order; `tail` s later their next turn begins and its hand-over is
-- pressed.
local function wait(world, handlers, turn, seats, tail, each)
	world.pid, world.turn, world.active[0] = 0, turn, false
	handlers.LocalPlayerTurnEnd()
	for _, s in ipairs(seats) do
		frames(handlers, s[2], each)
		world.pid, world.active[s[1]] = s[1], true
		handlers.LocalPlayerTurnBegin()
		frames(handlers, s[3], each)
		world.active[s[1]] = false
		handlers.LocalPlayerTurnEnd()
	end
	frames(handlers, tail, each)
	world.pid, world.turn, world.active[0], world.paused[0] = 0, turn + 1, true, true
	handlers.LocalPlayerTurnBegin()
	world.paused[0] = false
	handlers.PlayerChange_Close(0)
end
local function field(line, name)
	return line:match('"' .. name .. '":"?([^,"}]*)')
end

-- The first wait counts as soon as one seat's turn is known.
do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatCoverTimer = true }))
	assert(last(did, "hotseat_cover_armed"):find('"timer":true', 1, true))
	world.pid, world.active[0] = 0, false
	handlers.LocalPlayerTurnEnd()
	assert(cover.wait and cover.wait.timed and cover.wait.seat == 0 and cover.wait.estimate == nil)
	assert(did.bar.hidden == false, "the bar is shown")
	assert(did.bar.text == "", "nothing to say yet")
	frames(handlers, 0.3)
	world.pid, world.active[1] = 1, true
	handlers.LocalPlayerTurnBegin()
	-- Nothing on record: a fifth of the track sweeping there and back, no number.
	local seen = {}
	for _ = 1, 24 do
		handlers.update(0.1)
		assert(did.bar.width == cover.BAR / 5, "a fifth of the track")
		assert(did.bar.offset >= 0 and did.bar.offset + did.bar.width <= cover.BAR, "inside it")
		seen[did.bar.offset] = true
	end
	local distinct = 0
	for _ in pairs(seen) do distinct = distinct + 1 end
	assert(distinct >= 10, "and it moves: " .. distinct)
	assert(did.bar.text == "1 of 2", "which seat, and no guess: " .. did.bar.text)
	frames(handlers, 0.6)
	world.active[1] = false
	handlers.LocalPlayerTurnEnd()
	-- Seat 1 took 3 s: seat 2 is taken to be like it.
	assert(math.abs(cover.seatTimes[1][1] - 3) < 1e-6 and math.abs(cover.gapTimes[1][1] - 0.3) < 1e-6)
	handlers.update(0.1)
	assert(did.bar.text == "1 of 2, about 4 s", did.bar.text)
	assert(did.bar.offset == 0 and did.bar.width > 1 and did.bar.width < cover.BAR / 2, did.bar.width)
	frames(handlers, 0.2)
	world.pid, world.active[2] = 2, true
	handlers.LocalPlayerTurnBegin()
	handlers.update(0.1)
	assert(did.bar.text == "2 of 2, about 3 s", did.bar.text)
	frames(handlers, 1.9)
	world.active[2] = false
	handlers.LocalPlayerTurnEnd()
	assert(cover.wait.timed and cover.wait.seat == 0, "a CIVVIS seat's end of turn starts no wait")
	-- Only the last stretch is left, and none is on record yet.
	handlers.update(0.1)
	assert(did.bar.text == "Any moment now", did.bar.text)
	frames(handlers, 1.4)
	world.pid, world.turn, world.active[0], world.paused[0] = 0, 2, true, true
	handlers.LocalPlayerTurnBegin()
	world.paused[0] = false
	handlers.PlayerChange_Close(0)
	assert(cover.wait == nil)
	local lifted = last(did, "hotseat_cover")
	assert(math.abs(tonumber(field(lifted, "waited")) - 7.1) < 1e-6, lifted)
	assert(field(lifted, "timed") == "true" and field(lifted, "estimate") == "null"
		and field(lifted, "seats") == "2", lifted)
	assert(lifted:find('"took":"1:3.00,2:2.00"', 1, true) and lifted:find('"said":"-,3.0"', 1, true), lifted)
	assert(math.abs(cover.tailTimes[0][1] - 1.5) < 1e-6, "the last stretch: " .. cover.tailTimes[0][1])
	assert(#cover.lineups[0] == 2)

	-- Their next wait counts down from the start: every part of it, as it goes.
	world.active[0] = false
	handlers.LocalPlayerTurnEnd()
	-- gap, seat 1, gap, seat 2, the last stretch
	assert(math.abs(cover.wait.estimate - (0.3 + 3 + 0.3 + 2 + 1.5)) < 1e-6, cover.wait.estimate)
	assert(did.bar.width == 1 and did.bar.text == "About 8 s", did.bar.text)
end

-- A seat that runs over holds the count; one that finishes early takes its
-- time off at once; the bar only moves forward, is never full, and is nearly
-- there when the person's turn is back. The last seat running over never says
-- "any moment".
do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatCoverTimer = true }))
	local usual = { { 1, 0.1, 2 }, { 2, 0.1, 2 } }
	for turn = 1, 3 do wait(world, handlers, turn, usual, 1.2, nil) end
	local widths, texts, previous = {}, {}, 0
	local function watch()
		assert(did.bar.width >= previous, "never backwards: " .. previous .. " -> " .. did.bar.width)
		assert(did.bar.width < cover.BAR, "never full")
		previous = did.bar.width
		widths[#widths + 1], texts[#texts + 1] = did.bar.width, did.bar.text
	end
	-- Seat 1 takes 8 s instead of 2.
	world.pid, world.turn, world.active[0] = 0, 4, false
	handlers.LocalPlayerTurnEnd()
	previous = 0
	frames(handlers, 0.1, watch)
	world.pid, world.active[1] = 1, true
	handlers.LocalPlayerTurnBegin()
	frames(handlers, 1.0, watch)
	assert(did.bar.text == "1 of 2, about 5 s", did.bar.text)
	frames(handlers, 1.5, watch)
	local held = did.bar.text
	assert(held == "1 of 2, about 4 s", held)
	local widthThen = did.bar.width
	frames(handlers, 5.5, watch)
	assert(did.bar.text == held, "held while it runs over: " .. did.bar.text)
	assert(did.bar.width > widthThen, "and the bar still creeps forward")
	world.active[1] = false
	handlers.LocalPlayerTurnEnd()
	frames(handlers, 0.1, watch)
	world.pid, world.active[2] = 2, true
	handlers.LocalPlayerTurnBegin()
	frames(handlers, 1.0, watch)
	assert(did.bar.text == "2 of 2, about 3 s", "moving again: " .. did.bar.text)
	-- The last seat runs over too: the count holds at the last stretch, a number.
	frames(handlers, 6, watch)
	assert(did.bar.text == "2 of 2, about 2 s", did.bar.text)
	for _, text in ipairs(texts) do assert(text ~= "Any moment now", "not while a seat plays") end
	world.active[2] = false
	handlers.LocalPlayerTurnEnd()
	frames(handlers, 1, watch)
	world.pid, world.turn, world.active[0], world.paused[0] = 0, 5, true, true
	handlers.LocalPlayerTurnBegin()
	handlers.PlayerChange_Close(0)

	-- An ordinary wait again: nearly full at the lift.
	previous = 0
	wait(world, handlers, 5, usual, 1.2, watch)
	assert(widths[#widths] >= 0.9 * cover.BAR, "there as the turn is: " .. widths[#widths])

	-- Seat 1 done in 0.5 s: its 1.5 s comes off at once (in text), and the bar
	-- steps forward over a few frames, not one.
	world.pid, world.turn, world.active[0] = 0, 6, false
	handlers.LocalPlayerTurnEnd()
	previous = 0
	frames(handlers, 0.1, watch)
	world.pid, world.active[1] = 1, true
	handlers.LocalPlayerTurnBegin()
	frames(handlers, 0.5, watch)
	local before, textBefore = did.bar.width, did.bar.text
	world.active[1] = false
	handlers.LocalPlayerTurnEnd()
	frames(handlers, 0.1, watch)
	local jump = did.bar.width - before
	assert(textBefore == "1 of 2, about 6 s" and did.bar.text == "1 of 2, about 4 s",
		textBefore .. " -> " .. did.bar.text)
	frames(handlers, 0.5, watch)
	assert(did.bar.width - before > jump, "the step is drawn over more than one frame")
end

-- One slow turn moves the next estimates by a little, and not for long.
do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatCoverTimer = true }))
	local usual = { { 1, 0.1, 2 }, { 2, 0.1, 2 } }
	for turn = 1, 4 do wait(world, handlers, turn, usual, 1) end
	local base = cover.remaining(cover.startWait(0))
	cover.wait = nil
	wait(world, handlers, 5, { { 1, 0.1, 20 }, { 2, 0.1, 2 } }, 1)
	local after = cover.remaining(cover.startWait(0))
	cover.wait = nil
	assert(after - base < 0.2 * 18, "18 s over moves the next by " .. (after - base))
	for turn = 6, 10 do wait(world, handlers, turn, usual, 1) end
	assert(math.abs(cover.remaining(cover.startWait(0)) - base) < 1e-6, "and five waits on, not at all")
	cover.wait = nil
	assert(#cover.seatTimes[1] == cover.KEEP and #cover.tailTimes[0] == cover.KEEP, "the last five kept")
end

-- The bar never goes back, even when the count goes up (the first wait: seat 2
-- far slower than the seat 1 the others were taken to be like), and is never
-- full while the wait lasts, however long the last stretch runs over.
do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatCoverTimer = true,
		HotseatSeats = { 1, 2, 3, 4 } }))
	local previous, rose = 0, false
	local before = nil
	local function watch()
		local left = cover.remaining(cover.wait)
		if left == nil then return end          -- the sweep, before seat 1's turn is known
		assert(did.bar.width >= previous, "never backwards: " .. previous .. " -> " .. did.bar.width)
		assert(did.bar.width < cover.BAR, "never full: " .. did.bar.width)
		previous = did.bar.width
		if before and left > before + 1 then rose = true end
		before = left
	end
	wait(world, handlers, 1, { { 1, 0.1, 1 }, { 2, 0.1, 6 }, { 3, 0.1, 1 }, { 4, 0.1, 1 } }, 0.5, watch)
	assert(rose, "the count did go up")
	previous, before = 0, nil
	wait(world, handlers, 2, { { 1, 0.1, 1 }, { 2, 0.1, 6 }, { 3, 0.1, 1 }, { 4, 0.1, 1 } }, 20, watch)
	assert(did.bar.width >= 0.95 * cover.BAR, "and close to the end: " .. did.bar.width)
end
-- A long stretch before the first seat counts down too.
do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatCoverTimer = true }))
	for turn = 1, 3 do wait(world, handlers, turn, { { 1, 4, 1.2 }, { 2, 0.1, 1.2 } }, 0.5) end
	world.pid, world.turn, world.active[0] = 0, 4, false
	handlers.LocalPlayerTurnEnd()
	handlers.update(0.1)
	assert(did.bar.text == "About 7 s", did.bar.text)
	frames(handlers, 2)
	assert(did.bar.text == "About 5 s", did.bar.text)
end
-- What is not a seat's own stretch is not learnt as one: a turn begun while
-- another seat's never ended, the end of a wait the cover stood aside in.
do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatCoverTimer = true }))
	world.pid, world.active[0] = 0, false
	handlers.LocalPlayerTurnEnd()
	world.pid, world.active[1] = 1, true
	handlers.LocalPlayerTurnBegin()
	frames(handlers, 0.5)
	handlers.LocalPlayerTurnBegin()          -- the same turn said twice: one seat, still counting
	assert(#cover.wait.seen == 1 and cover.wait.mark == 0, "seen " .. #cover.wait.seen)
	frames(handlers, 0.5)
	world.pid, world.active[2] = 2, true      -- seat 1's end never came
	handlers.LocalPlayerTurnBegin()
	assert(cover.gapTimes[2] == nil and cover.seatTimes[1] == nil and cover.wait.playing == 2)
	frames(handlers, 1)
	world.active[1], world.active[2] = false, false
	handlers.LocalPlayerTurnEnd()
	assert(#cover.seatTimes[2] == 1)
	-- Nothing begins for three minutes: the cover stands aside, nothing is learnt.
	frames(handlers, 181)
	assert(not cover.up and last(did, "hotseat_cover"):find('"reason":"yield_between"', 1, true))
	assert(cover.tailTimes[0] == nil and cover.lineups[0] == nil)
end

-- Fast turns and slow ones are counted alike: tenths of a second, or minutes.
for _, scale in ipairs({ 0.1, 15 }) do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatCoverTimer = true }))
	local seats = { { 1, 0.1, 2 * scale }, { 2, 0.1, 2 * scale } }
	for turn = 1, 3 do wait(world, handlers, turn, seats, 1, nil) end
	local w = cover.startWait(0)
	local left = cover.remaining(w)
	assert(math.abs(left - (0.2 + 4 * scale + 1)) < 0.11, scale .. ": " .. left)
	cover.wait = nil
end

-- The UI clock decides where it answers: a frame the engine caps the dt of.
do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatCoverTimer = true }))
	world.clock = 1000
	world.pid, world.active[0] = 0, false
	handlers.LocalPlayerTurnEnd()
	world.pid, world.active[1] = 1, true
	world.clock = 1000.5
	handlers.LocalPlayerTurnBegin()
	world.clock = 1005.5
	handlers.update(0.1)
	assert(math.abs(cover.wait.elapsed - 5.5) < 1e-9, "the clock's 5.5 s, not the frame's 0.1")
	world.clock = 900    -- a clock that jumped back: the frame's own dt
	handlers.update(0.25)
	assert(math.abs(cover.wait.elapsed - 5.75) < 1e-9)
	-- The stretch between the last frame and an event counts too.
	world.clock = 902
	world.active[1] = false
	handlers.LocalPlayerTurnEnd()
	assert(math.abs(cover.seatTimes[1][1] - 7.25) < 1e-9, "seat 1, begin to end: " .. cover.seatTimes[1][1])
	world.clock = 903
	world.pid, world.turn, world.active[0], world.paused[0] = 0, 2, true, false
	handlers.LocalPlayerTurnBegin()
	handlers.PlayerChange_Close(0)
	assert(math.abs(tonumber(field(last(did, "hotseat_cover"), "waited")) - 8.75) < 1e-9)
	assert(math.abs(cover.tailTimes[0][1] - 1) < 1e-9)
	-- Up and down are stamped with the UI clock (the seats' `end_turn_wait.ui_now`).
	assert(kinds(did, "hotseat_cover")[1]:find('"at":1000', 1, true), kinds(did, "hotseat_cover")[1])
	assert(last(did, "hotseat_cover"):find('"at":903', 1, true))
end

-- What does not go on record: what the game menu was opened over, a wait the
-- cover stood aside in, a wait joined part-way (which only sweeps).
do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatCoverTimer = true }))
	wait(world, handlers, 1, { { 1, 0.1, 2 } }, 1)
	assert(#cover.seatTimes[1] == 1 and #cover.tailTimes[0] == 1)
	-- The game menu opened over seat 1's turn: not how long CIVVIS takes.
	world.pid, world.turn, world.active[0] = 0, 2, false
	handlers.LocalPlayerTurnEnd()
	world.pid, world.active[1] = 1, true
	handlers.LocalPlayerTurnBegin()
	world.menuQueued = true
	frames(handlers, 40)
	world.menuQueued = false
	world.active[1] = false
	handlers.LocalPlayerTurnEnd()
	frames(handlers, 1)
	world.pid, world.turn, world.active[0], world.paused[0] = 0, 3, true, false
	handlers.LocalPlayerTurnBegin()
	handlers.PlayerChange_Close(0)
	assert(#cover.seatTimes[1] == 1, "40 s in the menu is not a seat's turn")
	assert(#cover.tailTimes[0] == 2, "the stretch after it, clean, is")
	assert(last(did, "hotseat_cover"):find('"timed":false', 1, true))
	-- The cover stood aside for a stalled seat; raised again, it only sweeps.
	world.active[0] = false
	handlers.LocalPlayerTurnEnd()
	world.pid, world.active[1] = 1, true
	handlers.LocalPlayerTurnBegin()
	frames(handlers, 91)
	assert(not cover.up and cover.wait == nil, "a stall is not a wait")
	assert(#cover.seatTimes[1] == 1 and #cover.tailTimes[0] == 2)
	world.active[1] = false
	handlers.LocalPlayerTurnEnd()
	assert(cover.up and cover.wait and not cover.wait.timed and cover.wait.estimate == nil,
		"joined part-way: no countdown")
	frames(handlers, 1)
	assert(did.bar.width == cover.BAR / 5 and did.bar.text == "", "a sweep: " .. did.bar.text)
	world.pid, world.turn, world.active[0], world.paused[0] = 0, 4, true, false
	handlers.LocalPlayerTurnBegin()
	handlers.PlayerChange_Close(0)
	assert(#cover.tailTimes[0] == 2, "the end of a wait joined part-way is not on record")
	-- A game loaded on a CIVVIS turn: a sweep; the seats seen whole are learnt.
	local cover2, world2, did2, handlers2 = load(HOTSEAT({ HotseatCoverTimer = true }))
	world2.pid, world2.turn, world2.active[1] = 1, 30, true
	handlers2.LoadScreenClose()
	assert(cover2.wait and not cover2.wait.timed)
	frames(handlers2, 3)
	assert(did2.bar.width == cover2.BAR / 5 and did2.bar.text == "")
	world2.active[1] = false
	handlers2.LocalPlayerTurnEnd()
	world2.pid, world2.active[2] = 2, true
	handlers2.LocalPlayerTurnBegin()
	frames(handlers2, 2)
	world2.active[2] = false
	handlers2.LocalPlayerTurnEnd()
	world2.pid, world2.turn, world2.active[0], world2.paused[0] = 0, 31, true, false
	handlers2.LocalPlayerTurnBegin()
	handlers2.PlayerChange_Close(0)
	assert(cover2.seatTimes[1] == nil and #cover2.seatTimes[2] == 1, "seat 2's turn was seen whole")
	assert(cover2.tailTimes[0] == nil and cover2.lineups[0] == nil)
end
-- `--no-cover-timer` and an older install show no bar.
for _, timer in ipairs({ false, "unset" }) do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatCoverTimer = timer ~= "unset" and timer or nil }))
	wait(world, handlers, 1, { { 1, 0.1, 5 } }, 1)
	world.active[0] = false
	handlers.LocalPlayerTurnEnd()
	ticks(handlers, 2)
	assert(did.bar.hidden == true and did.bar.width == nil and did.bar.text == nil, "no bar")
end
-- Two people at one table: each waits their own wait.
do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatCoverTimer = true,
		HotseatHumanSeats = { 0, 2 }, HotseatSeats = { 1, 3 }, HotseatAutoStart = false }))
	world.pid, world.active[0] = 0, false
	handlers.LocalPlayerTurnEnd()
	world.pid, world.active[1] = 1, true
	handlers.LocalPlayerTurnBegin()
	ticks(handlers, 9)
	assert(did.bar.text == "1 of 1", did.bar.text)
	world.active[1] = false
	handlers.LocalPlayerTurnEnd()
	world.pid, world.active[2], world.paused[2] = 2, true, true
	handlers.LocalPlayerTurnBegin()
	assert(not cover.up and #cover.tailTimes[0] == 1 and cover.tailTimes[0][1] == 0)
	world.active[2] = false
	handlers.LocalPlayerTurnEnd()
	assert(cover.wait.seat == 2)
	world.pid, world.active[3] = 3, true
	handlers.LocalPlayerTurnBegin()
	ticks(handlers, 4)
	world.active[3] = false
	handlers.LocalPlayerTurnEnd()
	ticks(handlers, 1)
	world.pid, world.turn, world.active[0], world.paused[0] = 0, 2, true, true
	handlers.LocalPlayerTurnBegin()
	assert(cover.tailTimes[2][1] == 1 and #cover.tailTimes[0] == 1)
	assert(#cover.lineups[0] == 1 and cover.lineups[0][1] == 1 and cover.lineups[2][1] == 3)
	assert(cover.seatTimes[1][1] == 9 and cover.seatTimes[3][1] == 4)
end

print("hotseat_cover_test: ok")
