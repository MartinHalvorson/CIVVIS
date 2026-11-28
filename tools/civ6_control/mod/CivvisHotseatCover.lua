-- ★★ THE PERSON'S SCREEN WHILE CIVVIS PLAYS (a hotseat game a person plays).
--
-- In a hotseat game the screen belongs to whichever seat is playing, so the
-- person used to watch every CIVVIS seat's map, units and popups, and then
-- pressed Start Turn on Civ VI's hand-over screen to get their own back. This
-- context makes it feel like single player instead:
--
--  * From the person's end of turn until their next turn begins, one opaque
--    full-screen popup covers everything (`HotseatCover`). It is raised on
--    `Events.LocalPlayerTurnEnd`, long before the next seat's hand-over is
--    pressed, and on every CIVVIS seat's `LocalPlayerTurnBegin` again (a
--    no-op while it is up), so no frame of a CIVVIS seat's map is drawn
--    uncovered. It consumes every click (the Box) and every key (the input
--    handler, the way PlayerChange.lua:25-34 consumes all input), except
--    Esc, which opens the game menu exactly as the hand-over screen's own Esc
--    does (PlayerChange.lua:30 -> :79 `LuaEvents.PlayerChange_OpenInGameOptionsMenu`,
--    which InGameTopOptionsMenu.lua:724 answers). Save, load and quit stay
--    one key away; the menu sits above the cover.
--  * The person's own hand-over is pressed for them (`HotseatAutoStart`, in
--    CivvisControlAutoClose.lua) and the cover lifts on that press, which the
--    shipped screen announces itself: `LuaEvents.PlayerChange_Close(pid)`
--    (PlayerChange.lua:59, the same signal HotSeatBackground.lua:37 hides
--    on). Before it lifts, the view is put back: the camera where the person
--    ended their last turn, no unit or city of another seat left selected,
--    the plain selection mode.
--
-- WHERE IT SITS: exactly where the shipped hotseat backdrop does. A popup's
-- place in the queue is its priority (Loaders/PopupPriorityLoader_Base.lua:11-15:
-- HotSeatBackground Utmost+1, PausePanel +2, PlayerChange +3, EndGameMenu +4,
-- the game menu +5), and a lower popup stays drawn only with
-- `AlwaysVisibleInQueue`. The backdrop asks for that to stay under the
-- hand-over (HotSeatBackground.lua:27); the cover asks for it too and takes
-- the PausePanel slot, which a hotseat game never fills (Panels/PausePanel.lua:77,
-- :86). So it is over the backdrop and over every screen a CIVVIS seat can
-- raise (High or lower, or no popup at all, like the leader screens), and
-- under the hand-over, the end of the game and the game menu. The hand-over
-- is therefore queued, shown, pressed and dequeued exactly as it ships, never
-- under another popup: over the cover CivvisControlAutoClose.lua hides only
-- its dialog and its "please wait" line (what PlayerChange.lua:61/:194 and
-- :172 hide themselves), so what shows over the cover is its menu button.
-- ⚠ The first cut sat ABOVE the hand-over (the EndGameMenu slot) and kept the
-- hand-over drawn under it with an injected `AlwaysVisibleInQueue`; Civ VI
-- exited silently within a second of a Start Turn press in both live games
-- tried that way (cover-live-1/-2, 2026-11-27), and not in a control without
-- either. Neither is done any more.
--
-- WHAT KEEPS PLAYING UNDER IT. A screen queued under a higher popup is held
-- hidden, and a hidden context does not tick: an announcement holding the
-- game's event lock would never be closed and the CIVVIS seat's turn would
-- stop. So in a cover game every screen CivvisControlAutoClose.lua serves
-- (but the hand-over and the end screens) queues itself `AlwaysVisibleInQueue`
-- on a CIVVIS seat's turn (`CivvisCoverQueue` there): drawn, ticking and
-- closed exactly as before, under the cover. As a backstop this context's own
-- clock asks the hand-over to press (`LuaEvents.CivvisHotseatStartTurn`, which
-- reaches a hidden context) when a seat's pause still stands 1.5 s in.
--
-- A STALL IS NEVER HIDDEN FOR GOOD. The HUD's seat watch
-- (CivvisControlHeartbeat.lua) keeps running under the cover and ends a quiet
-- CIVVIS turn by itself. If a CIVVIS seat's controller stays quiet for
-- `HotseatCoverQuietSeconds` (90) anyway, or one CIVVIS seat's turn runs for
-- `HotseatCoverLongSeconds` (240) however busy its controller looks, or no
-- seat's turn is active for `HotseatCoverBetweenSeconds` (180), or the
-- person's turn waits paused for `HotseatCoverPersonSeconds` (10), the cover
-- stands aside for the rest of that turn (`hotseat_cover`, a `yield_*` reason)
-- and the harness's screen rescue sees the game again. (The long clock: a
-- controller past its forfeit ladder still stamps its pass every second on
-- the HUD pulse, so `quiet` never grows on a wedged turn that beats,
-- e.g. selfplay-duel-20261127T0654Z-p1 turn 36, nine wakes in 20 s of a
-- 30-tick stall; the seat watch forces such a turn at 180 s, #6007, and 240 s
-- leaves it that and two retries.) A quit request (Cmd-Q, or the harness closing
-- the game) lifts it too, so the shipped confirmation is on screen.
--
-- IT IS QUIET (`HotseatCoverSound`). The cover hides pixels; a CIVVIS seat's
-- units, fights, chimes and voice lines were still heard through it. So while
-- it is up over a turn that is not the person's, the sound buses that carry
-- them are turned to 0 and put back the moment the person's turn begins: the
-- shipped way to silence a bus for a while, EndGameMenu.lua:434-435
--   m_savedMusicVol = Options.GetAudioOption("Sound", "Music Volume");
--   Options.SetAudioOption("Sound", "Music Volume", 0, 0);
-- and :452 `Options.SetAudioOption("Sound", "Music Volume", m_savedMusicVol, 0)`.
-- The last argument 0 is "this session only": Options.lua:225-229 passes 1 only
-- under "Confirm clicked: set audio system's .ini to slider values", so a game
-- that dies muted starts the next time at the person's own volumes. `music`
-- (the default) silences SFX, Ambience and Speech and lets the soundtrack play
-- on, as it does through an AI's turn in single player; `silent` turns the
-- Master bus down instead; anything else leaves the sound alone. With the game
-- menu open over the cover the volumes are the person's again (its Options
-- screen reads and can save them: Options.lua:1508, :225), and every way the
-- cover comes down, and this context shutting down, puts them back.
--
-- HOW LONG (`HotseatCoverTimer`). A bar and one line under the seat's name:
-- which CIVVIS seat of how many is playing, and about how long is left. The
-- cover sees every seat's turn begin and end (the turn events, with the local
-- player), so it keeps each seat's own recent turns, the moments between them
-- and the stretch from the last seat to the person's turn (the turn's
-- rollover and their hand-over), and counts down what is left: the rest of
-- the seat playing now, every seat still to come, and that last stretch. Each
-- is a robust average of its last few, so one slow turn does not linger
-- (`typical`). It is worked out again every frame, so a seat that finishes
-- early takes its time off at once, and a seat that runs over holds the count
-- where it is until the next seat starts. The bar is the share of the wait
-- gone by that reckoning: it only moves forward, and it reaches the end as the
-- person's turn comes back. A wait with nothing known yet (the first seat of
-- the first one, or a game loaded part-way through one) shows a sweep instead
-- of a guess; after one seat's turn the count starts.
-- (Live 2026-11-27, hotseat-20261128T031456Z, five CIVVIS seats: the
-- whole-wait estimate this replaces read long on 11 of 15 waits, 3 s long on
-- the one after a single slow turn, and said "Any moment now" for 6.4 s with
-- more than 1.5 s still to go; hotseat_cover_wait_test.lua replays them.)
--
-- Nothing here runs outside a hotseat game a person plays: install.py adds
-- this context only for one, and every entry point checks the settings too.

