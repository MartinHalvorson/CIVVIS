-- ★ DIAGNOSTIC (`tools/civ6_hotseat.py --probe still|turns`): what a
-- see-through wait would stand on, measured in a live hotseat game.
--
-- The person asked to keep looking at their own map, with their own fog, while
-- CIVVIS plays. In hotseat the screen is the playing seat's, so that needs:
--   1. the map drawn with ANOTHER seat's fog. The engine has a "local observer"
--      apart from the local player (`Game.GetLocalObserver()`); the one setter
--      is the gameplay side's `PlayerManager.SetLocalObserverTo`
--      (CivvisHotseatObserver.lua). Does the drawn fog follow it, and what does
--      a seat change do to it?
--   2. the playing seat's HUD and world labels out of sight, without stopping
--      the contexts CIVVIS's controller is clocked from;
--   3. the map panned and zoomed from under a popup that consumes every click.
--
-- `still`: on the person's turn 1, once their hand-over is closed, a timed list
-- of steps, each named on screen (Controls.Step) so a screenshot says what it
-- shows, each one `probe_step` record with the camera, the selection and the
-- observer as the step begins. Steps named `act:...` are where the driver
-- outside (the harness side) sends real input. Nobody plays: the game sits on
-- the person's turn.
-- `turns`: with a stub in the person's seat (`--auto-human gated`), one record
-- per turn begin and end, and on the CIVVIS seat's turns the observer is set
-- to the person and sampled, then the HUD groups are hidden turn by turn, to
-- see whether a seat change resets the observer and whether CIVVIS still plays.
--
-- Installed only with `HotseatProbe` set. Nothing here is the product.

local cfg = CivvisControlConfig or {};
local PREFIX = "CIVVISJSON ";
local RUN = tostring(cfg.RunTag or "unset");

CivvisProbe = { mode = cfg.HotseatProbe, at = nil, next = 1, clock = 0, hidden = {},
	samples = 0, sampleAt = 0, zoom = nil };

CivvisProbe.HUD = { "/InGame/HUD", "/InGame/Screens", "/InGame/TopLevelHUD" };
CivvisProbe.TOP = { "/InGame/PartialScreens" };
-- Which of these resolve is itself a finding (InGame.xml:23-26 nests the two
-- managers in the `BannerAndFlags` container under `WorldViewControls`).
CivvisProbe.FLAGS = { "/InGame/WorldViewControls/BannerAndFlags", "/InGame/BannerAndFlags",
	"/InGame/CityBannerManager", "/InGame/UnitFlagManager" };
CivvisProbe.LABELS = { "/InGame/WorldViewPlotMessages", "/InGame/SelectedUnit",
	"/InGame/WorldViewControls/WorldViewPlotMessages", "/InGame/WorldViewControls/SelectedUnit" };

function CivvisProbe.field(value)
	if type(value) == "boolean" then return tostring(value); end
	if type(value) == "number" and value == value then return string.format("%.14g", value); end
	if type(value) == "string" then return '"' .. value:gsub('[%c"\\]', "_") .. '"'; end
	return "null";
