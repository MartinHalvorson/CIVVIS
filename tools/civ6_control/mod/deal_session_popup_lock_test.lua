-- A popup queued behind a deal session's leader screen holds its event lock
-- until that screen closes, and the rival's verdict cannot be computed behind
-- it (live 2026-11-15: four asks waited out their 30 s behind
-- `RockBandMoviePopup`, two hung the rival's turn). The diplomacy view that is
-- up for the session asks the lock's owner to close; the owner runs its own
-- `Close()`. This loads the real shim once per context, with one shared event
-- bus, lock ledger and engine lock table, as the game does.
--
-- Run: lua5.1 tools/civ6_control/mod/deal_session_popup_lock_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."
local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", name, tostring(got)))
	end
end

-- One "game": the engine's held event ids, the cross-context tables, the log.
local function world()
	local w = { engine = {}, nextId = 100, logs = {}, exposed = {}, listeners = {} }
	w.bus = setmetatable({}, { __index = function(bus, event)
		local channel = setmetatable({ Add = function(fn)
			w.listeners[event] = w.listeners[event] or {}
			table.insert(w.listeners[event], fn)
		end }, { __call = function(_, ...)
			for _, fn in ipairs(w.listeners[event] or {}) do fn(...) end
		end })
		rawset(bus, event, channel)
		return channel
	end })
	function w.rows(kind, needle)
		local n = 0
		for _, line in ipairs(w.logs) do
			if line:find('"kind":"' .. kind .. '"', 1, true)
					and (needle == nil or line:find(needle, 1, true)) then n = n + 1 end
		end
		return n
	end
	function w.heldCount()
		local n = 0
		for _ in pairs(w.engine) do n = n + 1 end
		return n
	end
	return w
end

-- One UI context running the shim. `closes` counts the shipped Close() calls.
local function context(w, name, opts)
	opts = opts or {}
	local h = { hidden = opts.hidden ~= false and opts.hidden or false, closes = 0, name = name }
	if opts.hidden == true then h.hidden = true end
	local env = setmetatable({
		CivvisControlConfig = { AnnouncementSeconds = 0.05, DialogueSeconds = 0.25,
			RunTag = "popup-lock-test" },
		ExposedMembers = w.exposed, LuaEvents = w.bus,
	}, { __index = _G })
	env.include = function() end
	env.Game = { GetCurrentGameTurn = function() return 159 end, GetLocalPlayer = function() return 0 end }
	env.UI = {
		GetElapsedTime = function() return 0 end,
		ReferenceCurrentEvent = function()
			w.nextId = w.nextId + 1
			w.engine[w.nextId] = name
			return w.nextId
		end,
		ReleaseEventID = function(id) w.engine[id] = nil end,
	}
	env.Controls = { BlackFadeAnim = { IsStopped = function() return true end } }
	env.ContextPtr = {
		GetID = function() return name end, IsHidden = function() return h.hidden end,
		SetUpdate = function(_, f) h.tick = f end, SetShowHandler = function() end,
	}
	env.Automation = { Log = function(line) w.logs[#w.logs + 1] = line end }
	env.OnShow = function() end
	if opts.popup then
		-- The shipped popup: lock on its event, queued (hidden behind a leader
		-- screen), and `Close()` gives the lock back and dequeues.
		env.Close = function()
			h.closes = h.closes + 1
			if not opts.refusesToClose then
				env.UI.ReleaseEventID(h.eventID)
				h.eventID = nil
				h.hidden = true
			end
		end
		env.OnClose = function() env.Close() end
		h.fire = function() h.eventID = env.UI.ReferenceCurrentEvent() end
	else
		env.OnClose = function() h.closes = h.closes + 1 end
	end
	local chunk = assert(loadfile(here .. "/CivvisControlAutoClose.lua"))
	setfenv(chunk, env)
	chunk()
	h.env = env
	return h
end

local function run(view, seconds, step)
	for _ = 1, math.floor(seconds / step + 0.5) do view.tick(step) end
end

-- 1. The live case: the leader screen is up for a deal session, a concert
--    fires, its popup is queued hidden with the lock.
do
	local w = world()
	local leader = context(w, "DiplomacyActionView")
	local band = context(w, "RockBandMoviePopup", { popup = true, hidden = true })
	local wonder = context(w, "NaturalWonderPopup", { popup = true, hidden = true })
	w.bus.CivvisDealSession(4, true, 30)
	run(leader, 0.1, 0.02)
	check("a deal hold with no lock asks nothing", w.rows("autoclose_behind_deal"), 0)
	band.fire()
	check("the popup's lock is held in the engine", w.heldCount(), 1)
	check("…and named in the ledger", w.exposed.CivvisEventLocks.held[band.eventID].ctx, "RockBandMoviePopup")
	run(leader, 0.2, 0.02)
	check("inside the grace the popup is left alone", band.closes, 0)
	run(leader, 0.1, 0.02)
	check("past the grace its own Close() ran", band.closes, 1)
	check("…once", w.rows("autoclose_behind_deal", '"holder":"RockBandMoviePopup"'), 1)
	check("…the engine lock is back", w.heldCount(), 0)
	check("…the ledger is empty", w.exposed.CivvisEventLocks.count, 0)
	check("…and the close is recorded as one behind a deal",
		w.rows("autoclose", '"behind_deal":true,"released":true'), 1)
	check("the other popup, holding nothing, was not touched", wonder.closes, 0)
	check("the leader screen itself was not closed by this", leader.closes, 0)
	run(leader, 5, 0.02)
	check("nothing is asked again once the lock is gone", w.rows("autoclose_behind_deal"), 1)
	check("the deal hold is still running", w.rows("deal_session_timeout"), 0)
	-- A second concert in the same hold is its own lock.
	band.hidden = true
	band.fire()
	run(leader, 0.4, 0.02)
	check("a second lock in the same hold is released too", band.closes, 2)
	check("…leaving the engine free", w.heldCount(), 0)
end

-- 2. No deal session: the leader screen does not police other contexts.
do
	local w = world()
	local leader = context(w, "DiplomacyActionView")
	local band = context(w, "RockBandMoviePopup", { popup = true, hidden = true })
	band.fire()
	run(leader, 0.2, 0.02)
	check("without a deal hold a held lock is not asked for", w.rows("autoclose_behind_deal"), 0)
	check("…and the popup was not closed from outside", band.closes, 0)
end

-- 3. The diplomacy views' own locks are the session, not an obstacle.
do
	local w = world()
	local leader = context(w, "DiplomacyActionView")
	local deal = context(w, "DiplomacyDealView")
	w.bus.CivvisDealSession(4, true, 30)
	local own = leader.env.UI.ReferenceCurrentEvent()
	local dealLock = deal.env.UI.ReferenceCurrentEvent()
	run(leader, 2, 0.02)
	run(deal, 2, 0.02)
	check("a diplomacy view's own lock is never asked for", w.rows("autoclose_behind_deal"), 0)
	check("…and stays held", w.engine[own] ~= nil and w.engine[dealLock] ~= nil, true)
end

-- 3b. The wonder movie keeps its own clock: it is never closed unseen.
do
	local w = world()
	local leader = context(w, "DiplomacyActionView")
	local wonder = context(w, "WonderBuiltPopup", { popup = true, hidden = true })
	w.bus.CivvisDealSession(4, true, 30)
	wonder.fire()
	run(leader, 2, 0.02)
	check("a wonder popup's lock is not asked for", w.rows("autoclose_behind_deal"), 0)
	w.bus.CivvisCloseBehindDeal("WonderBuiltPopup", wonder.eventID)
	check("…and an ask that names it is refused", wonder.closes, 0)
	check("…so its lock stays its own", w.heldCount(), 1)
end

-- 3c. Two popups hold locks: each owner is asked for its own, once, and a
--     lock taken while the first is being closed is not lost.
do
	local w = world()
	local leader = context(w, "DiplomacyActionView")
	local band = context(w, "RockBandMoviePopup", { popup = true, hidden = true })
	local natural = context(w, "NaturalWonderPopup", { popup = true, hidden = true })
	local disaster = context(w, "NaturalDisasterPopup", { popup = true, hidden = true })
	w.bus.CivvisDealSession(4, true, 30)
	band.fire()
	natural.fire()
	-- The freed game dispatches the next event from inside the first close.
	local shippedClose = band.env.Close
	band.env.Close = function() shippedClose(); disaster.fire() end
	band.env.OnClose = function() band.env.Close() end
	run(leader, 0.3, 0.02)
	check("both held locks were given back by their owners", band.closes + natural.closes, 2)
	check("…leaving only the lock taken during the close", w.heldCount(), 1)
	run(leader, 0.3, 0.02)
	check("…which is given back after its own grace", disaster.closes, 1)
	check("…and the engine is free", w.heldCount(), 0)
	check("three locks, three asks", w.rows("autoclose_behind_deal"), 3)
end

-- 4. Both diplomacy views are up for a session: one ask, one close.
do
	local w = world()
	local leader = context(w, "DiplomacyActionView")
	local deal = context(w, "DiplomacyDealView")
	local band = context(w, "RockBandMoviePopup", { popup = true, hidden = true })
	w.bus.CivvisDealSession(4, true, 30)
	band.fire()
	for _ = 1, 30 do leader.tick(0.02); deal.tick(0.02) end
	check("with both views watching the popup closes once", band.closes, 1)
	check("…and the lock is back", w.heldCount(), 0)
end

-- 5. The operator's Pause: a held game's screens are the operator's.
do
	local w = world()
	local leader = context(w, "DiplomacyActionView")
	local band = context(w, "RockBandMoviePopup", { popup = true, hidden = true })
	w.bus.CivvisDealSession(4, true, 30)
	band.fire()
	w.exposed.CivvisOperatorHold = true
	w.bus.CivvisCloseBehindDeal("RockBandMoviePopup", band.eventID)
	check("an ask during the operator's hold closes nothing", band.closes, 0)
	w.exposed.CivvisOperatorHold = false
	w.bus.CivvisCloseBehindDeal("RockBandMoviePopup", band.eventID)
	check("the same ask after the resume does", band.closes, 1)
end

-- 6. The ask names a lock; a context that no longer holds it does nothing.
do
	local w = world()
	local band = context(w, "RockBandMoviePopup", { popup = true, hidden = true })
	local wonder = context(w, "NaturalWonderPopup", { popup = true, hidden = true })
	band.fire()
	local stale = band.eventID
	band.env.Close()
	check("(the popup closed itself)", band.closes, 1)
	w.bus.CivvisCloseBehindDeal("RockBandMoviePopup", stale)
	check("an ask for a lock already given back is ignored", band.closes, 1)
	wonder.fire()
	w.bus.CivvisCloseBehindDeal("RockBandMoviePopup", wonder.eventID)
	check("an ask naming another context's lock is ignored by both",
		band.closes + wonder.closes, 1)
	w.bus.CivvisCloseBehindDeal("EndGameMenu", wonder.eventID)
	check("…and so is one for a context that is not there", wonder.closes, 0)
end

-- 7. A popup whose Close() does not give the lock back is asked once, said
--    so, and the deal hold then runs out as it always did.
do
	local w = world()
	local leader = context(w, "DiplomacyActionView")
	local band = context(w, "RockBandMoviePopup", { popup = true, hidden = true, refusesToClose = true })
	w.bus.CivvisDealSession(4, true, 2)
	band.fire()
	run(leader, 1, 0.02)
	check("a lock that stays held is asked for once, not every frame", band.closes, 1)
	check("…and the row says it was not released",
		w.rows("autoclose", '"behind_deal":true,"released":false'), 1)
	run(leader, 1.5, 0.02)
	check("the hold still ends on its own clock", w.rows("deal_session_timeout"), 1)
end

-- 8. Fail-open: a broken ledger or a missing event bus changes nothing.
do
	local w = world()
	local leader = context(w, "DiplomacyActionView")
	w.bus.CivvisDealSession(4, true, 30)
	w.exposed.CivvisEventLocks = { held = "broken", count = 0 }
	local ok = pcall(run, leader, 1, 0.02)
	check("a broken ledger does not stop the leader screen's tick", ok, true)
	w.exposed.CivvisEventLocks = { held = { [7] = "not a table", [8] = { ctx = 5 } }, count = 2 }
	ok = pcall(run, leader, 1, 0.02)
	check("…nor do malformed entries", ok, true)
	check("…and nothing is asked for them", w.rows("autoclose_behind_deal"), 0)
end

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("\nall deal-session popup-lock checks passed")
