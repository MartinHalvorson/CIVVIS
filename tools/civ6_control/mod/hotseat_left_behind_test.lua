-- A CIVVIS seat's era score is not left for the person's turn
-- (CivvisControlAutoClose.lua, `CivvisCoverQueue.leftBehind`). The whole closer
-- is loaded as each screen's context over a model of the shipped script, in the
-- shipped functions' own shapes:
--   DLC/Expansion2/UI/Additions/HistoricMoments.lua
--     :128-135 ShowNewTimelineMoment(popupData)  -> DisplayTimeline -> Show
--     :138-177 OnProcessNotification(playerID, notificationID, activatedByUser):
--              `if playerID == Game.GetLocalPlayer()`, `popupData.playerID = playerID`,
--              `if not UI.CanShowPopup(...) then table.insert(m_kQueuedPopups, popupData)`
--     :412-421 ShowNextQueuedPopup, :424-443 Show (`if ContextPtr:IsHidden()` -> QueuePopup),
--     :446-452 Close (`if ContextPtr:IsVisible()` -> DequeuePopup, HistoricMoments_Closed),
--     :520-525 OnUIIdle, :533-538 OnLocalPlayerTurnEnd (hotseat: Close if visible)
--   Base/Assets/UI/Popups/TechCivicCompletedPopup.lua :303-308 Close, :398-402
--   Base/Assets/UI/Popups/BoostUnlockedPopup.lua :17-29 ShowBoost, :302-319, :340-344
--   DLC/Expansion2/UI/Additions/EraReviewPopup.lua :237-247 OnShow/OnClose/OnContinue
-- Run: lua5.1 tools/civ6_control/mod/hotseat_left_behind_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."

