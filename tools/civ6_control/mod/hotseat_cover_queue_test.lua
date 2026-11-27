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

print("hotseat_cover_queue_test: ok")
