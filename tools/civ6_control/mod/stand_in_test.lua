-- The hotseat test stand-in (`CivvisSeat.standIn`, CivvisControlAgent.lua):
-- the `--auto-human` stub plays its turn to the end, and says in one record
-- what stands when it cannot. The worlds below are built from the stub's own
-- records of the runs that wedged on its turn:
--   * manual-live-1 / manual-cover-1 turn 5: Scout 196608, sent on a
--     multi-turn `probe` on turn 4, refused SKIP_TURN, FORTIFY, ALERT and SLEEP;
--     `ENDTURN_BLOCKING_UNITS`; Warrior 131073 on alert (not ready).
--   * smoke-6p-20261128T0244Zb turn 2: `NO_ENDTURN_BLOCKING`, one end-turn
--     request while a kept Historic Moments screen was up, then nothing.
-- Run: lua5.1 tools/civ6_control/mod/stand_in_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."
local function read(name)
	local handle = assert(io.open(here .. "/" .. name, "r"))
	local text = handle:read("*a")
	handle:close()
	return text
end
local agent = read("CivvisControlAgent.lua")
local first = assert(agent:find("CivvisSeat.standIn = { key = nil };", 1, true),
	"the agent defines the stand-in")
local last = assert(agent:find("\nfunction Initialize()", first, true),
	"the stand-in sits before Initialize")
local block = agent:sub(first, last)

local UNITS, NONE, RESEARCH, ENVOY = 101, 0, 102, 103
local NAMES = { [UNITS] = "ENDTURN_BLOCKING_UNITS", [NONE] = "NO_ENDTURN_BLOCKING",
	[RESEARCH] = "ENDTURN_BLOCKING_RESEARCH", [ENVOY] = "ENDTURN_BLOCKING_GIVE_INFLUENCE_TOKEN" }
local OPS = { UNITOPERATION_SKIP_TURN = "op:skip", UNITOPERATION_FORTIFY = "op:fortify",
	UNITOPERATION_ALERT = "op:alert", UNITOPERATION_SLEEP = "op:sleep" }

