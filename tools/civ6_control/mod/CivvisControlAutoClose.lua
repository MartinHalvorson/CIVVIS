-- Close an announcement screen on its own, a couple of seconds after it opens.
--
-- Civilization VI stops the world for its own announcements. A wonder
-- finishing, an era ending with its era score, a technology completing: each
-- is a full-screen context, and ExclusivePopupManager:Lock holds an engine
-- event until the player proceeds. With this controller in the seat there is
-- no player to proceed, so the announcement sits there over the map -- on a
-- game that is being watched precisely to see the map.
--
-- Every one of those screens already knows how to close itself; what it lacks
-- is somebody to say when. This gives each of them a stopwatch instead of a
-- person. The screen opens, it stays up long enough to read, and then it ends
-- exactly the way its own close button ends it. Nothing about the
-- announcement changes -- what it says, when it fires, what it locks -- only
-- who finishes it.
--
-- Screens that *ask* something are deliberately not on the list in
-- CivvisControl.modinfo: a dedication, a great person, a promotion. Closing a
-- question answers it, and those answers belong to the agent's blocker loop,
-- not to a timer. One screen sits on the boundary: the era review both reports
-- the era score and leads to the dedication choice, so it is *continued*
-- rather than closed -- its Close button skips the dedication and its Continue
-- button raises it.
--
-- One file serves every screen. Each ReplaceUIScript in the .modinfo points a
-- context at this file; the file asks the context for its own name, loads the
-- shipped script of that name into itself, and adds the stopwatch on top. No
-- shipped code is copied here, so none of it goes stale when the game updates.

local cfg = CivvisControlConfig or {};

-- Long enough to read a headline and the line of quote under it, short enough
-- that nobody sits through it twice. It is a setting because "long enough to
-- read" is a judgement about the person watching, not a fact about the game.
local SECONDS = tonumber(cfg.AnnouncementSeconds) or 1.0;

-- WonderBuiltPopup has two stock alpha tracks and two stock slide tracks. The
-- longest stock track is configured at `.5` seconds, while the generic
-- announcement clock is deliberately much shorter on climb runs. Never let
-- that generic clock cut the wonder reveal off before its animation has a
-- chance to finish. The one-second floor covers the animation's first frames;
-- the animation state below remains authoritative after that. A hard upper
-- bound still keeps a broken animation from covering the map forever.
local WONDER_MIN_SECONDS = 1.0;
local WONDER_ANIMATION_TIMEOUT_SECONDS = 8.0;

-- Dialogue is a blocker, not a readable announcement. Keep the configured
-- value bounded even when an older launcher or a hand-written config supplies
-- something slower. The pixel backstop uses the same budget independently.
local MAX_DIALOGUE_SECONDS = 2.0;
local DIALOGUE_SECONDS = tonumber(cfg.DialogueSeconds) or 0.25;

-- Era screens get a shorter clock than the rest. They are the most frequent
-- interruption in a long game and carry the least the agent needs to read, and
-- at Online speed an era can turn over every twenty turns or so.
local ERA_SECONDS = tonumber(cfg.EraAnnouncementSeconds) or 0.5;
local ERA_SCREENS = {
	EraCompletePopup = true, EraReviewPopup = true,
	DedicationPopup = true, BoostUnlockedPopup = true,
	-- The era-score animations: a historic moment plays a card flourish and
	-- the era progress panel animates a bar. Both are pure spectacle for an
	-- unattended run and both sit over the map while they play.
	HistoricMoments = true, EraProgressPanel = true,
};

-- EndGameMenu owns the native victory movie and result. The harness waits for
-- its completion callback before shutting down; the ordinary popup close ladder
-- must never skip that movie. The clock remains available for legacy config.
local END_SECONDS = tonumber(cfg.EndGameSeconds) or 10.0;
local END_SCREENS = { EndGameMenu = true };
-- ⚠ The era clock is applied further down, AFTER `NAME` is declared. It used to
-- be applied right here, twelve lines before `local NAME` existed, so `NAME` was
-- the nil global and `ERA_SCREENS[nil]` was nil: the branch never once ran and
-- every era screen sat for the full announcement time. Lua does not complain —
-- indexing a table with a nil key READS fine and only assignment throws — so the
-- setting simply had no effect. Found by check_scope.py.

-- A screen that will not go away must not write a line every two seconds for
-- the rest of a multi-hour run. Legitimate repeats -- two wonders finishing on
-- the same turn, a queue of completed technologies -- clear this counter by
-- being hidden in between, so only a screen that ignores its own close
-- callback ever reaches the limit.
local GIVE_UP_AFTER = 20;
-- Dialogue contexts can remain technically visible behind a different modal.
-- Ask the desktop classifier to inspect the real pixels after one second while
-- the complete Lua exit ladder keeps trying in the background. This caught a
-- live one-button "Unit Captured" acknowledgement that otherwise waited for
-- all twenty rungs.
local DESKTOP_AFTER = GIVE_UP_AFTER;

local PREFIX = "CIVVISJSON ";

local NAME = "unknown";
pcall(function() NAME = ContextPtr:GetID() or "unknown"; end);

-- Now that this context knows its own name, the era screens can get their
-- shorter clock. This must stay below `local NAME`.
if ERA_SCREENS[NAME] then SECONDS = ERA_SECONDS; end
-- ⚠ AFTER the era line and BEFORE the dialogue `math.min` below, and both of
-- those orderings are load-bearing. The era table can only shorten a clock, and
-- the dialogue rule takes a MINIMUM — so an end screen that ever matched it
-- would be clamped back to 0.25s and the hold would silently not happen. This
-- is also why `NAME` has to be declared above: `ERA_SCREENS[nil]` reads as nil
-- without complaint, which is how the era clock went a whole project unapplied.
if END_SCREENS[NAME] then SECONDS = END_SECONDS; end
if NAME == "WonderBuiltPopup" then
	SECONDS = math.max(SECONDS, WONDER_MIN_SECONDS);
end
-- Leader/deal views are interactive overlays rather than readable completion
-- cards. When one refuses its first exit path we need to reach the later response
-- rungs promptly; twenty one-second probes left a real leader screen up for twenty
-- seconds before the desktop fallback even began.
if NAME == "DiplomacyActionView" or NAME == "DiplomacyDealView"
		or NAME == "EspionagePopup" or NAME == "EspionageEscape" then
	SECONDS = math.min(SECONDS, math.max(0, DIALOGUE_SECONDS), MAX_DIALOGUE_SECONDS);
	DESKTOP_AFTER = 4;
end
if SECONDS < 0 then SECONDS = 0; end

local RUN = tostring(cfg.RunTag or "unset");

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

-- The agent's channel, and for the same reason: this build writes no Lua.log,
-- so Automation.Log is the only output that survives. Every field written here
-- is a name or a number, so the encoder the agent needs is not needed here.
local function report(kind, fields)
	-- The person-facing install writes nothing without a live harness.
	if not CivvisHarness.live() then return; end
	local line = string.format('{"kind":"%s","ctx":"autoclose","screen":"%s","run":"%s"%s}',
	                           kind, NAME, RUN, fields or "");
	pcall(function() Automation.Log(PREFIX .. line); end);
end

-- ★★ WHO HOLDS THE GAME? AN EVENT-LOCK LEDGER. G84 t117 froze twice with the
-- Game Core thread idle in pthread_cond_wait and WinMain still rendering: the
-- app stopped dispatching the game, which is what a UI context does while it
-- holds an event lock (`UI.ReferenceCurrentEvent`, released by
-- `UI.ReleaseEventID`; DiplomacyActionView.lua:2116/:2175). This build writes
-- no Lua.log, so nothing could say which context held one. Every context this
-- shim runs in wraps the pair once per UI table and keeps the held set in
-- `ExposedMembers.CivvisEventLocks`, which the agent's `ai_phase_stall` and
-- `end_turn_wait` read back.
-- ⚠ FAIL-OPEN: the original is called first and its exact results returned;
-- all recording is inside pcall, so the ledger cannot change or block a lock.
-- Bounded: released entries are dropped and at most EVENT_LOCK_CAP are held.
local EVENT_LOCK_CAP = 32;
local function eventLedger()
	if type(ExposedMembers) ~= "table" then return nil; end
	local ledger = ExposedMembers.CivvisEventLocks;
	if type(ledger) ~= "table" then
		ledger = { held = {}, count = 0, overflow = 0 };
		ExposedMembers.CivvisEventLocks = ledger;
	end
	return ledger;
end
local function noteEventLock(op, id)
	local ledger = eventLedger();
	local key = tonumber(id);
	if key == nil then return; end
	if ledger == nil then
		-- No shared table in this context: still say what happened.
		report("event_lock", string.format(',"op":"%s","id":%d,"held":-1', op, key));
		return;
	end
	if op == "ref" then
		if ledger.held[key] == nil then
			if ledger.count >= EVENT_LOCK_CAP then
				ledger.overflow = ledger.overflow + 1;
			else
				local turn = -1;
				pcall(function() turn = Game.GetCurrentGameTurn(); end);
				local at = -1;
				pcall(function() at = UI.GetElapsedTime(); end);
				ledger.held[key] = { ctx = NAME, turn = turn, at = at };
				ledger.count = ledger.count + 1;
			end
		end
	elseif ledger.held[key] ~= nil then
		ledger.held[key] = nil;
		ledger.count = ledger.count - 1;
	end
	report("event_lock", string.format(',"op":"%s","id":%d,"held":%d', op, key, ledger.count));
end
local function packResults(...)
	return { n = select("#", ...), ... };
end
-- Install the wrap. In place when `UI` is a plain table that takes the
-- assignment; otherwise this context's global `UI` is shadowed by a proxy
-- that holds only the two wrapped lock calls and hands back the REAL object
-- for every other key (writes go through to the real UI too).
-- ⚠ G90 (pin bbb5732): 19 DiplomacyActionView closes and no `event_lock`
-- row, so the in-place wrap never took on this build; `UI` is not a plain
-- table there. Each context reports what it did (`event_ledger`), and if
-- anything fails the global `UI` is left exactly as it was.
local function installEventLedger()
	local ui = UI;
	local kind = type(ui);
	if kind ~= "table" and kind ~= "userdata" then return false, "none", kind, "no_ui"; end
	local marked = false;
	pcall(function() marked = ui.CivvisEventLedger == true; end);
	if marked then return false, "already", kind, nil; end
	local reference, release;
	pcall(function() reference, release = ui.ReferenceCurrentEvent, ui.ReleaseEventID; end);
	if type(reference) ~= "function" or type(release) ~= "function" then
		return false, "none", kind, "no_lock_functions";
	end
	local function wrappedReference(...)
		local results = packResults(reference(...));
		pcall(noteEventLock, "ref", results[1]);
		return unpack(results, 1, results.n);
	end
	local function wrappedRelease(id, ...)
		local results = packResults(release(id, ...));
		pcall(noteEventLock, "release", id);
		return unpack(results, 1, results.n);
	end
	if kind == "table" then
		local wrote = pcall(function()
			ui.ReferenceCurrentEvent = wrappedReference;
			ui.ReleaseEventID = wrappedRelease;
			ui.CivvisEventLedger = true;
		end);
		local took = false;
		-- `==`, not rawequal: Havok Script's sandbox has no rawequal (G89,
		-- 2026-10-05: every context threw here after a successful swap).
		-- Neither side has __eq, so this is identity.
		pcall(function()
			took = ui.ReferenceCurrentEvent == wrappedReference
				and ui.ReleaseEventID == wrappedRelease;
		end);
		if wrote and took then return true, "direct", kind, nil; end
		-- Undo a partial write before shadowing.
		pcall(function() ui.ReferenceCurrentEvent = reference; ui.ReleaseEventID = release; end);
	end
	local proxy = nil;
	pcall(function()
		proxy = setmetatable({
			ReferenceCurrentEvent = wrappedReference,
			ReleaseEventID = wrappedRelease,
			CivvisEventLedger = true,
		}, {
			__index = function(_, key) return ui[key]; end,
			__newindex = function(_, key, value) ui[key] = value; end,
		});
	end);
	if proxy == nil then return false, "none", kind, "proxy_not_built"; end
	local swapped = pcall(function() UI = proxy; end);
	local current = nil;
	pcall(function() current = UI; end);
	if swapped and current == proxy then return true, "proxy", kind, nil; end
	pcall(function() UI = ui; end);
	return false, "none", kind, "global_not_replaced";
end
-- Not in a dormant person-facing context: there `UI` stays the shipped object.
if not CivvisHarness.dormant() then
	local ran, installed, how, uiType, why = pcall(installEventLedger);
	if not ran then
		-- `installed` holds the error message when the install raised.
		why = "threw: " .. tostring(installed):gsub('["\\]', "'");
		installed, how, uiType = false, "none", type(UI);
	end
	report("event_ledger", string.format(',"installed":%s,"how":"%s","ui_type":"%s","exposed":"%s"%s',
		tostring(installed == true), tostring(how), tostring(uiType), type(ExposedMembers),
		why and string.format(',"why":"%s"', tostring(why)) or ""));
end

-- ★★ HOTSEAT COVER: A SCREEN UNDER THE COVER STILL COMES UP, AND STILL CLOSES.
-- In a game a person plays, CivvisHotseatCover.lua lays one popup over
-- everything while CIVVIS plays (`HotseatCover`). A popup queued under a
-- higher one is held hidden unless it asks to stay drawn
-- (`AlwaysVisibleInQueue`: HotSeatBackground.lua:27 asks, to stay under the
-- hand-over; FullscreenMapPopup.lua:167 asks, to stay under the MP pause),
-- and a hidden context does not tick. Under the cover that would leave an
-- announcement holding the game's event lock (`ExclusivePopupManager:Lock`,
-- PopupManager.lua:52-61) never closed, so the CIVVIS seat's turn would stop.
-- So in a cover game this context's global `UIManager` is a proxy whose
-- `QueuePopup` adds `AlwaysVisibleInQueue` when this context queues itself on
-- a CIVVIS seat's turn. Drawn under the cover, it shows, ticks and is closed
-- exactly as before; the person sees none of it. Every other call reaches the
-- real object with the real `self`, and the proxy is used only if it answers
-- `IsInPopupQueue` the way the real one does.
-- The hand-over (it sits above the cover and is pressed as it ships) and the
-- end screens get no proxy at all. ⚠ The first cut also kept the hand-over
-- drawn under a cover above it this way, and Civ VI exited within a second of
-- a Start Turn press twice in two live games (CivvisHotseatCover.lua, WHERE IT
-- SITS). `HotseatCoverQueue = false` (`civ6_hotseat.py --no-cover-queue`)
-- turns the proxy off to tell its part apart: the screens then wait under the
-- cover until it stands aside.
CivvisCoverQueue = { installed = false, visible = 0 };
function CivvisCoverQueue.listed(list, pid)
	if type(list) ~= "table" or pid == nil then return false; end
	for _, seat in ipairs(list) do
		if tonumber(seat) == pid then return true; end
	end
	return false;
end
function CivvisCoverQueue.on()
	return cfg.Hotseat == true and cfg.HotseatCover == true and cfg.HotseatCoverQueue ~= false
		and NAME ~= "PlayerChange" and not END_SCREENS[NAME]
		and (cfg.RequireHarness ~= true or not CivvisHarness.dormant());
end
function CivvisCoverQueue.wanted(name, pid)
	if not CivvisCoverQueue.on() or name == "PlayerChange" or END_SCREENS[name] then
		return false;
	end
	-- `RequireHarness`: the shipped queueing unless CIVVIS's seat, harness live.
	if cfg.RequireHarness == true and not CivvisHarness.mine(pid) then return false; end
	return CivvisCoverQueue.listed(cfg.HotseatSeats, pid)
		and not CivvisCoverQueue.listed(cfg.HotseatHumanSeats, pid);
end
function CivvisCoverQueue.params(params)
	local out = {};
	if type(params) == "table" then
		for key, value in pairs(params) do out[key] = value; end
	end
	out.AlwaysVisibleInQueue = true;
	return out;
