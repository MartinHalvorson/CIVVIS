pcall(function() include("CivvisControlMapView"); end);
-- Keep the shipped HUD and its expansion chain, then borrow its visible UI
-- clock. Popup clocks stop when hidden; the script-only agent has no clock.
-- TopPanel_Expansion2.lua:5 includes Expansion1, which includes TopPanel.
pcall(function() include("TopPanel_Expansion2"); end);
if type(LateInitialize) ~= "function" then
	pcall(function() include("TopPanel_Expansion1"); end);
end
if type(LateInitialize) ~= "function" then include("TopPanel"); end

local cfg = CivvisControlConfig or {};

-- >>> CivvisHarness: the same block in CivvisControlAgent.lua,
-- CivvisControlAutoClose.lua and CivvisControlHeartbeat.lua (harness_gate_test.lua).
-- ★★ THE PERSON-FACING INSTALL DOES NOTHING WITHOUT A LIVE HARNESS.
-- `RequireHarness` is set only by `tools/civ6_hotseat.py --manual-lobby`, the
-- install a person who is not this project's operator may leave in place and
-- play every other game with. Two rules keep it stock for them:
--   * outside a hotseat game it is dormant (`dormant`): the shipped screen or
--     HUD and nothing of ours, and no agent context registers anything;
--   * in a hotseat game it acts and writes only while the harness that
--     installed it is live (`live`): a `harness` row under `HarnessRun` in the
--     orders database whose `beat` (Unix seconds, rewritten every 2 s) is at
--     most `HarnessFreshSeconds` old by SQLite's own clock. A stopped harness
--     deletes the row and a killed one goes stale; either way every seat is a
--     person's again, as Civ VI ships it.
-- Read at most every 3 s of `Automation.GetTime()`, never per tick: a
-- `DB.Query` per game-core tick starves the log flush. Unset, `live` is true,
-- `dormant` false, and nothing here runs.
CivvisHarness = { ok = false, calls = 0 };
function CivvisHarness.hotseat()
	local on = false;
	pcall(function() on = GameConfiguration.IsHotseat() == true; end);
	return on;
end
function CivvisHarness.dormant()
	return cfg.RequireHarness == true and not CivvisHarness.hotseat();
end
function CivvisHarness.read()
	if not CivvisHarness.hotseat() then return false; end
	local path = string.gsub(tostring(cfg.OrdersDb or ""), "['\\]", "");
	local run = string.gsub(tostring(cfg.HarnessRun or ""), "['\\]", "");
	if path == "" or run == "" then return false; end
	if not CivvisHarness.attached then
		-- One connection serves every context: a second ATTACH of the alias
		-- fails, and the read below goes through whichever context's took.
		pcall(function() DB.Query("ATTACH DATABASE '" .. path .. "' AS civvis"); end);
	end
	local fresh = tonumber(cfg.HarnessFreshSeconds) or 20;
	local live = false;
	local rows = DB.Query("SELECT CAST(strftime('%s','now') AS INTEGER) - beat AS age"
			.. " FROM civvis.harness WHERE run = '" .. run .. "'");
	-- Attached once a read went through; until then each reading tries again.
	CivvisHarness.attached = true;
	for _, row in ipairs(rows) do
		local age = tonumber(row.age);
		if age ~= nil and age >= -5 and age <= fresh then live = true; end
	end
	return live;
end
function CivvisHarness.live()
	if cfg.RequireHarness ~= true then return true; end
	local h = CivvisHarness;
	if h.checking then return h.ok; end
	local now = nil;
	pcall(function() now = Automation.GetTime(); end);
	h.calls = h.calls + 1;
	if h.at ~= nil then
		if type(now) == "number" and now >= h.at and now - h.at < 3 then return h.ok; end
		if type(now) ~= "number" and h.calls % 200 ~= 0 then return h.ok; end
	end
	h.checking, h.at = true, type(now) == "number" and now or -1;
	local ok, live = pcall(h.read);
	h.ok, h.checking = ok and live == true, false;
	return h.ok;
end
-- Whether the local player's turn is CIVVIS's: one of the installed seats (or
-- the test stub) while the harness is live. Every other turn is a person's.
function CivvisHarness.mine(pid)
	if not CivvisHarness.live() then return false; end
	for _, list in ipairs({ cfg.HotseatSeats or {}, cfg.HotseatStubSeats or {} }) do
		for _, seat in ipairs(list) do
			if tonumber(seat) == pid then return true; end
		end
	end
	return false;
