-- The hand-over and the screen closers under the hotseat cover
-- (CivvisControlAutoClose.lua): the person's Start Turn pressed for them
-- (`HotseatAutoStart`), the cover's backstop press, and every screen kept
-- drawn and ticking under the cover on a CIVVIS seat's turn (`CivvisCoverQueue`).
-- Run: lua5.1 tools/civ6_control/mod/hotseat_cover_queue_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."
local function read(name)
	local handle = assert(io.open(here .. "/" .. name, "r"))
	local text = handle:read("*a")
	handle:close()
	return text
end
local closer = read("CivvisControlAutoClose.lua")

-- 1. The person's hand-over. ------------------------------------------------
local screenBlock = assert(closer:match("(CivvisHotseatScreen = { .-\n)if NAME == \"PlayerChange\" then"),
	"AutoClose defines CivvisHotseatScreen before the PlayerChange arm")
local localPlayer, turn, dialogHidden, waitLineHidden = 0, 4, false, false
local paused, active, password, held = {}, {}, {}, false
local reports, presses, listeners = {}, 0, {}
local function screenEnv(config)
	local env = {
		cfg = config, RUN = "hs",
		tonumber = tonumber, tostring = tostring, type = type, pcall = pcall,
		ipairs = ipairs, string = string,
		report = function(kind, fields) reports[#reports + 1] = { kind = kind, fields = fields or "" } end,
		Game = { GetLocalPlayer = function() return localPlayer end,
		         GetCurrentGameTurn = function() return turn end },
		Controls = {
			PopupAlphaIn = { IsHidden = function() return dialogHidden end,
			                 SetHide = function(self, hide) dialogHidden = hide end },
			PlayerChangingText = { IsHidden = function() return waitLineHidden end,
			                       SetHide = function(self, hide) waitLineHidden = hide end },
		},
		OnOk = function() presses = presses + 1 end,
		PlayerConfigurations = setmetatable({}, { __index = function(_, pid)
			return { GetWantsPause = function() return paused[pid] == true end,
			         GetHotseatPassword = function() return password[pid] or "" end }
		end }),
		Players = setmetatable({}, { __index = function(_, pid)
			return { IsTurnActive = function() return active[pid] == true end }
		end }),
		ExposedMembers = {},
		SaveLocations = { LOCAL_STORAGE = 7 },
		Network = { GetGameConfigurationSaveType = function() return 3 end, SaveGame = function() end },
		Events = { LocalPlayerTurnBegin = { Add = function() end },
		           LocalPlayerTurnEnd = { Add = function() end } },
		LuaEvents = { CivvisHotseatStartTurn = { Add = function(f) listeners.startTurn = f end } },
		ContextPtr = { SetUpdate = function() end },
	}
	env.ExposedMembers.CivvisOperatorHold = held
	local chunk = assert(loadstring(screenBlock))
	setfenv(chunk, env)
	chunk()
	return env.CivvisHotseatScreen, env
end
local base = { Hotseat = true, HotseatHumanSeats = { 0 }, HotseatSeats = { 1 }, HotseatStubSeats = {} }
local function with(extra)
	local out = {}
	for k, v in pairs(base) do out[k] = v end
	for k, v in pairs(extra) do out[k] = v end
	return out
end

-- Without `HotseatAutoStart` (every install before it, `--keep-handover`):
-- the operator's hand-over is theirs, exactly as before.
local screen = screenEnv(with({ HotseatAutoStart = false }))
localPlayer = 0
for _ = 1, 20 do screen.frame(0.1) end
assert(presses == 0, "--keep-handover: nobody presses the operator's Start Turn")
assert(screen.mayPress(0) == false and screen.mayPress(1) == true)

-- With it: pressed after the same short wait a CIVVIS seat gets, and said so.
screen = screenEnv(with({ HotseatAutoStart = true }))
screen.frame(0.1)
assert(presses == 0, "not on the first frame")
screen.frame(0.25)
assert(presses == 1, "the operator's hand-over is pressed for them")
local row = reports[#reports]
assert(row.kind == "hotseat_start_turn" and row.fields:find('"person":true', 1, true)
	and row.fields:find('"via":"dialog"', 1, true), row.fields)
-- Never a save on the operator's press: saves belong to the first CIVVIS seat.
for _, r in ipairs(reports) do assert(r.kind ~= "hotseat_saved") end
-- A seat with a hotseat password keeps its screen (PlayerChange.lua:42-43).
password[0] = "secret"
reports = {}
for _ = 1, 30 do screen.frame(0.2) end
assert(presses == 1 and #reports == 0, "a password is typed by its owner")
password[0] = nil
-- A single-player install has no hand-over and no human seat list.
local solo = screenEnv({ HotseatAutoStart = true, HotseatHumanSeats = { 0 } })
assert(solo.autoStart(0) == false and solo.mayPress(0) == false)

-- The cover's backstop press: only on a standing pause, on the local seat's
-- active turn, for a seat this presses, and never over an operator hold.
screen, presses = screenEnv(with({ HotseatAutoStart = true, HotseatCover = true })), 0
screen.arm()
assert(type(listeners.startTurn) == "function", "listens for the cover in a cover game")
localPlayer, active[1], paused[1] = 1, true, true
assert(screen.fromCover(0, 4) == false and presses == 0, "not another seat's")
assert(screen.fromCover(1, 4) == true and presses == 1)
assert(reports[#reports].fields:find('"via":"cover"', 1, true))
paused[1] = false
assert(screen.fromCover(1, 4) == false and presses == 1, "nothing to press once unpaused")
paused[1], active[1] = true, false
assert(screen.fromCover(1, 4) == false, "not between turns")
active[1] = true
localPlayer, active[0], paused[0] = 0, true, true
assert(screen.fromCover(0, 4) == true and presses == 2, "the operator's, with HotseatAutoStart")
local keep = screenEnv(with({ HotseatAutoStart = false, HotseatCover = true }))
assert(keep.fromCover(0, 4) == false, "--keep-handover: the cover never presses theirs")
held = true
local holding = screenEnv(with({ HotseatAutoStart = true, HotseatCover = true }))
assert(holding.fromCover(0, 4) == false, "an operator hold is never pressed through")
held = false
listeners.startTurn = nil
screenEnv(with({ HotseatAutoStart = true })).arm()
assert(listeners.startTurn == nil, "no cover, no listener")

-- Over the cover the hand-over is pressed as it ships, but never drawn: its
-- dialog is hidden on its first frame and its "please wait" line with it.
do
	local covered = screenEnv(with({ HotseatAutoStart = true, HotseatCover = true }))
	presses, reports = 0, {}
	localPlayer, turn, active[1], paused[1] = 1, 9, true, true
	dialogHidden, waitLineHidden = false, false
	covered.frame(0.05)
	assert(dialogHidden == true and waitLineHidden == true, "hidden on the first frame")
	assert(presses == 0, "not pressed on the first frame")
	covered.frame(0.1)
	covered.frame(0.2)
	assert(presses == 1, "pressed on the same clock as a shown dialog")
	assert(reports[#reports].fields:find('"via":"dialog"', 1, true))
	-- The pause stands (the press did not take): pressed again 2 s on.
	covered.frame(1.0)
	assert(presses == 1)
	covered.frame(1.1)
	assert(presses == 2 and reports[#reports].fields:find('"again":true', 1, true))
	-- It took: nothing more is pressed however long the hand-over stays up.
	paused[1] = false
	for _ = 1, 20 do covered.frame(0.5) end
	assert(presses == 2, "never pressed once unpaused")
	-- "Please wait" between turns, whoever's turn ended: the line stays hidden.
	waitLineHidden = false
	localPlayer = 0
	covered.frame(0.1)
	assert(waitLineHidden == true)
	-- With --keep-handover the person's own dialog is theirs, over no cover.
	local keepOwn = screenEnv(with({ HotseatAutoStart = false, HotseatCover = true }))
	dialogHidden, presses = false, 0
	localPlayer, turn, paused[0] = 0, 10, true
	for _ = 1, 10 do keepOwn.frame(0.2) end
	assert(dialogHidden == false and presses == 0, "their dialog, their press")
	-- Without the cover, the dialog is never touched.
	local plain = screenEnv(with({ HotseatAutoStart = true }))
	dialogHidden, waitLineHidden, presses = false, false, 0
	localPlayer, turn, paused[1] = 1, 11, true
	for _ = 1, 3 do plain.frame(0.2) end
	assert(dialogHidden == false and waitLineHidden == false and presses == 1)
	dialogHidden = true
	for _ = 1, 20 do plain.frame(0.5) end
	assert(presses == 1, "no dialog, no press: exactly as before")
	paused[1], paused[0], dialogHidden = nil, nil, false
end

-- 2. Screens under the cover stay drawn on a CIVVIS seat's turn. ------------
local queueBlock = assert(closer:match("(CivvisCoverQueue = { .-\n)if CivvisCoverQueue%.on%(%) then"),
	"AutoClose defines CivvisCoverQueue")
assert(closer:find("if CivvisCoverQueue.on() then", 1, true)
	< closer:find("if not haveScreen() then pcall(function() include(NAME); end); end", 1, true),
	"the proxy is in place before the shipped screen is loaded")

local function fakeManager(answers)
	local real = { calls = {}, Version = 3 }
	local asked = 0
	function real.QueuePopup(self, context, priority, params)
		assert(self == real, "QueuePopup reached with the real self")
		real.calls[#real.calls + 1] = { context = context, priority = priority, params = params }
	end
	function real.DequeuePopup(self, context)
		assert(self == real, "DequeuePopup reached with the real self")
		real.dequeued = context
	end
	function real.IsInPopupQueue(self, context)
		assert(self == real, "IsInPopupQueue reached with the real self")
		asked = asked + 1
		return answers and answers[asked] or false
	end
	return real
end
local function queueEnv(config, name, manager)
	local me = {}
	local env = {
		cfg = config, NAME = name, END_SCREENS = { EndGameMenu = true },
		tonumber = tonumber, tostring = tostring, type = type, pcall = pcall,
		ipairs = ipairs, pairs = pairs, setmetatable = setmetatable, string = string,
		report = function(kind, fields) reports[#reports + 1] = { kind = kind, fields = fields or "" } end,
		Game = { GetLocalPlayer = function() return localPlayer end },
		ContextPtr = me, UIManager = manager,
	}
	local chunk = assert(loadstring(queueBlock))
	setfenv(chunk, env)
	chunk()
	return env, me
end
local coverGame = with({ HotseatCover = true, HotseatSeats = { 1, 2 } })

-- Off in every game without the cover, and with `--no-cover-queue`: the
-- global is never touched.
for _, config in ipairs({ {}, with({ HotseatCover = false }), { HotseatCover = true },
		with({ HotseatCover = true, HotseatCoverQueue = false }) }) do
	local manager = fakeManager()
	local env = queueEnv(config, "TechCivicCompletedPopup", manager)
	local ok, why = env.CivvisCoverQueue.install()
	assert(ok == false and why == "off" and env.UIManager == manager)
end

do
	local manager = fakeManager()
	local env, me = queueEnv(coverGame, "TechCivicCompletedPopup", manager)
	local ok, how = env.CivvisCoverQueue.install()
	assert(ok == true and how == "table" and env.UIManager ~= manager, tostring(how))
	local proxy = env.UIManager
	-- A CIVVIS seat's turn: kept drawn, the screen's own parameters kept, its
	-- table not written into.
	localPlayer = 1
	local own = { DelayShow = true }
	proxy:QueuePopup(me, 40, own)
	local call = manager.calls[#manager.calls]
	assert(call.priority == 40 and call.params.DelayShow == true
		and call.params.AlwaysVisibleInQueue == true, "drawn under the cover")
	assert(own.AlwaysVisibleInQueue == nil, "the caller's table is left as it was")
	proxy:QueuePopup(me, 40)
	assert(manager.calls[#manager.calls].params.AlwaysVisibleInQueue == true, "no parameters at all")
	-- The person's turn: exactly as shipped (the same table, untouched).
	localPlayer = 0
	proxy:QueuePopup(me, 40, own)
	assert(manager.calls[#manager.calls].params == own)
	-- Another context queued from here (a child screen) is left alone.
	localPlayer = 1
	local child = {}
	proxy:QueuePopup(child, 40, own)
	assert(manager.calls[#manager.calls].params == own)
	-- Every other call reaches the real object, with the real self.
	proxy:DequeuePopup(me)
	assert(manager.dequeued == me)
	assert(proxy:IsInPopupQueue(me) == false)
	assert(proxy.Version == 3, "plain values read through")
	proxy.Flag = true
	assert(manager.Flag == true, "writes go through")
	-- Called the long way round too.
	proxy.QueuePopup(proxy, me, 41, nil)
	assert(manager.calls[#manager.calls].priority == 41)
end

-- The hand-over and the end screen get no proxy at all: they sit above the
-- cover and are queued as they ship.
for _, name in ipairs({ "PlayerChange", "EndGameMenu" }) do
	local manager = fakeManager()
	local env = queueEnv(coverGame, name, manager)
	local ok, why = env.CivvisCoverQueue.install()
	assert(ok == false and why == "off" and env.UIManager == manager, name)
	assert(env.CivvisCoverQueue.wanted(name, 1) == false, name)
end

-- Fail open: a proxy that does not answer like the real object is not used.
do
	local manager = fakeManager({ true, false })
	local env = queueEnv(coverGame, "WonderBuiltPopup", manager)
	local ok, why = env.CivvisCoverQueue.install()
	assert(ok == false and why == "proxy_refused" and env.UIManager == manager)
	local none = queueEnv(coverGame, "WonderBuiltPopup", nil)
	assert(select(2, none.CivvisCoverQueue.install()) == "no_uimanager")
end

-- 2b. The screens that ask first are let up on a CIVVIS seat's turn. ---------
-- HistoricMoments.lua:163 and BoostUnlockedPopup.lua:257 ask `UI.CanShowPopup`
-- and, told no, keep the announcement for later: under the cover that was the
-- person's next turn (five seats' first moments at once, 2026-11-27).
assert(closer:find("pcall(CivvisCoverQueue.installAsk)", 1, true)
	< closer:find("if not haveScreen() then pcall(function() include(NAME); end); end", 1, true),
	"the answer is in place before the shipped screen is loaded")
local function fakeUI(answer)
	local real = { asked = {}, sounds = {}, Version = 7 }
	function real.CanShowPopup(priority)
		real.asked[#real.asked + 1] = priority
		return answer
	end
	function real.PlaySound(name) real.sounds[#real.sounds + 1] = name end
	return real
end
local function askEnv(config, name, ui, extra)
	local env = queueEnv(config, name, fakeManager())
	env.UI = ui
	for k, v in pairs(extra or {}) do env[k] = v end
	return env
end
for _, name in ipairs({ "HistoricMoments", "BoostUnlockedPopup" }) do
	for _, answer in ipairs({ false, true }) do
		local ui = fakeUI(answer)
		local env = askEnv(coverGame, name, ui)
		local ok, how = env.CivvisCoverQueue.installAsk()
		assert(ok == true and how == "table" and env.UI ~= ui, name .. ": " .. tostring(how))
		-- A CIVVIS seat's turn: yes, whatever the game says (the cover is why it says no).
		localPlayer = 1
		assert(env.UI.CanShowPopup(30) == true and #ui.asked == 0, name)
		localPlayer = 2
		assert(env.UI.CanShowPopup(30) == true and env.CivvisCoverQueue.let == 2)
		-- The person's turn: the game's own answer, asked with what was passed.
		localPlayer = 0
		assert(env.UI.CanShowPopup(30) == answer and #ui.asked == 1 and ui.asked[1] == 30,
			name .. ": the person's turn is the game's to answer")
		assert(env.CivvisCoverQueue.let == 2)
		-- Every other name is the real one's.
		env.UI.PlaySound("Pride_Moment")
		assert(ui.sounds[1] == "Pride_Moment" and env.UI.Version == 7)
		env.UI.Flag = true
		assert(ui.Flag == true, "writes go through")
	end
end
localPlayer = 1
-- No other screen's `UI` is touched, and none without the cover and its queue.
for _, case in ipairs({
	{ coverGame, "TechCivicCompletedPopup" }, { coverGame, "WonderBuiltPopup" },
	{ coverGame, "PlayerChange" }, { coverGame, "EndGameMenu" },
	{ with({ HotseatCover = false }), "HistoricMoments" },
	{ with({ HotseatCover = true, HotseatCoverQueue = false }), "BoostUnlockedPopup" },
	{ {}, "HistoricMoments" },
}) do
	local ui = fakeUI(false)
	local env = askEnv(case[1], case[2], ui)
	local ok, why = env.CivvisCoverQueue.installAsk()
	assert(ok == false and why == "off" and env.UI == ui, case[2])
	assert(env.UI.CanShowPopup(30) == false)
end
-- The person-facing install: yes only while its harness is live and the seat is CIVVIS's.
do
	local live = false
	local gated = with({ HotseatCover = true, HotseatSeats = { 1, 2 }, RequireHarness = true })
	local ui = fakeUI(false)
	local env = askEnv(gated, "HistoricMoments", ui, { CivvisHarness = {
		dormant = function() return false end,
		mine = function(pid) return live and pid ~= 0 end } })
	assert(env.CivvisCoverQueue.installAsk() == true)
	localPlayer = 1
	assert(env.UI.CanShowPopup(30) == false, "no live harness: the seat is a person's, as shipped")
	live = true
	assert(env.UI.CanShowPopup(30) == true)
	localPlayer = 0
	assert(env.UI.CanShowPopup(30) == false)
end
-- Fail open: a game whose `UI` has no such question, or no `UI`, is left alone.
do
	local bare = { PlaySound = function() end }
	local env = askEnv(coverGame, "HistoricMoments", bare)
	local ok, why = env.CivvisCoverQueue.installAsk()
	assert(ok == false and why == "no_can_show" and env.UI == bare)
	local none = askEnv(coverGame, "HistoricMoments", nil)
	assert(select(2, none.CivvisCoverQueue.installAsk()) == "no_ui")
end
localPlayer = 0

-- 3. The card that parked run cover-live-6: the whole closer, loaded as the
-- HistoricMoments context (as popup_retry_test.lua loads it), on seat 0's turn.
local function card(config)
	local h = { hidden = false, closes = 0, logs = {} }
	local env = setmetatable({ CivvisControlConfig = config, ExposedMembers = {} }, { __index = _G })
	env.include = function() end
	env.OnClose = function() h.closes = h.closes + 1; h.hidden = true end
	env.OnShow = function() end
	env.Controls = {}
	env.Game = { GetLocalPlayer = function() return 0 end, GetCurrentGameTurn = function() return 1 end }
	env.ContextPtr = {
		GetID = function() return "HistoricMoments" end, IsHidden = function() return h.hidden end,
		SetUpdate = function(_, f) h.tick = f end, SetShowHandler = function(_, f) h.show = f end,
	}
	env.Automation = { Log = function(line) h.logs[#h.logs + 1] = line end }
	env.LuaEvents = {}
	local chunk = assert(loadfile(here .. "/CivvisControlAutoClose.lua"))
	setfenv(chunk, env)
	chunk()
	return h
end
local gated = { Hotseat = true, HotseatSeats = { 1 }, HotseatHumanSeats = { 0 },
                HotseatStubSeats = { 0 }, HotseatCover = true, HotseatAutoStart = true,
                AnnouncementSeconds = 0.25, EraAnnouncementSeconds = 0.25 }
local stubCard = card(gated)
for _ = 1, 40 do if not stubCard.hidden then stubCard.tick(0.25) end end
assert(stubCard.closes > 0 and stubCard.hidden, "the gated stub's own card is closed for it")
local person = {}
for k, v in pairs(gated) do person[k] = v end
person.HotseatStubSeats = {}
local personCard = card(person)
for _ = 1, 400 do personCard.tick(0.25) end
assert(personCard.closes == 0 and not personCard.hidden, "a real person's card is theirs to close")

-- 4. The scenarios' event popup (install.py HOTSEAT_EVENT_POPUP): the whole
-- closer loaded as the EventPopup context over a model of the shipped script,
-- Base/Assets/UI/Popups/EventPopup.lua:
--   :35    if (kPopupData.ForPlayer ~= nil and kPopupData.ForPlayer ~= playerID) then return; end
--   :41-46 shown at once if none is up, else kept in its own list (m_kQueuedPopups)
--   :175   UIManager:QueuePopup(ContextPtr, PopupPriority.Medium);
--   :253-256 OnClose: UIManager:DequeuePopup(ContextPtr); ShowNextQueuedPopup();
--   :303-306 OnUIIdle: if UI.CanShowPopup(PopupPriority.Medium) then ShowNextQueuedPopup(); end
-- Under the cover a queued popup is held hidden unless it is AlwaysVisibleInQueue,
-- and the cover is a higher popup, so `UI.CanShowPopup` says no.
local function eventPopup(config, pid)
	local h = { hidden = true, inQueue = false, covered = pid ~= 0, visible = {}, shown = {}, logs = {} }
	local env = setmetatable({ CivvisControlConfig = config, ExposedMembers = {} }, { __index = _G })
	local manager = {}
	function manager:QueuePopup(context, priority, params)
		local always = type(params) == "table" and params.AlwaysVisibleInQueue == true
		h.visible[#h.visible + 1] = always
		h.inQueue = true
		h.hidden = h.covered and not always
	end
	function manager:DequeuePopup() h.inQueue, h.hidden = false, true end
	function manager:IsInPopupQueue() return h.inQueue end
	env.UIManager = manager
	env.UI = { CanShowPopup = function() return not h.covered end, PlaySound = function() end }
	-- The model runs in the context's own environment, as the shipped script
	-- would: `UIManager`, `UI`, `Game` and `ContextPtr` are read when called.
	env.include = function(name)
		if name ~= "EventPopup" then return end
		local model = assert(loadstring([[
			local waiting, queued = false, {}
			function ShowCompletedPopup(data)
				h.shown[#h.shown + 1] = data.EventKey
				UIManager:QueuePopup(ContextPtr, 30)
				waiting = true
			end
			function ShowNextQueuedPopup()
				for i, entry in ipairs(queued) do
					ShowCompletedPopup(entry)
					table.remove(queued, i)
					break
				end
				if #queued == 0 then waiting = false end
			end
			function OnAddEventPopupRequest(data)
				if data.ForPlayer ~= nil and data.ForPlayer ~= Game.GetLocalPlayer() then return end
				if not waiting then ShowCompletedPopup(data) else table.insert(queued, data) end
			end
			function OnClose()
				h.closes = (h.closes or 0) + 1
				UIManager:DequeuePopup(ContextPtr)
				ShowNextQueuedPopup()
			end
			function OnUIIdle()
				if UI.CanShowPopup(30) then ShowNextQueuedPopup() end
			end
			h.queued = function() return #queued end
		]], "EventPopup"))
		setfenv(model, env)
		model()
	end
	env.h = h
	env.Controls = {}
	env.Game = { GetLocalPlayer = function() return pid end, GetCurrentGameTurn = function() return 40 end }
	env.ContextPtr = {
		GetID = function() return "EventPopup" end, IsHidden = function() return h.hidden end,
		SetUpdate = function(_, f) h.tick = f end, SetShowHandler = function() end,
	}
	env.Automation = { Log = function(line) h.logs[#h.logs + 1] = line end }
	env.LuaEvents = {}
	local chunk = assert(loadfile(here .. "/CivvisControlAutoClose.lua"))
	setfenv(chunk, env)
	chunk()
	h.env = env
	h.run = function(seconds)
		for _ = 1, seconds * 4 do if not h.hidden then h.tick(0.25) end end
	end
	return h
end
local scenario = { Hotseat = true, HotseatSeats = { 1, 2 }, HotseatHumanSeats = { 0 }, HotseatStubSeats = {},
                   HotseatCover = true, HotseatAutoStart = true, AnnouncementSeconds = 0.25 }
do
	-- A CIVVIS seat's two events on its own turn: drawn under the cover, closed in turn.
	local h = eventPopup(scenario, 1)
	h.env.OnAddEventPopupRequest({ ForPlayer = 1, EventKey = "SCENARIO_EVENT_A" })
	h.env.OnAddEventPopupRequest({ ForPlayer = 1, EventKey = "SCENARIO_EVENT_B" })
	assert(h.visible[1] == true and not h.hidden, "drawn under the cover, not held for the person")
	assert(h.queued() == 1, "the second waits in the screen's own list")
	h.run(10)
	assert(h.closes == 2 and h.hidden and h.queued() == 0 and not h.inQueue,
		"both closed on the seat's own turn: " .. tostring(h.closes))
	assert(h.visible[2] == true and h.shown[2] == "SCENARIO_EVENT_B")
	-- Its own list drains on UIIdle too: the cover is not a reason to keep it.
	assert(h.env.UI.CanShowPopup(30) == true, "a CIVVIS seat's turn: told yes")
	assert(eventPopup(scenario, 2).env.UI.CanShowPopup(30) == true)
	-- Another player's event is the shipped script's to ignore (:35).
	local other = eventPopup(scenario, 1)
	other.env.OnAddEventPopupRequest({ ForPlayer = 0, EventKey = "SCENARIO_EVENT_A" })
	assert(#other.shown == 0 and other.hidden)
end
do
	-- The person's own event on their turn is theirs to read and close.
	local h = eventPopup(scenario, 0)
	h.env.OnAddEventPopupRequest({ ForPlayer = 0, EventKey = "SCENARIO_EVENT_A" })
	assert(not h.hidden and h.visible[1] == false, "the shipped queueing on the person's turn")
	h.run(100)
	assert((h.closes or 0) == 0 and not h.hidden, "never closed for the person")
	h.covered = true
	assert(h.env.UI.CanShowPopup(30) == false, "and the game answers for itself on the person's turn")
end

print("hotseat_cover_queue_test: ok")
