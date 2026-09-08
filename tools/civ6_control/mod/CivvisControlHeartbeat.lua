-- Keep the shipped HUD and its expansion chain, then borrow its visible UI
-- clock. Popup clocks stop when hidden; the script-only agent has no clock.
-- TopPanel_Expansion2.lua:5 includes Expansion1, which includes TopPanel.
pcall(function() include("TopPanel_Expansion2"); end);
if type(LateInitialize) ~= "function" then
	pcall(function() include("TopPanel_Expansion1"); end);
end
if type(LateInitialize) ~= "function" then include("TopPanel"); end

local cfg = CivvisControlConfig or {};
if cfg.Play ~= false and cfg.CivvisDecides then
	local elapsed = 0;
	ContextPtr:SetUpdate(function(dt)
		elapsed = elapsed + math.max(0, tonumber(dt) or 0);
		if elapsed < 1 then return; end
		-- A long frame gets one pulse, never a burst of catch-up calls.
		elapsed = 0;
		pcall(function() LuaEvents.CivvisControlPulse(); end);
	end);
end