end
-- <<< CivvisHarness

-- The person-facing install outside a hotseat game: the shipped HUD, as
-- included above, and nothing of ours after it.
if CivvisHarness.dormant() then return; end

-- ★ AN IN-GAME VSYNC A/B, OFF UNLESS `VSyncABTurns` IS SET. The rival-AI
-- phase is ~3 s of a 7-9 s turn, and during it the Game Core thread sits in
-- its message wait 82-91% of the time while this app's frame loop runs at the
-- 120 Hz panel's vsync (G63, `sample` at end turn). Whether frames pace that
-- phase is the question; across games the answer drowns in map and war
-- differences, so this alternates VSync in blocks of `VSyncABTurns` turns
-- (even blocks on, odd blocks off) inside ONE game, and writes the frame
-- rate each turn ran at. The switch is the shipped options screen's own
-- (Options.lua:871-874, `SetGraphicsOption("Video", "VSync", v)`, then
-- :254 `Options.ApplyGraphicsOptions()`), which changes the running engine
-- without saving; `civ6_setup.VERIFICATION_OPTIONS` pins the launch value so
-- a block's setting written back on exit cannot leak into the next game.
CivvisVSyncAB = { frames = 0, seconds = 0 };
function CivvisVSyncAB.log(kind, fields)
	-- The person-facing install writes nothing without a live harness.
	if not CivvisHarness.live() then return; end
	pcall(function()
		Automation.Log("CIVVISJSON " .. string.format(
			'{"ctx":"heartbeat","kind":"%s","run":"%s",%s}\n',
			kind, tostring(cfg.RunTag or "unset"), fields));
	end);
end
function CivvisVSyncAB.frame(dt)
	CivvisVSyncAB.frames = CivvisVSyncAB.frames + 1;
	CivvisVSyncAB.seconds = CivvisVSyncAB.seconds + dt;
end
function CivvisVSyncAB.pulse(blockTurns)
	local ab = CivvisVSyncAB;
	local turn = tonumber(Game.GetCurrentGameTurn());
	if turn == nil then return; end
	if ab.turn ~= nil and turn ~= ab.turn then
		if ab.seconds > 0 then
			ab.log("frame_rate", string.format('"turn":%d,"vsync":%s,"fps":%.1f,"seconds":%.1f',
				ab.turn, tostring(ab.vsync), ab.frames / ab.seconds, ab.seconds));
		end
		ab.frames, ab.seconds = 0, 0;
	end
	ab.turn = turn;
	local want = (math.floor(turn / blockTurns) % 2 == 1) and 0 or 1;
	if want == ab.vsync then return; end
	-- Recorded before the call, so a throwing API is tried once per block,
	-- not once a second.
	ab.vsync = want;
	local applied = pcall(function()
		Options.SetGraphicsOption("Video", "VSync", want);
		Options.ApplyGraphicsOptions();
	end);
	local readBack = nil;
	pcall(function() readBack = Options.GetGraphicsOption("Video", "VSync"); end);
	ab.log("vsync_ab", string.format('"turn":%d,"vsync":%d,"applied":%s,"read_back":%s',
		turn, want, tostring(applied), tostring(tonumber(readBack) or "null")));
end

