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
-- `HotseatCoverQuietSeconds` (90) anyway, or no seat's turn is active for
-- `HotseatCoverBetweenSeconds` (180), or the person's turn waits paused for
-- `HotseatCoverPersonSeconds` (10), the cover stands aside for the rest of
-- that turn (`hotseat_cover`, a `yield_*` reason) and the harness's screen
-- rescue sees the game again. A quit request (Cmd-Q, or the harness closing
-- the game) lifts it too, so the shipped confirmation is on screen.
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
	yielded = nil,     -- the seat:turn the cover stood aside for
	key = nil,         -- the seat:turn the clocks below are counting
	quiet = 0, lastBeat = nil, between = 0, personWait = 0, pausedFor = 0,
	textAt = 0,
	views = {},        -- person seat -> { x, y, zoom } at the end of their turn
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
-- behind it for "one more turn".
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
		return true;
	end
	local queued = pcall(function()
		UIManager:QueuePopup(ContextPtr, c.priority(), { AlwaysVisibleInQueue = true });
	end);
	c.up = queued;
	c.reset(key);
	c.show(pid);
	c.report("hotseat_cover", { { "up", queued }, { "reason", reason }, { "player", pid },
		{ "turn", c.turn() } });
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
	c.report("hotseat_cover", { { "up", false }, { "reason", reason }, { "player", pid },
		{ "turn", c.turn() }, { "quiet", c.quiet }, { "between", c.between } });
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

-- ------------------------------------------------------------------ events

function CivvisHotseatCover.onTurnEnd()
	local pid = CivvisHotseatCover.localPlayer();
	if CivvisHotseatCover.person(pid) and CivvisHotseatCover.viewKept() then
		CivvisHotseatCover.saveView(pid);
	end
	CivvisHotseatCover.raise("turn_end", true);
end
function CivvisHotseatCover.onTurnBegin()
	local pid = CivvisHotseatCover.localPlayer();
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
function CivvisHotseatCover.onOneMoreTurn()
	CivvisHotseatCover.over = false;
end
function CivvisHotseatCover.onQuitRequest()
	CivvisHotseatCover.lift("quit_request");
end
function CivvisHotseatCover.openMenu()
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
	if c.endGameUp() then
		c.lift("end_game");
		return;
	end
	local pid = c.localPlayer();
	local key = c.keyFor(pid);
	if key ~= c.key then c.reset(key); end
	c.textAt = c.textAt + dt;
	if c.textAt >= 0.5 then
		c.textAt = 0;
		c.show(pid);
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
	pcall(function() LuaEvents.EndGameMenu_Shown.Add(guarded(function() c.lift("end_game"); end)); end);
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
	c.report("hotseat_cover_armed", { { "cover", c.enabled() }, { "view", c.viewKept() },
		{ "auto_start", cfg.HotseatAutoStart == true }, { "priority", c.priority() } });
	return true;
end

CivvisHotseatCover.arm();
