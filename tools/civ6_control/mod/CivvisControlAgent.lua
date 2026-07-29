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

local function canOperate(unit, hash)
	if hash == nil then return false; end
	local ok, result = pcall(function()
		return UnitManager.CanStartOperation(unit, hash, nil, false, false);
	end);
	return ok and result == true;
end

local function operate(unit, hash, params)
	if hash == nil then return false; end
	return pcall(function()
		UnitManager.RequestOperation(unit, hash, params or {});
	end);
end

local function commandUnit(unit, hash, params)
	if hash == nil then return false; end
	return pcall(function()
		UnitManager.RequestCommand(unit, hash, params or {});
	end);
end

-- Try each order in turn and take the first the engine accepts. Asking "can
-- you start this" rather than reasoning about terrain, charges and movement
-- keeps the controller honest: a rule this code does not model refuses the
-- order, and the next one down is tried instead.
local function firstOperation(unit, names)
	for _, name in ipairs(names) do
		local hash = OP[name];
		if canOperate(unit, hash) and operate(unit, hash) then
			return name;
		end
	end
	return nil;
end

local function orderSettler(unit)
	-- Founding somewhere legal on turn one beats holding the perfect site in
	-- reserve forever, which is what a settler policy that only ever moves
	-- does. Walking to a better site is a real improvement and a later one.
	return firstOperation(unit, {
		"UNITOPERATION_FOUND_CITY",
		"UNITOPERATION_AUTOMATE_EXPLORE",
	});
end

local function orderBuilder(unit)
	-- Builder automation is the engine's own improvement chooser: it picks the
	-- tile and the improvement with the logic the shipped AI uses, which beats
	-- a hand-rolled ranking and costs nothing to keep current.
	if commandUnit(unit, CMD["UNITCOMMAND_AUTOMATE"]) then return "automate"; end
	return firstOperation(unit, { "UNITOPERATION_BUILD_IMPROVEMENT" });
end

