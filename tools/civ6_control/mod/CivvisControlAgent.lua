-- Take a seat in Civilization VI and play it, turn after turn, unattended.
--
-- CivvisGrounding watches games the shipped AI plays with itself. This does
-- the other thing: it occupies a *human* slot and issues every order that slot
-- needs, so the game is under program control rather than merely observed.
-- That is what makes difficulty mean anything -- the handicap system gives its
-- bonuses to a human seat, so "beat this on Settler" is a claim about a
-- controller, not about the shipped AI playing both sides.
--
-- The turn loop is built around the game's own end-turn blockers rather than a
-- checklist of things a player might want to do. Civilization VI already knows
-- every decision it is waiting on -- research, civic, city production, a unit
-- without orders, a pantheon to pick -- and publishes them through
-- NotificationManager.GetFirstEndTurnBlocking. Asking the game what it wants
-- and answering that is smaller than enumerating decisions ourselves, and it
-- cannot silently skip a decision type this build has and this code does not
-- know about: an unrecognised blocker is reported by name rather than ignored.
--
-- Every order goes through the same UnitManager/CityManager/
-- UI.RequestPlayerOperation calls the shipped interface uses, so nothing here
-- bypasses a rule, and nothing here reads a pixel: state in, orders out.
--
-- Settings arrive as a `CivvisControlConfig` table prepended by the installer;
-- see CivvisControlSetup.lua for why they are prepended rather than included.

local cfg = CivvisControlConfig or {};
local PREFIX = "CIVVISJSON ";

-- ---------------------------------------------------------------- reporting

local function esc(s)
	s = tostring(s);
	s = s:gsub("\\", "\\\\"):gsub('"', '\\"');
	s = s:gsub("\n", "\\n"):gsub("\r", "\\r"):gsub("\t", "\\t");
	return s;
end