end
function CivvisCoverQueue.install()
	if not CivvisCoverQueue.on() then return false, "off"; end
	local real = UIManager;
	local kind = type(real);
	if kind ~= "table" and kind ~= "userdata" then return false, "no_uimanager"; end
	local queue = nil;
	pcall(function() queue = real.QueuePopup; end);
	if type(queue) ~= "function" then return false, "no_queue"; end
	local proxy = nil;
	local bound = {};
	local function queuePopup(self, context, priority, params, ...)
		local pass = params;
		local ok, want = pcall(function()
			return context == ContextPtr and CivvisCoverQueue.wanted(NAME, Game.GetLocalPlayer());
		end);
		if ok and want then
			pass = CivvisCoverQueue.params(params);
			CivvisCoverQueue.visible = CivvisCoverQueue.visible + 1;
			report("cover_queue_visible", string.format(',"count":%d', CivvisCoverQueue.visible));
		end
		-- Whose turn queued it, for `CivvisCoverQueue.leftBehind`.
		if context == ContextPtr then
			pcall(function() CivvisCoverQueue.owner = Game.GetLocalPlayer(); end);
		end
		if self == proxy then self = real; end
		return queue(self, context, priority, pass, ...);
	end
	proxy = setmetatable({ QueuePopup = queuePopup }, {
		__index = function(_, key)
			local value = real[key];
			if type(value) ~= "function" then return value; end
			if bound[key] == nil then
				bound[key] = function(self, ...)
					if self == proxy then return value(real, ...); end
					return value(self, ...);
				end;
			end
			return bound[key];
		end,
		__newindex = function(_, key, value) real[key] = value; end,
	});
	local direct, through = nil, nil;
	local readDirect = pcall(function() direct = real:IsInPopupQueue(ContextPtr); end);
	local readThrough = pcall(function() through = proxy:IsInPopupQueue(ContextPtr); end);
	if not readThrough or (readDirect and direct ~= through) then return false, "proxy_refused"; end
	local swapped = pcall(function() UIManager = proxy; end);
	local current = nil;
	pcall(function() current = UIManager; end);
	if not swapped or current ~= proxy then
		pcall(function() UIManager = real; end);
		return false, "global_not_replaced";
	end
	CivvisCoverQueue.installed = true;
	return true, kind;
end
-- ★ AND IT IS LET UP. Two of these screens ask before they queue themselves
-- whether a popup of their priority can be shown now, and when it cannot they
-- keep the announcement in a list of their own and show it later, on
-- `Events.UIIdle`:
--   HistoricMoments.lua:163-166   if not UI.CanShowPopup(GetPopupPriority()) then
--                                     table.insert(m_kQueuedPopups, popupData); return;
--   HistoricMoments.lua:520-524   OnUIIdle: if UI.CanShowPopup(...) then ShowNextQueuedPopup()
--   BoostUnlockedPopup.lua:256-258 CanShow: UI.CanShowPopup(PopupPriority.Low) and m_isOnQueue==false
--   BoostUnlockedPopup.lua:352-357 OnUIIdle, the same way
--   EventPopup.lua:303-306         OnUIIdle: if UI.CanShowPopup(PopupPriority.Medium) then
--                                     ShowNextQueuedPopup(); its second popup and later
--                                     wait in its own list (:39-46), served only in
--                                     hotseat (install.py HOTSEAT_EVENT_POPUP)
-- Under the cover the answer is no for the whole of a CIVVIS seat's turn (the
-- cover is a higher popup), so the announcement was never queued, never drawn
-- under the cover and never closed: it waited in the screen's own list until
-- the cover lifted and then opened on the PERSON's turn. Live 2026-11-27
-- (hotseat-20261128T031456Z, five CIVVIS seats): each seat's first Historic
-- Moment came up one after another at the start of the person's turn 2, drawn
-- for the local player, so each read "Your civilization hasn't experienced any
-- Historic Moments." The same game's stand-in run parked on its turn 2 behind
-- them. So on a CIVVIS seat's turn in a cover game these screens are told yes:
-- they queue themselves (the proxy above keeps them drawn under the cover) and
-- are closed as they were before the cover existed. On every other turn the
-- game's own answer stands. `UI` is read with a dot everywhere in both scripts
-- (no `UI:` call), so a table that answers this one question and reads every
-- other name from the real one is the whole proxy.
CivvisCoverQueue.ASKS = { HistoricMoments = true, BoostUnlockedPopup = true, EventPopup = true };
CivvisCoverQueue.let = 0;
function CivvisCoverQueue.installAsk()
	if not CivvisCoverQueue.on() or not CivvisCoverQueue.ASKS[NAME] then return false, "off"; end
	local real = UI;
	local kind = type(real);
	if kind ~= "table" and kind ~= "userdata" then return false, "no_ui"; end
	local can = nil;
	pcall(function() can = real.CanShowPopup; end);
	if type(can) ~= "function" then return false, "no_can_show"; end
	local function canShowPopup(...)
		local ok, want = pcall(function()
			return CivvisCoverQueue.wanted(NAME, Game.GetLocalPlayer());
		end);
		if ok and want then
			CivvisCoverQueue.let = CivvisCoverQueue.let + 1;
			return true;
		end
		return can(...);
	end
	local proxy = setmetatable({ CanShowPopup = canShowPopup }, {
		__index = function(_, key) return real[key]; end,
		__newindex = function(_, key, value) real[key] = value; end,
	});
	-- Used only if every other name reads through to the real one.
	local through = false;
	pcall(function() through = proxy.PlaySound == real.PlaySound; end);
	if not through then return false, "proxy_refused"; end
	local swapped = pcall(function() UI = proxy; end);
	local current = nil;
	pcall(function() current = UI; end);
	if not swapped or current ~= proxy then
		pcall(function() UI = real; end);
		return false, "global_not_replaced";
	end
	return true, kind;
end
if CivvisCoverQueue.on() then
	local ran, installed, how = pcall(CivvisCoverQueue.install);
	report("cover_queue", string.format(',"installed":%s,"how":"%s"',
		tostring(ran and installed == true), (tostring(ran and how or installed):gsub('["\\]', "'"))));
	if CivvisCoverQueue.ASKS[NAME] then
		ran, installed, how = pcall(CivvisCoverQueue.installAsk);
		report("cover_queue_ask", string.format(',"installed":%s,"how":"%s"',
			tostring(ran and installed == true), (tostring(ran and how or installed):gsub('["\\]', "'"))));
	end
end

-- One screen is already replaced by a DLC, on a criterion true of every run
-- here: GranColombia_Maya swaps NaturalDisasterPopup for fourteen lines that
-- add a comet-strike label. Two ReplaceUIScript actions on one context is a
-- race, and the mod that loads later wins it -- measured, eight screens armed
-- and this one did not. CivvisControl.modinfo now references that mod so this
-- one loads after it; this then loads *their* file rather than the shipped
-- one, so their comet-strike label survives. Their file opens with
-- include("NaturalDisasterPopup"), which is this same pattern, from Firaxis.
--
-- The same table also covers the other case where the live script is not the one
-- named after the context.
--
-- ⚠ Gathering Storm is Expansion2 and ships a *Replacement* for the diplomacy
-- view, so `include("DiplomacyActionView")` pulls the BASE file and misses
-- everything the expansion changed, including its own `Close`. Chain to the
-- replacement, which includes Expansion1, which includes the base.
local CHAINED = {
	NaturalDisasterPopup = "NaturalDisasterPopup_GranColombia_Maya",
	DiplomacyActionView = "DiplomacyActionView_Expansion2",
	-- Sukritact's Simple UI Adjustments is a later replacement in the stock
	-- install. It includes the Firaxis popup and only changes the wonder audio;
	-- retaining it here prevents the high-load-order closer from regressing that
	-- user-visible behavior while still adding the native close timer.
	WonderBuiltPopup = "Suk_WonderBuiltPopup",
};

-- Whether a screen is in there at all. An include that finds no file fails
-- silently on this build, so the test has to be for what the script defines
-- rather than for the include returning.
--
-- ⚠⚠ THESE MUST BE BARE GLOBAL REFERENCES, NOT `_G[name]` LOOKUPS.
--
-- A previous version of this file kept the names in a table and tested them with
-- `type(_G[name]) == "function"` so that this check and `endScreen` could share
-- one list. It broke every screen: each Civilization VI UI context runs in its
-- own environment, so `_G["OnClose"]` does not resolve the same name that a bare
-- `OnClose` resolves. The run went from three unarmed screens to EIGHT, taking
-- EraCompletePopup, NaturalWonderPopup, NaturalDisasterPopup, WonderBuiltPopup
-- and RockBandMoviePopup with it — all of which had been arming correctly for
-- hours. Found by the `autoclose_unarmed` line in the event stream.
--
-- So the list is spelled out. It has to stay in step with the ladder in
-- `endScreen` by hand, and tools/civ6_control/check_closers.py fails if it
-- drifts or if anybody reaches for `_G` again.
local function haveScreen()
	return type(OnClose) == "function"
		or type(Close) == "function"
		or type(OnCancel) == "function"           -- EspionagePopup
		or type(OnContinue) == "function"
		or type(OnPass) == "function"             -- WorldCongressPopup emergency proposal
		or type(OnClosePopup) == "function"
		or type(OnHideScreen) == "function"        -- GreatWorkShowcase
		or type(OnButton1) == "function"           -- ChooseArtifact
		or type(OnButton4) == "function"           -- EspionageEscape city-center route
		or type(ReleaseEventLock) == "function"    -- WorldCongressBetweenTurns
		or type(OnAccept) == "function"            -- WorldCongressPopup
		or type(CloseFocusedState) == "function"   -- DiplomacyActionView
		or type(OnRefuseDeal) == "function"        -- DiplomacyDealView
		or type(OnSelectConversationDiplomacyStatement) == "function"  -- leader asking
		or type(OnSelectInitialDiplomacyStatement) == "function"       -- opening statement
		or type(ExitConversationMode) == "function"
		or type(OnHide) == "function"
		or type(InputHandler) == "function";
end

-- The shipped screen, unchanged and unread. A context's id is the name of the
-- script the game loads into it, so asking for that name asks for exactly the
-- file this replacement stands in front of.
-- ★ QUICK DEALS. The Workshop mod Quick Deals (2460661464, the owner's, on this
-- Mac) replaces this same context at `<LoadOrder>99999</LoadOrder>`
-- (QuickDeals.modinfo `qd_diplomacyactionview`), so it, not this file, was the
-- leader screen in every hotseat game here (28 runs: `autoclose_armed` for
-- DiplomacyDealView, never for DiplomacyActionView; Modding.log applies
-- `qd_diplomacyactionview` after `CivvisControlAutoCloseDiplomacyActionView`).
-- A hotseat install now sits above it (install.py `HOTSEAT_LOAD_ORDER`), and so
-- loads it first where it is installed: its file includes the stock chain
-- itself (diplomacyactionview_qd.lua:4-16, Expansion2 first) and keeps its own
-- deal screen working (:28-57), which the shipped chain alone would drop.
if NAME == "DiplomacyActionView" then pcall(function() include("diplomacyactionview_qd"); end); end
if CHAINED[NAME] then
	if type(QD_BASE_LateInitialize) ~= "function" then pcall(function() include(CHAINED[NAME]); end); end
end
if not haveScreen() then pcall(function() include(NAME); end); end

-- ReplaceUIScript bypasses the expansion add-ons unless we chain them too.
-- Expansion2's Styles["VICTORY_DIPLOMATIC"] selects XP2Victory_Diplomatic.bk2.
if END_SCREENS[NAME] then
    if type(OnHistoricMoments) ~= "function" then
        pcall(function() include("EndGameMenu_Expansion1"); end);
    end
    if type(Styles) == "table" and Styles["VICTORY_DIPLOMATIC"] == nil then
        pcall(function() include("EndGameMenu_Expansion2"); end);
    end
end

-- The person-facing install outside a hotseat game: the shipped screen, as
-- loaded above, and nothing of ours after it -- no wrapper, no clock, no line.
if CivvisHarness.dormant() then return; end

-- EndGameMenu.lua:1328 owns the natural movie-finished callback. Keep its
-- handler and report the boundary so the harness can wait for the full movie.
-- A result screen is never an ordinary popup to dismiss or Escape through.
if END_SCREENS[NAME] and Controls ~= nil and Controls.Movie ~= nil then
    local nativeMovieFinished = OnMovieExitOrFinished;
    if type(nativeMovieFinished) == "function" then
        Controls.Movie:SetMovieFinishedCallback(function()
            report("victory_movie_finished");
            nativeMovieFinished();
        end);
    end
    local nativeReplay = OnReplayMovie;
    if type(nativeReplay) == "function" then
        OnReplayMovie = function(...)
            nativeReplay(...);
            if Controls.MovieFill ~= nil and Controls.MovieFill:IsVisible() then
                report("victory_movie_started");
            end
        end;
        if Controls.ReplayMovieButton ~= nil then
            Controls.ReplayMovieButton:RegisterCallback(Mouse.eLClick, OnReplayMovie);
        end
    end
end

-- First contact is an information decision. Capture the actual, filtered
-- selections through the shipped handler (DiplomacyActionView.lua:566-605).
-- DiplomacySelections FIRST_MEET_VISIT_RECIPIENT offers hospitality at THEIR
-- city; NEAR_INITIATOR offers OUR city and NO_MANS offers mutual disclosure.
local firstMeetChoice = nil;
local firstMeetText = nil;
local firstMeetAnswered = false;
local dialogueObserved = false;
local dialogueChoice = nil;
local dialogueCallback = nil;
local dialogueAnswered = false;
local dialogueContext = "";
local dialogueWarWithheld = false;
local function responseString(value)
    if type(value) ~= "string" then return "null"; end
    return '"' .. string.gsub(value, '[%z\1-\31\\"]', function(c)
        return string.format('\\u%04x', string.byte(c));
    end) .. '"';
end
local function responsePlayer(value)
    return type(value) == "number" and value % 1 == 0 and tostring(value) or "null";
end
-- Gene `dialogue-never-declares-war`. Civ VI binds some answers to a war:
-- WARNING_TOO_MANY_TROOPS_NEAR_ME_FROM_AI's CHOICE_NEGATIVE carries
-- DIPLOACTION_DECLARE_SURPRISE_WAR (DiplomacyStatements_Warning.xml), and the
-- NEGATIVE-first ladder below picked it: live civvis-20261005T045443Z went to
-- surprise war with Korea at t94 while the board was holding off. When the
-- brain selects the gene, each order batch renews a lease through
-- `LuaEvents.CivvisDialogueNoWar(turn)`. It covers the next turn too, because
-- a statement waiting at turn start is answered before that turn's first
-- batch lands. Without a lease nothing below changes.
local dialogueNoWarTurn = nil;
if NAME == "DiplomacyActionView" then
    pcall(function()
        LuaEvents.CivvisDialogueNoWar.Add(function(turn)
            dialogueNoWarTurn = tonumber(turn);
        end);
    end);
end
local function dialogueNoWar()
    if dialogueNoWarTurn == nil then return false; end
    local ok, now = pcall(function() return Game.GetCurrentGameTurn(); end);
    -- An unreadable turn keeps the lease: refusing a war is the safe side.
    if not ok or type(now) ~= "number" then return true; end
    return now <= dialogueNoWarTurn + 1;
end
-- Gene `score-seat-keeps-friends` (opt-in). A declared friendship forbids a
-- war between the pair while it stands; no rival declared on our seats while
-- one stood, yet the ladder below answered every offered DECLARE_FRIEND with
-- CHOICE_NEGATIVE (52 in one night of Score games). Under this lease, which
-- each order batch renews like the no-war lease, a major's offer is answered
-- with its CHOICE_POSITIVE (DiplomacyStatements_DeclareFriendship.xml:82-83,
-- LOC_DIPLO_DECLARE_FRIEND_CHOICE_POSITIVE), unless it comes from the player
-- the batch names as spared (the campaign's target, or a converter whose faith
-- stands at match point; a comma list). The bridge sends the lease on every
-- seat but a Domination one. Without a lease nothing below changes.
local keepsFriendsTurn, keepsFriendsSpared = nil, {};
if NAME == "DiplomacyActionView" then
    pcall(function()
        LuaEvents.CivvisDialogueKeepsFriends.Add(function(turn, spared)
            keepsFriendsTurn = tonumber(turn);
            -- A comma list of host players (a lone number reads the same).
            keepsFriendsSpared = {};
            for id in string.gmatch(tostring(spared or ""), "%d+") do
                keepsFriendsSpared[tonumber(id)] = true;
            end
        end);
    end);