local MODELS = {
	HistoricMoments = [[
		local m_kQueuedPopups = {}
		function ShowNewTimelineMoment(popupData)
			h.shown[#h.shown + 1] = popupData.playerID
			Show()
		end
		function OnProcessNotification(playerID, notificationID, activatedByUser)
			if playerID == Game.GetLocalPlayer() then
				local popupData = { showAnim = true, momentID = notificationID, playerID = playerID,
				                    notificationID = notificationID, activatedByUser = activatedByUser }
				if not UI.CanShowPopup(30) then table.insert(m_kQueuedPopups, popupData); return end
				ShowNewTimelineMoment(popupData)
			end
		end
		function ShowNextQueuedPopup()
			for i, entry in ipairs(m_kQueuedPopups) do
				ShowNewTimelineMoment(entry)
				table.remove(m_kQueuedPopups, i)
				break
			end
		end
		function Show()
			if ContextPtr:IsHidden() then
				UIManager:QueuePopup(ContextPtr, 30, {})
				LuaEvents.HistoricMoments_Opened()
			end
		end
		function Close()
			if ContextPtr:IsVisible() then
				UIManager:DequeuePopup(ContextPtr)
				LuaEvents.HistoricMoments_Closed()
			end
		end
		function OnClose() Close() end
		function OnUIIdle() if UI.CanShowPopup(30) then ShowNextQueuedPopup() end end
		function OnLocalPlayerTurnEnd()
			if GameConfiguration.IsHotseat() and ContextPtr:IsVisible() then Close() end
		end
		Events.NotificationActivated.Add(OnProcessNotification)
		Events.LocalPlayerTurnEnd.Add(OnLocalPlayerTurnEnd)
		Events.UIIdle.Add(OnUIIdle)
		h.queue = function(entry) table.insert(m_kQueuedPopups, entry) end
		h.waiting = function() return #m_kQueuedPopups end
	]],
	TechCivicCompletedPopup = [[
		local m_kPopupData = {}
		function Show(player)
			h.shown[#h.shown + 1] = player
			UIManager:QueuePopup(ContextPtr, 30)
		end
		function Close()
			m_kPopupData = {}
			UIManager:DequeuePopup(ContextPtr)
		end
		function OnClose() Close() end
		Events.LocalPlayerTurnEnd.Add(function() if GameConfiguration.IsHotseat() then Close() end end)
	]],
	BoostUnlockedPopup = [[
		local m_queuedBoosts, m_isOnQueue = {}, false
		function ShowBoost(queueEntry)
			h.shown[#h.shown + 1] = queueEntry.techIndex
			UIManager:QueuePopup(ContextPtr, 20, { DelayShow = true })
			m_isOnQueue = true
		end
		function ShowNextQueuedPopup()
			for i, entry in ipairs(m_queuedBoosts) do
				ShowBoost(m_queuedBoosts[i])
				table.remove(m_queuedBoosts, i)
				break
			end
		end
		function OnClose()
			UIManager:DequeuePopup(ContextPtr)
			m_isOnQueue = false
			ShowNextQueuedPopup()
		end
		h.queue = function(entry) table.insert(m_queuedBoosts, entry) end
		h.waiting = function() return #m_queuedBoosts end
	]],
	EraReviewPopup = [[
		function OnShow() UIManager:QueuePopup(ContextPtr, 40) end
		function OnClose() UIManager:DequeuePopup(ContextPtr) end
		function OnContinue()
			UIManager:DequeuePopup(ContextPtr)
			h.dedication = true
		end
	]],
}

-- One screen's context: the closer over the model, under a cover when `covered`.
local function screen(name, config)
	local h = { pid = 5, inQueue = false, hidden = true, covered = true, shown = {}, logs = {},
	            opened = 0, closed = 0, listeners = {}, ui = {} }
	local env = setmetatable({ CivvisControlConfig = config, ExposedMembers = {} }, { __index = _G })
	local manager = {}
	function manager:QueuePopup() h.inQueue, h.hidden = true, false end
	function manager:DequeuePopup() h.inQueue, h.hidden = false, true end
	function manager:IsInPopupQueue() return h.inQueue end
	env.UIManager = manager
	env.UI = { CanShowPopup = function() return not h.covered end, PlaySound = function() end }
	env.Game = { GetLocalPlayer = function() return h.pid end, GetCurrentGameTurn = function() return 33 end }
	env.GameConfiguration = { IsHotseat = function() return true end }
	-- Drawn under the cover, the context does not read as visible to its own script.
	env.ContextPtr = {
		GetID = function() return name end, IsHidden = function() return h.hidden end,
		IsVisible = function() return not h.hidden and not h.covered end,
		SetUpdate = function(_, f) h.tick = f end, SetShowHandler = function() end,
		ChangeParent = function() end, LookUpControl = function() return nil end,
	}
	env.Events = setmetatable({}, { __index = function(events, key)
		local event = { Add = function(f)
			h.listeners[key] = h.listeners[key] or {}
			table.insert(h.listeners[key], f)
		end }
		rawset(events, key, event)
		return event
	end })
	env.LuaEvents = setmetatable({
		HistoricMoments_Opened = function() h.opened = h.opened + 1 end,
		HistoricMoments_Closed = function() h.closed = h.closed + 1 end,
	}, { __index = function(events, key)
		local event = setmetatable({ Add = function() end }, { __call = function() end })
		rawset(events, key, event)
		return event
	end })
	env.Automation = { Log = function(line) h.logs[#h.logs + 1] = line end }
	env.Controls = {}
	env.h = h
	env.include = function(script)
		if script ~= name then return end
		local model = assert(loadstring(MODELS[name], name))
		setfenv(model, env)
		model()
	end
	local chunk = assert(loadfile(here .. "/CivvisControlAutoClose.lua"))
	setfenv(chunk, env)
	chunk()
	h.env = env
	h.fire = function(key, ...)
		for _, f in ipairs(h.listeners[key] or {}) do f(...) end
	end
	-- The person's turn begins: the local player is theirs, the cover still up.
	h.personTurn = function() h.pid = 0; h.fire("LocalPlayerTurnBegin") end
	h.kinds = function(kind)
		local out = {}
		for _, line in ipairs(h.logs) do
			if line:find('"kind":"' .. kind .. '"', 1, true) then out[#out + 1] = line end
		end
		return out
	end
	return h
end
local COVER = { Hotseat = true, HotseatSeats = { 1, 2, 3, 4, 5 }, HotseatHumanSeats = { 0 },
                HotseatStubSeats = {}, HotseatCover = true, HotseatAutoStart = true, RunTag = "hs" }

-- 1. The last CIVVIS seat's moment, its own close not taken under the cover:
-- closed with the shipped lines as the person's turn begins, said once.
do
	local h = screen("HistoricMoments", COVER)
	h.fire("NotificationActivated", 5, 7001, false)
	assert(h.inQueue and h.shown[1] == 5, "seat 5's moment is drawn under the cover")
	h.fire("LocalPlayerTurnEnd")
	assert(h.inQueue, "its own end-of-turn close asks IsVisible, and under the cover it is not")
	h.personTurn()
	assert(not h.inQueue and h.hidden, "gone before the person's turn is shown")
	assert(h.closed == 1, "and the launch bar is told it closed (HistoricMoments_Closed)")
	local said = h.kinds("hotseat_cover_left")
	assert(#said == 1 and said[1]:find('"screen":"HistoricMoments"', 1, true)
		and said[1]:find('"owner":5,"player":0,"closed":true', 1, true), said[1])
	h.fire("LocalPlayerTurnBegin")
	assert(#h.kinds("hotseat_cover_left") == 1, "nothing left to close a second time")
end
-- 2. One raised in the rollover, after the last seat's end of turn, while it is still local.
do
	local h = screen("HistoricMoments", COVER)
	h.fire("LocalPlayerTurnEnd")
	h.fire("NotificationActivated", 5, 7002, false)
	assert(h.inQueue)
	h.personTurn()
	assert(not h.inQueue, "a rollover moment of seat 5's is not the person's")
end
-- 3. The person's own moment is theirs: kept, whether or not a CIVVIS one came first.
do
	local h = screen("HistoricMoments", COVER)
	h.pid, h.covered = 0, false
	h.fire("NotificationActivated", 0, 7003, false)
	h.fire("LocalPlayerTurnBegin")
	assert(h.inQueue and #h.kinds("hotseat_cover_left") == 0, "the person's own moment stays")
	-- Seat 4's timeline up, then the person's own moment shown in the same context.
	h = screen("HistoricMoments", COVER)
	h.pid = 4
	h.fire("NotificationActivated", 4, 7004, false)
	h.pid, h.covered = 0, false
	h.fire("NotificationActivated", 0, 7005, false)
	h.fire("LocalPlayerTurnBegin")
	assert(h.inQueue and h.shown[2] == 0, "the last moment shown is the person's: kept")
end
-- 4. Another seat's moment kept in the screen's own list does not open on the person's turn.
do
	local h = screen("HistoricMoments", COVER)
	h.pid, h.covered = 0, false
	h.queue({ playerID = 3, momentID = 7006, showAnim = true })
	h.queue({ playerID = 0, momentID = 7007, showAnim = true })
	h.fire("UIIdle")
	assert(not h.inQueue and #h.shown == 0, "seat 3's queued moment is dropped, not shown")
	assert(h.kinds("hotseat_cover_left")[1]:find('"owner":3,"player":0,"dropped":true', 1, true))
	h.fire("UIIdle")
	assert(h.inQueue and h.shown[1] == 0, "the person's own queued moment opens")
end
-- 5. The other screens a CIVVIS turn leaves: each closed its own way, the person's kept.
do
	local tech = screen("TechCivicCompletedPopup", COVER)
	tech.env.Show(5)
	tech.personTurn()
	assert(not tech.inQueue and #tech.kinds("hotseat_cover_left") == 1)
	local mine = screen("TechCivicCompletedPopup", COVER)
	mine.pid = 0
	mine.env.Show(0)
	mine.fire("LocalPlayerTurnBegin")
	assert(mine.inQueue, "the person's own tech is theirs")
	-- Boosts: the one up and the two waiting in its list, none shown for the person.
	local boost = screen("BoostUnlockedPopup", COVER)
	boost.env.ShowBoost({ techIndex = 11 })
	boost.queue({ techIndex = 12 })
	boost.queue({ techIndex = 13 })
	boost.personTurn()
	assert(not boost.inQueue and boost.waiting() == 0 and #boost.shown == 1,
		"seat 5's boost and its queued boosts are gone, unshown: " .. #boost.shown)
	boost.env.ShowBoost({ techIndex = 14 })
	assert(boost.inQueue and #boost.shown == 2, "and the person's next boost shows as before")
	-- The era review is closed, never continued (a dedication for whoever is local).
	local review = screen("EraReviewPopup", COVER)
	review.env.OnShow()
	review.personTurn()
	assert(not review.inQueue and review.env.h.dedication == nil)
end
-- 6. Only in a cover game, and only the four screens; the ladder is as shipped.
do
	local SEATS = { 1, 2, 3, 4, 5 }
	for _, config in ipairs({ { RunTag = "solo", HotseatSeats = SEATS, HotseatHumanSeats = { 0 } },
			{ Hotseat = true, HotseatSeats = SEATS, HotseatHumanSeats = { 0 }, HotseatCover = false },
			{ Hotseat = true, HotseatSeats = SEATS, HotseatHumanSeats = { 0 }, HotseatCover = true,
			  HotseatCoverQueue = false } }) do
		local h = screen("HistoricMoments", config)
		h.covered = false
		h.fire("NotificationActivated", 5, 7008, false)
		assert(h.inQueue, "shown, as it ships")
		h.personTurn()
		assert(h.inQueue and #h.kinds("hotseat_cover_left") == 0, "no cover game: nothing of ours")
	end
	local closer = assert(io.open(here .. "/CivvisControlAutoClose.lua")):read("*a")
	local list = assert(closer:match("CivvisCoverQueue%.LEFT = (%b{})"))
	for _, name in ipairs({ "HistoricMoments", "EraReviewPopup", "BoostUnlockedPopup",
			"TechCivicCompletedPopup" }) do
		assert(list:find(name .. " = true", 1, true), name)
	end
	assert(not list:find("DiplomacyActionView", 1, true) and not list:find("EraCompletePopup", 1, true),
		"a leader screen and the new era's screen (shown after the person's own Start Turn) are not on it")
end

print("hotseat_left_behind_test: ok")