-- ★ REAL SECONDS. The engine's debug `timescale` speeds this context's frame
-- deltas up with the UI clock, and every pulse the agent counts (the stall
-- probe's 8 ticks among them) is paced from here. So each delta is divided by
-- the scale the agent shares in `ExposedMembers.CivvisTimeScale` (1 when it
-- is missing or nonsense), and this context says what it read
-- (`CivvisClockAck`) and how its frame deltas compare with the UI clock over
-- 10 s windows (`CivvisFrameClock`); the agent reverts the timescale if
-- either disagrees (`CivvisQueue.checkTimescaleClock`).
-- ⚠ Only CONSECUTIVE frames are compared: a frame counts when the UI clock
-- moved 0 < dui <= 0.5 s since this context's previous frame. A leader screen
-- hides TopPanel and stops this SetUpdate while the UI clock runs on (G98
-- 04:12:15-20, a first meeting), and a long hitch caps its frame delta; both
-- read as missing frame time (G98: 0.48, the very signature of unscaled
-- deltas) though nothing is wrong. Deltas the timescale does not scale still
-- read ~0.5 in every ordinary frame.
CivvisFrameClock = { raw = 0, ui = 0, scale = 1 };
function CivvisFrameClock.scaleNow()
	local s = nil;
	pcall(function() s = tonumber(ExposedMembers.CivvisTimeScale); end);
	if s == nil or s ~= s or s < 1 or s > 8 then return 1; end
	return s;
end
function CivvisFrameClock.frame(raw)
	local fc = CivvisFrameClock;
	local scale = fc.scaleNow();
	local ui = nil;
	pcall(function() ui = UI.GetElapsedTime(); end);
	if type(ui) ~= "number" then return raw / scale; end
	if scale ~= fc.scale then
		fc.scale, fc.raw, fc.ui = scale, 0, 0;
	elseif type(fc.last) == "number" and ui - fc.last > 0 and ui - fc.last <= 0.5 then
		fc.raw = fc.raw + raw;
		fc.ui = fc.ui + (ui - fc.last);
		if fc.ui >= 10 then
			local ratio = math.floor(fc.raw / fc.ui * 100 + 0.5) / 100;
			pcall(function()
				ExposedMembers.CivvisFrameClock = { scale = scale, ratio = ratio, at = ui };
			end);
			fc.raw, fc.ui = 0, 0;
		end
	end
	fc.last = ui;
	pcall(function()
		local ack = ExposedMembers.CivvisClockAck;
		ack.scale.Heartbeat = scale;
		ack.at.Heartbeat = ui;
	end);
	return raw / scale;
end

-- RECORD-ONLY: how long this frame loop stops. G115 (civvis-20261005T091120Z)
-- went silent 18 times for 2.5-5.4 s, 13 of them inside our own turn. At t20
-- a strike's 30 grace ticks then landed in the same instant as the next board,
-- so the UI thread had stopped, not the relay; the same hosts' screen
-- captures stall for exactly 5 s while systemstatusd spins. The engine caps a
-- long frame's `dt`, so the wall clock decides: two or more whole seconds of
-- `Automation.GetTime()` between consecutive frames is a stop of over a
-- second. `ui` is the UI clock's jump across it and `dt` the frame's own
-- delta, so a hidden TopPanel (a leader screen stops this SetUpdate while the
-- UI clock runs on) reads apart from a frozen loop.
CivvisUiGap = {};
function CivvisUiGap.frame(dt, uiBefore)
	local wall = nil;
	pcall(function() wall = Automation.GetTime(); end);
	if type(wall) ~= "number" then return; end
	local g = CivvisUiGap;
	if type(g.wall) == "number" and wall - g.wall >= 2 then
		local ui = CivvisFrameClock.last;
		local turn = nil;
		pcall(function() turn = tonumber(Game.GetCurrentGameTurn()); end);
		CivvisVSyncAB.log("ui_gap", string.format('"wall":%.0f,"ui":%.2f,"dt":%.3f,"turn":%d',
			wall - g.wall,
			(type(ui) == "number" and type(uiBefore) == "number") and (ui - uiBefore) or -1,
			tonumber(dt) or -1, turn or -1));
	end
	g.wall = wall;
end

-- ★★★ A CIVVIS HOTSEAT SEAT WHOSE TURN IS UP AND NOTHING IS PLAYING IT.
--
-- Tournament 6, 2026-11-21, twice in two games: the map on a CIVVIS seat's
-- turn, NEXT TURN lit, and the seat's controller silent -- 12 minutes at
-- civvis-t6-20261121T131243Z t61 (the World Congress phase gives a seat a
-- second turn inside the same game turn, which the controller had already
-- booked as played; its own record said `ai_phase_stall`, `active_players:
-- [1]`, on its own seat) and 7 minutes at civvis-t6-20261121T135836Z t180
-- (the first turn after a save was loaded: a forced end turn, 84
-- `turn_retry_settled`, then not one record). The outside harness can only
-- click NEXT TURN with Civ VI in front and nobody at the Mac, so both tables
-- sat until a watchdog killed them or somebody looked.
--
-- The controller cannot see this from inside: every one of its entry points
-- goes through `CivvisSeat.gate`, which is shut while the hand-over's pause
-- stands (`PlayerConfigurations[pid]:GetWantsPause()`, set by the shipped
-- Base/Assets/UI/Popups/PlayerChange.lua:229 `SetWantsPause(bNewPause)` and
-- released only by its Start Turn, :54 `OnOk` -> :57 `SetPause(false)`), and
-- a game waiting on input publishes no game-core events to tick it. Hotseat
-- hides the pause banner too (Panels/PausePanel.lua:77), so a pause left
-- standing with no hand-over on screen looks exactly like a game waiting for
-- NEXT TURN.
--
-- So the HUD watches the seat instead, from the two clocks that kept running
-- through both stalls: this context's frame update, and the shipped clock
-- label's own two-second timer (TopPanel.lua:516
-- `Controls.TimeCallback:RegisterEndCallback( OnRefreshTimeTick )`, which was
-- still repainting the time through the t180 stall). The seat's controller
-- stamps `ExposedMembers.CivvisSeatBeat[seat]` on every pass
-- (`CivvisSeat.beat`); while the seat's turn is active and that stamp does
-- not move:
--
--   * the hand-over's pause standing WITHOUT its Start Turn dialog on screen
--     for `SeatPauseGraceSeconds` (6) is released the way the dialog itself
--     would (`SetWantsPause(false)` and `Network.BroadcastPlayerInfo()`,
--     PlayerChange.lua:229-230) -> `hotseat_pause_released`;
--   * `SeatQuietSeconds` (45) of a quiet controller is reported with what
--     the engine says about the turn (`seat_quiet`: paused, turn complete
--     sent, the first end-turn blocker, whether the hand-over is up), and
--     the turn is ended with the shipped SHIFT+ENTER request
--     (ActionPanel.lua:1099 `UI.RequestAction(ActionTypes.ACTION_ENDTURN,
--     { REASON = "UserForced" } )`) -> `seat_quiet_end_turn`. Today's 4,000
--     seat turns took 5-8 s each; a seat idle 45 s on its own active turn is
--     not thinking.
--
-- Only seconds this watch itself saw pass are counted (at most 3 a reading),
-- so a leader screen that hides the HUD for a minute is not a quiet minute.
-- A person's seat (`HotseatHumanSeats`), a game held by the operator
-- (`CivvisOperatorHold`), a finished game and every single-player run are
-- left alone: the watch arms only when `cfg.Hotseat` is set.
CivvisSeatWatch = { quiet = 0, forces = 0, over = false };
function CivvisSeatWatch.now()
	local t = nil;
	pcall(function() t = Automation.GetTime(); end);
	if type(t) ~= "number" then pcall(function() t = os.time(); end); end
	if type(t) ~= "number" or t ~= t then return nil; end
	return t;
