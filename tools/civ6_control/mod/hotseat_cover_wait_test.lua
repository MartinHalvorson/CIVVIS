-- The wait bar's countdown (CivvisHotseatCover.lua, HOW LONG) replayed on the
-- waits of a real game: the person's live 6-player game of 2026-11-27
-- (hotseat-20261128T031456Z, five CIVVIS seats). Each wait below is what the
-- cover saw: when each seat's turn began and ended (the seats' own
-- `end_turn_wait` UI clock, seat 1 from the harness's turns.jsonl) and the
-- whole wait it recorded (`hotseat_cover` `waited`).
--
-- The same waits go through the cover itself, frame by frame, and through the
-- estimate it replaces (the person's last four whole waits, 4:3:2:1, a tenth
-- over, fixed at the wait's start: main before this test was added), and the
-- test prints what the person would have read: how far off the time left was
-- at the wait's start, at every seat's begin and end and on every frame; how
-- long "Any moment now" showed with more than 1.5 s still to go; and the bar.
-- Also run faster (CIVVIS seats in a fifth of a second), slower (half a
-- minute) and with a slow turn every few waits: the countdown must stay sane.
-- Run: lua5.1 tools/civ6_control/mod/hotseat_cover_wait_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."
local handle = assert(io.open(here .. "/CivvisHotseatCover.lua", "r"))
local source = handle:read("*a")
handle:close()

-- hotseat-20261128T031456Z: { seat, from the stretch before to its begin (s),
-- begin to end (s) } per seat; `tail` from the last seat's end to the lift.
local LIVE = {
	{ turn = 1, waited = 9.62, tail = 1.15, seats = { { 1, 0.10, 2.60 }, { 2, 0.05, 1.48 }, { 3, 0.05, 1.05 }, { 4, 0.05, 1.63 }, { 5, 0.05, 1.40 } } },
	{ turn = 2, waited = 6.30, tail = 1.02, seats = { { 1, 0.10, 1.00 }, { 2, 0.05, 1.07 }, { 3, 0.05, 0.88 }, { 4, 0.05, 1.07 }, { 5, 0.05, 0.97 } } },
	{ turn = 3, waited = 7.30, tail = 1.45, seats = { { 1, 0.10, 0.50 }, { 2, 0.05, 1.58 }, { 3, 0.05, 1.48 }, { 4, 0.05, 1.00 }, { 5, 0.05, 0.98 } } },
	{ turn = 4, waited = 7.23, tail = 1.27, seats = { { 1, 0.10, 1.10 }, { 2, 0.05, 1.65 }, { 3, 0.05, 0.90 }, { 4, 0.05, 0.97 }, { 5, 0.05, 1.05 } } },
	{ turn = 5, waited = 7.80, tail = 1.16, seats = { { 1, 0.10, 1.10 }, { 2, 0.05, 1.12 }, { 3, 0.05, 2.08 }, { 4, 0.05, 0.99 }, { 5, 0.05, 1.05 } } },
	{ turn = 6, waited = 7.51, tail = 0.97, seats = { { 1, 0.10, 1.00 }, { 2, 0.05, 1.17 }, { 3, 0.05, 1.38 }, { 4, 0.05, 1.57 }, { 5, 0.05, 1.12 } } },
	{ turn = 7, waited = 8.02, tail = 1.50, seats = { { 1, 0.10, 0.50 }, { 2, 0.05, 1.59 }, { 3, 0.05, 1.98 }, { 4, 0.05, 1.17 }, { 5, 0.05, 0.98 } } },
	{ turn = 8, waited = 8.62, tail = 1.14, seats = { { 1, 0.10, 1.60 }, { 2, 0.05, 1.66 }, { 3, 0.05, 1.02 }, { 4, 0.05, 1.87 }, { 5, 0.05, 1.03 } } },
	{ turn = 9, waited = 7.55, tail = 1.37, seats = { { 1, 0.10, 1.00 }, { 2, 0.05, 1.17 }, { 3, 0.05, 1.26 }, { 4, 0.05, 1.07 }, { 5, 0.05, 1.38 } } },
	{ turn = 10, waited = 8.11, tail = 1.09, seats = { { 1, 0.10, 1.10 }, { 2, 0.05, 1.05 }, { 3, 0.05, 2.00 }, { 4, 0.05, 0.97 }, { 5, 0.05, 1.60 } } },
	{ turn = 11, waited = 8.29, tail = 1.74, seats = { { 1, 0.10, 1.10 }, { 2, 0.05, 1.47 }, { 3, 0.05, 1.11 }, { 4, 0.05, 1.59 }, { 5, 0.05, 0.98 } } },
	{ turn = 12, waited = 9.15, tail = 1.49, seats = { { 1, 0.10, 1.10 }, { 2, 0.05, 1.98 }, { 3, 0.05, 1.62 }, { 4, 0.05, 1.66 }, { 5, 0.05, 1.00 } } },
	{ turn = 13, waited = 11.21, tail = 1.61, seats = { { 1, 0.10, 1.60 }, { 2, 0.05, 1.43 }, { 3, 0.05, 2.47 }, { 4, 0.05, 1.10 }, { 5, 0.05, 2.70 } } },
	{ turn = 14, waited = 9.65, tail = 2.48, seats = { { 1, 0.10, 1.60 }, { 2, 0.05, 1.12 }, { 3, 0.05, 1.41 }, { 4, 0.05, 1.19 }, { 5, 0.05, 1.55 } } },
	{ turn = 15, waited = 17.47, tail = 2.76, seats = { { 1, 0.10, 1.10 }, { 2, 0.05, 1.66 }, { 3, 0.05, 9.17 }, { 4, 0.05, 1.15 }, { 5, 0.05, 1.33 } } },
	{ turn = 16, waited = 11.36, tail = 2.71, seats = { { 1, 0.10, 1.10 }, { 2, 0.05, 1.70 }, { 3, 0.05, 1.43 }, { 4, 0.05, 2.04 }, { 5, 0.05, 2.08 } } },
}

-- A longer game: the 2-seat self-play rehearsal selfplay-duel-20261127T0808Z,
-- 118 turns, each seat's turn (turns.jsonl, 0.5 s steps) as two CIVVIS seats
-- of a three-seat table. It had no person and no cover, so the moment before
-- each seat is 0.05 s and the last stretch 1.5 s, growing a second per 100
-- turns (as the live game's rollover grew).
local SELFPLAY_TURNS = {
	{ 2.2, 1.1 }, { 1.1, 1.1 }, { 1.6, 1.6 }, { 1.1, 1.0 }, { 1.6, 1.1 }, { 1.0, 1.1 }, { 1.6, 1.0 },
	{ 1.0, 2.1 }, { 1.1, 2.1 }, { 2.1, 1.6 }, { 2.6, 2.1 }, { 2.1, 1.6 }, { 1.1, 1.1 }, { 1.1, 1.6 },
	{ 1.6, 1.6 }, { 1.1, 1.6 }, { 2.1, 0.5 }, { 1.6, 2.6 }, { 1.6, 1.0 }, { 2.6, 1.6 }, { 2.1, 1.0 },
	{ 2.6, 1.0 }, { 2.1, 2.1 }, { 2.1, 1.6 }, { 2.2, 1.1 }, { 2.2, 0.5 }, { 2.2, 1.6 }, { 1.6, 2.2 },
	{ 1.6, 2.1 }, { 2.2, 1.6 }, { 1.2, 1.1 }, { 1.1, 1.6 }, { 1.1, 1.6 }, { 1.6, 1.1 }, { 1.1, 1.7 },
	{ 2.1, 1.1 }, { 2.2, 1.6 }, { 2.2, 2.1 }, { 1.6, 1.6 }, { 2.7, 1.6 }, { 2.7, 1.1 }, { 3.2, 1.7 },
	{ 2.7, 1.6 }, { 3.3, 1.7 }, { 1.7, 1.6 }, { 2.3, 1.1 }, { 1.7, 1.6 }, { 1.1, 2.2 }, { 1.6, 2.1 },
	{ 2.1, 10.5 }, { 2.1, 1.6 }, { 2.8, 3.7 }, { 1.6, 3.7 }, { 1.1, 1.6 }, { 1.1, 2.1 }, { 1.7, 2.2 },
	{ 2.8, 1.1 }, { 1.6, 1.1 }, { 1.6, 1.6 }, { 1.6, 3.2 }, { 1.1, 1.1 }, { 1.6, 4.3 }, { 3.3, 2.1 },
	{ 2.2, 3.2 }, { 1.6, 2.2 }, { 2.2, 3.9 }, { 1.7, 3.9 }, { 1.6, 1.7 }, { 2.2, 2.3 }, { 1.7, 3.3 },
	{ 2.3, 1.6 }, { 2.2, 3.2 }, { 2.2, 3.8 }, { 2.8, 2.2 }, { 2.2, 1.7 }, { 2.2, 1.1 }, { 1.6, 2.2 },
	{ 2.2, 1.6 }, { 16.4, 1.7 }, { 10.5, 2.2 }, { 1.1, 1.1 }, { 3.9, 1.7 }, { 3.9, 2.3 },
	{ 2.8, 2.9 }, { 2.3, 1.1 }, { 9.9, 2.3 }, { 2.7, 2.2 }, { 1.7, 2.2 }, { 3.4, 2.2 }, { 3.4, 1.7 },
	{ 3.9, 1.7 }, { 3.4, 2.3 }, { 3.4, 2.2 }, { 1.7, 1.7 }, { 2.2, 2.8 }, { 3.4, 2.2 }, { 3.4, 2.2 },
	{ 1.7, 2.9 }, { 5.7, 2.8 }, { 3.4, 2.8 }, { 1.1, 1.1 }, { 2.2, 2.7 }, { 2.2, 2.7 }, { 3.3, 1.6 },
	{ 3.8, 1.7 }, { 2.8, 2.2 }, { 2.2, 1.6 }, { 2.8, 2.2 }, { 2.2, 2.2 }, { 2.8, 2.8 }, { 1.7, 2.7 },
	{ 2.2, 2.7 }, { 2.8, 4.4 }, { 1.6, 2.8 }, { 1.7, 2.8 }, { 2.2, 3.9 }, { 2.2, 4.6 }, { 3.4, 4.6 },
}
local SELFPLAY = {}
for turn, seats in ipairs(SELFPLAY_TURNS) do
	local tail = 1.5 + turn / 100
	SELFPLAY[turn] = { turn = turn, tail = tail, waited = 0.15 + seats[1] + seats[2] + tail,
		seats = { { 1, 0.10, seats[1] }, { 2, 0.05, seats[2] } } }
end

local DT = 0.05
local BAR = 320

-- The cover in a game where seat 0 is the person and seats 1-5 are CIVVIS's,
-- driven by its own event handlers and frame update, the UI clock ours.
local function load(seats)
	local world = { pid = 0, turn = 1, active = {}, paused = {}, clock = 0 }
	local bar = {}
	local handlers = {}
	local function event(name)
		return setmetatable({ Add = function(f) handlers[name] = f end }, { __call = function() end })
	end
	local env = {
		CivvisControlConfig = { Hotseat = true, RunTag = "replay", HotseatSeats = seats,
			HotseatHumanSeats = { 0 }, HotseatCover = true, HotseatAutoStart = true,
			HotseatCoverTimer = true },
		include = function() end,
		tonumber = tonumber, tostring = tostring, type = type, pcall = pcall,
		ipairs = ipairs, pairs = pairs, string = string, table = table, math = math,
		setmetatable = setmetatable,
		Automation = { Log = function() end },
		PopupPriority = { Utmost = 100, HotSeatBackground = 101, PausePanel = 102 },
		UIManager = { QueuePopup = function() end, DequeuePopup = function() end,
			IsInPopupQueue = function() return false end },
		Controls = {
			Wait = { SetHide = function(_, hide) bar.hidden = hide end },
			WaitFill = { SetSizeX = function(_, x) bar.width = x end,
			             SetOffsetX = function(_, x) bar.offset = x end },
			WaitText = { SetText = function(_, text) bar.text = text end },
		},
		ExposedMembers = { CivvisSeatBeat = {} },
		Game = { GetLocalPlayer = function() return world.pid end,
		         GetCurrentGameTurn = function() return world.turn end },
		Players = setmetatable({}, { __index = function(_, pid)
			return { IsTurnActive = function() return world.active[pid] == true end }
		end }),
		PlayerConfigurations = setmetatable({}, { __index = function(_, pid)
			return { GetWantsPause = function() return world.paused[pid] == true end,
			         GetHotseatPassword = function() return "" end }
		end }),
		UI = { GetElapsedTime = function() return world.clock end },
		Events = {}, LuaEvents = {},
		ContextPtr = {
			LookUpControl = function() return nil end,
			SetShutdown = function() end,
			SetInputHandler = function() end,
			SetUpdate = function(_, f) handlers.update = f end,
		},
	}
	for _, name in ipairs({ "LocalPlayerTurnEnd", "LocalPlayerTurnBegin", "LoadScreenClose",
			"TeamVictory", "UserRequestClose" }) do
		env.Events[name] = event(name)
	end
	for _, name in ipairs({ "PlayerChange_Close", "EndGameMenu_Shown", "EndGameMenu_OneMoreTurn",
			"PlayerChange_OpenInGameOptionsMenu", "CivvisHotseatStartTurn" }) do
		env.LuaEvents[name] = event(name)
	end
	local chunk = assert(loadstring(source))
	setfenv(chunk, env)
	chunk()
	return env.CivvisHotseatCover, world, bar, handlers
end

-- The estimate this replaces, as it was: one number for the whole wait,
-- fixed at its start, and the bar's ease past 85%.
local Old = {}
function Old.new() return { waits = {} } end
function Old.estimate(o)
	if #o.waits == 0 then return nil end
	local sum, weight = 0, 0
	for back = 0, math.min(#o.waits, 4) - 1 do
		local w = 4 - back
		sum, weight = sum + o.waits[#o.waits - back] * w, weight + w
	end
	return math.max(sum / weight * 1.1, 0.5)
end
function Old.share(elapsed, estimate)
	local x = elapsed / estimate
	if x <= 0.85 then return x end
	return math.min(0.85 + 0.15 * (1 - math.exp(-(x - 0.85) / 0.6)), 0.99)
end

local function metrics()
	-- `stalls`: times the bar moved less than 1% of its track in 2 s.
	return { start = {}, change = {}, frames = {}, early = 0, back = 0, jumps = 0, lifted = {},
	         stalls = 0 }
end
local function mean(list)
	local sum = 0
	for _, x in ipairs(list) do sum = sum + x end
	return #list > 0 and sum / #list or 0 / 0
end

-- Every wait of `waits` through the cover and through the old estimate.
local function replay(waits, seats)
	local cover, world, bar, handlers = load(seats or { 1, 2, 3, 4, 5 })
	local old = Old.new()
	local new, was = metrics(), metrics()
	-- The person's turn 1 is on when the game starts.
	world.pid, world.active[0] = 0, true
	local function frame(t, W, o)
		world.clock = world.clock + DT
		handlers.update(DT)
		local truth = W - t
		-- the cover
		local left = cover.remaining(cover.wait)
		if left ~= nil then new.frames[#new.frames + 1] = math.abs(left - truth) end
		if bar.text == "Any moment now" and truth > 1.5 then new.early = new.early + DT end
		-- (the bar's fill, once there is a count; before it, a sweep)
		if left ~= nil and o.width ~= nil then
			if bar.width < o.width then new.back = new.back + 1 end
			if bar.width - o.width > 0.1 * BAR then new.jumps = new.jumps + 1 end
		end
		o.width = left ~= nil and bar.width or nil
		o.trail[#o.trail + 1] = o.width or false
		local before = o.trail[#o.trail - 2 / DT]
		if o.width and before and o.width - before < 0.01 * BAR then new.stalls = new.stalls + 1 end
		-- the old estimate
		if o.estimate ~= nil then
			was.frames[#was.frames + 1] = math.abs(math.max(o.estimate - t, 0) - truth)
			if o.estimate - t <= 0.5 and truth > 1.5 then was.early = was.early + DT end
			local width = Old.share(t, o.estimate) * BAR
			if o.oldWidth ~= nil and width - o.oldWidth > 0.1 * BAR then was.jumps = was.jumps + 1 end
			o.oldWidth = width
			o.oldTrail[#o.oldTrail + 1] = width
			local before = o.oldTrail[#o.oldTrail - 2 / DT]
			if before and width - before < 0.01 * BAR then was.stalls = was.stalls + 1 end
		end
	end
	for _, w in ipairs(waits) do
		local o = { estimate = Old.estimate(old), trail = {}, oldTrail = {} }
		world.turn, world.active[0] = w.turn, false
		world.pid = 0
		handlers.LocalPlayerTurnEnd()
		local left = cover.remaining(cover.wait)
		if left ~= nil then new.start[#new.start + 1] = math.abs(left - w.waited) end
		if o.estimate ~= nil then was.start[#was.start + 1] = math.abs(o.estimate - w.waited) end
		local t = 0
		local function run(seconds)
			local stop = t + seconds
			while t + DT <= stop + 1e-9 do
				t = t + DT
				frame(t, w.waited, o)
			end
		end
		local function changed()
			local l = cover.remaining(cover.wait)
			if l ~= nil then new.change[#new.change + 1] = math.abs(l - (w.waited - t)) end
			if o.estimate ~= nil then
				was.change[#was.change + 1] = math.abs(math.max(o.estimate - t, 0) - (w.waited - t))
			end
		end
		for _, s in ipairs(w.seats) do
			run(s[2])
			world.pid, world.active[s[1]] = s[1], true
			handlers.LocalPlayerTurnBegin()
			changed()
			run(s[3])
			world.active[s[1]] = false
			handlers.LocalPlayerTurnEnd()
			changed()
		end
		run(w.waited - t)
		new.lifted[#new.lifted + 1] = (o.width or 0) / BAR
		if o.oldWidth then was.lifted[#was.lifted + 1] = o.oldWidth / BAR end
		world.pid, world.turn, world.active[0], world.paused[0] = 0, w.turn + 1, true, true
		handlers.LocalPlayerTurnBegin()
		world.paused[0] = false
		handlers.PlayerChange_Close(0)
		old.waits[#old.waits + 1] = w.waited
	end
	return new, was
end

local function line(name, m)
	return string.format("%-28s start %5.2f s (%2d)  seat changes %5.2f s  frames %5.2f s  "
		.. "'any moment' early %5.1f s  bar back %d, jumps>10%% %d, stalled %.0f s, at the lift %.2f",
		name, mean(m.start), #m.start, mean(m.change), mean(m.frames), m.early, m.back, m.jumps,
		m.stalls * DT, mean(m.lifted))
end

-- The waits, changed: every seat's turn and the moments between them times
-- `seats`, the last stretch times `tail`, and every `every`th wait one seat's
-- turn `slow` times longer.
local function varied(waits, seats, tail, every, slow)
	local out = {}
	for i, w in ipairs(waits) do
		local copy, total = { turn = w.turn, seats = {} }, 0
		local hit = every and i % every == 0 and ((i % 5) + 1) or nil
		for _, s in ipairs(w.seats) do
			local took = s[3] * seats * ((s[1] == hit) and slow or 1)
			copy.seats[#copy.seats + 1] = { s[1], s[2] * seats, took }
			total = total + s[2] * seats + took
		end
		copy.tail = w.tail * tail
		copy.waited = total + copy.tail
		out[#out + 1] = copy
	end
	return out
end

local function check(name, waits, rules, seats)
	local new, was = replay(waits, seats)
	print(line(name .. ", before", was))
	print(line(name .. ", now", new))
	assert(new.back == 0, name .. ": the bar went back " .. new.back .. " times")
	assert(new.jumps == 0, name .. ": the bar jumped " .. new.jumps .. " times")
	assert(new.early <= rules.early, name .. ": 'Any moment now' with time to go: " .. new.early)
	assert(mean(new.change) < mean(was.change), name .. ": no better at the seat changes")
	assert(mean(new.frames) < mean(was.frames), name .. ": no better frame by frame")
	assert(mean(new.lifted) > 0.9, name .. ": the bar is not near the end at the lift")
	if rules.start then
		assert(mean(new.start) <= mean(was.start) + rules.start, name .. ": worse at the start")
	end
	return new, was
end

-- The live game as it was played.
check("live game", LIVE, { early = 0.5, start = 0.1 })
-- And with CIVVIS seats five times faster, a few times slower with a slow
-- turn every fourth wait, and as played with a slow turn every fifth.
check("seats x0.2", varied(LIVE, 0.2, 0.5), { early = 0.5 })
check("seats x6, slow 1 in 4", varied(LIVE, 6, 2, 4, 5), { early = 3 })
check("slow turn 1 in 5", varied(LIVE, 1, 1, 5, 6), { early = 0.5, start = 0 })
-- And a long game whose turns grow.
check("self-play, 118 turns", SELFPLAY, { early = 0.5, start = 0 }, { 1, 2 })

print("hotseat_cover_wait_test: ok")