end
function CivvisProbe.report(kind, fields)
	local parts = { string.format('"kind":"%s","ctx":"probe","run":"%s"', kind, RUN) };
	for _, pair in ipairs(fields or {}) do
		parts[#parts + 1] = string.format('"%s":%s', pair[1], CivvisProbe.field(pair[2]));
	end
	pcall(function() Automation.Log(PREFIX .. "{" .. table.concat(parts, ",") .. "}"); end);
	pcall(function()
		Automation.Log(PREFIX .. string.format('{"kind":"probe_flush","ctx":"probe","run":"%s"}', RUN));
	end);
end
function CivvisProbe.state()
	local s = {};
	pcall(function() s.lx, s.ly = UI.GetMapLookAtWorldTarget(); end);
	pcall(function() s.zoom = UI.GetMapZoom(); end);
	pcall(function() s.observer = Game.GetLocalObserver(); end);
	pcall(function() s.player = Game.GetLocalPlayer(); end);
	pcall(function() s.turn = Game.GetCurrentGameTurn(); end);
	pcall(function() s.mode = UI.GetInterfaceMode(); end);
	s.unit = "none";
	pcall(function()
		local unit = UI.GetHeadSelectedUnit();
		if unit ~= nil then s.unit = unit:GetOwner() .. ":" .. unit:GetID(); end
	end);
	return s;
end
function CivvisProbe.step(name, extra)
	local s = CivvisProbe.state();
	pcall(function() Controls.Step:SetText("PROBE: " .. name); end);
	local fields = { { "step", name }, { "turn", s.turn }, { "player", s.player },
		{ "observer", s.observer }, { "lx", s.lx }, { "ly", s.ly }, { "zoom", s.zoom },
		{ "unit", s.unit }, { "mode", s.mode } };
	for _, pair in ipairs(extra or {}) do fields[#fields + 1] = pair; end
	CivvisProbe.report("probe_step", fields);
end

-- The observer, set one of the three ways there could be.
function CivvisProbe.observe(pid, via)
	local ok, why = false, "";
	if via == "gameplay" then
		if not pcall(function() ok, why = ExposedMembers.CivvisObserver.set(pid); end) then
			ok, why = false, "ExposedMembers.CivvisObserver is not there";
		end
	elseif via == "ui" then
		ok, why = pcall(function() PlayerManager.SetLocalObserverTo(pid); end);
	elseif via == "autoplay" then
		ok, why = pcall(function() AutoplayManager.SetObserveAsPlayer(pid); end);
	end
	local now = nil;
	pcall(function() now = Game.GetLocalObserver(); end);
	return { { "via", via }, { "to", pid }, { "ok", ok == true }, { "why", ok == true and "" or tostring(why) },
		{ "observer_after", now } };
end
function CivvisProbe.surface()
	local fields = {};
	local function has(name, read)
		local kind = "error";
		pcall(function() kind = type(read()); end);
		fields[#fields + 1] = { name, kind };
	end
	has("ui_PlayerManager", function() return PlayerManager; end);
	has("ui_SetLocalObserverTo", function() return PlayerManager.SetLocalObserverTo; end);
	has("ui_AutoplayManager", function() return AutoplayManager; end);
	has("ui_SetObserveAsPlayer", function() return AutoplayManager.SetObserveAsPlayer; end);
	has("ui_GetLocalObserver", function() return Game.GetLocalObserver; end);
	has("gameplay_bridge", function() return ExposedMembers.CivvisObserver.set; end);
	pcall(function()
		local names, observer, player = ExposedMembers.CivvisObserver.surface();
		fields[#fields + 1] = { "gameplay_names", names };
		fields[#fields + 1] = { "gameplay_observer", observer };
		fields[#fields + 1] = { "gameplay_player", player };
	end);
	return fields;
end
-- Hide or show by path; says which paths resolved.
function CivvisProbe.hide(paths, hide)
	local found = {};
	for _, path in ipairs(paths) do
		pcall(function()
			local control = ContextPtr:LookUpControl(path);
			if control ~= nil then
				control:SetHide(hide);
				found[#found + 1] = path;
				CivvisProbe.hidden[path] = hide or nil;
			end
		end);
	end
	return { { "hide", hide }, { "resolved", table.concat(found, " ") } };
end
function CivvisProbe.showAll()
	for path in pairs(CivvisProbe.hidden) do
		pcall(function() ContextPtr:LookUpControl(path):SetHide(false); end);
	end
	CivvisProbe.hidden = {};
end
function CivvisProbe.glass(up, own)
	pcall(function() LuaEvents.CivvisProbeGlass(up, own); end);
end
function CivvisProbe.person()
	return tonumber((cfg.HotseatHumanSeats or {})[1]) or 0;
end
function CivvisProbe.rival()
	return tonumber((cfg.HotseatSeats or {})[1]) or 1;
end

-- ------------------------------------------------------------------ `still`
-- { seconds after the person's hand-over closed, name, what to do }.
function CivvisProbe.plan()
	local p = CivvisProbe;
	local me, rival = p.person(), p.rival();
	local steps = {
		{ 4, "baseline", function() return p.surface(); end },
		{ 8, "observer to the CIVVIS seat (gameplay script)", function() return p.observe(rival, "gameplay"); end },
		{ 16, "observer back to the person (gameplay script)", function() return p.observe(me, "gameplay"); end },
		{ 20, "observer to the CIVVIS seat (UI PlayerManager)", function() return p.observe(rival, "ui"); end },
		{ 28, "observer back to the person (UI PlayerManager)", function()
			p.observe(me, "gameplay");
			return p.observe(me, "ui");
		end },
		{ 32, "observer to the CIVVIS seat (AutoplayManager, autoplay off)", function() return p.observe(rival, "autoplay"); end },
		{ 40, "observer back to the person (AutoplayManager)", function()
			local fields = p.observe(me, "autoplay");
			p.observe(me, "gameplay");
			return fields;
		end },
		{ 44, "HUD, Screens, TopLevelHUD hidden", function() return p.hide(p.HUD, true); end },
		{ 50, "and PartialScreens hidden", function() return p.hide(p.TOP, true); end },
		{ 56, "HUD groups back", function()
			p.hide(p.TOP, false);
			return p.hide(p.HUD, false);
		end },
		{ 60, "banners and flags hidden", function() return p.hide(p.FLAGS, true); end },
		{ 66, "plot messages and selection hidden too", function() return p.hide(p.LABELS, true); end },
		{ 72, "world labels back", function()
			p.hide(p.LABELS, false);
			return p.hide(p.FLAGS, false);
		end },
		{ 76, "UI.PanMap(1, 0)", function() pcall(function() UI.PanMap(1, 0); end); end },
		{ 78, "UI.PanMap(0, 0)", function() pcall(function() UI.PanMap(0, 0); end); end },
		{ 82, "UI.SetMapZoom + 0.2", function()
			pcall(function()
				p.zoom = UI.GetMapZoom();
				UI.SetMapZoom(p.zoom + 0.2, 0.0, 0.0);
			end);
		end },
		{ 86, "zoom back", function() pcall(function() UI.SetMapZoom(p.zoom, 0.0, 0.0); end); end },
	};
	-- The driver's input, three times: on the open game (does the driver's
	-- input move the map at all), under a glass that only consumes (what gets
	-- past it), and under a glass that moves the map itself.
	local at = 90;
	for _, phase in ipairs({
		{ "open", nil },
		{ "glass, consuming only", function() p.glass(true, false); end },
		{ "glass, its own pan and zoom", function() p.glass(true, true); end },
	}) do
		local label, raise = phase[1], phase[2];
		steps[#steps + 1] = { at, label, raise or function() end };
		for i, act in ipairs({ "wheel", "key", "edge", "click", "drag" }) do
			steps[#steps + 1] = { at + 4 + (i - 1) * 7, "act:" .. act .. " (" .. label .. ")", function() end };
		end
		at = at + 4 + 5 * 7;
	end
	steps[#steps + 1] = { at, "glass down", function() p.glass(false, false); end };
	steps[#steps + 1] = { at + 4, "done", function() p.showAll(); end };
	return steps;
end
function CivvisProbe.start(pid)
	local p = CivvisProbe;
	if p.mode ~= "still" or p.at ~= nil or tonumber(pid) ~= p.person() then return; end
	p.at, p.steps = 0, p.plan();
	p.step("started");
end

-- ------------------------------------------------------------------ `turns`
function CivvisProbe.onTurnBegin()
	local p = CivvisProbe;
	if p.mode ~= "turns" then return; end
	local s = p.state();
	if s.player == p.person() then
		p.showAll();
		local fields = p.observe(p.person(), "gameplay");
		fields[#fields + 1] = { "observer_before", s.observer };
		p.step("turn_begin person", fields);
		p.samples = 0;
		return;
	end
	local turn = tonumber(s.turn) or 0;
	local fields = { { "observer_before", s.observer } };
	local did = "nothing";
	if turn >= 3 and turn <= 7 then
		for _, pair in ipairs(p.observe(p.person(), "gameplay")) do fields[#fields + 1] = pair; end
		did = "observer";
	end
	if turn == 4 then
		p.hide(p.HUD, true);
		did = did .. "+hud";
	elseif turn == 5 then
		p.hide(p.HUD, true);
		p.hide(p.TOP, true);
		did = did .. "+hud+top";
	elseif turn == 6 then
		p.hide(p.FLAGS, true);
		p.hide(p.LABELS, true);
		did = did .. "+flags";
	end
	fields[#fields + 1] = { "did", did };
	p.step("turn_begin civvis", fields);
	p.samples, p.sampleAt = 8, 0;
end
function CivvisProbe.onTurnEnd()
	if CivvisProbe.mode ~= "turns" then return; end
	CivvisProbe.step("turn_end");
end

function CivvisProbe.tick(dt)
	local p = CivvisProbe;
	dt = math.min(math.max(tonumber(dt) or 0, 0), 3);
	if p.mode == "still" and p.at ~= nil then
		p.at = p.at + dt;
		while p.steps[p.next] ~= nil and p.at >= p.steps[p.next][1] do
			local step = p.steps[p.next];
			p.next = p.next + 1;
			local ok, extra = pcall(step[3]);
			p.step(step[2], ok and extra or { { "error", tostring(extra) } });
		end
	elseif p.mode == "turns" and p.samples > 0 then
		p.sampleAt = p.sampleAt + dt;
		if p.sampleAt >= 0.2 then
			p.sampleAt = 0;
			p.samples = p.samples - 1;
			local s = p.state();
			p.report("probe_sample", { { "turn", s.turn }, { "player", s.player },
				{ "observer", s.observer }, { "left", p.samples } });
		end
	end
end

function CivvisProbe.arm()
	local p = CivvisProbe;
	if cfg.Hotseat ~= true or (p.mode ~= "still" and p.mode ~= "turns") then
		pcall(function() ContextPtr:SetHide(true); end);
		return false;
	end
	local function guarded(handler)
		return function(...) pcall(handler, ...); end;
	end
	pcall(function() LuaEvents.PlayerChange_Close.Add(guarded(p.start)); end);
	pcall(function() Events.LocalPlayerTurnBegin.Add(guarded(p.onTurnBegin)); end);
	pcall(function() Events.LocalPlayerTurnEnd.Add(guarded(p.onTurnEnd)); end);
	pcall(function() ContextPtr:SetUpdate(guarded(p.tick)); end);
	pcall(function() ContextPtr:SetShutdown(guarded(p.showAll)); end);
	p.report("probe_armed", { { "mode", p.mode } });
	return true;
end

CivvisProbe.arm();