local function orderMilitary(unit, stillExploring)
	if stillExploring then
		local acted = firstOperation(unit, { "UNITOPERATION_AUTOMATE_EXPLORE" });
		if acted then return acted; end
	end
	return firstOperation(unit, {
		"UNITOPERATION_HEAL", "UNITOPERATION_FORTIFY", "UNITOPERATION_ALERT",
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

local function orderFor(unit, turn)
	local name = unitTypeName(unit);
	local row = GameInfo.Units[name];
	if name == "UNIT_SETTLER" then
		return orderSettler(unit);
	elseif name == "UNIT_BUILDER" then
		return orderBuilder(unit);
	elseif name == "UNIT_SCOUT" then
		return firstOperation(unit, { "UNITOPERATION_AUTOMATE_EXPLORE" });
	elseif row ~= nil and (row.Combat or 0) > 0 then
		return orderMilitary(unit, turn < (cfg.ExploreUntilTurn or 80));
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
local function orderUnits(player, turn)
	local given, stuck = 0, 0;
	local lastId, repeats = nil, 0;
	for _ = 1, (cfg.MaxUnitOrders or 40) do
		local unit = try(function() return player:GetUnits():GetFirstReadyUnit(); end);
		if unit == nil then break; end
		local id = try(function() return unit:GetID(); end, -1);
		if id == lastId then
			repeats = repeats + 1;
			-- The same unit came back after being given an order. One more try
			-- with the idle orders, then leave it: a unit that cannot be
			-- cleared is a reason to end the turn anyway, not to spin.
			if repeats > 1 then stuck = stuck + 1; break; end
		else
			lastId, repeats = id, 0;
		end
		local action = orderFor(unit, turn) or orderIdle(unit);
		if action == nil then stuck = stuck + 1; break; end
		given = given + 1;
	end
	return given, stuck;
end

-- --------------------------------------------------------- city production

local function chooseProduction(city, counts, nCities, turn)
	local function playable(name)
		local row = GameInfo.Types[name];
		if row == nil then return nil; end
		local ok, can = pcall(function()
			return city:GetBuildQueue():CanProduce(row.Hash, true);
		end);
		if ok and can == true then return row; end
		return nil;
	end

	local ladder = {};
	if (nCities + counts.settler) < (cfg.CityTarget or 6)
			and turn < (cfg.SettlerStopTurn or 9999) then
		ladder[#ladder + 1] = { "UNIT_SETTLER", "expand" };
	end
	if counts.scout < 1 and turn < 30 then
		ladder[#ladder + 1] = { "UNIT_SCOUT", "scout" };
	end
	if counts.military < math.max(2, nCities * (cfg.MilitaryPerCity or 1.5)) then
		for _, name in ipairs({ "UNIT_SWORDSMAN", "UNIT_ARCHER", "UNIT_SPEARMAN",
		                        "UNIT_SLINGER", "UNIT_WARRIOR" }) do
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

local function driveProduction(player, turn)
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
		if current == nil or current == 0 then
			local name, row, why = chooseProduction(city, counts, #cities, turn);
			if row ~= nil then
				local params = buildParams(row);
				if params ~= nil then
					local ok = pcall(function()
						CityManager.RequestOperation(city, CityOperationTypes.BUILD, params);
					end);
					if ok then
						issued = issued + 1;
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
};

-- Answer the decision the game says it is waiting on. Returning the name of
-- what was answered (rather than a boolean) is what makes a stuck run
-- diagnosable: the log says which blocker recurred, not merely that one did.
local function answerBlocker(player, pid, blocker, turn)
	local name = blockerName(blocker);
	if name == "ENDTURN_BLOCKING_RESEARCH" then
		return chooseResearch(player, pid);
	elseif name == "ENDTURN_BLOCKING_CIVIC" then
		return chooseCivic(player, pid);
	elseif name == "ENDTURN_BLOCKING_PRODUCTION" then
		return driveProduction(player, turn) > 0 and "production" or nil;
	elseif name == "ENDTURN_BLOCKING_PANTHEON" then
		return choosePantheon(player, pid);
	elseif name == "ENDTURN_BLOCKING_FILL_CIVIC_SLOT" then
		return fillPolicies(player);
	elseif name == "ENDTURN_BLOCKING_CONSIDER_GOVERNMENT_CHANGE" then
		return chooseGovernment(player) or "government considered";
	elseif name == "ENDTURN_BLOCKING_UNITS"
			or name == "ENDTURN_BLOCKING_UNIT_NEEDS_ORDERS"
			or name == "ENDTURN_BLOCKING_STACKED_UNITS" then
		local given = orderUnits(player, turn);
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

local function playTurn(player, pid, turn)
	local research, civic;
	if try(function() return player:GetTechs():GetResearchingTech(); end, -1) < 0 then
		research = chooseResearch(player, pid);
	end
	if try(function() return player:GetCulture():GetProgressingCivic(); end, -1) < 0 then
		civic = chooseCivic(player, pid);
	end
	local policies = fillPolicies(player);
	local builds = driveProduction(player, turn);
	local ordered, stuck = orderUnits(player, turn);
	emit("turn", {
		policies = policies,
		turn = turn,
		score = try(function() return player:GetScore(); end, -1),
		gold = try(function() return math.floor(player:GetTreasury():GetGoldBalance()); end, -1),
		cities = cityCount(player),
		units = countUnits(player).total,
		research = research, civic = civic,
		builds = builds, ordered = ordered, stuck = stuck,
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
			playTurn(player, pid, turn);
		end

		local blocker = currentBlocker(pid);
		local none = try(function() return EndTurnBlockingTypes.NO_ENDTURN_BLOCKING; end, 0);
		if blocker ~= nil and blocker ~= none then
			local name = blockerName(blocker);
			if SOFT_BLOCKERS[name] then
				-- Give the idle units something to do, then end the turn
				-- regardless. Waiting for this to clear is waiting forever.
				orderUnits(player, turn);
			else
				attempts = attempts + 1;
				local answered = answerBlocker(player, pid, blocker, turn);
				if attempts == 1 or attempts % (cfg.BlockerReportEvery or 25) == 0 then
					emit("blocked", { turn = turn, blocker = name,
					                  attempts = attempts, answered = answered });
				end
				if attempts >= (cfg.MaxBlockedAttempts or 40) then
					local dropped = dismissBlocker(pid, blocker);
					emit("dismissed", { turn = turn, blocker = name,
					                    dismissed = dropped, attempts = attempts });
					attempts = 0;
					if not dropped then return; end
				end
				return;
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
local function onGameCoreTick()
	ensureStarted();
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
