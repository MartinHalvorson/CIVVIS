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
			"TeamVictory", "UserRequestClose" }) do
		env.Events[name] = event(name)
	end
	for _, name in ipairs({ "PlayerChange_Close", "EndGameMenu_Shown", "EndGameMenu_OneMoreTurn",
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
	ticks(handlers, 240, function()
		beat = beat + 1
		env.ExposedMembers.CivvisSeatBeat[1] = beat
	end)
	assert(cover.up, "a seat that keeps playing is never uncovered")
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
	assert(not cover.up)
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

-- 9. How long (`HotseatCoverTimer`). ------------------------------------------
do
	local track = layout:match('<Box ID="WaitTrack"[^>]*>')
	local cover = load(HOTSEAT())
	assert(track and track:find('Size="' .. cover.BAR .. ',', 1, true),
		"the script's bar width is the layout's track: " .. tostring(track))
	assert(layout:find('<Container ID="Wait"[^>]*Hidden="1"'), "hidden until a wait is on")
	assert(layout:find('ID="WaitFill"', 1, true) and layout:find('ID="WaitText"', 1, true))

	-- The estimate: nothing without a wait on record; the newest weighted most.
	assert(cover.estimate(0) == nil)
	cover.waits[0] = { 10 }
	assert(math.abs(cover.estimate(0) - 11) < 1e-9, "one wait and a tenth")
	cover.waits[0] = { 99, 10, 10, 10, 20 }
	assert(math.abs(cover.estimate(0) - (20 * 4 + 10 * 3 + 10 * 2 + 10) / 10 * 1.1) < 1e-9,
		"the last four, 4:3:2:1")
	assert(cover.estimate(3) == nil, "each person's seat has its own waits")
	cover.waits[0] = { 0.01 }
	assert(cover.estimate(0) == 0.5, "never less than half a second")

	-- The bar: straight to 85%, then ever slower, never full, never backwards.
	assert(cover.progress(5, nil) == nil and cover.progress(5, 0) == nil)
	assert(cover.progress(0, 10) == 0 and math.abs(cover.progress(4, 10) - 0.4) < 1e-9)
	assert(math.abs(cover.progress(8.5, 10) - 0.85) < 1e-9)
	local previous = 0
	for tenth = 0, 2000 do
		local share = cover.progress(tenth / 10, 10)
		assert(share >= previous and share < 1, "at " .. tenth / 10 .. " s: " .. share)
		previous = share
	end
	assert(cover.progress(10, 10) > 0.85 and cover.progress(10, 10) < 0.9, "on time: not yet full")
	assert(cover.progress(200, 10) == 0.99)

	assert(cover.remaining(3, nil) == "")
	assert(cover.remaining(0, 12) == "About 12 s")
	assert(cover.remaining(0.2, 12) == "About 12 s" and cover.remaining(1.5, 12) == "About 11 s")
	assert(cover.remaining(11.4, 12) == "About 1 s")
	assert(cover.remaining(11.6, 12) == "Any moment now" and cover.remaining(40, 12) == "Any moment now")
	assert(cover.remaining(0, 59.5) == "About 1 min 00 s", cover.remaining(0, 59.5))
	assert(cover.remaining(0, 125) == "About 2 min 05 s")
end
-- A person's game: the first wait is a sweep, the second a countdown.
local function round(world, handlers, seconds, turn)
	-- The person (seat 0) ends their turn; one CIVVIS seat plays `seconds`; their
	-- next turn begins and its hand-over is pressed.
	world.pid, world.active[0] = 0, false
	handlers.LocalPlayerTurnEnd()
	world.pid, world.active[1] = 1, true
	handlers.LocalPlayerTurnBegin()
	ticks(handlers, seconds)
	world.active[1] = false
	handlers.LocalPlayerTurnEnd()
	world.pid, world.turn, world.active[0], world.paused[0] = 0, turn, true, true
	handlers.LocalPlayerTurnBegin()
	world.paused[0] = false
	handlers.PlayerChange_Close(0)
end
do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatCoverTimer = true }))
	assert(last(did, "hotseat_cover_armed"):find('"timer":true', 1, true))
	world.pid, world.active[0] = 0, false
	handlers.LocalPlayerTurnEnd()
	assert(cover.wait and cover.wait.timed and cover.wait.seat == 0 and cover.wait.estimate == nil)
	assert(did.bar.hidden == false, "the bar is shown")
	world.pid, world.active[1] = 1, true
	handlers.LocalPlayerTurnBegin()
	-- No wait on record: a fifth of the track sweeping there and back, no number.
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
	assert(did.bar.text == "", "no guess")
	ticks(handlers, 5)
	world.active[1] = false
	handlers.LocalPlayerTurnEnd()
	assert(cover.wait.timed and cover.wait.seat == 0, "a CIVVIS seat's end of turn starts no wait")
	world.pid, world.turn, world.active[0], world.paused[0] = 0, 2, true, true
	handlers.LocalPlayerTurnBegin()
	world.paused[0] = false
	handlers.PlayerChange_Close(0)
	assert(cover.wait == nil and #cover.waits[0] == 1)
	assert(math.abs(cover.waits[0][1] - 7.4) < 1e-6, "the wait, whole: " .. cover.waits[0][1])
	assert(last(did, "hotseat_cover"):find('"waited":7.4', 1, true)
		and last(did, "hotseat_cover"):find('"timed":true', 1, true), last(did, "hotseat_cover"))

	-- Their next wait counts down from it: 7.4 s and a tenth.
	world.active[0] = false
	handlers.LocalPlayerTurnEnd()
	assert(math.abs(cover.wait.estimate - 8.14) < 1e-6)
	assert(did.bar.width == 1 and did.bar.offset == 0 and did.bar.text == "About 9 s", did.bar.text)
	world.pid, world.active[1] = 1, true
	handlers.LocalPlayerTurnBegin()
	ticks(handlers, 4)
	assert(did.bar.width == math.floor(4 / 8.14 * cover.BAR + 0.5), "4 s of 8.14: " .. did.bar.width)
	assert(did.bar.text == "About 5 s", did.bar.text)
	ticks(handlers, 4)
	assert(did.bar.text == "Any moment now" and did.bar.width < cover.BAR, did.bar.text)
	local late = did.bar.width
	ticks(handlers, 30)
	assert(did.bar.width > late and did.bar.width < cover.BAR, "a late turn: slower, never full")
	assert(did.bar.offset == 0)
	assert(#did.bar.hides == 1, "shown once, not every frame")
end
-- The UI clock decides where it answers: a frame the engine caps the dt of.
do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatCoverTimer = true }))
	world.clock = 1000
	world.pid, world.active[0] = 0, false
	handlers.LocalPlayerTurnEnd()
	world.pid, world.active[1] = 1, true
	handlers.LocalPlayerTurnBegin()
	world.clock = 1005.5
	handlers.update(0.1)
	assert(math.abs(cover.wait.elapsed - 5.5) < 1e-9, "the clock's 5.5 s, not the frame's 0.1")
	world.clock = 900    -- a clock that jumped back: the frame's own dt
	handlers.update(0.25)
	assert(math.abs(cover.wait.elapsed - 5.75) < 1e-9)
	-- The stretch between the last frame and the lift counts too.
	world.clock = 902
	world.active[1] = false
	handlers.LocalPlayerTurnEnd()
	world.pid, world.turn, world.active[0], world.paused[0] = 0, 2, true, false
	handlers.LocalPlayerTurnBegin()
	handlers.PlayerChange_Close(0)
	assert(math.abs(cover.waits[0][1] - 7.75) < 1e-9, cover.waits[0][1])
