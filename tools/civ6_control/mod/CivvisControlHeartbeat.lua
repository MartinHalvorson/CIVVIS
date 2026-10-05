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
CivvisFrameClock = { raw = 0, scale = 1 };
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
	if scale ~= fc.scale or type(fc.ui0) ~= "number" then
		fc.scale, fc.raw, fc.ui0 = scale, 0, ui;
	else
		fc.raw = fc.raw + raw;
		if ui - fc.ui0 >= 10 then
			local ratio = math.floor(fc.raw / (ui - fc.ui0) * 100 + 0.5) / 100;
			pcall(function()
				ExposedMembers.CivvisFrameClock = { scale = scale, ratio = ratio, at = ui };
			end);
			fc.raw, fc.ui0 = 0, ui;
		end
	end
	pcall(function()
		local ack = ExposedMembers.CivvisClockAck;
		ack.scale.Heartbeat = scale;
		ack.at.Heartbeat = ui;
	end);
	return raw / scale;
end

if cfg.Play ~= false and cfg.CivvisDecides then
	local elapsed = 0;
	local abTurns = math.floor(tonumber(cfg.VSyncABTurns) or 0);
	-- The agent's landed-orders peek on this clock, which keeps running while
	-- the game core publishes nothing (`CivvisQueue.onPeekPulse`). The agent
	-- bounds the query itself; this only offers it a chance each interval.
	local peekElapsed = 0;
	local peekEvery = math.max(0.02, tonumber(cfg.OrdersPeekSeconds) or 0.05);
	ContextPtr:SetUpdate(function(dt)
		local delta = CivvisFrameClock.frame(math.max(0, tonumber(dt) or 0));
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
		if abTurns > 0 then pcall(CivvisVSyncAB.pulse, abTurns); end
	end);
end