-- One game, one stub seat. `w` is the engine as the stand-in sees it.
local function world(cfg, opts)
	opts = opts or {}
	local w = {
		pid = 0, turn = opts.turn or 5, active = true, paused = false, sent = false,
		now = 100, blocker = nil, events = {}, requests = {}, ticks = 0, dismissed = 0,
		answered = {}, considered = 0, cancels = 0, screens = opts.screens or {},
		lastRequest = nil, units = {}, ledger = opts.locks, errors = {},
	}
	function w.unit(spec)
		spec.ready = spec.ready ~= false
		w.units[#w.units + 1] = spec
		return spec
	end
	function w.anyReady()
		for _, u in ipairs(w.units) do if u.ready then return u end end
	end
	function w.currentBlocker()
		if w.blocker ~= nil then return w.blocker end
		return w.anyReady() and UNITS or NONE
	end
	-- `requestEndTurn` as the agent has it: false when the turn is already
	-- sent or a request went out inside the last quarter second; otherwise the
	-- request. The engine ends the turn only when nothing stands in its way:
	-- no screen up, no unit waiting for orders, no hard blocker.
	function w.request(turn, parameters)
		if w.sent then return false end
		if w.lastRequest ~= nil and w.now - w.lastRequest < 0.25 then return false end
		w.lastRequest = w.now
		w.requests[#w.requests + 1] = { turn = turn, reason = parameters and parameters.REASON, at = w.now }
		if #w.screens == 0 and w.anyReady() == nil and w.blocker == nil then
			w.active = false
		end
	end
	local env = {
		cfg = cfg,
		ipairs = ipairs, pairs = pairs, tonumber = tonumber, tostring = tostring, type = type,
		pcall = pcall, math = math, table = table, string = string,
		CivvisSeat = { seat = cfg.HotseatSeat },
		-- The agent's guard reports and carries on; every error is kept here
		-- and no scenario may raise one.
		CivvisK = { guard = function(fn)
			local ok, err = pcall(fn)
			if not ok then w.errors[#w.errors + 1] = tostring(err) end
		end },
		try = function(fn, fallback)
			local ok, value = pcall(fn)
			if ok and value ~= nil then return value end
			return fallback
		end,
		emit = function(kind, payload)
			payload = payload or {}
			payload.kind = kind
			w.events[#w.events + 1] = payload
		end,
		OP = OPS,
		CMD = { UNITCOMMAND_CANCEL = "cmd:cancel" },
		operate = function(unit, hash)
			if hash == nil or not unit.ready or unit.activity == "operation" then return false end
			unit.ready, unit.held = false, hash
			return true
		end,
		unitTypeName = function(unit) return unit.kind end,
		eachUnit = function(_, fn) for _, u in ipairs(w.units) do fn(u) end end,
		UnitManager = {
			CanStartCommand = function(unit, hash) return hash == "cmd:cancel" and unit.activity == "operation" end,
			-- Asynchronous, as the engine's is: the cancel lands on the next frame.
			RequestCommand = function(unit, hash)
				assert(hash == "cmd:cancel")
				unit.cancelPending = true
				w.cancels = w.cancels + 1
			end,
			CanStartOperation = function(unit)
				if unit.activity == "operation" then
					return false, { reasons = { "LOC_UNIT_BUSY_WITH_OPERATION" } }
				end
				return true, {}
			end,
			GetActivityType = function(unit) return unit.activity end,
			GetQueuedDestination = function(unit) return unit.queued and 1234 or nil end,
		},
		OperationResultsTypes = { ALL = "all" },
		UnitOperationResults = { FAILURE_REASONS = "reasons" },
		Locale = { Lookup = function(key) return "text:" .. key end },
		CivvisBoard = { activityName = function(kind) return kind end },
		ContextPtr = { LookUpControl = function(_, path)
			local name = path:match("^/InGame/(.*)$")
			local up = false
			for _, screen in ipairs(w.screens) do if screen == name then up = true end end
			return { IsHidden = function() return not up end }
		end },
		ExposedMembers = { CivvisEventLocks = w.ledger },
		EndTurnBlockingTypes = { NO_ENDTURN_BLOCKING = NONE },
		currentBlocker = function() return w.currentBlocker() end,
		blockerName = function(id) return NAMES[id] or tostring(id) end,
		UNIT_BLOCKERS = { ENDTURN_BLOCKING_UNITS = true, ENDTURN_BLOCKING_UNIT_NEEDS_ORDERS = true,
			ENDTURN_BLOCKING_STACKED_UNITS = true },
		answerBlocker = function(_, _, blocker, _, residual)
			w.answered[#w.answered + 1] = { blocker = NAMES[blocker], residual = residual }
			if blocker == RESEARCH then w.blocker = nil; return "TECH_POTTERY" end
			return nil
		end,
		dismissBlocker = function() w.dismissed = w.dismissed + 1; return true end,
		CivvisMarkEnvoyConsidered = function()
			w.considered = w.considered + 1
			if w.blocker == ENVOY then w.blocker = nil end
			return true
		end,
		CivvisQueue = { requestEndTurn = function(turn, parameters) return w.request(turn, parameters) end },
		UI = {
			HasSentTurnComplete = function() return w.sent end,
			CanEndTurn = function() return w.anyReady() == nil end,
			IsGameCoreBusy = function() return false end,
			IsProcessingMessages = function() return false end,
		},
		PlayerConfigurations = setmetatable({}, { __index = function()
			return { GetWantsPause = function() return w.paused end }
		end }),
		Game = { GetCurrentGameTurn = function() return w.turn end },
		CivvisClock = { now = function() return w.now end },
		finished = false, inTick = false, lastTurnSeen = w.turn,
		ensureStarted = function() end,
		-- The ordinary loop: it asks for the end of the turn as `tick` does
		-- (a plain request), and plays a new turn first.
		tick = function()
			w.ticks = w.ticks + 1
			w.env.lastTurnSeen = w.turn
			if w.currentBlocker() == NONE then w.request(w.turn) end
		end,
	}
	local player = {
		IsTurnActive = function() return w.active end,
		GetUnits = function() return { GetFirstReadyUnit = function() return w.anyReady() end } end,
	}
	env.localPlayer = function() return player, w.pid end
	w.env = env
	local chunk = assert(loadstring(block))
	setfenv(chunk, env)
	chunk()
	w.seat = env.CivvisSeat
	-- Units are tables with the engine's methods.
	local methods = {
		GetID = function(u) return u.id end, GetX = function(u) return u.x end,
		GetY = function(u) return u.y end, GetMovesRemaining = function(u) return u.moves end,
		GetMaxMoves = function(u) return u.max end,
		IsReadyToSelect = function(u) return u.ready end,
	}
	local plain = w.unit
	function w.unit(spec)
		setmetatable(spec, { __index = methods })
		return plain(spec)
	end
	-- One second of game: queued commands land, then the HUD's pulse.
	function w.second(source)
		w.now = w.now + 1
		for _, u in ipairs(w.units) do
			if u.cancelPending then u.cancelPending, u.activity, u.queued = nil, "awake", false end
		end
		w.seat.standInPulse(source or "TopPanel")
	end
	function w.clean()
		assert(#w.errors == 0, "the stand-in raised: " .. table.concat(w.errors, "; "))
	end
	function w.kinds(kind)
		local out = {}
		for _, e in ipairs(w.events) do if e.kind == kind then out[#out + 1] = e end end
		return out
	end
	return w
end

local STUB = { Hotseat = true, HotseatSeat = 0, CivvisDecides = false,
	HotseatSeats = { 1, 2, 3, 4, 5 }, HotseatHumanSeats = { 0 }, HotseatStubSeats = { 0 } }

-- 1. Only a test stub's context stands in. ----------------------------------
do
	local w = world(STUB)
	assert(w.seat.standsIn() == true, "the gated stub stands in")
	local plainStub = {}
	for k, v in pairs(STUB) do plainStub[k] = v end
	plainStub.HotseatHumanSeats = {}
	assert(world(plainStub).seat.standsIn() == true, "the plain `--auto-human stub` stands in")
	for label, change in pairs({
		civvis_seat = function(c) c.CivvisDecides = true end,
		civvis_seat_1 = function(c) c.CivvisDecides = true; c.HotseatSeat = 1 end,
		single_player = function(c) c.Hotseat = nil; c.HotseatSeat = nil end,
		no_seat = function(c) c.HotseatSeat = nil end,
		not_listed = function(c) c.HotseatStubSeats = {} end,
		other_stub = function(c) c.HotseatStubSeats = { 3 } end,
	}) do
		local c = {}
		for k, v in pairs(STUB) do c[k] = v end
		change(c)
		local other = world(c)
		assert(other.seat.standsIn() == false, label .. " must not stand in")
		other.unit({ id = 196608, kind = "UNIT_SCOUT", activity = "operation", queued = true })
		for _ = 1, 60 do other.second() end
		assert(other.ticks == 0 and #other.requests == 0 and #other.events == 0,
			label .. ": a context that does not stand in does nothing on the pulse")
	end
end

-- 2. smoke-6p t2: the first request went out under a screen; the next pulse
--    asks again once it is gone. ----------------------------------------------
do
	local w = world(STUB, { turn = 2, screens = { "HistoricMoments" } })
	w.request(2)                              -- the turn-begin pass, under the screen
	assert(w.active, "a screen up keeps the turn")
	w.second("HistoricMoments")               -- the screen's own pulse: still up
	assert(w.active and w.ticks == 1, "the stand-in ticks on a screen's pulse too")
	w.screens = {}                            -- the closer took it down
	w.second()
	assert(not w.active, "the next pulse ends the turn")
	assert(#w.kinds("stand_in_stuck") == 0, "a turn that ends needs no report")
	assert(#w.requests == 3 and w.requests[3].reason == nil, "with the loop's plain request")
	w.clean()
end

-- 3. manual-live-1 t5: a Scout that refuses every hold until its stale
--    operation is cancelled. ---------------------------------------------------
do
	local w = world(STUB)
	local scout = w.unit({ id = 196608, kind = "UNIT_SCOUT", x = 16, y = 27, moves = 3, max = 3,
		activity = "operation", queued = true })
	w.unit({ id = 131073, kind = "UNIT_WARRIOR", x = 16, y = 26, moves = 2, max = 2,
		activity = "sentry", ready = false })
	for _ = 1, 4 do w.second() end
	assert(w.active and #w.kinds("stand_in_stuck") == 0 and w.cancels == 0,
		"the first four seconds are the ordinary loop's")
	assert(w.ticks == 4)
	w.second()                                -- 4 s after the first pulse: the first step
	assert(w.ticks == 4, "a step replaces the pulse's tick, so its forced request is not deferred")
	assert(w.cancels == 1, "the refusing Scout's operation is cancelled")
	local stuck = w.kinds("stand_in_stuck")
	assert(#stuck == 1, "the first step reports")
	local r = stuck[1]
	assert(r.blocker == "ENDTURN_BLOCKING_UNITS" and r.turn == 5 and r.seat == 0 and r.step == 1, r.blocker)
	assert(r.answered == "held:0+cancelled:1", tostring(r.answered))
	assert(r.dismissed == true and r.requested == true and r.forced == 1)
	assert(w.requests[#w.requests].reason == "UserForced", "the shipped SHIFT+ENTER request")
	assert(#r.refusing == 1, "one unit still refuses")
	local u = r.refusing[1]
	assert(u.id == 196608 and u.kind == "UNIT_SCOUT" and u.activity == "operation"
		and u.queued == true and u.cancelled == true and u.moves == 3 and u.max == 3,
		"the refusing unit is described")
	assert(u.why == "text:LOC_UNIT_BUSY_WITH_OPERATION", "in the engine's own words: " .. tostring(u.why))
	assert(w.kinds("stand_in_stuck_flush")[1] ~= nil, "a record behind it delivers it")
	assert(w.active, "the forced request is refused while the Scout still waits")
	-- Ordinary loop between steps, then the next step holds the Scout.
	for _ = 1, 2 do w.second() end
	assert(w.ticks == 6 and w.active)
	w.second()
	assert(scout.held == "op:skip", "the cancelled Scout takes SKIP_TURN: " .. tostring(scout.held))
	assert(not w.active, "and the forced request ends the turn")
	assert(#w.kinds("stand_in_stuck") == 1, "reported once in 30 s")
	w.clean()
end

-- 4. A unit that never holds: reported every 30 s, stepped every 3 s, and the
--    steps are bounded. ------------------------------------------------------------
do
	local c = {}
	for k, v in pairs(STUB) do c[k] = v end
	c.StandInMaxSteps = 25
	local w = world(c)
	w.unit({ id = 262146, kind = "UNIT_BUILDER", moves = 0, max = 2, activity = "operation", queued = false })
	w.env.UnitManager.CanStartCommand = function() return false end   -- not even Cancel
	for _ = 1, 65 do w.second() end
	local stuck = w.kinds("stand_in_stuck")
	assert(#stuck == 3, "reported at 4, 34 and 64 s: " .. #stuck)
	assert(stuck[1].seconds == 4 and stuck[2].seconds == 34 and stuck[3].seconds == 64,
		tostring(stuck[2].seconds))
	assert(stuck[3].refusing[1].cancelled == false and stuck[3].answered == "held:0+cancelled:0")
	assert(stuck[1].step == 1 and stuck[2].step == 11 and stuck[3].step == 21, tostring(stuck[2].step))
	for _ = 1, 60 do w.second() end
	local forced = 0
	for _, req in ipairs(w.requests) do if req.reason == "UserForced" then forced = forced + 1 end end
	assert(forced == 25, "steps stop at StandInMaxSteps: " .. forced)
	w.clean()
	-- The game's ready query jams on a unit that takes nothing, as it does
	-- (`orderUnits`: "GetFirstReadyUnit keeps returning this same unit
	-- forever"); the roster reaches the Warrior behind it.
	local j = world(STUB)
	local jammed = j.unit({ id = 262146, kind = "UNIT_BUILDER", moves = 0, max = 2, activity = "operation" })
	local warrior = j.unit({ id = 131073, kind = "UNIT_WARRIOR", moves = 2, max = 2, activity = "awake" })
	j.env.UnitManager.CanStartCommand = function() return false end
	j.anyReady = function()
		if jammed.ready then return jammed end
		if warrior.ready then return warrior end
	end
	for _ = 1, 5 do j.second() end
	assert(warrior.held == "op:fortify" or warrior.held == "op:skip", "the unit behind a jam is held")
	assert(j.kinds("stand_in_stuck")[1].answered == "held:1+cancelled:0")
	j.clean()
end

-- 5. Other blockers: the ladder's answer, the envoy prompt marked considered,
--    and the record names what is up on screen and the event locks held. -----
do
	local w = world(STUB, { screens = { "TechCivicCompletedPopup" },
		locks = { count = 1, held = { [7] = { ctx = "TechCivicCompletedPopup", turn = 5, at = 99 } } } })
	w.blocker = RESEARCH
	for _ = 1, 5 do w.second() end
	assert(w.answered[1] and w.answered[1].blocker == "ENDTURN_BLOCKING_RESEARCH"
		and w.answered[1].residual == true, "a hard blocker gets the ladder's answer")
	local r = w.kinds("stand_in_stuck")[1]
	assert(r.answered == "TECH_POTTERY" and r.blocker == "ENDTURN_BLOCKING_RESEARCH")
	assert(r.visible[1] == "TechCivicCompletedPopup", "the screen up is named")
	assert(r.held_locks[1].ctx == "TechCivicCompletedPopup" and r.held_locks[1].id == 7, "and its lock")
	assert(w.active, "a screen up still holds the turn")
	w.screens = {}
	for _ = 1, 3 do w.second() end
	assert(not w.active, "the next step ends it")

	local e = world(STUB)
	e.blocker = ENVOY
	for _ = 1, 5 do e.second() end
	assert(e.considered == 1 and #e.answered == 0, "an envoy prompt is marked considered, not spent")
	assert(not e.active)
	w.clean(); e.clean()
end

-- 6. Not before the turn is played, not past its end, never while sent. -------
do
	local w = world(STUB)
	w.env.lastTurnSeen = 4                    -- the movement wait holds the first pass
	w.env.tick = function() w.ticks = w.ticks + 1 end
	w.unit({ id = 196608, kind = "UNIT_SCOUT", activity = "operation" })
	for _ = 1, 10 do w.second() end
	assert(w.ticks == 10 and #w.kinds("stand_in_stuck") == 0 and w.cancels == 0,
		"an unplayed turn is only ticked")
	local s = world(STUB)
	s.sent = true
	s.unit({ id = 196608, kind = "UNIT_SCOUT", activity = "operation" })
	for _ = 1, 5 do s.second() end
	local r = s.kinds("stand_in_stuck")[1]
	assert(r and r.sent == true and r.requested == false, "a sent turn is reported, not asked again")
	local p = world(STUB)
	p.active = false
	for _ = 1, 10 do p.second() end
	assert(p.ticks == 0 and #p.events == 0, "between turns nothing runs")
	-- A new turn starts its own clock.
	local n = world(STUB)
	n.unit({ id = 196608, kind = "UNIT_SCOUT", activity = "operation" })
	for _ = 1, 5 do n.second() end
	assert(#n.kinds("stand_in_stuck") == 1)
	n.turn = 6; n.env.lastTurnSeen = 6
	for _ = 1, 4 do n.second() end
	assert(#n.kinds("stand_in_stuck") == 1, "turn 6 gets its own grace")
	n.second()
	assert(#n.kinds("stand_in_stuck") == 2 and n.kinds("stand_in_stuck")[2].turn == 6)
	w.clean(); s.clean(); p.clean(); n.clean()
end

-- 7. The agent wires it: the pulse through the seat gate, and the stub waits
--    for its units' movement like a CIVVIS seat. -------------------------------
assert(agent:find("LuaEvents.CivvisControlPulse.Add(CivvisSeat.gate(CivvisSeat.standInPulse));", 1, true),
	"the stand-in rides the HUD pulse, gated to its own turn")
assert(agent:find("if (cfg.CivvisDecides or CivvisSeat.standsIn())\n\t\t\t\tand not (cfg.IsolatedActionProbes == true", 1, true),
	"the stand-in's turn opens once movement is restored")

print("all stand-in checks passed")