include("PopupPriorityLoader_", true);

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
local PREFIX = "CIVVISJSON ";
local RUN = tostring(cfg.RunTag or "unset");

CivvisHotseatCover = {
	up = false,        -- queued as the cover right now
	over = false,      -- a team has won (until "one more turn")
	defeated = nil,    -- the seat whose defeat the end screen shows (Events.PlayerDefeat)
	kept = false,      -- the cover stays under that end screen (keptUnder)
	endShown = false,  -- the end screen said it is up (EndGameMenu_Shown), until it closes
	yielded = nil,     -- the seat:turn the cover stood aside for
	key = nil,         -- the seat:turn the clocks below are counting
	quiet = 0, lastBeat = nil, between = 0, personWait = 0, pausedFor = 0,
	turnFor = 0,       -- how long the CIVVIS seat's turn now active has run
	textAt = 0,
	views = {},        -- person seat -> { x, y, zoom } at the end of their turn
	muted = nil,       -- sound bus -> the person's volume, while we hold it at 0
	theirTurn = nil,   -- a person's turn is on, by the turn events (nil before the first)
	wait = nil,        -- the wait the cover is up for (startWait)
	-- What the countdown is made of, each list oldest first, the last `KEEP` kept:
	seatTimes = {},    -- CIVVIS seat -> its own turns, begin to end (s)
	gapTimes = {},     -- CIVVIS seat -> from what came before to its turn's begin (s)
	tailTimes = {},    -- person seat -> the last CIVVIS seat's end to their turn (s)
	lineups = {},      -- person seat -> the CIVVIS seats, in order, of their last whole wait
	waitOn = nil, waitText = nil,
};
-- How many of each are kept and averaged.
CivvisHotseatCover.KEEP = 5;
-- The track's width in CivvisHotseatCover.xml.
CivvisHotseatCover.BAR = 320;
-- `HotseatCoverSound` -> the buses turned down (Options.lua:225-229 names them).
CivvisHotseatCover.BUSES = {
	music = { "SFX Volume", "Ambience Volume", "Speech Volume" },
	silent = { "Master Volume" },
};

function CivvisHotseatCover.listed(list, pid)
	if type(list) ~= "table" or pid == nil then return false; end
	for _, seat in ipairs(list) do
		if tonumber(seat) == pid then return true; end
	end
	return false;
end
function CivvisHotseatCover.person(pid)
	return CivvisHotseatCover.listed(cfg.HotseatHumanSeats, pid);
end
function CivvisHotseatCover.civvis(pid)
	return CivvisHotseatCover.listed(cfg.HotseatSeats, pid) and not CivvisHotseatCover.person(pid);
end
-- The cover itself: a hotseat game, asked for, with a person in it.
function CivvisHotseatCover.enabled()
	return cfg.Hotseat == true and cfg.HotseatCover == true
		and type(cfg.HotseatHumanSeats) == "table" and #cfg.HotseatHumanSeats > 0;
end
-- The person's view kept across the turns they do not play: with the cover,
-- or with their hand-over pressed for them (`--no-cover` alone).
function CivvisHotseatCover.viewKept()
	return cfg.Hotseat == true and (cfg.HotseatCover == true or cfg.HotseatAutoStart == true)
		and type(cfg.HotseatHumanSeats) == "table" and #cfg.HotseatHumanSeats > 0;
end
-- Whether the person's hand-over is pressed for them. A seat with a hotseat
-- password keeps its screen, as the shipped Enter key does (PlayerChange.lua:42-43).
function CivvisHotseatCover.autoStart(pid)
	if cfg.Hotseat ~= true or cfg.HotseatAutoStart ~= true or not CivvisHotseatCover.person(pid) then
		return false;
	end
	local password = "";
	pcall(function() password = PlayerConfigurations[pid]:GetHotseatPassword() or ""; end);
	return password == "";
end
function CivvisHotseatCover.seconds(name, default)
	local value = tonumber(cfg[name]);
	if value == nil or value ~= value or value < 0 then return default; end
	return value;
end

function CivvisHotseatCover.localPlayer()
	local pid = -1;
	pcall(function() pid = Game.GetLocalPlayer(); end);
	return tonumber(pid) or -1;
end
function CivvisHotseatCover.turn()
	local turn = -1;
	pcall(function() turn = Game.GetCurrentGameTurn(); end);
	return tonumber(turn) or -1;