end
-- "accept", "spared" or nil (no lease, or a stale one).
local function keepsFriendsFrom(player)
    if keepsFriendsTurn == nil then return nil; end
    local ok, now = pcall(function() return Game.GetCurrentGameTurn(); end);
    if not ok or type(now) ~= "number" or now > keepsFriendsTurn + 1 then return nil; end
    if keepsFriendsSpared[player] then return "spared"; end
    return "accept";
end
local dialogueKeepsFriends = nil;
local function declaresWar(selection)
    local action = selection.DiplomaticActionType;
    return type(action) == "string"
        and string.find(action, "^DIPLOACTION_DECLARE_.*WAR$") ~= nil;
end
if NAME == "DiplomacyActionView" and type(DefaultHandlers) == "table" then
    local stockApply = ApplyStatement;
    ApplyStatement = function(handler, statementType, subType, toPlayer, statement)
        firstMeetChoice, firstMeetText = nil, nil;
        firstMeetAnswered = false;
        dialogueObserved, dialogueChoice, dialogueCallback, dialogueAnswered = false, nil, nil, false;
        -- Shipped ApplyStatement passes the response subtype and actors to
        -- ExtractStatement (DiplomacyActionView.lua:566-569). Both delegation
        -- acceptance and refusal offer EXIT; the choice alone cannot tell them apart.
        dialogueContext = string.format(',"statement":%s,"subtype":%s,"from_player":%s,"to_player":%s,"initiator":%s',
            responseString(statementType), responseString(subType),
            responsePlayer(statement.FromPlayer), responsePlayer(toPlayer),
            responsePlayer(statement.Initiator));
        dialogueWarWithheld = false;
        dialogueKeepsFriends = nil;
        stockApply(handler, statementType, subType, toPlayer, statement);
        local firstMeet = type(statementType) == "string"
            and string.find(statementType, "^FIRST_MEET_") ~= nil;
        local parsed = handler.ExtractStatement(handler, statementType, subType,
            statement.FromPlayer, GetStatementMood(statement.FromPlayer, statement.FromPlayerMood),
            statement.Initiator);
        local localPlayer = Game.GetLocalPlayer();
        local otherPlayer = statement.FromPlayer == localPlayer and toPlayer or statement.FromPlayer;
        handler.RemoveInvalidSelections(parsed, localPlayer, otherPlayer);
        if not firstMeet then
            -- Match the displayed, enabled options and their own callback.
            -- Prefer answering a request to leaving it pending via EXIT.
            -- Welcome the two visitor offers the shipped game identifies by
            -- these exact labels (DiplomacyStatements_Delegation.xml:198-203,
            -- DiplomacyStatements_Embassy.xml:198-203). Live Pericles 151846Z
            -- otherwise refused both while trying to keep a peaceful Culture
            -- lane. Other requests keep the conservative decline policy.
            dialogueObserved = true;
            -- `DealCounterRefuse` (opt-in): the agent tells a refusal shown as
            -- conversation from one left on the deal screen.
            if cfg.DealCounterRefuse == true then
                pcall(function()
                    LuaEvents.CivvisDealDialogue(statement.FromPlayer, statementType, subType);
                end);
            end
            local rank = { CHOICE_NEGATIVE = 3, CHOICE_IGNORE = 2, CHOICE_EXIT = 1 };
            local visitorText = statementType == "DIPLOMATIC_DELEGATION"
                and "LOC_DIPLO_DELEGATION_CHOICE_POSITIVE"
                or statementType == "RESIDENT_EMBASSY"
                and "LOC_DIPLO_EMBASSY_CHOICE_POSITIVE" or nil;
            local best = 0;
            local noWar = dialogueNoWar();
            -- `score-seat-keeps-friends`: a major's offered friendship.
            local keepFriend = statementType == "DECLARE_FRIEND"
                and statement.Initiator ~= localPlayer
                and keepsFriendsFrom(statement.FromPlayer) or nil;
            dialogueKeepsFriends = keepFriend;
            for _, selection in ipairs(parsed.Selections or {}) do
                local score = rank[selection.Key] or 0;
                if keepFriend == "accept" and selection.Key == "CHOICE_POSITIVE"
                        and selection.Text == "LOC_DIPLO_DECLARE_FRIEND_CHOICE_POSITIVE"
                        and not declaresWar(selection) then
                    score = 4;
                end
                if visitorText ~= nil and selection.Key == "CHOICE_POSITIVE"
                        and selection.Text == visitorText and not declaresWar(selection) then
                    score = 4;
                end
                if noWar and score > 0 and not selection.IsDisabled and declaresWar(selection) then
                    dialogueWarWithheld = true;
                elseif not selection.IsDisabled and score > best then
                    dialogueChoice, best = selection.Key, score;
                end
            end
            dialogueCallback = handler.OnSelectionButtonClicked;
            return;
        end
        -- A Domination seat trades capitals: its opening needs the nearest
        -- rival capital charted, and a rival's closed borders keep a scout out
        -- (live Macedon met a same-land rival at t30 and first saw its capital,
        -- 24 tiles away, at t97). DiplomacySelections FIRST_MEET_NO_MANS answers
        -- the greeting with CHOICE_POSITIVE LOC_DIPLO_CHOICE_NO_MANS_POSITIVE;
        -- FIRST_MEET_NO_MANS_INFO_EXCHANGE then offers CHOICE_POSITIVE
        -- LOC_DIPLO_CHOICE_FIRST_MEET_EXCHANGE_INFO ("Exchanging information on
        -- our capitals"). Both are taken only with the baked `VictoryLane`; any
        -- other lane, or none, keeps the capital private.
        local shareCapital = cfg.VictoryLane == "domination";
        for _, selection in ipairs(parsed.Selections or {}) do
            if not selection.IsDisabled then
                -- Greeting their nearby scout reveals nothing; the follow-up
                -- invitation is accepted. Our invitation always exits.
                local safePositive = selection.Text == "LOC_DIPLO_CHOICE_FIRST_MEET_VISIT"
                    or selection.Text == "LOC_DIPLO_CHOICE_FIRST_MEET_NEAR_RECIPIENT_POSITIVE"
                    or (shareCapital and (selection.Text == "LOC_DIPLO_CHOICE_NO_MANS_POSITIVE"
                        or selection.Text == "LOC_DIPLO_CHOICE_FIRST_MEET_EXCHANGE_INFO"));
                if selection.Key == "CHOICE_EXIT" and firstMeetChoice == nil then
                    firstMeetChoice, firstMeetText = selection.Key, selection.Text;
                elseif selection.Key == "CHOICE_POSITIVE" and safePositive then
                    firstMeetChoice, firstMeetText = selection.Key, selection.Text;
                end
            end
        end
    end;
    for _, handler in pairs(StatementHandlers or {}) do
        if handler.ApplyStatement == stockApply then handler.ApplyStatement = ApplyStatement; end
    end
end

-- A stock diplomacy context can be visible before its own InitializeView has
-- completed. In that state CloseFocusedState() calls Close(), but the stock
-- UninitializeView() returns before ContextPtr:SetHide(true), so every native
-- close path can report success while the context remains over the map. This
-- is not a dialogue to answer: the action view has no active session, and the
-- deal view has no open session to reject. Prove both facts, give the shipped
-- close path one last chance, then hide only that stale, session-less context.
-- A failed state read is treated as unsafe and leaves the desktop backstop in
-- charge; this must never become a blind click equivalent.
local function closeStaleDiplomacyContext()
	if NAME ~= "DiplomacyActionView" and NAME ~= "DiplomacyDealView" then
		return false;
	end

	local visible = false;
	if not pcall(function() visible = not ContextPtr:IsHidden(); end) or not visible then
		return false;
	end

	local function sessionStillOpen()
		if NAME == "DiplomacyActionView" then
			-- Base/Assets/UI/DiplomacyActionView.lua:299-301 validates the
			-- remembered ID with IsSessionIDOpen before treating it as live.
			-- An ended session can retain its ID while the view fades out.
			if ms_ActiveSessionID ~= nil then
				local open = DiplomacyManager.IsSessionIDOpen(ms_ActiveSessionID);
				if type(open) ~= "boolean" then error("session state unavailable"); end
				return open;
			end
			return false;
		elseif g_OtherPlayer ~= nil then
			-- The deal view keeps the other-player handle global. Require its
			-- native session lookup to prove that the session is gone.
			local sessionID = DiplomacyManager.FindOpenSessionID(
				Game.GetLocalPlayer(), g_OtherPlayer:GetID());
			return sessionID ~= nil;
		end
		return false;
	end
	local sessionReadable, sessionOpen = pcall(sessionStillOpen);
	if not sessionReadable or sessionOpen then return false; end

	-- A visible native popup is still an actionable state. Let its own ladder
	-- consume it instead of hiding its parent context underneath it. The action
	-- view's popup handle is global in the shipped script; the deal view keeps
	-- its handle local, so its session check above is the available proof.
	if NAME == "DiplomacyActionView" and m_PopupDialog ~= nil then
		local popupOpen = false;
		local popupReadable = pcall(function() popupOpen = m_PopupDialog:IsOpen(); end);
		if not popupReadable or popupOpen then return false; end
	end

	local nativeClose = false;
	if NAME == "DiplomacyActionView" and type(Close) == "function" then
		nativeClose = pcall(Close);
	elseif NAME == "DiplomacyDealView" and type(OnContinue) == "function" then
		nativeClose = pcall(OnContinue);
	else
		return false;
	end

	local hidden = false;
	local hiddenReadable = pcall(function() hidden = ContextPtr:IsHidden(); end);
	if not hiddenReadable then return false; end
	if hidden then return true; end
	-- Close() can synchronously advance the queue to a different session.
	-- Never hide that replacement using the earlier session's observation.
	sessionReadable, sessionOpen = pcall(sessionStillOpen);
	if not sessionReadable or sessionOpen then return false; end

	-- The native path above is deliberately first: this branch is only for the
	-- stock uninitialized-context bug, where UninitializeView returned false.
	local hideCalled = pcall(function() ContextPtr:SetHide(true); end);
	local hiddenAfterFallback = false;
	local fallbackReadable = pcall(function() hiddenAfterFallback = ContextPtr:IsHidden(); end);
	if hideCalled and fallbackReadable and hiddenAfterFallback then
		report("autoclose_stale_hide", string.format(',"native_close":%s', tostring(nativeClose)));
		return true;
	end
	return false;
end

-- InGamePopup is the one context here that renders more than one kind of
-- thing. Every generic in-game dialog goes through it: "your unit has been
-- captured", which has a single button and asks nothing, and "raze or keep
-- this city?", which has two and asks everything. Only the first may be closed
-- on a timer, so this screen arms per *dialog* rather than per context --
-- count the buttons as each one opens, and arm only when there is exactly one.
--
-- The wrapper has to be re-registered, not just assigned: the shipped
-- Initialize already handed LuaEvents the original function, and reassigning
-- the global does not change what the event calls. If the swap cannot be made
-- cleanly the dialog is simply never armed, which leaves this screen exactly
-- as it ships.
-- The shipped constant, read off `PopupDialog` when it is loaded and named
-- literally when it is not. `InGamePopup.lua` opens with
-- `include("PopupDialog")` and this file includes `InGamePopup` above, so the
-- table is normally there; the literal is what that constant has always been
-- and keeps the rule working rather than silently disarming it.
local CANCEL_COMMAND = "_CMD_CANCEL";
if type(PopupDialog) == "table" and type(PopupDialog.COMMAND_CANCEL) == "string" then
	CANCEL_COMMAND = PopupDialog.COMMAND_CANCEL;
end

-- ★★★★ WHICH GENERIC DIALOGS MAY BE ESCAPED, AND WHY THESE.
--
-- Returns `dismissable, buttons` for an option list raised through
-- `LuaEvents.OnRaisePopupInGame`. Two shapes qualify and no third:
--
--   * **one button** — an acknowledgement. UNIT CAPTURED. Nothing is asked, so
--     nothing is answered by ending it. This was the whole rule until now.
--   * **any button carrying `_CMD_CANCEL`** — a dialog whose own author wrote a
--     decline path. `PopupDialogInGame:AddCancelButton` tags its option with
--     `PopupDialog.COMMAND_CANCEL`, and the shipped `InGamePopup.InputHandler`
--     maps Escape to `ActivateCommand(COMMAND_CANCEL)` first. Escape therefore
--     runs that author's cancel callback, which is `nil` in every shipped
--     caller — the definition of dismissing without consequence.
--
-- ⚠⚠ THE REASON THE OLD RULE GAVE FOR REFUSING TWO BUTTONS IS ABOUT A
-- DIFFERENT SCREEN. It cited "raze or keep this city ... has two and asks
-- everything". Raze/keep is `RazeCity.lua` with its own `RazeCity.xml`, queued
-- through `UIManager:QueuePopup` and holding its own input handler; it never
-- reaches `PopupDialogInGame` and nothing here can touch it. Every shipped
-- `PopupDialogInGame:new` caller with a CANCEL is a confirmation of an action a
-- person just took in a panel — `UnitPanel`'s delete, `WorldInput`'s WMD
-- launch, `GovernmentScreen`'s anarchy switch, `GovernorAssignmentChooser`'s
-- replacement, `StrategicView_MapPlacement`'s pin. This controller takes none
-- of them through a panel; it issues the operations directly. A confirmation
-- reaching an unattended seat is a dialog nothing was waiting on, and declining
-- it gives the map back.
--
-- ⚠ A dialog with two buttons and NO cancel is a forced choice and is left
-- exactly as it ships. That is the line, and it is read off the data the
-- dialog carries rather than off a count.
--
-- ⚠ A bare global rather than a local, because it is the offline test's only
-- way in — the same convention as `CivvisResidualBucket` in the agent. Never
-- `_G.`: the sandbox has none.
CivvisDialogDismissable = function(options)
	local buttons = 0;
	local declinable = false;
	local seen = {};
	if type(options) == "table" then
		for _, option in ipairs(options) do
			if option.Type == "Button" then
				buttons = buttons + 1;
				local command = option.CommandString;
				if command ~= nil then
					seen[#seen + 1] = tostring(command);
					if command == CANCEL_COMMAND then declinable = true; end
				end
			end
		end
	end
	return (buttons == 1) or declinable, table.concat(seen, "+");
end

local dialogIsAnnouncement = false;
-- What the open dialog's buttons were, as the shipped command strings joined by
-- '+' ("" when it named none). Reported with the close so the population of
-- dialogs that actually reach an unattended run becomes a census instead of a
-- guess: `InGamePopup` is one context rendering every generic dialog in the
-- game, so `screen:"InGamePopup"` on its own names nothing.
local dialogButtons = "";

if NAME == "InGamePopup" and type(OnPopupOpen) == "function" then
	local basePopupOpen = OnPopupOpen;
	local function countingPopupOpen(id, options)
		dialogIsAnnouncement, dialogButtons = CivvisDialogDismissable(options);
		return basePopupOpen(id, options);
	end
	local swapped = pcall(function()
		LuaEvents.OnRaisePopupInGame.Remove(basePopupOpen);
		LuaEvents.OnRaisePopupInGame.Add(countingPopupOpen);
	end);
	if swapped then OnPopupOpen = countingPopupOpen; end
end

-- WorldCongressPopup.lua:78-88 opens voting only in the two Congress
-- segments. The same context also shows reviews with old resolutions still
-- readable, so a later-turn review must not request another agent ballot.
local function requestCongressBallot()
	local ok, voting = pcall(function()
		local segment = Game.GetCurrentTurnSegment();
		if segment == nil then return false; end
		return segment == DB.MakeHash("TURNSEG_WORLDCONGRESS_1")
			or segment == DB.MakeHash("TURNSEG_WORLDCONGRESS_2");
	end);
	if ok and voting then
		pcall(function() LuaEvents.CivvisCongressBallot(); end);
	end
end

-- What a click on this screen's own button does. Everything shipped registers
-- OnClose; the era review is the exception explained at the top.
local function endScreen(attempt)
    if NAME == "DiplomacyActionView" and dialogueChoice ~= nil
            and not dialogueAnswered and type(dialogueCallback) == "function" then
        local choice, callback = dialogueChoice, dialogueCallback;
        -- Dispatch can install a follow-up synchronously. Record the response
        -- being answered, including its war guard, before invoking its callback.
        local context = dialogueContext .. (dialogueWarWithheld and ',"war_withheld":true' or '')
            .. (dialogueKeepsFriends ~= nil
                and string.format(',"keeps_friends":"%s"', dialogueKeepsFriends) or '');
        dialogueAnswered = true;
        local ok = pcall(function() callback(choice); end);
        -- A callback may synchronously replace this statement. Do not mark
        -- the replacement answered, or close it on its predecessor's tick.
        if not ok and dialogueCallback == callback and dialogueChoice == choice then
            dialogueAnswered = false;
        end
        report("diplomacy_choice", string.format(',"choice":"%s","sent":%s%s',
            choice, tostring(ok), context));
        return ok;
    end
    if NAME == "DiplomacyActionView" and firstMeetChoice ~= nil then
        if not firstMeetAnswered then
            -- Clear the flag before dispatch: AddResponse can synchronously
            -- install the next statement through ApplyStatement.
            firstMeetAnswered = true;
            local choice = firstMeetChoice;
            -- Name the statement and the chosen line before dispatch replaces them.
            local context = dialogueContext .. ',"text":' .. responseString(firstMeetText);
            local ok = pcall(function() OnSelectConversationDiplomacyStatement(choice); end);
            if not ok then firstMeetAnswered = false; end
            report("first_meet_choice", string.format(',"choice":"%s","sent":%s%s',
                choice, tostring(ok), context));
            return ok;
        end
        -- A dispatched response can leave the context visible. Let Firaxis
        -- finish closing it on the next tick, without answering twice or
        -- falling through to the generic NEGATIVE/IGNORE response ladder.
        -- Do not close on the dispatch tick: ApplyStatement may synchronously
        -- install a follow-up invitation with its own unanswered choice.
        return type(CloseFocusedState) == "function"
            and pcall(function() CloseFocusedState(true); end);
    end
	-- WonderBuiltPopup is a readable completion announcement, not a choice.
	-- Keep its own Firaxis close path explicit: the shipped context defines
	-- OnClose as a wrapper around Close(), and Close() also drains a queued
	-- second wonder before releasing the exclusive popup lock.  This branch is
	-- intentionally before the generic handlers so a future context adds no
	-- ambiguity about which completion screen is being closed.
	if NAME == "WonderBuiltPopup" then
		if type(OnClose) == "function" then OnClose(); return true; end
		if type(Close) == "function" then Close(); return true; end
		return false;
	end
	if NAME == "InGamePopup" then
		-- The shipped Escape path rather than a bare close, so the single
		-- button's own callback runs. Escape tries CANCEL, then DEFAULT, then
		-- gives up and closes -- which is exactly what a person pressing it
		-- would get, and on a one-button dialog there is nothing else it can
		-- reach.
		if type(InputHandler) == "function" then
			InputHandler(KeyEvents.KeyUp, Keys.VK_ESCAPE, 0);
			return true;
		end
		if type(OnClosePopup) == "function" then OnClosePopup(); return true; end
		return false;
	end
	-- Espionage's briefing/result card is informational after the controller has
	-- already requested the operation. Its Cancel callback just dequeues the
	-- popup and returns to the map: it neither starts a new mission nor aborts an
	-- active one. Use that real UI path rather than calling Close indirectly.
	if NAME == "EspionagePopup" and type(OnCancel) == "function" then
		OnCancel();
		return true;
	end
	-- A spy that must escape is different: its plain OnClose only hides the
	-- panel, leaving the end-turn blocker unanswered so the screen comes back.
	-- The shipped fourth button routes through the city center, which is the one
	-- route it always enables, and submits SET_ESCAPE_ROUTE before hiding.
	if NAME == "EspionageEscape" and type(OnButton4) == "function" then
		OnButton4();
		return true;
	end
	-- A military-emergency proposal is raised by WorldCongressPopup itself,
	-- not WorldCongressIntro or WorldCongressBetweenTurns.  Its shipped Pass
	-- button calls OnPass: ClosePopup alone hides the panel but leaves the
	-- special-session notification pending, so the same screen can immediately
	-- return.  Use the complete Firaxis path a person pressing Pass would use.
	if NAME == "WorldCongressPopup" and type(OnPass) == "function" then
		-- ⚠⚠ THIS IS THE RUNG THAT RUNS FOR THE SESSION POPUP TOO. The shipped
		-- WorldCongressPopup defines BOTH `OnPass` (the emergency-proposal
		-- review's Pass button) and `OnAccept` (the session's submit), and this
		-- rung sits first — so the `OnAccept` rung below, and the ballot event
		-- it raises, never ran for a session: batch-9 game
		-- civvis-20260816T223457Z shows the popup closing here 0.05 s after it
		-- opened at t61/81/101/121 with no `source:"popup"` ballot anywhere.
		-- The ballot is raised here as well; the agent's handler ignores a
		-- second call in the same turn. The phase guard below excludes reviews
		-- even when the host still exposes the previous session's resolutions.
		requestCongressBallot();
		OnPass();
		return true;
	end
	if NAME == "EraReviewPopup" and type(OnContinue) == "function" then
		OnContinue();
		return true;
	end
	-- ★★★★★ THE WORLD CONGRESS HOLDS THE GAME UNTIL THE SEAT SUBMITS ITS TURN.
	--
	-- A special session demands a thumbs vote on EVERY proposal before its Next
	-- button unlocks ("You must vote on all submitted Proposals"), then a
	-- confirm page — and no generic closer touches any of it: batch-4 attempt 2
	-- (run civvis-20260801T211015Z) stalled at t221 for twenty minutes until
	-- the watchdog killed the attempt, with a healthy empire on the board.
	--
	-- The screen's own OnAccept() is the whole exit. With no votes cast it
	-- logs a DataError and then, deliberately, still submits the empty congress
	-- turn (WORLD_CONGRESS_SUBMIT_TURN), requests ACTION_ENDTURN, stages the
	-- between-turns banner and closes itself — the same abstain a person gets
	-- by committing zero favor. Abstaining IS a decision; it is the congress
	-- shape of the decline-everything policy every diplomacy closer here
	-- already applies, and the measured alternative is a dead attempt.
	if NAME == "WorldCongressPopup" and type(OnAccept) == "function" then
		-- ★★★★★ THE AGENT'S BALLOT IS CAST HERE, NOW — THE MOMENT A PERSON VOTES.
		--
		-- The agent used to vote when it first saw the WORLD_CONGRESS_SESSION
		-- blocker, before this popup (or even the intro) had opened, and every
		-- one of those votes was silently refused: across
		-- civvis-20260816T184500Z and T205104Z the `wc_vote` ledger read
		-- `spent 760 / 924 / 420` while Favor never fell (822→829→836), and the
		-- `wc_outcome` review showed our selection on EVERY resolution — the
		-- diplomatic-victory one included — as `option 1, votes 1`: the core's
		-- default free vote, cast FOR the leader gaining two points. The
		-- shipped screen votes from inside this popup in stage 1 and submits
		-- with `OnAccept`; so the agent is asked to vote from exactly here,
		-- through a LuaEvent (the contexts do not share globals; LuaEvents are
		-- what crosses them, synchronously), and `OnAccept` submits what it
		-- cast. `pcall`, so an agent that is not loaded still gets the
		-- abstain-and-submit this rung always did.
		requestCongressBallot();
		OnAccept();
		return true;
	end
	if closeStaleDiplomacyContext() then return true; end
	-- ⚠ THE MEET-A-NEW-CIV SCREEN NEEDS ITS OWN EXIT.
	--
	-- `OnClose`/`Close` do not clear a leader conversation, and the operator saw
	-- Wilfrid Laurier sitting over the map with his three dialogue options while
	-- turns went on advancing behind him. The evidence was
	-- `autoclose_stuck {"attempts":20,"screen":"DiplomacyActionView"}` — twenty
	-- failed tries, then the shim gave up. A first-contact scene is a diplomacy
	-- SESSION, and it ends only when the session is closed.
	--
	-- `ExitConversationMode` is the shipped exit and is a global function, so it
	-- is reachable from here; it closes the active session and falls back to
	-- `Close()` when there is none. `ms_ActiveSessionID` is likewise a global in
	-- the shipped script (no `local`), so closing the session directly is
	-- available as a second try if the view mode is not what we assumed.
	--
	-- ⚠ ESCALATE, do not retry one rung forever. `ExitConversationMode` returns
	-- silently when the view is not in conversation mode, so `pcall` succeeding
	-- does not mean the screen went away — the same "pcall success is not
	-- acceptance" trap that voided every build order in this project. If the
	-- first few attempts have not worked, stop trying this rung and fall through
	-- to the plain handlers below.
	-- ⚠ MY FIRST FIX HERE WAS AIMED AT THE WRONG MODE AND DID NOT WORK: the run
	-- reported `autoclose_stuck DiplomacyActionView` a second time, after twenty
	-- attempts, with `ExitConversationMode` in the ladder.
	--
	-- `ExitConversationMode` only acts `if ms_currentViewMode == CONVERSATION_MODE`.
	-- A first-contact leader is CINEMA_MODE — a full-screen portrait, which is
	-- exactly what the screenshot showed — so it returned having done nothing, and
	-- `Close()` does not dismiss a cinema either. `CloseFocusedState` is the one
	-- entry point that handles all three states (open popup, conversation, cinema)
	-- and it is what the shipped Escape key calls, so it is what we call.
	-- ★★★★★ THE DEAL SCREEN CLOSES BY REFUSING, AND NOTHING ELSE SHUTS IT.
	--
	-- `DiplomacyDealView` is another civilization proposing a trade. It exposes no
	-- `OnClose`/`Close`, and `CloseFocusedState` does not dismiss it either — measured:
	-- twenty attempts, every one reporting `ended: false`, then `autoclose_stuck`, and
	-- the game held there until the harness gave up. That is now the DOMINANT way runs
	-- die: the governor segfault is fixed, and the last three attempts ended in stalls
	-- at t87, t184 and t95, the best of them holding FOUR cities and score 139.
	--
	-- The shipped screen's own exit is `OnRefuseDeal(bForceClose)` — declining is what
	-- closing this screen means, and `true` forces it shut rather than waiting on the
	-- other player. Tried FIRST, because it is the specific answer and the generic ones
	-- demonstrably do nothing here.
	--
	-- ⚠ This declines every offered deal. That is a decision, and the honest defence is
	-- that the alternative measured is not "consider the deal" but "the run ends".
	if type(OnRefuseDeal) == "function"
			and pcall(function() OnRefuseDeal(true); end) then
		return true;
	end
	-- A leader question is the one dialogue that cannot be dismissed by closing
	-- the view: the pending request reopens it. The first CloseFocusedState rung
	-- handles cinema/overview screens; if the view is still present on the next
	-- settled tick, answer the request negatively instead of spending fourteen
	-- 250ms rungs getting there. The session ID is deliberately required so this
	-- cannot turn an unrelated action-view screen into a response.
	-- Under the `dialogue-never-declares-war` lease an unread statement is
	-- answered IGNORE first: its NEGATIVE may be the war the board never chose.
	if NAME == "DiplomacyActionView"
			and (attempt or 1) >= 2 and (attempt or 1) <= 3
			and not dialogueObserved
			and type(OnSelectConversationDiplomacyStatement) == "function"
			and ms_ActiveSessionID ~= nil
			and pcall(function()
				OnSelectConversationDiplomacyStatement(
					dialogueNoWar() and "CHOICE_IGNORE" or "CHOICE_NEGATIVE");
			end) then
		return true;
	end
	if (attempt or 1) <= 6 and type(CloseFocusedState) == "function"
			and pcall(function() CloseFocusedState(true); end) then
		return true;
	end
	if (attempt or 1) <= 9 and type(ExitConversationMode) == "function"
			and pcall(function() ExitConversationMode(true); end) then
		return true;
	end
	-- A direct deal submission can leave the action view visible after its
	-- session has been closed.  `CloseSession` alone is intentionally not a
	-- view close in Firaxis' API; the next frame then reopens the uninitialized
	-- action context above the map.  Close only after the host confirms that
	-- this exact session is gone, and refuse to hide a replacement session that
	-- arrived in the meantime.
	if (attempt or 1) <= 12 and type(DiplomacyManager) == "table"
			and ms_ActiveSessionID ~= nil then
		local sessionID = ms_ActiveSessionID;
		local closed = false;
		local noReplacement = false;
		local checked = pcall(function()
			DiplomacyManager.CloseSession(sessionID);
			closed = not DiplomacyManager.IsSessionIDOpen(sessionID);
			noReplacement = ms_ActiveSessionID == nil or ms_ActiveSessionID == sessionID;
		end);
		if checked and closed and noReplacement and type(Close) == "function"
				and pcall(function() Close(); end) then
			report("autoclose_session_finalize", string.format(",\"session\":%s", tostring(sessionID)));
			return true;
		end
	end
	-- ★★★★★ A LEADER ASKING A QUESTION IS NOT A POPUP — but answering it is a LATER
	-- rung, not the first.
	--
	-- Photographed at the moment of two stalls (`stalled.png`): Cyrus, then Wilhelmina,
	-- asking "Will you allow us to establish an embassy in your capital?" with three
	-- dialogue choices. Nothing that merely CLOSES a screen answers a question.
	--
	-- ⚠⚠ AND THE FIRST VERSION OF THIS MADE IT WORSE, in the way this project keeps
	-- being bitten. It sat at the TOP of the ladder and returned true whenever `pcall`
	-- succeeded — but `pcall` succeeding means the call did not throw, NOT that the
	-- screen closed. Measured: **22 `autoclose` events, every one `ended: true`,
	-- followed by `autoclose_stuck`**. `CHOICE_EXIT` runs `ExitConversationMode(false)`,
	-- which drops the view while the REQUEST is still pending, so the screen reopens
	-- immediately — and by returning first it also short-circuited the
	-- `DiplomacyManager.CloseSession` rung that had been running before it.
	--
	-- So it goes here, after the generic closers and after CloseSession have each had
	-- their attempts, and it is bounded like every other rung.
	-- ⚠⚠ DECLINE THE REQUEST, DO NOT JUST LEAVE THE ROOM. `CHOICE_EXIT` runs
	-- `ExitConversationMode`, which drops the VIEW while the request is still pending,
	-- so the screen comes straight back — 22 closes and 22 reopens, measured. The
	-- shipped handler answers with `DiplomacyManager.AddResponse(session, player,
	-- "NEGATIVE")` under `CHOICE_NEGATIVE`, and an answered request does not return.
	-- `CHOICE_IGNORE` is the same shape via "RESPONSE_IGNORE".
	if (attempt or 1) <= 14 and not dialogueObserved
			and type(OnSelectConversationDiplomacyStatement) == "function"
			and pcall(function()
				OnSelectConversationDiplomacyStatement("CHOICE_NEGATIVE");
			end) then
		return true;
	end
	if (attempt or 1) <= 15 and not dialogueObserved
			and type(OnSelectConversationDiplomacyStatement) == "function"
			and pcall(function()
				OnSelectConversationDiplomacyStatement("CHOICE_IGNORE");
			end) then
		return true;
	end
	if (attempt or 1) <= 16 and not dialogueObserved
			and type(OnSelectConversationDiplomacyStatement) == "function"
			and pcall(function()
				OnSelectConversationDiplomacyStatement("CHOICE_EXIT");
			end) then
		return true;
	end
	if (attempt or 1) <= 18 and not dialogueObserved
			and type(OnSelectInitialDiplomacyStatement) == "function"
			and pcall(function()
				OnSelectInitialDiplomacyStatement("CHOICE_EXIT");
			end) then
		return true;
	end

	-- The relic screens. Neither exposes OnClose or Close, which is why both sat
	-- over the map: the operator reported "relic found" alongside the Canada
	-- delegation. Their close buttons are wired to these globals instead.
	--
	-- GreatWorkShowcase shows a relic or great work just acquired and its close
	-- button calls OnHideScreen. ChooseArtifact is a real decision -- which
	-- artifact an Archaeologist lifts -- and Button1 takes the first. Taking the
	-- first is a worse choice than a human would make and a far better one than
	-- staring at the dialog until the timeout, which is the current behaviour.
	if type(OnHideScreen) == "function" then OnHideScreen(); return true; end
	if NAME == "ChooseArtifact" and type(OnButton1) == "function" then
		OnButton1();
		return true;
	end
	-- The between-turns congress screen must be HIDDEN, not merely unlocked.
	-- `ReleaseEventLock` lets the event playback continue but leaves the modal
	-- context over the map. The shipped Escape key and its close button both call
	-- `OnHide`, which releases the lock and dequeues the popup in one operation.
	if NAME == "WorldCongressBetweenTurns"
			and type(OnHide) == "function" then
		OnHide();
		return true;
	end
	if type(OnClose) == "function" then OnClose(); return true; end
	if type(Close) == "function" then Close(); return true; end
	return false;
end

-- ★★ HOTSEAT. Two jobs, both only in a game installed for hotseat play
-- (`cfg.Hotseat`; the installer adds the PlayerChange context only then).
--
-- 1. The operator's turn is theirs. While the local player is one of
--    `HotseatHumanSeats`, no screen here is closed, answered or escaped: the
--    announcement, the leader and the deal are the person's to read. The one
--    exception is a seat the `--auto-human gated` test stub plays
--    (`personTurn` below): nobody is there to read them.
-- 2. PlayerChange is the hand-over screen. PlayerChange.lua shows it on every
--    human seat's turn (BuildTurnControls :121, which also pauses the game)
--    and only its Start Turn button (`OnOk` :54) unpauses and dequeues it. On
--    a CIVVIS seat this presses it; on the operator's seat it waits for them,
--    unless the game asks for their hand-over to be pressed too
--    (`HotseatAutoStart`, the default for a game a person plays; the cover in
--    CivvisHotseatCover.lua lifts on that press). It also says, for the
--    harness, whose turn begins and ends: the seat agents are silent outside
--    their own turns, and so is the operator.
-- Every save the harness asks for is made here, at the start of the first
-- CIVVIS seat's turn, through InGameTopOptionsMenu.lua:183's own call.
CivvisHotseatScreen = { waited = 0, pressedFor = nil, savedTurn = -1, hiddenFor = nil };
function CivvisHotseatScreen.listed(list, pid)
	if type(list) ~= "table" or pid == nil then return false; end
	for _, seat in ipairs(list) do
		if tonumber(seat) == pid then return true; end
	end
	return false;
end
function CivvisHotseatScreen.localPlayer()
	local pid = -1;
	pcall(function() pid = Game.GetLocalPlayer(); end);
	return tonumber(pid) or -1;
end
function CivvisHotseatScreen.turn()
	local turn = -1;
	pcall(function() turn = Game.GetCurrentGameTurn(); end);
	return tonumber(turn) or -1;
end
function CivvisHotseatScreen.humanTurn()
	if cfg.Hotseat ~= true then return false; end
	-- `RequireHarness`: CIVVIS's turn only on its own seats, harness live.
	if cfg.RequireHarness == true
			and not CivvisHarness.mine(CivvisHotseatScreen.localPlayer()) then return true; end
	return CivvisHotseatScreen.listed(cfg.HotseatHumanSeats,
		CivvisHotseatScreen.localPlayer());
end
-- Whose screens no closer touches: a real person's. The `--auto-human gated`
-- test stub sits in a person's seat (`HotseatHumanSeats` AND
-- `HotseatStubSeats`) so the cover and the pressed hand-over treat it as the
-- person, but nobody is there to dismiss its announcements: run cover-live-6
-- (2026-11-27) parked on turn 1 behind its own "Roman Empire Makes History"
-- card. So the closers stand down for a person's seat only when no stub plays
-- it. A stub seat is only ever listed by the `--auto-human` test arrangements.
function CivvisHotseatScreen.personTurn()
	if not CivvisHotseatScreen.humanTurn() then return false; end
	-- `RequireHarness`: with no live harness the stub's seat is a person's too.
	if cfg.RequireHarness == true
			and not CivvisHarness.mine(CivvisHotseatScreen.localPlayer()) then return true; end
	return not CivvisHotseatScreen.listed(cfg.HotseatStubSeats,
		CivvisHotseatScreen.localPlayer());
end
-- `--name dev` (HotseatPersonName): the person's seat has no agent context to
-- name it as a CIVVIS seat names itself (CivvisSeat.claim), so this screen
-- does, when it loads: before the first hand-over is shown, so it already says
-- "dev" and not "Player 1". EditHotseatPlayer.lua:120 names a seat this way.
-- Read back, never assumed, and tried again at a turn's start until it reads so.
function CivvisHotseatScreen.namePerson()
	local name = cfg.HotseatPersonName;
	if type(name) ~= "string" or name == "" or CivvisHotseatScreen.personNamed then return; end
	local all = true;
	for _, seat in ipairs(cfg.HotseatHumanSeats or {}) do
		seat = tonumber(seat);
		local applied = pcall(function() PlayerConfigurations[seat]:SetHotseatName(name); end);
		local shown = "?";
		pcall(function() shown = tostring(PlayerConfigurations[seat]:GetPlayerName()); end);
		if shown ~= name then all = false; end
		report("hotseat_person", string.format(',"seat":%d,"applied":%s,"shown":"%s"',
			seat, tostring(applied and shown == name), (shown:gsub('[\\"%c]', ""))));
	end
	CivvisHotseatScreen.personNamed = all;
end
-- The UI clock on the hand-over's records, `"ui"` in seconds: the clock the
-- agent's `end_turn_wait.ui_now` and the cover's `waited` read, so
-- `tools/hotseat_wait.py` can tell the hand-over and the press apart from the
-- rest of a seat's opening (the mod's other clock, `Automation.GetTime`, ticks
-- whole seconds). Empty where there is no such clock.
function CivvisHotseatScreen.clock()
	local ui = nil;
	pcall(function() ui = UI.GetElapsedTime(); end);
	if type(ui) ~= "number" or ui ~= ui or ui < 0 or ui > 1e12 then return ""; end
	return string.format(',"ui":%.3f', ui);
