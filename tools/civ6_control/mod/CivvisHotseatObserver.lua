-- ★★ WHOSE FOG THE SCREEN DRAWS (gameplay script; hotseat with a person only).
--
-- In a hotseat game the map is drawn with the fog of the seat that is playing.
-- The engine keeps that as a separate thing from the local player, the "local
-- observer" (`Game.GetLocalObserver()`, read by WorldView/CameraManager.lua:19
-- and WorldViewIconsManager.lua:294), and the one call that sets it alone,
-- `PlayerManager.SetLocalObserverTo`, is the gameplay side's (Debug/Map.ltp:54
-- "Set Observer To Selected"); a UI context has only the getters. So it is
-- offered from here through `ExposedMembers`, the table both sides share.
-- Nothing calls it but the hotseat probe and the see-through wait.
local function set(pid)
	local done, why = pcall(function() PlayerManager.SetLocalObserverTo(pid); end);
	return done, done and "" or tostring(why);
end
-- What this side has, for the probe's record.
local function surface()
	local names = {};
	pcall(function()
		for name, value in pairs(PlayerManager) do
			if type(name) == "string" and (name:find("bserver") or name:find("LocalPlayer")) then
				names[#names + 1] = name .. ":" .. type(value);
			end
		end
	end);
	table.sort(names);
	local observer, player = nil, nil;
	pcall(function() observer = Game.GetLocalObserver(); end);
	pcall(function() player = Game.GetLocalPlayer(); end);
	return table.concat(names, " "), observer, player;
end
ExposedMembers.CivvisObserver = { set = set, surface = surface };
print("CivvisHotseatObserver: ready");