end
function CivvisHotseatCover.active(pid)
	local active = false;
	pcall(function() active = Players[pid]:IsTurnActive() == true; end);
	return active;
end
function CivvisHotseatCover.paused(pid)
	local paused = false;
	pcall(function() paused = PlayerConfigurations[pid]:GetWantsPause() == true; end);
	return paused;
end
function CivvisHotseatCover.keyFor(pid)
	return tostring(pid) .. ":" .. tostring(CivvisHotseatCover.turn());
end

function CivvisHotseatCover.field(value)
	if type(value) == "boolean" then return tostring(value); end
	if type(value) == "number" and value == value then return string.format("%.14g", value); end
	if type(value) == "string" then return '"' .. value:gsub('[%c"\\]', "_") .. '"'; end
	return "null";
end
-- `Automation.Log` is the one channel out (docs/CIV6_COMPUTER_CONTROL.md).
-- The game writes its newest record only when the next one is logged, so a
-- flush line follows every report: a cover that stood aside for a stalled
-- seat must say so while the game is still stalled.
function CivvisHotseatCover.report(kind, fields)
	-- The person-facing install writes nothing without a live harness.
	if not CivvisHarness.live() then return; end
	local parts = { string.format('"kind":"%s","ctx":"cover","run":"%s"', kind, RUN) };
	for _, pair in ipairs(fields or {}) do
		parts[#parts + 1] = string.format('"%s":%s', pair[1], CivvisHotseatCover.field(pair[2]));
	end
	pcall(function() Automation.Log(PREFIX .. "{" .. table.concat(parts, ",") .. "}"); end);
	pcall(function()
		Automation.Log(PREFIX .. string.format('{"kind":"cover_flush","ctx":"cover","run":"%s"}', RUN));
	end);
end

-- Over the hotseat backdrop, under the hand-over (see WHERE IT SITS above).
function CivvisHotseatCover.priority()
	local priority = nil;
	pcall(function()
		priority = PopupPriority.PausePanel
			or (PopupPriority.HotSeatBackground and PopupPriority.HotSeatBackground + 1)
			or PopupPriority.Utmost;
	end);
	return priority;
end
-- HotSeatBackground.lua:10-17's own test for the end of the game. The end
-- screen is above the cover anyway; the cover still goes, so the map is there
-- behind it for "one more turn" (but see `keptUnder`).
function CivvisHotseatCover.endGameUp()
	local up = false;
	pcall(function()
		local menu = ContextPtr:LookUpControl("/InGame/EndGameMenu");
		up = menu ~= nil and UIManager:IsInPopupQueue(menu) == true;
	end);
	return up;
end

function CivvisHotseatCover.seatName(pid)
	local name = nil;
	pcall(function() name = cfg.HotseatNames[tostring(pid)]; end);
	if type(name) ~= "string" or name == "" then
		pcall(function() name = PlayerConfigurations[pid]:GetPlayerName(); end);
	end
	if type(name) ~= "string" or name == "" then name = "CIVVIS"; end
	return name;
end
-- One calm line, and whose turn it is. Nothing about that empire.
function CivvisHotseatCover.detail(pid)
	local turn = CivvisHotseatCover.turn();
	if CivvisHotseatCover.civvis(pid) then
		return string.format("%s, turn %d", CivvisHotseatCover.seatName(pid), turn);
	end
	if turn >= 0 then return string.format("Turn %d", turn); end
	return "";
end
function CivvisHotseatCover.show(pid)
	local text = CivvisHotseatCover.detail(pid);
	pcall(function() Controls.Detail:SetText(text); end);
	pcall(function() Controls.Lines:CalculateSize(); end);
	return text;
end
function CivvisHotseatCover.reset(key)
	local c = CivvisHotseatCover;
	c.key, c.quiet, c.lastBeat, c.between, c.personWait, c.pausedFor = key, 0, nil, 0, 0, 0;
	c.turnFor = 0;
end

-- Up. `force` (a turn has just ended) also clears a stand-aside: whatever
-- stalled has moved on.
-- `RequireHarness` (`tools/civ6_hotseat.py --manual-lobby`, CivvisHarness): the
-- cover stands only while the harness is live, and only over the person's own
-- hand-over or a seat CIVVIS plays. Without the key this is always true.
function CivvisHotseatCover.harnessed(pid)
	if cfg.RequireHarness ~= true then return true; end
	return CivvisHarness.live()
		and (CivvisHotseatCover.person(pid) or CivvisHarness.mine(pid));
end
function CivvisHotseatCover.raise(reason, force)
	local c = CivvisHotseatCover;
	if not c.enabled() or c.over or c.endGameUp() then return false; end
	if not c.harnessed(c.localPlayer()) then return false; end
	local pid = c.localPlayer();
	local key = c.keyFor(pid);
	if force then
		c.yielded = nil;
	elseif c.yielded == key then
		return false;
	end
	if c.up then
		c.show(pid);
		-- Still up as the person's own turn begins (their hand-over is about to
		-- be pressed): its sounds are theirs from this moment.
		c.sound(reason);
		return true;
	end
	local queued = pcall(function()
		UIManager:QueuePopup(ContextPtr, c.priority(), { AlwaysVisibleInQueue = true });
	end);
	c.up = queued;
	c.reset(key);
	c.show(pid);
	if queued then
		-- Joined part-way until `onTurnEnd` says a person's turn just ended.
		c.startWait(nil);
		c.showWait();
	end
	-- `at`: the UI clock, so a wait can be laid against the seats' own records.
	c.report("hotseat_cover", { { "up", queued }, { "reason", reason }, { "player", pid },
		{ "turn", c.turn() }, { "at", c.clock() } });
	c.sound(reason);
	return queued;