end
function CivvisHotseatScreen.announce(phase)
	pcall(CivvisHotseatScreen.namePerson);
	local pid = CivvisHotseatScreen.localPlayer();
	report("hotseat_turn", string.format(
		',"phase":"%s","player":%d,"turn":%d,"human":%s,"civvis":%s',
		phase, pid, CivvisHotseatScreen.turn(),
		tostring(CivvisHotseatScreen.listed(cfg.HotseatHumanSeats, pid)),
		tostring(CivvisHotseatScreen.listed(cfg.HotseatSeats, pid)))
		.. CivvisHotseatScreen.clock());
end
function CivvisHotseatScreen.save(pid, turn)
	local every = tonumber(cfg.HotseatSaveEvery) or 0;
	local first = nil;
	for _, seat in ipairs(cfg.HotseatSeats or {}) do
		seat = tonumber(seat);
		if seat ~= nil and (first == nil or seat < first) then first = seat; end
	end
	if every <= 0 or pid ~= first or turn < 1 or turn % every ~= 0
			or CivvisHotseatScreen.savedTurn == turn then
		return;
	end
	CivvisHotseatScreen.savedTurn = turn;
	local name = string.format("%s-T%03d", tostring(cfg.HotseatSaveName or RUN), turn);
	local saved = pcall(function()
		local gameFile = {};
		gameFile.Name = name;
		gameFile.Location = SaveLocations.LOCAL_STORAGE;
		gameFile.Type = Network.GetGameConfigurationSaveType();
		gameFile.IsAutosave = false;
		gameFile.IsQuicksave = false;
		Network.SaveGame(gameFile);
	end);
	report("hotseat_saved", string.format(',"name":"%s","turn":%d,"requested":%s',
		name, turn, tostring(saved)));