end
function CivvisSeatWatch.listed(list, pid)
	if type(list) ~= "table" then return false; end
	for _, seat in ipairs(list) do
		if tonumber(seat) == pid then return true; end
	end
	return false;
end
function CivvisSeatWatch.reset()
	local w = CivvisSeatWatch;
	w.key, w.prev, w.lastBeat, w.pausedFor = nil, nil, nil, nil;
	w.quiet, w.forces, w.reported, w.forced = 0, 0, nil, nil;
end
-- What the engine says about the seat's turn; every read guarded, a failed
-- one left nil (and written as null).
function CivvisSeatWatch.read(pid)
	local s = { player = pid };
	pcall(function() s.turn = Game.GetCurrentGameTurn(); end);
	pcall(function() s.active = Players[pid]:IsTurnActive() == true; end);
	pcall(function() s.paused = PlayerConfigurations[pid]:GetWantsPause() == true; end);
	pcall(function() s.sent = UI.HasSentTurnComplete() == true; end);
	pcall(function() s.can_end = UI.CanEndTurn() == true; end);
	pcall(function() s.busy = UI.IsGameCoreBusy() == true; end);
	pcall(function()
		local blocker = NotificationManager.GetFirstEndTurnBlocking(pid);
		if blocker ~= nil and blocker ~= EndTurnBlockingTypes.NO_ENDTURN_BLOCKING then
			s.blocker = tostring(blocker);
			for name, value in pairs(EndTurnBlockingTypes) do
				if value == blocker then s.blocker = tostring(name); end
			end
		end
	end);
	pcall(function()
		s.handover_hidden = ContextPtr:LookUpControl("/InGame/PlayerChange"):IsHidden() == true;
	end);
	pcall(function()
		s.dialog_hidden = ContextPtr:LookUpControl("/InGame/PlayerChange/PopupAlphaIn"):IsHidden() == true;
	end);
	pcall(function()
		s.end_game = ContextPtr:LookUpControl("/InGame/EndGameMenu"):IsHidden() == false;
	end);
	pcall(function() s.held = ExposedMembers.CivvisOperatorHold == true; end);
	return s;
