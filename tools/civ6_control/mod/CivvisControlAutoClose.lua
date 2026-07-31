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

local PREFIX = "CIVVISJSON ";

local NAME = "unknown";
pcall(function() NAME = ContextPtr:GetID() or "unknown"; end);

-- Now that this context knows its own name, the era screens can get their
-- shorter clock. This must stay below `local NAME`.
if ERA_SCREENS[NAME] then SECONDS = ERA_SECONDS; end
if SECONDS < 0 then SECONDS = 0; end

local RUN = tostring(cfg.RunTag or "unset");

-- The agent's channel, and for the same reason: this build writes no Lua.log,
-- so Automation.Log is the only output that survives. Every field written here
-- is a name or a number, so the encoder the agent needs is not needed here.
local function report(kind, fields)
	local line = string.format('{"kind":"%s","ctx":"autoclose","screen":"%s","run":"%s"%s}',
	                           kind, NAME, RUN, fields or "");
	pcall(function() Automation.Log(PREFIX .. line); end);
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
		or type(OnContinue) == "function"
		or type(OnClosePopup) == "function"
		or type(OnHideScreen) == "function"        -- GreatWorkShowcase
		or type(OnButton1) == "function"           -- ChooseArtifact
		or type(ReleaseEventLock) == "function"    -- WorldCongressBetweenTurns
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
if CHAINED[NAME] then pcall(function() include(CHAINED[NAME]); end); end
if not haveScreen() then pcall(function() include(NAME); end); end

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
local dialogIsAnnouncement = false;

if NAME == "InGamePopup" and type(OnPopupOpen) == "function" then
	local basePopupOpen = OnPopupOpen;
	local function countingPopupOpen(id, options)
		local buttons = 0;
		if type(options) == "table" then
			for _, option in ipairs(options) do
				if option.Type == "Button" then buttons = buttons + 1; end
			end
		end
		dialogIsAnnouncement = (buttons == 1);
		return basePopupOpen(id, options);
	end
	local swapped = pcall(function()
		LuaEvents.OnRaisePopupInGame.Remove(basePopupOpen);
		LuaEvents.OnRaisePopupInGame.Add(countingPopupOpen);
	end);
	if swapped then OnPopupOpen = countingPopupOpen; end
end

-- What a click on this screen's own button does. Everything shipped registers
-- OnClose; the era review is the exception explained at the top.
local function endScreen(attempt)
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
	if NAME == "EraReviewPopup" and type(OnContinue) == "function" then
		OnContinue();
		return true;
	end
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
	if (attempt or 1) <= 6 and type(CloseFocusedState) == "function"
			and pcall(function() CloseFocusedState(true); end) then
		return true;
	end
	if (attempt or 1) <= 9 and type(ExitConversationMode) == "function"
			and pcall(function() ExitConversationMode(true); end) then
		return true;
	end
	if (attempt or 1) <= 12 and type(DiplomacyManager) == "table"
			and ms_ActiveSessionID ~= nil
			and pcall(function()
				DiplomacyManager.CloseSession(ms_ActiveSessionID);
			end) then
		return true;
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
	if (attempt or 1) <= 14 and type(OnSelectConversationDiplomacyStatement) == "function"
			and pcall(function()
				OnSelectConversationDiplomacyStatement("CHOICE_NEGATIVE");
			end) then
		return true;
	end
	if (attempt or 1) <= 15 and type(OnSelectConversationDiplomacyStatement) == "function"
			and pcall(function()
				OnSelectConversationDiplomacyStatement("CHOICE_IGNORE");
			end) then
		return true;
	end
	if (attempt or 1) <= 16 and type(OnSelectConversationDiplomacyStatement) == "function"
			and pcall(function()
				OnSelectConversationDiplomacyStatement("CHOICE_EXIT");
			end) then
		return true;
	end
	if (attempt or 1) <= 18 and type(OnSelectInitialDiplomacyStatement) == "function"
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
	-- The between-turns congress screen holds an EVENT LOCK, so closing it is not
	-- cosmetic: until the lock is released the game will not proceed. Releasing it
	-- is what the shipped Continue button does.
	if NAME == "WorldCongressBetweenTurns"
			and type(ReleaseEventLock) == "function" then
		ReleaseEventLock();
		return true;
	end
	if type(OnClose) == "function" then OnClose(); return true; end
	if type(Close) == "function" then Close(); return true; end
	return false;
end

if not haveScreen() then
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
	-- Whether this screen's failure has already been reported. Giving up is a
	-- BACK-OFF, never a stop -- see the end of `tick` for why nothing here may
	-- be permanent.
	local reported = false;

	-- How long to leave a screen alone once it has refused GIVE_UP_AFTER times.
	-- Long enough not to hammer it, short enough that the map comes back on its
	-- own if whatever held it goes away.
	local RETRY_SECONDS = 30.0;

	-- What "up" means. Everywhere else it is the context not being hidden, the
	-- same test the shipped popup manager uses. InGamePopup is *pushed as a
	-- modal* rather than shown, so it reads the flag the counting wrapper sets
	-- instead -- which is true only while a one-button dialog is open, and
	-- that is also the only time this screen may act at all.
	local function isUp()
		if NAME == "InGamePopup" then return dialogIsAnnouncement; end
		return not ContextPtr:IsHidden();
	end

	local function tick(fDTime)
		if not isUp() then
			showing = false;
			closes = 0;
			reported = false;
			return;
		end
		if not showing then
			showing = true;
			remaining = SECONDS;
			shown = 0;
		end
		local dt = tonumber(fDTime) or 0;
		remaining = remaining - dt;
		shown = shown + dt;
		if remaining > 0 then return; end

		-- Re-armed before the close, not after: a screen with more
		-- announcements queued behind it stays up, and the next one is owed
		-- its own two seconds rather than inheriting this one's spent clock.
		local upFor = shown;
		remaining = SECONDS;
		shown = 0;
		closes = closes + 1;
		local ended = false;
		pcall(function() ended = endScreen(closes); end);
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
		report("autoclose", string.format(',"after":%.2f,"ended":%s,"gone":%s',
		                                  upFor, tostring(ended), tostring(gone)));
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
		if gone then closes = 0; reported = false; end
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
			remaining = RETRY_SECONDS;
		end
	end

	ContextPtr:SetUpdate(tick);
	report("autoclose_armed", string.format(',"seconds":%.2f', SECONDS));
end