local encode;
encode = function(v)
	local t = type(v);
	if v == nil then
		return "null";
	elseif t == "boolean" then
		return v and "true" or "false";
	elseif t == "number" then
		if v == math.floor(v) and v == v and v ~= math.huge and v ~= -math.huge then
			return string.format("%d", v);
		end
		return string.format("%.6g", v);
	elseif t == "string" then
		return '"' .. esc(v) .. '"';
	elseif t == "table" then
		local n = 0;
		for _ in pairs(v) do n = n + 1; end
		local parts = {};
		if #v == n then
			for i = 1, #v do parts[#parts + 1] = encode(v[i]); end
			return "[" .. table.concat(parts, ",") .. "]";
		end
		local keys = {};
		for k in pairs(v) do keys[#keys + 1] = tostring(k); end
		table.sort(keys);
		for _, k in ipairs(keys) do
			parts[#parts + 1] = '"' .. esc(k) .. '":' .. encode(v[k]);
		end
		return "{" .. table.concat(parts, ",") .. "}";
	end
	return '"<' .. t .. '>"';
end

local function emit(kind, payload)
	payload = payload or {};
	payload.kind = kind;
	payload.ctx = "agent";
	payload.run = cfg.RunTag or "unset";
	local line = PREFIX .. encode(payload);
	pcall(function() print(line); end);
	pcall(function() Automation.Log(line); end);
	pcall(function() UI.DataError(line); end);
end

-- Every read of the game API is guarded. A method this ruleset does not have,
-- or an object in a transient state, must not take the whole turn down: a
-- controller that dies mid-turn leaves the game at a prompt forever, which
-- looks exactly like a game that is thinking.
local function try(fn, fallback)
	local ok, result = pcall(fn);
	if ok then return result; end
	return fallback;
end

-- --------------------------------------------------------------- action ids
--
-- Operations are looked up in GameInfo, not on the UnitOperationTypes table.
-- The named table is a convenience list and it is *not* complete: on this
-- build it has no SKIP_TURN, no SLEEP and no AUTOMATE_EXPLORE, while the
-- database defines all three. Reading a missing name off the enum yields nil,
-- the guarded call then refuses the order, and a unit that could have been
-- told to skip instead blocks the end of the turn forever.

local function opHash(name)
	return try(function()
		local row = GameInfo.UnitOperations[name];
		return row and row.Hash or nil;
	end);
end

local function cmdHash(name)
	return try(function()
		local row = GameInfo.UnitCommands[name];
		return row and row.Hash or nil;
	end);
end

local OP = {};
local CMD = {};

local function resolveActions()
	for _, name in ipairs({
		"UNITOPERATION_FOUND_CITY", "UNITOPERATION_MOVE_TO",
		"UNITOPERATION_FORTIFY", "UNITOPERATION_ALERT",
		"UNITOPERATION_SKIP_TURN", "UNITOPERATION_SLEEP",
		"UNITOPERATION_HEAL", "UNITOPERATION_AUTOMATE_EXPLORE",
		"UNITOPERATION_BUILD_IMPROVEMENT", "UNITOPERATION_RANGE_ATTACK",
		"UNITOPERATION_HARVEST_RESOURCE", "UNITOPERATION_REST_REPAIR",
	}) do
		OP[name] = opHash(name);
	end
	for _, name in ipairs({
		"UNITCOMMAND_AUTOMATE", "UNITCOMMAND_PROMOTE", "UNITCOMMAND_WAKE",
		"UNITCOMMAND_UPGRADE", "UNITCOMMAND_DELETE",
	}) do
		CMD[name] = cmdHash(name);
	end
	local resolved, missing = {}, {};
	for name, hash in pairs(OP) do
		if hash then resolved[#resolved + 1] = name else missing[#missing + 1] = name end
	end
	for name, hash in pairs(CMD) do
		if hash then resolved[#resolved + 1] = name else missing[#missing + 1] = name end
	end
	table.sort(resolved); table.sort(missing);
	emit("actions", { resolved = resolved, missing = missing });
end

-- ------------------------------------------------------------------ survey

local function typeName(kindTable, hash)
	return try(function()
		local row = kindTable[hash];
		return row and (row.DifficultyType or row.MapSizeType or row.GameSpeedType
			or row.Type) or nil;
	end);
end

local function enumMembers(getter)
	local ok, tbl = pcall(getter);
	if not ok or type(tbl) ~= "table" then return nil; end
	local names = {};
	for k, v in pairs(tbl) do
		if type(k) == "string" then names[#names + 1] = k .. "=" .. tostring(v); end
	end
	table.sort(names);
	return names;
end

local function survey()
	if cfg.Survey == false then return; end
	local pid = try(function() return Game.GetLocalPlayer(); end, -1);
	local player = (pid ~= nil and pid >= 0) and Players[pid] or nil;

	-- The configuration getters return hashes, not names. Reporting the hash
	-- is reporting nothing: "difficulty=-179952465" cannot be checked against
	-- the difficulty that was asked for, and the whole ladder is measured in
	-- difficulty. Resolve through GameInfo or say so.
	emit("seat", {
		local_player = pid,
		is_human = player and try(function() return player:IsHuman(); end, false) or false,
		civ = (pid ~= nil and pid >= 0) and try(function()
			return PlayerConfigurations[pid]:GetCivilizationTypeName();
		end, "?") or "?",
		leader = (pid ~= nil and pid >= 0) and try(function()
			return PlayerConfigurations[pid]:GetLeaderTypeName();
		end, "?") or "?",
		difficulty = typeName(GameInfo.Difficulties,
			try(function() return GameConfiguration.GetHandicapType(); end)) or "?",
		speed = typeName(GameInfo.GameSpeeds,
			try(function() return GameConfiguration.GetGameSpeedType(); end)) or "?",
		map = try(function() return MapConfiguration.GetScript(); end, "?"),
		size = typeName(GameInfo.Maps,
			try(function() return MapConfiguration.GetMapSize(); end)) or "?",
		max_turns = try(function() return GameConfiguration.GetMaxTurns(); end, -1),
		players = try(function() return #PlayerManager.GetAliveMajorIDs(); end, -1),
		-- Left behind by the setup context; see CivvisControlSetup.lua. Absent
		-- means this game was started some other way -- by a person clicking
		-- Play Now, say -- so its settings are the game's defaults and not the
		-- ones this run asked for.
		setup = try(function() return GameConfiguration.GetValue("CIVVIS_SETUP"); end)
			or "(absent)",
	});

	if cfg.SurveyEnums then
		for name, getter in pairs({
			UnitOperationTypes = function() return UnitOperationTypes; end,
			UnitCommandTypes = function() return UnitCommandTypes; end,
			CityOperationTypes = function() return CityOperationTypes; end,
			PlayerOperations = function() return PlayerOperations; end,
			EndTurnBlockingTypes = function() return EndTurnBlockingTypes; end,
		}) do
			local members = enumMembers(getter);
			emit("enum", { table = name, members = members or {},
			               available = members ~= nil });
		end
	end
end

-- ---------------------------------------------------------------- rehosting
--
-- The setup context is meant to configure a game from the main menu, but a
-- mod's FrontEnd context does not load on this build: the mod is discovered
-- and listed among the target mods, its InGame context runs, and its FrontEnd
-- one never does -- with nothing in Modding.log either way, because the mod
-- framework does not log UI actions at all.
--
-- So the configuration is applied from in here instead. A game started any
-- other way -- someone clicking Play Now, or the harness doing it -- carries
-- no CIVVIS_SETUP marker, and that is the signal to write this run's settings
-- into the configuration and host again. The second game carries the marker
-- and is played. One throwaway game per run buys exact control of difficulty,
-- map, size, speed and seed, which is the whole point of a ladder.

local function updatePlayerCounts()
	local def = try(function() return GameInfo.Maps[MapConfiguration.GetMapSize()]; end);
	if def == nil or def.DefaultPlayers == nil then return; end
	pcall(function()
		MapConfiguration.SetMaxMajorPlayers(def.DefaultPlayers);
		GameConfiguration.SetParticipatingPlayerCount(
			def.DefaultPlayers + GameConfiguration.GetHiddenPlayerCount());
	end);
end

-- Exactly one seat is ours; every other major is the shipped AI. Slot status
-- has to be settled before hosting: it is what decides whether the game waits
-- for orders or plays itself.
local function seatHuman(count)
	count = count or 1;
	local seated = 0;
	pcall(function()
		for _, id in ipairs(GameConfiguration.GetHumanPlayerIDs()) do
			PlayerConfigurations[id]:SetSlotStatus(SlotStatus.SS_COMPUTER);
		end
		local needed = count;
		for _, id in ipairs(GameConfiguration.GetAvailablePlayerIDs()) do
			if needed <= 0 then break; end
			PlayerConfigurations[id]:SetSlotStatus(SlotStatus.SS_TAKEN);
			needed = needed - 1;
		end
		if needed > 0 then
			for _, id in ipairs(GameConfiguration.GetAIPlayerIDs()) do
				if needed <= 0 then break; end
				local level = PlayerConfigurations[id]:GetCivilizationLevelTypeID();
				if level == CivilizationLevelTypes.CIVILIZATION_LEVEL_FULL_CIV then
					PlayerConfigurations[id]:SetSlotStatus(SlotStatus.SS_TAKEN);
					needed = needed - 1;
				end
			end
		end
		seated = count - needed;
	end);
	return seated;
end

-- This mod's own id, as written in CivvisControl.modinfo. Needed because
-- resetting the game configuration also clears the enabled-mod list, and this
-- mod is on it.
local MOD_ID = "4d2c8b16-7e05-49af-a3c1-6b90d5f2e841";

-- Put this mod back into the configuration after a reset. Skipping this
-- produced the most confusing failure of the lot: a correctly configured
-- Settler duel that started, drew its map, and then sat on turn 1 with a
-- settler asking for orders and not one line in any log -- because the only
-- thing that writes lines had been configured out of the game.
local function reenableSelf()
	return try(function()
		local handle = Modding.GetModHandle(MOD_ID);
		if handle == nil then return "no handle"; end
		Modding.EnableMod(handle, true);
		return "enabled";
	end, "unavailable");
end

local function applyConfiguration()
	-- Starting from defaults rather than from the bootstrap game's settings.
	-- Carrying them over leaves a six-player Small configuration half-rewritten
	-- into a two-player Duel one, and hosting that took the whole application
	-- down rather than reporting anything.
	local mods = nil;
	if cfg.SetToDefaults ~= false then
		GameConfiguration.SetToDefaults();
		GameConfiguration.SetValue("RULESET", nil);
		mods = reenableSelf();
	end

	if cfg.RuleSet then GameConfiguration.SetRuleSet(cfg.RuleSet); end
	if cfg.MapScript then MapConfiguration.SetScript(cfg.MapScript); updatePlayerCounts(); end
	if cfg.MapSize then MapConfiguration.SetMapSize(cfg.MapSize); updatePlayerCounts(); end
	local seated = seatHuman(cfg.HumanPlayers or 1);
	if cfg.Difficulty then GameConfiguration.SetHandicapType(cfg.Difficulty); end
	if cfg.GameSpeed then GameConfiguration.SetGameSpeedType(cfg.GameSpeed); end
	if cfg.StartEra then GameConfiguration.SetStartEra(cfg.StartEra); end
	if cfg.MapSeed then MapConfiguration.SetValue("RANDOM_SEED", cfg.MapSeed); end
	if cfg.GameSeed then GameConfiguration.SetValue("GAME_SYNC_RANDOM_SEED", cfg.GameSeed); end
	if cfg.MaxTurns and cfg.MaxTurns >= 1 then
		GameConfiguration.SetMaxTurns(cfg.MaxTurns);
		GameConfiguration.SetTurnLimitType(TurnLimitTypes.CUSTOM);
	end
	return seated, mods;
end

local function rehost()
	local ok, err, mods = pcall(applyConfiguration);
	-- Not `ok and nil or tostring(err)`. In Lua that idiom collapses: when ok
	-- is true the first branch is nil, which is falsy, so the expression falls
	-- through to the second and reports pcall's *return value* as an error. A
	-- successful configure logged `"error": "1"` that way -- a failure message
	-- on a run that worked, which is worse than no message.
	local failure = nil;
	if not ok then failure = tostring(err); mods = nil; end
	pcall(function()
		GameConfiguration.SetValue("CIVVIS_SETUP", ok and "ok" or failure);
	end);
	-- Read back rather than report what was asked for: a setter that silently
	-- refuses a value produces a game at the wrong difficulty, and the ladder
	-- this exists to climb is measured in difficulty.
	emit("rehost", {
		configured = ok,
		error = failure,
		mods = mods,
		difficulty = typeName(GameInfo.Difficulties,
			try(function() return GameConfiguration.GetHandicapType(); end)) or "?",
		size = typeName(GameInfo.Maps,
			try(function() return MapConfiguration.GetMapSize(); end)) or "?",
		speed = typeName(GameInfo.GameSpeeds,
			try(function() return GameConfiguration.GetGameSpeedType(); end)) or "?",
		max_turns = try(function() return GameConfiguration.GetMaxTurns(); end, -1),
		humans = try(function() return GameConfiguration.GetHumanPlayerCount(); end, -1),
	});
	-- Leaving first is what the shipped main menu does before it starts
	-- another session, and both calls are made from the same invocation so the
	-- host request is issued before this context can be torn down. It is a
	-- setting because it turned out to be wrong: leaving first and then hosting
	-- from an in-game context takes the whole application down -- the
	-- configuration is applied and logged correctly, and the next thing in the
	-- log is nothing, because the process is gone. Hosting without leaving is
	-- the default for that reason.
	local left = false;
	if cfg.LeaveBeforeHost == true then
		left = pcall(function() Network.LeaveGame(); end);
	end
	local hosted = pcall(function() Network.HostGame(ServerType.SERVER_TYPE_NONE); end);
	emit("rehost_issued", { left = left, hosted = hosted });
end

-- ------------------------------------------------------------ world reading

local function localPlayer()
	local pid = try(function() return Game.GetLocalPlayer(); end, -1);
	if pid == nil or pid < 0 then return nil, -1; end
	return Players[pid], pid;
end

local function unitTypeName(unit)
	return try(function()
		local row = GameInfo.Units[unit:GetUnitType()];
		return row and row.UnitType or "?";
	end, "?");
end

local function eachUnit(player, fn)
	pcall(function()
		for _, unit in player:GetUnits():Members() do fn(unit); end
	end);
end

local function eachCity(player, fn)
	pcall(function()
		for _, city in player:GetCities():Members() do fn(city); end
	end);
end

local function countUnits(player)
	local counts = { settler = 0, builder = 0, military = 0, scout = 0, total = 0 };
	eachUnit(player, function(unit)
		local name = unitTypeName(unit);
		local row = GameInfo.Units[name];
		counts.total = counts.total + 1;
		if name == "UNIT_SETTLER" then
			counts.settler = counts.settler + 1;
		elseif name == "UNIT_BUILDER" then
			counts.builder = counts.builder + 1;
		elseif name == "UNIT_SCOUT" then
			counts.scout = counts.scout + 1;
		elseif row ~= nil and (row.Combat or 0) > 0 then
		-- Cheaper than losing the unit and rebuilding it a tier late.
		local better = upgradeUnit(unit);
		if better then return better; end
			counts.military = counts.military + 1;
		end
	end);
	return counts;
end

local function cityCount(player)
	local n = 0;
	eachCity(player, function() n = n + 1; end);
	return n;
end

-- ------------------------------------------------------------ unit handling

-- ⚠ PASS THE PARAMETERS. `CanStartOperation(unit, hash, nil, false, false)`
-- asks "can this unit move", which is not the question — the question is "can
-- this unit move TO (x, y)". `WorldInput.lua` passes the same parameter table it
-- is about to request with:
--
--   UnitManager.CanStartOperation( pUnit, OP, nil, tParameters )
--   UnitManager.RequestOperation ( pUnit, OP, tParameters )
--
-- Without it a rejected order is indistinguishable from an accepted one, and
-- that is how 518 `advance` orders were logged while the army stood still in
-- its own city: `pcall` succeeded every time because the call did not throw,
-- and the engine quietly declined to move anything.
local function canOperate(unit, hash, params)
	if hash == nil then return false; end
	local ok, result = pcall(function()
		return UnitManager.CanStartOperation(unit, hash, nil, params or {});
	end);
	return ok and result == true;
end

-- Never report an order as given unless the engine said it could start.
--
-- `pcall` returning true means the call did not raise, NOT that the operation
-- was accepted — the identical trap that made every production order look
-- applied while nothing was ever built. Checking first is what turns a silent
-- no-op into an observable refusal, so a fallback can actually run.
local function operate(unit, hash, params)
	if hash == nil then return false; end
	params = params or {};
	if not canOperate(unit, hash, params) then return false; end
	return pcall(function()
		UnitManager.RequestOperation(unit, hash, params);
	end);
end

-- Same discipline as `operate`: ask whether the command can start before
-- claiming it was given. `pcall` only reports that the call did not raise.
local function commandUnit(unit, hash, params)
	if hash == nil then return false; end
	params = params or {};
	local ok, can = pcall(function()
		return UnitManager.CanStartCommand(unit, hash, nil, params);
	end);
	if not (ok and can == true) then return false; end
	return pcall(function()
		UnitManager.RequestCommand(unit, hash, params);
	end);
end

-- Spend gold to bring a unit up to date before spending its life.
--
-- ⚠ The agent fielded WARRIORs and SPEARMEN in 1100 AD against swordsmen and
-- archers — military strength 78 against 357, a 4.5:1 deficit — and the combat
-- log read as a rout. The army ladder builds ancient units and nothing ever
-- upgraded them, while the treasury sat on 478 unspent Gold. UNITCOMMAND_UPGRADE
-- is in this build's resolved command list, so this is available and was simply
-- never attempted.
local function upgradeUnit(unit)
	if commandUnit(unit, CMD["UNITCOMMAND_UPGRADE"]) then return "upgrade"; end
	return nil;
end

-- Try each order in turn and take the first the engine accepts. Asking "can
-- you start this" rather than reasoning about terrain, charges and movement
-- keeps the controller honest: a rule this code does not model refuses the
-- order, and the next one down is tried instead.
local function firstOperation(unit, names)
	for _, name in ipairs(names) do
		local hash = OP[name];
		if operate(unit, hash) then
			return name;
		end
	end
	return nil;
end

local function plotDistance(x1, y1, x2, y2)
	return try(function() return Map.GetPlotDistance(x1, y1, x2, y2); end, 99);
end

-- Whether this unit can actually walk there.
--
-- Ordering a move to a plot with no route does not fail: the engine accepts it
-- and then prints `Distance: 2147483647` -- its no-path sentinel -- once per
-- attempt, forever. A settler aiming across water and an army aiming at a
-- capital on another continent both do this, and a run that did it for twenty
-- turns ended with the game gone and nothing in the log but that line.
local function reachable(unit, x, y)
	local path = try(function()
		local index = Map.GetPlotIndex(x, y);
		return UnitManager.GetMoveToPathEx(unit, index);
	end);
	if path == nil or path.plots == nil then return false; end
	local n = 0;
	for _ in pairs(path.plots) do n = n + 1; end
	return n > 1;
end

-- Where this settler should walk to found a city.
--
-- Cities have to be a few tiles apart, so after the capital every settler is
-- standing somewhere it cannot found. The first version handled that by
-- falling back to explore automation, which is why a game reached turn 50 with
-- six units, one city, and two hundred settler orders: the settlers wandered,
-- and wandering never becomes a city.
--
-- The search is deliberately plain -- land, passable, unowned or ours, far
-- enough from every city we have, nearest such plot wins. Site *quality* is a
-- real lever and a separate one; site *existence* is what was missing.
-- Sites found this turn, keyed by unit. The search below reads a 15x15 block
-- of plots through guarded calls, and the order pass runs on every batch of
-- game-core events, so recomputing it per tick is what took a turn from thirty
-- seconds to four minutes -- the controller was starving the game it was
-- playing.
local siteMemo = { turn = -1, sites = {} };
-- Where each settler decided to go, kept across turns. See findSettleSite.
local committedSite = {};
-- Sites the engine refused to move a given settler to.
--
-- ⚠ Only visible once `operate` started checking CanStartOperation WITH the
-- destination: before that a refused move was logged as a successful
-- `move_to_site`, 166 of them in one run. With refusals honest, the real ratio
-- was 4 moves to 15 SKIP_TURNs — the chosen site simply could not be reached,
-- and re-offering it every turn was the whole failure.
local refusedSite = {};
-- Which city each unit is the garrison of, kept across turns.
--
-- ⚠ Without this the assignment oscillates. thinnestCity counts defenders, so
-- with two units and two cities a warrior sent to the second city leaves the
-- capital on zero, is ordered home next turn, which empties the second city
-- again. It shuttles forever and looks exactly like two warriors parked in the
-- capital — which is what was observed just before turn 50 of run
-- settler-20260730T013057Z. A garrison is only a garrison if it stays.
local garrisonOf = {};
local findSettleSite;

findSettleSite = function(player, pid, unit, turn)
	local id = try(function() return unit:GetID(); end, -1);
	if siteMemo.turn ~= turn then siteMemo = { turn = turn, sites = {} }; end
	local cached = siteMemo.sites[id];
	if cached ~= nil then
		if cached == false then return nil; end
		return cached;
	end
	-- ⚠ COMMIT TO A SITE ACROSS TURNS, not just within one.
	--
	-- The score charges distance, so as a settler walks the ranking shifts
	-- underneath it and two comparable sites can trade places every turn. The
	-- settler then re-targets, walks back, re-targets again, and never arrives:
	-- run settler-20260730T004226Z logged `move_to_site` three times on turn 40
	-- while the empire was still on two cities. A destination is only worth
	-- having if it survives the walk, so a committed site is kept until it stops
	-- being legal or the settler stands on it.
	local held = committedSite[id];
	if held ~= nil then
		local plot = try(function() return Map.GetPlot(held.x, held.y); end);
		local ux0 = try(function() return unit:GetX(); end, -1);
		local uy0 = try(function() return unit:GetY(); end, -1);
		local arrived = (ux0 == held.x and uy0 == held.y);
		local ok = plot ~= nil and try(function()
			local owner = plot:GetOwner();
			return (not plot:IsWater()) and (not plot:IsImpassable())
				and (owner == -1 or owner == pid);
		end, false);
		if ok and not arrived then
			siteMemo.sites[id] = plot;
			return plot;
		end
		committedSite[id] = nil;
	end

	local ux = try(function() return unit:GetX(); end, -1);
	local uy = try(function() return unit:GetY(); end, -1);
	if ux < 0 then siteMemo.sites[id] = false; return nil; end

	local cities = {};
	eachCity(player, function(city)
		cities[#cities + 1] = { try(function() return city:GetX(); end, -1),
		                        try(function() return city:GetY(); end, -1) };
	end);

	local spacing = cfg.MinCitySpacing or 4;
	local radius = cfg.SettleSearchRadius or 7;
	local best, bestScore;
	for dx = -radius, radius do
		for dy = -radius, radius do
			local plot = try(function() return Map.GetPlot(ux + dx, uy + dy); end);
			if plot ~= nil then
				local usable = try(function()
					return (not plot:IsWater()) and (not plot:IsImpassable());
				end, false);
				local owner = try(function() return plot:GetOwner(); end, -1);
				local blocked = refusedSite[id] ~= nil
					and plot ~= nil
					and refusedSite[id][plot:GetX() .. ":" .. plot:GetY()];
				if usable and not blocked and (owner == -1 or owner == pid) then
					local px = plot:GetX();
					local py = plot:GetY();
					local nearest = 99;
					for _, city in ipairs(cities) do
						local d = plotDistance(px, py, city[1], city[2]);
						if d < nearest then nearest = d; end
					end
					if nearest >= spacing then
						-- Score the GROUND, not just the geometry.
						--
						-- This used to be `-distance + nearest`, which never
						-- looked at a single yield: any legal tile at the right
						-- spacing scored the same as a river-grassland start.
						-- CIVVIS measures settle siting as worth 99.9% of the
						-- value on offer when it is scored on yields, so the
						-- shape below is the simulator's: the workable ring
						-- valued with food ahead of production ahead of gold,
						-- a real premium on fresh water, a smaller one for the
						-- coast, and distance charged as the turns it costs to
						-- walk there rather than as a tiebreak.
						local food, prod, gold = 0, 0, 0;
						for rx = -2, 2 do
							for ry = -2, 2 do
								local ring = try(function()
									return Map.GetPlot(px + rx, py + ry);
								end);
								if ring ~= nil
										and plotDistance(px, py, ring:GetX(), ring:GetY()) <= 2 then
									food = food + (try(function()
										return ring:GetYield(YieldTypes.YIELD_FOOD);
									end, 0) or 0);
									prod = prod + (try(function()
										return ring:GetYield(YieldTypes.YIELD_PRODUCTION);
									end, 0) or 0);
									gold = gold + (try(function()
										return ring:GetYield(YieldTypes.YIELD_GOLD);
									end, 0) or 0);
								end
							end
						end
						local fresh = try(function()
							return plot:IsFreshWater();
						end, false) and (cfg.FreshWaterValue or 12) or 0;
						local coast = try(function()
							return plot:IsCoastalLand();
						end, false) and (cfg.CoastValue or 4) or 0;
						-- A settler covers about a tile a turn in practice, so
						-- each step of distance is a turn the city does not
						-- exist. CIVVIS measured that walk at 16.8 turns per
						-- city and the biggest single win on its whole
						-- expansion axis was removing it.
						local walk = plotDistance(ux, uy, px, py)
							* (cfg.WalkTurnCost or 2.0);
						local score = (food * (cfg.FoodWeight or 2.0))
							+ (prod * (cfg.ProductionWeight or 1.5))
							+ (gold * (cfg.GoldWeight or 0.5))
							+ fresh + coast - walk;
						if bestScore == nil or score > bestScore then
							best, bestScore = plot, score;
						end
					end
				end
			end
		end
	end
	siteMemo.sites[id] = best or false;
	if best ~= nil then
		committedSite[id] = { x = best:GetX(), y = best:GetY() };
	end
	return best;
end

local function orderSettler(player, pid, unit, turn)
	if canOperate(unit, OP["UNITOPERATION_FOUND_CITY"])
			and operate(unit, OP["UNITOPERATION_FOUND_CITY"]) then
		return "found_city";
	end
	local plot = findSettleSite(player, pid, unit, turn);
	if plot ~= nil then
		local px, py = plot:GetX(), plot:GetY();
		-- ⚠ NO `reachable` GATE. It counts the plots in the path and demands
		-- more than one, which is false for a site the settler is already
		-- standing next to — so a settler one tile from a good site could
		-- neither found nor move, fell through to AUTOMATE_EXPLORE (which a
		-- settler cannot do), and ended on SKIP_TURN. Measured at turn 20 of
		-- run settler-20260730T001117Z: `UNIT_SETTLER:UNITOPERATION_SKIP_TURN=2`
		-- with the empire still on one city. MOVE_TO fails harmlessly when the
		-- site truly cannot be reached, so trying it is strictly safer.
		local params = {};
		params[UnitOperationTypes.PARAM_X] = px;
		params[UnitOperationTypes.PARAM_Y] = py;
		if operate(unit, OP["UNITOPERATION_MOVE_TO"], params) then
			return "move_to_site";
		end
		-- The engine will not path this settler there. Blacklist that ground
		-- for this unit, drop the commitment, and let the search pick again
		-- rather than re-offering a site it has already declined.
		refusedSite[id] = refusedSite[id] or {};
		refusedSite[id][px .. ":" .. py] = true;
		committedSite[id] = nil;
		siteMemo.sites[id] = nil;
		local retry = findSettleSite(player, pid, unit, turn);
		if retry ~= nil then
			local rx, ry = retry:GetX(), retry:GetY();
			local again = {};
			again[UnitOperationTypes.PARAM_X] = rx;
			again[UnitOperationTypes.PARAM_Y] = ry;
			if operate(unit, OP["UNITOPERATION_MOVE_TO"], again) then
				return "move_to_site";
			end
		end
	end
	-- A settler cannot explore, so this is nearly always nil; it is left as the
	-- last resort rather than removed because a settler with nowhere to go is
	-- better wandering than blocking the tile it stands on.
	return firstOperation(unit, { "UNITOPERATION_AUTOMATE_EXPLORE" });
end

local function orderBuilder(unit)
	-- Builder automation is the engine's own improvement chooser: it picks the
	-- tile and the improvement with the logic the shipped AI uses, which beats
	-- a hand-rolled ranking and costs nothing to keep current.
	if commandUnit(unit, CMD["UNITCOMMAND_AUTOMATE"]) then return "automate"; end
	return firstOperation(unit, { "UNITOPERATION_BUILD_IMPROVEMENT" });
end

-- Which of our cities has the fewest defenders standing on or beside it.
--
-- ⚠ Without this every idle unit fortifies wherever it was built, which is the
-- capital, and the empire ends up with a pile of units on one hill and outlying
-- cities naked. Observed directly: ten units alive at turn 75 with the only
-- orders being FORTIFY and ALERT, all at home. Spreading them is free — they
-- were going to stand still anyway — and it is what stops a border city being
-- lost to the first raider that wanders past.
local function thinnestCity(player, unit)
	local best, bestCount;
	local ux = try(function() return unit:GetX(); end, -1);
	local uy = try(function() return unit:GetY(); end, -1);
	eachCity(player, function(city)
		local cx = try(function() return city:GetX(); end, -1);
		local cy = try(function() return city:GetY(); end, -1);
		if cx < 0 then return; end
		local near = 0;
		eachUnit(player, function(other)
			local row = GameInfo.Units[unitTypeName(other)];
			if row ~= nil and (row.Combat or 0) > 0 then
				local ox = try(function() return other:GetX(); end, -1);
				local oy = try(function() return other:GetY(); end, -1);
				if ox >= 0 and plotDistance(ox, oy, cx, cy) <= 1 then
					near = near + 1;
				end
			end
		end);
		-- Prefer the thinnest city, and among equals the nearest one, so units
		-- do not cross the empire past a city that needed them.
		local worse = bestCount == nil or near < bestCount
			or (near == bestCount and best ~= nil
				and plotDistance(ux, uy, cx, cy) < plotDistance(ux, uy, best.x, best.y));
		if worse then best, bestCount = { x = cx, y = cy }, near; end
	end);
	return best, bestCount;
end

local function orderMilitary(unit, stillExploring, player)
	if stillExploring then
		local acted = firstOperation(unit, { "UNITOPERATION_AUTOMATE_EXPLORE" });
		if acted then return acted; end
	end
	-- Heal first: a damaged garrison is not a garrison.
	local damage = try(function() return unit:GetDamage(); end, 0) or 0;
	if damage > 0 then
		local healed = firstOperation(unit, { "UNITOPERATION_HEAL" });
		if healed then return healed; end
	end
	if player ~= nil then
		local id = try(function() return unit:GetID(); end, -1);
		local ux = try(function() return unit:GetX(); end, -1);
		local uy = try(function() return unit:GetY(); end, -1);
		-- Keep a standing assignment. Only pick a new one if this unit has none
		-- or the city it was posted to has stopped being ours.
		local post = garrisonOf[id];
		if post ~= nil then
			local mine = false;
			eachCity(player, function(city)
				local cx = try(function() return city:GetX(); end, -1);
				local cy = try(function() return city:GetY(); end, -1);
				if cx == post.x and cy == post.y then mine = true; end
			end);
			if not mine then garrisonOf[id] = nil; post = nil; end
		end
		if post == nil then
			local city, count = thinnestCity(player, unit);
			if city ~= nil and (count or 0) < (cfg.GarrisonPerCity or 2) then
				garrisonOf[id] = { x = city.x, y = city.y };
				post = garrisonOf[id];
			end
		end
		if post ~= nil and plotDistance(ux, uy, post.x, post.y) > 1 then
			local params = {};
			params[UnitOperationTypes.PARAM_X] = post.x;
			params[UnitOperationTypes.PARAM_Y] = post.y;
			if operate(unit, OP["UNITOPERATION_MOVE_TO"], params) then
				return "garrison";
			end
		end
	end
	return firstOperation(unit, {
		"UNITOPERATION_FORTIFY", "UNITOPERATION_ALERT",
	});
end

-- The order of last resort. A unit that ends the turn with no orders is an
-- end-turn blocker, so something has to stick or the game never advances.
local function orderIdle(unit)
	return firstOperation(unit, {
		"UNITOPERATION_SKIP_TURN", "UNITOPERATION_FORTIFY",
		"UNITOPERATION_ALERT", "UNITOPERATION_SLEEP",
	});
end

-- --------------------------------------------------------------------- war
--
-- A duel on the lowest difficulty ends fastest by conquest: two players, one
-- enemy capital, and taking it is a Domination victory outright. Score at the
-- turn limit is the alternative and it is not available -- the simple setup
-- screen has no max-turns control, so an unattended game otherwise runs to
-- turn five hundred.
--
-- Only civilizations this player has actually met are considered. Reading the
-- position of a capital we have never seen would win games the controller did
-- not earn, and a ladder built on that measures nothing.

local warTarget = nil;
local warDeclared = {};

-- How many units have already been aimed at the target plot this turn, and
-- which approach tiles are taken.
--
-- ⚠ Civilization VI allows ONE military unit per tile. Ordering the whole army
-- at a single city plot therefore builds a traffic jam, not an assault: the
-- front unit blocks the rest and the column backs up along the border. Observed
-- directly on turn 93 of run settler-20260730T004826Z — a line of units strung
-- along a river west of Ulundi, every one with full movement, none attacking,
-- 518 advances logged and no capture. Only a couple of units can usefully be on
-- the objective; the rest need their own ground to stand on.
local assault = { turn = -1, onTarget = 0, taken = {} };

local function findWarTarget(player, pid)
	-- Reachability is checked per unit at order time, not here: different
	-- units have different routes, and a path query per candidate city per
	-- turn is exactly the kind of cost that starves the game.
	local diplomacy = try(function() return player:GetDiplomacy(); end);
	if diplomacy == nil then return nil; end
	local home = try(function() return player:GetCities():GetCapitalCity(); end);
	local hx = home and try(function() return home:GetX(); end, 0) or 0;
	local hy = home and try(function() return home:GetY(); end, 0) or 0;

	local best, bestScore;
	for _, otherId in ipairs(try(function() return PlayerManager.GetAliveMajorIDs(); end, {})) do
		if otherId ~= pid and try(function() return diplomacy:HasMet(otherId); end, false) then
			local other = Players[otherId];
			pcall(function()
				for _, city in other:GetCities():Members() do
					local cx = city:GetX();
					local cy = city:GetY();
					-- A capital is worth walking further for: taking every
					-- original capital is what actually ends the game.
					local capital = try(function() return city:IsCapital(); end, false);
					local score = -plotDistance(hx, hy, cx, cy) + (capital and 12 or 0);
					if bestScore == nil or score > bestScore then
						best = { player = otherId, x = cx, y = cy, capital = capital };
						bestScore = score;
					end
				end
			end);
		end
	end
	return best;
end

local function declareWar(player, pid, counts, turn)
	if cfg.MakeWar == false then return nil; end
	if turn < (cfg.WarFromTurn or 25) then return nil; end
	if counts.military < (cfg.WarArmy or 4) then return nil; end
	local target = warTarget;
	if target == nil then return nil; end
	local diplomacy = try(function() return player:GetDiplomacy(); end);
	if diplomacy == nil then return nil; end
	if try(function() return diplomacy:IsAtWarWith(target.player); end, false) then
		warDeclared[target.player] = true;
		return nil;
	end
	if warDeclared[target.player] then return nil; end
	if not try(function() return diplomacy:CanDeclareWarOn(target.player); end, true) then
		return nil;
	end
	local params = {};
	params[PlayerOperations.PARAM_PLAYER_ONE] = pid;
	params[PlayerOperations.PARAM_PLAYER_TWO] = target.player;
	local ok = pcall(function()
		UI.RequestPlayerOperation(pid, PlayerOperations.DIPLOMACY_DECLARE_WAR, params);
	end);
	if ok then
		warDeclared[target.player] = true;
		emit("war", { turn = turn, target = target.player, x = target.x, y = target.y,
		              capital = target.capital, army = counts.military });
	end
	return ok and "war" or nil;
end

local function atWar(player)
	local diplomacy = try(function() return player:GetDiplomacy(); end);
	if diplomacy == nil or warTarget == nil then return false; end
	return try(function() return diplomacy:IsAtWarWith(warTarget.player); end, false);
end

-- Walk at the target and let the move become an attack. Civilization VI
-- resolves a melee unit ordered onto an occupied enemy plot as an attack, so
-- "advance" and "attack" are the same order; ranged units get their own.
-- A free tile next to the target that nobody has been sent to yet.
local function approachTile(unit)
	local best, bestDist;
	local ux = try(function() return unit:GetX(); end, -1);
	local uy = try(function() return unit:GetY(); end, -1);
	for dx = -1, 1 do
		for dy = -1, 1 do
			local x, y = warTarget.x + dx, warTarget.y + dy;
			local key = x .. ":" .. y;
			if not (dx == 0 and dy == 0) and not assault.taken[key] then
				local plot = try(function() return Map.GetPlot(x, y); end);
				local usable = plot ~= nil and try(function()
					return (not plot:IsWater()) and (not plot:IsImpassable());
				end, false);
				if usable then
					local d = plotDistance(ux, uy, x, y);
					if bestDist == nil or d < bestDist then
						best, bestDist = { x = x, y = y, key = key }, d;
					end
				end
			end
		end
	end
	return best;
end

local function pressAttack(unit, turn)
	if warTarget == nil then return nil; end
	if assault.turn ~= turn then
		assault = { turn = turn, onTarget = 0, taken = {} };
	end
	local params = {};
	params[UnitOperationTypes.PARAM_X] = warTarget.x;
	params[UnitOperationTypes.PARAM_Y] = warTarget.y;
	if canOperate(unit, OP["UNITOPERATION_RANGE_ATTACK"])
			and operate(unit, OP["UNITOPERATION_RANGE_ATTACK"], params) then
		return "range_attack";
	end
	-- A ranged unit that cannot shoot the city yet should close the distance,
	-- but it must never be the thing standing on the objective: it cannot
	-- capture, and a ranged unit parked on the approach blocks the melee that
	-- can. Fortifying in place beside the target is more useful than shuffling.
	local row = GameInfo.Units[unitTypeName(unit)];
	local ranged = row ~= nil and (row.RangedCombat or 0) > 0;
	if ranged and plotDistance(try(function() return unit:GetX(); end, 0),
			try(function() return unit:GetY(); end, 0),
			warTarget.x, warTarget.y) <= 2 then
		local held = firstOperation(unit, { "UNITOPERATION_FORTIFY",
		                                   "UNITOPERATION_ALERT" });
		if held then return held; end
	end
	-- ⚠ NO `reachable` GATE ON THE ATTACK MOVE.
	--
	-- In Civilization VI a melee unit captures a city by moving ONTO it, so
	-- this MOVE_TO is the capture, not merely an approach. `reachable` counts
	-- the plots in the path and demands more than one — which is exactly false
	-- for a unit already standing next to the city. A unit that had walked the
	-- whole way to the enemy capital could therefore never strike it, and would
	-- fall through to FORTIFY and sit there. That is the "units stack up and
	-- never take it" shape.
	--
	-- MOVE_TO fails harmlessly when the target genuinely cannot be pathed to,
	-- so attempting it unconditionally costs nothing and is strictly safer than
	-- a gate that can be wrong in the one position that matters.
	-- Only the first couple of units are aimed at the city itself. In this
	-- build MOVE_TO onto the plot *is* the capture, so those are the ones that
	-- can take it; everybody else gets their own approach tile and waits their
	-- turn rather than forming a column that blocks the assault.
	if assault.onTarget < (cfg.AssaultWidth or 2) then
		if operate(unit, OP["UNITOPERATION_MOVE_TO"], params) then
			assault.onTarget = assault.onTarget + 1;
			return "advance";
		end
	end
	local spot = approachTile(unit);
	if spot ~= nil then
		local near = {};
		near[UnitOperationTypes.PARAM_X] = spot.x;
		near[UnitOperationTypes.PARAM_Y] = spot.y;
		if operate(unit, OP["UNITOPERATION_MOVE_TO"], near) then
			assault.taken[spot.key] = true;
			return "surround";
		end
	end
	-- Nowhere useful to stand. Holding position beats shuffling into the queue.
	return firstOperation(unit, { "UNITOPERATION_FORTIFY", "UNITOPERATION_ALERT" });
end

local function orderFor(player, pid, unit, turn)
	local name = unitTypeName(unit);
	local row = GameInfo.Units[name];
	if name == "UNIT_SETTLER" then
		return orderSettler(player, pid, unit, turn);
	elseif name == "UNIT_BUILDER" then
		return orderBuilder(unit);
	elseif name == "UNIT_SCOUT" then
		return firstOperation(unit, { "UNITOPERATION_AUTOMATE_EXPLORE" });
	elseif row ~= nil and (row.Combat or 0) > 0 then
		-- Damaged units heal first: sending a hurt unit at a city loses it and
		-- the war with it.
		local damage = try(function() return unit:GetDamage(); end, 0) or 0;
		if damage > (cfg.HealBelow or 40) then
			local healed = firstOperation(unit, { "UNITOPERATION_HEAL",
			                                      "UNITOPERATION_REST_REPAIR",
			                                      "UNITOPERATION_FORTIFY" });
			if healed then return healed; end
		end
		if atWar(player) then
			local pressed = pressAttack(unit, turn);
			if pressed then return pressed; end
		end
		-- Exploring is how the army evaporates: units 6 -> 4 -> 3 across turns
		-- 20/40/100 of run settler-20260729T235910Z, wandering into barbarians
		-- while the war threshold was never reached. Default is now early.
		return orderMilitary(unit, turn < (cfg.ExploreUntilTurn or 12), player);
	end
	return nil;
end

-- Work the game's own ready list rather than every unit the player owns.
--
-- Iterating all units and re-issuing an order to each looks equivalent and is
-- not: a unit that was told to explore last turn is still owned, still has
-- movement, and being told to explore again *restarts* it, so the ready list
-- never empties and the same turn is ordered forever. GetFirstReadyUnit is the
-- same query the shipped interface uses to decide whether units are holding up
-- the turn, so working it to exhaustion is exactly the human loop.
-- What the last order pass actually did, by action name. Counting orders is
-- not enough: "seven units ordered" was true on every turn of a game that
-- reached turn fifty with one city, because every settler was being told to
-- explore. The breakdown is what says which.
local lastActions = {};

local function orderUnits(player, pid, turn)
	local given, stuck = 0, 0;
	lastActions = {};
	local ordered, attempts = {}, {};

	local function give(unit, id)
		local action = orderFor(player, pid, unit, turn) or orderIdle(unit);
		if action == nil then return false; end
		ordered[id] = true;
		local key = unitTypeName(unit) .. ":" .. action;
		lastActions[key] = (lastActions[key] or 0) + 1;
		given = given + 1;
		return true;
	end

	-- Pass 1: the game's own ready list. Cheap, and the same query the shipped
	-- interface uses to decide whether units are holding up the turn.
	for _ = 1, (cfg.MaxUnitOrders or 40) do
		local unit = try(function() return player:GetUnits():GetFirstReadyUnit(); end);
		if unit == nil then break; end
		local id = try(function() return unit:GetID(); end, -1);
		attempts[id] = (attempts[id] or 0) + 1;
		-- Jammed on one unit. Leave it to pass 2 rather than surrender the
		-- pass: GetFirstReadyUnit keeps returning this same unit forever.
		if attempts[id] > 2 then break; end
		give(unit, id);
	end

	-- ⚠ PASS 2 IS THE WHOLE POINT. Pass 1 alone leaves every other unit
	-- standing still the moment one unit will not clear, because
	-- GetFirstReadyUnit only ever offers the first ready unit and a unit that
	-- cannot be cleared is offered forever. Measured across three runs: turn
	-- 90 with NINETEEN units and exactly three orders given, all of them
	-- `UNIT_BUILDER:automate`, and one city. Parking the jammed unit did not
	-- help — the query still returned it. The only way past a blocked query is
	-- to stop using it, so this walks the roster directly for anything pass 1
	-- never reached.
	eachUnit(player, function(unit)
		local id = try(function() return unit:GetID(); end, -1);
		if ordered[id] then return; end
		-- No movement gate. `GetMovesRemaining` is not used anywhere else in
		-- this mod and there is no evidence it exists on this build; `try`
		-- returned its 0 default for every unit, so the gate silently skipped
		-- the entire roster and turns 12 and 13 recorded `actions: []` with
		-- five units alive. An order that cannot be given fails harmlessly, so
		-- attempting it is strictly safer than guarding it.
		if not give(unit, id) then stuck = stuck + 1; end
	end);
	return given, stuck;
end

-- --------------------------------------------------------- city production

local function chooseProduction(city, counts, nCities, turn, refused)
	refused = refused or {};
	local function playable(name)
		-- Already asked for this one on this turn and the game did not start
		-- it. `CanProduce` will keep saying yes, so the caller's record of what
		-- was refused is the only thing that makes the ladder fall through.
		if refused[name] then return nil; end
		local row = GameInfo.Types[name];
		if row == nil then return nil; end
		local ok, can = pcall(function()
			return city:GetBuildQueue():CanProduce(row.Hash, true);
		end);
		if ok and can == true then return row; end
		return nil;
	end

	local ladder = {};
	-- ⚠ ONE SETTLER IN FLIGHT AT A TIME.
	--
	-- The condition used to be `(nCities + counts.settler) < CityTarget` with no
	-- cap on how many settlers could be walking at once. With one city and two
	-- settlers alive that is 3 < 6, so it built another settler — and another,
	-- and another. Run settler-20260730T010409Z reached turn 60 having ordered
	-- SEVENTEEN settlers and NOTHING else: no monument, no granary, no district.
	-- 166 move_to_site orders produced 2 cities, because a Tiny map shared with
	-- three rivals has almost no legal ground at four-tile spacing, so the
	-- surplus settlers just walked.
	--
	-- Score comes from cities, population, districts and buildings. A capital
	-- that only ever builds settlers scores nothing and expands barely, which is
	-- the worst of both. One at a time is also what the engine's own AI does.
	if (nCities + counts.settler) < (cfg.CityTarget or 6)
			and counts.settler < (cfg.SettlersInFlight or 1)
			and turn < (cfg.SettlerStopTurn or 9999) then
		ladder[#ladder + 1] = { "UNIT_SETTLER", "expand" };
	end
	if counts.scout < 1 and turn < 30 then
		ladder[#ladder + 1] = { "UNIT_SCOUT", "scout" };
	end
	-- An army large enough to take a city, not merely to garrison one. The
	-- floor rises as the war turn approaches, because a declaration made with
	-- two warriors is a declaration that loses.
	local wantArmy = math.max(2, nCities * (cfg.MilitaryPerCity or 1.5));
	if turn >= (cfg.WarFromTurn or 25) - 10 then
		wantArmy = math.max(wantArmy, (cfg.WarArmy or 4) + 2);
	end
	if counts.military < wantArmy then
		-- ⚠ MELEE FIRST. A ranged unit can bombard a city forever and never
		-- take it — only a melee unit captures, by moving onto the plot. Run
		-- settler-20260730T004226Z declared war on a capital at turn 65 and by
		-- turn 107 had logged 518 archer advances and 31 range attacks without
		-- a single capture, because the army it had built could not capture
		-- anything. Swordsman needs Iron and is often unavailable, so the
		-- melee that is always buildable comes before the ranged that is
		-- always tempting.
		for _, name in ipairs({ "UNIT_SWORDSMAN", "UNIT_SPEARMAN", "UNIT_WARRIOR",
		                        "UNIT_ARCHER", "UNIT_SLINGER" }) do
			ladder[#ladder + 1] = { name, "army" };
		end
	end
	if counts.builder < math.max(1, nCities * (cfg.BuilderPerCity or 0.8)) then
		ladder[#ladder + 1] = { "UNIT_BUILDER", "improve" };
	end
	for _, name in ipairs({ "BUILDING_MONUMENT", "BUILDING_GRANARY",
	                        "DISTRICT_CAMPUS", "BUILDING_LIBRARY",
	                        "DISTRICT_HOLY_SITE", "DISTRICT_COMMERCIAL_HUB",
	                        "BUILDING_WATER_MILL", "DISTRICT_THEATER",
	                        "BUILDING_ANCIENT_WALLS" }) do
		ladder[#ladder + 1] = { name, "develop" };
	end
	-- Always-available floor. A city with an empty queue and nothing it can
	-- build is a permanent end-turn blocker; a project never is.
	for _, name in ipairs({ "PROJECT_CAMPUS_RESEARCH_GRANT", "UNIT_WARRIOR",
	                        "UNIT_BUILDER", "UNIT_SLINGER" }) do
		ladder[#ladder + 1] = { name, "floor" };
	end

	for _, entry in ipairs(ladder) do
		local row = playable(entry[1]);
		if row ~= nil then return entry[1], row, entry[2]; end
	end
	return nil, nil, nil;
end

local function buildParams(row)
	local params = {};
	-- ⚠ WITHOUT AN INSERT MODE THE GAME REJECTS EVERY BUILD.
	--
	-- `ProductionPanel.lua`'s own `GetBuildInsertMode` sets these two on every
	-- request it makes, and with the queue panel closed — which is always, under
	-- program control — it sends REPLACE_AT at destination 0. We sent neither,
	-- so `RequestOperation` returned without throwing and did nothing.
	--
	-- That is the whole reason no run ever built anything. Run
	-- settler-20260729T221605Z asked for a Settler on 83 consecutive turns,
	-- every one `applied = true`, and finished with one city and zero units;
	-- once refusal detection was added, the ladder simply refused every item in
	-- turn and the city still built nothing. The order was never malformed in a
	-- way Lua could see — it was missing a parameter the engine requires.
	params[CityOperationTypes.PARAM_INSERT_MODE] =
		CityOperationTypes.VALUE_REPLACE_AT;
	params[CityOperationTypes.PARAM_QUEUE_DESTINATION_LOCATION] = 0;
	if row.Kind == "KIND_UNIT" then
		params[CityOperationTypes.PARAM_UNIT_TYPE] = row.Hash;
	elseif row.Kind == "KIND_BUILDING" then
		params[CityOperationTypes.PARAM_BUILDING_TYPE] = row.Hash;
	elseif row.Kind == "KIND_DISTRICT" then
		params[CityOperationTypes.PARAM_DISTRICT_TYPE] = row.Hash;
	elseif row.Kind == "KIND_PROJECT" then
		params[CityOperationTypes.PARAM_PROJECT_TYPE] = row.Hash;
	else
		return nil;
	end
	return params;
end

-- What each city was last told to build, and on which turn. Re-sending the
-- same order every tick is how one game logged two hundred settler requests
-- in fifty turns: the queue read comes back empty for a few frames after a
-- request, so the "queue is empty" test fires again and again.
local lastBuild = {};

-- Items a given city asked for and never started, remembered across turns.
--
-- ⚠ The queue does NOT reflect a BUILD request in the same tick it is made, so
-- a synchronous "did it start?" check reads false for everything and is worse
-- than no check: it made the ladder re-order all six candidates every turn.
-- The honest place to notice is the FOLLOWING turn — if the queue is still
-- empty then, the order never took, and that item is refused for this city from
-- now on so the ladder can fall through to something it can actually build.
local refusedByCity = {};

local function driveProduction(player, turn, force)
	local counts = countUnits(player);
	local cities = {};
	eachCity(player, function(city) cities[#cities + 1] = city; end);
	local issued = 0;
	for _, city in ipairs(cities) do
		-- Only fill an empty queue. Replacing the build every tick is how a
		-- controller spends a whole game producing nothing: each switch is
		-- legal and banks its progress, but the city never finishes anything.
		local current = try(function()
			local queue = city:GetBuildQueue();
			return queue and queue:GetCurrentProductionTypeHash() or 0;
		end, 0);
		local cityId = try(function() return city:GetID(); end, -1);
		local remembered = lastBuild[cityId];
		-- The memo stops per-tick spam, but when the game says it is *blocked*
		-- on production the whole point is to try again: an order that was
		-- refused once must not lock the city out for the rest of the turn.
		local fresh = force or (remembered == nil) or (remembered.turn ~= turn);
		-- ⚠ NO REFUSAL LIST. It was a workaround for the missing
		-- PARAM_INSERT_MODE, and with that fixed it does more harm than good:
		-- it rests on GetCurrentProductionTypeHash reading back reliably, and
		-- it does not. Measured with the insert mode in place, one run logged
		-- 12 build orders against 15 refusals — BUILDING_GRANARY refused seven
		-- times, which a multi-turn building cannot honestly earn. A read that
		-- returns empty while something is genuinely in production marks every
		-- candidate refused in turn and empties the ladder, which is the same
		-- failure it was written to prevent.
		if (current == nil or current == 0) and fresh then
			-- Ask for candidates in ladder order and keep going until one
			-- actually STARTS. `pcall` succeeding means the request did not
			-- throw, not that the game accepted it, and `CanProduce` green-lit
			-- items this city could not begin -- so a refused first entry used
			-- to end the attempt and the ladder's fallthrough never ran.
			--
			-- Measured before this existed: run settler-20260729T221605Z asked
			-- for UNIT_SETTLER in its capital on all 83 turns from turn 2 to
			-- 85, every one `applied = true`, and finished with ONE city, ZERO
			-- units and 501 unspent Gold. The queue read back empty every turn
			-- because the order never landed, so the city built nothing at all
			-- for the whole game while the always-available floor sat unused
			-- below a candidate that could never start.
			-- ⚠ NO `goto` HERE. Civilization VI's script runtime is Lua 5.1 and
			-- labels arrived in 5.2, so a `goto continue` loads fine under a
			-- modern `luac -p` and then silently refuses to compile in the game.
			-- That is not hypothetical: it cost run settler-20260729T224210Z,
			-- which sat in a configured game for half an hour emitting only
			-- `autoclose` events because THIS FILE never loaded. The autoclose
			-- context lives in a separate file and kept working, which is what
			-- made it look like a stalled game rather than a broken script.
			local name, row, why = chooseProduction(city, counts, #cities, turn);
			if row ~= nil then
				local params = buildParams(row);
				if params ~= nil then
					local ok = pcall(function()
						CityManager.RequestOperation(city, CityOperationTypes.BUILD, params);
					end);
					if ok then
						issued = issued + 1;
						lastBuild[cityId] = { turn = turn, item = name };
						if name == "UNIT_SETTLER" then counts.settler = counts.settler + 1;
						elseif name == "UNIT_BUILDER" then counts.builder = counts.builder + 1;
						elseif name == "UNIT_SCOUT" then counts.scout = counts.scout + 1;
						elseif row.Kind == "KIND_UNIT" then counts.military = counts.military + 1;
						end
					end
					emit("build", {
						turn = turn,
						city = try(function() return Locale.Lookup(city:GetName()); end, "?"),
						item = name, reason = why, applied = ok,
					});
				end
			end
		end
	end
	return issued;
end

-- ------------------------------------------------------- research and civics
--
-- Cheapest-first. Which branch is taken matters far less than that one is
-- always taken: an unset research is an end-turn blocker, and a blocker that
-- is never answered freezes the whole run.

local function chooseResearch(player, pid)
	local techs = try(function() return player:GetTechs(); end);
	if techs == nil then return nil; end
	local best, bestCost;
	for row in GameInfo.Technologies() do
		if try(function() return techs:CanResearch(row.Index); end, false) then
			local cost = try(function() return techs:GetResearchCost(row.Index); end, 0) or 0;
			if bestCost == nil or cost < bestCost then best, bestCost = row, cost; end
		end
	end
	if best == nil then return nil; end
	local params = {};
	params[PlayerOperations.PARAM_TECH_TYPE] =
		try(function() return techs:GetResearchPath(best.Hash); end) or best.Hash;
	params[PlayerOperations.PARAM_INSERT_MODE] = PlayerOperations.VALUE_EXCLUSIVE;
	local ok = pcall(function()
		UI.RequestPlayerOperation(pid, PlayerOperations.RESEARCH, params);
	end);
	return ok and best.TechnologyType or nil;
end

local function chooseCivic(player, pid)
	local culture = try(function() return player:GetCulture(); end);
	if culture == nil then return nil; end
	local best, bestCost;
	for row in GameInfo.Civics() do
		if try(function() return culture:CanProgress(row.Index); end, false) then
			local cost = try(function() return culture:GetCultureCost(row.Index); end, 0) or 0;
			if bestCost == nil or cost < bestCost then best, bestCost = row, cost; end
		end
	end
	if best == nil then return nil; end
	local params = {};
	params[PlayerOperations.PARAM_CIVIC_TYPE] =
		try(function() return culture:GetCivicPath(best.Hash); end) or best.Hash;
	params[PlayerOperations.PARAM_INSERT_MODE] = PlayerOperations.VALUE_EXCLUSIVE;
	local ok = pcall(function()
		UI.RequestPlayerOperation(pid, PlayerOperations.PROGRESS_CIVIC, params);
	end);
	return ok and best.CivicType or nil;
end

-- Policy cards are free strength: every open slot is a bonus not being taken,
-- and an unfilled slot is also an end-turn blocker, so leaving them alone
-- costs twice. The choice is deliberately crude -- the first unlocked card
-- that fits the slot -- because having a card in every slot is worth far more
-- than choosing between cards, and the alternative is a policy model that has
-- to be kept current with the ruleset.
local function fillPolicies(player)
	local culture = try(function() return player:GetCulture(); end);
	if culture == nil then return nil; end
	local open = try(function() return culture:GetNumPolicySlotsOpen(); end, 0) or 0;
	if open <= 0 then return nil; end
	local slots = try(function() return culture:GetNumPolicySlots(); end, 0) or 0;
	if slots <= 0 then return nil; end

	local taken = {};
	for i = 0, slots - 1 do
		local policy = try(function() return culture:GetSlotPolicy(i); end, -1);
		if policy ~= nil and policy >= 0 then taken[policy] = true; end
	end

	-- Clear every slot and re-send every card, exactly as the shipped
	-- government screen's Confirm does. Sending only the additions leaves the
	-- engine believing the untouched cards are still in their slots, and the
	-- request is refused as a conflict -- which shows up as a civic-slot
	-- blocker that is answered every turn and never goes away.
	--
	-- addList is keyed by slot index, not a sequence: the key *is* the slot the
	-- card goes into.
	local clearList, addList, added = {}, {}, 0;
	for i = 0, slots - 1 do
		clearList[#clearList + 1] = i;
		local policy = try(function() return culture:GetSlotPolicy(i); end, -1);
		if policy ~= nil and policy >= 0 then
			local row = GameInfo.Policies[policy];
			if row ~= nil then addList[i] = row.Hash; end
		else
			local slotId = try(function() return culture:GetSlotType(i); end);
			local slotName = slotId ~= nil and try(function()
				return GameInfo.GovernmentSlots[slotId].GovernmentSlotType;
			end) or nil;
			for row in GameInfo.Policies() do
				local fits = (slotName == nil)
					or (row.GovernmentSlotType == slotName)
					or (slotName == "SLOT_WILDCARD");
				if fits and not taken[row.Index]
						and try(function() return culture:IsPolicyUnlocked(row.Hash); end, false)
						and not try(function() return culture:IsPolicyObsolete(row.Hash); end, false) then
					addList[i] = row.Hash;
					taken[row.Index] = true;
					added = added + 1;
					break;
				end
			end
		end
	end
	if added == 0 then return nil; end
	local ok = pcall(function() culture:RequestPolicyChanges(clearList, addList); end);
	return ok and ("policies+" .. added) or nil;
end

-- Later governments are strictly stronger and unlock more slots, so the
-- newest unlocked one is taken whenever the game will allow the change.
local function chooseGovernment(player)
	local culture = try(function() return player:GetCulture(); end);
	if culture == nil then return nil; end
	if not try(function() return culture:CanChangeGovernmentAtAll(); end, false) then
		return nil;
	end
	local current = try(function() return culture:GetCurrentGovernment(); end, -1);
	local best;
	for row in GameInfo.Governments() do
		if try(function() return culture:IsGovernmentUnlocked(row.Hash); end, false) then
			best = row;
		end
	end
	if best == nil or best.Index == current then
		pcall(function() culture:SetGovernmentChangeConsidered(true); end);
		return nil;
	end
	local ok = pcall(function() culture:RequestChangeGovernment(best.Hash); end);
	pcall(function() culture:SetGovernmentChangeConsidered(true); end);
	return ok and best.GovernmentType or nil;
end

local function choosePantheon(player, pid)
	local religion = try(function() return player:GetReligion(); end);
	if religion == nil then return nil; end
	for row in GameInfo.Beliefs() do
		if row.BeliefClassType == "BELIEF_CLASS_PANTHEON" then
			local taken = try(function() return Game.GetReligion():IsInSomePantheon(row.Index); end, true);
			if taken == false then
				local params = {};
				params[PlayerOperations.PARAM_BELIEF_TYPE] = row.Hash;
				local ok = pcall(function()
					UI.RequestPlayerOperation(pid, PlayerOperations.FOUND_PANTHEON, params);
				end);
				if ok then return row.BeliefType; end
			end
		end
	end
	return nil;
end

-- ---------------------------------------------------------------- the turn

-- How many times each expensive pass may run in one turn.
--
-- The controller shares a process with the game it is playing, so a pass that
-- walks the policy table or tests twenty build items runs *instead of* the
-- game advancing. Unbounded, they turned a four-second turn into ten minutes:
-- every batch of game-core events re-ran the lot. Bounded, the same decisions
-- get made and the game gets its frames back.
local passes = {};

local function spend(name, limit)
	if (passes[name] or 0) >= limit then return false; end
	passes[name] = (passes[name] or 0) + 1;
	return true;
end

local ticksSeen = 0;
local ticksTaken = 0;

local blockerNames = nil;

local function blockerName(id)
	if blockerNames == nil then
		blockerNames = {};
		pcall(function()
			for k, v in pairs(EndTurnBlockingTypes) do blockerNames[v] = k; end
		end);
	end
	return blockerNames[id] or tostring(id);
end

local function currentBlocker(pid)
	return try(function()
		return NotificationManager.GetFirstEndTurnBlocking(pid);
	end);
end

-- Not every blocker actually blocks. "Units have moves" and its relatives are
-- the interface nagging that something could still be done this turn -- the
-- shipped end-turn button cycles to the next idle unit instead of ending, but
-- that is a courtesy, not a rule, and the option that drives it is a user
-- preference. Treating them as hard stops is what produced a run that spent
-- eighty-one attempts and twenty-seven forfeited notifications on turn 1
-- without ever ending it.
local SOFT_BLOCKERS = {
	ENDTURN_BLOCKING_UNITS = true,
	ENDTURN_BLOCKING_UNIT_NEEDS_ORDERS = true,
	ENDTURN_BLOCKING_STACKED_UNITS = true,
	ENDTURN_BLOCKING_CITY_RANGE_ATTACK = true,
	ENDTURN_BLOCKING_DISTRICT_RANGE_ATTACK = true,
	ENDTURN_BLOCKING_UNIT_PROMOTION = true,
	ENDTURN_BLOCKING_CONSIDER_RAZE_CITY = true,
	ENDTURN_BLOCKING_CONSIDER_DISLOYAL_CITY = true,
	-- Prompts this controller has no answer for. Listing them is not giving
	-- up: an unlisted blocker burns forty attempts and then forfeits its
	-- notification, and each of those attempts re-runs a table scan while the
	-- game waits. Named here, they cost one order pass and the turn ends.
	ENDTURN_BLOCKING_COMMEMORATION_AVAILABLE = true,
	ENDTURN_BLOCKING_GOVERNOR_APPOINTMENT = true,
	ENDTURN_BLOCKING_GOVERNOR_PROMOTION = true,
	ENDTURN_BLOCKING_GOVERNOR_IDLE = true,
	ENDTURN_BLOCKING_GOVERNOR_OPPORTUNITY = true,
	ENDTURN_BLOCKING_GIVE_INFLUENCE_TOKEN = true,
	ENDTURN_BLOCKING_CLAIM_GREAT_PERSON = true,
	ENDTURN_BLOCKING_ARTIFACT = true,
	ENDTURN_BLOCKING_EMERGENCY_NEEDS_ATTENTION = true,
	ENDTURN_BLOCKING_WORLD_CONGRESS_LOOK = true,
	ENDTURN_BLOCKING_WORLD_CONGRESS_SESSION = true,
	ENDTURN_BLOCKING_SPY_CHOOSE_ESCAPE_ROUTE = true,
	ENDTURN_BLOCKING_SPY_CHOOSE_DRAGNET_PRIORITY = true,
};

-- Answer the decision the game says it is waiting on. Returning the name of
-- what was answered (rather than a boolean) is what makes a stuck run
-- diagnosable: the log says which blocker recurred, not merely that one did.
local function answerBlocker(player, pid, blocker, turn)
	local name = blockerName(blocker);
	-- Every answer below walks a GameInfo table, so each is budgeted. Answering
	-- twice in a turn is the useful case -- the first attempt can be refused
	-- while something else settles -- and answering two hundred times is how a
	-- turn takes ten minutes.
	if name == "ENDTURN_BLOCKING_RESEARCH" then
		if not spend("research", cfg.MaxResearchPasses or 2) then return nil; end
		return chooseResearch(player, pid);
	elseif name == "ENDTURN_BLOCKING_CIVIC" then
		if not spend("civic", cfg.MaxCivicPasses or 2) then return nil; end
		return chooseCivic(player, pid);
	elseif name == "ENDTURN_BLOCKING_PRODUCTION" then
		if not spend("production", cfg.MaxProductionPasses or 4) then return nil; end
		return driveProduction(player, turn, true) > 0 and "production" or nil;
	elseif name == "ENDTURN_BLOCKING_PANTHEON" then
		if not spend("pantheon", cfg.MaxPantheonPasses or 2) then return nil; end
		return choosePantheon(player, pid);
	elseif name == "ENDTURN_BLOCKING_FILL_CIVIC_SLOT" then
		if not spend("policies", cfg.MaxPolicyPasses or 2) then return nil; end
		return fillPolicies(player);
	elseif name == "ENDTURN_BLOCKING_CONSIDER_GOVERNMENT_CHANGE" then
		if not spend("government", cfg.MaxGovernmentPasses or 2) then return nil; end
		return chooseGovernment(player) or "government considered";
	elseif name == "ENDTURN_BLOCKING_UNITS"
			or name == "ENDTURN_BLOCKING_UNIT_NEEDS_ORDERS"
			or name == "ENDTURN_BLOCKING_STACKED_UNITS" then
		local given = orderUnits(player, pid, turn);
		return given > 0 and "units" or nil;
	end
	return nil;
end

-- The escape hatch. A blocker this controller cannot answer -- a governor
-- promotion, a World Congress session -- would otherwise stop the run dead,
-- and a run that stops reports nothing at all. Dismissing forfeits that one
-- decision and is logged as such, which is a far better trade than a hang.
local function dismissBlocker(pid, blocker)
	return try(function()
		local notification = NotificationManager.FindEndTurnBlocking(blocker, pid);
		if notification == nil then return false; end
		NotificationManager.Dismiss(notification:GetPlayerID(), notification:GetID());
		return true;
	end, false);
end

local turnsPlayed = 0;
local lastTurnSeen = -1;
local attempts = 0;

local inTick = false;
local finished = false;


-- ------------------------------------------------------------- state export
--
-- The full board, once a turn, so CIVVIS can be the thing that decides.
--
-- The controller's own heuristics are a weak second AI: they duplicate what
-- `AdvancedAi` already does and do it worse, which is why an ancient army was
-- still being built in 1100 AD. The intended architecture is the other way
-- round -- mirror the real state into the simulator, let the simulator
-- strategise, and let this mod (or the harness) actuate the answer.
--
-- ⚠ BUDGET. `GameCoreEventPublishComplete` fires many times per frame and a
-- full-map scan on every one of them took a turn from three seconds to over ten
-- minutes once already. This runs from `playTurn`, which is once per turn, and
-- only when `cfg.ExportState` asks for it. Tiles are emitted in chunks so no
-- single log line is unbounded.
local function exportState(player, pid, turn)
	if cfg.ExportState ~= true then return; end

	local cities = {};
	eachCity(player, function(city)
		local queue = try(function()
			local q = city:GetBuildQueue();
			return q and q:GetCurrentProductionTypeHash() or 0;
		end, 0);
		cities[#cities + 1] = {
			id = try(function() return city:GetID(); end, -1),
			x = try(function() return city:GetX(); end, -1),
			y = try(function() return city:GetY(); end, -1),
			pop = try(function() return city:GetPopulation(); end, -1),
			capital = try(function() return city:IsCapital(); end, false),
			producing = queue,
			food = try(function() return city:GetGrowth():GetFood(); end, -1),
			defense = try(function() return city:GetDistricts():GetDefenseStrength(); end, -1),
			damage = try(function() return city:GetDistricts():GetDefenseDamage(); end, -1),
		};
	end);

	local units = {};
	eachUnit(player, function(unit)
		local name = unitTypeName(unit);
		local row = GameInfo.Units[name];
		units[#units + 1] = {
			id = try(function() return unit:GetID(); end, -1),
			kind = name,
			x = try(function() return unit:GetX(); end, -1),
			y = try(function() return unit:GetY(); end, -1),
			hp = 100 - (try(function() return unit:GetDamage(); end, 0) or 0),
			moves = try(function() return unit:GetMovesRemaining(); end, -1),
			combat = row ~= nil and (row.Combat or 0) or 0,
			ranged = row ~= nil and (row.RangedCombat or 0) or 0,
		};
	end);

	-- Rivals: only what we have actually met, so the mirror never contains
	-- knowledge the seat has not earned.
	local rivals = {};
	local diplomacy = try(function() return player:GetDiplomacy(); end);
	for _, otherId in ipairs(try(function() return PlayerManager.GetAliveMajorIDs(); end, {})) do
		if otherId ~= pid and diplomacy ~= nil
				and try(function() return diplomacy:HasMet(otherId); end, false) then
			local other = Players[otherId];
			local theirCities = {};
			pcall(function()
				for _, city in other:GetCities():Members() do
					theirCities[#theirCities + 1] = {
						x = city:GetX(), y = city:GetY(),
						capital = try(function() return city:IsCapital(); end, false),
						defense = try(function()
							return city:GetDistricts():GetDefenseStrength();
						end, -1),
					};
				end
			end);
			rivals[#rivals + 1] = {
				player = otherId,
				at_war = try(function() return diplomacy:IsAtWarWith(otherId); end, false),
				score = try(function() return other:GetScore(); end, -1),
				cities = theirCities,
			};
		end
	end

	emit("state", {
		turn = turn,
		gold = try(function() return math.floor(player:GetTreasury():GetGoldBalance()); end, -1),
		faith = try(function() return math.floor(player:GetReligion():GetFaithBalance()); end, -1),
		science = try(function() return player:GetTechs():GetScienceYield(); end, -1),
		culture = try(function() return player:GetCulture():GetCultureYield(); end, -1),
		score = try(function() return player:GetScore(); end, -1),
		cities = cities,
		units = units,
		rivals = rivals,
	});
end


-- The map itself, in chunks, on a slow cadence.
--
-- A `Game` cannot be reconstructed from cities and units alone — every decision
-- CIVVIS makes is about ground. So the terrain, features, resources, ownership
-- and revealed state have to cross too.
--
-- ⚠ This is the one genuinely expensive emit in the mod. A Tiny map is roughly
-- 1,100 plots; scanning them every turn is precisely the pattern that once took
-- a turn from three seconds to over ten minutes. So it runs every
-- `TileExportEvery` turns (default 25), and splits the map across several lines
-- so no single log record is unbounded. Terrain does not change often; what
-- changes fast is ownership and revelation, and 25 turns is well inside the
-- window where those still matter.
local function exportTiles(player, pid, turn)
	if cfg.ExportState ~= true then return; end
	local every = cfg.TileExportEvery or 25;
	if turn % every ~= 0 then return; end
	local width = try(function() return Map.GetGridSize(); end, 0) or 0;
	local height = 0;
	pcall(function() width, height = Map.GetGridSize(); end);
	if width <= 0 or height <= 0 then return; end

	local chunk, chunks, index = {}, 0, 0;
	local function flush()
		if #chunk == 0 then return; end
		chunks = chunks + 1;
		emit("tiles", {
			turn = turn, width = width, height = height,
			chunk = chunks, plots = chunk,
		});
		chunk = {};
	end

	for y = 0, height - 1 do
		for x = 0, width - 1 do
			local plot = try(function() return Map.GetPlot(x, y); end);
			if plot ~= nil then
				local revealed = try(function()
					return plot:IsRevealed();
				end, false);
				-- Unrevealed ground is deliberately sent as a hole rather than
				-- as its true terrain: the mirror must not know more than the
				-- seat does, or the simulator would plan on stolen information.
				if revealed then
					index = index + 1;
					chunk[index] = {
						x = x, y = y,
						t = try(function() return plot:GetTerrainType(); end, -1),
						f = try(function() return plot:GetFeatureType(); end, -1),
						r = try(function() return plot:GetResourceType(); end, -1),
						o = try(function() return plot:GetOwner(); end, -1),
						w = try(function() return plot:IsWater(); end, false),
						i = try(function() return plot:IsImpassable(); end, false),
						fw = try(function() return plot:IsFreshWater(); end, false),
					};
					if index >= (cfg.TileChunk or 250) then
						flush(); index = 0;
					end
				end
			end
		end
	end
	flush();
	emit("tiles_done", { turn = turn, chunks = chunks, width = width, height = height });
end

local function playTurn(player, pid, turn)
	local research, civic;
	if try(function() return player:GetTechs():GetResearchingTech(); end, -1) < 0 then
		research = chooseResearch(player, pid);
	end
	if try(function() return player:GetCulture():GetProgressingCivic(); end, -1) < 0 then
		civic = chooseCivic(player, pid);
	end
	local policies = fillPolicies(player);
	-- Refresh the war picture once a turn, not once a tick: it walks every
	-- city of every civilization this player has met.
	warTarget = findWarTarget(player, pid);
	local war = declareWar(player, pid, countUnits(player), turn);
	exportState(player, pid, turn);
	exportTiles(player, pid, turn);
	local builds = driveProduction(player, turn);
	local ordered, stuck = orderUnits(player, pid, turn);
	emit("turn", {
		policies = policies,
		war = war,
		target = warTarget and (warTarget.capital and "capital" or "city") or nil,
		turn = turn,
		score = try(function() return player:GetScore(); end, -1),
		gold = try(function() return math.floor(player:GetTreasury():GetGoldBalance()); end, -1),
		cities = cityCount(player),
		units = countUnits(player).total,
		research = research, civic = civic,
		builds = builds, ordered = ordered, stuck = stuck,
		actions = lastActions,
		ticks_seen = ticksSeen, ticks_taken = ticksTaken,
		blocker = blockerName(currentBlocker(pid)),
	});
end

local function tick()
	if finished or inTick or cfg.Play == false then return; end
	inTick = true;
	local ok, err = pcall(function()
		local player, pid = localPlayer();
		if player == nil then return; end
		if not try(function() return player:IsTurnActive(); end, false) then return; end

		local turn = try(function() return Game.GetCurrentGameTurn(); end, -1);
		if turn ~= lastTurnSeen then
			lastTurnSeen = turn;
			turnsPlayed = turnsPlayed + 1;
			attempts = 0;
			passes = {};
			playTurn(player, pid, turn);
		end

		-- Answer whatever the game says it is waiting on, then end the turn
		-- anyway.
		--
		-- Waiting for the blocker to clear before ending was the design, and it
		-- deadlocks: Civilization VI's end-turn blockers are the interface
		-- telling a person what they could still do, not the engine refusing to
		-- advance. A policy slot that cannot be filled, a city with nothing it
		-- can build, a unit that will not take an order -- each of those is a
		-- notification that comes straight back after it is answered or
		-- dismissed, and a controller that treats it as a gate sits on the same
		-- turn until the run times out. Answering and then ending loses at most
		-- the value of one decision; waiting loses the whole game.
		local blocker = currentBlocker(pid);
		local none = try(function() return EndTurnBlockingTypes.NO_ENDTURN_BLOCKING; end, 0);
		if blocker ~= nil and blocker ~= none then
			local name = blockerName(blocker);
			attempts = attempts + 1;
			local answered;
			if SOFT_BLOCKERS[name] then
				-- Bounded per turn. The order pass is the expensive one, and a
				-- soft blocker that will not clear -- a unit the engine keeps
				-- listing as ready -- would otherwise run it on every batch of
				-- game-core events for the rest of the turn.
				if spend("soft", cfg.MaxSoftPasses or 3) then
					orderUnits(player, pid, turn);
				end
				answered = "units";
			else
				answered = answerBlocker(player, pid, blocker, turn);
			end
			if attempts == 1 or attempts % (cfg.BlockerReportEvery or 25) == 0 then
				emit("blocked", { turn = turn, blocker = name,
				                  attempts = attempts, answered = answered });
			end
			-- Only if the same blocker has survived a whole turn's worth of
			-- attempts is the notification dropped, and that is reported as the
			-- forfeit it is.
			if attempts >= (cfg.MaxBlockedAttempts or 40) then
				local dropped = dismissBlocker(pid, blocker);
				emit("dismissed", { turn = turn, blocker = name,
				                    dismissed = dropped, attempts = attempts });
				attempts = 0;
			end
		end

		pcall(function()
			if UI.GetInterfaceMode() ~= InterfaceModeTypes.SELECTION then
				UI.SetInterfaceMode(InterfaceModeTypes.SELECTION);
			end
			UI.RequestAction(ActionTypes.ACTION_ENDTURN);
		end);
	end);
	inTick = false;
	if not ok then emit("error", { where = "tick", error = tostring(err) }); end
end

-- ------------------------------------------------------------------- events

local started = false;
local ticks = 0;

local function ensureStarted()
	if started then return; end
	-- Wait for a seat and a started game before doing anything. The game core
	-- publishes events through most of loading, so the first tick arrives long
	-- before there is a local player to read; a survey taken then reports
	-- nothing useful and a rehost issued then is simply dropped.
	--
	-- The gate is the game's own state, not a tick count. A count looks
	-- equivalent and is not: once a game is sitting idle waiting for the
	-- player, the core publishes almost nothing, so a threshold of twenty
	-- ticks is never reached and the controller never starts -- which is
	-- indistinguishable from a controller that was never loaded.
	ticks = ticks + 1;
	local pid = try(function() return Game.GetLocalPlayer(); end, -1);
	if pid == nil or pid < 0 then return; end
	local turn = try(function() return Game.GetCurrentGameTurn(); end, -1);
	if turn == nil or turn < 1 then return; end
	if ticks < (cfg.StartAfterTicks or 3) then return; end
	started = true;

	pcall(resolveActions);

	-- Try to impose a turn limit from inside the game.
	--
	-- The simple Create Game screen has no max-turns control and the Advanced
	-- Setup one is below the fold of a panel that has to be scrolled, so a game
	-- started by the harness runs to turn five hundred and a Score victory --
	-- the cheapest win there is -- never arrives. Whether the engine honours a
	-- limit set after the game exists is not documented anywhere, so it is set
	-- and then *read back*, and the readback is what the seat record reports.
	if (cfg.MaxTurns or 0) > 0 then
		pcall(function()
			GameConfiguration.SetMaxTurns(cfg.MaxTurns);
			GameConfiguration.SetTurnLimitType(TurnLimitTypes.CUSTOM);
		end);
		emit("turn_limit", {
			asked = cfg.MaxTurns,
			config = try(function() return GameConfiguration.GetMaxTurns(); end, -1),
			game = try(function() return Game.GetMaxTurns(); end, -1),
		});
	end

	pcall(survey);
	-- The load screen waits on a keypress before the first turn runs, and
	-- nothing in any log says so -- a harness watching for turn data sees only
	-- a hang. Both dismissals the shipped screen offers are attempted.
	pcall(function() Events.LoadScreenClose(); end);
	pcall(function()
		local id = Input.GetActionId("StartGame");
		if id ~= nil then Events.InputActionTriggered(id); end
	end);

	-- A game with no marker was not configured by this run: it is whatever the
	-- menu started. Reconfigure and host again rather than play the wrong game
	-- and report a difficulty that was never set.
	local marker = try(function() return GameConfiguration.GetValue("CIVVIS_SETUP"); end);
	if cfg.Rehost ~= false and (marker == nil or marker == "") then
		finished = true;  -- do not play the throwaway game
		pcall(rehost);
	end
end

-- What actually finishes a turn is being called again after each batch of game
-- core events. A per-frame ContextPtr:SetUpdate does not run in a script-only
-- in-game context -- it was tried, and the first game sat on turn 1 with a
-- founded city and an unanswered production prompt, having emitted exactly one
-- turn record. Orders also resolve over several frames, so one pass at the
-- bottom of playTurn could not end a turn even if the update did fire.
-- How many game-core publish batches have arrived, and how many of them this
-- controller acted on. The event fires many times per frame, and every tick
-- queries the notification system and the turn state before deciding it has
-- nothing to do -- so acting on all of them spends the game's own frame budget
-- on asking whether there is anything to spend it on.
local function onGameCoreTick()
	ensureStarted();
	ticksSeen = ticksSeen + 1;
	if ticksSeen % (cfg.TickEvery or 16) ~= 0 then return; end
	ticksTaken = ticksTaken + 1;
	tick();
end

local function onLocalPlayerTurnBegin()
	ensureStarted();
	tick();
end

local function onTeamVictory(team, victoryType, eventID)
	local pid = try(function() return Game.GetLocalPlayer(); end, -1);
	local ourTeam = try(function()
		return (pid ~= nil and pid >= 0) and Players[pid]:GetTeam() or -1;
	end, -1);
	finished = true;
	emit("victory", {
		turn = try(function() return Game.GetCurrentGameTurn(); end, -1),
		team = team, victory = victoryType,
		local_player = pid, local_team = ourTeam,
		won = (team == ourTeam),
		score = try(function()
			return (pid ~= nil and pid >= 0) and Players[pid]:GetScore() or -1;
		end, -1),
		turns_played = turnsPlayed,
	});
end

local function onPlayerDefeat(player, defeat, eventID)
	local pid = try(function() return Game.GetLocalPlayer(); end, -1);
	if player == pid then finished = true; end
	emit("defeat", {
		turn = try(function() return Game.GetCurrentGameTurn(); end, -1),
		player = player, defeat = defeat, local_player = pid,
		ours = (player == pid),
	});
end

function Initialize()
	emit("loaded", { version = 2, play = cfg.Play ~= false });
	for name, handler in pairs({
		LocalPlayerTurnBegin = onLocalPlayerTurnBegin,
		GameCoreEventPublishComplete = onGameCoreTick,
		EndTurnBlockingChanged = onGameCoreTick,
		CityAddedToMap = onGameCoreTick,
		UnitAddedToMap = onGameCoreTick,
		CityProductionCompleted = onGameCoreTick,
		LoadGameViewStateDone = ensureStarted,
		TeamVictory = onTeamVictory,
		PlayerDefeat = onPlayerDefeat,
	}) do
		pcall(function() Events[name].Add(handler); end);
	end
end

Initialize();