end
-- Down. `yield` marks the seat's turn so nothing but its end raises it again.
function CivvisHotseatCover.lift(reason, yield)
	local c = CivvisHotseatCover;
	local pid = c.localPlayer();
	if yield then c.yielded = c.keyFor(pid); end
	if not c.up then return false; end
	pcall(function() UIManager:DequeuePopup(ContextPtr); end);
	c.up = false;
	-- Their turn is back: the wait ran its course unless the cover stood aside.
	local w = c.endWait(not yield and (reason == "person_start" or reason == "person_turn"));
	c.report("hotseat_cover", { { "up", false }, { "reason", reason }, { "player", pid },
		{ "turn", c.turn() }, { "at", c.clock() }, { "quiet", c.quiet }, { "between", c.between },
		{ "seat_turn", c.turnFor },
		{ "waited", w and w.elapsed }, { "estimate", w and w.estimate },
		{ "timed", w and w.timed }, { "seats", w and #w.seen },
		-- Each seat's turn ("seat:seconds") and the time left the bar gave as
		-- each one began: enough to replay the estimate from the record.
		{ "took", w and table.concat(w.took, ",") }, { "said", w and table.concat(w.said, ",") } });
	c.unmute(reason);
	return true;
end

-- Where the person left the camera: the look-at point as a plot
-- (Automation/Automation_ObserverCamera.lua:381-382) and the zoom (WorldInput.lua:346).
function CivvisHotseatCover.saveView(pid)
	local view = nil;
	pcall(function()
		local wx, wy = UI.GetMapLookAtWorldTarget();
		local x, y = UI.GetPlotCoordFromWorld(wx, wy);
		if type(x) == "number" and type(y) == "number" then view = { x = x, y = y }; end
	end);
	if view == nil then return nil; end
	pcall(function() view.zoom = tonumber(UI.GetMapZoom()); end);
	CivvisHotseatCover.views[pid] = view;
	return view;
end
-- The person's view as they left it, before the cover lifts on it. A CIVVIS
-- seat's unit or city still selected is deselected (ActionPanel.lua:646-647),
-- the interface back in plain selection; the person's own selection is theirs.
-- No saved view (their first turn, or a loaded game): their capital, if any.
function CivvisHotseatCover.restoreView(pid)
	local done = { unit = false, city = false, mode = false, camera = "none" };
	pcall(function()
		local unit = UI.GetHeadSelectedUnit();
		if unit ~= nil and unit:GetOwner() ~= pid then
			UI.DeselectAllUnits();
			done.unit = true;
		end
	end);
	pcall(function()
		local city = UI.GetHeadSelectedCity();
		if city ~= nil and city:GetOwner() ~= pid then
			UI.DeselectAllCities();
			done.city = true;
		end
	end);
	pcall(function()
		if UI.GetInterfaceMode() ~= InterfaceModeTypes.SELECTION then
			UI.SetInterfaceMode(InterfaceModeTypes.SELECTION);
			done.mode = true;
		end
	end);
	local view = CivvisHotseatCover.views[pid];
	if view ~= nil then
		if pcall(function() UI.LookAtPlot(view.x, view.y); end) then done.camera = "saved"; end
		if view.zoom ~= nil then pcall(function() UI.SetMapZoom(view.zoom, 0.0, 0.0); end); end
	else
		pcall(function()
			local capital = Players[pid]:GetCities():GetCapitalCity();
			if capital ~= nil then
				UI.LookAtPlot(capital:GetX(), capital:GetY());
				done.camera = "capital";
			end
		end);
	end
	CivvisHotseatCover.report("hotseat_view", { { "player", pid }, { "turn", CivvisHotseatCover.turn() },
		{ "camera", done.camera }, { "x", view and view.x }, { "y", view and view.y },
		{ "zoom", view and view.zoom }, { "deselected_unit", done.unit },
		{ "deselected_city", done.city }, { "mode_reset", done.mode } });
	return done;
end

-- ------------------------------------------------------------------- sound

-- The game menu over the cover: InGame.xml:137 `<LuaContext ID="TopOptionsMenu"`,
-- which queues itself (Menus/InGameTopOptionsMenu.lua:553) and stays queued
-- under its own Options screen (:199 queues that one on top).
function CivvisHotseatCover.menuUp()
	local up = false;
	pcall(function()
		local menu = ContextPtr:LookUpControl("/InGame/TopOptionsMenu");
		up = menu ~= nil and UIManager:IsInPopupQueue(menu) == true;
	end);
	return up;
end
-- Down to 0, each bus's own volume kept. A bus already at 0 is the person's
-- choice and is left out, so it is never "restored" to anything.
function CivvisHotseatCover.mute()
	local c = CivvisHotseatCover;
	local buses = c.BUSES[cfg.HotseatCoverSound];
	if buses == nil or c.muted ~= nil then return false; end
	local saved, count = {}, 0;
	for _, bus in ipairs(buses) do
		local volume = nil;
		pcall(function() volume = tonumber(Options.GetAudioOption("Sound", bus)); end);
		if volume ~= nil and volume > 0
				and pcall(function() Options.SetAudioOption("Sound", bus, 0, 0); end) then
			saved[bus], count = volume, count + 1;
		end
	end
	if count == 0 then return false; end
	c.muted = saved;
	-- What the game says the first bus is at now: 0 if the call took.
	local now = nil;
	pcall(function() now = tonumber(Options.GetAudioOption("Sound", buses[1])); end);
	c.report("hotseat_cover_sound", { { "muted", true }, { "mode", cfg.HotseatCoverSound },
		{ "buses", count }, { "was", saved[buses[1]] }, { "now", now } });
	return true;
end
-- Back to the person's volumes. A bus that would not take its volume stays on
-- the list and is tried again on the next call.
function CivvisHotseatCover.unmute(reason)
	local c = CivvisHotseatCover;
	local saved = c.muted;
	if saved == nil then return false; end
	local left, count = nil, 0;
	for bus, volume in pairs(saved) do
		if pcall(function() Options.SetAudioOption("Sound", bus, volume, 0); end) then
			count = count + 1;
		else
			left = left or {};
			left[bus] = volume;
		end
	end
	c.muted = left;
	c.report("hotseat_cover_sound", { { "muted", false }, { "reason", reason },
		{ "buses", count }, { "left", left ~= nil } });
	return left == nil;
end
-- Whether a person's turn is on. The local player stays the person's seat from
-- the end of their turn until the next seat's begins, so the seat alone does
-- not say: the turn events do, and `IsTurnActive` before the first of them (a
-- game just loaded).
function CivvisHotseatCover.theirs()
	local c = CivvisHotseatCover;
	local pid = c.localPlayer();
	if not c.person(pid) then return false; end
	if c.theirTurn ~= nil then return c.theirTurn; end
	return c.active(pid);
end
-- Quiet exactly while the cover is up over a turn that is not the person's and
-- the game menu is closed.
function CivvisHotseatCover.sound(reason)
	local c = CivvisHotseatCover;
	if c.up and not c.theirs() and not c.menuUp() then
		return c.mute();
	end
	return c.unmute(reason);
end

-- -------------------------------------------------------------------- wait

-- The UI clock (PlotToolTip.lua:879 reads it the same way): sub-second, and
-- it keeps time across a frame the engine caps the `dt` of.
function CivvisHotseatCover.clock()
	local now = nil;
	pcall(function() now = tonumber(UI.GetElapsedTime()); end);
	return now;
end
-- One more of something timed, the last `KEEP` kept.
function CivvisHotseatCover.note(book, key, seconds)
	if key == nil or type(seconds) ~= "number" or seconds ~= seconds or seconds < 0 then return; end
	local list = book[key] or {};
	list[#list + 1] = seconds;
	while #list > CivvisHotseatCover.KEEP do table.remove(list, 1); end
	book[key] = list;
end
-- About how long one of these takes, from the last `KEEP` of them (`all`:
-- every one, alike): a mean, the newer weighted more, with nothing counted at
-- more than twice their median. A mean, not the median, because a wait is a
-- sum and turns run long more often than short (the sum of the seats' medians
-- read short on 12 of 15 waits of the live game, by 0.9 s at the median); the
-- cap so that one slow turn moves the next estimates by a little, not by its
-- whole length. nil with none on record.
function CivvisHotseatCover.typical(list, all)
	if type(list) ~= "table" or #list == 0 then return nil; end
	local first = all and 1 or math.max(#list - CivvisHotseatCover.KEEP + 1, 1);
	local recent, sorted = {}, {};
	for i = first, #list do
		recent[#recent + 1] = list[i];
		sorted[#sorted + 1] = list[i];
	end
	table.sort(sorted);
	local n = #sorted;
	local median = sorted[math.floor((n + 1) / 2)];
	if n % 2 == 0 then median = (sorted[n / 2] + sorted[n / 2 + 1]) / 2; end
	local sum, weight = 0, 0;
	for i, seconds in ipairs(recent) do
		local w = all and 1 or i;
		sum, weight = sum + math.min(seconds, 2 * median) * w, weight + w;
	end
	return sum / weight;
end
-- Every seat's last few together: a seat with nothing on record yet is taken
-- to play like the others (the first round, after its first seat).
function CivvisHotseatCover.pooled(book)
	local all = {};
	for _, list in pairs(book) do
		for _, seconds in ipairs(list) do all[#all + 1] = seconds; end
	end
	return CivvisHotseatCover.typical(all, true);
end
function CivvisHotseatCover.seatTime(seat)
	local c = CivvisHotseatCover;
	return c.typical(c.seatTimes[seat]) or c.pooled(c.seatTimes);
end
function CivvisHotseatCover.gapTime(seat)
	local c = CivvisHotseatCover;
	return c.typical(c.gapTimes[seat]) or c.pooled(c.gapTimes) or 0;
end
-- The CIVVIS seats that play between `person`'s turn and the next person's:
-- the ones seen last time, or before any, the seats in turn order (a hotseat
-- game plays its seats one by one by player number) after `person`, up to the
-- next person's seat.
function CivvisHotseatCover.lineup(person)
	local c = CivvisHotseatCover;
	if type(c.lineups[person]) == "table" and #c.lineups[person] > 0 then return c.lineups[person]; end
	local seats = {};
	for _, list in ipairs({ cfg.HotseatSeats or {}, cfg.HotseatHumanSeats or {} }) do
		for _, seat in ipairs(list) do
			seat = tonumber(seat);
			if seat ~= nil then seats[#seats + 1] = seat; end
		end
	end
	table.sort(seats);
	local start = #seats + 1;
	for i, seat in ipairs(seats) do
		if seat > person then start = i; break; end
	end
	local out = {};
	for step = 0, #seats - 1 do
		local seat = seats[(start - 1 + step) % #seats + 1];
		if c.person(seat) then break; end
		if out[#out] ~= seat then out[#out + 1] = seat; end
	end
	return out;
end
-- The seats of the lineup that have not begun yet in this wait.
function CivvisHotseatCover.toCome(w)
	local out = {};
	for _, seat in ipairs(CivvisHotseatCover.lineup(w.seat)) do
		local seen = seat == w.playing;
		for _, done in ipairs(w.seen) do
			if done == seat then seen = true; end
		end
		if not seen then out[#out + 1] = seat; end
	end
	return out;
end
-- About how long is left of the wait `w`, and whether only the last stretch
-- is (no seat playing or still to come). nil: nothing known to count with
-- (a wait joined part-way, or no seat's turn seen yet).
-- What is left of the seat playing now is never less than nothing: a seat
-- that runs over holds the count at what comes after it.
function CivvisHotseatCover.remaining(w)
	local c = CivvisHotseatCover;
	if w == nil or w.seat == nil then return nil, false; end
	local since = w.elapsed - (w.mark or 0);
	local left = 0;
	if w.playing ~= nil then
		local seconds = c.seatTime(w.playing);
		if seconds == nil then return nil, false; end
		left = math.max(seconds - since, 0);
	end
	local coming = c.toCome(w);
	for i, seat in ipairs(coming) do
		local seconds = c.seatTime(seat);
		if seconds == nil then return nil, false; end
		local gap = c.gapTime(seat);
		if i == 1 and w.playing == nil then gap = math.max(gap - since, 0); end
		left = left + gap + seconds;
	end
	local tail = c.typical(c.tailTimes[w.seat]) or 0;
	if w.playing == nil and #coming == 0 then
		return math.max(tail - since, 0), true;
	end
	return left + tail, false;
end
function CivvisHotseatCover.timeLeft(left, last)
	if type(left) ~= "number" then return ""; end
	-- "Any moment now" only once no seat is playing or still to come.
	if left <= 0.5 and last then return "Any moment now"; end
	local seconds = math.max(math.ceil(left), 1);
	if seconds < 60 then return string.format("About %d s", seconds); end
	return string.format("About %d min %02d s", math.floor(seconds / 60), seconds % 60);
end
-- The line under the bar: "3 of 5, about 6 s" while a seat plays or is to come.
function CivvisHotseatCover.waitLine(w, left, last)
	local c = CivvisHotseatCover;
	local text = c.timeLeft(left, last);
	if w.seat == nil or last or #w.seen == 0 then return text; end
	local count = string.format("%d of %d", #w.seen, #w.seen + #c.toCome(w));
	if text == "" then return count; end
	return count .. ", " .. text:gsub("^About", "about");
end
-- A wait begins. `seat` is the person whose turn just ended (timed from its
-- start); nil for one joined part-way (a load, or after the cover stood
-- aside), which counts nothing down: a sweep.
function CivvisHotseatCover.startWait(seat)
	local c = CivvisHotseatCover;
	c.wait = { seat = seat, elapsed = 0, at = c.clock(), timed = seat ~= nil,
		-- `mark`: when the stretch now running began (nil: not seen);
		-- `playing`: the CIVVIS seat whose turn it is; `seen`: every seat begun;
		-- `clean`: the game menu was not opened over the stretch now running.
		mark = seat ~= nil and 0 or nil, playing = nil, seen = {}, clean = true,
		shown = 0, shownAt = 0, said = {}, took = {} };
	c.wait.estimate = c.remaining(c.wait);
	return c.wait;
end
-- The wait's clock, by the UI clock where it answers and the frame's own `dt`
-- where it does not.
function CivvisHotseatCover.advance(dt)
	local c = CivvisHotseatCover;
	local w = c.wait;
	if w == nil then return nil; end
	local now = c.clock();
	local step = math.min(math.max(tonumber(dt) or 0, 0), 60);
	if now ~= nil and w.at ~= nil and now >= w.at and now - w.at < 600 then
		step = now - w.at;
	end
	w.at = now;
	w.elapsed = w.elapsed + step;
	return w.elapsed;
end
-- A CIVVIS seat's turn begins under the cover (`Events.LocalPlayerTurnBegin`).
function CivvisHotseatCover.seatBegins(pid)
	local c = CivvisHotseatCover;
	local w = c.wait;
	if w == nil or pid == w.playing then return; end
	c.advance(0);
	if w.mark ~= nil and w.playing == nil and w.clean then
		c.note(c.gapTimes, pid, w.elapsed - w.mark);
	end
	w.playing, w.mark, w.clean = pid, w.elapsed, true;
	w.seen[#w.seen + 1] = pid;
	local left = c.remaining(w);
	w.said[#w.said + 1] = left and string.format("%.1f", left) or "-";
end
-- ...and ends (`Events.LocalPlayerTurnEnd`).
function CivvisHotseatCover.seatEnds(pid)
	local c = CivvisHotseatCover;
	local w = c.wait;
	if w == nil or pid ~= w.playing then return; end
	c.advance(0);
	local took = w.elapsed - w.mark;
	if w.clean then c.note(c.seatTimes, pid, took); end
	w.took[#w.took + 1] = string.format("%d:%.2f", pid, took);
	w.playing, w.mark, w.clean = nil, w.elapsed, true;
end
-- The wait is over. One that ran its course (their turn's end to the start of
-- their next) teaches the stretch after the last seat, and which seats play.
function CivvisHotseatCover.endWait(ranItsCourse)
	local c = CivvisHotseatCover;
	local w = c.wait;
	if w == nil then return nil; end
	c.advance(0);
	c.wait = nil;
	if ranItsCourse and w.seat ~= nil and w.playing == nil and #w.seen > 0 then
		if w.clean then c.note(c.tailTimes, w.seat, w.elapsed - w.mark); end
		c.lineups[w.seat] = w.seen;
	end
	return w;
end
-- The bar and its line, every frame the cover is up.
function CivvisHotseatCover.showWait()
	local c = CivvisHotseatCover;
	local w = c.wait;
	local on = cfg.HotseatCoverTimer == true and w ~= nil;
	if on ~= c.waitOn then
		c.waitOn = on;
		pcall(function() Controls.Wait:SetHide(not on); end);
		pcall(function() Controls.Lines:CalculateSize(); end);
	end
	if not on then return nil; end
	local left, last = c.remaining(w);
	local share = nil;
	local width, offset = c.BAR / 5, 0;
	if left ~= nil then
		-- The share of the wait gone by, never full while it lasts. It only
		-- moves forward, and a step forward (a seat done early) is drawn over
		-- about a quarter of a second rather than in one frame.
		local whole = w.elapsed + left;
		local target = whole > 0 and math.min(w.elapsed / whole, 0.99) or 0.99;
		local step = math.max(w.elapsed - w.shownAt, 0);
		if target > w.shown then
			w.shown = w.shown + (target - w.shown) * math.min(step / 0.25, 1);
		end
		share = w.shown;
		width = math.max(share * c.BAR, 1);
	else
		-- Nothing to count with: a fifth of the track, there and back every 2.4 s.
		local phase = (w.elapsed % 2.4) / 1.2;
		if phase > 1 then phase = 2 - phase; end
		offset = phase * (c.BAR - width);
	end
	w.shownAt = w.elapsed;
	pcall(function()
		Controls.WaitFill:SetSizeX(math.floor(width + 0.5));
		Controls.WaitFill:SetOffsetX(math.floor(offset + 0.5));
	end);
	local text = c.waitLine(w, left, last);
	if text ~= c.waitText then
		c.waitText = text;
		pcall(function() Controls.WaitText:SetText(text); end);
	end
	return share;
end

-- ------------------------------------------------------------------ events

function CivvisHotseatCover.onTurnEnd()
	local pid = CivvisHotseatCover.localPlayer();
	CivvisHotseatCover.theirTurn = false;
	if CivvisHotseatCover.person(pid) and CivvisHotseatCover.viewKept() then
		CivvisHotseatCover.saveView(pid);
	end
	if CivvisHotseatCover.civvis(pid) then CivvisHotseatCover.seatEnds(pid); end
	if CivvisHotseatCover.raise("turn_end", true) and CivvisHotseatCover.person(pid) then
		-- The wait for their next turn starts here, whole.
		CivvisHotseatCover.startWait(pid);
		CivvisHotseatCover.showWait();
	end
end
function CivvisHotseatCover.onTurnBegin()
	local pid = CivvisHotseatCover.localPlayer();
	CivvisHotseatCover.theirTurn = CivvisHotseatCover.person(pid);
	if CivvisHotseatCover.person(pid) then
		-- Their hand-over is pressed for them and the cover lifts on the press;
		-- a person who presses their own sees it as soon as their turn begins.
		if CivvisHotseatCover.autoStart(pid) then
			CivvisHotseatCover.raise("person_handover");
		else
			CivvisHotseatCover.lift("person_turn");
		end
		return;
	end
	CivvisHotseatCover.raise("civvis_turn");
	if CivvisHotseatCover.civvis(pid) then CivvisHotseatCover.seatBegins(pid); end
end
-- PlayerChange.lua:59: the hand-over closed, by our press or theirs.
function CivvisHotseatCover.onHandoverClosed(pid)
	pid = tonumber(pid);
	if pid == nil or not CivvisHotseatCover.person(pid) then return; end
	if CivvisHotseatCover.viewKept() and CivvisHotseatCover.harnessed(pid) then
		CivvisHotseatCover.restoreView(pid);
	end
	CivvisHotseatCover.lift("person_start");
end
-- A game starting or loading: covered unless it is the person's own turn and
-- they press their own hand-over (PlayerChange.lua:143-158 decides which).
function CivvisHotseatCover.onLoadScreenClose()
	local pid = CivvisHotseatCover.localPlayer();
	if CivvisHotseatCover.person(pid) and CivvisHotseatCover.active(pid)
			and not CivvisHotseatCover.autoStart(pid) then
		return;
	end
	CivvisHotseatCover.raise("load");
end
function CivvisHotseatCover.onVictory()
	CivvisHotseatCover.over = true;
	CivvisHotseatCover.lift("end_game");
end
-- ★ A SEAT'S DEFEAT ON A CIVVIS TURN IS NOT THE END OF THAT TURN. In hotseat
-- the end screen shows any human seat's defeat to whoever's turn it is
-- (EndGameMenu.lua:1012-1013 `or (GameConfiguration.IsHotseat() and
-- defeatPlayer ~= nil and defeatPlayer:IsHuman())) then -- Hotseat Only -
-- Another human player was defeated`), and its Next Player only closes the
-- screen (:304-318: `UI.RequestAction(ActionTypes.ACTION_ENDTURN)` only when
-- the viewer is that seat and its turn is active, then `Close()`): the turn
-- under it goes on. A cover that lifted for it left the CIVVIS seat's own map,
-- HUD and units on screen, and in the person's hands, from Next Player to that
-- seat's end of turn. So when the screen shows a CIVVIS seat's defeat on a
-- turn that is not a person's, the cover stays queued under it (the screen is
-- above it, PopupPriorityLoader_Base.lua:14 EndGameMenu, and is seen and
-- pressed as it ships) and the turn stays covered after Next Player. A
-- person's defeat, a win, or an end screen whose defeat is not known lift it
-- as before.
function CivvisHotseatCover.onDefeat(player)
	CivvisHotseatCover.defeated = tonumber(player);
end
function CivvisHotseatCover.keptUnder()
	local c = CivvisHotseatCover;
	if c.over or c.defeated == nil or c.person(c.defeated) then return false; end
	return not c.person(c.localPlayer());
end
-- EndGameMenu.lua:547 `LuaEvents.EndGameMenu_Shown()` is raised from inside
-- the defeat's own handler (:1009 OnPlayerDefeat -> View -> OnShow), so this
-- context's `onDefeat` may not have run yet: a win lifts at once (it has its
-- own event, `onVictory`), anything else on the next frame.
function CivvisHotseatCover.onEndScreenShown()
	CivvisHotseatCover.endShown = true;
	if CivvisHotseatCover.over then CivvisHotseatCover.lift("end_game"); end
end
-- :578 `LuaEvents.EndGameMenu_Closed()`, Next Player's `Close()` among others.
function CivvisHotseatCover.onEndScreenClosed()
	local c = CivvisHotseatCover;
	c.defeated, c.kept, c.endShown = nil, false, false;
end
function CivvisHotseatCover.onOneMoreTurn()
	CivvisHotseatCover.over = false;
end
function CivvisHotseatCover.onQuitRequest()
	CivvisHotseatCover.lift("quit_request");
end
function CivvisHotseatCover.openMenu()
	-- The person's volumes before the menu (and its Options screen) is up.
	CivvisHotseatCover.unmute("menu");
	pcall(function() LuaEvents.PlayerChange_OpenInGameOptionsMenu(); end);
	CivvisHotseatCover.report("hotseat_cover_menu", { { "player", CivvisHotseatCover.localPlayer() },
		{ "turn", CivvisHotseatCover.turn() } });
end
-- Every key is the cover's (PlayerChange.lua:33 "Consume all input"); Esc,
-- on its release as there, opens the game menu.
function CivvisHotseatCover.onInput(uiMsg, wParam)
	local escape = false;
	pcall(function() escape = uiMsg == KeyEvents.KeyUp and wParam == Keys.VK_ESCAPE; end);
	if escape then CivvisHotseatCover.openMenu(); end
	return true;
end

-- The hand-over of the seat now playing still holds its pause: ask it to
-- press (CivvisControlAutoClose.lua, `CivvisHotseatScreen.fromCover`).
function CivvisHotseatCover.nudge(pid, dt)
	local c = CivvisHotseatCover;
	c.pausedFor = c.pausedFor + dt;
	if c.pausedFor < 1.5 then return false; end
	c.pausedFor = 0;
	pcall(function() LuaEvents.CivvisHotseatStartTurn(pid, c.turn()); end);
	return true;
end

-- Every frame the cover is up (a popup context ticks while it shows).
function CivvisHotseatCover.tick(dt)
	local c = CivvisHotseatCover;
	dt = math.min(math.max(tonumber(dt) or 0, 0), 3);
	if not c.up then return; end
	-- A harness that stopped or died takes the cover down within its freshness
	-- window, and so does a turn that is neither the person's nor CIVVIS's.
	if not c.harnessed(c.localPlayer()) then
		c.lift("no_harness");
		return;
	end
	local endUp = c.endGameUp();
	-- Kept under a screen that has left the queue without saying so: closed.
	if c.kept and not endUp and not c.endShown then c.onEndScreenClosed(); end
	if endUp or c.endShown then
		if not c.keptUnder() then
			c.endShown = false;
			c.lift("end_game");
			return;
		end
		if endUp then
			-- Under the end screen: it holds the game until Next Player and the
			-- person sees it, so no stall is hidden and nothing here counts one.
			-- Its sound is theirs. Not a stretch of CIVVIS's to learn from.
			if not c.kept then
				c.kept = true;
				c.report("hotseat_cover_kept", { { "reason", "defeat" }, { "defeated", c.defeated },
					{ "player", c.localPlayer() }, { "turn", c.turn() } });
			end
			c.advance(dt);
			if c.wait ~= nil then c.wait.timed, c.wait.clean = false, false; end
			c.unmute("end_screen");
			return;
		end
		-- Said to be up, and gone from the queue: closed.
		c.onEndScreenClosed();
	end
	local pid = c.localPlayer();
	local key = c.keyFor(pid);
	if key ~= c.key then c.reset(key); end
	-- The wait's clock and bar, every frame.
	c.advance(dt);
	c.showWait();
	c.textAt = c.textAt + dt;
	if c.textAt >= 0.5 then
		c.textAt = 0;
		c.show(pid);
		-- Twice a second: the sound follows the game menu (quiet again once it
		-- closes), and what the menu was opened over is not one to learn from.
		if c.wait ~= nil and c.menuUp() then c.wait.timed, c.wait.clean = false, false; end
		c.sound("tick");
	end
	if not c.active(pid) then
		-- Between turns: the game is processing, or the next seat's turn is
		-- about to begin.
		c.between = c.between + dt;
		if c.between >= c.seconds("HotseatCoverBetweenSeconds", 180) then
			c.lift("yield_between", true);
		end
		return;
	end
	c.between = 0;
	local paused = c.paused(pid);
	if c.person(pid) then
		-- Their turn is running: whoever pressed Start Turn, it is theirs.
		if not paused then
			c.lift("person_turn");
			return;
		end
		c.personWait = c.personWait + dt;
		if c.autoStart(pid) then c.nudge(pid, dt); end
		if c.personWait >= c.seconds("HotseatCoverPersonSeconds", 10) then
			c.lift("person_wait", true);
		end
		return;
	end
	-- A CIVVIS seat: the pass stamp its controller leaves for the HUD's seat
	-- watch (`CivvisSeat.beat`, CivvisControlAgent.lua). Paused counts as quiet.
	local beat = nil;
	pcall(function() beat = ExposedMembers.CivvisSeatBeat[pid]; end);
	if paused or beat == c.lastBeat then
		c.quiet = c.quiet + dt;
	else
		c.quiet = 0;
	end
	c.lastBeat = beat;
	if paused then c.nudge(pid, dt); end
	if c.quiet >= c.seconds("HotseatCoverQuietSeconds", 90) then
		c.lift("yield_quiet", true);
	end
	c.turnFor = c.turnFor + dt;
	if c.turnFor >= c.seconds("HotseatCoverLongSeconds", 240) then
		c.lift("yield_long", true);
	end
end

function CivvisHotseatCover.arm()
	local c = CivvisHotseatCover;
	-- The person-facing install outside a hotseat game: nothing at all, and
	-- this context (the opaque cover) is kept hidden.
	if CivvisHarness.dormant() then
		pcall(function() ContextPtr:SetHide(true); end);
		return false;
	end
	if not (c.enabled() or c.viewKept()) then
		c.report("hotseat_cover_armed", { { "cover", false }, { "view", false } });
		return false;
	end
	local function guarded(handler)
		return function(...) pcall(handler, ...); end;
	end
	-- The shipped hotseat backdrop, dimmed (HotSeatBackground.lua:34); the Box
	-- under it is opaque whether or not the texture loads.
	pcall(function() Controls.Backdrop:SetTexture("LEADER_HOJO_BACKGROUND"); end);
	pcall(function() Events.LocalPlayerTurnEnd.Add(guarded(c.onTurnEnd)); end);
	pcall(function() Events.LocalPlayerTurnBegin.Add(guarded(c.onTurnBegin)); end);
	pcall(function() Events.LoadScreenClose.Add(guarded(c.onLoadScreenClose)); end);
	pcall(function() Events.TeamVictory.Add(guarded(c.onVictory)); end);
	pcall(function() Events.UserRequestClose.Add(guarded(c.onQuitRequest)); end);
	pcall(function() LuaEvents.PlayerChange_Close.Add(guarded(c.onHandoverClosed)); end);
	pcall(function() Events.PlayerDefeat.Add(guarded(c.onDefeat)); end);
	pcall(function() LuaEvents.EndGameMenu_Shown.Add(guarded(c.onEndScreenShown)); end);
	pcall(function() LuaEvents.EndGameMenu_Closed.Add(guarded(c.onEndScreenClosed)); end);
	pcall(function() LuaEvents.EndGameMenu_OneMoreTurn.Add(guarded(c.onOneMoreTurn)); end);
	pcall(function() Controls.MenuButton:RegisterCallback(Mouse.eLClick, guarded(c.openMenu)); end);
	pcall(function()
		ContextPtr:SetInputHandler(function(uiMsg, wParam, lParam)
			-- `RequireHarness`: no key is taken unless the cover is up.
			if cfg.RequireHarness == true and not c.up then return false; end
			local ok, consumed = pcall(c.onInput, uiMsg, wParam);
			return (not ok) or consumed;
		end);
	end);
	pcall(function() ContextPtr:SetUpdate(guarded(c.tick)); end);
	-- A context that goes away muted puts the volumes back (PausePanel.lua:75
	-- registers its own the same way).
	pcall(function() ContextPtr:SetShutdown(guarded(function() c.unmute("shutdown"); end)); end);
	c.report("hotseat_cover_armed", { { "cover", c.enabled() }, { "view", c.viewKept() },
		{ "auto_start", cfg.HotseatAutoStart == true }, { "priority", c.priority() },
		{ "sound", cfg.HotseatCoverSound }, { "timer", cfg.HotseatCoverTimer == true } });
	return true;
end

CivvisHotseatCover.arm();
