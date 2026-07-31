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
local SECONDS = tonumber(cfg.AnnouncementSeconds) or 2.0;
if SECONDS < 0 then SECONDS = 0; end

-- A screen that will not go away must not write a line every two seconds for
-- the rest of a multi-hour run. Legitimate repeats -- two wonders finishing on
-- the same turn, a queue of completed technologies -- clear this counter by
-- being hidden in between, so only a screen that ignores its own close
-- callback ever reaches the limit.
local GIVE_UP_AFTER = 20;

local PREFIX = "CIVVISJSON ";

local NAME = "unknown";
pcall(function() NAME = ContextPtr:GetID() or "unknown"; end);

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
local CHAINED = {
	NaturalDisasterPopup = "NaturalDisasterPopup_GranColombia_Maya",
};

-- Whether a screen is in there at all. An include that finds no file fails
-- silently on this build, so the test has to be for what the script defines
-- rather than for the include returning.
local function haveScreen()
	return type(OnClose) == "function" or type(Close) == "function"
	       or type(OnContinue) == "function" or type(OnClosePopup) == "function";
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
local function endScreen()
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
	-- Giving up is about THIS screen, never about the context. See `tick`.
	local gaveUp = false;

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
			-- The screen went down, so whatever we gave up on is gone and the
			-- next one gets the full budget again.
			gaveUp = false;
			return;
		end
		-- Stop hammering a screen that has had its attempts, but keep ticking:
		-- the reset above is the only way back, and it only runs while this
		-- update is still installed.
		if gaveUp then return; end
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
		pcall(function() ended = endScreen(); end);
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
		if closes >= GIVE_UP_AFTER then
			-- ⚠⚠ NEVER `ContextPtr:ClearUpdate()` HERE. It unhooks the update from
			-- the CONTEXT, not from this screen, and `closes` is only reset by the
			-- `not isUp()` branch of the tick that no longer runs -- so one stubborn
			-- screen permanently disables autoclose for every screen that context
			-- ever shows again. That is the operator's "not all screens are always
			-- closing out": the run above lost DiplomacyActionView and
			-- TechCivicCompletedPopup by turn 73, and every later leader conversation
			-- and completed-tech popup then sat over the map untouched for the rest
			-- of the run. Latch the screen instead; the context stays armed.
			gaveUp = true;
			report("autoclose_stuck", string.format(',"attempts":%d', closes));
		end
	end

	ContextPtr:SetUpdate(tick);
	report("autoclose_armed", string.format(',"seconds":%.2f', SECONDS));
end