end
-- The person's hand-over, pressed for them. A seat with a hotseat password
-- keeps its screen, as the shipped Enter key does (PlayerChange.lua:42-43).
function CivvisHotseatScreen.autoStart(pid)
	if cfg.Hotseat ~= true or cfg.HotseatAutoStart ~= true
			or not CivvisHotseatScreen.listed(cfg.HotseatHumanSeats, pid) then
		return false;
	end
	local password = "";
	pcall(function() password = PlayerConfigurations[pid]:GetHotseatPassword() or ""; end);
	return password == "";
end
-- Whose hand-over this presses: every CIVVIS seat and test stub, and the
-- operator's only with `HotseatAutoStart`.
function CivvisHotseatScreen.mayPress(pid)
	if pid == nil or pid < 0 then return false; end
	-- `RequireHarness`: no Start Turn is pressed for anyone without a live harness.
	if cfg.RequireHarness == true and not CivvisHarness.live() then return false; end
	if cfg.Hotseat == true and CivvisHotseatScreen.listed(cfg.HotseatHumanSeats, pid) then
		return CivvisHotseatScreen.autoStart(pid);
	end
	return CivvisHotseatScreen.listed(cfg.HotseatSeats, pid)
		or CivvisHotseatScreen.listed(cfg.HotseatStubSeats, pid);
end
function CivvisHotseatScreen.press(pid, turn, via)
	local key = pid .. ":" .. turn;
	pcall(CivvisHotseatScreen.save, pid, turn);
	local pressed = type(OnOk) == "function" and pcall(OnOk);
	report("hotseat_start_turn", string.format(
		',"player":%d,"turn":%d,"pressed":%s,"again":%s,"person":%s,"via":"%s"',
		pid, turn, tostring(pressed), tostring(CivvisHotseatScreen.pressedFor == key),
		tostring(CivvisHotseatScreen.listed(cfg.HotseatHumanSeats, pid)), tostring(via))
		.. CivvisHotseatScreen.clock());
	CivvisHotseatScreen.pressedFor = key;
	return pressed;
end
-- A game with the cover (CivvisHotseatCover.lua). The hand-over sits above it
-- and is pressed as it ships; all it may show over the cover is its menu
-- button.
function CivvisHotseatScreen.covered()
	return cfg.Hotseat == true and cfg.HotseatCover == true
		and (cfg.RequireHarness ~= true or CivvisHarness.live());
end
function CivvisHotseatScreen.paused(pid)
	local paused = false;
	pcall(function() paused = PlayerConfigurations[pid]:GetWantsPause() == true; end);
	return paused;