end
-- What does not go on record: a wait joined part-way, one the cover stood
-- aside in, one the game menu was opened over.
do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatCoverTimer = true }))
	round(world, handlers, 6, 2)
	assert(#cover.waits[0] == 1)
	-- The game menu opened over it.
	world.pid, world.active[0] = 0, false
	handlers.LocalPlayerTurnEnd()
	world.pid, world.active[1] = 1, true
	handlers.LocalPlayerTurnBegin()
	world.menuQueued = true
	ticks(handlers, 40)
	world.menuQueued = false
	world.active[1] = false
	handlers.LocalPlayerTurnEnd()
	world.pid, world.turn, world.active[0], world.paused[0] = 0, 3, true, false
	handlers.LocalPlayerTurnBegin()
	handlers.PlayerChange_Close(0)
	assert(#cover.waits[0] == 1, "40 s in the menu is not how long CIVVIS takes")
	assert(last(did, "hotseat_cover"):find('"timed":false', 1, true))
	-- The cover stood aside for a stalled seat; raised again, it only sweeps.
	world.active[0] = false
	handlers.LocalPlayerTurnEnd()
	world.pid, world.active[1] = 1, true
	handlers.LocalPlayerTurnBegin()
	ticks(handlers, 91)
	assert(not cover.up and cover.wait == nil and #cover.waits[0] == 1, "a stall is not a wait")
	world.active[1] = false
	handlers.LocalPlayerTurnEnd()
	assert(cover.up and cover.wait and not cover.wait.timed and cover.wait.estimate == nil,
		"joined part-way: no countdown")
	world.pid, world.turn, world.active[0], world.paused[0] = 0, 4, true, false
	handlers.LocalPlayerTurnBegin()
	handlers.PlayerChange_Close(0)
	assert(#cover.waits[0] == 1)
	-- A game loaded on a CIVVIS turn.
	local cover2, world2, did2, handlers2 = load(HOTSEAT({ HotseatCoverTimer = true }))
	world2.pid, world2.turn, world2.active[1] = 1, 30, true
	handlers2.LoadScreenClose()
	assert(cover2.wait and not cover2.wait.timed)
	ticks(handlers2, 3)
	assert(did2.bar.width == cover2.BAR / 5 and did2.bar.text == "")
	world2.active[1] = false
	handlers2.LocalPlayerTurnEnd()
	world2.pid, world2.turn, world2.active[0], world2.paused[0] = 0, 31, true, false
	handlers2.LocalPlayerTurnBegin()
	handlers2.PlayerChange_Close(0)
	assert(cover2.waits[0] == nil)
end
-- Eight waits are kept; `--no-cover-timer` and an older install show no bar.
do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatCoverTimer = true }))
	for turn = 2, 12 do round(world, handlers, turn, turn) end
	assert(#cover.waits[0] == 8 and cover.waits[0][8] == 12 and cover.waits[0][1] == 5)
end
for _, timer in ipairs({ false, "unset" }) do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatCoverTimer = timer ~= "unset" and timer or nil }))
	round(world, handlers, 5, 2)
	world.active[0] = false
	handlers.LocalPlayerTurnEnd()
	ticks(handlers, 2)
	assert(did.bar.hidden == true and did.bar.width == nil and did.bar.text == nil, "no bar")
end
-- Two people at one table: each waits their own wait.
do
	local cover, world, did, handlers = load(HOTSEAT({ HotseatCoverTimer = true,
		HotseatHumanSeats = { 0, 2 }, HotseatSeats = { 1 }, HotseatAutoStart = false }))
	world.pid, world.active[0] = 0, false
	handlers.LocalPlayerTurnEnd()
	world.pid, world.active[1] = 1, true
	handlers.LocalPlayerTurnBegin()
	ticks(handlers, 9)
	world.active[1] = false
	handlers.LocalPlayerTurnEnd()
	world.pid, world.active[2], world.paused[2] = 2, true, true
	handlers.LocalPlayerTurnBegin()
	assert(not cover.up and #cover.waits[0] == 1 and cover.waits[0][1] == 9)
	world.active[2] = false
	handlers.LocalPlayerTurnEnd()
	assert(cover.wait.seat == 2)
	world.pid, world.turn, world.active[0], world.paused[0] = 0, 2, true, true
	ticks(handlers, 1)
	handlers.LocalPlayerTurnBegin()
	assert(#cover.waits[2] == 1 and cover.waits[2][1] == 1 and #cover.waits[0] == 1)
end

print("hotseat_cover_test: ok")