end
function CivvisSeatWatch.field(value)
	if type(value) == "boolean" then return tostring(value); end
	if type(value) == "number" and value == value then return string.format("%.14g", value); end
	if type(value) == "string" then
		return '"' .. value:gsub('[%c"\\]', "_") .. '"';
	end
	return "null";
end
function CivvisSeatWatch.report(kind, s, extra)
	-- `t` first: it dates the record, and keeps the event bridge from taking
	-- a later turn's identical report for one it has already copied.
	local parts = { string.format('"t":%s', CivvisSeatWatch.field(CivvisSeatWatch.now())) };
	for _, key in ipairs({ "turn", "player", "active", "paused", "sent", "can_end", "busy",
			"blocker", "handover_hidden", "dialog_hidden" }) do
		parts[#parts + 1] = string.format('"%s":%s', key, CivvisSeatWatch.field(s[key]));
	end
	for _, pair in ipairs(extra or {}) do
		parts[#parts + 1] = string.format('"%s":%s', pair[1], CivvisSeatWatch.field(pair[2]));
	end
	CivvisVSyncAB.log(kind, table.concat(parts, ","));
	-- The game writes its latest Automation.Log record only when the next one
	-- is logged, and a stalled game logs nothing else: this delivers it.
	CivvisVSyncAB.log("seat_watch_flush", string.format('"turn":%s', CivvisSeatWatch.field(s.turn)));
end
function CivvisSeatWatch.beat(source)
	local w = CivvisSeatWatch;
	if w.over then return; end
	local now = w.now();
	if now == nil or w.checked == now then return; end
	w.checked = now;
	local pid = -1;
	pcall(function() pid = Game.GetLocalPlayer(); end);
	if type(pid) ~= "number" or pid < 0 or not w.listed(cfg.HotseatSeats, pid)
			or w.listed(cfg.HotseatHumanSeats, pid)
			or (cfg.RequireHarness == true and not CivvisHarness.mine(pid)) then
		w.reset();
		return;
	end
	local s = w.read(pid);
	if s.active ~= true or s.end_game == true or s.held == true then
		w.reset();
		return;
	end
	local key = pid .. ":" .. tostring(s.turn);
	if w.key ~= key then
		w.reset();
		w.key = key;
	end
	-- Seconds this watch saw pass since its last reading, never a gap it slept through.
	local dt = (type(w.prev) == "number" and now > w.prev) and math.min(now - w.prev, 3) or 0;
	w.prev = now;
	if s.paused == true then
		-- The Start Turn dialog is up: the hand-over owns the pause and
		-- CivvisControlAutoClose presses it (0.3 s, then every 2 s). A screen
		-- that could not be read is taken as up: the pause is released only
		-- when the hand-over, or its dialog, is known to be hidden.
		if not (s.handover_hidden == true or s.dialog_hidden == true) then
			w.pausedFor, w.quiet = nil, 0;
			return;
		end
		w.pausedFor = (w.pausedFor or 0) + dt;
		w.quiet = 0;
		if w.pausedFor >= (tonumber(cfg.SeatPauseGraceSeconds) or 6) then
			local waited = w.pausedFor;
			w.pausedFor = nil;
			local released = pcall(function()
				PlayerConfigurations[pid]:SetWantsPause(false);
				Network.BroadcastPlayerInfo();
			end);
			local still = nil;
			pcall(function() still = PlayerConfigurations[pid]:GetWantsPause() == true; end);
			w.report("hotseat_pause_released", s, {
				{ "waited", waited }, { "released", released }, { "still_paused", still },
				{ "source", source } });
		end
		return;
	end
	w.pausedFor = nil;
	local beat = nil;
	pcall(function() beat = ExposedMembers.CivvisSeatBeat[pid]; end);
	if beat ~= w.lastBeat then
		w.lastBeat, w.quiet = beat, 0;
		return;
	end
	w.quiet = w.quiet + dt;
	if w.quiet < (tonumber(cfg.SeatQuietSeconds) or 45) then return; end
	local forceNow = type(beat) == "number" and s.sent ~= true
		and w.forces < (tonumber(cfg.SeatQuietMaxForces) or 6)
		and (w.forced == nil or now - w.forced >= (tonumber(cfg.SeatQuietForceEvery) or 20));
	if w.reported == nil or now - w.reported >= 60 then
		w.reported = now;
		w.report("seat_quiet", s, {
			{ "quiet", w.quiet }, { "controller_ran", type(beat) == "number" },
			{ "forces", w.forces }, { "source", source } });
	end
	-- A seat no controller ever ran on has nothing of ours to end, and a turn
	-- already sent complete is the engine's to finish.
	if not forceNow then return; end
	w.forced, w.forces = now, w.forces + 1;
	local asked = pcall(function()
		UI.RequestAction(ActionTypes.ACTION_ENDTURN, { REASON = "UserForced" });
	end);
	w.report("seat_quiet_end_turn", s, {
		{ "quiet", w.quiet }, { "asked", asked }, { "forces", w.forces }, { "source", source } });
end
function CivvisSeatWatch.arm()
	-- The shipped clock label's timer, kept: LateInitialize registers whatever
	-- `OnRefreshTimeTick` names when the context initializes, after this file.
	local stockTick = OnRefreshTimeTick;
	if type(stockTick) == "function" then
		OnRefreshTimeTick = function(...)
			pcall(CivvisSeatWatch.beat, "clock");
			return stockTick(...);
		end;
	end
	pcall(function()
		Events.TeamVictory.Add(function() CivvisSeatWatch.over = true; end);
	end);
	CivvisVSyncAB.log("seat_watch_armed", string.format('"clock":%s,"quiet_seconds":%s',
		tostring(type(stockTick) == "function"),
		CivvisSeatWatch.field(tonumber(cfg.SeatQuietSeconds) or 45)));
end
-- ★ THE LOBBY THE PERSON MADE, READ BACK ONCE A GAME (`RequireHarness` only).
-- `tools/civ6_hotseat.py --manual-lobby` leaves the Hot Seat setup to the
-- person and learns what the game is from this record: each participating
-- full civ (StagingRoom.lua:1122, `GetCivilizationLevelTypeID() ==
-- CivilizationLevelTypes.CIVILIZATION_LEVEL_FULL_CIV`), whether its slot is a
-- person's (`SlotStatus.SS_TAKEN`, :1397) or Civ VI's AI (`SS_COMPUTER`,
-- :880), its own difficulty (hotseat difficulty is per player:
-- InGameTopOptionsMenu.lua:437-438, `GetHandicapTypeID()` into
-- `GameInfo.Difficulties`), and the map, size and speed (:442). Written on the
-- first HUD pulse of a started game while the harness is live, then a second
-- line so the game writes the first one out at once.
CivvisLobby = { sent = false };
function CivvisLobby.typeOf(rows, hash, column)
	local name = nil;
	pcall(function()
		local row = rows[hash];
		if row ~= nil then name = row[column]; end
	end);
	return name;
end
function CivvisLobby.player(id)
	local f = CivvisSeatWatch.field;
	local pc = PlayerConfigurations[id];
	if pc == nil then return nil; end
	local full = false;
	pcall(function()
		full = pc:GetCivilizationLevelTypeID() == CivilizationLevelTypes.CIVILIZATION_LEVEL_FULL_CIV;
	end);
	if not full then return nil; end
	local slot = "other";
	pcall(function()
		local status = pc:GetSlotStatus();
		if status == SlotStatus.SS_TAKEN then slot = "human";
		elseif status == SlotStatus.SS_COMPUTER then slot = "computer"; end
	end);
	if slot == "other" then return nil; end
	local human, alive, name, civ, leader, handicap = nil, nil, nil, nil, nil, nil;
	pcall(function() human = pc:IsHuman() == true; end);
	pcall(function() alive = Players[id]:IsAlive() == true; end);
	pcall(function() name = pc:GetPlayerName(); end);
	pcall(function() civ = pc:GetCivilizationTypeName(); end);
	pcall(function() leader = pc:GetLeaderTypeName(); end);
	pcall(function() handicap = pc:GetHandicapTypeID(); end);
	return string.format(
		'{"id":%s,"slot":%s,"human":%s,"alive":%s,"name":%s,"civ":%s,"leader":%s,"difficulty":%s}',
		f(id), f(slot), f(human), f(alive), f(name), f(civ), f(leader),
		f(CivvisLobby.typeOf(GameInfo.Difficulties, handicap, "DifficultyType")));
end
function CivvisLobby.players()
	local ids = nil;
	pcall(function() ids = GameConfiguration.GetParticipatingPlayerIDs(); end);
	if type(ids) ~= "table" or #ids == 0 then
		ids = {};
		for id = 0, 63 do ids[#ids + 1] = id; end
	end
	local out = {};
	for _, id in ipairs(ids) do
		local row = nil;
		pcall(function() row = CivvisLobby.player(id); end);
		if row ~= nil then out[#out + 1] = row; end
	end
	return out;
end
function CivvisLobby.pulse()
	if CivvisLobby.sent or cfg.RequireHarness ~= true or not CivvisHarness.live() then return; end
	local turn = nil;
	pcall(function() turn = tonumber(Game.GetCurrentGameTurn()); end);
	if turn == nil or turn < 1 then return; end
	CivvisLobby.sent = true;
	local f = CivvisSeatWatch.field;
	local pid, script, size, speed, ruleset, maxTurns = nil, nil, nil, nil, nil, nil;
	pcall(function() pid = Game.GetLocalPlayer(); end);
	pcall(function() script = MapConfiguration.GetScript(); end);
	pcall(function() size = MapConfiguration.GetMapSize(); end);
	pcall(function() speed = GameConfiguration.GetGameSpeedType(); end);
	pcall(function() ruleset = GameConfiguration.GetRuleSet(); end);
	pcall(function() maxTurns = Game.GetMaxGameTurns(); end);
	CivvisVSyncAB.log("hotseat_lobby", string.format(
		'"t":%s,"turn":%d,"local_player":%s,"hotseat":%s,"map":%s,"size":%s,"speed":%s,'
		.. '"ruleset":%s,"max_turns":%s,"players":[%s]',
		f(CivvisSeatWatch.now()), turn, f(pid), f(CivvisHarness.hotseat()), f(script),
		f(CivvisLobby.typeOf(GameInfo.Maps, size, "MapSizeType")),
		f(CivvisLobby.typeOf(GameInfo.GameSpeeds, speed, "GameSpeedType")),
		f(type(ruleset) == "string" and ruleset or nil), f(maxTurns),
		table.concat(CivvisLobby.players(), ",")));
	CivvisVSyncAB.log("hotseat_lobby_flush", string.format('"turn":%d', turn));
end

local seatWatch = cfg.Play ~= false and cfg.CivvisDecides and cfg.Hotseat == true;
if seatWatch then CivvisSeatWatch.arm(); end

if cfg.Play ~= false and cfg.CivvisDecides then
	local elapsed = 0;
	local abTurns = math.floor(tonumber(cfg.VSyncABTurns) or 0);
	-- The agent's landed-orders peek on this clock, which keeps running while
	-- the game core publishes nothing (`CivvisQueue.onPeekPulse`). The agent
	-- bounds the query itself; this only offers it a chance each interval.
	local peekElapsed = 0;
	local peekEvery = math.max(0.02, tonumber(cfg.OrdersPeekSeconds) or 0.05);
	ContextPtr:SetUpdate(function(dt)
		local uiBefore = CivvisFrameClock.last;
		local delta = CivvisFrameClock.frame(math.max(0, tonumber(dt) or 0));
		pcall(CivvisUiGap.frame, dt, uiBefore);
		if abTurns > 0 then CivvisVSyncAB.frame(delta); end
		peekElapsed = peekElapsed + delta;
		if peekElapsed >= peekEvery then
			peekElapsed = 0;
			pcall(function() LuaEvents.CivvisControlPeek(); end);
		end
		elapsed = elapsed + delta;
		if elapsed < 1 then return; end
		-- A long frame gets one pulse, never a burst of catch-up calls.
		elapsed = 0;
		pcall(function() CivvisMapView.Pulse(); end);
		pcall(function() LuaEvents.CivvisControlPulse("TopPanel"); end);
		if seatWatch then pcall(CivvisSeatWatch.beat, "frame"); end
		if cfg.RequireHarness == true then pcall(CivvisLobby.pulse); end
		if abTurns > 0 then pcall(CivvisVSyncAB.pulse, abTurns); end
	end);
end
