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

-- The shipped screen, unchanged and unread. A context's id is the name of the
-- script the game loads into it, so asking for that name asks for exactly the
-- file this replacement stands in front of.
local loaded = pcall(function() include(NAME); end);

-- What a click on this screen's own button does. Everything shipped registers
-- OnClose; the era review is the exception explained at the top.
local function endScreen()
	if NAME == "EraReviewPopup" and type(OnContinue) == "function" then
		OnContinue();
		return true;
	end
	if type(OnClose) == "function" then OnClose(); return true; end
	if type(Close) == "function" then Close(); return true; end
	return false;
end

local armed = loaded and (type(OnClose) == "function" or type(Close) == "function"
                          or type(OnContinue) == "function");

if not armed then
	-- The failure that has to be loud. A replacement whose include did not
	-- land leaves the context with no shipped code at all, which does not look
	-- like a broken mod: the announcement simply never appears, and a run
	-- reads as a quiet game rather than a game missing a screen.
	report("autoclose_unarmed", string.format(',"included":%s', tostring(loaded)));
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

	local function tick(fDTime)
		if ContextPtr:IsHidden() then
			showing = false;
			closes = 0;
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
		pcall(function() ended = endScreen(); end);
		report("autoclose", string.format(',"after":%.2f,"ended":%s', upFor, tostring(ended)));
		if closes >= GIVE_UP_AFTER then
			ContextPtr:ClearUpdate();
			report("autoclose_stuck", string.format(',"attempts":%d', closes));
		end
	end

	ContextPtr:SetUpdate(tick);
	report("autoclose_armed", string.format(',"seconds":%.2f', SECONDS));
end
