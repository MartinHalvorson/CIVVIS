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

-- ⚠ DECLARED HERE, NOT WHERE THEY ARE USED. Both are read by `answerBlocker`,
-- which is defined hundreds of lines above the orders section that owns them. A
-- Lua local referenced before its `local` statement resolves to a GLOBAL instead —
-- silently nil, no error — so the residual counter would have counted nothing and
-- the fires-check would have read clean. `check_scope.py` exists because of this
-- exact family of bug.
--
-- `awaiting` is the state of this turn's decision handshake; `residualAnswers`
-- counts the built-in passes that ran on a turn CIVVIS was credited with.
local awaiting = { turn = -1, ticks = 0, polls = 0, done = false, source = "none" };
local residualAnswers = {};
-- Emitted once: a defeat is not a per-turn condition.
local defeatReported = false;

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
	-- ⚠⚠ THE TRAILING NEWLINE IS REQUIRED, NOT COSMETIC. `Automation.Log` does not
	-- terminate its record — measured: the log's final byte was `}`. `watch.py`
	-- splits on newlines and holds the unterminated tail as `partial`, so the LAST
	-- event written was never delivered. With CIVVIS deciding, the last event
	-- written is the `state` the brain must answer, so the loop deadlocked waiting
	-- for a line that was already on disk. Terminating it here fixes every reader
	-- rather than relying on some later event to flush the earlier one.
	local line = PREFIX .. encode(payload);
	pcall(function() print(line); end);
	pcall(function() Automation.Log(line .. "\n"); end);
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
		"UNITOPERATION_MAKE_TRADE_ROUTE",
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

-- What a city is CURRENTLY building, as a name rather than a hash.
--
-- ★★★★ `GetCurrentProductionTypeHash()` returns a signed hash, and the export
-- shipped it raw: `producing: -1743686858`. That is the same trap already fixed for
-- difficulty — **reporting the hash is reporting nothing**, because nothing
-- downstream can turn it back into a thing. The mirror consequently had no idea what
-- any city was already building, so CIVVIS re-decided production from scratch every
-- turn with no knowledge of work in progress.
--
-- A production item can be a unit, a building, a district or a project, and the hash
-- namespace is shared, so each table is tried in turn.
local function productionName(hash)
	if hash == nil or hash == 0 then return nil; end
	local tables = { GameInfo.Units, GameInfo.Buildings,
	                 GameInfo.Districts, GameInfo.Projects };
	for _, kindTable in ipairs(tables) do
		local name = try(function()
			local row = kindTable[hash];
			return row and (row.UnitType or row.BuildingType
				or row.DistrictType or row.ProjectType or row.Type) or nil;
		end);
		if name ~= nil then return name; end
	end
	return nil;
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
		-- ★★★ WHICH VICTORIES THIS GAME ACTUALLY ALLOWS. Everything the war layer does
		-- is aimed at domination, and none of it means anything if the lobby has
		-- VICTORY_CONQUEST switched off. That was never checked — the whole siege
		-- chain could have been debugged against a game that cannot be won that way.
		-- `Game.IsVictoryEnabled` is the shipped `WorldRankings.lua` check.
		victories = {
			conquest = try(function() return Game.IsVictoryEnabled("VICTORY_CONQUEST"); end, nil),
			score = try(function() return Game.IsVictoryEnabled("VICTORY_SCORE"); end, nil),
			technology = try(function() return Game.IsVictoryEnabled("VICTORY_TECHNOLOGY"); end, nil),
			culture = try(function() return Game.IsVictoryEnabled("VICTORY_CULTURE"); end, nil),
			religious = try(function() return Game.IsVictoryEnabled("VICTORY_RELIGIOUS"); end, nil),
			diplomatic = try(function() return Game.IsVictoryEnabled("VICTORY_DIPLOMATIC"); end, nil),
		},
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

-- ⚠⚠ THE pcall GOES INSIDE THE LOOP.
--
-- This is the bug that hid every other unit bug in this file. With one pcall
-- wrapped around the whole walk, the FIRST callback that throws abandons the
-- rest of the roster — silently, because pcall reports nothing. Two things then
-- happen at once and neither looks like the cause:
--
-- * `countUnits` stops early, so the telemetry reports `units=2` while the
--   empire has ten. Every "is the army big enough" decision reads that number.
-- * pass 2 of `orderUnits` stops early, so the units after the throwing one get
--   NO ORDER AT ALL and stand where they were built. That is the "units are
--   stuck in cities" the operator kept seeing at turns 50, 75 and 100 of runs
--   settler-20260730T013057Z and T014005Z, and it is why it survived the
--   GetFirstReadyUnit fix, the reachable fix and the garrison fix: none of them
--   were ever reached.
--
-- Per-iteration pcall costs nothing and makes one bad unit cost one unit.
local function eachUnit(player, fn)
	pcall(function()
		for _, unit in player:GetUnits():Members() do
			pcall(function() fn(unit); end);
		end
	end);
end

local function eachCity(player, fn)
	pcall(function()
		for _, city in player:GetCities():Members() do
			pcall(function() fn(city); end);
		end
	end);
end

-- How badly our units are piled up, measured rather than inferred.
--
-- ⚠ Every single time units stacked in a city this session, the OPERATOR found it
-- on screen and no statistic in this file showed it. `stuck` does not: a unit on
-- standing automation legitimately accepts no new order, so a high `stuck` proves
-- nothing either way. This counts what the complaint actually describes — how
-- many of our units share one plot — so the next occurrence shows up in the
-- stream instead of in a screenshot.
-- The best score among civilizations we have met, and whether we lead it.
--
-- ★ THE NUMBER A SCORE VICTORY TURNS ON, and it was invisible. Score at the turn
-- limit is the reachable victory here — VICTORY_SCORE is EnabledByDefault in the
-- shipped Victories table — but the telemetry only ever showed OUR score, so
-- "score 98 at turn 94" could equally be a comfortable lead or a rout. Rival
-- score already existed in `exportState`, which is off by default, so no run had
-- it.
--
-- ⚠ Only civilizations we have MET. Reading the score of a civilization we have
-- never encountered is knowledge the seat has not earned, and a decision taken on
-- it would make the run worthless as a measurement.
local function rivalBest(player, pid)
	local diplomacy = try(function() return player:GetDiplomacy(); end);
	if diplomacy == nil then return nil, 0; end
	local best, met = nil, 0;
	for _, otherId in ipairs(try(function() return PlayerManager.GetAliveMajorIDs(); end, {})) do
		if otherId ~= pid
				and try(function() return diplomacy:HasMet(otherId); end, false) then
			met = met + 1;
			local score = try(function() return Players[otherId]:GetScore(); end, -1) or -1;
			if score >= 0 and (best == nil or score > best) then best = score; end
		end
	end
	return best, met;
end

local function stackCensus(player)
	local perPlot = {};
	eachUnit(player, function(unit)
		-- ⚠ MILITARY ONLY. Civilians share a plot with a garrison legally and
		-- constantly — a settler waiting in the capital, a builder passing
		-- through — so counting every unit reported `pile = 4` for one defender
		-- and three civilians and made a healthy empire look like the bug the
		-- operator originally reported. One military unit per tile is the rule
		-- this measures.
		local row = GameInfo.Units[unitTypeName(unit)];
		if row == nil or (row.Combat or 0) <= 0 then return; end
		local x = try(function() return unit:GetX(); end, -1);
		local y = try(function() return unit:GetY(); end, -1);
		if x >= 0 then
			local key = x .. ":" .. y;
			perPlot[key] = (perPlot[key] or 0) + 1;
		end
	end);
	local worst, piles = 0, 0;
	for _, n in pairs(perPlot) do
		if n > worst then worst = n; end
		if n > 1 then piles = piles + 1; end
	end
	return worst, piles;
end

-- Pure. ⚠ An `upgradeUnit` call had been spliced into the military branch here,
-- so *counting* the army issued upgrade orders — and its `return better` skipped
-- the increment, so an upgrading unit was never counted as military. Counting
-- runs more than once a turn and feeds the war threshold; it must not act.
-- Upgrading belongs in `orderFor`, which is where it now lives.
local function countUnits(player)
	local counts = { settler = 0, builder = 0, military = 0, scout = 0, siege = 0,
	                 ranged = 0, total = 0 };
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
		elseif name == "UNIT_BATTERING_RAM" or name == "UNIT_SIEGE_TOWER" then
			-- Support units: no Combat, so the military branch below never sees
			-- them and without this they would be built without limit.
			counts.siege = counts.siege + 1;
		elseif row ~= nil and (row.Combat or 0) > 0 then
			counts.military = counts.military + 1;
			-- Ranged counts as military AND as ranged: a siege needs both kinds and
			-- the ladder has to be able to tell them apart.
			if (row.RangedCombat or 0) > 0 then
				counts.ranged = counts.ranged + 1;
			end
		end
	end);
	return counts;
end

local function cityCount(player)
	local n = 0;
	eachCity(player, function() n = n + 1; end);
	return n;
end

-- ★★★★★ CITY DEFENCE, WHICH HAS READ -1 FOR THE WHOLE PROJECT.
--
-- `city:GetDistricts():GetDefenseStrength()` calls the method on the districts COLLECTION,
-- which does not have it. `try` swallowed the error and handed back its -1 fallback, so
-- every `defense` and `damage` field ever exported — ours AND every rival city — has been
-- -1. Any reasoning about how well defended a city is has been blind.
--
-- The shipped UI calls `GetDefenseStrength()` on a DISTRICT object, obtained from a plot:
-- `CityManager.GetDistrictAt(plot)` (CityBannerManager/CityPanel do this in a dozen
-- places). Same failure family as `CanProduce`'s exclusion test and the missing
-- PARAM_INSERT_MODE: the right name on the wrong object, failing silently.
--
-- ⚠ Why it matters now rather than as tidying: three gates must align for a war and they
-- have no common window — turn >= 35 from t35, ratio <= 1.3 only until ~t45, army >= 12
-- only after ~t70. Choosing WHICH gate to move needs to know what the target can actually
-- resist. `WarArmy = 12` was measured against WALLED cities late in a game; an unwalled
-- capital at t40 is a different problem, and defence strength is what distinguishes them.
--
-- Exported before any threshold is changed, deliberately: the loyalty work went the same
-- way — make it visible, read the real numbers, then decide. Guessing a number and calling
-- it a fix is what four attempts at the production ladder cost.
local function cityDefence(x, y)
	local plot = try(function() return Map.GetPlot(x, y); end);
	if plot == nil then return nil, nil; end
	local district = try(function() return CityManager.GetDistrictAt(plot); end);
	if district == nil then return nil, nil; end
	local strength = try(function() return district:GetDefenseStrength(); end);
	local damage = try(function() return district:GetDefenseDamage(); end);
	return strength, damage;
end

-- ★★★★★ Loyalty, the mechanism that has been quietly destroying the empire.
--
-- 22 of 39 runs past turn 60 lost at least one city, AT PEACE. A city that loses
-- loyalty becomes a Free City -- a different player -- so it vanishes from our list
-- and never appears in a rival's, which is precisely why 45 runs of telemetry showed
-- cities "peak then decline" with no cause attached.
--
-- Accessors copied from the shipped `CityBannerManager.lua` (Expansion2), not recalled.
-- `GetPotentialTransferPlayer()` is the game telling us outright who the city will fall
-- to -- it drives the banner's "LOC_LOYALTY_CITY_WILL_FALL_TO_TT" warning -- so it is a
-- free early warning rather than something to infer from the trend.
--
-- Returns loyalty, per-turn change, and who it will fall to (nil for nobody). All
-- three nil when the ruleset has no loyalty at all: Rise & Fall introduced it, so a
-- Vanilla ruleset has no `GetCulturalIdentity` and must not throw.
local function cityLoyalty(city)
	local identity = try(function() return city:GetCulturalIdentity(); end);
	if identity == nil then return nil, nil, nil; end
	local loyalty = try(function() return identity:GetLoyalty(); end);
	local perTurn = try(function() return identity:GetLoyaltyPerTurn(); end);
	local fallsTo = try(function() return identity:GetPotentialTransferPlayer(); end);
	-- The engine uses -1 for "nobody"; keep nil for that so callers cannot treat
	-- player 0 (us, the local player) as "no threat" by truthiness.
	if fallsTo ~= nil and fallsTo < 0 then fallsTo = nil; end
	return loyalty, perTurn, fallsTo;
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
local findSettleSite;

-- CIVVIS's own ranking of where the next city goes, baked in at install time.
--
-- ★ THIS IS CIVVIS PROVIDING THE DECISION, which is the architecture asked for.
-- The route is indirect because it has to be: the mod cannot read a file at
-- runtime (no `io` in this sandbox) and FireTuner does not answer — with a live
-- game and the correct log path, seven plausible framings on ports 4318/4319
-- executed nothing. Config baked at install time is the only inbound channel that
-- works, and it is sufficient here because the world is a function of the SEED:
-- `civvis-advise --plan` reads one run's exported map, ranks the ground with
-- `AdvancedAi::settle_ranking`, and the next run on that same seed follows it.
--
-- Measured reason to bother: on run settler-20260730T034143Z the agent's own
-- choices sat at CIVVIS ranks 25/48, 10/25 and 4/15 — middling ground by CIVVIS's
-- reckoning, on the axis CIVVIS's oracle work calls its biggest lever.
--
-- ⚠ The plan is ADVICE, not an order. A site is taken only if the engine agrees it
-- is legal for this settler right now; anything refused falls through to the
-- hand-rolled search. A plan that disagreed with the live game would otherwise
-- strand settlers exactly the way `committedSite` was built to prevent.
local settlePlan = nil;

-- ★ THE FIRES-CHECK, and it is not optional.
--
-- This project's most expensive mistakes have all been treatments that looked
-- applied and were not: a Settler requested on 83 consecutive turns with
-- `applied = true` and nothing ever built, and a value evaluator that has never
-- once loaded while `docs/EVAL.md` concluded it was "good and inert". A settle plan
-- baked into the config is exactly that shape of change — silent when it works and
-- silent when it does not.
--
-- So the stream says which brain chose each city: `plan` means CIVVIS's ranking
-- picked it, `search` means the hand-rolled Lua score did. An evaluation of
-- CIVVIS-as-decider is meaningless until `plan_sites` is non-zero.
local planFires = { plan = 0, search = 0, offered = 0 };
-- Is the loyalty-reach penalty starving the settle search? `capped` counts legal
-- sites rejected for being out of support range, `in_reach` the ones inside it.
-- ⚠ A single number could not answer this: `capped` alone rises on a big map with
-- plenty of near ground, and `in_reach` alone cannot show what was given up.
local siteCap = { capped = 0, in_reach = 0 };

