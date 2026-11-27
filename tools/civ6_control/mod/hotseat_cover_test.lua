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
	         looks = {}, endGameQueued = false }
end

-- Load the cover into a fresh context with `config`. Returns the cover table,
-- the world it reads, and everything it did.
local function load(config)
	local world = newWorld()
	local did = { queued = {}, dequeued = 0, logs = {}, menus = 0, nudges = {}, detail = nil,
	              texture = nil }
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
	local endGameMenu = {}
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
			IsInPopupQueue = function(self, ctx) return ctx == endGameMenu and world.endGameQueued end,
		},
		Controls = {
			Detail = { SetText = function(self, text) did.detail = text end },
			Lines = { CalculateSize = function() end },
			Backdrop = { SetTexture = function(self, name) did.texture = name end },
			MenuButton = { RegisterCallback = function(self, kind, f) handlers.menuButton = f end },
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
		end,
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

print("hotseat_cover_test: ok")