end
-- How long the hand-over is up before its first press. 0.3 s lets it slide in
-- for whoever watches; under the cover nobody does (it is hidden on its first
-- frame, below), and the seat's whole turn waits on the press: the agent does
-- nothing while the hand-over's pause stands (`CivvisSeat.mine`; in the live
-- game hotseat-20261128T031456Z every seat's first record followed its
-- `hotseat_start_turn` in Automation.log, 75 of 75). Six presses a round at
-- five CIVVIS seats (theirs and the person's own) is 1.8 s of every wait.
-- `HotseatFastTurns` (`tools/civ6_hotseat.py --fast-turns`, off unless asked)
-- presses a covered hand-over on its first frame up. PlayerChange.lua pauses
-- before it shows (`BuildTurnControls` :125 `SetPause(true)`, then
-- `QueuePopup` :135) and its Start Turn is `OnOk` (:54), which unpauses
-- (:57) and dequeues (:60), so the press finds the pause it lifts. An
-- explicit `HotseatPressSeconds` still wins; a retry still waits 2 s.
function CivvisHotseatScreen.firstPause(covered)
	local set = tonumber(cfg.HotseatPressSeconds);
	if set ~= nil then return set; end
	if covered and cfg.HotseatFastTurns == true then return 0; end
	return 0.3;
end
-- ★ REAL SECONDS, AND A PERSON'S TURN STARTED AT 1. The console's `timescale`
-- (hotseat `--fast-animations`, `HotseatFastAnimations`; the agent's
-- `CivvisClock.fastWant`) speeds this context's frame deltas up with the UI
-- clock, so the press's 0.3 s and its 2 s retry count each delta divided by
-- the scale the table shares (`ExposedMembers.CivvisTimeScale`; 1 when it is
-- missing or nonsense, and outside hotseat). With the key, a person's Start
-- Turn (theirs, or the test stub's that stands in for them) is pressed only
-- once that scale reads 1: if it does not, every agent context is asked to
-- put it back (`LuaEvents.CivvisFastAnimations("reset")`), and the press
-- waits for it up to `SCALE_HOLD` real seconds, then goes ahead and says so
-- (`hotseat_scale_unconfirmed`; `hotseat_scale_hold` when the wait began).
CivvisHotseatScreen.SCALE_HOLD = 1.0;
function CivvisHotseatScreen.scale()
	if cfg.Hotseat ~= true then return 1; end
	local s = nil;
	pcall(function() s = tonumber(ExposedMembers.CivvisTimeScale); end);
	if s == nil or s ~= s or s < 1 or s > 8 then return 1; end
	return s;
end
function CivvisHotseatScreen.atOne()
	local s = nil;
	pcall(function() s = ExposedMembers.CivvisTimeScale; end);
	return s == nil or tonumber(s) == 1;
end
-- Whether `pid`'s press may go now, as far as the scale is concerned.
function CivvisHotseatScreen.scaleReady(pid, key, step)
	local s = CivvisHotseatScreen;
	if cfg.Hotseat ~= true or tonumber(cfg.HotseatFastAnimations) == nil
			or not (s.listed(cfg.HotseatHumanSeats, pid) or s.listed(cfg.HotseatStubSeats, pid)) then
		return true;
	end
	if s.atOne() then return true; end
	pcall(function() LuaEvents.CivvisFastAnimations("reset", "person_handover"); end);
	if s.atOne() then return true; end
	if s.scaleHeldFor ~= key then
		s.scaleHeldFor, s.scaleHeld = key, 0;
		report("hotseat_scale_hold", string.format(',"player":%d,"turn":%d,"scale":"%s"',
			pid, s.turn(), tostring(ExposedMembers and ExposedMembers.CivvisTimeScale)));
		return false;
	end
	s.scaleHeld = s.scaleHeld + step;
	if s.scaleHeld < s.SCALE_HOLD then return false; end
	s.scaleHeldFor = nil;
	report("hotseat_scale_unconfirmed", string.format(',"player":%d,"turn":%d,"held":%.2f',
		pid, s.turn(), s.scaleHeld));
	return true;
end
-- One press per seat and turn, after the screen has been up a moment (its
-- own slide-in) and retried every two seconds while it stays up. Over the
-- cover, a hand-over this presses is hidden on its first frame, the way
-- PlayerChange.lua hides it itself (:61 after Start Turn, :194 for "please
-- wait"), and pressed on the same clock while its pause stands; its "please
-- wait" line (:191) is hidden too (:172). Nothing else of it is touched.
function CivvisHotseatScreen.frame(fDTime)
	local dialog = false;
	pcall(function() dialog = not Controls.PopupAlphaIn:IsHidden(); end);
	local pid = CivvisHotseatScreen.localPlayer();
	local covered = CivvisHotseatScreen.covered();
	if covered then
		pcall(function()
			if not Controls.PlayerChangingText:IsHidden() then
				Controls.PlayerChangingText:SetHide(true);
			end
		end);
	end
	if not CivvisHotseatScreen.mayPress(pid) then
		CivvisHotseatScreen.waited = 0;
		return;
	end
	local turn = CivvisHotseatScreen.turn();
	local key = pid .. ":" .. turn;
	if dialog and covered then
		pcall(function() Controls.PopupAlphaIn:SetHide(true); end);
		CivvisHotseatScreen.hiddenFor = key;
	elseif not dialog and not (covered and CivvisHotseatScreen.hiddenFor == key
			and CivvisHotseatScreen.paused(pid)) then
		CivvisHotseatScreen.waited = 0;
		return;
	end
	local step = (tonumber(fDTime) or 0) / CivvisHotseatScreen.scale();
	CivvisHotseatScreen.waited = CivvisHotseatScreen.waited + step;
	-- (A handed-back turn's hand-over in the diagnostic `press` reaction is
	-- pressed as a first one, see `onBuild`.)
	local pause = (CivvisHotseatScreen.pressedFor == key and CivvisHotseatScreen.pressAgain ~= key) and 2.0
		or CivvisHotseatScreen.firstPause(covered);
	if CivvisHotseatScreen.waited < pause then return; end
	if not CivvisHotseatScreen.scaleReady(pid, key, step) then return; end
	CivvisHotseatScreen.waited = 0;
	CivvisHotseatScreen.pressAgain = nil;
	CivvisHotseatScreen.press(pid, turn, "dialog");
end
-- The cover's backstop (CivvisHotseatCover.lua, `nudge`): the seat now
-- playing still holds its hand-over pause 1.5 s in. The hand-over sits above
-- the cover and its own frame presses first; were it ever held hidden its
-- frame would stop, and an event still reaches it. The same press, for the
-- same seats, and only on a standing hand-over pause the operator did not
-- ask for.
function CivvisHotseatScreen.fromCover(pid, turn)
	pid = tonumber(pid);
	if pid == nil or pid ~= CivvisHotseatScreen.localPlayer()
			or not CivvisHotseatScreen.mayPress(pid) then
		return false;
	end
	local paused, active, held = false, false, false;
	pcall(function() paused = PlayerConfigurations[pid]:GetWantsPause() == true; end);
	pcall(function() active = Players[pid]:IsTurnActive() == true; end);
	pcall(function() held = ExposedMembers.CivvisOperatorHold == true; end);
	if not paused or not active or held then return false; end
	-- Never a second press of a turn handed back (see `onBuild`).
	local turn = CivvisHotseatScreen.turn();
	if CivvisHotseatScreen.returned == pid .. ":" .. turn and CivvisHotseatScreen.reaction() == "safe" then
		return false;
	end
	return CivvisHotseatScreen.press(pid, turn, "cover");
end
-- ★★★ A TURN THE ENGINE HANDS BACK IS NOT HANDED OVER A SECOND TIME.
-- The engine gives a seat its turn back by itself when a unit still has
-- movement after the walk it was on is finished at the turn complete
-- (net_message_debug.log, live-6p-20261128T0819Zg turn 6: `Player 0 Unit
-- 262146 has movement remaining after completing automation.  Attempting to
-- turn active player.`, `AppRequestTurnUnready: Player 0`), as single player
-- gives the turn back to a unit needing orders. In hotseat that seat's turn
-- then BEGINS AGAIN: `LocalPlayerTurnEnd` and `LocalPlayerTurnBegin` for the
-- same seat and turn with no other seat's begin between (the same frame in
-- the stand-in's game; 94 ms and 164 ms apart in the owner's games
-- hotseat-20261128T062253Z turn 18 and hotseat-20261128T045523Z turn 33).
-- PlayerChange.lua shows the hand-over for it as for a new seat
-- (OnLocalPlayerTurnBegin :83-91 -> BuildTurnControls :121-140,
-- `SetPause(true)` :125, the game really pausing: `GameCore SEND:
-- GamePaused: gamePaused=1`), this screen took it for a retry of a press
-- already made (2 s), the cover's backstop pressed it at 1.5 s (`again:true`,
-- `via:"cover"`), and Civ VI died: in all three games of the night that had
-- one, and the stand-in's report puts the crash 0.1 s after that press.
-- So in a hotseat game a turn handed back is not handed over again
-- (`HotseatReturnedTurn`, a diagnostic for crash check games only, else
-- "safe"): the shipped `BuildTurnControls` is not called for it, the game is
-- not paused, and the "please wait" screen its end of turn raised
-- (OnLocalPlayerTurnEnd :93-106) is closed at once the way its Start Turn
-- closes it (`OnOk` :54-64: resume the effects, `SetPause(false)` (a no-op,
-- nothing is paused), `PlayerChange_Close`, dequeue). The seat's turn simply
-- goes on; the cover lifts for a person's (CivvisHotseatCover.lua) and the
-- seat's agent settles its units before ending it again
-- (CivvisControlAgent.lua `CivvisQueue.settleReturned`, told through
-- `ExposedMembers.CivvisTurnReturned`). `legacy` keeps the old hand-over;
-- `press` keeps the pause and presses it as a first press, on its first frame.
CivvisHotseatScreen.ended, CivvisHotseatScreen.returned = nil, nil;
function CivvisHotseatScreen.reaction()
	local mode = cfg.HotseatReturnedTurn;
	if mode == "legacy" or mode == "press" then return mode; end
	return "safe";
end
-- The turn events, as they come: the seat and turn of the last end, until a
-- different seat's or turn's begin.
function CivvisHotseatScreen.noteTurn(phase)
	local s = CivvisHotseatScreen;
	local key = s.localPlayer() .. ":" .. s.turn();
	if phase == "end" then
		s.ended = key;
	elseif s.ended ~= key then
		s.ended = nil;
	end
end
-- In place of the shipped `BuildTurnControls` (see `wrapBuild`): true when it
-- handled a turn handed back, so the shipped hand-over is not built.
function CivvisHotseatScreen.onBuild()
	local s = CivvisHotseatScreen;
	if cfg.Hotseat ~= true then return false; end
	local pid, turn = s.localPlayer(), s.turn();
	local key = pid .. ":" .. turn;
	if s.ended ~= key then return false; end
	s.ended = nil;
	s.returned = key;
	local count = 1;
	pcall(function()
		local back = ExposedMembers.CivvisTurnReturned;
		if type(back) == "table" and back.player == pid and back.turn == turn then
			count = (tonumber(back.n) or 0) + 1;
		end
		ExposedMembers.CivvisTurnReturned = { player = pid, turn = turn, n = count };
	end);
	local mode = s.reaction();
	local closed = false;
	if mode == "safe" then
		closed = type(OnOk) == "function" and pcall(OnOk) == true;
	elseif mode == "press" then
		s.pressAgain, s.waited = key, 0;
	end
	report("hotseat_turn_returned", string.format(
		',"player":%d,"turn":%d,"returned":%d,"reaction":"%s","closed":%s,"pressed_before":%s,"person":%s',
		pid, turn, count, mode, tostring(closed), tostring(s.pressedFor == key),
		tostring(s.listed(cfg.HotseatHumanSeats, pid))) .. s.clock());
	return mode == "safe" and closed;
end
-- PlayerChange.lua's `OnLocalPlayerTurnBegin` (:89) and `OnLoadScreenClose`
-- (:150, through it) call the global `BuildTurnControls` by name, so it is
-- replaced here, in this context only, and calls the shipped one for every
-- turn that is not handed back. Hotseat only (the PlayerChange context is
-- installed for nothing else).
function CivvisHotseatScreen.wrapBuild()
	local stock = BuildTurnControls;
	if cfg.Hotseat ~= true or type(stock) ~= "function" then return false; end
	BuildTurnControls = function(...)
		local ok, handled = pcall(CivvisHotseatScreen.onBuild);
		if ok and handled then return; end
		return stock(...);
	end;
	return true;
end
function CivvisHotseatScreen.arm()
	pcall(function()
		Events.LocalPlayerTurnBegin.Add(function()
			CivvisHotseatScreen.announce("begin");
			pcall(CivvisHotseatScreen.noteTurn, "begin");
		end);
	end);
	pcall(function()
		Events.LocalPlayerTurnEnd.Add(function()
			CivvisHotseatScreen.announce("end");
			pcall(CivvisHotseatScreen.noteTurn, "end");
		end);
	end);
	pcall(CivvisHotseatScreen.wrapBuild);
	if cfg.HotseatCover == true then
		pcall(function()
			LuaEvents.CivvisHotseatStartTurn.Add(function(pid, turn)
				pcall(CivvisHotseatScreen.fromCover, pid, turn);
			end);
		end);
	end
	ContextPtr:SetUpdate(function(fDTime) pcall(CivvisHotseatScreen.frame, fDTime); end);
	pcall(CivvisHotseatScreen.namePerson);
	report("hotseat_armed", string.format(',"start_turn":%s', tostring(type(OnOk) == "function")));
end

if NAME == "PlayerChange" then
	CivvisHotseatScreen.arm();
elseif not haveScreen() then
	-- The failure that has to be loud. A replacement whose include did not
	-- land leaves the context with no shipped code at all, which does not look
	-- like a broken mod: the announcement simply never appears, and a run
	-- reads as a quiet game rather than a game missing a screen.
	report("autoclose_unarmed");
else
	-- A popup context is hidden except while its popup is up, which is how the
	-- shipped code itself tests for showing (ExclusivePopupManager checks
	-- IsHidden before it unlocks). Watching that rather than a show handler
	-- catches every way a screen can be raised -- a wonder, a second wonder
	-- queued behind it, a reload restoring one -- and clobbers nothing: two of
	-- these screens already register a show handler of their own.
	--
	-- The stopwatch starts on the *change* from down to up, never on merely
	-- being up. TechCivicCompletedPopup is authored visible and is hidden by
	-- the popup manager afterwards, so "not hidden" on the first frame of a
	-- game is not a screen anybody is looking at. Waiting for the edge costs
	-- nothing -- an update ticks whether or not its context is showing, which
	-- is how the shipped FiraxisLiveMessaging times its own auto-close -- and
	-- it makes a wrong guess here cost a screen that does not close rather
	-- than a screen closed that was never open.
	local showing = false;
	local remaining = 0;
	local shown = 0;
	local closes = 0;
	local wonderAnimationWaitReported = false;
	-- Whether this screen's failure has already been reported. Giving up is a
	-- BACK-OFF, never a stop -- see the end of `tick` for why nothing here may
	-- be permanent.
	local reported = false;
	local desktopReportedAt = -1;   -- attempts count at the last ask, -1 = never
	local heartbeatFrames = 0;      -- tick() calls since load, hidden ones included
	local heartbeatSeconds = 0;     -- real seconds over those same calls
	local controllerPulseSeconds = 0;
	local dialogueFadeExpired = false;

	-- ★★★★★ A FRESH SHOW IS A FRESH DIALOGUE, EVEN WHEN THIS CONTEXT NEVER TICKS
	-- WHILE HIDDEN.
	--
	-- `ContextPtr:SetUpdate` stops with the hidden context. That means the
	-- `not isUp()` reset in `tick` cannot observe the ordinary
	-- close -> hide -> next-show sequence for DiplomacyActionView: its `closes`
	-- counter survives into the next, unrelated leader interaction. Once the
	-- old interaction had reached GIVE_UP_AFTER, every later interaction began
	-- directly on the 30-second retry path and emitted misleading desktop-help
	-- asks even though the native ladder was only looking at a new screen.
	--
	-- Run `civvis-20260904T042312Z-capture-free-1` showed that exact shape:
	-- attempts 32/36 belonged to later sessions, while turns continued and the
	-- capture-free owner intentionally had no desktop rescue. Reset only the
	-- *attempt accounting* when Firaxis says this context is shown again. The
	-- active deal hold is deliberately left alone: it is a real CIVVIS-owned
	-- session and may have been armed before the view becomes visible.
	local function resetShownAttemptState()
		dialogueFadeExpired = false;
        firstMeetChoice, firstMeetText = nil, nil;
        firstMeetAnswered = false;
        dialogueObserved, dialogueChoice, dialogueCallback, dialogueAnswered = false, nil, nil, false;
        dialogueContext = "";
		showing = false;
		controllerPulseSeconds = 0;
		remaining = 0;
		shown = 0;
		closes = 0;
		reported = false;
		desktopReportedAt = -1;
		wonderAnimationWaitReported = false;
	end

	-- The shipped scripts install `OnShow` during their Initialize path. Preserve
	-- it exactly and layer the accounting reset around the same context callback;
	-- this is native lifecycle evidence, not a timer guess or a desktop action.
	local shippedOnShow = OnShow;
	if type(shippedOnShow) == "function" then
		pcall(function()
			ContextPtr:SetShowHandler(function(...)
				resetShownAttemptState();
				return shippedOnShow(...);
			end);
		end);
	end

	-- ★★★★★ A DEAL SESSION CIVVIS OPENED IS NOT A SCREEN TO REFUSE. The
	-- agent's sale, passage and peace arms now ask inside a `MAKE_DEAL`
	-- session (the only place a rival evaluates a working deal — see
	-- `CivvisTrade.ask` in CivvisControlAgent.lua), and the diplomacy views
	-- come up for it. `LuaEvents.CivvisDealSession(subject, open, seconds)`
	-- says the agent owns the session for at most `seconds`; the ladder below
	-- waits that long and then runs exactly as before, so an unanswered
	-- session is refused and closed the way every other screen is. Contexts
	-- do not share globals; LuaEvents are the one channel there is.
	local dealHold = 0;
	local dealSubject = nil;
	local holdReported = false;
	-- A Civvis-owned deal is allowed to use the opening hold while the
	-- shipped diplomacy fade settles.  If that fade never reports stopped,
	-- `dialogueReady()` must not turn the bounded hold into an unbounded one.
	-- Keep this separate from `dealHold`: once the countdown expires it must
	-- remain true across ticks until the native close ladder has actually hidden
	-- the context.
	local dealForceClose = false;
	pcall(function()
		LuaEvents.CivvisDealSession.Add(function(subject, open, seconds)
				if open then
					dealHold = tonumber(seconds) or 4;
					dealSubject = subject;
					holdReported = false;
					dealForceClose = false;
				else
					dealHold = 0;
					-- CivvisTrade emits this event after the host has already
					-- closed the owned session. That can happen before the
					-- configured opening hold expires. The ordinary dialogue
					-- readiness gate is no longer safe in that state: its fade
					-- may be waiting on the session that just disappeared. Keep
					-- the force-close latch set until the view is proven gone.
					dealForceClose = true;
				end
		end);
	end);

	-- How long to leave a screen alone once it has refused GIVE_UP_AFTER times.
	-- Long enough not to hammer it, short enough that the map comes back on its
	-- own if whatever held it goes away.
	-- How often this context says it is alive. One a minute per armed screen
	-- is enough to bracket a wedge (the outside watchdog allows five minutes)
	-- without filling the log: a full 250-turn game adds a few hundred bytes.
	-- ★★★★ ANSWERED, AND THE ANSWER IS NO: A HIDDEN CONTEXT DOES NOT TICK.
	--
	-- This was built to decide whether the UI thread outlives a parked Game
	-- Core, because if it did, a nudge from this side could unpark a turn the
	-- agent can no longer reach. Run civvis-20260829T194002Z settles it:
	-- **two** heartbeats across 121 turns, both `"up":true`, both from the one
	-- screen that was actually showing.
	--
	-- 600 frames took 5.0 seconds, so the game renders about 120fps. Were
	-- `SetUpdate` running on hidden contexts, each of the two dozen armed
	-- screens would have emitted one every five seconds — hundreds apiece over
	-- a 35-minute game. So `ContextPtr:SetUpdate` runs only while its context
	-- is visible, and there is NO always-on UI tick here.
	--
	-- ⚠ Do not rebuild the in-mod nudge on this: there is nothing to hang it
	-- on. An EXTERNAL keystroke is a different path and is unaffected — the
	-- harness sends SHIFT+RETURN through the OS into the app's event loop
	-- rather than through any mod tick (`macos_input.press_key`, #2781). If a
	-- parked turn is recoverable at all, that is where to try it.
	--
	-- The heartbeat stays because it still reports something real and cheap:
	-- a screen that held the UI for five seconds or more, which is the shape a
	-- stuck popup makes.
	--
	-- Counted in FRAMES, not seconds, and that is the whole point.
	--
	-- The first attempt accumulated `fDTime` to sixty seconds and emitted
	-- NOTHING across a whole game, which looked like an answer and was not:
	-- each popup context is only up for a second or two at a time, so a
	-- per-context clock can never reach sixty however alive the thread is.
	-- A frame counter climbs whenever `tick` is CALLED, which is exactly the
	-- question — and carrying the seconds alongside it separates the two
	-- remaining possibilities: frames climbing with seconds near zero means
	-- the tick runs while hidden but `fDTime` is not meaningful there, and
	-- no frames at all means `SetUpdate` does not run on a hidden context.
	local HEARTBEAT_FRAMES = 600;
	-- Retry a complete close ladder promptly, including after many failures.
	local RETRY_SECONDS = 1.0;
	local DIALOGUE_FADE_TIMEOUT_SECONDS = 2.0;
	local DIALOGUE_READY_RETRY_SECONDS = 0.05;

	-- What "up" means. Everywhere else it is the context not being hidden, the
	-- same test the shipped popup manager uses. InGamePopup is *pushed as a
	-- modal* rather than shown, so it reads the flag the counting wrapper sets
	-- instead -- which is true only while a one-button dialog is open, and
	-- that is also the only time this screen may act at all.
	local function isUp()
		if NAME == "InGamePopup" then return dialogIsAnnouncement; end
		return not ContextPtr:IsHidden();
	end

	-- The operator's Pause (CivvisBoard.operatorHold in the agent). While the
	-- live game is held, a screen up is one the operator opened to look at — a
	-- leader, the trade overview, the Congress — or one they are reading, so
	-- nothing here closes it. Their Resume hands the screen back to the clock.
	local function operatorHolds()
		local held = false;
		pcall(function() held = ExposedMembers.CivvisOperatorHold == true; end);
		return held;
	end

	-- The shipped diplomacy contexts are visible before they can accept an
	-- action. During the opening fade the view mode is still being constructed;
	-- calling its handlers then is a successful Lua call that does nothing. The
	-- old timer counted those no-ops as failures, reached GIVE_UP_AFTER in about
	-- one second, and then waited RETRY_SECONDS. Wait without consuming a rung.
	-- This is intentionally conservative: if a control is absent or its state
	-- cannot be read, the normal closer remains the fallback.
	local function dialogueReady()
		if NAME == "DiplomacyActionView" and Controls ~= nil
				and Controls.BlackFadeAnim ~= nil then
			local stopped = false;
			local readable = pcall(function() stopped = Controls.BlackFadeAnim:IsStopped(); end);
			if readable and not stopped then return false; end
		elseif NAME == "DiplomacyDealView" and Controls ~= nil
				and Controls.TradePanelFade ~= nil then
			local stopped = false;
			local readable = pcall(function() stopped = Controls.TradePanelFade:IsStopped(); end);
			if readable and not stopped then return false; end
		end
		return true;
	end

	-- WonderBuiltPopup's stock XML exposes the four tracks that make up the
	-- reveal. Return a second value saying whether the controls were readable;
	-- a known-running animation is given the full timeout, while an
	-- absent/incompatible control gets the same bounded fallback so one bad UI
	-- build cannot cover the map forever.
	local function wonderAnimationReady()
		if Controls == nil or Controls.HeaderAlpha == nil
				or Controls.HeaderSlide == nil or Controls.QuoteAlpha == nil
				or Controls.QuoteSlide == nil then
			return false, false;
		end
		local animations = {
			Controls.HeaderAlpha, Controls.HeaderSlide,
			Controls.QuoteAlpha, Controls.QuoteSlide,
		};
		for _, animation in ipairs(animations) do
			local stopped = nil;
			local readable = pcall(function() stopped = animation:IsStopped(); end);
			if not readable or type(stopped) ~= "boolean" then return false, false; end
			if not stopped then return false, true; end
		end
		return true, true;
	end

	-- ★ REAL SECONDS. The engine's debug `timescale` speeds frame deltas up
	-- with the UI clock, and the deal hold, the close delay, the fade and
	-- wonder timeouts and this context's agent pulse all count them. So each
	-- delta is divided by the scale the agent shares in
	-- `ExposedMembers.CivvisTimeScale` (1 when it is missing or nonsense), and
	-- the scale this context read is acknowledged in `CivvisClockAck`, which
	-- the agent checks before it keeps the timescale.
	local function realSeconds(fDTime)
		local raw = math.max(0, tonumber(fDTime) or 0);
		local scale = nil;
		pcall(function() scale = tonumber(ExposedMembers.CivvisTimeScale); end);
		if scale == nil or scale ~= scale or scale < 1 or scale > 8 then scale = 1; end
		pcall(function()
			local ack = ExposedMembers.CivvisClockAck;
			ack.scale[NAME] = scale;
			ack.at[NAME] = UI.GetElapsedTime();
		end);
		return raw / scale;
	end

	-- ★★★ A POPUP QUEUED BEHIND A DEAL SESSION HOLDS THE GAME, AND THE RIVAL'S
	-- VERDICT WITH IT. A popup takes an event lock when its event fires
	-- (`ms_eventID = UI.ReferenceCurrentEvent()`, DLC/Expansion2/UI/Additions/
	-- RockBandMoviePopup.lua:138) and gives it back in its own `Close()`
	-- (`UI.ReleaseEventID( ms_eventID )`, :34). Until then the game core
	-- dispatches nothing more. This shim closes such a popup 0.05 s after it
	-- comes up, but a popup queued while a leader screen is showing does not
	-- come up, and a hidden context does not tick (see the heartbeat note), so
	-- nothing closes it until the leader screen goes. During a deal session
	-- the agent keeps that screen up for `DealAnswerHoldSeconds` (30) waiting
	-- for the rival's verdict, which the core cannot compute behind the lock.
	--
	-- Live 2026-11-15, Pericles culture seat, civvis-20261115T071444Z and its
	-- reload: 70 asks, 66 closed inside 0.8 s. The other four (t134, t142,
	-- t159, t175) each had a Rock Band concert land 0.02 to 0.25 s after the
	-- ask; `RockBandMoviePopup` held its lock for the whole 30 s and gave it
	-- back within 0.25 s of the leader screen closing on the timeout. In
	-- three of the four the rival then spoke 0.1 to 1.0 s after that release.
	-- Two of them hung the rival's turn (t159, t175: the verdict came as the
	-- rival's own session, which the game left `Adding to pending`), the
	-- first costing 6.8 minutes and a reload. No ask of those two runs waited
	-- more than 3 s without such a lock.
	--
	-- So the diplomacy view that is up for a deal session watches the shared
	-- ledger, and a lock another context has held for `BEHIND_DEAL_SECONDS`
	-- is handed back by its owner: `LuaEvents.CivvisCloseBehindDeal(ctx, id)`
	-- reaches every context's shim, and the one named runs its ordinary close
	-- (`endScreen`, the popup's own `Close()`), the same call it would make
	-- 0.05 s after coming up. The grace lets the popup finish the handler
	-- that took the lock. Nothing is released from outside its owner, a lock
	-- is asked for once, and the operator's Pause, the end screens and the
	-- wonder movie are left alone as everywhere else in this file.
	local BEHIND_DEAL_SECONDS = 0.25;
	local behindDeal = { seen = {}, asked = {} };
	local function closeLocksBehindDeal(dt)
		local ledger = nil;
		pcall(function() ledger = ExposedMembers.CivvisEventLocks; end);
		if type(ledger) ~= "table" or type(ledger.held) ~= "table" then return; end
		-- Collected first and asked after the walk: the owner's close takes
		-- its entry out of `held`, and whatever the freed game dispatches
		-- next may put another in.
		local due = {};
		for id, entry in pairs(ledger.held) do
			local ctx = type(entry) == "table" and entry.ctx or nil;
			-- The wonder movie keeps its own clock (`wonderAnimationReady`):
			-- closing it unseen would cut it from the recording.
			if type(ctx) == "string" and ctx ~= "DiplomacyActionView"
					and ctx ~= "DiplomacyDealView" and ctx ~= "WonderBuiltPopup"
					and not behindDeal.asked[id] then
				local seen = (behindDeal.seen[id] or 0) + dt;
				behindDeal.seen[id] = seen;
				if seen >= BEHIND_DEAL_SECONDS then
					due[#due + 1] = { id = id, ctx = ctx, seen = seen };
				end
			end
		end
		-- Forget locks that are gone, so the two tables stay as small as the
		-- ledger itself (at most EVENT_LOCK_CAP entries).
		for id in pairs(behindDeal.seen) do
			if ledger.held[id] == nil then behindDeal.seen[id] = nil; end
		end
		for id in pairs(behindDeal.asked) do
			if ledger.held[id] == nil then behindDeal.asked[id] = nil; end
		end
		for _, lock in ipairs(due) do
			behindDeal.asked[lock.id] = true;
			behindDeal.seen[lock.id] = nil;
			report("autoclose_behind_deal", string.format(
				',"holder":"%s","id":%d,"held_for":%.2f',
				lock.ctx, tonumber(lock.id) or -1, lock.seen));
			pcall(function() LuaEvents.CivvisCloseBehindDeal(lock.ctx, lock.id); end);
		end
	end
	pcall(function()
		LuaEvents.CivvisCloseBehindDeal.Add(function(holder, id)
			if holder ~= NAME or END_SCREENS[NAME] or operatorHolds()
					or CivvisHotseatScreen.personTurn() then return; end
			if NAME == "DiplomacyActionView" or NAME == "DiplomacyDealView"
					or NAME == "WonderBuiltPopup" then return; end
			-- Only while this context still holds that lock: a popup that
			-- closed in the meantime has nothing to give back.
			local entry = nil;
			pcall(function() entry = ExposedMembers.CivvisEventLocks.held[id]; end);
			if type(entry) ~= "table" or entry.ctx ~= NAME then return; end
			local ended = false;
			pcall(function() ended = endScreen(1); end);
			local released = true;
			pcall(function() released = ExposedMembers.CivvisEventLocks.held[id] == nil; end);
			report("autoclose", string.format(
				',"after":0.00,"ended":%s,"gone":%s,"behind_deal":true,"released":%s',
				tostring(ended), tostring(not isUp()), tostring(released)));
		end);
	end);

	local function tick(fDTime)
		local realDT = realSeconds(fDTime);
		-- ★★★★ PROOF THAT THE UI THREAD IS STILL ALIVE WHILE THE GAME IS NOT.
		--
		-- The dominant way a run now dies is a parked Game Core: the agent reports
		-- one blocker, then `GameCoreEventPublishComplete` stops firing and the
		-- agent — which is driven ONLY by that event — never ticks again. Every
		-- sampled thread sits in `__psynch_cvwait`; of 35 threads in the sample
		-- from run civvis-20260829T163259Z exactly one had a non-idle frame, and
		-- that was a single Metal sample. The game is computing nothing and
		-- waiting for input only the controller can give.
		--
		-- Whether that is recoverable turns on one question this log could not
		-- answer: is the UI thread still running? The agent's silence proves
		-- nothing about it, and this context's silence proves nothing either,
		-- because it only ever spoke when a screen was up. So say so once a
		-- minute. `ContextPtr:SetUpdate` runs on the UI thread every frame even
		-- while this screen is hidden — that is why `isUp()` is the first thing
		-- below — so a heartbeat here is a heartbeat for the whole UI context.
		--
		-- ⚠ A per-frame `SetUpdate` is NOT available to the agent: it was tried
		-- and does not run in a script-only in-game context (see the note beside
		-- `onGameCoreTick`). That is exactly why the answer has to come from here.
		--
		-- Read it as: heartbeats continuing after the agent's last event means the
		-- UI thread lives and a nudge from this side could unpark the turn;
		-- heartbeats stopping too means the whole process is gone and only the
		-- outside watchdog can help.
		heartbeatFrames = heartbeatFrames + 1;
		heartbeatSeconds = heartbeatSeconds + realDT;
		if heartbeatFrames >= HEARTBEAT_FRAMES then
			report("ui_heartbeat", string.format(',"frames":%d,"seconds":%.1f,"up":%s',
			                                     heartbeatFrames, heartbeatSeconds,
			                                     tostring(isUp())));
			heartbeatFrames = 0;
			heartbeatSeconds = 0;
		end
		if not isUp() then
			dialogueFadeExpired = false;
			controllerPulseSeconds = 0;
			showing = false;
			closes = 0;
			reported = false;
			desktopReportedAt = -1;
			wonderAnimationWaitReported = false;
			dealForceClose = false;
			return;
		end
		if END_SCREENS[NAME] then return; end -- the harness owns native victory playback
		-- A leader screen can hide TopPanel while its own UI clock continues.
		-- Use the same guarded agent wakeup here; keep deal holds and native
		-- close handling below intact. One long frame produces only one pulse.
		if cfg.Play ~= false and cfg.CivvisDecides then
			controllerPulseSeconds = controllerPulseSeconds + realDT;
			if controllerPulseSeconds >= 1 then
				controllerPulseSeconds = 0;
				pcall(function() LuaEvents.CivvisControlPulse(NAME); end);
				if not isUp() then showing = false; return; end
			end
		end
		-- After the pulse above, which is how a held agent hears the resume
		-- while a leader screen hides TopPanel. The clock restarts on resume.
		if operatorHolds() or CivvisHotseatScreen.personTurn() then
			showing = false;
			return;
		end
		if not showing then
			showing = true;
			remaining = SECONDS;
			shown = 0;
		end
		local dt = realDT;
		local civvisDealView = NAME == "DiplomacyActionView" or NAME == "DiplomacyDealView";
		if civvisDealView and dealHold > 0 then
			dealHold = dealHold - dt;
			if not holdReported then
				holdReported = true;
				report("autoclose_hold", string.format(',"seconds":%.2f', dealHold));
			end
			-- The verdict this hold waits for cannot come while another
			-- context holds the game (see `closeLocksBehindDeal`).
			pcall(closeLocksBehindDeal, dt);
			if dealHold > 0 then return; end
			-- `DealCounterRefuse` (opt-in): before closing, hand an expired
			-- hold back to the agent, which may decline a refusal left on the
			-- deal screen (`CivvisTrade.onHoldExpired`) and re-arm the hold.
			-- The deal view is up by being here; the action view's deal mode
			-- hides its conversation panel (DiplomacyActionView.lua:2552).
			if cfg.DealCounterRefuse == true then
				local dealUp = nil;
				if NAME == "DiplomacyDealView" then
					dealUp = true;
				else
					pcall(function() dealUp = Controls.ConversationContainer:IsHidden() == true; end);
				end
				pcall(function() LuaEvents.CivvisDealHoldExpired(NAME, dealSubject, dealUp); end);
				if dealHold > 0 then return; end
			end
			-- A deal session with no rival statement can leave BlackFadeAnim
			-- running forever.  The hold is our explicit ownership proof, so the
			-- close ladder may act even when the ordinary dialogue readiness gate
			-- cannot.  The later `isUp()` check still decides whether it worked.
			dealForceClose = true;
			report("deal_session_timeout");
		end
		remaining = remaining - dt;
		shown = shown + dt;
		-- ApplyStatement has already filtered and selected this response. Send
		-- it on the next ready frame, including a follow-up invitation arriving
		-- during the previous response's timer or a stale 30-second back-off.
		-- Keep the deal ownership hold above and the native fade gate below.
		local responseReady = NAME == "DiplomacyActionView"
			and ((firstMeetChoice ~= nil and not firstMeetAnswered)
                or (dialogueChoice ~= nil and not dialogueAnswered
                    and type(dialogueCallback) == "function"));
		if remaining > 0 and not responseReady then return; end
		if civvisDealView and not dealForceClose and not dialogueFadeExpired
				and not dialogueReady() then
			if shown < DIALOGUE_FADE_TIMEOUT_SECONDS then
				-- Allow a normal transition, but bound a frozen animation.
				remaining = DIALOGUE_READY_RETRY_SECONDS;
				return;
			end
			-- A frozen transition must not prevent the native exit ladder forever.
			dialogueFadeExpired = true;
			report("autoclose_fade_timeout");
		end
		local wonderAnimationReadyAtClose = true;
		local wonderAnimationTimedOut = false;
		if NAME == "WonderBuiltPopup" then
			local ready, stateKnown = wonderAnimationReady();
			if not ready then
				if shown < WONDER_ANIMATION_TIMEOUT_SECONDS then
					if not wonderAnimationWaitReported then
						wonderAnimationWaitReported = true;
						report("autoclose_wait_animation", string.format(
							',"minimum":%.2f,"timeout":%.2f,"state_known":%s',
							WONDER_MIN_SECONDS, WONDER_ANIMATION_TIMEOUT_SECONDS,
							tostring(stateKnown)));
					end
					remaining = DIALOGUE_READY_RETRY_SECONDS;
					return;
				end
				-- The controls could not report their state. Dismiss rather than
				-- leave an unreadable modal over the game indefinitely.
				wonderAnimationReadyAtClose = false;
				wonderAnimationTimedOut = true;
			end
		end

		-- Re-armed before the close, not after: a screen with more
		-- announcements queued behind it stays up, and the next one is owed
		-- its own two seconds rather than inheriting this one's spent clock.
		local upFor = shown;
		remaining = SECONDS;
		shown = 0;
		closes = closes + 1;
		-- ★★★★★ THE RUNG NUMBER CYCLES; THE FAILURE COUNT DOES NOT.
		--
		-- Every bounded rung in `endScreen` is `attempt <= N` with N ≤ 18, and
		-- `closes` used to be passed straight through — so from the 19th try
		-- on, and on EVERY 30-second retry after `autoclose_stuck`, the only
		-- rungs that ran were the tail (`OnHideScreen`/`OnClose`/`Close`), which
		-- for a diplomacy view is `CloseFocusedState(false)` again and again.
		-- Measured on civvis-20260816T175306Z (leading a batch-7 game, 731 vs
		-- 958 at t207) and civvis-20260816T115139Z (leading 804 vs 715 at
		-- t178): a late FIRST-CONTACT leader scene, the twenty tries spent
		-- inside ~5 s while the intro was still fading in (`gone:false` on all),
		-- then `autoclose … ended:true gone:false` every 30 s for the rest of
		-- the 900 s watchdog — `CloseSession`, the CHOICE_* answers and the
		-- initial-statement exit never ran again, the forced end turn could not
		-- take past the open session, and both games died on the clock. The
		-- rung is now `((closes - 1) % GIVE_UP_AFTER) + 1`, so a retry burst
		-- walks the whole ladder again; `closes` keeps counting failures for
		-- the desktop and stuck thresholds and the back-off.
		local rung = ((closes - 1) % GIVE_UP_AFTER) + 1;
		local ended = false;
		pcall(function() ended = endScreen(rung); end);
		if NAME == "InGamePopup" then dialogIsAnnouncement = false; end
		-- ⚠ `ended` IS NOT EVIDENCE THAT THE SCREEN CLOSED. It is whatever
		-- `endScreen` returned, and every rung in there returns true when its
		-- `pcall` did not throw -- the "pcall success is not acceptance" trap
		-- this project keeps paying for. Measured on run civvis-20260731T144251Z:
		-- DiplomacyActionView and TechCivicCompletedPopup each reported
		-- `ended: true` on 20 of 20 attempts and then `autoclose_stuck`, with the
		-- screen still sitting over the map. Reading those events, there was no
		-- way to tell a rung that worked from one that did nothing.
		--
		-- `isUp` is the same test the shipped popup manager uses, so ask it.
		-- It may read pessimistically when a hide lands at end of frame -- a
		-- screen that really closed then shows one `gone: false` and no more,
		-- because the next tick resets the counter -- so COUNT is the signal:
		-- one line means closed, twenty mean stuck.
		local gone = not isUp();
		-- ⚠ SAY WHICH STATE A DIPLOMACY VIEW IS STUCK IN. Twenty `gone:false`
		-- lines could not tell a first-contact cinema still fading in from a
		-- conversation waiting on an answer from a session the core has already
		-- closed — and those need different rungs. `ms_currentViewMode`,
		-- `ms_ActiveSessionID`, the black-fade animation and the popup dialog
		-- are the shipped script's own globals/controls, read only.
		local detail = "";
		if NAME == "DiplomacyActionView" or NAME == "DiplomacyDealView" then
			local mode = -1;
			pcall(function() if type(ms_currentViewMode) == "number" then mode = ms_currentViewMode; end end);
			local session = -1;
			pcall(function() if type(ms_ActiveSessionID) == "number" then session = ms_ActiveSessionID; end end);
			local fading = "nil";
			pcall(function()
				if Controls ~= nil and Controls.BlackFadeAnim ~= nil then
					fading = tostring(not Controls.BlackFadeAnim:IsStopped());
				end
			end);
			local popup = "nil";
			pcall(function()
				if m_PopupDialog ~= nil then popup = tostring(m_PopupDialog:IsOpen()); end
			end);
			detail = string.format(',"rung":%d,"mode":%d,"session":%d,"fading":%s,"popup":%s',
			                       rung, mode, session, fading, popup);
		end
		-- ⚠ WHICH DIALOG. `InGamePopup` is one context rendering every generic
		-- dialog in the game, so `screen:"InGamePopup"` names nothing. The
		-- command strings are what the dialog's author wrote, and they are the
		-- only field that distinguishes a one-button acknowledgement from a
		-- confirmation with a Cancel from a forced choice. Without them the
		-- question "which dialogs actually reach an unattended run" can only be
		-- answered by reasoning about shipped source, which is how the rule
		-- above came to protect against a screen that lives somewhere else.
		if NAME == "InGamePopup" then
			detail = detail .. string.format(',"buttons":"%s"', dialogButtons);
		end
		local animationDetail = "";
		if NAME == "WonderBuiltPopup" then
			animationDetail = string.format(
				',"animation_ready":%s,"animation_timeout":%s',
				tostring(wonderAnimationReadyAtClose), tostring(wonderAnimationTimedOut));
		end
		report("autoclose", string.format(',"after":%.2f,"ended":%s,"gone":%s%s%s',
		                                  upFor, tostring(ended), tostring(gone),
		                                  animationDetail, detail));
		-- ★★★★★ A CLOSE THAT WORKED MUST CLEAR THE COUNTER HERE, NOT LATER.
		--
		-- `closes` is meant to count consecutive FAILURES, and the `not isUp()`
		-- branch above is meant to reset it between screens. That branch does not
		-- run: an update installed on a popup context does not tick while the
		-- context is hidden, so the only ticks that ever happen are the ones with
		-- a screen up. `closes` therefore counted every successful close for the
		-- lifetime of the run and nothing ever reset it.
		--
		-- Measured on run civvis-20260731T144251Z: TechCivicCompletedPopup's 20
		-- closes are spread over turns 8 to 64 and DiplomacyActionView's over
		-- turns 11 to 49 -- one every few turns, each a different popup that
		-- closed correctly. Both then hit exactly GIVE_UP_AFTER and were declared
		-- stuck. Nothing was stuck. Every context simply died on its 20th popup
		-- and covered the map from then on, which is why a run gets worse the
		-- longer it lasts.
		if gone then
			closes = 0;
			reported = false;
			desktopReportedAt = -1;
			dealForceClose = false;
		end
		-- ⚠⚠⚠ THE ASK WAS LATCHED, AND THE LATCH ONLY CLEARS WHEN THE SCREEN
		-- GOES AWAY -- which is exactly what it does not do when help is needed.
		-- The declaration above already states the rule this broke: "Giving up is
		-- a BACK-OFF, never a stop", and the give-up arm below spells out why a
		-- latched flag is no good, because "the only code that could clear it is
		-- the `not isUp()` branch, and that branch does not run". `desktopReported`
		-- was such a flag.
		--
		-- A leader conversation cannot be dismissed blind: Escape does nothing on
		-- it (verified by hand) and Escape with nothing to close opens the pause
		-- menu, so the desktop side must SEE the screen to choose an option. That
		-- capture can fail transiently -- macOS returns no image while its status
		-- service is busy -- and one such failure used to end the run, because the
		-- ask never came again.
		--
		-- Measured 2026-08-29, run civvis-20260829T093602Z: one
		-- `autoclose_desktop` for DiplomacyDealView at 4 attempts, the desktop
		-- side answered "popup capture unavailable", and the game sat on John
		-- Curtin's leader screen until the watchdog killed it at turn 40. The
		-- photograph shows the diplomacy action list still up.
		--
		-- So ask again every DESKTOP_AFTER attempts while the screen is still
		-- there. The close attempts are already backed off by RETRY_SECONDS below,
		-- so this is a slow retry, not a spin.
		if closes >= DESKTOP_AFTER and closes - desktopReportedAt >= DESKTOP_AFTER then
			desktopReportedAt = closes;
			report("autoclose_desktop", string.format(',"attempts":%d', closes));
		end
		if closes >= GIVE_UP_AFTER then
			-- ⚠⚠ GIVING UP MUST NOT BE PERMANENT, AND MUST NOT BE ABOUT THE CONTEXT.
			--
			-- This used to call `ContextPtr:ClearUpdate()`, which unhooks the update
			-- from the CONTEXT rather than from the screen in front of it. Combined
			-- with the counter bug above, every context died on its 20th popup and
			-- covered the map for the rest of the run -- the operator's "not all
			-- screens are always closing out", and worse the longer a game ran.
			--
			-- A latched flag would be no better: the only code that could clear it
			-- is the `not isUp()` branch, and that branch does not run. Anything
			-- that can only be undone while hidden is permanent in practice.
			--
			-- So back off instead of stopping. The screen is left alone for
			-- RETRY_SECONDS and then tried again, forever; the report is emitted
			-- once per screen rather than once per attempt.
			if not reported then
				reported = true;
				report("autoclose_stuck", string.format(',"attempts":%d', closes));
			end
			if closes % GIVE_UP_AFTER == 0 then remaining = RETRY_SECONDS; end
		end
	end

	ContextPtr:SetUpdate(tick);
	report("autoclose_armed", string.format(',"seconds":%.2f', SECONDS));
end

-- ★★ A CIVVIS SEAT'S ERA SCORE IS NOT LEFT FOR THE PERSON'S TURN. Owner,
-- 2026-11-28, while playing: "make sure we do not get the era score updates
-- from the last player when we start our turn". The screens below come up on
-- a CIVVIS seat's turn (drawn under the cover, `CivvisCoverQueue.install`) and
-- each closes itself at that seat's end of turn in hotseat:
--   HistoricMoments.lua:533-538   OnLocalPlayerTurnEnd: if GameConfiguration.IsHotseat() and
--                                     ContextPtr:IsVisible() then Close();
--   TechCivicCompletedPopup.lua:398-402, BoostUnlockedPopup.lua:340-344, the same.
-- One raised after it, in the rollover while the last CIVVIS seat is still
-- the local player, or one its own close did not take (`Close` asks
-- `ContextPtr:IsVisible()`, HistoricMoments.lua:446-447), is still in the popup
-- queue when the person's turn begins, and opens for them as the cover lifts:
-- a CIVVIS seat's timeline and era score (`ShowNewTimelineMoment` titles it
-- for whoever is local, :128-135). In the owner's games the Historic Moments
-- context was up 0.5-1.6 s into the person's turn after a CIVVIS seat's moment
-- late in the round (hotseat-20261128T045523Z turns 16, 26, 29;
-- hotseat-20261128T062253Z turn 3: `ui_heartbeat` "up":true), and this closer
-- reported no close under the cover in either game. So in a cover game, as
-- the person's turn begins (`Events.LocalPlayerTurnBegin`, under the cover:
-- the hand-over is pressed after it), a screen of these still up or queued that
-- a CIVVIS seat's turn queued (`CivvisCoverQueue.owner`) is closed with the
-- shipped lines of its own close; one the person's turn queued is theirs.
-- Late arrivals: each script ignores another player's event (HistoricMoments
-- .lua:139, BoostUnlockedPopup.lua:267/:288); a moment kept in Historic
-- Moments' own list for another player (`popupData.playerID`, :147) is dropped
-- if it would open on the person's turn.
CivvisCoverQueue.LEFT = { HistoricMoments = true, EraReviewPopup = true,
	BoostUnlockedPopup = true, TechCivicCompletedPopup = true };
CivvisCoverQueue.left = 0;
function CivvisCoverQueue.person(pid)
	return CivvisCoverQueue.listed(cfg.HotseatHumanSeats, pid);
end
function CivvisCoverQueue.civvis(pid)
	return CivvisCoverQueue.listed(cfg.HotseatSeats, pid) and not CivvisCoverQueue.person(pid);
end
-- Each screen's own close, as shipped, less what needs it on screen first.
function CivvisCoverQueue.closeLeft()
	if NAME == "HistoricMoments" then
		-- HistoricMoments.lua:446-452, without `if ContextPtr:IsVisible()` and the sound.
		UIManager:DequeuePopup(ContextPtr);
		LuaEvents.HistoricMoments_Closed();
	elseif NAME == "TechCivicCompletedPopup" then
		Close();          -- :303-308, which also drops its queued data
	elseif NAME == "BoostUnlockedPopup" then
		-- :313-319, and the boosts it kept in its own list, unshown (`ShowBoost` stands down).
		CivvisCoverQueue.dropping = true;
		pcall(OnClose);
		if type(ShowNextQueuedPopup) == "function" then
			for _ = 1, 16 do pcall(ShowNextQueuedPopup); end
		end
		CivvisCoverQueue.dropping = false;
	elseif NAME == "EraReviewPopup" then
		OnClose();        -- :241-243 (not Continue, which raises a dedication for whoever is local)
	end
end
function CivvisCoverQueue.leftBehind()
	local c = CivvisCoverQueue;
	local pid = Game.GetLocalPlayer();
	local owner = tonumber(c.owner);
	if not c.person(pid) or owner == nil or owner == pid or not c.civvis(owner) then return false; end
	local up = false;
	pcall(function() up = ContextPtr:IsHidden() == false; end);
	if not up then pcall(function() up = UIManager:IsInPopupQueue(ContextPtr) == true; end); end
	if not up then return false; end
	local closed = pcall(c.closeLeft);
	c.left = c.left + 1;
	report("hotseat_cover_left", string.format(',"owner":%d,"player":%d,"closed":%s',
		owner, tonumber(pid) or -1, tostring(closed)));
	return closed;
end
if CivvisCoverQueue.on() and CivvisCoverQueue.LEFT[NAME] then
	pcall(function()
		Events.LocalPlayerTurnBegin.Add(function() pcall(CivvisCoverQueue.leftBehind); end);
	end);
	if NAME == "HistoricMoments" and type(ShowNewTimelineMoment) == "function" then
		local shown = ShowNewTimelineMoment;
		ShowNewTimelineMoment = function(popupData, ...)
			local owner = type(popupData) == "table" and tonumber(popupData.playerID) or nil;
			local pid = nil;
			pcall(function() pid = Game.GetLocalPlayer(); end);
			if owner ~= nil and owner ~= pid and CivvisCoverQueue.person(pid) then
				report("hotseat_cover_left", string.format(',"owner":%d,"player":%d,"dropped":true',
					owner, tonumber(pid) or -1));
				return;
			end
			if owner ~= nil then CivvisCoverQueue.owner = owner; end
			return shown(popupData, ...);
		end;
	end
	if NAME == "BoostUnlockedPopup" and type(ShowBoost) == "function" then
		local boost = ShowBoost;
		ShowBoost = function(...)
			if CivvisCoverQueue.dropping then return; end
			return boost(...);
		end;
	end
end