local function planSite(player, pid, unit)
	-- ⚠ A PLAN SITE THE ENGINE HAS REFUSED MUST BE SKIPPED, or the plan is a trap.
	--
	-- Replacing the old "taken on offer" bookkeeping with board-derived occupancy
	-- removed the only escape hatch: with ZERO cities nothing is occupied, so the
	-- top-ranked site is offered every turn forever. If the settler cannot path
	-- there it walks for the whole game. Run settler-20260730T053416Z reached turn 50
	-- with **cities = 0** and one settler still trudging — worse than having no plan
	-- at all.
	--
	-- `orderSettler` already records a refusal in `refusedSite[id]` when the engine
	-- declines the move. Honouring it here is what lets the plan fall down its own
	-- ranking instead of dying on its first entry.
	local unitId = try(function() return unit:GetID(); end, -1);
	local refused = refusedSite[unitId] or {};
	if settlePlan == nil then
		settlePlan = {};
		local raw = cfg.SettlePlan;
		if type(raw) == "table" then
			for i = 1, #raw do
				local entry = raw[i];
				if type(entry) == "table" and entry.x ~= nil and entry.y ~= nil then
					settlePlan[#settlePlan + 1] = { x = entry.x, y = entry.y };
				end
			end
		end
	end
	if #settlePlan == 0 then return nil; end
	planFires.offered = planFires.offered + 1;
	-- ⚠ "USED" MEANS A CITY STANDS THERE, NOT THAT WE ONCE LOOKED AT IT.
	--
	-- The first version marked a site taken the moment it was OFFERED. This
	-- function runs once per settler per turn, so the plan burned through all 24
	-- sites in a handful of turns: run settler-20260730T045220Z read
	-- `plan_sites 24` at turn 37 with ONE city. The plan destroyed itself and then
	-- fell back to the Lua search, which is the opposite of the intent.
	--
	-- Occupancy is now derived from the board instead of remembered: a site is used
	-- if one of our cities is within the spacing rule of it. Stateless, so it cannot
	-- drift out of step with the game, and a settler that dies on the way leaves its
	-- target available again.
	local spacing = cfg.MinCitySpacing or 3;
	local occupied = {};
	eachCity(player, function(city)
		local cx = try(function() return city:GetX(); end, -1);
		local cy = try(function() return city:GetY(); end, -1);
		if cx >= 0 then occupied[#occupied + 1] = { x = cx, y = cy }; end
	end);
	-- ★★★★★ NEAREST OF THE GOOD ONES, NOT SIMPLY THE BEST ONE.
	--
	-- This loop used to return the FIRST unoccupied site in plan order, and the plan
	-- is CIVVIS's value ranking computed FROM THE CAPITAL (`advise.rs` passes
	-- `from = capital`). So every settler, wherever it was built, was sent to the
	-- globally top-ranked plot — often clean across the empire.
	--
	-- Measured cost of that, over twelve runs: **484 `move_to_site` orders against 48
	-- `found_city` orders — about TEN TURNS OF WALKING PER CITY.** With one settler
	-- in flight that is one city per ~15 turns, and over the ~100-turn horizon a run
	-- actually gets ([[civvis-civ6-runs-never-finish]]) it caps the empire at 3-4
	-- cities. The observed median is 3. That arithmetic, not any broken mechanism, is
	-- what starves the army (`wantArmy = MilitaryPerCity x cities`), which is why war
	-- is declared in only 19 of 47 runs, which is why no capital is ever taken.
	--
	-- ⚠ RAISING `SettlersInFlight` DOES NOT FIX IT AND WAS ALREADY REFUTED: run
	-- 010409Z ordered SEVENTEEN settlers, walked 166 times and founded 2 cities. More
	-- settlers walking the same long distances buys walking. The travel time is the
	-- constraint, so cut the travel.
	--
	-- ⚠ CIVVIS STILL DECIDES WHICH GROUND IS GOOD — that is the operator's
	-- architecture and this must not quietly become a hand-rolled scorer. What
	-- changes is only the choice AMONG comparably good ground: take the top
	-- `PlanNearWindow` sites CIVVIS offers, and let the settler that has to do the
	-- walking pick the closest of them.
	--
	-- It also agrees with CIVVIS's own measurement: civilian MOVEMENT was the largest
	-- single grant on its expansion axis (`expansion_swift`, 59.5%) while settler COST
	-- measured null. Distance-to-site is that same quantity from the other end.
	local window = cfg.PlanNearWindow or 6;
	local ux = try(function() return unit:GetX(); end, -1);
	local uy = try(function() return unit:GetY(); end, -1);
	local best, bestKey, bestRank, bestDist = nil, nil, -1, nil;
	local considered = 0;
	for i = 1, #settlePlan do
		local site = settlePlan[i];
		local key = site.x .. ":" .. site.y;
		local tooClose = refused[key] == true;
		for j = 1, #occupied do
			if plotDistance(occupied[j].x, occupied[j].y, site.x, site.y) < spacing then
				tooClose = true;
				break;
			end
		end
		if not tooClose then
			-- Legality is not asserted here. `orderSettler` already asks the
			-- engine to FOUND_CITY or MOVE_TO and honours a refusal, and this
			-- project has been burned repeatedly by gates that answered wrongly
			-- in exactly the position that mattered. Offer the plot; let the
			-- engine be the judge.
			local plot = try(function() return Map.GetPlot(site.x, site.y); end);
			if plot ~= nil then
				-- ⚠ Distance from the SETTLER, not from the capital. A settler
				-- built in the third city is the one that has to walk.
				local dist = 0;
				if ux >= 0 then
					dist = plotDistance(ux, uy, site.x, site.y);
				end
				if best == nil or dist < bestDist then
					best, bestKey, bestRank, bestDist = plot, key, i, dist;
				end
				considered = considered + 1;
				-- The window keeps this a tie-break among CIVVIS's best ground
				-- rather than a licence to settle anywhere near. Without it the
				-- nearest legal plot on the whole map wins and the ranking is
				-- discarded.
				if considered >= window then break; end
			end
		end
	end
	if best ~= nil then
		-- Both numbers, because "a plan site was chosen" reads green whether the
		-- window saved a walk or changed nothing. `rank` says how far down
		-- CIVVIS's ranking the choice was; `dist` is the walk it now faces.
		planFires.near_rank = (planFires.near_rank or 0) + bestRank;
		planFires.near_dist = (planFires.near_dist or 0) + (bestDist or 0);
		planFires.near_n = (planFires.near_n or 0) + 1;
		return best, bestKey, bestRank, bestDist;
	end
	return nil;
end

findSettleSite = function(player, pid, unit, turn)
	-- CIVVIS first. Its ranking already accounts for ring yields, fresh water,
	-- spacing against our own cities and distance; the hand-rolled search below is
	-- the fallback for ground the plan does not cover (a map it never saw, or every
	-- planned site already used).
	-- ⚠⚠ THE PLAN MUST NEVER BE ABLE TO STOP US FOUNDING A CITY.
	--
	-- It has now broken two runs. Advice that can starve the empire is not advice,
	-- and a settle plan is a nicety next to having any city at all: run
	-- settler-20260730T053416Z sat at **cities = 0 through turn 80** with one settler
	-- walking at a site it could not reach.
	--
	-- So the plan is abandoned outright if it has not produced a capital by
	-- `PlanGiveUpTurn`. CIVVIS keeps the decision when CIVVIS is working; the
	-- hand-rolled search takes over the moment the plan is demonstrably not.
	local planUsable = turn < (cfg.PlanGiveUpTurn or 25)
		or cityCount(player) > 0;
	local planned, planKey, planRank, planDist = nil, nil, nil, nil;
	if planUsable then
		planned, planKey, planRank, planDist = planSite(player, pid, unit);
	end
	if planned ~= nil then
		planFires.plan = planFires.plan + 1;
		emit("settle_choice", {
			source = "plan",
			x = try(function() return planned:GetX(); end, -1),
			y = try(function() return planned:GetY(); end, -1),
			-- `rank` is how far down CIVVIS's ranking this choice sat and `dist`
			-- is the walk it faces. Together they price the near-window: rank
			-- rising while dist falls is the trade working, rank rising while
			-- dist does NOT fall means the window is only losing value.
			rank = planRank,
			dist = planDist,
			turn = turn,
		});
		return planned;
	end
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

	-- ⚠ THREE, NOT FOUR. `CITY_MIN_RANGE` in the shipped GlobalParameters is **3**,
	-- so a four-tile rule is stricter than Civilization VI itself and rejects
	-- ground the game would happily accept. On a Tiny map shared with three rivals
	-- that is most of the map: run settler-20260730T010409Z ordered SEVENTEEN
	-- settlers and founded two cities, and the surplus simply walked.
	--
	-- Settling tight is right twice over. CIVVIS measured the settler walk at 16.8
	-- turns a city and found removing it the single biggest win on its whole
	-- expansion axis, and a city founded far from the capital is the one that
	-- FLIPS — runs 023440Z and 030431Z both bled cities with `war = null` on every
	-- turn, and 030431Z was eliminated at turn 130.
	local spacing = cfg.MinCitySpacing or 3;
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
					-- ★★★★★ A MAXIMUM DISTANCE, NOT ONLY A MINIMUM. Loyalty support
				-- comes from the population of our OWN nearby cities, so ground
				-- beyond that reach cannot be held at any price.
				--
				-- Measured with the new loyalty telemetry, run 071729Z: city
				-- (42,17) founded turn 29 THIRTEEN TILES from the capital (55,17)
				-- opened at loyalty 77 and **-23 a turn** — 77, 54, 37, 14, gone by
				-- turn 33. Four turns. It was doomed the moment it was founded, and
				-- 56% of runs past turn 60 lose a city exactly this way.
				--
				-- ⚠ A GOVERNOR CANNOT SAVE THIS and I built that fix first: the
				-- shipped `IdentityPressure: 8` against -23 a turn still leaves -15.
				-- Propping up remote cities is the wrong lever by a factor of three;
				-- the fix is not to found them there.
				--
				-- ⚠ Why the existing score did not catch it: `walk` charges distance
				-- from the SETTLER, and a settler that has already wandered thirteen
				-- tiles finds good ground right next to itself and scores it well.
				-- Nothing charged distance from the EMPIRE. `nearest` was computed
				-- and then only ever compared against the minimum spacing.
				--
					-- ⚠ A PENALTY, NOT A HARD CAP — deliberately, and I wrote it as a cap
					-- first. A cap is authority, and three regressions this session came
					-- from a mechanism handed a decision with no recourse when it was wrong:
					-- the settle plan starved the empire to ZERO cities through turn 90 in
					-- exactly that way. On a Tiny map shared with three rivals the 3..6 band
					-- can hold no legal ground at all, and a cap would then strand the
					-- settler for the whole game.
					--
					-- A penalty this large behaves AS a cap whenever anything in reach
					-- exists — no in-reach site can lose to an out-of-reach one — and
					-- degrades to "the least bad far site" when nothing is in reach. A city
					-- that revolts in four turns is a poor trade; a settler that never founds
					-- anything is a total loss.
					--
					-- Both branches counted (`capped` against `in_reach`) so that "the cap is
					-- starving the search" is visible rather than inferred.
					local reach = cfg.MaxEmpireDistance or 6;
					local outOfReach = (#cities > 0 and nearest > reach);
					if nearest >= spacing then
						if outOfReach then
							siteCap.capped = siteCap.capped + 1;
						else
							siteCap.in_reach = siteCap.in_reach + 1;
						end
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
							+ fresh + coast - walk
							-- Out of loyalty support range: dominate every yield
							-- term so an in-reach site always wins, without
							-- making the site illegal.
							- (outOfReach and (cfg.OutOfReachPenalty or 1000) or 0);
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
		-- The other half of the fires-check: a site the hand-rolled search chose.
		-- `plan` against `search` is what says whether CIVVIS is actually deciding.
		planFires.search = planFires.search + 1;
		emit("settle_choice", {
			source = "search",
			x = best:GetX(), y = best:GetY(), turn = turn,
		});
	end
	return best;
end

local function orderSettler(player, pid, unit, turn)
	-- ⚠ This binding was MISSING and `id` was therefore a nil global, so
	-- `refusedSite[id] = ...` threw "table index is nil" for every settler the
	-- engine would not path to its chosen site. The settler then got no order at
	-- all, which is why the empire sat on two cities and never settled again.
	-- Invisible until the roster pcall moved inside the loop: before that the
	-- throw simply ended the whole unit walk.
	local id = try(function() return unit:GetID(); end, -1);
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

-- ⚠⚠ A GARRISON POST IS A PLOT, CLAIMED BY EXACTLY ONE UNIT.
--
-- The version before this posted a unit to a *city* and called it home once
-- `plotDistance <= 1`. Both halves of that were the stack:
--
-- * A unit built in the capital is already at distance 0, so it fortified in the
--   city centre and never took a step.
-- * Once two units stood anywhere in the ring, `thinnestCity` refused to post a
--   third, so every later unit got no post and fell through to FORTIFY exactly
--   where it stood — the capital again.
--
-- Civilization VI permits any number of units to stack on a city centre, so
-- nothing in the engine ever pushed back. Five units on one plot, reported from
-- the screen at turns 50, 75 and 100.
--
-- Claiming plots makes it impossible by construction: two units cannot hold one
-- post, and standing still is only licensed ON one's own post.
local function postKey(x, y) return x .. ":" .. y; end

local garrisonPost = {};    -- unit id -> { x, y, key }
local postClaims = {};      -- "x:y"  -> unit id

local function releasePost(id)
	local post = garrisonPost[id];
	if post == nil then return; end
	if postClaims[post.key] == id then postClaims[post.key] = nil; end
	garrisonPost[id] = nil;
end

local function standable(x, y)
	local plot = try(function() return Map.GetPlot(x, y); end);
	if plot == nil then return false; end
	return try(function()
		return (not plot:IsWater()) and (not plot:IsImpassable());
	end, false);
end

-- ⚠ A dead unit's claim would hold its plot forever and the empire slowly runs
-- out of anywhere to stand. Rebuild from the living once a turn.
local function sweepPosts(player)
	local alive = {};
	eachUnit(player, function(unit)
		local id = try(function() return unit:GetID(); end, -1);
		if id >= 0 then alive[id] = true; end
	end);
	for id in pairs(garrisonPost) do
		if not alive[id] then releasePost(id); end
	end
end

-- The defensive posts of one city: the centre first, because the fortified unit
-- in the centre is the one that actually holds the place, then the ring outward
-- so the rest watch the approaches instead of joining the pile.
local function postsOf(city)
	local cx = try(function() return city:GetX(); end, -1);
	local cy = try(function() return city:GetY(); end, -1);
	local out = {};
	if cx < 0 then return out; end
	if standable(cx, cy) then
		out[#out + 1] = { x = cx, y = cy, key = postKey(cx, cy) };
	end
	for dx = -1, 1 do
		for dy = -1, 1 do
			if not (dx == 0 and dy == 0) and standable(cx + dx, cy + dy) then
				out[#out + 1] = { x = cx + dx, y = cy + dy,
				                  key = postKey(cx + dx, cy + dy) };
			end
		end
	end
	return out;
end

-- The nearest free post, filling the thinnest city first so a border city is not
-- left naked while the capital collects a crowd.
local function claimPost(player, unit)
	local id = try(function() return unit:GetID(); end, -1);
	if id < 0 then return nil; end
	local ux = try(function() return unit:GetX(); end, -1);
	local uy = try(function() return unit:GetY(); end, -1);
	local cap = cfg.GarrisonPerCity or 2;
	local best, bestRank;
	eachCity(player, function(city)
		local posts = postsOf(city);
		local held, free = 0, {};
		for i = 1, #posts do
			if postClaims[posts[i].key] ~= nil then
				held = held + 1;
			else
				free[#free + 1] = posts[i];
			end
		end
		for i = 1, #free do
			if held + i > cap then break; end
			-- Thinnest city dominates; walking distance breaks the tie.
			local rank = held * 1000 + plotDistance(ux, uy, free[i].x, free[i].y);
			if bestRank == nil or rank < bestRank then best, bestRank = free[i], rank; end
		end
	end);
	if best ~= nil then
		garrisonPost[id] = best;
		postClaims[best.key] = id;
	end
	return best;
end

-- Every city post is taken and this unit is surplus. It still may not stand on
-- top of somebody: step to the emptiest neighbouring plot instead. Fortifying in
-- place is precisely the behaviour being removed.
local function stepAside(player, unit)
	local id = try(function() return unit:GetID(); end, -2);
	local ux = try(function() return unit:GetX(); end, -1);
	local uy = try(function() return unit:GetY(); end, -1);
	if ux < 0 then return nil; end
	local taken = {};
	eachUnit(player, function(other)
		local oid = try(function() return other:GetID(); end, -3);
		if oid ~= id then
			local ox = try(function() return other:GetX(); end, -1);
			local oy = try(function() return other:GetY(); end, -1);
			if ox >= 0 then taken[postKey(ox, oy)] = true; end
		end
	end);
	if not taken[postKey(ux, uy)] then return nil; end   -- already alone
	local best, bestCrowd;
	for dx = -1, 1 do
		for dy = -1, 1 do
			local x, y = ux + dx, uy + dy;
			local key = postKey(x, y);
			if not (dx == 0 and dy == 0) and standable(x, y)
					and not taken[key] and postClaims[key] == nil then
				local crowd = 0;
				for ax = -1, 1 do
					for ay = -1, 1 do
						if taken[postKey(x + ax, y + ay)] then crowd = crowd + 1; end
					end
				end
				if bestCrowd == nil or crowd < bestCrowd then
					best, bestCrowd = { x = x, y = y }, crowd;
				end
			end
		end
	end
	if best == nil then return nil; end
	local params = {};
	params[UnitOperationTypes.PARAM_X] = best.x;
	params[UnitOperationTypes.PARAM_Y] = best.y;
	if operate(unit, OP["UNITOPERATION_MOVE_TO"], params) then return "disperse"; end
	return nil;
end

-- The nearest ground we have SEEN that belongs to a civilization we have met.
--
-- ★ WHY EXPLORING WAS NOT ENOUGH. `findWarTarget` needs a rival city plot to be
-- revealed, and letting two units run `AUTOMATE_EXPLORE` did not deliver one:
-- turn 120 of run settler-20260730T025856Z still read `target = None` with
-- twenty units alive, so no war was declared and domination stayed impossible —
-- while the rival's score ran to 493 against our 126. Automated explore seeks
-- unrevealed terrain, not enemies, and once the neighbourhood is charted it has
-- no reason to walk into somebody's borders.
--
-- Territory is the signpost. A civilization's cities sit inside its borders, so
-- the nearest plot we have seen that it owns is the direction to walk to find
-- one. `GetOwner` on a revealed plot is knowledge the seat has earned — the mirror
-- already exports exactly this field — so nothing here is stolen.
--
-- ⚠ Scanned on a schedule, not every turn. A full sweep is width*height plots and
-- doing that per turn per unit is precisely the cost that starves the game;
-- `exportTiles` scans on the same principle. The answer barely moves between
-- sweeps because borders do not.
-- Whether THIS SEAT has revealed a plot.
--
-- ★★ SUSPECTED CAUSE OF "NO WAR, EVER". `plot:IsRevealed()` takes no player
-- argument, and a gameplay context has no "local player" to resolve against, so
-- it may answer false for every plot on the map. Every honesty gate in this file
-- was built on it: `findWarTarget` needs a revealed rival city, `enemyGround`
-- needs a revealed owned plot, and `exportTiles` sends only revealed plots. If the
-- call always says false then war is impossible by construction — which is exactly
-- what the log shows: run settler-20260730T031023Z reached turn 229 having MET ALL
-- THREE rivals with `target = None` on every single turn.
--
-- So this tries the player-explicit table first and falls back to the plot method,
-- and EMITS which one answered. I am not going to assert which API is right on a
-- build where three previous API guesses were wrong; the event stream can say.
--
-- ⚠ FAILS OPEN if neither works, and says so. A gate that cannot be evaluated and
-- quietly answers "not revealed" is what disabled war silently for every game ever
-- played. A run whose stream carries `revealed_api: none` is NOT a valid
-- measurement of fog-honest play and must not be reported as one.
local revealedHow = nil;

local function noteRevealedApi(how)
	if revealedHow ~= nil then return; end
	revealedHow = how;
	emit("revealed_api", { how = how });
end

local function plotRevealed(pid, x, y)
	local viaTable = try(function()
		return PlayersVisibility[pid]:IsRevealed(x, y);
	end);
	if viaTable ~= nil then
		noteRevealedApi("PlayersVisibility");
		return viaTable == true;
	end
	local viaPlot = try(function()
		local plot = Map.GetPlot(x, y);
		if plot == nil then return nil; end
		return plot:IsRevealed();
	end);
	if viaPlot ~= nil then
		noteRevealedApi("plot");
		return viaPlot == true;
	end
	noteRevealedApi("none");
	return true;
end

local enemyGroundMemo = { turn = -1, pos = nil };

local function enemyGround(player, pid, turn)
	local every = cfg.EnemyScanEvery or 12;
	if enemyGroundMemo.turn >= 0 and (turn - enemyGroundMemo.turn) < every then
		return enemyGroundMemo.pos;
	end
	enemyGroundMemo.turn = turn;
	enemyGroundMemo.pos = nil;

	local diplomacy = try(function() return player:GetDiplomacy(); end);
	if diplomacy == nil then return nil; end
	local met = {};
	for _, otherId in ipairs(try(function() return PlayerManager.GetAliveMajorIDs(); end, {})) do
		if otherId ~= pid
				and try(function() return diplomacy:HasMet(otherId); end, false) then
			met[otherId] = true;
		end
	end
	if next(met) == nil then return nil; end

	local home = try(function() return player:GetCities():GetCapitalCity(); end);
	local hx = home and try(function() return home:GetX(); end, 0) or 0;
	local hy = home and try(function() return home:GetY(); end, 0) or 0;
	local width, height = 0, 0;
	pcall(function() width, height = Map.GetGridSize(); end);
	if width <= 0 or height <= 0 then return nil; end

	-- ★★★★★ AIM AT THE FRONTIER OF THEIR LAND, NOT AT ITS EDGE NEAREST US.
	--
	-- This used to keep the enemy-owned plot with the SMALLEST distance from home, and
	-- the nearest owned plot is by definition the border facing us. The probe walked to
	-- the border and stopped, so their BORDER got revealed and their CITIES never did —
	-- and `findWarTarget` needs a revealed CITY. Measured on run 083136Z at turn 70,
	-- after the frontier bootstrap had already worked:
	--
	--     probe_kind = enemy   (their ground IS revealed)
	--     rival cities SEEN = 0        target = None
	--
	-- So prefer an enemy plot WE HAVE SEEN that still has an UNSEEN neighbour: that is
	-- the edge of our knowledge inside their empire, and walking there is what uncovers
	-- what lies deeper. Nearest such plot, so the probe advances progressively instead
	-- of setting off for the far side of the world.
	--
	-- ⚠ Falls back to the deepest known enemy plot when their land holds no unseen edge,
	-- because at that point everything of theirs is charted and a city is already
	-- visible if one exists — a nil here would silently stop probing.
	--
	-- One engine pass for reveal state, then table lookups, for the reason given in
	-- `frontierGround`: nine calls per plot is what starves a turn.
	local seen, ours = {}, {};
	for y = 0, height - 1 do
		for x = 0, width - 1 do
			if plotRevealed(pid, x, y) then
				seen[y * width + x] = true;
				local owner = try(function()
					local plot = Map.GetPlot(x, y);
					return plot ~= nil and plot:GetOwner() or -1;
				end, -1);
				if owner ~= nil and met[owner] then ours[y * width + x] = true; end
			end
		end
	end

	local edge, edgeDist, deep, deepDist;
	for y = 0, height - 1 do
		for x = 0, width - 1 do
			if ours[y * width + x] then
				local d = plotDistance(hx, hy, x, y);
				local frontier = false;
				for dx = -1, 1 do
					for dy = -1, 1 do
						if not frontier and not (dx == 0 and dy == 0) then
							local nx, ny = x + dx, y + dy;
							if nx >= 0 and nx < width and ny >= 0 and ny < height
									and not seen[ny * width + nx] then
								frontier = true;
							end
						end
					end
				end
				if frontier and (edgeDist == nil or d < edgeDist) then
					edge, edgeDist = { x = x, y = y }, d;
				end
				if deepDist == nil or d > deepDist then
					deep, deepDist = { x = x, y = y }, d;
				end
			end
		end
	end
	enemyGroundMemo.pos = edge or deep;
	return enemyGroundMemo.pos;
end

-- ★★★★★ WHERE TO GO WHEN NO ENEMY GROUND IS KNOWN YET. `enemyGround` scans for REVEALED
-- plots owned by a met rival, so with none revealed it returns nil — and the probe that
-- exists to find the enemy could not start looking for them. Measured deadlock on run
-- 082338Z at turn 91:
--
--     met = 1 (a major civ)   their cities_SEEN = 0
--     explore orders after turn 25 = 195      probe orders = 0
--     war_blocked = no_target
--
-- `findWarTarget` needs HasMet AND a revealed rival city. Contact happens through units
-- and reveals none of their land, so the revealed gate binds forever, and
-- `AUTOMATE_EXPLORE` charts terrain with no reason to walk toward anybody.
--
-- The frontier is the answer available from earned knowledge alone: a plot WE HAVE SEEN
-- that still has an unseen neighbour is the edge of our map, and the farthest such plot
-- from home is the direction the world continues in. Walking there pushes the revealed
-- region outward instead of wandering inside it.
--
-- ⚠ Uses only what the seat has earned — every candidate is a plot we have revealed, and
-- an unrevealed NEIGHBOUR is knowledge of our own ignorance, not of the map.
local frontierMemo = { turn = -1, pos = nil };

local function frontierGround(player, pid, turn)
	local every = cfg.EnemyScanEvery or 12;
	if frontierMemo.turn >= 0 and (turn - frontierMemo.turn) < every then
		return frontierMemo.pos;
	end
	frontierMemo.turn = turn;
	frontierMemo.pos = nil;

	local home = try(function() return player:GetCities():GetCapitalCity(); end);
	local hx = home and try(function() return home:GetX(); end, 0) or 0;
	local hy = home and try(function() return home:GetY(); end, 0) or 0;
	local width, height = 0, 0;
	pcall(function() width, height = Map.GetGridSize(); end);
	if width <= 0 or height <= 0 then return nil; end

	-- ⚠ ONE ENGINE PASS, THEN PURE LUA. Asking the engine about each plot AND its eight
	-- neighbours is nine times the work of `enemyGround`'s scan — about twenty thousand
	-- guarded calls on a 60x38 map — and a table scan that runs while the game waits is
	-- exactly what has starved turns here before. So reveal-state is read once into a
	-- table and the edge test is then table lookups.
	local seen = {};
	for y = 0, height - 1 do
		for x = 0, width - 1 do
			if plotRevealed(pid, x, y) then seen[y * width + x] = true; end
		end
	end

	local best, bestDist;
	for y = 0, height - 1 do
		for x = 0, width - 1 do
			if seen[y * width + x] then
				-- An edge plot: seen, but with at least one unseen neighbour.
				local edge = false;
				for dx = -1, 1 do
					for dy = -1, 1 do
						if not edge and not (dx == 0 and dy == 0) then
							local nx, ny = x + dx, y + dy;
							if nx >= 0 and nx < width and ny >= 0 and ny < height
									and not seen[ny * width + nx] then
								edge = true;
							end
						end
					end
				end
				if edge then
					-- FARTHEST, not nearest: the near frontier is already being
					-- charted by the explorers, and the point is to reach ground a
					-- rival might own.
					local d = plotDistance(hx, hy, x, y);
					if bestDist == nil or d > bestDist then
						best, bestDist = { x = x, y = y }, d;
					end
				end
			end
		end
	end
	frontierMemo.pos = best;
	return best;
end

local function orderMilitary(unit, stillExploring, player, probeTo)
	-- A probe with somewhere to be walks there. Automated explore is for charting
	-- empty ground; finding a rival's city means going to a rival's border.
	if probeTo ~= nil then
		local params = {};
		params[UnitOperationTypes.PARAM_X] = probeTo.x;
		params[UnitOperationTypes.PARAM_Y] = probeTo.y;
		if operate(unit, OP["UNITOPERATION_MOVE_TO"], params) then
			return "probe";
		end
	end
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
	if player == nil then
		return firstOperation(unit, {
			"UNITOPERATION_FORTIFY", "UNITOPERATION_ALERT",
		});
	end
	local id = try(function() return unit:GetID(); end, -1);
	local ux = try(function() return unit:GetX(); end, -1);
	local uy = try(function() return unit:GetY(); end, -1);

	-- Drop a post that has stopped belonging to a city of ours.
	local post = garrisonPost[id];
	if post ~= nil then
		local mine = false;
		eachCity(player, function(city)
			local cx = try(function() return city:GetX(); end, -1);
			local cy = try(function() return city:GetY(); end, -1);
			if cx >= 0 and plotDistance(cx, cy, post.x, post.y) <= 1 then mine = true; end
		end);
		if not mine then releasePost(id); post = nil; end
	end
	if post == nil then post = claimPost(player, unit); end

	if post ~= nil then
		if ux == post.x and uy == post.y then
			return firstOperation(unit, {
				"UNITOPERATION_FORTIFY", "UNITOPERATION_ALERT",
			});
		end
		local params = {};
		params[UnitOperationTypes.PARAM_X] = post.x;
		params[UnitOperationTypes.PARAM_Y] = post.y;
		if operate(unit, OP["UNITOPERATION_MOVE_TO"], params) then
			return "garrison";
		end
		releasePost(id);   -- unreachable for now; do not hoard the plot
	end
	local aside = stepAside(player, unit);
	if aside then return aside; end
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

-- The army we had at the start of this turn, and whether it is enough to assault.
--
-- ★★★ THE ARMY IS DESTROYED BY ITS OWN SIEGE. Across every run that reached a wall
-- the pattern is identical: attack every turn, lose a unit a turn, never stop.
-- settler-20260730T054547Z went 15 units to 8 to 4 to 3 while the capital held, and
-- an earlier run went 26 to 7. Each melee strike on a defended city trades badly, so
-- attacking below strength does not chip the city down — it feeds the city.
--
-- A person stops, rebuilds, and returns in force. `assaultReady` is that: below the
-- threshold the army holds and garrisons while production replaces losses, and the
-- assault resumes only when there is enough of it to matter.
local armyNow = 0;
local assaultReady = false;

-- How many military units are out looking for the enemy this turn.
--
-- ★ THE REASON NO WAR EVER STARTED. `findWarTarget` requires a rival city plot to
-- be `IsRevealed()` — correctly, because meeting a civilization does not reveal
-- its empire and targeting a capital the seat has never seen would win games it
-- did not earn. But nothing then went looking. Military exploration stopped at
-- `ExploreUntilTurn` and every unit garrisoned for the rest of the game, so run
-- settler-20260730T023440Z reached turn 90 with TWENTY units, `war = null` and
-- `target = null` on every single turn, and 236 warrior ALERTs. Contact had been
-- made — the operator watched two first-contact screens — and the army still had
-- nobody to fight.
--
-- So the explore gate is no longer only a turn number: while no target is known,
-- a few units keep probing. Capped, because exploring is also how an army
-- evaporates — one run went 6 units to 3 wandering into barbarians while the war
-- threshold was never reached — so this buys reconnaissance without spending the
-- garrison.
local probesOut = 0;
-- What the last probe decision actually resolved to, for the fires-check.
local probeDest, probeKind = nil, nil;
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

	-- ★★★★★ WHO WE FIGHT, NOT JUST WHERE. This used to score candidates purely on
	-- `-distance + (capital and 12 or 0)` — proximity and capital-ness, with no notion
	-- of whether the target can be BEATEN. So it would pick the strongest civ on the
	-- map, and measured on the deepest run in project history it did exactly that:
	--
	--     war declared t88, held to t198 against player 3
	--     their visible cities 1 -> 10, they ELIMINATED another civ mid-war
	--     final score 203 (us) against 1066 (them)
	--
	-- Every mechanism on our side worked through all of it — cities held, army 10-18,
	-- assault running for 110 turns — and the target grew straight through the siege.
	-- No amount of siege tuning wins that fight, so target choice is upstream of all
	-- of it.
	--
	-- The term is a RATIO of scores, not a difference: score scales through the game,
	-- so "twice as strong" means the same thing at turn 50 and turn 200 while "300
	-- ahead" does not. A rival at twice our score is charged `StrengthWeight`, which
	-- at 20 is worth twenty tiles of walking; a rival at half our score is credited
	-- the same, so the weakest reachable enemy wins ties against a nearer strong one.
	-- Bounded either way: the credit cannot exceed StrengthWeight, and distance still
	-- rules out an unreachable weakling.
	--
	-- ⚠ Uses only what the seat can see: `HasMet` already gates the loop, and
	-- `GetScore` is what the HUD shows for a met rival — the same accessor the state
	-- export and `rivalBest` already use.
	--
	-- ⚠ Falls back to the old proximity-only behaviour when either score is
	-- unavailable (-1), rather than letting a failed accessor silently distort the
	-- choice.
	local ourScore = try(function() return player:GetScore(); end, -1) or -1;
	local best, bestScore;
	for _, otherId in ipairs(try(function() return PlayerManager.GetAliveMajorIDs(); end, {})) do
		if otherId ~= pid and try(function() return diplomacy:HasMet(otherId); end, false) then
			local other = Players[otherId];
			-- Once per rival, not once per city: this is a per-turn scan already.
			local theirScore = try(function() return other:GetScore(); end, -1) or -1;
			local strength = 0;
			if ourScore > 0 and theirScore >= 0 then
				strength = (cfg.StrengthWeight or 20)
					* ((theirScore / ourScore) - 1.0);
			end
			pcall(function()
				for _, city in other:GetCities():Members() do
					local cx = city:GetX();
					local cy = city:GetY();
					-- ⚠ The comment above this function promises only knowledge
					-- the seat has earned, and gating on HasMet alone did not
					-- deliver it: GetCities():Members() lists every city the
					-- rival owns, so the target could be a capital this seat had
					-- never laid eyes on. Meeting a civilization does not reveal
					-- its empire. Require the plot to be revealed to us.
					local seen = plotRevealed(pid, cx, cy);
					-- A capital is worth walking further for: taking every
					-- original capital is what actually ends the game.
					local capital = try(function() return city:IsCapital(); end, false);
					local score = -plotDistance(hx, hy, cx, cy) + (capital and 12 or 0)
						- strength;
					if seen and (bestScore == nil or score > bestScore) then
						best = { player = otherId, x = cx, y = cy, capital = capital,
						         their_score = theirScore, our_score = ourScore };
						bestScore = score;
					end
				end
			end);
		end
	end
	return best;
end

-- ★★★★ NAME THE REASON WAR DID NOT HAPPEN. This function had SIX silent `return nil`
-- paths, and a silent gate has already cost this project its single most expensive bug:
-- `plot:IsRevealed()` could not answer in a gameplay context, so `findWarTarget` never
-- succeeded and war was impossible by construction for the whole history of the project
-- — invisible because nothing recorded WHY.
--
-- ⚠ Live symptom this is aimed at: run 081203Z reached turn 90 with `army = 14` against a
-- gate of 12 and `target = city`, and STILL never declared. Both documented gates were
-- open, so the block is one of the others and no telemetry distinguished them. The call
-- itself is correct — PARAM_PLAYER_ONE/TWO and RequestPlayerOperation match the shipped
-- `DeclareWarPopup.lua` line for line.
--
-- ⚠ A LIKELY ANSWER THIS WILL CONFIRM OR KILL: runs report `tgt = city` while `met = 0`.
-- `findWarTarget` wants a revealed rival city, and a CITY-STATE or FREE CITY can be
-- revealed without meeting any major civ — so the target may be a minor we cannot declare
-- on, or one whose capture does nothing for domination. `war_blocked` will say which.
local warBlock = nil;

local function declareWar(player, pid, counts, turn)
	local function blocked(why)
		warBlock = why;
		return nil;
	end
	if cfg.MakeWar == false then return blocked("disabled"); end
	if turn < (cfg.WarFromTurn or 25) then return blocked("too_early"); end
	local target = warTarget;
	if target == nil then return blocked("no_target"); end
	-- ⚠ ORDER: TARGET, THEN VETO, THEN ARMY. Two orderings were wrong before this. Gates
	-- report the FIRST failure, so with the veto below the army check it was simply
	-- unreachable: run 095626Z showed `war_blocked` cycling `too_early` then `army_4`
	-- with a target at ratio 2.54, and never once evaluated the veto. A guard placed
	-- after a gate that usually fails is a guard that never runs.
	--
	-- The order is also right on the merits: if the target is hopeless, building MORE
	-- army toward it is equally wasted, so "too strong" should be the answer even when
	-- the army is short.
	--
	-- ⚠⚠ AND IT MUST COME AFTER `local target = warTarget`. Hoisting it above that line
	-- put `target.player` out of scope: a nil global, so `Players[nil]:GetScore()` threw
	-- inside `try`, `theirNow` read -1, and the veto SKIPPED ITSELF. A guard that
	-- silently declines to guard is worse than no guard, and `check_scope.py` is what
	-- caught it — "'target' is a local elsewhere in this file but is not in scope here".
	-- ★★★★★ A VETO, NOT JUST A BIAS. The strength term in `findWarTarget` lowers a
	-- strong rival's SCORE, which does nothing when there is only one candidate — and
	-- `met` is frequently 1 here. Measured on settler-20260730T094745Z: the term was
	-- live, the sole met rival was 2.17x our score, we declared anyway, and the run went
	-- from 3 cities to ONE CITY AND ONE UNIT by turn 150 (score 115 against 530).
	--
	-- Preferring the weakest of several enemies and refusing to fight a hopeless one are
	-- different mechanisms. This is the second: no declaration at all above the ratio.
	--
	-- ⚠ The ratio is recomputed here rather than trusted from selection time, because it
	-- DRIFTS: another run chose a target at 0.95 and was at 1.76 thirty turns later. The
	-- guard has to read the world as it is when the decision is made.
	--
	-- ⚠ Skipped when either score is unavailable, so a failed accessor cannot silently
	-- forbid every war — the failure mode of a veto is worse than that of a bias.
	local ourNow = try(function() return player:GetScore(); end, -1) or -1;
	local theirNow = try(function()
		return Players[target.player]:GetScore();
	end, -1) or -1;
	if ourNow > 0 and theirNow >= 0 then
		local ratio = theirNow / ourNow;
		if ratio > (cfg.MaxTargetRatio or 1.3) then
			return blocked(string.format("too_strong_%.2f", ratio));
		end
	end
	if counts.military < (cfg.WarArmy or 4) then
		return blocked("army_" .. tostring(counts.military));
	end
	local diplomacy = try(function() return player:GetDiplomacy(); end);
	if diplomacy == nil then return blocked("no_diplomacy"); end
	if try(function() return diplomacy:IsAtWarWith(target.player); end, false) then
		warDeclared[target.player] = true;
		return blocked("already_at_war");
	end
	if warDeclared[target.player] then return blocked("already_declared"); end
	if not try(function() return diplomacy:CanDeclareWarOn(target.player); end, true) then
		-- Records the player id, because "cannot declare on 62" (the Free Cities slot)
		-- and "cannot declare on 1" (a major civ) are completely different problems.
		return blocked("cannot_declare_on_" .. tostring(target.player));
	end
	warBlock = nil;
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
	-- ⚠ Support units still move up: a ram in position is what makes the NEXT
	-- assault work, and it takes no return fire on an approach tile.
	local row = GameInfo.Units[unitTypeName(unit)];
	local isSupport = row ~= nil and (row.Combat or 0) <= 0;
	if not assaultReady and not isSupport then
		-- Hold rather than trickle in. Returning nil sends this unit down to
		-- `orderMilitary`, which garrisons it and keeps it alive to attack later.
		return nil;
	end
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
	-- A support unit confers its bonus by STANDING NEXT TO the city; it has no
	-- combat strength, cannot attack and cannot capture. Aiming a battering ram at
	-- the city plot would throw it away and, worse, consume one of the
	-- `AssaultWidth` slots that only melee can use.
	local support = row ~= nil
		and (row.FormationClass == "FORMATION_CLASS_SUPPORT"
		     or (row.Combat or 0) <= 0);
	if support then
		local spot = approachTile(unit);
		if spot ~= nil then
			local near = {};
			near[UnitOperationTypes.PARAM_X] = spot.x;
			near[UnitOperationTypes.PARAM_Y] = spot.y;
			if operate(unit, OP["UNITOPERATION_MOVE_TO"], near) then
				assault.taken[spot.key] = true;
				return "siege_up";
			end
		end
		return firstOperation(unit, { "UNITOPERATION_FORTIFY", "UNITOPERATION_ALERT" });
	end
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
	elseif name == "UNIT_BATTERING_RAM" or name == "UNIT_SIEGE_TOWER" then
		-- ⚠ SUPPORT UNITS HAVE Combat = 0, so the military branch below never sees
		-- them. Without this branch a battering ram would be built and then fall
		-- through to SKIP_TURN at home for the rest of the game — production spent
		-- on the one unit that solves the wall, and it never walks to the wall.
		if atWar(player) then
			local pressed = pressAttack(unit, turn);
			if pressed then return pressed; end
		end
		-- No war yet: wait with the garrison rather than wander, since a support
		-- unit alone is free experience for anything that finds it.
		return firstOperation(unit, { "UNITOPERATION_FORTIFY", "UNITOPERATION_ALERT",
		                              "UNITOPERATION_SKIP_TURN" });
	elseif row ~= nil and (row.Combat or 0) > 0 then
		-- Upgrading is cheaper than losing the unit and rebuilding it a tier
		-- late, and an un-upgraded army is why strength read 78 against 357 in
		-- 1100 AD. This is the only place it belongs: it is an ORDER, and it
		-- consumes the unit's turn, so it must be reached through orderFor and
		-- never from countUnits.
		local better = upgradeUnit(unit);
		if better then return better; end
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
		local early = turn < (cfg.ExploreUntilTurn or 12);
		-- ⚠ HOME FIRST. Probing was my own fix and it backfired: with two units
		-- always out looking for an enemy, a newly founded city stood undefended
		-- and barbarians took it within five turns. Run settler-20260730T033439Z
		-- logged 38 probes, 34 explores, 18 heals and just TWO garrison orders,
		-- and went two cities to one between turn 20 and turn 25.
		--
		-- Reconnaissance is worth nothing if the empire it reports to has been
		-- captured, so nobody probes until every city has a defender posted.
		local posted = 0;
		for _ in pairs(garrisonPost) do posted = posted + 1; end
		local defended = posted >= cityCount(player);
		-- ⚠ THE DESTINATION IS DECIDED BEFORE THE PROBE IS SPENT. This used to set
		-- `probing = true` and increment `probesOut` and only THEN ask `enemyGround`
		-- where to go — so when it returned nil (which is always, until some enemy
		-- land is revealed) up to `ProbeUnits` units were counted as probing every
		-- turn while actually just exploring. The counter read as though
		-- reconnaissance were happening; the only honest signal was that `probe`
		-- never appeared in the action histogram.
		local probing = false;
		local probeTo = nil;
		if not early and defended and warTarget == nil
				and probesOut < (cfg.ProbeUnits or 2) then
			-- Known enemy ground first; the frontier only when nothing is known.
			probeTo = enemyGround(player, pid, turn);
			probeKind = probeTo ~= nil and "enemy" or nil;
			if probeTo == nil then
				probeTo = frontierGround(player, pid, turn);
				probeKind = probeTo ~= nil and "frontier" or "none";
			end
			probeDest = probeTo and (probeTo.x .. ":" .. probeTo.y) or nil;
			if probeTo ~= nil then
				probing = true;
				probesOut = probesOut + 1;
			end
		end
		return orderMilitary(unit, early or probing, player, probeTo);
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
	sweepPosts(player);
	probesOut = 0;

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
		-- Already asked for this one on this turn and the game did not start it.
		-- ⚠ This comment used to say `CanProduce` "will keep saying yes, so the
		-- caller's record of what was refused is the only thing that makes the
		-- ladder fall through" — which was true only because the call below was
		-- the wrong one (see the next block). With the correct predicate the
		-- engine now rejects what it cannot build, so `refused` is a backstop for
		-- genuine per-turn refusals rather than the sole escape hatch.
		if refused[name] then return nil; end
		local row = GameInfo.Types[name];
		if row == nil then return nil; end
		-- ★★★★★ `CanProduce(hash, true)` IS THE EXCLUSION TEST, NOT "can I build
		-- this now". The shipped `ProductionPanel.lua` documents the signature at
		-- line 1943:
		--
		--   BuildQueue::CanProduce(nDistrictHash, bExclusionTest, bReturnResults, ...)
		--
		-- and uses TWO different calls: `CanProduce(hash, true)` to decide whether an
		-- item appears in the LIST at all, then `CanProduce(hash, false, true)` to
		-- decide whether it can actually be STARTED (line 2038, and 1913 derives
		-- `isDisabled` from it). This agent used the listing test as its build check,
		-- so anything merely *available in principle* read as buildable.
		--
		-- ⚠ MEASURED CONSEQUENCE: `UNIT_SWORDSMAN` needs Iron. Without Iron it is not
		-- excluded, so the exclusion test said yes forever. Run 072900Z issued **329
		-- swordsman requests — TWENTY IN A SINGLE TURN across four cities — and not
		-- one swordsman was ever built.** The army froze at 8 warriors from turn 60 to
		-- turn 111 while `army` requests piled up, so the war gate (12) never opened
		-- and the run could not fight. An earlier run logged 255 of the same.
		--
		-- ⚠ Same failure class as the PARAM_INSERT_MODE bug: an API that answers a
		-- DIFFERENT QUESTION than the one being asked, while every request logs
		-- `applied = true`. "The request did not throw" is not "the engine took it",
		-- and here it was not even the right question.
		local ok, can = pcall(function()
			-- Three-arg form returns (canStart, results); take the verdict only.
			local canStart = city:GetBuildQueue():CanProduce(row.Hash, false, true);
			return canStart;
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
	-- ⚠ DEFEND BEFORE EXPANDING, because the settler sits ABOVE the army in this
	-- ladder and will otherwise take every hammer forever.
	--
	-- Measured on run settler-20260730T044823Z, the first run where CIVVIS chose the
	-- sites: turn 40 with TWO cities and ONE unit, three cities founded and one
	-- already lost. A city target of six kept a settler queued permanently, so the
	-- army was never built and barbarians took what the settlers founded. Expansion
	-- that cannot be held is not expansion.
	--
	-- One defender per city is the floor, not a garrison — enough that a new city is
	-- not free to the first raider. Above the floor the settler competes normally.
	local defenders = counts.military or 0;
	local floorNeeded = math.max(1, nCities);
	if (nCities + counts.settler) < (cfg.CityTarget or 6)
			and counts.settler < (cfg.SettlersInFlight or 1)
			and defenders >= floorNeeded
			and turn < (cfg.SettlerStopTurn or 9999) then
		ladder[#ladder + 1] = { "UNIT_SETTLER", "expand" };
	end
	if counts.scout < 1 and turn < 30 then
		ladder[#ladder + 1] = { "UNIT_SCOUT", "scout" };
	end
	-- An army large enough to take a city, not merely to garrison one. The
	-- floor rises as the war turn approaches, because a declaration made with
	-- two warriors is a declaration that loses.
	-- ★★★★★ A SATISFIABLE ARMY TARGET, or everything below it is dead code AGAIN.
	--
	-- This was `nCities * MilitaryPerCity` with no ceiling: 25 at five cities, and runs
	-- typically field 10-18, so it is never met. The seven development entries below the
	-- army block were therefore unreachable for the whole of a 203-turn game:
	--
	--     build reasons: army 88, expand 26, grow 14, improve 9, siege 8, ranged 5,
	--                    scout 4, defend 2        <-- `develop` NEVER FIRES
	--     built: 71 warriors, 26 settlers, 19 spearmen, 7 monuments, 7 granaries,
	--            and ZERO districts, campuses, libraries, commercial hubs or theatres
	--
	-- That is the score gap: 203 against 1088. Score comes from population, districts
	-- and buildings, and the empire was building none of the last two.
	--
	-- ⚠ THIS IS THE THIRD TIME THIS EXACT CLASS HAS BITTEN — the battering ram and the
	-- ranged floor were both unreachable below this same gate — and the second time I
	-- have fixed it only halfway: hoisting the builder, monument and granary above the
	-- army left the other seven entries exactly where they were. "Never put anything you
	-- need below an open-ended target" also means: check nothing ELSE is down there.
	--
	-- An army needs to be big enough to FIGHT, which is a fixed quantity (`WarArmy`,
	-- and `assaultReady`'s AssaultMin), not a multiple that grows with the empire
	-- forever. Capped, the ladder reaches development once the army is adequate, and
	-- falls back to rebuilding whenever losses drop it below the cap.
	local wantArmy = math.max(2, math.min(
		nCities * (cfg.MilitaryPerCity or 1.5),
		cfg.ArmyCap or ((cfg.WarArmy or 4) + 6)));
	if turn >= (cfg.WarFromTurn or 25) - 10 then
		wantArmy = math.max(wantArmy, (cfg.WarArmy or 4) + 2);
	end
	-- ★★★ A BATTERING RAM, AND IT MUST COME BEFORE THE ARMY.
	--
	-- This is the last broken link in the chain find -> declare -> besiege ->
	-- BREAK THE WALL -> capture. Ancient Walls halve melee damage and the city
	-- repairs between turns, so melee alone cannot take a walled city: one run fed
	-- **165 assault orders** into a wall and went from 26 units to 7, another logged
	-- 82 advances and 83 surrounds and never broke anything.
	--
	-- ⚠ I previously reported this entry as added and it WAS NOT IN THE CODE. The
	-- support dispatch in `orderFor` and the `counts.siege` bucket both went in; the
	-- one line that puts a ram in a build queue did not, so nothing ever asked for
	-- one. Two runs then declared war with no ram and I misread the cause as Masonry
	-- never being researched — true, but downstream of this.
	--
	-- ⚠ ORDER IS LOAD-BEARING. Below the army block this is unreachable:
	-- `wantArmy` is `MilitaryPerCity * cities` (15 at three cities) and is never
	-- satisfied, so production never gets past it. One ram at 65 production beats
	-- the fifteenth Warrior at 40, because without it every extra warrior is another
	-- unit that dies without breaking anything.
	-- ⚠ FOUR, NOT TWO — the rams are the breach and they die. Measured on run
	-- 083136Z, 53 turns into a verified war (`at_war = True`) against a CAPITAL:
	-- eight rams built, 26 `siege_up` positioning orders, and `siege = 0` alive at
	-- turn 141 with no capture. The floor already rebuilds them (eight from a floor
	-- of two is four replacements), so the problem is not that they stop coming — it
	-- is that fewer than two are alive at any moment, and walls halve melee damage
	-- while the city heals between turns, so a breach needs rams PRESENT.
	--
	-- ⚠ A support unit has no combat strength and the city bombards it, so more rams
	-- means more rams dying; this buys concurrency, not survival. The deeper fix is
	-- to escort them or to stop parking half the army on approach tiles (`surround`
	-- 47 against `advance` 46 at the same turn) — neither is attempted here, because
	-- one change at a time is the rule while pairing is unavailable.
	if warTarget ~= nil and (counts.siege or 0) < (cfg.SiegeUnits or 4) then
		ladder[#ladder + 1] = { "UNIT_BATTERING_RAM", "siege" };
	end
	-- ★★ A PURE MELEE ARMY CANNOT REDUCE A CITY, only walk into one.
	--
	-- Melee-first is right for CAPTURE and wrong for the phase before it. Measured
	-- on run settler-20260730T054547Z: rams built and in position, **118 warrior
	-- advances**, the capital never fell, and the army sat at 8 units because every
	-- attacker trades badly against a city and dies. The roster had ZERO ranged units
	-- — the melee-first ladder always finds Warrior buildable, so Archer is never
	-- reached.
	--
	-- Ranged bombards from range 2 and takes no return fire, so it reduces the city
	-- while melee stays alive to capture. ⚠ A floor, not a preference: an
	-- ARCHER-ONLY army is the opposite failure and this project has already had it —
	-- 518 archer advances and 31 range attacks with zero captures, because ranged
	-- cannot take a plot. Two or three archers alongside the melee, then melee again.
	if warTarget ~= nil and (counts.ranged or 0) < (cfg.RangedFloor or 3) then
		for _, name in ipairs({ "UNIT_ARCHER", "UNIT_SLINGER" }) do
			ladder[#ladder + 1] = { name, "ranged" };
		end
	end
	-- ★★★★★ THE ECONOMY GOES ABOVE THE OPEN-ENDED ARMY, OR IT IS DEAD CODE.
	--
	-- The builder and the nine development entries used to sit BELOW the army block.
	-- `playable` returns the first entry that can be built, and
	-- `wantArmy = nCities * MilitaryPerCity` is 25 at five cities — never satisfied —
	-- so a melee unit was always playable and nothing below it was ever reached.
	--
	-- Measured on run 072231Z, EVERY production request in 110 turns:
	--     255 UNIT_SWORDSMAN, 48 UNIT_SETTLER, 21 other units,
	--     ZERO buildings, ZERO districts, ZERO builders.
	-- Not one monument or granary in the whole game. Cities sat at pop 5/4/2/2 with a
	-- single builder, score 132 against a rival's 398 from ONE visible city.
	--
	-- ⚠ AND IT IS SELF-DEFEATING: the army was stuck at 10 for thirty turns against a
	-- gate of 12 *because* all production went to the army. Undeveloped cities produce
	-- almost nothing, so 255 swordsman requests bought ten units. Spending everything
	-- on soldiers is what made the army small.
	--
	-- ⚠ This is the THIRD time ladder order has done this here — the battering ram was
	-- unreachable below the same gate, and so was the ranged floor. The rule: never
	-- put anything you need below an open-ended target.
	--
	-- Bounded on purpose. Only the cheap per-city growth core goes above the army —
	-- a builder, a monument, a granary — all of which COMPLETE and stop being
	-- playable. Putting all nine development items above an open-ended army block
	-- would invert the failure and never raise a soldier at all.
	local defenceFloor = math.max(1, nCities);
	if counts.military < defenceFloor then
		for _, name in ipairs({ "UNIT_WARRIOR", "UNIT_SPEARMAN", "UNIT_SLINGER" }) do
			ladder[#ladder + 1] = { name, "defend" };
		end
	end
	if counts.builder < math.max(1, nCities * (cfg.BuilderPerCity or 0.8)) then
		ladder[#ladder + 1] = { "UNIT_BUILDER", "improve" };
	end
	for _, name in ipairs({ "BUILDING_MONUMENT", "BUILDING_GRANARY" }) do
		ladder[#ladder + 1] = { name, "grow" };
	end
	-- ★★★★★ A GUARANTEED SHARE, BECAUSE A LADDER POSITION IS NOT ENOUGH.
	--
	-- Capping `wantArmy` at 18 made development reachable in principle and it still never
	-- happened. Measured at turn 78 with the cap in place and a war running:
	--
	--     army = 12   (gate 12, cap 18)      siege = 6 alive
	--     `develop` still never fires, zero districts or libraries requested
	--
	-- Combat losses hold the army permanently BELOW its cap once fighting starts — which
	-- is exactly when a long game most needs science and gold — so anything below the
	-- army block stays unreachable for the rest of the game. Score is population,
	-- districts and buildings, and the previous run finished 203 against 1088 with seven
	-- monuments, seven granaries and nothing else.
	--
	-- So development gets one turn in `DevelopEvery` where it outranks the army outright.
	-- Not a reordering — a SHARE. The army still wins the other two turns in three, and
	-- the defence floor above still comes first, so this cannot strip a city of its
	-- garrison.
	--
	-- ⚠ FOURTH APPEARANCE OF THIS CLASS, and the second time my own fix for it was
	-- insufficient rather than wrong: the ram and the ranged floor were dead below this
	-- gate, then seven district entries were, then the cap turned out to be unreachable
	-- in the state that matters. The rule has three parts now: never put what you need
	-- below an open-ended target; check what ELSE is down there; and check the target is
	-- reachable in the state you actually care about.
	local DEVELOP = { "DISTRICT_CAMPUS", "BUILDING_LIBRARY",
	                  "DISTRICT_HOLY_SITE", "DISTRICT_COMMERCIAL_HUB",
	                  "BUILDING_WATER_MILL", "DISTRICT_THEATER",
	                  "BUILDING_ANCIENT_WALLS" };
	local function pushDevelop()
		for _, name in ipairs(DEVELOP) do
			ladder[#ladder + 1] = { name, "develop" };
		end
	end
	local devFirst = (turn % (cfg.DevelopEvery or 3)) == 0;
	if devFirst then pushDevelop(); end
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
	-- The same list below the army on the other two turns in three, so development is
	-- still preferred over the always-available floor when the army IS satisfied.
	if not devFirst then pushDevelop(); end
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
	-- ★ WAR NEEDS SPECIFIC TECHS, AND CHEAPEST-FIRST NEVER REACHES THEM.
	--
	-- `UNIT_BATTERING_RAM` requires TECH_MASONRY, and the ram is the only thing that
	-- lets melee through Ancient Walls. Measured on run settler-20260730T052930Z:
	-- war declared on a capital at turn 38, 82 advances and 83 surrounds logged, and
	-- **no ram ever built** because Masonry was never researched. The army went 10
	-- units to 6 hitting a wall it could not break. The same gap keeps the army in
	-- the Bronze Age: cheapest-first is a fine default and a poor war plan.
	--
	-- So while a war target exists, these come first if they can be researched. Only
	-- three, all Ancient, all cheap: the wall-breaker and the two melee upgrades.
	local wanted = { "TECH_MASONRY", "TECH_BRONZE_WORKING", "TECH_IRON_WORKING" };
	if warTarget ~= nil then
		for _, name in ipairs(wanted) do
			local row = GameInfo.Technologies[name];
			if row ~= nil
					and try(function() return techs:CanResearch(row.Index); end, false) then
				local params = {};
				params[PlayerOperations.PARAM_TECH_TYPE] =
					try(function() return techs:GetResearchPath(row.Hash); end) or row.Hash;
				params[PlayerOperations.PARAM_INSERT_MODE] = PlayerOperations.VALUE_EXCLUSIVE;
				local ok = pcall(function()
					UI.RequestPlayerOperation(pid, PlayerOperations.RESEARCH, params);
				end);
				if ok then return name .. ":war"; end
			end
		end
	end
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

-- Take a Dedication at the era boundary.
--
-- ★ MEASURED AS THE MOST-DECLINED DECISION IN THE PROJECT. Across 48 recorded
-- runs, `ENDTURN_BLOCKING_COMMEMORATION_AVAILABLE` fired **1763** times — more
-- than every other unanswered prompt combined, and against 44 for the next one
-- (influence tokens) and 24 for governor appointments. It was on the "no answer
-- for this" list, which kept it cheap but meant the empire forfeited a
-- Dedication in every era of every game ever played.
--
-- The preference order is not taste. `CommemorationModifiers` in the shipped
-- database says what each one actually grants, and INFRASTRUCTURE carries exactly
-- four: `GA_MOVEMENT`, `GA_PURCHASE_CIVILIAN`, `SETTLER_DISCOUNT_MODIFIER` and
-- `BUILDER_DISCOUNT_MODIFIER`. That is CIVVIS's expansion axis item for item, and
-- CIVVIS measured civilian **movement** as the single largest grant on it
-- (`expansion_swift`, 59.5%) while measuring settler **cost** as null. RELIGIOUS
-- also carries `GA_MOVEMENT`, so it comes second.
--
-- ⚠ Those `GA_` modifiers only apply in a Golden Age, and these runs are mostly
-- Normal or Dark. In a Normal age the choice mostly grants an era-score *quest*
-- instead — which still beats declining, because era score is what decides
-- whether the next age is Golden or Dark, and a Dark Age is a standing penalty.
-- So the rule is: always take one; take the expansion one when there is a choice.
local DEDICATION_ORDER = {
	"COMMEMORATION_INFRASTRUCTURE",
	"COMMEMORATION_RELIGIOUS",
	"COMMEMORATION_SCIENTIFIC",
	"COMMEMORATION_ECONOMIC",
	"COMMEMORATION_INDUSTRIAL",
	"COMMEMORATION_MILITARY",
	"COMMEMORATION_EXPLORATION",
	"COMMEMORATION_CULTURAL",
};

local function chooseDedication(player, pid)
	-- ⚠ GUARD THE ENUM MEMBERS FIRST. `params[nil] = x` throws "table index is
	-- nil", and that is exactly how orderSettler silently stopped settling
	-- earlier today. If this build does not expose the commemorate operation,
	-- decline cleanly instead of throwing once an era.
	local param = try(function() return PlayerOperations.PARAM_COMMEMORATION_TYPE; end);
	local operation = try(function() return PlayerOperations.COMMEMORATE; end);
	if param == nil or operation == nil then return nil; end
	local eras = try(function() return Game.GetEras(); end);
	if eras == nil then return nil; end
	local allowed = try(function()
		return eras:GetPlayerNumAllowedCommemorations(pid);
	end, 0) or 0;
	if allowed <= 0 then return nil; end
	local choices = try(function()
		return eras:GetPlayerCommemorateChoices(pid);
	end);
	if choices == nil then return nil; end

	-- The choices arrive as type names or as hashes depending on build, so index
	-- both ways rather than assuming one.
	local offered, names = {}, {};
	for _, choice in ipairs(choices) do
		local row = GameInfo.CommemorationTypes[choice];
		local name = row and row.CommemorationType or tostring(choice);
		offered[name] = choice;
		names[#names + 1] = name;
	end
	if #names == 0 then return nil; end

	local taken = 0;
	for _, preferred in ipairs(DEDICATION_ORDER) do
		if taken >= allowed then break; end
		local choice = offered[preferred];
		if choice ~= nil then
			local params = {};
			params[param] = choice;
			local ok = pcall(function()
				UI.RequestPlayerOperation(pid, operation, params);
			end);
			if ok then
				taken = taken + 1;
				offered[preferred] = nil;
			end
		end
	end
	-- Anything at all beats leaving the prompt standing: an unanswered
	-- commemoration costs an order pass every turn for the rest of the game and
	-- forfeits the bonus outright.
	if taken == 0 then
		for _, name in ipairs(names) do
			if offered[name] ~= nil then
				local params = {};
				params[param] = offered[name];
				local ok = pcall(function()
					UI.RequestPlayerOperation(pid, operation, params);
				end);
				if ok then taken = 1; break; end
			end
		end
	end
	return taken > 0 and ("dedication:" .. table.concat(names, ",")) or nil;
end

-- Appoint a governor, then post them to a city that has none.
--
-- ★ THIS IS THE ANSWER TO LOSING A CITY WITHOUT A WAR. Run
-- settler-20260730T023440Z went from three cities to two at turn 96 with
-- `war = null` on every turn — nobody had declared war on us. A city lost with no
-- war is a LOYALTY flip, and every governor in the shipped `Governors` table
-- carries `IdentityPressure: 8`: establishing any one of them is +8 loyalty a turn
-- in that city, which is the strongest single counter in the game. The agent
-- declined the prompt (24 occurrences measured), so it never had one.
--
-- Preference is from the database, not taste. THE_BUILDER is Magnus — production
-- and cheaper settlers, which is the expansion axis CIVVIS measures as its
-- biggest lever — and THE_DEFENDER carries `TransitionStrength: 150`, the highest
-- of the base governors, so it establishes hardest where loyalty is contested.
-- The two `AssignCityState` governors are skipped because they cannot be posted
-- to one of our own cities, and the Secret Societies ones do not exist in a
-- standard game.
local GOVERNOR_ORDER = {
	"GOVERNOR_THE_BUILDER",
	"GOVERNOR_THE_DEFENDER",
	"GOVERNOR_THE_EDUCATOR",
	"GOVERNOR_THE_MERCHANT",
	"GOVERNOR_THE_RESOURCE_MANAGER",
	"GOVERNOR_THE_CARDINAL",
};

-- Which city each appointed governor was posted to, kept across turns. The engine
-- has query methods for this but their names differ between builds, and guessing
-- a Civilization VI API has cost this project three failed fixes today, so the
-- assignment we made is the assignment we remember.
local governorPost = {};

local function chooseGovernor(player, pid)
	-- ⚠⚠ OFF BY DEFAULT: THIS FUNCTION SEGFAULTS THE GAME CORE. See the
	-- `ENDTURN_BLOCKING_GOVERNOR_APPOINTMENT` entry in `SOFT_BLOCKERS` for the
	-- three-run, byte-identical-stack evidence. Belt-and-braces with the skip list:
	-- a blocker route added later must not silently re-arm a known crash.
	if not (cfg.GovernorAppoint or cfg.GovernorAssign) then return nil; end
	-- ⚠ Enum members first. `params[nil] = x` throws "table index is nil".
	local govParam = try(function() return PlayerOperations.PARAM_GOVERNOR_TYPE; end);
	local cityParam = try(function() return PlayerOperations.PARAM_CITY_DEST; end);
	local appointOp = try(function() return PlayerOperations.APPOINT_GOVERNOR; end);
	local assignOp = try(function() return PlayerOperations.ASSIGN_GOVERNOR; end);
	if govParam == nil or appointOp == nil then return nil; end

	local governors = try(function() return player:GetGovernors(); end);
	if governors == nil then return nil; end

	-- 1. Spend a title if one is going spare.
	local appointed = nil;
	if cfg.GovernorAppoint and try(function() return governors:CanAppoint(); end, false) then
		for _, wanted in ipairs(GOVERNOR_ORDER) do
			local row = GameInfo.Governors[wanted];
			if row ~= nil then
				local held = try(function()
					return governors:HasGovernor(row.Hash);
				end, false);
				local possible = try(function()
					return governors:CanEverAppointGovernor(row.Hash);
				end, false);
				if not held and possible then
					local params = {};
					params[govParam] = row.Hash;
					local ok = pcall(function()
						UI.RequestPlayerOperation(pid, appointOp, params);
					end);
					if ok then appointed = wanted; break; end
				end
			end
		end
	end

	-- 2. Post anyone we hold to the city that is ABOUT TO REVOLT.
	--
	-- ★★★★★ THIS IS WHY THE EMPIRE NEVER GROWS. 22 of 39 runs past turn 60 — 56% —
	-- LOSE at least one city, some catastrophically (7 down to 4, 6 down to 2, 4 down
	-- to 2). It is not conquest. Run 070750Z held (20,20) and (18,25) at turn 59 with
	-- THREE military adjacent to the second and `at_war = false` against every rival;
	-- at turn 60 the city was simply gone, and it appears in no rival's city list. A
	-- city cannot be captured at peace, and a city that flips becomes a FREE CITY,
	-- which is a separate player. **It revolted.** Pop 5, five tiles from the capital,
	-- with a rival's cities five to seven tiles away pressing on it.
	--
	-- That single mechanism explains the shape every run has: cities peak, then
	-- decline. It also explains why every production fix bounced off — settling faster
	-- into ground that revolts is a treadmill, and it is why 010409Z's seventeen
	-- settlers produced two cities.
	--
	-- ⚠ Loyalty was INVISIBLE in the telemetry for 45 runs, which is exactly why this
	-- went unnoticed. `exportState` now carries it per city.
	--
	-- Governors are the lever, and the shipped database says so: every Governor row
	-- carries `IdentityPressure: 8`, so establishing one is +8 loyalty a turn, enough
	-- to turn a -5 slide into +3. Posting was previously to "the first city with no
	-- governor", which is iteration order — and the capital comes first and never
	-- flips, so the title went where it was needed least.
	local posted = nil;
	-- Gated independently of the appointment: two mutations, two flags, so whichever
	-- one faults can be identified without re-running both.
	if cfg.GovernorAssign and assignOp ~= nil and cityParam ~= nil then
		local taken = {};
		for _, where in pairs(governorPost) do taken[where] = true; end
		local target, targetRank = nil, nil;
		eachCity(player, function(city)
			local id = try(function() return city:GetID(); end, -1);
			if id < 0 or taken[id] then return; end
			-- Rank by the SLIDE, lowest first; ungoverned-but-stable last.
			--
			-- ⚠ This first used `GetPotentialTransferPlayer()` as the top priority,
			-- which was wrong: it names the transfer target (62, Free Cities) for
			-- every city we own, safe ones included, so every city tied in the top
			-- band. The rate is the only honest signal. Harmless by luck — the band
			-- was ordered by rate anyway — but it was measuring nothing.
			--
			-- ⚠ AND A GOVERNOR IS NOT ENOUGH ON ITS OWN: `IdentityPressure: 8`
			-- against the measured -23 a turn still loses. The real fix is
			-- `MaxEmpireDistance` in the settle search, which stops the city being
			-- founded out of reach in the first place. This just salvages the
			-- borderline ones.
			local loyalty, perTurn = cityLoyalty(city);
			local rank;
			if perTurn ~= nil and perTurn < 0 then
				rank = perTurn;
			else
				rank = 1000;
			end
			if targetRank == nil or rank < targetRank then
				target, targetRank = id, rank;
			end
		end);
		if target ~= nil then
			for _, wanted in ipairs(GOVERNOR_ORDER) do
				local row = GameInfo.Governors[wanted];
				if row ~= nil and governorPost[wanted] == nil
						and try(function() return governors:HasGovernor(row.Hash); end, false) then
					local params = {};
					params[govParam] = row.Hash;
					params[cityParam] = target;
					local ok = pcall(function()
						UI.RequestPlayerOperation(pid, assignOp, params);
					end);
					if ok then
						governorPost[wanted] = target;
						posted = wanted;
						break;
					end
				end
			end
		end
	end

	if appointed == nil and posted == nil then return nil; end
	return "governor:" .. tostring(appointed or "-") .. "/" .. tostring(posted or "-");
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

-- Spend envoys, take suzerainty, and levy the army that comes with it.
--
-- ★★★★ THE LARGEST UNANSWERED DECISION IN THE PROJECT, and it was on the
-- give-up list below for the whole of its history. In a 106-turn run,
-- `ENDTURN_BLOCKING_GIVE_INFLUENCE_TOKEN` fired **44 times** -- more than
-- production (21), research (13), units (8) and civics (7). The comment above
-- `chooseDedication` even names the number ("44 for the next one (influence
-- tokens)") because that count came out of the same census that moved
-- commemoration off the list. It was read and then left alone.
--
-- ⚠ It is also what HANGS the run. Force-skipping the blocker does not clear
-- it: at turn 106 the seat sat 10 minutes on `GIVE_INFLUENCE_TOKEN` while the
-- stall watchdog clicked dialogue coordinates at a prompt that is not a
-- dialogue. The shipped screen clears it with `SetGivingTokensConsidered(true)`
-- (`CityStates.lua:451`), so that is done here even when nothing is worth
-- buying -- an unspendable envoy must still end the turn.
--
-- ★ Why this is worth more than another combat tuning pass. CIVVIS measured
-- suzerainty as its single biggest headroom: an oracle granted it wins **56.7%
-- against 22.7%**, a near doubling and the largest gap any ablation has
-- produced. In the real game the seat has been discarding that lever for free
-- while sitting on hundreds of unspent gold.
--
-- ⚠ CONCENTRATE, DO NOT SPREAD. One envoy in six city-states buys six small
-- yield bonuses; three in one buys SUZERAINTY -- its unique bonus and, decisive
-- for a domination route, the right to levy its whole army. The runs that
-- collapse do so with the army at 2-3 units and gold unspent, so a levied army
-- is the one source of force that does not have to be built first.
--
-- Every accessor here is copied from the shipped `CityStates.lua` rather than
-- recalled: `GetTokensToGive`, `CanGiveInfluence`, `CanGiveTokensToPlayer`,
-- `GetTokensReceived`, `GetMostTokensReceived`, `GetSuzerain`, `CanLevyMilitary`,
-- `GetLevyMilitaryCost`, and one `GIVE_INFLUENCE_TOKEN` request PER TOKEN.
local MIN_ENVOY_TOKENS_SUZERAIN = 3;

-- Carried onto every turn event. ⚠ A boolean "envoys handled" would read green
-- both when a suzerainty was bought and when nothing was buyable and the flag
-- was merely cleared -- the same trap that let a Settler request report
-- `applied = true` on 83 consecutive turns with nothing built. So: envoys
-- placed, suzerainties held, and levies taken, separately.
local envoyTally = { placed = 0, suzerainties = 0, levies = 0, met_minors = 0 };

local function chooseEnvoy(player, pid, turn)
	local influence = try(function() return player:GetInfluence(); end);
	local oneParam = try(function() return PlayerOperations.PARAM_PLAYER_ONE; end);
	if influence == nil or oneParam == nil then return nil; end
	local giveOp = try(function() return PlayerOperations.GIVE_INFLUENCE_TOKEN; end);
	local levyOp = try(function() return PlayerOperations.LEVY_MILITARY; end);

	local diplomacy = try(function() return player:GetDiplomacy(); end);
	local tokens = try(function() return influence:GetTokensToGive(); end, 0) or 0;
	local canGive = try(function() return influence:CanGiveInfluence(); end, false);

	-- ⚠ MET CITY-STATES ONLY. The operator's standing constraint is that this
	-- agent may use only what a human in the seat can see, and an unmet minor
	-- is not on the human's screen. `PlayerManager.GetAliveMinors()` returns
	-- every one of them whether met or not, so the HasMet gate is what keeps
	-- this honest -- without it the agent would be reading the roster.
	local seen, suzerainties, levied = {}, 0, nil;
	local minors = try(function() return PlayerManager.GetAliveMinors(); end);
	if minors ~= nil then
		for _, minor in ipairs(minors) do
			pcall(function()
				local mid = try(function() return minor:GetID(); end, -1) or -1;
				local theirs = try(function() return minor:GetInfluence(); end);
				if mid < 0 or theirs == nil then return; end
				if not try(function() return theirs:CanReceiveInfluence(); end, false) then
					return;
				end
				if diplomacy ~= nil
					and not try(function() return diplomacy:HasMet(mid); end, false) then
					return;
				end
				local mine = try(function() return theirs:GetTokensReceived(pid); end, 0) or 0;
				local most = try(function() return theirs:GetMostTokensReceived(); end, 0) or 0;
				local holder = try(function() return theirs:GetSuzerain(); end, -1) or -1;
				-- Tokens still needed to take it: the suzerain is whoever holds
				-- the most, with a floor of three. `GetMostTokensReceived`
				-- already counts ours, so this reads 1 when we lead 2-2.
				local need = 0;
				if holder ~= pid then
					need = math.max(MIN_ENVOY_TOKENS_SUZERAIN, most + 1) - mine;
					if need < 1 then need = 1; end
				else
					suzerainties = suzerainties + 1;
				end
				seen[#seen + 1] = {
					id = mid, mine = mine, need = need, ours = holder == pid,
					takes = try(function()
						return influence:CanGiveTokensToPlayer(mid);
					end, false),
				};
			end);
		end
	end

	-- 1. Levy first, because it is the cheapest army in the game and this agent's
	--    failure mode is running out of units. Gold is checked against the
	--    engine's own quote; `CanLevyMilitary` already refuses a levy that is
	--    still on cooldown.
	-- ⚠ Each mutation has its OWN flag so the crash can be isolated one variable
	-- at a time. Never re-enable all three at once — that is how three runs were
	-- spent learning only that "something in chooseEnvoy" faults.
	if levyOp ~= nil and cfg.EnvoyLevy ~= false then
		local purse = try(function()
			return math.floor(player:GetTreasury():GetGoldBalance());
		end, 0) or 0;
		for _, minor in ipairs(seen) do
			if levied == nil and minor.ours
				and try(function() return influence:CanLevyMilitary(minor.id); end, false)
			then
				local cost = try(function()
					return influence:GetLevyMilitaryCost(minor.id);
				end, -1) or -1;
				if cost >= 0 and purse >= cost then
					local params = {};
					params[oneParam] = minor.id;
					local ok = pcall(function()
						UI.RequestPlayerOperation(pid, levyOp, params);
					end);
					if ok then levied = minor.id; end
				end
			end
		end
	end

	-- 2. Place every envoy on ONE target: the cheapest city-state to flip that
	--    we do not already hold. Ties go to the one we have most invested in, so
	--    a part-built claim finishes instead of a new one starting.
	local placed, target = 0, nil;
	if giveOp ~= nil and canGive and tokens > 0 and cfg.EnvoyPlace ~= false then
		local best = nil;
		for _, minor in ipairs(seen) do
			if minor.takes and not minor.ours then
				if best == nil or minor.need < best.need
					or (minor.need == best.need and minor.mine > best.mine) then
					best = minor;
				end
			end
		end
		-- Nothing flippable: top up somewhere legal anyway rather than forfeit
		-- the token. A held envoy expires with the game.
		if best == nil then
			for _, minor in ipairs(seen) do
				if minor.takes and best == nil then best = minor; end
			end
		end
		if best ~= nil then
			target = best.id;
			for _ = 1, tokens do
				local params = {};
				params[oneParam] = best.id;
				local ok = pcall(function()
					UI.RequestPlayerOperation(pid, giveOp, params);
				end);
				if not ok then break; end
				placed = placed + 1;
			end
		end
	end

	-- 3. Clear the prompt whatever happened. This is the line that ends the
	--    turn, and skipping it is what left a run wedged for ten minutes.
	--    ⚠ It is also a PRIME SUSPECT for the game-core segfault: it writes a
	--    flag the shipped code only ever writes from the CityStates screen's own
	--    context, and the fault is delayed by 6-9 turns, which fits desynced
	--    bookkeeping better than a bad operation request.
	if cfg.EnvoyConsider ~= false then
		pcall(function()
			if not influence:IsGivingTokensConsidered() then
				influence:SetGivingTokensConsidered(true);
			end
		end);
	end

	envoyTally.placed = envoyTally.placed + placed;
	envoyTally.suzerainties = suzerainties;
	envoyTally.met_minors = #seen;
	if levied ~= nil then envoyTally.levies = envoyTally.levies + 1; end

	-- Both branches counted, because "envoys handled" reads green whether the
	-- agent bought a suzerainty or found nothing and cleared the flag.
	emit("envoy", {
		turn = turn, held = tokens, placed = placed, target = target,
		met_minors = #seen, suzerainties = suzerainties, levied = levied,
	});
	if levied ~= nil then return "levy"; end
	if placed > 0 then return "envoy"; end
	return "envoy_considered";
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
	-- ⚠ COMMEMORATION IS NO LONGER HERE. It was, and across 48 runs it fired
	-- 1763 times — more than every other unanswered prompt combined — so the
	-- empire forfeited a Dedication in every era of every game. `answerBlocker`
	-- now calls `chooseDedication`. Kept as a comment because "listed here" and
	-- "answered" are the two states this table distinguishes, and moving one out
	-- is the interesting event.
	ENDTURN_BLOCKING_GOVERNOR_PROMOTION = true,
	ENDTURN_BLOCKING_GOVERNOR_IDLE = true,
	ENDTURN_BLOCKING_GOVERNOR_OPPORTUNITY = true,
	-- ⚠⚠⚠ GOVERNOR_APPOINTMENT JOINS ITS SIBLINGS, AND THIS ONE IS MEASURED.
	-- Answering it with `chooseGovernor` CRASHES THE GAME CORE. Three runs, three
	-- different maps and civilizations, all with CIVVIS deciding:
	--     civvis-…T111953Z  t38      civvis-…T112719Z  t37      civvis-…T113303Z  t37
	-- Every one died with EXC_BAD_ACCESS at KERN_INVALID_ADDRESS 0x18 on the
	-- **Game Core** thread, and the faulting frames are BYTE-IDENTICAL across all
	-- three: GameCore_XP2.dll +746148, +745916, +2560108, +2565576, +2567456,
	-- +546828. In each run the very last event written is the turn record with
	-- `blocker: ENDTURN_BLOCKING_GOVERNOR_APPOINTMENT, answered: null` — the game
	-- dies in the answer, before the answer can be reported.
	--
	-- ⚠ WHY THIS NEVER SHOWED UP BEFORE: the prompt has to be RAISED first, and it
	-- is raised by holding a governor title. The long heuristic runs
	-- (settler-…T101628Z reached t190) answered it ZERO times — the cheapest-tech,
	-- cheapest-civic ladder never took the civic that grants one. CIVVIS picks
	-- Code of Laws, Craftsmanship and Foreign Trade, so it earns a title around
	-- turn 37 and walks straight into this. That also makes it a strong candidate
	-- for the t44-47 cluster this project recorded as unexplained and wrongly
	-- attributed to envoys: same thread, same null offset, different prompt.
	--
	-- ⚠ THIS COSTS SOMETHING REAL. A governor is +8 loyalty and 56% of runs past
	-- t60 lose a city to revolt, so this is not a free skip — it is the lesser of
	-- two failures, exactly as with `GIVE_INFLUENCE_TOKEN`. `GovernorAppoint` and
	-- `GovernorAssign` re-enable the two mutations SEPARATELY. Do not turn both on
	-- at once: that is precisely what made the envoy crash un-attributable.
	ENDTURN_BLOCKING_GOVERNOR_APPOINTMENT = true,
	-- ⚠⚠ GIVE_INFLUENCE_TOKEN IS BACK HERE, AND THE REASON MATTERS. Answering it
	-- with `chooseEnvoy` CRASHES THE GAME CORE. On one fixed seed (425255), same
	-- flags, same everything:
	--     envoy_events = 0  ->  t92, t106      no crash
	--     envoy_events = 1  ->  t44, t47, t45  EXC_BAD_ACCESS each time
	-- Three fresh SIGSEGVs in `GameCore_XP2.dll` on the `Game Core` thread, 6-9
	-- turns AFTER the single envoy was placed — a delayed fault, so corrupted
	-- state rather than a bad immediate call. Civ 6 does segfault on its own
	-- (there is a pre-envoy crash at t25), but 3-for-3 against 0-for-2 on the
	-- SAME SEED is a controlled comparison, not a coincidence.
	-- Set `EnvoyEnabled` to re-enable and isolate which mutation does it:
	-- `GIVE_INFLUENCE_TOKEN` from a gameplay context, or the
	-- `SetGivingTokensConsidered` write. Until then the known-stable skip stands,
	-- and the ten-minute wedge is the lesser of the two failures.
	ENDTURN_BLOCKING_GIVE_INFLUENCE_TOKEN = true,
	ENDTURN_BLOCKING_CLAIM_GREAT_PERSON = true,
	ENDTURN_BLOCKING_ARTIFACT = true,
	ENDTURN_BLOCKING_EMERGENCY_NEEDS_ATTENTION = true,
	ENDTURN_BLOCKING_WORLD_CONGRESS_LOOK = true,
	ENDTURN_BLOCKING_WORLD_CONGRESS_SESSION = true,
	ENDTURN_BLOCKING_SPY_CHOOSE_ESCAPE_ROUTE = true,
	ENDTURN_BLOCKING_SPY_CHOOSE_DRAGNET_PRIORITY = true,
};

-- Decisions CIVVIS issues orders for, and which the heuristics must therefore not
-- answer over while CIVVIS's reply is still in flight.
--
-- ⚠ Kept as an explicit list rather than "everything". A blocker CIVVIS has no
-- opinion on must be answered immediately or the turn cannot end, and waiting on an
-- answer that will never come is how a run stalls.
local CIVVIS_OWNED_BLOCKERS = {
	ENDTURN_BLOCKING_RESEARCH = true,
	ENDTURN_BLOCKING_CIVIC = true,
	ENDTURN_BLOCKING_PRODUCTION = true,
};

-- Answer the decision the game says it is waiting on. Returning the name of
-- what was answered (rather than a boolean) is what makes a stuck run
-- diagnosable: the log says which blocker recurred, not merely that one did.
local function answerBlocker(player, pid, blocker, turn)
	local name = blockerName(blocker);
	-- ⚠⚠ THE HONEST DENOMINATOR FOR "CIVVIS IS DECIDING". Even on a turn CIVVIS
	-- answered, the game's own end-turn prompts route back into the hand-written
	-- passes below — `chooseResearch`, `driveProduction`, `orderUnits` — because a
	-- blocker must be answered for the turn to end. So a turn can read
	-- `orders_source: civvis` while a heuristic picked the tech.
	--
	-- This counts those. `residual` is what stands between a real measurement and
	-- the failure this project has already shipped twice: a mechanism that reads
	-- connected whether it is driving or not.
	-- ⚠⚠⚠ THIS CONDITION USED TO REQUIRE `awaiting.source == "civvis"`, AND THAT
	-- MADE THE COUNTER READ ZERO FOR THE WHOLE PROJECT.
	--
	-- Blockers are answered from the game-core event loop, which runs BEFORE
	-- `settleTurn` has received CIVVIS's reply and set the source. At that moment the
	-- source is still "pending", so the increment never happened. Measured on run
	-- 233331Z, which reported `residual: NONE` across 233 turns while the event log
	-- shows the heuristics deciding twice:
	--
	--     t9   ENDTURN_BLOCKING_FILL_CIVIC_SLOT  answered "policies+2"
	--     t23  ENDTURN_BLOCKING_PANTHEON         answered "BELIEF_DANCE_OF_THE_AURORA"
	--
	-- Two policy cards and a pantheon chosen by hand-written code on a run reported
	-- as 100% CIVVIS. **This is the exact failure the comment above warns about — a
	-- mechanism that reads connected whether it is driving or not — committed by the
	-- instrument built to detect it.**
	--
	-- The question the counter answers is "did anything other than CIVVIS decide
	-- something on a run where CIVVIS was supposed to decide", and that does not
	-- depend on when in the turn it happened. `source` is recorded alongside instead,
	-- so the timing is still visible without gating the count on it.
	if cfg.CivvisDecides then
		residualAnswers[name] = (residualAnswers[name] or 0) + 1;
		residualAnswers[name .. "@" .. tostring(awaiting.source)] =
			(residualAnswers[name .. "@" .. tostring(awaiting.source)] or 0) + 1;
	end
	-- ★★★★★ WAIT FOR CIVVIS BEFORE ANSWERING A DECISION IT IS ABOUT TO MAKE.
	--
	-- These blockers fire from the game-core event loop, which runs BEFORE
	-- `settleTurn` has CIVVIS's reply. The heuristic therefore answered first, every
	-- time, and CIVVIS's own choice arrived to find the decision already taken.
	--
	-- Measured on run 233331Z. On 37 turns the heuristic named a tech or civic, and
	-- on every one of them CIVVIS sent its own research/civic order the same turn.
	-- They DISAGREED: at t11, t16 and t22 the heuristic answered `TECH_POTTERY`
	-- while CIVVIS wanted `TECH_ASTROLOGY`. Pottery completed at **t24**; Astrology
	-- not until **t59**. The heuristic's pick is what the game actually researched,
	-- on a run reported as 100% CIVVIS.
	--
	-- So: while CIVVIS's answer is still outstanding, decline to answer. Returning
	-- nil leaves the blocker up and the loop retries, which is exactly the pause
	-- needed for the handshake to land. The fallback below still runs once CIVVIS
	-- has answered, failed, or gone stale -- a blocker that is never answered would
	-- hang the turn forever, which is worse than a heuristic answer.
	--
	-- ⚠ Only the decisions CIVVIS actually issues orders for. Blockers it has no
	-- opinion on must still be answered immediately or the turn cannot end.
	if cfg.CivvisDecides and awaiting.source == "pending"
			and CIVVIS_OWNED_BLOCKERS[name]
			and spend("civvis_wait_" .. name, cfg.MaxCivvisWaitPasses or 12) then
		return nil;
	end
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
	elseif name == "ENDTURN_BLOCKING_GOVERNOR_APPOINTMENT" then
		if not spend("governor", cfg.MaxGovernorPasses or 2) then return nil; end
		return chooseGovernor(player, pid);
	elseif name == "ENDTURN_BLOCKING_COMMEMORATION_AVAILABLE" then
		if not spend("dedication", cfg.MaxDedicationPasses or 2) then return nil; end
		return chooseDedication(player, pid);
	elseif name == "ENDTURN_BLOCKING_GIVE_INFLUENCE_TOKEN" then
		-- ⚠ OFF BY DEFAULT because it crashes the game core: 3 SIGSEGVs in 3 runs
		-- on a seed that reached t92/t106 without it. See SOFT_BLOCKERS. Turning
		-- this on is an experiment, not a fix — isolate ONE mutation at a time.
		if not cfg.EnvoyEnabled then return nil; end
		-- One pass is enough: `chooseEnvoy` places every held token and then
		-- sets the considered flag, so a second call has nothing left to do.
		if not spend("envoy", cfg.MaxEnvoyPasses or 1) then return nil; end
		return chooseEnvoy(player, pid, turn);
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
		-- Once per city, not three times: this runs for every city every turn and
		-- each call is three guarded engine reads.
		local loyalNow, loyalRate, loyalFallsTo = cityLoyalty(city);
		local defStrength, defDamage = cityDefence(
			try(function() return city:GetX(); end, -1),
			try(function() return city:GetY(); end, -1));
		-- What this city has ALREADY built. Same reason as `im` in the tiles export: a
		-- city whose buildings are invisible looks empty forever, so CIVVIS keeps
		-- ordering the granary it finished twenty turns ago.
		local built = {};
		local blds = try(function() return city:GetBuildings(); end);
		if blds ~= nil then
			for row in GameInfo.Buildings() do
				if try(function() return blds:HasBuilding(row.Index); end, false) then
					built[#built + 1] = row.BuildingType;
				end
			end
		end
		cities[#cities + 1] = {
			id = try(function() return city:GetID(); end, -1),
			-- The banner name, so the mirror can label the same city the same way.
			-- Without it the reconstruction falls back to CIVVIS's own list for
			-- whatever civilization it assigned, which is how a Persian game came
			-- out as ROME / OSTIA / ANTIUM on the left-hand screen.
			name = try(function() return Locale.Lookup(city:GetName()); end, ""),
			buildings = built,
			x = try(function() return city:GetX(); end, -1),
			y = try(function() return city:GetY(); end, -1),
			pop = try(function() return city:GetPopulation(); end, -1),
			capital = try(function() return city:IsCapital(); end, false),
			-- The NAME, not the hash. See `productionName`: shipping the raw hash
			-- meant the mirror never knew what any city was already building, so
			-- CIVVIS re-decided production every turn blind to work in progress.
			producing = productionName(queue),
			producing_hash = queue,
			food = try(function() return city:GetGrowth():GetFood(); end, -1),
			-- ⚠ Was `GetDistricts():GetDefenseStrength()` — the method on the
			-- collection, which does not exist, so this read -1 for the whole
			-- project's history on every city on the board.
			defense = defStrength,
			damage = defDamage,
			-- ⚠ THE FIELD WHOSE ABSENCE HID THE BIGGEST DEFECT IN THE PROJECT.
			-- 45 runs of telemetry recorded cities peaking and then declining with
			-- no cause attached, because loyalty was never exported. `falls_to` is
			-- the game's own verdict on who the city is about to be lost to.
			loyalty = loyalNow,
			loyalty_per_turn = loyalRate,
			falls_to = loyalFallsTo,
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
			-- ★★★ ALREADY FORTIFIED, WHICH IS WHY FORTIFY WAS BEING REFUSED.
			--
			-- Civilization VI refuses `UNITOPERATION_FORTIFY` on a unit that is
			-- already fortified. CIVVIS's board did not carry the state, so it
			-- re-ordered every turn and the refusal repeated: run 233331Z shows
			-- exactly one FORTIFY refusal per turn from t196 onward, 28 in all.
			--
			-- Harmless on its own -- the unit is already doing what CIVVIS asked --
			-- but it is noise on top of the refusal counters that real defects are
			-- read from, and it is the same shape as the settler and builder loops:
			-- the board did not know, so the order repeated forever.
			fortified = try(function()
				return (unit:GetFortifyTurns() or 0) > 0;
			end, false),
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
					-- ⚠ ONLY CITIES WE HAVE ACTUALLY SEEN.
					--
					-- GetCities():Members() returns every city the rival owns,
					-- including ones this seat has never laid eyes on. Meeting a
					-- civilization does not reveal its empire to a human player,
					-- so a mirror built from the full list would let the
					-- simulator plan against ground the controller never
					-- scouted. Gate on the plot being revealed to us.
					local cx = city:GetX();
					local cy = city:GetY();
					local seen = plotRevealed(pid, cx, cy);
					if seen then
						-- ⚠ THIS is the number that decides which war gate to move,
						-- and it read -1 for the project's whole history: the
						-- method was called on the districts COLLECTION. `WarArmy
						-- = 12` was measured against WALLED cities late in a game;
						-- an unwalled capital at turn 40 is a different problem and
						-- only defence strength tells them apart.
						local theirDef, theirDmg = cityDefence(cx, cy);
						theirCities[#theirCities + 1] = {
							x = cx, y = cy,
							name = try(function()
								return Locale.Lookup(city:GetName());
							end, ""),
							capital = try(function() return city:IsCapital(); end, false),
							-- Defence is on the city banner when the city is
							-- visible, so this is information a human has.
							defense = theirDef,
							damage = theirDmg,
						};
					end
				end
			end);
			-- ⚠ Visible enemy units, so CIVVIS can weigh a field army rather than only
			-- a city's walls. Gated on the plot being visible to us — NOT merely
			-- revealed: a remembered tile does not show who is standing on it now, and
			-- letting the mirror see through fog would make CIVVIS plan against
			-- knowledge this seat has not earned.
			-- ⚠ THE pcall GOES INSIDE THE LOOP. One wrapped around the whole roster walk
			-- is the single defect in this file that hid seven others: the first unit
			-- that throws abandons every remaining unit and reports nothing, so the
			-- export silently truncates and every count read off it is wrong.
			-- ⚠⚠ `Members()` RETURNS AN ITERATOR TRIPLE, NOT A VALUE. Capturing it
			-- through `try` keeps only the first of the three, so the loop called the
			-- aux function with no state and the game threw "Not a valid instance" —
			-- inside `exportState`, which aborted the whole export, so the brain got
			-- NO board and every turn starved. Measured on run civvis-20260730T111537Z.
			--
			-- The `for` statement therefore has to stay whole inside one pcall (a bad
			-- iterator costs this one rival), with a second pcall INSIDE the loop (a
			-- bad unit costs one unit). That inner pcall is the one that matters: a
			-- single pcall around a roster walk is the defect that once hid seven
			-- others in this file.
			local theirUnits = {};
			pcall(function()
				for _, unit in other:GetUnits():Members() do
					pcall(function()
						local ux = unit:GetX();
						local uy = unit:GetY();
						-- `IsVisible`, not `IsRevealed`: a remembered tile does not show
						-- who stands on it now. Confirmed on a player-indexed
						-- `PlayersVisibility` handle, which is the only form that
						-- answers in a gameplay context.
						if PlayersVisibility[pid]:IsVisible(ux, uy) then
							local name = unitTypeName(unit);
							local row = GameInfo.Units[name];
							theirUnits[#theirUnits + 1] = {
								x = ux, y = uy, kind = name,
								hp = 100 - (try(function() return unit:GetDamage(); end, 0) or 0),
								combat = row ~= nil and (row.Combat or 0) or 0,
								ranged = row ~= nil and (row.RangedCombat or 0) or 0,
							};
						end
					end);
				end
			end);
			rivals[#rivals + 1] = {
				player = otherId,
				-- Who they actually are. The reconstruction seats rivals from
				-- CIVVIS's default roster otherwise, so the standings table on the
				-- left named a different set of civilizations than the game.
				civ = try(function()
					return PlayerConfigurations[otherId]:GetCivilizationTypeName();
				end, ""),
				leader = try(function()
					return PlayerConfigurations[otherId]:GetLeaderTypeName();
				end, ""),
				at_war = try(function() return diplomacy:IsAtWarWith(otherId); end, false),
				-- ★★★ THE GAME'S OWN ANSWER TO "MAY WE DECLARE ON THEM". CIVVIS gates a
				-- war on its own diplomatic bookkeeping — it wants a casus belli, and
				-- failing that it denounces and waits five turns for a Formal War. That
				-- bookkeeping does not exist in a reconstruction with no turn
				-- processing, so the wait never ends: measured over 81 replayed turns
				-- with a persistent agent and `strategy = conquest` on 26 of them,
				-- CIVVIS declared war ZERO times. Exporting the real permission lets
				-- the reconstruction offer the action Civilization VI would actually
				-- allow, instead of a CIVVIS rule with no counterpart here.
				can_declare = try(function()
					return diplomacy:CanDeclareWarOn(otherId);
				end, false),
				score = try(function() return other:GetScore(); end, -1),
				-- ★★★★★ THE NUMBER THE WAR DECISION ACTUALLY NEEDS, and it was never
				-- exported. The old veto compared SCORE ratios, and on Settler the
				-- shipped AI always outscores this seat — so "their score is 2.59x ours"
				-- forbade every war for 190 turns while our 19 units stood on ALERT.
				-- Score counts wonders, techs and population; none of them defends a
				-- city. `GetMilitaryStrength` is what the game's own diplomacy ribbon
				-- shows a human (`DiplomacyRibbon.lua:159`), copied rather than recalled.
				military = try(function() return other:GetStats():GetMilitaryStrength(); end, -1),
				cities = theirCities,
				units = theirUnits,
			};
		end
	end

	-- ★★★ WITHOUT THESE, CIVVIS DECIDES IN THE ANCIENT ERA FOREVER. The
	-- reconstruction had no research at all, so `civvis-orders` on the turn-190
	-- board of run 101628Z ordered SLINGERS and `TECH_ASTROLOGY` — reasonable
	-- advice for turn 5, worthless at turn 190 against Pikemen. What a seat knows
	-- is half of what it can decide.
	--
	-- ⚠ Completed only. `CanResearch` would describe what is reachable, which is a
	-- different question and would let CIVVIS build from a tree it does not have.
	local techs, civics = {}, {};
	local ptechs = try(function() return player:GetTechs(); end);
	if ptechs ~= nil then
		for row in GameInfo.Technologies() do
			if try(function() return ptechs:HasTech(row.Index); end, false) then
				techs[#techs + 1] = row.TechnologyType;
			end
		end
	end
	local pculture = try(function() return player:GetCulture(); end);
	if pculture ~= nil then
		for row in GameInfo.Civics() do
			if try(function() return pculture:HasCivic(row.Index); end, false) then
				civics[#civics + 1] = row.CivicType;
			end
		end
	end

	-- ★★★★★ BARBARIANS, WHICH THE RIVAL EXPORT STRUCTURALLY CANNOT SEE.
	--
	-- `rivals` is built from `PlayerManager.GetAliveMajorIDs()`, so barbarians are
	-- absent by construction and can never show `at_war`. That is not a cosmetic gap:
	-- run 233331Z lost Adrianople at t98 with loyalty 100 while "at peace" with every
	-- civilization it had met, and the analysis of HOW cities are lost was made with
	-- an instrument blind to the most likely culprit. `first_war = None` did not mean
	-- nobody attacked us.
	--
	-- ⚠ Only units on plots this seat can SEE, the same rule the rival export uses.
	-- A barbarian camp in the fog is not information a human has.
	local hostiles = {};
	pcall(function()
		local ids = try(function() return PlayerManager.GetAliveBarbarianIDs(); end, {}) or {};
		for _, bid in ipairs(ids) do
			local barb = Players[bid];
			if barb ~= nil then
				pcall(function()
					for _, unit in barb:GetUnits():Members() do
						local ux, uy = unit:GetX(), unit:GetY();
						if plotRevealed(pid, ux, uy) then
							hostiles[#hostiles + 1] = {
								x = ux, y = uy, player = bid,
								type = try(function()
									return GameInfo.Units[unit:GetUnitType()].UnitType;
								end, "?"),
							};
						end
					end
				end);
			end
		end
	end);

	-- ★★★★ WHAT GOVERNMENT WE ARE ALREADY UNDER.
	--
	-- Nothing carried it, so CIVVIS's mirrored player had none — and a player with no
	-- government asks for one. Measured on run `civvis-20260731T052021Z`: **62
	-- `cannot_change_government` refusals in 96 turns**, one every single turn, plus
	-- `already_GOVERNMENT_CHIEFDOM` once the seat did have one. That is a decision
	-- CIVVIS re-makes from scratch every turn against a fact it was never told, and
	-- while it is cheap in orders it is not cheap in belief: policy slots hang off the
	-- government, and CIVVIS is choosing cards for a government it does not know it has.
	local government = try(function()
		local culture = player:GetCulture();
		local index = culture:GetCurrentGovernment();
		if index == nil or index < 0 then return nil; end
		local row = GameInfo.Governments[index];
		return row ~= nil and row.GovernmentType or nil;
	end);
	-- ★★★★ THE PANTHEON WE ALREADY HOLD. Same shape as the government above and the
	-- same consequence: nothing carried it, so CIVVIS's mirrored player had none and
	-- chose one again every turn -- 125 `pantheon` orders in 173 turns of run
	-- civvis-20260731T055749Z, all counted applied, against exactly one pantheon.
	-- Refusing the order stops the waste; telling CIVVIS stops the decision.
	local pantheon = try(function()
		local index = player:GetReligion():GetPantheon();
		if index == nil or index < 0 then return nil; end
		local row = GameInfo.Beliefs[index];
		return row ~= nil and row.BeliefType or nil;
	end);
	-- ★★★★ THE POLICY CARDS ALREADY SLOTTED, and how many slots exist at all.
	--
	-- The third instance of one shape tonight, after the government and the pantheon:
	-- a fact Civilization VI holds, CIVVIS is never told, and therefore re-decides
	-- every turn. Measured on run civvis-20260731T070956Z in 61 turns:
	-- `no_slot_for_POLICY_URBAN_PLANNING` 46, `no_slot_for_POLICY_AGOGE` 27,
	-- `already_POLICY_DISCIPLINE` 23 — CIVVIS asking for cards it already holds and
	-- for slots this government does not have.
	--
	-- ⚠ Slot COUNT matters as much as the cards. A seat under Chiefdom has a
	-- different shape of government than one under Monarchy, and CIVVIS choosing a
	-- military card for a slot that does not exist is not a bad choice, it is an
	-- uninformed one.
	local policies, policy_slots = {}, 0;
	local pcult = try(function() return player:GetCulture(); end);
	if pcult ~= nil then
		policy_slots = try(function() return pcult:GetNumPolicySlots(); end, 0) or 0;
		for i = 0, policy_slots - 1 do
			local index = try(function() return pcult:GetSlotPolicy(i); end, -1);
			if index ~= nil and index >= 0 then
				local row = GameInfo.Policies[index];
				if row ~= nil then policies[#policies + 1] = row.PolicyType; end
			end
		end
	end
	emit("state", {
		turn = turn,
		techs = techs,
		civics = civics,
		government = government,
		pantheon = pantheon,
		policies = policies,
		policy_slots = policy_slots,
		hostiles = hostiles,
		gold = try(function() return math.floor(player:GetTreasury():GetGoldBalance()); end, -1),
		faith = try(function() return math.floor(player:GetReligion():GetFaithBalance()); end, -1),
		science = try(function() return player:GetTechs():GetScienceYield(); end, -1),
		culture = try(function() return player:GetCulture():GetCultureYield(); end, -1),
		score = try(function() return player:GetScore(); end, -1),
		-- Ours, on the same scale as each rival's, so a comparison is possible at all.
		military = try(function() return player:GetStats():GetMilitaryStrength(); end, -1),
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
-- The game's own name for a terrain/feature/resource row index.
--
-- Returns nil for -1 (Civilization VI's "none", used for a tile with no feature
-- or no resource) and for anything that does not resolve. nil is deliberate: a
-- mirror that guessed would put the simulator on ground that does not exist, and
-- an absent field is a hole the reader can see.
local function typeName(table_name, column, index)
	if index == nil or index < 0 then return nil; end
	return try(function()
		local row = GameInfo[table_name][index];
		return row and row[column] or nil;
	end);
end

local function exportTiles(player, pid, turn)
	if cfg.ExportState ~= true then return; end
	local every = cfg.TileExportEvery or 25;
	-- ⚠ TURN 1 MUST EXPORT, whatever the cadence. `turn % 25` is false for turns
	-- 1..24, so CIVVIS spent the whole opening with NO MAP: `civvis-orders` on run
	-- smoke-20260730T105241Z answered "no revealed terrain yet" every turn to turn 9
	-- and would have to turn 24. The opening is where settling and the first army
	-- are decided, so that is precisely the window that cannot be handed to a
	-- fallback. Export on the first turn, then on the cadence.
	if turn > 1 and turn % every ~= 0 then return; end
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
				local revealed = plotRevealed(pid, x, y);
				-- Unrevealed ground is deliberately sent as a hole rather than
				-- as its true terrain: the mirror must not know more than the
				-- seat does, or the simulator would plan on stolen information.
				if revealed then
					index = index + 1;
					chunk[index] = {
						x = x, y = y,
						-- ⚠ NAMES, NOT INDICES. `GetTerrainType` returns a row
						-- index into the game's own Terrains table, and the
						-- CIVVIS vocabulary (tools/civ6_control/vocab.json) is
						-- keyed by TERRAIN_/FEATURE_/RESOURCE_ names. Mapping
						-- index to name on the Rust side would mean guessing the
						-- table's ordering; asking GameInfo here is authoritative
						-- and costs one lookup. A tile whose type does not
						-- resolve sends nil rather than a number that would be
						-- silently misread as a different terrain.
						t = typeName("Terrains", "TerrainType",
						             try(function() return plot:GetTerrainType(); end, -1)),
						f = typeName("Features", "FeatureType",
						             try(function() return plot:GetFeatureType(); end, -1)),
						r = typeName("Resources", "ResourceType",
						             try(function() return plot:GetResourceType(); end, -1)),
						o = try(function() return plot:GetOwner(); end, -1),
						w = try(function() return plot:IsWater(); end, false),
						i = try(function() return plot:IsImpassable(); end, false),
						fw = try(function() return plot:IsFreshWater(); end, false),
						-- ★★★★ WHAT IS ALREADY BUILT HERE. Without it the mirror shows a
						-- permanently unimproved world, so CIVVIS re-orders the same
						-- development every turn and never moves on. Measured on run
						-- civvis-20260730T132504Z: **19 UNIT_BUILDER and 17
						-- BUILDING_GRANARY orders for ONE city**, against one Warrior and
						-- one Slinger — the exact mirror image of the old all-army,
						-- no-economy failure, and just as self-defeating.
						im = typeName("Improvements", "ImprovementType",
						              try(function() return plot:GetImprovementType(); end, -1)),
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

-- ★★★★★ CHANNEL PROBE — is there ANY way for a decision to reach a running game?
--
-- The architecture asked for is "CIVVIS decides, this mod actuates". The outbound
-- half is proven and live (`Automation.Log` -> `watch.py`, one JSON object per
-- turn). The inbound half is not proven at all: `io` is absent from the sandbox,
-- FireTuner acknowledged seven framings and executed none of them, and config is
-- baked in at install time — which is a channel between GAMES, not within one.
-- Every decision the agent makes today is therefore a heuristic hard-coded here,
-- and that is exactly how "veto war when their SCORE exceeds 1.3x ours" survived
-- 190 turns of never fighting.
--
-- ⚠ This asks rather than assumes. Every API recalled from memory in this project
-- has been wrong on this build at least once — `plot:IsRevealed()` took no player
-- argument, `GetMovesRemaining` did not exist, `GetDefenseStrength` was on the
-- wrong object — and each failed SILENTLY. So each candidate is asked once per
-- turn and its answer is emitted verbatim.
--
-- ⚠⚠ EXISTENCE IS NOT A CHANNEL. A candidate counts only when the harness changes
-- the value from OUTSIDE and the emitted value FOLLOWS. `type()` reading "function"
-- proves the name resolves, nothing more — the same mistake as `applied = true` on
-- a build request the engine ignored. `tools/civ6_control/probe_channel.py` writes
-- a fresh nonce into every sink each turn; the channel is whichever field reports
-- that nonce back.
-- Run a query and describe the OUTCOME as text, keeping the three cases apart:
-- a throw (with its message), a nil return, and rows. Collapsing them is how a
-- silent gate happens.
local function sqlText(fn)
	local ok, res = pcall(fn);
	if not ok then return "ERR:" .. tostring(res):sub(1, 160); end
	if res == nil then return "nil"; end
	if type(res) ~= "table" then return "scalar:" .. tostring(res); end
	local parts = {};
	for _, row in ipairs(res) do
		if type(row) == "table" then
			for k, v in pairs(row) do
				parts[#parts + 1] = tostring(k) .. "=" .. tostring(v);
				if #parts >= 8 then break; end
			end
		else
			parts[#parts + 1] = tostring(row);
		end
		if #parts >= 8 then break; end
	end
	if #parts == 0 then return "rows:0"; end
	return table.concat(parts, ";");
end

local function probeChannels(turn)
	local report = { turn = turn };
	-- Bare globals, not `_G[name]`: each Civ 6 UI context runs in its own
	-- environment, and reading through `_G` is what made all 21 autoclose contexts
	-- report unarmed. `type` on an undefined global is "nil" and cannot throw.
	report.t_ModUserData = type(ModUserData);
	report.t_DB = type(DB);
	report.t_Options = type(Options);
	report.t_UserConfiguration = type(UserConfiguration);
	report.t_GameConfiguration = type(GameConfiguration);
	report.t_UIManager = type(UIManager);
	report.t_Network = type(Network);
	report.t_Modding = type(Modding);
	report.t_io = type(io);
	report.t_loadfile = type(loadfile);
	report.t_dofile = type(dofile);
	-- Reads. Each is guarded: indexing a nil global throws, and a throw here must
	-- cost this one field rather than the turn.
	report.clip = try(function() return tostring(UIManager:GetClipboardString()); end, nil);
	report.clip_fn = try(function() return type(UIManager.GetClipboardString); end, "absent");
	report.appopt = try(function() return tostring(Options.GetAppOption("Civvis", "Decision")); end, nil);
	report.useropt = try(function() return tostring(Options.GetUserOption("Civvis", "Decision")); end, nil);
	report.gamecfg = try(function() return tostring(GameConfiguration.GetValue("CIVVIS_DECISION")); end, nil);
	report.usercfg = try(function() return tostring(UserConfiguration.GetValue("CIVVIS_DECISION")); end, nil);
	report.mud = try(function() return tostring(ModUserData.GetValue("civvis_decision")); end, nil);
	report.db_q = try(function() return type(DB.Query); end, "absent");
	report.db_cfgq = try(function() return type(DB.ConfigurationQuery); end, "absent");
	-- ★ `DB.Query` and `DB.ConfigurationQuery` BOTH resolve to functions on this
	-- build — measured, not recalled. That makes SQL the only candidate left after
	-- the first probe killed the others: `ModUserData` is nil, `io`/`loadfile`/
	-- `dofile` are nil, and `UIManager` exists but has only the clipboard SETTER,
	-- so nothing can be handed in through it.
	--
	-- The question this asks is whether SQL can reach a file OUTSIDE the game.
	-- `ATTACH DATABASE` would do it, and it is much better than writing into the
	-- game's own `DebugGameplay.sqlite`: a database this process owns cannot be
	-- clobbered when the game rebuilds its cache, and cannot block a turn on a
	-- lock the game is holding.
	--
	-- ⚠ The error TEXT is the payload here. "no such table" means SQL ran and the
	-- schema is missing; "not authorized" means the sandbox forbids ATTACH; a nil
	-- return means the call itself failed. Those are three different projects, and
	-- a bare `try(..., nil)` collapses them into one.
	report.db_master = sqlText(function()
		return DB.Query("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name LIMIT 4");
	end);
	report.db_attach = sqlText(function()
		return DB.Query("ATTACH DATABASE '" .. tostring(cfg.OrdersDb or "") .. "' AS civvis");
	end);
	report.db_orders = sqlText(function()
		return DB.Query("SELECT turn, payload FROM civvis.orders WHERE id = 1");
	end);
	report.cfg_master = sqlText(function()
		return DB.ConfigurationQuery("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name LIMIT 3");
	end);
	emit("channel", report);
end

-- ------------------------------------------------------------- CIVVIS orders
--
-- ★★★★★ THE SEAT'S DECISIONS COME FROM CIVVIS, NOT FROM THIS FILE.
--
-- Everything above this line that *chooses* — which tech, what to build, whether
-- to fight — is a hand-written heuristic, and hand-written heuristics are how
-- "veto war when their SCORE exceeds 1.3x ours" survived 190 turns of never
-- fighting on a difficulty where the shipped AI always outscores this seat. CIVVIS
-- has a measured decision layer; this mod's job is to actuate it.
--
-- The channel is `DB.Query("ATTACH DATABASE <file the harness owns>")`, measured
-- working on run `sqlprobe-20260730T103836Z`: 23 distinct payloads followed a nonce
-- an outside process rewrote every second. See the memory note for what is
-- measured DEAD (ModUserData, io, the clipboard getter).
--
-- ⚠ NO JSON PARSER. The encoder here is write-only and Lua 5.1 has no decoder;
-- writing one is a needless risk when SQLite already structures data. Orders
-- arrive as ROWS with typed columns, so there is nothing to parse and a malformed
-- payload cannot take a turn down.
--
-- ⚠⚠ THE BRAIN MUST NOT BE ABLE TO WEDGE A TURN. Three regressions in this project
-- came from handing a mechanism authority with no floor for the case where it is
-- wrong. So: the turn WAITS for orders, but only for `OrdersWaitTicks`; past that
-- the built-in heuristics run and the turn is recorded as `fallback`. A brain that
-- crashes costs quality, never progress.
local ordersAttached = nil;
-- ⚠ `awaiting` and `residualAnswers` are declared at the top of the file, not here.
-- A second `local awaiting` at this point would shadow it for everything below,
-- so `answerBlocker` above would read one table while `settleTurn` wrote another
-- and the residual counter would silently always be empty.

local function sqlSafe(s)
	return tostring(s or ""):gsub("'", ""):gsub("\\", "");
end

-- ATTACH once per game. A second ATTACH under the same name errors, and that
-- error is not a failure worth reporting every turn.
local function attachOrders()
	if ordersAttached ~= nil then return ordersAttached; end
	local path = cfg.OrdersDb;
	if path == nil or path == "" then
		ordersAttached = false;
		return false;
	end
	local ok = pcall(function()
		DB.Query("ATTACH DATABASE '" .. sqlSafe(path) .. "' AS civvis");
	end);
	-- Prove the attach by reading through it, not by the absence of a throw:
	-- `applied = true` on a request the engine discarded is this project's
	-- signature bug.
	local reads = false;
	if ok then
		reads = pcall(function()
			DB.Query("SELECT count(*) AS n FROM civvis.ready");
		end);
	end
	ordersAttached = ok and reads;
	emit("orders_channel", { attached = ordersAttached, path = tostring(path) });
	return ordersAttached;
end

-- Has the brain finished writing this turn? `ready` is written last, so a
-- partially written turn is never actuated.
local function ordersReady(turn)
	if not attachOrders() then return nil; end
	local count = nil;
	pcall(function()
		local rows = DB.Query(string.format(
			"SELECT count AS n FROM civvis.ready WHERE run = '%s' AND turn = %d LIMIT 1",
			sqlSafe(cfg.RunTag), turn));
		for _, row in ipairs(rows) do count = row.n; end
	end);
	return count;
end

local function fetchOrders(turn)
	local out = {};
	pcall(function()
		local rows = DB.Query(string.format(
			"SELECT seq, kind, subject, verb, x, y FROM civvis.orders "
			.. "WHERE run = '%s' AND turn = %d ORDER BY seq",
			sqlSafe(cfg.RunTag), turn));
		for _, row in ipairs(rows) do out[#out + 1] = row; end
	end);
	return out;
end

-- The newest turn CIVVIS has answered at all, at or before `turn`.
--
-- ⚠ WHY STALE ORDERS BEAT THE HEURISTICS. The round trip is board -> log ->
-- `watch.py` -> brain -> SQLite, and it is not instant. When turn N's answer has
-- not landed, the choice is between CIVVIS's answer to turn N-1 and the
-- hand-written ladder — and the ladder is what spent 190 turns refusing to fight.
-- A unit told to walk somewhere it already stands is refused harmlessly and
-- counted; a wrong doctrine is not.
local function newestAnsweredTurn(turn)
	local best = nil;
	pcall(function()
		local rows = DB.Query(string.format(
			"SELECT max(turn) AS t FROM civvis.ready WHERE run = '%s' AND turn <= %d",
			sqlSafe(cfg.RunTag), turn));
		for _, row in ipairs(rows) do best = row.t; end
	end);
	return best;
end

-- Actuate one CIVVIS order. Every branch goes through the same
-- `canOperate`-then-request discipline the rest of this file uses, so a refused
-- order is reported as refused rather than counted as done.
-- ⚠⚠⚠ RE-RESOLVE THE SUBJECT, NEVER CACHE IT ACROSS ORDERS.
--
-- A cached handle can outlive the thing it points at, and CIVVIS issues several
-- orders per turn for the same units:
--
--   * a melee MOVE_TO onto a defended plot IS the attack, so the unit can DIE
--     partway through this turn's list;
--   * a settler that founds a city is CONSUMED;
--   * two MOVE_TOs for one unit are normal, since a unit has several movement points.
--
-- Building `units[id]` once and then issuing operations from it therefore hands
-- `UnitManager.RequestOperation` a freed unit, and the game does not survive that:
-- run civvis-20260730T111953Z died at turn 38 with SIGSEGV at
-- KERN_INVALID_ADDRESS 0x18 on the **Game Core** thread in `GameCore_XP2.dll` —
-- the exact signature of the t44-47 crash cluster this project had recorded as
-- unexplained, and which persisted with envoy code disabled.
--
-- `UnitManager.GetUnit(pid, id)` and `GetCities():FindID(id)` are the shipped UI's
-- own lookups (`UnitPanel.lua`, `CityPanel.lua`). Asking again costs one call and
-- returns nil for something that is gone, which is a refusal we can count.
-- ★★★ RESOLVE A TYPE NAME AGAINST THE GAME'S OWN DATABASE, LENIENTLY.
--
-- CIVVIS and Civilization VI do not spell every node the same way: CIVVIS's `wheel`
-- is Civ 6's `TECH_THE_WHEEL`, and a mechanical `TECH_` + upper-case gives
-- `TECH_WHEEL`, which does not exist. Measured on run civvis-20260730T120107Z:
-- **102 refused research orders**, all of them that one name, against 120 that landed
-- — so nearly half of CIVVIS's research decisions were silently discarded and the
-- seat researched whatever the blocker path happened to pick.
--
-- ⚠ Resolved HERE rather than guessed in the translator, because this is the only
-- place that can ask the shipped database whether a name exists. The alternative
-- spelling is tried and the name that actually resolved is reported, so a future
-- mismatch shows up as a named miss instead of a silent default.
local function resolveType(table_, name)
	if name == nil or name == "" then return nil, "empty"; end
	if table_[name] ~= nil then return table_[name], name; end
	local prefix, rest = string.match(name, "^([A-Z]+_)(.+)$");
	if prefix ~= nil then
		local alt = prefix .. "THE_" .. rest;
		if table_[alt] ~= nil then return table_[alt], alt; end
	end
	-- ⚠ CIVVIS'S NAMES CARRY WORDS CIV 6 DROPS. `government_plaza` is Civ 6's
	-- `DISTRICT_GOVERNMENT`; `theater_square` is `DISTRICT_THEATER`. Measured on run
	-- civvis-20260730T142203Z: **117 refused `DISTRICT_GOVERNMENT_PLAZA`** and 8
	-- `DISTRICT_THEATER_SQUARE` — 125 production orders thrown away on spelling.
	--
	-- Trimming trailing words is general where a hand-written pair list is not, and it
	-- only ever SHORTENS, so it cannot invent a name: whatever it lands on had to
	-- already exist in the game's own table.
	local trimmed = name;
	while true do
		local shorter = string.match(trimmed, "^(.+)_[^_]+$");
		if shorter == nil or shorter == prefix or #shorter <= #(prefix or "") then
			break;
		end
		if table_[shorter] ~= nil then return table_[shorter], shorter; end
		trimmed = shorter;
	end
	return nil, name;
end

local function liveUnit(pid, id)
	return try(function() return UnitManager.GetUnit(pid, id); end);
end

local function liveCity(player, id)
	return try(function() return player:GetCities():FindID(id); end);
end

local function applyOrder(player, pid, row, turn)
	local kind = tostring(row.kind or "");
	local verb = tostring(row.verb or "");
	local subject = tonumber(row.subject) or -1;
	local x, y = tonumber(row.x), tonumber(row.y);

	if kind == "war" then
		local diplomacy = try(function() return player:GetDiplomacy(); end);
		if diplomacy == nil then return false, "no_diplomacy"; end
		if try(function() return diplomacy:IsAtWarWith(subject); end, false) then
			return false, "already_at_war";
		end
		if not try(function() return diplomacy:CanDeclareWarOn(subject); end, true) then
			return false, "cannot_declare";
		end
		local params = {};
		params[PlayerOperations.PARAM_PLAYER_ONE] = pid;
		params[PlayerOperations.PARAM_PLAYER_TWO] = subject;
		local ok = pcall(function()
			UI.RequestPlayerOperation(pid, PlayerOperations.DIPLOMACY_DECLARE_WAR, params);
		end);
		if ok then
			warDeclared[subject] = true;
			-- ⚠ THE FIRES-CHECK FOR THE DECISION THAT DECIDES THE GAME. `declareWar`
			-- emits this on the built-in path; without it here, a CIVVIS-declared war
			-- appeared only as an anonymous `by.war = 1` count and the run's `war`
			-- field stayed null — which is exactly how "the army never fights" was
			-- misdiagnosed for the whole history of this project.
			emit("war", { turn = turn, target = subject, source = "civvis" });
		end
		return ok, ok and "declared" or "throw";
	end

	-- ★★★★★ CIVVIS'S OWN POLICY, GOVERNMENT AND PANTHEON CHOICES.
	--
	-- These had NO arm at all: CIVVIS issued `SlotPolicy` on every turn from t80 to
	-- t233 of run 233331Z -- six a turn by the end -- plus `Government` every turn
	-- from t40, and the bridge counted them all as `skipped` and threw them away.
	-- The mod then filled the slots with its own "first unlocked card that fits"
	-- heuristic. That is the exact arrangement this project was told to remove: the
	-- harness deciding while CIVVIS's decision is discarded.
	--
	-- Policy cards are not marginal here -- already measured as mattering
	-- (p=0.0023).
	if kind == "policy" then
		local culture = try(function() return player:GetCulture(); end);
		if culture == nil then return false, "no_culture"; end
		local row2, resolved = resolveType(GameInfo.Policies, verb);
		if row2 == nil then return false, "unknown_" .. verb; end
		if try(function() return culture:IsPolicyObsolete(row2.Hash); end, false) then
			return false, "obsolete_" .. resolved;
		end
		if not try(function() return culture:IsPolicyUnlocked(row2.Hash); end, false) then
			return false, "locked_" .. resolved;
		end
		-- Find a slot this card fits that is not already holding it. A wildcard slot
		-- takes anything; a typed slot must match.
		local slots = try(function() return culture:GetNumPolicySlots(); end, 0) or 0;
		local target;
		for i = 0, slots - 1 do
			local held = try(function() return culture:GetSlotPolicy(i); end, -1);
			if held == row2.Index then return false, "already_" .. resolved; end
			if target == nil and (held == nil or held < 0) then
				local slotName = try(function()
					local slotId = culture:GetSlotType(i);
					return GameInfo.GovernmentSlots[slotId].GovernmentSlotType;
				end);
				if slotName == nil or slotName == "SLOT_WILDCARD"
						or slotName == row2.GovernmentSlotType then
					target = i;
				end
			end
		end
		if target == nil then return false, "no_slot_for_" .. resolved; end
		local addList = {};
		addList[target] = row2.Hash;
		local ok = pcall(function() culture:RequestPolicyChanges({}, addList); end);
		return ok, ok and resolved or "throw";
	end

	if kind == "government" then
		local culture = try(function() return player:GetCulture(); end);
		if culture == nil then return false, "no_culture"; end
		if not try(function() return culture:CanChangeGovernmentAtAll(); end, false) then
			return false, "cannot_change_government";
		end
		local row2, resolved = resolveType(GameInfo.Governments, verb);
		if row2 == nil then return false, "unknown_" .. verb; end
		if not try(function() return culture:IsGovernmentUnlocked(row2.Hash); end, false) then
			return false, "locked_" .. resolved;
		end
		if try(function() return culture:GetCurrentGovernment(); end, -1) == row2.Index then
			return false, "already_" .. resolved;
		end
		local ok = pcall(function() culture:RequestChangeGovernment(row2.Hash); end);
		-- Tell the game the prompt has been dealt with either way, or it re-raises
		-- the blocker every turn.
		pcall(function() culture:SetGovernmentChangeConsidered(true); end);
		return ok, ok and resolved or "throw";
	end

	if kind == "pantheon" then
		local row2, resolved = resolveType(GameInfo.Beliefs, verb);
		if row2 == nil then return false, "unknown_" .. verb; end
		-- ★★★★★ WE ALREADY HAVE ONE, AND `pcall` WOULD NOT HAVE SAID SO.
		--
		-- `IsInSomePantheon` asks whether SOMEBODY has taken that belief. It does not
		-- ask whether WE have already founded a pantheon — and once we have, the
		-- request below does nothing while `pcall` returns true, so the order was
		-- counted applied. Measured on run `civvis-20260731T055749Z`: **125 `pantheon`
		-- orders**, every one recorded as applied, against exactly one pantheon.
		--
		-- The same trap as `PARAM_INSERT_MODE`, the silent purchase and the governor
		-- answer, in a file that warns about it three times: ASK THE ENGINE WHETHER
		-- THE THING CAN START.
		if try(function() return player:GetReligion():GetPantheon(); end, -1) >= 0 then
			return false, "pantheon_already_founded";
		end
		if try(function() return Game.GetReligion():IsInSomePantheon(row2.Index); end, true) then
			return false, "taken_" .. resolved;
		end
		local params = {};
		params[PlayerOperations.PARAM_BELIEF_TYPE] = row2.Hash;
		local ok = pcall(function()
			UI.RequestPlayerOperation(pid, PlayerOperations.FOUND_PANTHEON, params);
		end);
		return ok, ok and resolved or "throw";
	end

	if kind == "research" or kind == "civic" then
		local isTech = kind == "research";
		local table_ = isTech and GameInfo.Technologies or GameInfo.Civics;
		local row2, resolved = resolveType(table_, verb);
		if row2 == nil then return false, "unknown_" .. verb; end
		verb = resolved;
		local progress = try(function()
			return isTech and player:GetTechs() or player:GetCulture();
		end);
		if progress == nil then return false, "no_progress"; end
		-- ⚠ `GetResearchPath` is what makes a DISTANT node reachable: asking for a
		-- tech whose prerequisites are unmet is refused outright, while its path
		-- queues the prerequisites. CIVVIS names the destination; the game routes.
		local params = {};
		if isTech then
			params[PlayerOperations.PARAM_TECH_TYPE] =
				try(function() return progress:GetResearchPath(row2.Hash); end) or row2.Hash;
		else
			params[PlayerOperations.PARAM_CIVIC_TYPE] =
				try(function() return progress:GetCivicPath(row2.Hash); end) or row2.Hash;
		end
		params[PlayerOperations.PARAM_INSERT_MODE] = PlayerOperations.VALUE_EXCLUSIVE;
		local op = isTech and PlayerOperations.RESEARCH or PlayerOperations.PROGRESS_CIVIC;
		local ok = pcall(function() UI.RequestPlayerOperation(pid, op, params); end);
		return ok, ok and verb or "throw";
	end

	if kind == "produce" then
		local city = liveCity(player, subject);
		if city == nil then return false, "no_city"; end
		-- ⚠ `GameInfo.Types`, NOT the per-kind tables. `buildParams` switches on
		-- `row.Kind`, and only the `Types` table carries that column — rows from
		-- `GameInfo.Units`/`Buildings`/`Districts` have no `Kind`, so `buildParams`
		-- returned nil and EVERY produce order was refused `no_params`. Measured on
		-- run civvis-20260730T111537Z: 9 refusals in 10 turns, all of them this.
		-- The built-in ladder reads `GameInfo.Types[name]` for exactly this reason.
		local row2, resolved = resolveType(GameInfo.Types, verb);
		if row2 == nil then return false, "unknown_" .. verb; end
		verb = resolved;
		local params = buildParams(row2);
		if params == nil then return false, "no_params"; end
		-- ★★★★ DO NOT RE-ISSUE WHAT THE CITY IS ALREADY BUILDING.
		--
		-- Every produce order carries `VALUE_REPLACE_AT`, so re-sending one REPLACES the
		-- queue. CIVVIS re-decides production every turn from a board that cannot show
		-- partial progress, so it alternates: measured on run civvis-20260730T134705Z,
		-- **11 UNIT_BUILDER and 10 UNIT_GALLEY orders in 20 turns** in one city — a city
		-- that swaps its build every turn finishes neither.
		--
		-- Comparing against the queue's current item is an actuation concern, not a
		-- decision: CIVVIS still chooses, and a genuine change of mind still lands.
		local current = try(function()
			local q = city:GetBuildQueue();
			return q and q:GetCurrentProductionTypeHash() or 0;
		end, 0);
		if current ~= 0 and row2.Hash ~= nil and current == row2.Hash then
			return true, "already_building";
		end
		local ok = pcall(function()
			CityManager.RequestOperation(city, CityOperationTypes.BUILD, params);
		end);
		return ok, ok and verb or "throw";
	end

	-- ★★★★ BUY. CIVVIS spends gold and the seat sat on hundreds of it: 122 `Buy` and
	-- 224 `BuyBuilding` actions dropped in one 81-turn stretch. Worse than the waste,
	-- a purchase CIVVIS makes in its model and the bridge discards leaves it believing
	-- it owns a unit that does not exist — the same phantom that stopped it building
	-- settlers for a whole game.
	--
	-- Pattern copied from the shipped `ProductionPanel.lua`: a PURCHASE **command**,
	-- not a BUILD operation, with the item hash and the yield to pay from.
	if kind == "purchase" then
		local city = liveCity(player, subject);
		if city == nil then return false, "no_city"; end
		local row2, resolved = resolveType(GameInfo.Types, verb);
		if row2 == nil then return false, "unknown_" .. verb; end
		local params = {};
		if row2.Kind == "KIND_UNIT" then
			params[CityCommandTypes.PARAM_UNIT_TYPE] = row2.Hash;
			params[CityCommandTypes.PARAM_MILITARY_FORMATION_TYPE] =
				MilitaryFormationTypes.STANDARD_MILITARY_FORMATION;
		elseif row2.Kind == "KIND_BUILDING" then
			params[CityCommandTypes.PARAM_BUILDING_TYPE] = row2.Hash;
		elseif row2.Kind == "KIND_DISTRICT" then
			params[CityCommandTypes.PARAM_DISTRICT_TYPE] = row2.Hash;
		else
			return false, "no_params";
		end
		local gold = try(function() return GameInfo.Yields["YIELD_GOLD"].Index; end);
		if gold == nil then return false, "no_yield"; end
		params[CityCommandTypes.PARAM_YIELD_TYPE] = gold;
		-- ⚠⚠ ASK BEFORE CLAIMING. `pcall` succeeding means the call did not throw, not
		-- that the city bought anything — the trap this file documents three times and
		-- which I walked into again here. Run civvis-20260730T173235Z issued purchases
		-- for 238 turns and finished with **1589 unspent gold**, every one reported as
		-- applied.
		--
		-- ★ AND IT COSTS MORE THAN THE GOLD. CIVVIS's model spawns the bought unit
		-- immediately, so a purchase that fails in the real game leaves a PHANTOM
		-- settler — `phantom=[15:settler]` — and `advanced_units` then computes
		-- `decline_settlers = counts.settlers > 0` and refuses to BUILD one. Every turn.
		-- That is the phantom-settler failure returning through a different door, and it
		-- is why that run held ONE city to turn 238.
		local canBuy = false;
		local okCan = pcall(function()
			canBuy = CityManager.CanStartCommand(city, CityCommandTypes.PURCHASE,
			                                    params, true) == true;
		end);
		if not (okCan and canBuy) then return false, "cannot_buy_" .. resolved; end
		local ok = pcall(function()
			CityManager.RequestCommand(city, CityCommandTypes.PURCHASE, params);
		end);
		return ok, ok and resolved or "throw";
	end

	if kind == "unit" then
		-- Asked for fresh, right here: the previous order in this very list may have
		-- killed or consumed it.
		local unit = liveUnit(pid, subject);
		if unit == nil then return false, "unit_gone"; end
		-- FOUND_CITY, MOVE_TO and RANGE_ATTACK are the three that decide a game.
		-- ⚠ There is NO attack operation on this build — the resolved list is only
		-- MOVE_TO and RANGE_ATTACK — so a melee strike IS a MOVE_TO onto the
		-- defended plot. CIVVIS's `Attack` therefore translates to MOVE_TO, and
		-- that is not a workaround: it is how Civilization VI resolves it.
		if verb == "FOUND_CITY" then
			local placed = operate(unit, OP["UNITOPERATION_FOUND_CITY"], {});
			-- ★★★★★ ASK THE ENGINE WHY, DO NOT INFER IT.
			--
			-- Peak city count is 2 in every run of the ladder from t88 to t233,
			-- and `found` refusals were being counted without a cause: run
			-- 230605Z refused 18 of them while re-choosing the SAME tile (18,29)
			-- at t20, t33 and t79. Two theories were live -- a rival city hidden
			-- in the fog, or the settler not standing where the order aims -- and
			-- nothing in the export could separate them.
			--
			-- Civilization VI will say. `CanStartOperation` takes a results flag
			-- and returns a reason table; the mod was calling it without one and
			-- discarding exactly the answer it needed. Only on the failure path,
			-- so it costs nothing on a normal found.
			if not placed then
				local why = "unknown";
				pcall(function()
					local ok, results = UnitManager.CanStartOperation(
						unit, OP["UNITOPERATION_FOUND_CITY"], nil, {}, true);
					if results ~= nil then
						local parts = {};
						for key, value in pairs(results) do
							parts[#parts + 1] = tostring(key) .. "=" .. tostring(value);
						end
						if #parts > 0 then why = table.concat(parts, ","); end
					elseif ok ~= nil then
						why = "can_start=" .. tostring(ok);
					end
				end);
				emit("found_refused", { turn = turn, unit = subject, why = why,
				                        x = unit:GetX(), y = unit:GetY() });
			end
			return placed, "found";
		end
		if verb == "MOVE_TO" or verb == "ATTACK" then
			if x == nil or y == nil then return false, "no_dest"; end
			local params = {};
			params[UnitOperationTypes.PARAM_X] = x;
			params[UnitOperationTypes.PARAM_Y] = y;
			return operate(unit, OP["UNITOPERATION_MOVE_TO"], params), verb;
		end
		if verb == "RANGE_ATTACK" then
			if x == nil or y == nil then return false, "no_dest"; end
			local params = {};
			params[UnitOperationTypes.PARAM_X] = x;
			params[UnitOperationTypes.PARAM_Y] = y;
			return operate(unit, OP["UNITOPERATION_RANGE_ATTACK"], params), verb;
		end
		-- ★★★★★ IMPROVE — the order whose absence made CIVVIS build builders forever.
		--
		-- CIVVIS tells a builder to improve the tile it stands on; that order was not
		-- translated, so no tile was ever improved, so the mirror kept showing an
		-- undeveloped empire and CIVVIS kept ordering another builder. Measured on run
		-- civvis-20260730T134000Z: **22 `UNIT_BUILDER` orders by turn 31** with one
		-- military unit alive. Exporting improvements was necessary but not sufficient —
		-- there were none to export.
		--
		-- Params copied from the shipped `UnitPanel.lua`: the unit's OWN tile plus the
		-- improvement hash. `x`/`y` carry the target and default to where it stands.
		if verb == "IMPROVE" or string.sub(verb, 1, 8) == "IMPROVE:" then
			-- The improvement name rides in the verb as `IMPROVE:IMPROVEMENT_FARM`,
			-- because the order row has no spare column for it. No name means "build
			-- whatever this tile allows", which is what the shipped button does.
			local wanted = string.match(verb, "^IMPROVE:(.+)$");
			local row2 = wanted ~= nil and GameInfo.Improvements[wanted] or nil;
			local params = {};
			params[UnitOperationTypes.PARAM_X] = x or try(function() return unit:GetX(); end, -1);
			params[UnitOperationTypes.PARAM_Y] = y or try(function() return unit:GetY(); end, -1);
			-- Without a named improvement the engine picks what the tile allows, which
			-- is what a human gets from the "build improvement" button.
			if row2 ~= nil then
				params[UnitOperationTypes.PARAM_IMPROVEMENT_TYPE] = row2.Hash;
			end
			if operate(unit, OP["UNITOPERATION_BUILD_IMPROVEMENT"], params) then
				return true, wanted or "IMPROVE";
			end
			-- ★★★★ FALL BACK TO WHATEVER THIS TILE ALLOWS.
			--
			-- CIVVIS names the improvement from ITS terrain model, and the two rulesets
			-- do not agree tile for tile — measured on run civvis-20260730T135149Z: 34
			-- refused improvements by turn 42, **22 of them `IMPROVEMENT_MINE`**. A
			-- refused improvement means the tile stays bare, the mirror keeps reporting
			-- an undeveloped empire, and CIVVIS orders another builder: seven builders
			-- alive against an army of one.
			--
			-- The DECISION worth keeping is "improve this tile" — which tile is CIVVIS's
			-- call. Which improvement is legal there is the game's call, and dropping the
			-- name asks it exactly that, the same as the shipped build button.
			if row2 ~= nil then
				params[UnitOperationTypes.PARAM_IMPROVEMENT_TYPE] = nil;
				if operate(unit, OP["UNITOPERATION_BUILD_IMPROVEMENT"], params) then
					return true, "IMPROVE_ANY";
				end
			end
			-- ★★★ AND IF THIS TILE CANNOT BE IMPROVED AT ALL, AUTOMATE THE BUILDER.
			--
			-- Dropping the name was not enough: 26 `IMPROVEMENT_MINE` refusals survived
			-- it, so Civilization VI is refusing the tile itself — unowned, already
			-- improved, or simply not improvable. A builder that cannot act stands there
			-- while the mirror keeps reporting an undeveloped empire and CIVVIS orders
			-- another builder.
			--
			-- Same shape as `ExploreUnassigned`, and the same honesty applies: this is a
			-- POLICY. It does not pick a tile or an improvement — Civ 6's own builder
			-- automation does — and it only ever runs where CIVVIS's own choice was
			-- refused. Reported as `IMPROVE_AUTOMATED` so it is never counted as CIVVIS's.
			if cfg.AutomateStuckBuilders ~= false
					and commandUnit(unit, CMD["UNITCOMMAND_AUTOMATE"], {}) then
				return true, "IMPROVE_AUTOMATED";
			end
			-- ★★★★ TELL CIVVIS THE TILE IS DEAD, or it will order another builder.
			--
			-- All three rungs have now failed: the named improvement, any improvement,
			-- and automation. Civilization VI is refusing the TILE, and nothing carried
			-- that back — so CIVVIS re-derived the same target from the same board and
			-- re-sent it. Measured: 311 `IMPROVEMENT_MINE` refusals in one run, 51 and
			-- 31 in others, against an empire the mirror kept reporting as undeveloped,
			-- which is precisely why it kept building builders.
			--
			-- Same cure as the settler loop: record the ground, let CIVVIS's own
			-- planner route around it. See `Game::blocked_improvement_sites`.
			emit("improve_refused", { turn = turn, unit = subject,
			                          want = wanted or "IMPROVE",
			                          x = unit:GetX(), y = unit:GetY() });
			return false, wanted or "IMPROVE";
		end
		-- ★★★ SEND THE TRADER SOMEWHERE. Untranslated until now, so a trader stood
		-- where it was built for the whole game: `civ6_watchdogs.py` names one in every
		-- run, motionless for 114 turns in the longest case, with its production and
		-- its maintenance already paid.
		--
		-- ⚠ UNTESTED AGAINST A LIVE GAME when it was written, which is why it asks the
		-- engine rather than assuming: a refused operation is counted as a refusal
		-- under this verb, so the tally says plainly whether it works. If
		-- `UNITOPERATION_MAKE_TRADE_ROUTE` does not resolve on this build, `OP[...]` is
		-- nil and `operate` refuses — no throw, no silent success.
		if verb == "TRADE_ROUTE" then
			if x == nil or y == nil then return false, "no_dest"; end
			local params = {};
			params[UnitOperationTypes.PARAM_X] = x;
			params[UnitOperationTypes.PARAM_Y] = y;
			return operate(unit, OP["UNITOPERATION_MAKE_TRADE_ROUTE"], params), verb;
		end
		if verb == "UPGRADE" then
			return commandUnit(unit, CMD["UNITCOMMAND_UPGRADE"], {}), verb;
		end
		-- Anything else is a named operation from the resolved table: FORTIFY,
		-- ALERT, SKIP_TURN, HEAL, AUTOMATE_EXPLORE, BUILD_IMPROVEMENT.
		local hash = OP["UNITOPERATION_" .. verb];
		if hash == nil then return false, "unknown_op_" .. verb; end
		return operate(unit, hash, {}), verb;
	end

	return false, "unknown_kind_" .. kind;
end

-- ⚠ EMIT THE NUMERATOR AND THE DENOMINATOR. Four of this project's defects were
-- caught by a count and would have passed a boolean "is it wired up" check. An
-- `orders` event that says only "CIVVIS decided" reads identical whether every
-- order landed or every one was refused.
local function applyOrders(player, pid, turn, rows)
	local applied, refused = 0, 0;
	local byKind, whyNot = {}, {};

	-- ★★★★★ FOUND A CITY BEFORE MOVING, ALWAYS. This is an actuation rule of
	-- Civilization VI, not a decision, which is why it belongs here and not in CIVVIS.
	--
	-- A settler needs MOVEMENT REMAINING to found, and CIVVIS legitimately issues
	-- `MOVE_TO` then `FoundCity` for the same unit in one turn — it plans to walk onto
	-- the site and settle it. Applied in that order the move spends the last movement
	-- point and the found is refused, so the settler stands on a perfectly legal site
	-- and never settles.
	--
	-- Measured on run civvis-20260730T115158Z: `MOVE_TO 12 19` then `FOUND_CITY` every
	-- turn from t31 to t53, the settler at (12,19) with `moves = 0`, 4 tiles from the
	-- capital so `CITY_MIN_RANGE` was satisfied — 20+ turns of one refused order while
	-- the empire sat at ONE city and CIVVIS's own plan asked for three. It also
	-- oscillated (12,19)/(13,19) because the move was the only order that landed.
	--
	-- Founding first costs nothing when the settler is elsewhere (the found is refused,
	-- the move still happens, and it settles next turn) and fixes the case where it has
	-- already arrived. Either way it converges instead of looping.
	local ordered = {};
	local missed = 0;

	-- ★★★★★ DID THE UNIT ACTUALLY GO WHERE IT WAS SENT?
	--
	-- ⚠⚠ `applied = true` MEANS THE ENGINE ACCEPTED THE REQUEST, NOT THAT ANYTHING
	-- MOVED. That distinction has now cost this project four separate days — a Settler
	-- requested on 83 consecutive turns with `applied = true` and nothing built, a
	-- purchase whose `pcall` did not throw and bought nothing, and this. Every
	-- accounting we have is on the ISSUING side of the bridge; nothing has ever checked
	-- the RECEIVING side.
	--
	-- Measured on run `civvis-20260731T052021Z`, which is what this is for: at turn 42
	-- the settler stood at (13,11) and CIVVIS ordered `MOVE_TO (14,11)`. The order was
	-- counted applied, no refusal was recorded — and at turn 43 the settler was at
	-- (12,9), which is not on any path between those two tiles. It then bounced
	-- (13,11)/(12,9) for forty turns while the empire held one city. From the harness's
	-- side that run reads `orders_source: civvis`, `applied 11/12`, `residual: none`.
	--
	-- A move is not required to ARRIVE — `UNITOPERATION_MOVE_TO` is a multi-turn route
	-- and stopping short is ordinary. What is not ordinary is ending FARTHER from the
	-- destination than the unit started, so that is what this reports, with both
	-- positions, in Civilization VI's own OFFSET coordinates.
	local function plotDistance(x1, y1, x2, y2)
		return try(function() return Map.GetPlotDistance(x1, y1, x2, y2); end, -1);
	end
	local function runOrder(index, row)
		local kind = tostring(row.kind or "?");
		local verb = tostring(row.verb or "");
		local subject = tonumber(row.subject);
		local wantX, wantY = tonumber(row.x), tonumber(row.y);
		local fromX, fromY;
		local watched = (kind == "unit" and verb == "MOVE_TO"
			and subject ~= nil and wantX ~= nil and wantY ~= nil);
		if watched then
			local unit = liveUnit(pid, subject);
			if unit ~= nil then
				fromX = try(function() return unit:GetX(); end);
				fromY = try(function() return unit:GetY(); end);
			end
		end
		-- One pcall PER ORDER, never around the loop: the first order that throws
		-- must cost one order, not every order after it. That exact mistake
		-- (`pcall` outside the roster loop) hid seven separate bugs in this file.
		local ok, why = false, "throw";
		local safe, res1, res2 = pcall(function()
			return applyOrder(player, pid, row, turn);
		end);
		if safe then ok, why = res1, res2; end
		if ok then
			applied = applied + 1;
			byKind[kind] = (byKind[kind] or 0) + 1;
			if watched and fromX ~= nil then
				local unit = liveUnit(pid, subject);
				-- A unit that no longer exists was consumed or lost, which is not a
				-- missed move and must not be reported as one.
				if unit ~= nil then
					local toX = try(function() return unit:GetX(); end);
					local toY = try(function() return unit:GetY(); end);
					if toX ~= nil and toY ~= nil then
						local before = plotDistance(fromX, fromY, wantX, wantY);
						local after = plotDistance(toX, toY, wantX, wantY);
						if before >= 0 and after > before then
							missed = missed + 1;
							emit("move_missed", {
								turn = turn, unit = subject,
								from = { fromX, fromY }, to = { toX, toY },
								want = { wantX, wantY },
								before = before, after = after,
							});
						end
					end
				end
			end
		else
			refused = refused + 1;
			whyNot[tostring(why)] = (whyNot[tostring(why)] or 0) + 1;
		end
		ordered[index] = true;
	end

	for index, row in ipairs(rows) do
		if tostring(row.kind or "") == "unit" and tostring(row.verb or "") == "FOUND_CITY" then
			runOrder(index, row);
		end
	end
	for index, row in ipairs(rows) do
		if not ordered[index] then runOrder(index, row); end
	end

	-- ★★★★ UNITS CIVVIS DID NOT MENTION GO TO THE GAME'S OWN EXPLORE AUTOMATION.
	--
	-- ⚠ BE HONEST ABOUT WHAT THIS IS: it is a policy, and therefore a decision. It is
	-- here because the alternative is also a decision — a unit nobody ordered stands
	-- still — and standing still is what made domination unreachable. Measured at turn
	-- 21 of run civvis-20260730T132023Z: three units alive and the FURTHEST was **1
	-- tile** from the capital. Across whole games `met` stalls at 1-2 of 3 rivals, ZERO
	-- rival cities are ever seen, and an army of 20+ has nothing it can attack.
	--
	-- What it deliberately does NOT do is choose a destination. `AUTOMATE_EXPLORE` is
	-- Civilization VI's own automation, so the game picks where to go; this only decides
	-- that an idle unit should be doing something. Every unit CIVVIS actually assigns is
	-- untouched, and the count is reported separately as `explored` so a run's telemetry
	-- never presents this as CIVVIS's work.
	local explored = 0;
	if cfg.ExploreUnassigned ~= false then
		local mentioned = {};
		for _, row in ipairs(rows) do
			if tostring(row.kind or "") == "unit" then
				mentioned[tonumber(row.subject) or -1] = true;
			end
		end
		eachUnit(player, function(unit)
			local id = try(function() return unit:GetID(); end, -1);
			if id == -1 or mentioned[id] then return; end
			local name = unitTypeName(unit);
			-- Civilians cannot explore, and a settler that wanders is a settler that
			-- never founds — this project has already paid for both.
			if name == "UNIT_SETTLER" or name == "UNIT_BUILDER"
					or name == "UNIT_TRADER" then
				return;
			end
			if operate(unit, OP["UNITOPERATION_AUTOMATE_EXPLORE"], {}) then
				explored = explored + 1;
			end
		end);
	end

	emit("orders", {
		turn = turn, source = "civvis", seen = #rows,
		applied = applied, refused = refused, by = byKind, refusals = whyNot,
		-- Not part of `applied`: these are units CIVVIS said nothing about.
		explored = explored,
		-- Orders the engine ACCEPTED that left the unit farther from where it was
		-- sent. Counted apart from `refused` on purpose: a refusal is the bridge
		-- working and being told no; this is the bridge reporting success for
		-- something that did not happen.
		missed = missed,
	});

	-- ⚠⚠ A CIVVIS TURN MUST STILL EMIT A `turn` RECORD. The full one lives at the
	-- end of `playTurn`, which no longer runs when CIVVIS is deciding — so run
	-- civvis-20260730T110209Z produced ZERO turn records and every progress check,
	-- mine and the harness's, went blind. The harness's `[turn N]` lines come from
	-- this event, so its absence also disabled the stall watchdog's only clock.
	--
	-- Leaner than the heuristic path's record on purpose: the fields it omits
	-- (`war_blocked`, the settle-plan fires) describe built-ins that did not run.
	local counts = countUnits(player);
	local rivalTop, metCount = rivalBest(player, pid);
	local ourScore = try(function() return player:GetScore(); end, -1);
	local cityCount = 0;
	eachCity(player, function() cityCount = cityCount + 1; end);
	emit("turn", {
		turn = turn,
		score = ourScore,
		rival_best = rivalTop,
		met = metCount,
		lead = (rivalTop ~= nil and ourScore >= 0) and (ourScore - rivalTop) or nil,
		cities = cityCount,
		units = counts.total or counts.military,
		army = counts.military,
		gold = try(function() return math.floor(player:GetTreasury():GetGoldBalance()); end, -1),
		orders_source = awaiting.source,
		orders_seen = #rows,
		orders_applied = applied,
		orders_refused = refused,
		orders_polls = awaiting.polls,
		residual = residualAnswers,
		blocker = blockerName(currentBlocker(pid)),
	});
	return applied;
end

-- Publish the board and open the window in which CIVVIS answers.
--
-- Split out of `playTurn` because the export must happen whether or not CIVVIS
-- replies: it is the only thing the brain has to read, so skipping it on a
-- fallback turn would guarantee the next turn falls back too.
local function beginTurn(player, pid, turn)
	if cfg.ProbeChannels then probeChannels(turn); end
	-- Refreshed here rather than in the fallback so that the export, CIVVIS and the
	-- built-ins all describe the same war picture.
	warTarget = findWarTarget(player, pid);
	exportState(player, pid, turn);
	exportTiles(player, pid, turn);
	-- ★★★★ AN EMPIRE WITH NO CITIES IS DEFEATED, AND `PlayerDefeat` DOES NOT SAY SO.
	--
	-- Run civvis-20260730T170738Z ended on Civilization VI's DEFEAT screen at turn 80,
	-- and the event stream contains NO `defeat` event at all — the registered
	-- `PlayerDefeat` handler never fired. The harness then saw only silence and recorded
	-- `stalled: no event for 240s`, so a LOSS went into the ledger as a hung run. A
	-- ledger that cannot tell defeat from a wedge cannot be used to compare anything.
	--
	-- Losing every city is the definition of elimination here, and it is observable
	-- from inside without relying on an event that does not arrive.
	if not defeatReported and turn > 5 then
		local alive = 0;
		eachCity(player, function() alive = alive + 1; end);
		if alive == 0 then
			defeatReported = true;
			finished = true;
			emit("defeat", { turn = turn, ours = true, reason = "no cities remain",
			                 local_player = pid });
		end
	end

	awaiting.turn = turn;
	awaiting.ticks = 0;
	awaiting.polls = 0;
	awaiting.done = false;
	awaiting.source = "pending";
	-- Per turn, or the tally becomes cumulative and unreadable.
	residualAnswers = {};
end

-- Poll for CIVVIS's answer. Returns true once the turn's decisions are settled,
-- by CIVVIS or by the fallback; the caller must not end the turn before then.
local function settleTurn(player, pid, turn, playFallback)
	if awaiting.turn ~= turn then return true; end
	if awaiting.done then return true; end
	awaiting.ticks = awaiting.ticks + 1;

	-- ⚠⚠ DO NOT QUERY ON EVERY TICK. This is the bug that deadlocked run
	-- civvis-20260730T110209Z on turn 2: `GameCoreEventPublishComplete` fires
	-- thousands of times per turn, so a `DB.Query` per tick pinned the game at 139%
	-- CPU and — fatally — starved the `Automation.Log` flush that carries the board
	-- OUT. The brain never saw turn 2, so it never answered, so the wait never
	-- ended. A busy-wait on a channel whose other half needs the same thread is a
	-- deadlock, not a delay.
	--
	-- Polling every `OrdersPollTicks` costs 1/30th the queries and still answers
	-- within milliseconds of the orders landing.
	local every = cfg.OrdersPollTicks or 30;
	if awaiting.ticks % every ~= 0 then return false; end
	awaiting.polls = (awaiting.polls or 0) + 1;

	-- ★★★★★ THE HEARTBEAT IS LOAD-BEARING. IT IS NOT DIAGNOSTICS.
	--
	-- `watch.py` splits the log on newlines and keeps the trailing fragment as
	-- `partial` until a newline arrives. `Automation.Log` leaves no trailing newline,
	-- so the LAST line written is never delivered — and during this wait the last
	-- line written is exactly the `state` export the brain needs to answer. The
	-- outbound leg therefore required one more log line before it would release the
	-- previous one, and the loop deadlocked: measured twice, on turn 2 of runs
	-- civvis-20260730T110209Z and ...T111055Z, both with the log's final byte `}`
	-- and the game spinning at 139% CPU with nothing to do.
	--
	-- It also explains the earlier intermittency rather than leaving it a mystery:
	-- in the stub smoke test, turns succeeded only when some UNRELATED event
	-- (`blocked`, `build`) happened to terminate the state line. 4 of 10.
	--
	-- One line per poll terminates the previous one, so the state reaches the brain
	-- on the first poll. `polls` doubles as the wait's own telemetry.
	emit("await", { turn = turn, polls = awaiting.polls });

	local ready = ordersReady(turn);
	if ready ~= nil and ready > 0 then
		local rows = fetchOrders(turn);
		if #rows > 0 then
			-- ⚠ BEFORE, not after: `applyOrders` emits the turn record, which reads
			-- `awaiting.source`. Setting it afterwards made every CIVVIS turn report
			-- `orders_source: pending` — the one field that proves who drove the game,
			-- blank on the turns it was meant to describe.
			awaiting.source = "civvis";
			awaiting.done = true;
			applyOrders(player, pid, turn, rows);
			return true;
		end
	end

	-- Past the wait, prefer CIVVIS's most recent answer over the built-ins.
	if awaiting.polls >= (cfg.OrdersWaitPolls or 40) then
		local stale = newestAnsweredTurn(turn);
		local maxStale = cfg.OrdersMaxStale or 4;
		if stale ~= nil and stale > 0 and (turn - stale) <= maxStale then
			local rows = fetchOrders(stale);
			if #rows > 0 then
				awaiting.source = "civvis_stale";
				awaiting.done = true;
				applyOrders(player, pid, turn, rows);
				emit("orders_stale", { turn = turn, used = stale,
				                       behind = turn - stale, polls = awaiting.polls });
				return true;
			end
		end
	end
	-- ⚠ THE FLOOR. A brain that is slow, crashed, or has not been started must cost
	-- decision QUALITY, never progress: three regressions in this project came from
	-- a mechanism given authority with no floor for being wrong. Past the budget the
	-- built-in heuristics run and the turn is recorded as `fallback`, which is a
	-- number to watch — a run that is mostly fallback is not a measurement of CIVVIS.
	if awaiting.polls >= (cfg.OrdersFallbackPolls or 120) then
		-- ⚠ SET THE SOURCE *BEFORE* RUNNING THE FALLBACK. `playFallback` emits the
		-- turn record, which reads `awaiting.source` — assigning after the call made
		-- every fallback turn report `orders_source: pending`, so the one field that
		-- proves who drove the game was blank exactly when it mattered. Measured on
		-- run smoke-20260730T105241Z: six fallback turns, all labelled `pending`.
		awaiting.source = "fallback";
		awaiting.done = true;
		playFallback(player, pid, turn);
		emit("orders", { turn = turn, source = "fallback", seen = 0, applied = 0,
		                 refused = 0, waited = awaiting.ticks, polls = awaiting.polls });
		return true;
	end
	return false;
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
	-- ⚠ `warTarget` and the exports moved to `beginTurn`, which runs on every turn
	-- including one CIVVIS answers — the export is the only thing the brain has to
	-- read, so leaving it here would mean a CIVVIS turn published nothing and
	-- starved the next one. When CIVVIS is NOT deciding, `beginTurn` never runs, so
	-- this path still has to do them itself or the mirror goes dark.
	if not cfg.CivvisDecides then
		warTarget = findWarTarget(player, pid);
		exportState(player, pid, turn);
		exportTiles(player, pid, turn);
	end
	local war = declareWar(player, pid, countUnits(player), turn);
	local builds = driveProduction(player, turn);
	local ordered, stuck = orderUnits(player, pid, turn);
	-- Refreshed before any unit is ordered, so the whole army agrees this turn about
	-- whether it is strong enough to attack.
	armyNow = countUnits(player).military;
	-- ⚠ TWELVE, NOT EIGHT. Attacking at 8 is what the gate was built to prevent, one
	-- step removed: the army reaches 8, attacks, grinds to 6, rebuilds, attacks at 8
	-- again. Measured on settler-20260730T055245Z — army 9 at t45 assaulting, 11 at
	-- t60, back to 6 by t75, capital untouched. The threshold has to be a force that
	-- can finish the job, not the smallest force willing to start it.
	--
	-- `MilitaryPerCity * 4 cities` is 20, so 12 is reachable and still leaves
	-- garrisons. If `assaulting` never turns true in a run, this is the number to
	-- look at first.
	assaultReady = armyNow >= (cfg.AssaultMin or 12);
	local worstStack, piles = stackCensus(player);
	-- Both branches counted: `flipping` is how many cities the game says will fall,
	-- and worst/worst_rate describe the weakest city even when none is flipping yet.
	-- A count alone would read 0 right up to the turn a city is lost.
	local loyaltyWatch = { flipping = 0, worst = nil, worst_rate = nil };
	eachCity(player, function(city)
		local loyalty, perTurn, fallsTo = cityLoyalty(city);
		-- ⚠⚠ `falls_to` IS NOT THE DANGER SIGNAL, and I shipped it as one. It is
		-- the transfer TARGET — who a city would go to if it fell — and it reads
		-- **62** (the Free Cities slot) for every city we own, including a capital
		-- sitting at loyalty 100 and RISING +18 a turn. So `flipping` was just the
		-- city count: t25 cities=1 flipping=1, t29 cities=2 flipping=2.
		--
		-- Caught only because this emits the LEVEL and the RATE beside the count.
		-- A bare count would have looked plausible forever, which is the whole
		-- argument for never emitting a single number for a mechanism.
		--
		-- The honest signal is a NEGATIVE RATE: that is what killed (42,17) at -23
		-- a turn while `falls_to` said exactly the same thing about the safe capital.
		if perTurn ~= nil and perTurn < 0 then
			loyaltyWatch.flipping = loyaltyWatch.flipping + 1;
		end
		if loyalty ~= nil
				and (loyaltyWatch.worst == nil or loyalty < loyaltyWatch.worst) then
			loyaltyWatch.worst = loyalty;
			loyaltyWatch.worst_rate = perTurn;
		end
	end);
	local rivalTop, metCount = rivalBest(player, pid);
	local ourScore = try(function() return player:GetScore(); end, -1);
	emit("turn", {
		policies = policies,
		war = war,
		target = warTarget and (warTarget.capital and "capital" or "city") or nil,
		turn = turn,
		score = ourScore,
		rival_best = rivalTop,
		met = metCount,
		-- Positive means we would win a score victory at the turn limit against
		-- everyone we have met. This is the number that decides the reachable
		-- victory, and until now the log showed only our own half of it.
		lead = (rivalTop ~= nil and ourScore >= 0) and (ourScore - rivalTop) or nil,
		gold = try(function() return math.floor(player:GetTreasury():GetGoldBalance()); end, -1),
		cities = cityCount(player),
		units = countUnits(player).total,
		research = research, civic = civic,
		builds = builds, ordered = ordered, stuck = stuck,
		worst_stack = worstStack, piles = piles,
		-- ⚠ Loyalty on every turn event, not only under --export-state, because the
		-- defect it exposes cost 45 runs of not knowing: 56% of runs past turn 60
		-- lost a city AT PEACE, to revolt, and nothing in the telemetry said so.
		-- `flipping` counts cities the game itself says will fall; `worst_loyalty`
		-- and `worst_loyalty_rate` are the level and slide of the weakest city.
		-- Both a level and a RATE, because a city at 40 and climbing is safe while
		-- one at 60 and dropping 8 a turn is lost in three turns.
		sites_capped = siteCap.capped, sites_in_reach = siteCap.in_reach,
		-- ⚠ WHY war did not happen, not merely that it did not. Six silent `return
		-- nil` paths in `declareWar` used to be indistinguishable, and a silent gate
		-- already hid the worst bug this project has had. `war_target_player` is
		-- beside it because 'cannot declare on 62' (Free Cities) and 'cannot declare
		-- on 1' (a major civ) are different problems with the same symptom.
		-- ⚠ Both halves of the probe, because a `probesOut` counter above zero was
		-- exactly the lie that hid this: `probe_dest` says a destination was found
		-- and `probe_kind` says whether it came from known enemy land or from the
		-- frontier fallback. The action histogram showing `probe` is the real proof.
		probes_out = probesOut,
		probe_dest = probeDest,
		probe_kind = probeKind,
		war_blocked = warBlock,
		-- ⚠ THE FIRES-CHECK FOR THE WHOLE ARCHITECTURE. "CIVVIS is deciding" is a
		-- claim, and this is its denominator: `civvis` means the orders arrived and
		-- were actuated, `fallback` means the built-in heuristics ran because nothing
		-- arrived in time. A run that is mostly `fallback` is not a measurement of
		-- CIVVIS, however much CIVVIS code exists — the same trap as an evaluator
		-- that never loaded while the docs called it good and inert.
		orders_source = awaiting.source,
		orders_waited = awaiting.ticks,
		-- Built-in passes that ran on a turn credited to CIVVIS, by blocker name.
		-- Non-empty means the heuristics still made some of this turn's decisions.
		residual = residualAnswers,
		war_target_player = warTarget and warTarget.player or nil,
		-- ⚠ Whether the strength term actually CHANGED the choice, not merely that it
		-- ran. `target_ratio` above 1 means we are still attacking somebody stronger
		-- than us — which is the failure this term exists to prevent, and it would be
		-- invisible if only the target id were logged.
		target_their_score = warTarget and warTarget.their_score or nil,
		target_ratio = (warTarget and warTarget.our_score and warTarget.our_score > 0)
			and (warTarget.their_score / warTarget.our_score) or nil,
		flipping = loyaltyWatch.flipping,
		worst_loyalty = loyaltyWatch.worst,
		worst_loyalty_rate = loyaltyWatch.worst_rate,
		envoys_placed = envoyTally.placed, suzerainties = envoyTally.suzerainties,
		levies = envoyTally.levies, met_minors = envoyTally.met_minors,
		-- ★ The ram fires-check. `siege` staying 0 while `target` is set means no
		-- wall will ever break, whatever the ladder claims to queue. Reported every
		-- turn precisely because I once believed the entry existed when it did not.
		siege = countUnits(player).siege,
		-- Whether the army was allowed to attack this turn, and how big it was.
		army = armyNow,
		assaulting = assaultReady,
		-- Which brain is choosing city sites. `plan_sites` staying at zero while a
		-- plan is configured means CIVVIS is NOT deciding, whatever the config says.
		plan_sites = planFires.plan,
		own_sites = planFires.search,
		plan_offered = planFires.offered,
		-- The near-window's price and its payoff, as running totals. Divide by
		-- `near_n` for the means. ⚠ Both are needed: `near_rank` alone says only
		-- that value was given up, `near_dist` alone says only that walks are
		-- short. The trade is good when dist falls faster than rank rises.
		near_rank = planFires.near_rank or 0,
		near_dist = planFires.near_dist or 0,
		near_n = planFires.near_n or 0,
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
			if cfg.CivvisDecides then
				-- Publish the board; the decisions land on a later tick, once CIVVIS
				-- has answered. `GameCoreEventPublishComplete` fires many times per
				-- frame, so "later this turn" costs no wall clock the game was not
				-- already spending.
				beginTurn(player, pid, turn);
			else
				playTurn(player, pid, turn);
			end
		end

		-- ⚠ DO NOT END A TURN WHOSE DECISIONS HAVE NOT BEEN MADE. Every order below
		-- this point assumes the turn has been played. Ending first would hand CIVVIS
		-- a board it never acted on and read, in telemetry, exactly like a controller
		-- that decided to do nothing.
		if cfg.CivvisDecides and not settleTurn(player, pid, turn, playTurn) then
			return;
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
