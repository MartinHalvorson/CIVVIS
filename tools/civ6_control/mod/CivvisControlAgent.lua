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

-- The optional game modes, from the `ConfigurationId`s the content packs
-- register. Same list as CivvisControlSetup.lua's; that one sets them and this
-- one reports what the running game actually has.
local GAME_MODES = {
	"GAMEMODE_APOCALYPSE",
	"GAMEMODE_BARBARIAN_CLANS",
	"GAMEMODE_DRAMATICAGES",
	"GAMEMODE_HEROES",
	"GAMEMODE_MONOPOLIES",
	"GAMEMODE_RANDOM",
	"GAMEMODE_SECRETSOCIETIES",
	"GAMEMODE_TOWERDEFENSE",
	"GAMEMODE_TREE_RANDOMIZER",
};

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
-- Civ 6 city id -> the item CIVVIS asked that city to build THIS turn. Cleared with
-- the rest of the per-turn handshake state; see `chooseProduction`.
local civvisBuild = {};
-- Per-city production names the engine has already rejected on this turn. This is
-- deliberately turn-scoped: a strategic resource or prerequisite can change later,
-- but retrying the same impossible choice in every blocker pass cannot help.
local refusedByCity = {};
-- ⚠⚠ CITY LOSS IS NOT AN EVENT, AND IT IS THE CONSTRAINT ON EVERYTHING.
--
-- 36% of live runs reaching turn 150 end with ONE city, and **96% of those
-- founded cities and lost them** — peaks of 3, 4, 5, 6, even 7. Median peak is
-- 4 and median final is 2; 61% of runs lose at least one city and **32% of all
-- cities ever founded are lost**. Score is 194 at one city against 603 at seven.
-- Population is what science is, so this is the science ceiling.
--
-- And nothing reports it. There is no `kind` in `events.jsonl` for a city
-- changing hands, so the only way to study it has been to diff the periodic
-- state exports and read each city's LAST SEEN condition. That inference put
-- military capture at 41% and loyalty at 43% over 181 lost cities — but the
-- military share is a FLOOR, not a number, because a city taken BETWEEN two
-- exports is never observed damaged and lands in the wrong bucket.
--
-- This remembers the roster every turn instead, so a loss is caught at turn
-- resolution with the city's condition on the turn before it went. That is the
-- difference between a 181-row inference and a census, and it is the cheap
-- prerequisite to pricing either half of the holding problem — both of which
-- have already absorbed many attempts and measured null.
local lastRoster = {};
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
		"UNITOPERATION_FOUND_CITY", "UNITOPERATION_FOUND_RELIGION",
		"UNITOPERATION_MOVE_TO",
		"UNITOPERATION_FORTIFY", "UNITOPERATION_ALERT",
		"UNITOPERATION_SKIP_TURN", "UNITOPERATION_SLEEP",
		"UNITOPERATION_HEAL", "UNITOPERATION_AUTOMATE_EXPLORE",
		"UNITOPERATION_BUILD_IMPROVEMENT", "UNITOPERATION_REPAIR", "UNITOPERATION_RANGE_ATTACK",
		-- Pillage was never resolved, so `Action::Pillage` had no host verb and
		-- light cavalry's pillage-before-combat could not happen on the live
		-- seat. Parameterless, like FORTIFY: the unit pillages the tile it is on.
		"UNITOPERATION_PILLAGE",
		"UNITOPERATION_HARVEST_RESOURCE", "UNITOPERATION_REST_REPAIR",
		"UNITOPERATION_MAKE_TRADE_ROUTE", "UNITOPERATION_SPREAD_RELIGION",
		-- ★★★ ESPIONAGE, WHICH THE ENGINE MODELS IN FULL AND THE BRIDGE COULD
		-- NOT SEND. `Game::spies` -- the only structure `advanced_spies` and
		-- `BasicAi::spies` iterate -- is empty for an entire live game, so a
		-- tuned, victory-aimed disruption layer (great_work_heist 340 against a
		-- Culture leader, disrupt_rocketry 290 against a Science one) was a
		-- guaranteed no-op. These are the thirteen the live build exposes; a
		-- survey of `UnitOperationTypes` dumped every one.
		"UNITOPERATION_SPY_TRAVEL_NEW_CITY", "UNITOPERATION_SPY_COUNTERSPY",
		"UNITOPERATION_SPY_LISTENING_POST", "UNITOPERATION_SPY_GAIN_SOURCES",
		"UNITOPERATION_SPY_STEAL_TECH_BOOST", "UNITOPERATION_SPY_SIPHON_FUNDS",
		"UNITOPERATION_SPY_GREAT_WORK_HEIST", "UNITOPERATION_SPY_SABOTAGE_PRODUCTION",
		"UNITOPERATION_SPY_DISRUPT_ROCKETRY", "UNITOPERATION_SPY_NEUTRALIZE_GOVERNOR",
		"UNITOPERATION_SPY_RECRUIT_PARTISANS", "UNITOPERATION_SPY_FOMENT_UNREST",
		"UNITOPERATION_SPY_FABRICATE_SCANDAL",
	}) do
		OP[name] = opHash(name);
	end
	for _, name in ipairs({
		"UNITCOMMAND_AUTOMATE", "UNITCOMMAND_PROMOTE", "UNITCOMMAND_WAKE",
		"UNITCOMMAND_UPGRADE", "UNITCOMMAND_DELETE",
		"UNITCOMMAND_ACTIVATE_GREAT_PERSON", "UNITCOMMAND_ENTER_FORMATION",
		"UNITCOMMAND_EXIT_FORMATION",
		-- ★★★ THE ONLY WAY A SOLDIER TOUCHES A MISSIONARY. Religious units are
		-- excluded from ordinary combat by design -- they cannot be attacked,
		-- captured, or run over -- so an enemy Apostle standing in our land was
		-- untouchable by anything this bridge could send. CIVVIS has decided
		-- this action all along (`condemn_step`) and it was dropped in
		-- translation. Measured over the 39 live games of 2026-08-16/17: rival
		-- missionaries and apostles are visible on 47% of turn-samples, and 41%
		-- of those sightings already have one of our military units within two
		-- tiles, against 12,708 rival religious-unit sightings to our 590.
		"UNITCOMMAND_CONDEMN_HERETIC",
		-- Cancels a unit's queued path; see CivvisBoard.cancelQueuedPaths.
		"UNITCOMMAND_CANCEL",
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
	-- `GetRuleSet()` returns the configuration's type name directly, unlike
	-- the numeric hashes returned by difficulty, map size, and game speed.
	-- Indexing GameInfo with that string raises on the live build, and the
	-- guarded lookup silently turned every correctly configured game into `?`.
	if type(hash) == "string" then return hash; end
	return try(function()
		local row = kindTable[hash];
		return row and (row.DifficultyType or row.MapSizeType or row.GameSpeedType
			or row.RulesetType or row.Type) or nil;
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

-- Progress and cost for whatever a city is building right now.
--
-- ★★★★★ THERE IS NO GENERIC ACCESSOR, AND ASSUMING ONE COST A WHOLE RUN'S
-- DIAGNOSTICS. `GetBuildQueue():GetCurrentProductionProgress()` and
-- `…GetCurrentProductionCost()` do not exist on this build: both returned the
-- `try` sentinel **-1 on every city of every turn** of run
-- civvis-20260802T053109Z, while `GetProductionYield` and `GetTurnsLeft` beside
-- them worked and read 6/9 and 5/4/3.
--
-- The shipped UI names the real ones, and they are TYPE-SPECIFIC — read out of
-- `Base/Assets/UI/Panels/ProductionPanel.lua`, whose BuildQueue calls are exactly:
--
--     GetUnitProgress     GetUnitCost
--     GetBuildingProgress GetBuildingCost
--     GetDistrictProgress GetDistrictCost
--     GetProjectProgress  GetProjectCost
--     GetCurrentProductionTypeHash   GetTurnsLeft
--
-- So the hash has to be resolved to its KIND first, which `productionName` above
-- already does by walking the same four GameInfo tables. This walks them in the
-- same order and calls the matching pair.
--
-- ⚠ Returns two values and BOTH default to -1 independently, for the same reason
-- the four fields at the call site are guarded separately: a build where one of
-- these is missing must still yield the other.
local function productionProgress(city, hash)
	if hash == nil or hash == 0 then return -1, -1; end
	local queue = try(function() return city:GetBuildQueue(); end);
	if queue == nil then return -1, -1; end
	local kinds = {
		{ GameInfo.Units,      "GetUnitProgress",     "GetUnitCost"     },
		{ GameInfo.Buildings,  "GetBuildingProgress", "GetBuildingCost" },
		{ GameInfo.Districts,  "GetDistrictProgress", "GetDistrictCost" },
		{ GameInfo.Projects,   "GetProjectProgress",  "GetProjectCost"  },
	};
	for _, entry in ipairs(kinds) do
		-- ⚠⚠⚠ THE ACCESSORS TAKE `row.Index`, NOT THE HASH, AND GETTING THIS WRONG
		-- SEGFAULTS THE GAME — `try`/`pcall` CANNOT CATCH IT.
		--
		-- The shipped UI is unambiguous (ProductionPanel.lua):
		--     pBuildQueue:GetUnitCost( row.Index )
		--     pBuildQueue:GetUnitProgress( row.Index )
		--     pBuildQueue:GetBuildingProgress( pRow.Index )
		--     pBuildQueue:GetDistrictProgress( pRow.Index )
		--
		-- The first version of this function passed the 32-bit hash from
		-- `GetCurrentProductionTypeHash()` straight through. The engine indexed
		-- far out of bounds and Civilization VI died with EXC_BAD_ACCESS / SIGBUS,
		-- KERN_PROTECTION_FAILURE, in GameCore_XP2.dll +471072 — FOUR crashes in
		-- seventeen minutes on 2026-08-02, one per attempt, each run ending at
		-- turn 1 or 2 with `reason: "game exited"`. It took out a whole batch.
		--
		-- ⚠ A `pcall` around an engine call does not make it safe. It catches Lua
		-- errors; it does not catch a native memory fault. The guard that matters
		-- here is passing the right argument, not wrapping the wrong one.
		local row = try(function() return entry[1][hash]; end);
		local index = row and try(function() return row.Index; end);
		if index ~= nil then
			local progress = try(function() return queue[entry[2]](queue, index); end, -1);
			local cost = try(function() return queue[entry[3]](queue, index); end, -1);
			return progress, cost;
		end
	end
	return -1, -1;
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
		-- ★★★★★ WHICH GAME THIS IS. The harness SETS `RULESET_EXPANSION_2` and
		-- nothing ever read it back, so every other setting on this survey was
		-- verified from inside the game and the one that decides which rules
		-- exist at all was taken on trust. `setup: "(absent)"` on this build
		-- means requested settings can silently fail to apply — that is the
		-- whole reason this survey exists — and the ruleset was the one axis
		-- with no reading that could be wrong.
		--
		-- It is the same shape as the game-modes defect: GAMEMODE_HEROES ran on
		-- a live game for an unknown number of turns while every log said plain
		-- Gathering Storm, because nothing reported it. CIVVIS models Gathering
		-- Storm and nothing else, so a Vanilla or Rise & Fall game is not a
		-- weaker measurement of the same thing — it is a different game, and
		-- `civ6_fidelity.py` was separately found auditing against a Vanilla
		-- database while printing "Gathering Storm" at the top of its report.
		ruleset = typeName(GameInfo.Rulesets,
			try(function() return GameConfiguration.GetRuleSet(); end)) or "?",
		map = try(function() return MapConfiguration.GetScript(); end, "?"),
		size = typeName(GameInfo.Maps,
			try(function() return MapConfiguration.GetMapSize(); end)) or "?",
		max_turns = try(function() return GameConfiguration.GetMaxTurns(); end, -1),
		-- ★★★ WHICH OPTIONAL GAME MODES ARE ON, read from inside the game.
		-- Exactly the `victories` argument below, and it went the same way: the
		-- modes are the one setting on the Create Game screen that PERSISTS
		-- across games, so one switched on months ago stays on forever, and
		-- nothing here ever said so. GAMEMODE_HEROES was found true on a live
		-- run -- twelve hero units and their rules, on a board CIVVIS models
		-- none of, under a heading that said plain Gathering Storm.
		--
		-- Reported as the list of what is ON, so a clean run answers `[]`.
		-- `civ6_play.py` refuses to call a run "configured" when this is not
		-- what the run asked for.
		modes = (function()
			local on = {};
			for _, mode in ipairs(GAME_MODES) do
				local set = try(function() return GameConfiguration.GetValue(mode); end);
				-- 0 is truthy in Lua, and these come back 0/1 on some builds:
				-- a plain `if set then` reports every mode as enabled.
				if set == true or set == 1 then on[#on + 1] = mode; end
			end
			return on;
		end)(),
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
		-- ★★ THE HOST'S OWN VICTORY TABLE, index -> type, so the raw integer the
		-- `TeamVictory` event reports is self-describing inside every run's own
		-- record. `docs/CIV6_LADDER.md` refuses guessed names for that integer —
		-- rightly; joining 180 ladder rows to the Hall of Fame's `VictoryType`
		-- strings measured 0=SCORE 3=CULTURE 4=RELIGIOUS 5=TECHNOLOGY
		-- 6=DIPLOMATIC on this install — and this export replaces the join with
		-- the table the indices come from.
		victory_types = try(function()
			local types = {};
			for row in GameInfo.Victories() do
				types[#types + 1] = { index = row.Index, type = row.VictoryType };
			end
			return #types > 0 and types or nil;
		end, nil),
		players = try(function() return #PlayerManager.GetAliveMajorIDs(); end, -1),
		-- Reserve every configured city-state seat in the persistent mirror. The
		-- `minors` state list contains only actors already met, so sizing from that
		-- list on turn 1 leaves nowhere to put a city-state discovered later.
		city_states = try(function()
			return GameConfiguration.GetValue("CITY_STATE_COUNT");
		end, 0),
		-- Left behind by the setup context; see CivvisControlSetup.lua. Absent
		-- means this game was started some other way -- by a person clicking
		-- Play Now, say -- so its settings are the game's defaults and not the
		-- ones this run asked for.
		setup = try(function() return GameConfiguration.GetValue("CIVVIS_SETUP"); end)
			or "(absent)",
		-- ★★★★ WHAT THIS MOD CAN ACTUATE, read back by the brain. `civvis_orders`
		-- sends a unit's whole planned sequence (walk, strike, fortify) only
		-- when the mod that will apply it says it sequences per-unit orders
		-- (`CivvisQueue`); against an older mod it defers the follow-ups exactly
		-- as before. A capability the sender assumes and the receiver lacks is
		-- how an accepted order becomes a silent no-op.
		order_queue = cfg.OrderQueue ~= false,
		-- Every MOVE_TO capped to this turn's leg and combat units' queued
		-- paths cancelled at turn start, so `moves` at export means movement
		-- available this turn and the mirror may trust it. See CivvisBoard.
		moves_at_turn_start = cfg.CapMovesToReach ~= false,
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
	-- The FrontEnd setup context has the same assignment, but this in-game rehost
	-- is the path that actually runs on this build. Omitting it made every other
	-- requested setting exact while silently leaving the civilization random.
	if cfg.Leader then
		for _, id in ipairs(GameConfiguration.GetHumanPlayerIDs()) do
			PlayerConfigurations[id]:SetLeaderTypeName(cfg.Leader);
		end
	end
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
		leader = try(function()
			local ids = GameConfiguration.GetHumanPlayerIDs();
			return ids[1] and PlayerConfigurations[ids[1]]:GetLeaderTypeName() or nil;
		end),
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

-- What a unique unit REPLACES, so the board keeps it instead of dropping it.
--
-- ★★★★ A rival's unique unit is untranslatable and is therefore DISCARDED. Live run
-- `civvis-20260801T145302Z` dropped `UNIT_NORWEGIAN_LONGSHIP` every turn it was
-- visible: CIVVIS models no Norwegian uniques at all (`unique_to == Norway` is
-- empty in `data/units.json`), so an enemy WARSHIP simply was not on the board.
-- That is not one unit — it is every civilization's uniques, for every civilization
-- we meet.
--
-- A Longship is a Galley replacement, and a Galley CIVVIS does model. Sending the
-- base type lets the mirror fall back to something true rather than nothing, which
-- is the standing rule: a dropped entity is worse than an approximate one.
--
-- ⚠ Copied from `ToolTipHelper_Babylon_Heroes.lua`, not guessed:
--     local replaces = GameInfo.UnitReplaces[unitType];
--     if replaces then ... GameInfo.Units[replaces.ReplacesUnitType] end
local function unitBaseType(name)
	if name == nil or name == "" then return nil; end
	local replaces = try(function() return GameInfo.UnitReplaces[name]; end);
	if replaces == nil then return nil; end
	return try(function() return replaces.ReplacesUnitType; end);
end

-- A STANDALONE unique has no UnitReplaces row at all — Malón Raider, Varu,
-- Nihang — so `base` comes back nil and the mirror used to drop the unit
-- entirely. Its PromotionClass is still in the shipped database, and class is
-- enough for the mirror to land it as something true rather than nothing.
local function unitClass(name)
	if name == nil or name == "" then return nil; end
	local row = try(function() return GameInfo.Units[name]; end);
	if row == nil then return nil; end
	return try(function() return row.PromotionClass; end);
end

-- Facts that decide what a unit may do next. Reconstructing every live unit from
-- its type defaults reset Apostles to full charges with no promotion and military
-- units to level one on every turn, so CIVVIS repeatedly chose actions Firaxis had
-- already consumed. The stock Unit Panel reads these exact accessors.
local function unitProgress(unit)
	local experience = try(function() return unit:GetExperience(); end);
	local promotions = {};
	if experience ~= nil then
		for _, index in ipairs(try(function()
			return experience:GetPromotions();
		end, {}) or {}) do
			local row = try(function() return GameInfo.UnitPromotions[index]; end);
			if row ~= nil and row.UnitPromotionType ~= nil then
				promotions[#promotions + 1] = row.UnitPromotionType;
			end
		end
	end
	table.sort(promotions);
	local religion = nil;
	local religionIndex = try(function() return unit:GetReligionType(); end, -1);
	if religionIndex ~= nil and religionIndex >= 0 then
		local row = try(function() return GameInfo.Religions[religionIndex]; end);
		religion = row ~= nil and row.ReligionType or nil;
	end
	return {
		xp = experience ~= nil and try(function()
			return experience:GetExperiencePoints();
		end) or nil,
		level = experience ~= nil and try(function()
			return experience:GetLevel();
		end) or nil,
		promotions = promotions,
		build_charges = try(function() return unit:GetBuildCharges(); end, 0),
		spread_charges = try(function() return unit:GetSpreadCharges(); end, 0),
		religion = religion,
	};
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

-- Where each unit stood at the end of the last turn we looked, so "has this unit
-- moved?" can be asked at all. Keyed by unit id, which is stable for the life of
-- the unit.
local lastSeenAt = {};
-- How many consecutive turns a unit has held still while it still had movement.
local idleTurns = {};

-- ⚠⚠⚠ A SETTLER THAT HAS NOT MOVED IN N TURNS IS NOT "IN FLIGHT". This is the
-- predicate behind that sentence, lifted out so it can be tested without a
-- running Civ 6 — everything around it is engine accessors.
--
-- `moves > 0` with an UNCHANGED position is the tell. A settler genuinely
-- blocked by terrain or a unit spends its movement and stops; one that is
-- stranded is handed full movement every turn and does nothing with it.
-- Measured on run `civvis-20260803T231038Z`: the settler sat on a COAST tile at
-- offset (24,36) from t153 to the end of the run, embarked, with two movement
-- points EVERY turn, one tile from unowned grassland it had itself chosen.
-- Fleet-wide, 38% of runs park a settler for >=15 consecutive turns at full
-- movement (median streak 37).
--
-- ⚠ Why this matters far more than one wasted unit: the expansion gate is
-- `counts.settler < SettlersInFlight`, and `SettlersInFlight` is 1. So ONE
-- stranded settler stops the empire ordering another, forever. That run ordered
-- no settler between turns 51 and 134 and finished four cities behind.
local STRANDED_SETTLER_TURNS = 12;

local function settlerIsStranded(idle)
	return (idle or 0) >= STRANDED_SETTLER_TURNS;
end

-- Exposed for the offline test only. ⚠ A BARE GLOBAL, never `_G.` — Civ 6's UI
-- Lua sandbox does not expose `_G` and indexing it raises at chunk load, which
-- took the whole agent out once already.
CivvisSettlerIsStranded = settlerIsStranded;

-- Advance every unit's idle streak. ⚠ MUST be called exactly once per turn --
-- see the note at the call site. A streak advanced from `countUnits` would count
-- counting passes rather than turns.
local function trackIdleUnits(player)
	local seen = {};
	eachUnit(player, function(unit)
		local uid = try(function() return unit:GetID(); end, -1);
		if uid < 0 then return; end
		seen[uid] = true;
		local x = try(function() return unit:GetX(); end, -1);
		local y = try(function() return unit:GetY(); end, -1);
		local moves = try(function() return unit:GetMovesRemaining(); end, 0) or 0;
		local was = lastSeenAt[uid];
		-- Movement left AND the same tile as last turn is the signal. A unit that
		-- spent its movement is working, however little it achieved; a unit
		-- handed a full allowance it never spends is stuck.
		if was ~= nil and was.x == x and was.y == y and moves > 0 then
			idleTurns[uid] = (idleTurns[uid] or 0) + 1;
		else
			idleTurns[uid] = 0;
		end
		lastSeenAt[uid] = { x = x, y = y };
	end);
	-- Dead units must not keep their streaks: Civilization VI reuses unit ids,
	-- and inheriting a stranded streak would make a fresh settler read stranded
	-- the moment it was trained.
	for uid in pairs(lastSeenAt) do
		if not seen[uid] then lastSeenAt[uid] = nil; idleTurns[uid] = nil; end
	end
end

-- Pure. ⚠ An `upgradeUnit` call had been spliced into the military branch here,
-- so *counting* the army issued upgrade orders — and its `return better` skipped
-- the increment, so an upgrading unit was never counted as military. Counting
-- runs more than once a turn and feeds the war threshold; it must not act.
-- Upgrading belongs in `orderFor`, which is where it now lives.
local function countUnits(player)
	local counts = { settler = 0, builder = 0, military = 0, scout = 0, siege = 0,
	                 ranged = 0, total = 0, stranded_settler = 0 };
	eachUnit(player, function(unit)
		local name = unitTypeName(unit);
		local row = GameInfo.Units[name];
		counts.total = counts.total + 1;
		if name == "UNIT_SETTLER" then
			-- ⚠ COUNTED, THEN DISCOUNTED. The settler still exists and other
			-- code needs to know that; what changes is only whether it holds
			-- the expansion gate shut. See `settlerIsStranded`.
			counts.settler = counts.settler + 1;
			local uid = try(function() return unit:GetID(); end, -1);
			if settlerIsStranded(idleTurns[uid]) then
				counts.stranded_settler = counts.stranded_settler + 1;
			end
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
			if (row.RangedCombat or 0) > 0 or (row.Bombard or 0) > 0 then
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
	if plot == nil then return nil, nil, nil, nil, nil; end
	local district = try(function() return CityManager.GetDistrictAt(plot); end);
	if district == nil then return nil, nil, nil, nil, nil; end
	local strength = try(function() return district:GetDefenseStrength(); end);
	-- These are the exact calls used by Firaxis's CityBannerManager.lua. There is
	-- no `GetDefenseDamage()` accessor: calling it through `try` omitted `damage`
	-- from every state event and made the mirror assume every wall was pristine.
	local damage = try(function()
		return district:GetDamage(DefenseTypes.DISTRICT_GARRISON);
	end);
	local maxDamage = try(function()
		return district:GetMaxDamage(DefenseTypes.DISTRICT_GARRISON);
	end);
	local wallDamage = try(function()
		return district:GetDamage(DefenseTypes.DISTRICT_OUTER);
	end);
	local maxWallDamage = try(function()
		return district:GetMaxDamage(DefenseTypes.DISTRICT_OUTER);
	end);
	return strength, damage, maxDamage, wallDamage, maxWallDamage;
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
--
-- These commands are parameterless. Firaxis's shipped UnitPanel.lua checks
-- them with `(unit, hash, false, true)` and requests them with exactly two
-- arguments. Supplying `{}` as a third RequestCommand argument made the call
-- return without throwing but left Great Writers alive and upgrades undone.
-- `loose` asks the shipped UnitPanel's BUTTON gate instead of the strict
-- "right now" one, and names the reason when even that refuses.
--
-- ★★★★ THE STRICT GATE NEVER LET A DELETE THROUGH. `CanStartCommand(unit,
-- hash, false, true)` answered `false` to UNITCOMMAND_DELETE on every one of
-- the 495 attempts across runs civvis-20260815T220819Z (178), 233405Z (202)
-- and 20260816T003229Z (115): zero `retired_*` events exist in any run, and
-- the founded Prophet stood on its hex beside the capital all game. The
-- shipped UnitPanel.lua never asks the strict question for DELETE at all: it
-- gates the Delete button on the LOOSE form, `CanStartCommand(pUnit, DELETE,
-- true)`, then calls `RequestCommand` outright (OnPromptToDeleteUnit /
-- OnDeleteUnit). `loose == true` does exactly that. RequestCommand is
-- asynchronous, so `true` means "requested": the unit vanishing from the next
-- export is the confirmation, and one still there is asked again by the bridge
-- and shows up as a repeat under the same id.
local function commandUnit(unit, hash, loose)
	if hash == nil then return false; end
	local ok, can = pcall(function()
		if loose == true then
			return UnitManager.CanStartCommand(unit, hash, true);
		end
		return UnitManager.CanStartCommand(unit, hash, false, true);
	end);
	if not (ok and can == true) then
		if loose ~= true then return false; end
		-- Name the refusal through the results table, the way `upgradeUnit`
		-- does, so "asked and declined" is never anonymous.
		local why = "loose CanStartCommand refused";
		pcall(function()
			local _, results = UnitManager.CanStartCommand(unit, hash, false, true);
			if UnitCommandResults ~= nil and type(results) == "table" then
				local reasons = results[UnitCommandResults.FAILURE_REASONS];
				if type(reasons) == "table" and #reasons > 0 then
					why = table.concat(reasons, "; ");
				end
			end
		end);
		return false, why;
	end
	return pcall(function()
		UnitManager.RequestCommand(unit, hash);
	end);
end

-- Spend gold to bring a unit up to date before spending its life.
-- Spend gold to bring a unit up to date before spending its life.
--
-- ⚠ The agent fielded WARRIORs and SPEARMEN in 1100 AD against swordsmen and
-- archers — military strength 78 against 357, a 4.5:1 deficit — and the combat
-- log read as a rout. The army ladder builds ancient units and nothing ever
-- upgraded them, while the treasury sat on 478 unspent Gold. UNITCOMMAND_UPGRADE
-- is in this build's resolved command list, so this is available and was simply
-- never attempted.
-- ⚠ WHETHER THE ARMY CAN RE-ARM AT ALL, WHICH NO COUNTER HAS EVER SAID.
--
-- Run `civvis-20260801T065721Z` fielded nothing but Ancient units for 195 turns and
-- `upgrade` appears in no `by` map on the whole run -- yet `upgradeUnit` is called
-- for EVERY combat unit EVERY turn, first, from `orderFor`. So it was attempted
-- thousands of times and silently refused every single time, and nothing anywhere
-- recorded whether the block was gold, tech, or a missing strategic resource.
--
-- That is the same shape as `no_params` x 221 and `move_refused` x 33: an anonymous
-- count. Both times naming it made the cause fall out immediately and both times the
-- standing hypothesis was wrong, so this names it BEFORE anything is changed.
local upgradeTried, upgradeBlocked, upgradeBlockedWhy = 0, {}, {};
local function upgradeUnit(unit)
	upgradeTried = upgradeTried + 1;
	if commandUnit(unit, CMD["UNITCOMMAND_UPGRADE"]) then return "upgrade"; end
	local name = unitTypeName(unit) or "?";
	upgradeBlocked[name] = (upgradeBlocked[name] or 0) + 1;
	-- Ask the engine WHY, the way the shipped UnitPanel does: the two-flag form
	-- of CanStartCommand returns a results table whose FAILURE_REASONS names
	-- gold, missing tech, missing resource. Runs plateau at army 5-7 against
	-- rival 850+ with upgrade_blocked counting Warriors and Trebuchets every
	-- turn — the COUNT is known, the reason is the decision-relevant part.
	-- Blocked path only; the accepting path stays one call.
	-- ⚠⚠ AND SAY WHY THE REASON IS MISSING WHEN IT IS. #1237 wired this helper
	-- to CIVVIS's own orders and the counts finally moved — 93 attempts and 77
	-- refusals on the first run — but `upgrade_blocked_why` came back EMPTY on
	-- every one of them. An empty reason has FOUR distinct causes and they are
	-- not interchangeable: the global is absent (this sandbox has no `_G`, see
	-- the `revealed_api` lesson), `CanStartCommand` returned no table, the table
	-- carried no FAILURE_REASONS, or the list was there and empty.
	--
	-- Recording WHICH costs one string per blocked unit and is the difference
	-- between "the engine declined to answer" and "we never asked properly".
	try(function()
		-- UnitPanel's ordinary-command path asks the real, current-turn
		-- question with `(false, true)`. The old diagnostic used `(true,
		-- true)`, the loose exclusion test used only to decide whether a
		-- command could ever appear in the UI; on live upgrades that returned
		-- the Boolean and no results table, hiding every failure reason.
		local can, results = UnitManager.CanStartCommand(
			unit, CMD["UNITCOMMAND_UPGRADE"], false, true);
		if can == true then return; end
		if UnitCommandResults == nil then
			upgradeBlockedWhy[name] = "?no UnitCommandResults global";
			return;
		end
		if type(results) ~= "table" then
			upgradeBlockedWhy[name] = "?no results table (" .. type(results) .. ")";
			return;
		end
		local reasons = results[UnitCommandResults.FAILURE_REASONS];
		if type(reasons) ~= "table" then
			upgradeBlockedWhy[name] = "?no FAILURE_REASONS key";
			return;
		end
		if #reasons == 0 then
			upgradeBlockedWhy[name] = "?FAILURE_REASONS empty";
			return;
		end
		upgradeBlockedWhy[name] = table.concat(reasons, "; ");
	end);
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

-- Is the loyalty-reach penalty starving the settle search? `capped` counts legal
-- sites rejected for being out of support range, `in_reach` the ones inside it.
-- ⚠ A single number could not answer this: `capped` alone rises on a big map with
-- plenty of near ground, and `in_reach` alone cannot show what was given up.
local siteCap = { capped = 0, in_reach = 0 };

findSettleSite = function(player, pid, unit, turn)
	-- The built-in fallback ranks local legal ground. Per-turn CIVVIS decisions
	-- arrive through the order database; a saved map from another random world is
	-- not a valid substitute for either route.
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
					-- ⚠ A PENALTY, NOT A HARD CAP — deliberately. A cap is authority, and
					-- a mechanism handed a decision with no recourse can strand a settler.
					-- On a Tiny map shared with three rivals the 3..6 band
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
		-- Keep a per-site event for the settler trace and live diagnosis.
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
-- Last turn a peace deal was asked of each target, so a standing MakePeace
-- intent does not rebuild the working deal and re-open a session every turn
-- against a rival who just declined. See the `peace` arm of `applyOrder`.
local peaceAsked = {};
-- The `delegation` arm shares this table rather than declaring its own: the
-- main chunk sits at Lua's 200-register ceiling and one more file-scope local
-- fails the whole mod to compile in-game with no log line anywhere (the
-- install test guards this). Peace keys are numeric seats; delegation keys
-- are "<session name><seat>" strings, so the two can never collide.

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
		-- A completed CIVVIS pass can still leave the engine's unit prompt up.
		-- Its bounded residual unblock pass calls `orderUnits` so the turn can
		-- progress, but `orderSettler` founds immediately on the current tile.
		-- That must not overturn CIVVIS declining a site -- in particular one its
		-- loyalty forecast says will revolt. Explicit FOUND_CITY rows have already
		-- run through `applyOrders`; this only parks a settler CIVVIS left alone.
		-- Keep the normal founder for a genuine timeout fallback, where CIVVIS did
		-- not answer and the host must still be able to play the turn.
		if cfg.CivvisDecides
				and (awaiting.source == "civvis" or awaiting.source == "civvis_stale") then
			return orderIdle(unit);
		end
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

-- Park every unit the engine still calls ready, WITHOUT inventing moves.
--
-- The forfeit lever for `ENDTURN_BLOCKING_UNITS` (issue #1374). With CIVVIS
-- deciding, a ready unit after `settleTurn` is one CIVVIS deliberately left in
-- place, so the legacy `orderFor` pass must not touch it -- it once walked a
-- Settler out of a safe capital into a barbarian capture zone. `orderIdle` is
-- the position-preserving subset (skip, fortify, alert, sleep): it forfeits
-- only the unit's remaining movement, which ending the turn forfeits anyway,
-- and it removes the unit from the ready list so the engine stops re-raising
-- the blocker on every later batch.
--
-- Both of `orderUnits`'s passes, for the same reason `orderUnits` has both.
-- Pass 1 is the game's own ready query, bounded and jam-guarded, because
-- GetFirstReadyUnit offers an uncooperative unit forever. Pass 2 is the
-- roster, and skipping it would defeat the whole escalation: the query jams on
-- the FIRST unit that will not take an order, so a forfeit built on pass 1
-- alone would park one unit, leave the rest ready, and the blocker would come
-- straight back -- which is the "nineteen units, three orders given" failure
-- already recorded above.
local function parkReadyUnits(player)
	local parked = 0;
	local tries = {};
	for _ = 1, (cfg.MaxUnitOrders or 40) do
		local unit = try(function() return player:GetUnits():GetFirstReadyUnit(); end);
		if unit == nil then break; end
		local id = try(function() return unit:GetID(); end, -1);
		tries[id] = (tries[id] or 0) + 1;
		if tries[id] > 2 then break; end
		if orderIdle(unit) ~= nil then parked = parked + 1; end
	end
	-- No "is it ready?" guard. `GetMovesRemaining` is not trusted anywhere in
	-- this mod (see `orderUnits` pass 2) and an order that cannot be given
	-- fails harmlessly, so attempting it on the whole roster is strictly safer
	-- than guarding it and silently skipping everyone.
	eachUnit(player, function(unit)
		if orderIdle(unit) ~= nil then parked = parked + 1; end
	end);
	return parked;
end

-- --------------------------------------------------------- city production

-- ★★★★★ WHETHER A WAR IS BEING LOST, WHICH THE ARMY GATE COULD NOT SEE.
--
-- `wantArmy` is capped at `ArmyCap` (10 by default) and `counts.military` counts
-- UNITS. Run `civvis-20260801T065721Z` held ~40 units, so `counts.military <
-- wantArmy` was false from the early game onward and **the army block never fired
-- again** -- through a two-front war that took all six cities and ended the run in
-- defeat at turn 195. Builds after turn 140, at war and losing: 14 builders, 2
-- settlers, 1 trader, and ZERO military.
--
--     turn |  us | p1  | p2  | p3  | p4  | p5
--       74 | 185 |  -  |  57 |  -  |  -  |  29     <- we declared here, correctly
--      140 | 194 | 264 | 235 | 315 | 192 | 425
--      190 | 130 | 537 | 320 | 120 | 472 | 821
--
-- Our strength sat flat at ~190 from turn 70 to 166 -- strongest civ on the map to
-- weakest -- while every unit we ever built stayed Ancient era (warrior, slinger,
-- spearman, archer, battering ram, heavy chariot).
--
-- Unit COUNT cannot see any of that: forty warriors and forty musketmen are the same
-- number. `GetMilitaryStrength` can, it is what the game's own diplomacy ribbon shows
-- a human, and the mod already exports it for us and for every rival -- so the number
-- needed to fix this was in hand the whole time and nothing consulted it.
local function warPressure()
	local player = localPlayer();
	if player == nil then return false, 0, 0, 0; end
	local ours = try(function() return player:GetStats():GetMilitaryStrength(); end, 0) or 0;
	local diplomacy = try(function() return player:GetDiplomacy(); end, nil);
	if diplomacy == nil then return false, ours, 0, 0; end
	local pid = try(function() return player:GetID(); end, -1);
	local atWar, worst, strongestMet = false, 0, 0;
	for _, otherId in ipairs(try(function() return PlayerManager.GetAliveMajorIDs(); end, {})) do
		local warWith = try(function() return diplomacy:IsAtWarWith(otherId); end, false);
		-- A met major's strength exists in peace and war alike (the ribbon
		-- shows it to a human); the deterrence gate below needs it BEFORE any
		-- declaration, which is exactly when the at-war read above is blind.
		local met = otherId ~= pid
			and try(function() return diplomacy:HasMet(otherId); end, false);
		if warWith or met then
			local other = Players[otherId];
			local theirs = other ~= nil
				and (try(function() return other:GetStats():GetMilitaryStrength(); end, 0) or 0)
				or 0;
			if warWith then
				atWar = true;
				if theirs > worst then worst = theirs; end
			end
			if met and theirs > strongestMet then strongestMet = theirs; end
		end
	end
	return atWar, ours, worst, strongestMet;
end

local function productionFailureReasons(results)
	local reasons = {};
	pcall(function()
		local failures = results ~= nil
			and results[CityCommandResults.FAILURE_REASONS]
			or nil;
		if failures ~= nil then
			for _, reason in ipairs(failures) do
				reasons[#reasons + 1] = tostring(reason);
			end
		end
	end);
	return reasons;
end

local function chooseProduction(city, counts, nCities, turn, refused)
	refused = refused or {};
	-- Hoisted, because BOTH the expansion gate and the army cap need it and the
	-- expansion gate is ~190 lines earlier in the ladder — which is exactly how
	-- settlers came to outrank soldiers in a war that was being lost.
	local atWar, ourStrength, enemyStrength, strongestMet = warPressure();
	local losingWar = atWar and enemyStrength > ourStrength;
	-- ★★★★★ ANSWER WITH CIVVIS'S CHOICE WHEN IT HAS ONE.
	--
	-- The end-turn production prompt must be answered or the turn never ends, so this
	-- ladder cannot simply be switched off on a CIVVIS run — but it does not have to
	-- INVENT an answer when CIVVIS has already given one for this city. Anything below
	-- runs only when CIVVIS said nothing about this city this turn, or when what it
	-- asked for cannot be started.
	--
	-- ⚠ `playable` is defined below and still gates it, so a CIVVIS item the engine
	-- will not accept falls through to the ladder exactly as before. This changes WHO
	-- decides, never whether the prompt gets answered.
	local wanted = nil;
	if cfg.CivvisDecides then
		local cityId = try(function() return city:GetID(); end);
		if cityId ~= nil then
			-- A direct choice owns the current queue.  If that queue was already
			-- finishing when the board was exported, the Rust decider also sent a
			-- deferred next-build lease under a string key in this same table (the
			-- Lua 5.1 main chunk is at its 200-local ceiling). Consume that lease
			-- only after the host raises the production blocker, never while the old
			-- item is still running.
			wanted = civvisBuild[cityId]
				or civvisBuild[tostring(cityId) .. ":next"];
		end
	end
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
		local ok, can, results = pcall(function()
			-- Three-arg form returns (canStart, results); take the verdict only.
			return city:GetBuildQueue():CanProduce(row.Hash, false, true);
		end);
		if ok and can == true then return row; end
		refused[name] = true;
		return nil, productionFailureReasons(results);
	end

	local ladder = {};
	-- Era-proof land forces. The old fixed Warrior/Spearman/Swordsman list becomes
	-- entirely obsolete, after which a losing modern war silently falls through to a
	-- Builder. Firaxis's own unit table is the authority on the current ruleset;
	-- `playable` below still applies every city-specific prerequisite and resource rule.
	local landUnits = {};
	for row in GameInfo.Units() do
		if row.Domain == "DOMAIN_LAND" and (row.Combat or 0) > 0 then
			landUnits[#landUnits + 1] = {
				name = row.UnitType,
				capture = (row.RangedCombat or 0) <= 0 and (row.Bombard or 0) <= 0,
				strength = math.max(row.Combat or 0, row.RangedCombat or 0,
				                    row.Bombard or 0),
			};
		end
	end
	table.sort(landUnits, function(a, b)
		if a.capture ~= b.capture then return a.capture; end
		if a.strength ~= b.strength then return a.strength > b.strength; end
		return a.name < b.name;
	end);
	local function pushLandUnits(reason)
		for _, unit in ipairs(landUnits) do
			ladder[#ladder + 1] = { unit.name, reason };
		end
	end
	local function pushRangedLandUnits(reason)
		for _, unit in ipairs(landUnits) do
			if not unit.capture then
				ladder[#ladder + 1] = { unit.name, reason };
			end
		end
	end
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
	-- ★★★★★ AND NOT WHILE A WAR IS BEING LOST. FOURTH instance of this file's
	-- recurring class, after `ArmyCap`, the production floor and
	-- `MaxProductionPasses`: a gate that is right in peacetime and wrong in the one
	-- state that ends runs.
	--
	-- The "one defender per city" floor above does not catch it. Measured on live run
	-- `civvis-20260801T141601Z`: at turn 78-112, ONE city left, ten units, and
	-- `floorNeeded` was 1 — so the gate passed and the ladder built **five settlers
	-- and two builders** against ONE archer, while player 2 went from 146 to 203
	-- military and took Aquileia at t97 and Ostia at t101.
	--
	-- Settlers built in a losing war are worse than idle: they spend the production
	-- that would have defended, and then they are captured. The comment above already
	-- says "expansion that cannot be held is not expansion" — this is the condition
	-- that sentence was missing.
	--
	-- ⚠ Gated on being OUTMATCHED, not merely at war. A war we are winning is exactly
	-- when taking more ground is right, and `warPressure` already draws that line.
	if (nCities + counts.settler) < (cfg.CityTarget or 6)
			-- ⚠⚠⚠ A STRANDED SETTLER DOES NOT HOLD THIS GATE SHUT. "In flight"
			-- means walking to a site; a settler that has not moved in twelve
			-- turns while holding full movement is not walking anywhere, and
			-- with `SettlersInFlight = 1` its mere existence stops the empire
			-- ordering another one FOREVER. Run `civvis-20260803T231038Z`
			-- ordered no settler between turns 51 and 134 for exactly this
			-- reason and finished four cities behind; fleet-wide 38% of runs
			-- park a settler for 15+ consecutive turns at full movement.
			--
			-- ⚠ This does NOT rescue the stranded settler — see
			-- `settlerIsStranded`. It stops one stuck unit from also costing
			-- every future one. The cap still binds on settlers that are
			-- genuinely walking, which is the failure the cap was written for
			-- (seventeen settlers, two cities).
			and (counts.settler - counts.stranded_settler)
				< (cfg.SettlersInFlight or 1)
			and defenders >= floorNeeded
			and not losingWar
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
	-- ★★★★★ AND THE CAP MUST LIFT WHILE A WAR IS BEING LOST. See `warPressure`.
	--
	-- The cap above is right for the problem it was written for -- an army target that
	-- grew with the empire forever and starved development. It is wrong in the one
	-- state that ends runs: outmatched, at war, and building nothing that fights.
	-- Keeping the target two ABOVE the current count means the army block always fires
	-- while we are losing, and the ordinary cap resumes the moment we are not, so this
	-- cannot produce a runaway peacetime army.
	--
	-- ⚠ Deliberately gated on STRENGTH, not on being at war. A war we are winning does
	-- not need this, and the whole defect was a gate that could not tell those apart.
	if losingWar then
		wantArmy = math.max(wantArmy, (counts.military or 0) + 2);
	end
	-- ★★★ AND `losingWar` CANNOT SEE PEACETIME HOPELESSNESS — the wars that end
	-- runs are DECLARED ON US. `warPressure`'s at-war read returns 0 before any
	-- declaration, so the lift above arms only once the collapse has started
	-- (run 220954Z: Mali declared at 894 against our 481, six cities lost; the
	-- Aug-15..17 tail spends its Recovery turns in exactly this shape). The
	-- Rust seat gained a peacetime deterrence floor (#1297) but this ladder
	-- still decides roughly a quarter of production, and its army row stayed
	-- blind. Deterrence asks the strongest MET major's strength, which exists
	-- in peace and war alike: below HALF of it — the battering ram gate's own
	-- readiness bar — the army grows two units at a time. Unlike the wartime
	-- lift this one stays UNDER ArmyCap: deterrence wants a standing army, not
	-- a war footing (the Rust twin's PEACETIME_DETERRENCE_CEILING draws the
	-- same line), so it cannot reproduce the runaway army the cap was written
	-- to stop. Withholdable per run: `civ6_play --no-peace-deterrence`.
	if cfg.PeaceDeterrence and not atWar and strongestMet > 0
			and ourStrength * 2 < strongestMet then
		wantArmy = math.max(wantArmy, math.min((counts.military or 0) + 2,
			cfg.ArmyCap or ((cfg.WarArmy or 4) + 6)));
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
	-- ★★★★★ AND NOT WHILE BEING OVERRUN. A ram BREAKS a city; it cannot hold one.
	--
	-- FIFTH instance of this file's recurring class, after `ArmyCap`, the production
	-- floor, `MaxProductionPasses` and `expand`: a gate that is right when attacking
	-- and wrong in the one state that ends runs.
	--
	-- `UNIT_BATTERING_RAM` is a SUPPORT unit with no combat strength of its own — it
	-- only boosts an adjacent melee unit attacking a wall. Measured on live run
	-- `civvis-20260801T175955Z` (Egypt), builds from turn 60 while losing three of
	-- four cities:
	--
	--     siege 23    improve 11    ranged 7    army 7    civvis 4
	--
	-- **43% of wartime production**, and at the end ZERO rams were alive: all 23 were
	-- built, sent at the enemy and destroyed, while the empire went from four cities
	-- to one. The cap works (`counts.siege < SiegeUnits`); it just refills a hole.
	--
	-- The gate asked only whether a war TARGET exists, never whether WE are the ones
	-- under siege. Rams belong in an offensive, and an offensive is not what a seat
	-- losing its cities is conducting.
	--
	-- ⚠ AND `losingWar` CANNOT SEE PEACETIME HOPELESSNESS. `warPressure` reads
	-- strength only from players we are AT WAR with, so before any declaration it
	-- returns 0 and the guard above is inert. Run `civvis-20260801T211015Z`
	-- (Indonesia): 25 rams built t69–t210, 20 of them BEFORE the t186 war, at
	-- military strength 4 against a target above 1000 — 42% of the game's whole
	-- production spent refilling a siege train that died faster than it could
	-- march. A siege train serves an offensive, and an offensive needs an army
	-- near the target's class — so ask the TARGET's strength, which exists in
	-- peace and war alike, and require half of it before spending on rams.
	local targetStrength = warTarget ~= nil and (try(function()
		return Players[warTarget.player]:GetStats():GetMilitaryStrength();
	end, 0) or 0) or 0;
	-- ★★★★★ ON A CIVVIS SEAT, A WAR FOOTING NEEDS A WAR. SIXTH instance of this
	-- file's recurring class — a gate that is right when attacking and wrong in
	-- the state that ends runs — except this one is wrong at PEACE. `warTarget`
	-- is "who we would fight", and `findWarTarget` returns somebody the moment
	-- any major is met, so on a CIVVIS run — where `MakeWar` never fires because
	-- the Rust decider owns war policy — the ram entry and the ranged floor
	-- below have been a PERMANENT war footing, not a war-opening one.
	--
	-- Measured on run civvis-20260818T212725Z (Trajan, Settler, diplomacy lane):
	-- at peace from t156 to the end, yet 41 `ranged` build orders across the run
	-- and 23 UNIT_MACHINE_GUN starts — and ZERO machine guns ever alive in any
	-- state export, because the Rust decider displaces the foreign item a turn
	-- or two later, the floor reads ALIVE units, and the loop re-fires at every
	-- production prompt forever. Runs 165035Z (29 of 54 ranged orders before its
	-- t130 war) and 182702Z show the same signature; all three finished 450+
	-- behind. The same day's WIN (155500Z, +472, both ladder records) is what
	-- these prompts buy when they fall through: `develop` fired 72 times and
	-- built 22 campuses.
	--
	-- So on a CIVVIS seat these two entries require the war to EXIST — `atWar`
	-- from `warPressure`, hoisted above, true only against a living major. The
	-- legacy ladder (no Rust decider) keeps the pre-war build-up: there the mod
	-- itself declares, and arming BEFORE its own declaration is the design.
	-- `PeacetimeWarFloors` restores the old behaviour as the control arm, and
	-- rides in the run summary's `mod_arms` so a batch says which side it ran.
	local warFooting = atWar or not cfg.CivvisDecides or cfg.PeacetimeWarFloors;
	if warTarget ~= nil and warFooting and not losingWar
			and ourStrength * 2 >= targetStrength
			and (counts.siege or 0) < (cfg.SiegeUnits or 4) then
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
	-- ⚠ `warFooting` above: on a CIVVIS seat this floor exists only while a war
	-- does. It reads ALIVE units and its request can be displaced by the Rust
	-- decider before completing, so at peace it re-fires unsatisfiably forever.
	if warTarget ~= nil and warFooting
			and (counts.ranged or 0) < (cfg.RangedFloor or 3) then
		pushRangedLandUnits("ranged");
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
		pushLandUnits("defend");
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
	local DEVELOP = { "DISTRICT_CAMPUS",
	                  -- ⚠⚠ FIFTH APPEARANCE OF THIS CLASS, and this time nothing was
	                  -- misspelled — the entry was simply never written. This list had
	                  -- **no `DISTRICT_AQUEDUCT`, no `DISTRICT_NEIGHBORHOOD` and no
	                  -- `BUILDING_SEWER`**, so the rung that decides more builds than
	                  -- any other could not raise a city's housing ceiling at all.
	                  --
	                  -- Housing is what caps population, and population is what science
	                  -- IS (~1.16 science per citizen). `Game::housing_growth_mult`
	                  -- halves growth below headroom 2 and quarters it below 1.
	                  -- Measured over 12,969 host-exported city-turns across the 18
	                  -- live runs carrying `GetHousing()`: median headroom **1**,
	                  -- **71.2% of city-turns below the break-even**, mean growth
	                  -- multiplier **0.515**, and 87.9% throttled at pop >= 8.
	                  --
	                  -- And this rung built ZERO of the repair. Of 3,708 live builds
	                  -- (`events.jsonl` `kind:build`), by deciding program:
	                  --
	                  --   Aqueduct  5 -- civvis 5, ladder **0**
	                  --   Bath      4 -- civvis 4, ladder **0**
	                  --   Neighborhood 0 -- nobody, ever
	                  --   Library  59 -- civvis 5, **develop 54**
	                  --   University 119 -- civvis 1, **develop 118**
	                  --
	                  -- So the science BUILDINGS were almost entirely this rung's while
	                  -- the housing districts were entirely CIVVIS's, at a 20.8/79.2
	                  -- split. #1087 repairs CIVVIS's side; this repairs this one.
	                  --
	                  -- ⚠⚠ BUT DATE THAT SPLIT BEFORE COSTING ANYTHING FROM IT. Every
	                  -- figure above comes from runs BEFORE `civvis-20260802T140355Z`.
	                  -- `kind:build` is emitted only from this file's own production
	                  -- sweep, and that sweep is skipped when a city already has current
	                  -- production — so once CIVVIS began applying `produce` through the
	                  -- orders channel the event went silent. It is **zero in all 60+
	                  -- runs since**, including all 19 that carry the housing export,
	                  -- while those runs log 3,658 order-turns at `source: civvis` and
	                  -- 2,754 applied `produce` orders.
	                  --
	                  -- So the CURRENT split is UNMEASURED, and this rung is most likely
	                  -- a FALLBACK now rather than the majority decider. That does not
	                  -- make the omission harmless — the fallback still fires whenever
	                  -- CIVVIS's own choice is unplayable, which those same 19 runs
	                  -- record **482** times — but this is defence in depth, not the
	                  -- main lane, and nobody should price it as the main lane until
	                  -- `kind:build` (or an equivalent) reports again.
	                  --
	                  -- ⚠ PLACED AFTER THE CAMPUS, DELIBERATELY. The Campus is the
	                  -- larger funnel gap — 49 of 100 live end-of-game cities were
	                  -- never ordered one, at a median pop of 7 — and the ladder takes
	                  -- the FIRST playable entry, so putting housing above it would
	                  -- trade one gap for another. It goes above the Library because a
	                  -- Library's science is a flat bonus while population compounds,
	                  -- and the Aqueduct is the cheapest entry in this list (36).
	                  --
	                  -- ⚠ It cannot preempt an early Campus in any case: the Aqueduct
	                  -- needs `engineering` and the Campus only `writing`, so before
	                  -- Engineering `playable` skips this line and the ladder moves on.
	                  -- That is also why placing it high is low-risk rather than a
	                  -- re-ranking of the science chain.
	                  --
	                  -- ⚠⚠ KNOWN LIMITATION, DECLARED RATHER THAN DISCOVERED LATER:
	                  -- this list names BASE districts, and a civilization whose unique
	                  -- REPLACES one cannot build the base type at all — `CanProduce`
	                  -- says no and the ladder falls through, silently, for that civ.
	                  -- It affects the entries added here and the ones already present
	                  -- equally:
	                  --
	                  --   Bath (Rome)          replaces Aqueduct
	                  --   Mbanza (Kongo)       replaces Neighborhood
	                  --   Seowon (Korea)       replaces Campus
	                  --   Observatory (Maya)   replaces Campus
	                  --   Acropolis (Greece)   replaces Theater
	                  --
	                  -- So Rome gets no housing from this rung and Korea no Campus,
	                  -- exactly as before this change. CIVVIS's own side resolves this
	                  -- with `civ_district`; the ladder has no equivalent and giving it
	                  -- one is a separate change with its own measurement, because it
	                  -- moves the Campus and Theater lines too. Fixing it here would
	                  -- mean re-ranking families this PR is not measuring.
	                  "DISTRICT_AQUEDUCT",
	                  "BUILDING_LIBRARY",
	                  "BUILDING_UNIVERSITY", "BUILDING_RESEARCH_LAB",
	                  -- The late-game housing pair, below the science chain because
	                  -- both arrive with civics this agent reaches long after the
	                  -- Campus is settled. `BUILDING_SEWER` is +2 housing on
	                  -- `sanitation`; `DISTRICT_NEIGHBORHOOD` is 2-6 on `urbanization`,
	                  -- scaled by the site's Appeal. Neither has ever been built in a
	                  -- live run, and a Sewer stands in only 19 of 100 end-of-game
	                  -- cities.
	                  "DISTRICT_NEIGHBORHOOD", "BUILDING_SEWER",
	                  "DISTRICT_THEATER", "BUILDING_AMPHITHEATER",
	                  -- ⚠⚠ `BUILDING_MUSEUM_ART` / `BUILDING_MUSEUM_ARTIFACT`, NOT
	                  -- `BUILDING_ART_MUSEUM` / `BUILDING_ARCHAEOLOGICAL_MUSEUM`.
	                  -- Civilization VI has neither of the latter — the shipped rows
	                  -- read subject-last. Exactly the defect the `BUILDING_WALLS`
	                  -- comment below this list describes, sitting three lines above
	                  -- it and unnoticed, because a ladder entry that cannot fire is
	                  -- invisible: the ladder just moves to the next line.
	                  --
	                  -- ⚠ THIS RUNG IS NOT A CORNER. Across 50 live runs the harness
	                  -- ladder decides **2,935 of 3,708 builds (79%)** and `develop`
	                  -- is its largest rung at 879; CIVVIS itself gets 773. So both
	                  -- roads to a Museum were closed by the same class of bug and
	                  -- #959 only fixed CIVVIS's. Museums stand in **0 of 119** live
	                  -- end-of-game cities, Broadcast Centres in 0, and the Great
	                  -- People that need those slots rot: Artist 9 activated against
	                  -- 67 idle, Musician **0 against 31**, while Merchant runs 89%.
	                  --
	                  -- ⚠ ART BEFORE ARTIFACT, and the order is load-bearing. They
	                  -- are identical in cost and yield; only the slot kind differs.
	                  -- An Artifact slot needs an Archaeologist, which no live run has
	                  -- **ever** built, while 67 Great Artists sit idle for want of
	                  -- the Art slot. The ladder takes the first entry that is
	                  -- playable, so listing Art first spends the 290 on the museum
	                  -- whose slots can actually fill.
	                  "BUILDING_MUSEUM_ART", "BUILDING_MUSEUM_ARTIFACT",
	                  "BUILDING_BROADCAST_CENTER",
	                  "DISTRICT_COMMERCIAL_HUB", "BUILDING_MARKET",
	                  "BUILDING_BANK", "BUILDING_STOCK_EXCHANGE",
	                  "DISTRICT_HARBOR", "BUILDING_LIGHTHOUSE",
	                  "BUILDING_SHIPYARD", "BUILDING_SEAPORT",
	                  "DISTRICT_INDUSTRIAL_ZONE", "BUILDING_WORKSHOP",
	                  "BUILDING_FACTORY", "DISTRICT_HOLY_SITE",
	                  "BUILDING_WATER_MILL",
	                  -- ⚠ `BUILDING_WALLS`, not `BUILDING_ANCIENT_WALLS`. Civilization
	                  -- VI has no such type: grepping every shipped Asset for
	                  -- `BUILDING_ANCIENT_WALLS` returns exactly ONE file, this mod,
	                  -- while `BUILDING_WALLS` appears in Firaxis's own DLC data. So
	                  -- the development rung's only defensive entry could never be
	                  -- built, on any turn of any run, and nothing said so — the
	                  -- ladder simply moved to the next line.
	                  --
	                  -- Same class as the floor's obsolete units in #748: a list
	                  -- entry that cannot fire is invisible unless something asks
	                  -- whether the engine has ever accepted it.
	                  "BUILDING_WALLS" };
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
		pushLandUnits("army");
	end
	-- The same list below the army on the other two turns in three, so development is
	-- still preferred over the always-available floor when the army IS satisfied.
	if not devFirst then pushDevelop(); end
	-- Always-available floor. A city with an empty queue and nothing it can
	-- build is a permanent end-turn blocker; a project never is.
	--
	-- ★★★★★ AND IT DEGENERATED INTO A BUILDER FACTORY. Measured on run
	-- `civvis-20260801T065721Z`, which lost all six cities and the game:
	--
	--     floor:   UNIT_BUILDER 33      <-- and NOTHING else, all game
	--     improve: UNIT_BUILDER 9
	--
	-- Forty-four builders for a six-city empire that wanted five
	-- (`BuilderPerCity` 0.8). The floor reached its FIRST TWO entries zero times:
	-- the campus project needs a Campus in THAT city and most cities
	-- had none, and `UNIT_WARRIOR` and `UNIT_SLINGER` go OBSOLETE mid-game and stop
	-- being buildable at all. `UNIT_BUILDER` never obsoletes, so as the eras pass
	-- every fallback above it evaporates and the floor becomes "build a builder,
	-- forever" -- including on the turns a war was being lost.
	--
	-- ⚠ Same class as the army cap one screen up: correct in the Ancient era and
	-- silently wrong later. When a list is a fallback, check what remains PLAYABLE
	-- at turn 150, not what is playable at turn 1.
	--
	-- The army rung above is now derived from the live unit table and is present only
	-- while the bounded target is short. The floor therefore does not need a second,
	-- unconditional military list -- the exact escape hatch that produced 85 units.
	-- ⚠ `PROJECT_ENHANCE_DISTRICT_CAMPUS`, not `PROJECT_CAMPUS_RESEARCH_GRANT`.
	-- Civilization VI HAS NO SUCH PROJECT. Grepping every shipped Asset for
	-- `PROJECT_CAMPUS_RESEARCH_GRANT` returns exactly one file — this mod — while
	-- Firaxis's own district projects are `PROJECT_ENHANCE_DISTRICT_<DISTRICT>`.
	--
	-- So the floor's FIRST entry, the one that exists to guarantee a city always has
	-- something to build, has never been buildable on any turn of any run. That is
	-- the real reason the floor fell through to `UNIT_BUILDER` every time — #748
	-- attributed it to "needs a Campus", which was a guess about a name that does
	-- not resolve at all.
	--
	-- One project is offered for every ordinary specialty district. No single project
	-- is universal, but a developed city can now convert production into its own yield
	-- instead of falling from a missing Campus project into another military unit.
	for _, name in ipairs({ "PROJECT_ENHANCE_DISTRICT_CAMPUS",
	                        "PROJECT_ENHANCE_DISTRICT_THEATER",
	                        "PROJECT_ENHANCE_DISTRICT_COMMERCIAL_HUB",
	                        "PROJECT_ENHANCE_DISTRICT_HARBOR",
	                        "PROJECT_ENHANCE_DISTRICT_INDUSTRIAL_ZONE",
	                        "PROJECT_ENHANCE_DISTRICT_HOLY_SITE",
	                        "PROJECT_ENHANCE_DISTRICT_ENCAMPMENT" }) do
		ladder[#ladder + 1] = { name, "floor" };
	end
	-- Gated exactly like the `improve` rung, which was the only thing holding the
	-- builder count down and which the floor bypassed entirely.
	if counts.builder < math.max(1, nCities * (cfg.BuilderPerCity or 0.8)) then
		ladder[#ladder + 1] = { "UNIT_BUILDER", "floor" };
	end
	-- ⚠ LAST RESORT, deliberately ungated. Everything above can be unplayable --
	-- no Campus, every military tier obsolete or missing its strategic resource --
	-- and a city with nothing queued blocks the turn permanently, which is worse
	-- than a surplus builder. This is the guarantee the original list existed for;
	-- what changed is that it is now the last line rather than the fourth.
	ladder[#ladder + 1] = { "UNIT_BUILDER", "floor" };

	-- CIVVIS FIRST. Its choice for this city, this turn, gated by the same `playable`
	-- the ladder uses — so an item the engine will not start still falls through to
	-- the ladder below and the prompt is still answered. Reported with its own reason
	-- so the `build` events say which program decided, and the production fraction in
	-- `civ6_civvis_status.py` can be read honestly.
	if wanted ~= nil then
		local row, reasons = playable(wanted);
		if row ~= nil then return wanted, row, "civvis"; end
		-- ★★★★★ SAY WHAT CIVVIS ASKED FOR AND COULD NOT HAVE.
		--
		-- When `playable` refuses CIVVIS's choice the ladder silently takes the turn,
		-- and until now NOTHING recorded what the choice was. The `build` event says
		-- the ladder decided; it cannot say what it overrode.
		--
		-- That is the whole of the open question. On run civvis-20260801T065721Z only
		-- **16 of 97 builds** were CIVVIS's -- floor 21, develop 20, grow 10, improve
		-- 9, expand 8 -- and no telemetry anywhere could name a single item CIVVIS
		-- wanted instead. The same anonymity around `no_params` hid one district for
		-- an entire project until the refusal carried its verb.
		--
		-- ⚠ `item`, not `kind`: `emit` claims `kind`, `ctx` and `run`, and a payload
		-- field named `kind` is overwritten before the line is written. That already
		-- cost this file one blind instrument.
		if not refused[wanted] and reasons ~= nil and #reasons > 0 then
			emit("civvis_build_unplayable", {
				turn = turn,
				city = try(function() return city:GetID(); end, -1),
				item = tostring(wanted),
				reasons = reasons,
			});
		end
	end
	for _, entry in ipairs(ladder) do
		local row = playable(entry[1]);
		if row ~= nil then return entry[1], row, entry[2]; end
	end
	return nil, nil, nil;
end

-- ★★★★★ WHERE A DISTRICT GOES. A district BUILD carries a plot, and ours never did.
--
-- `CityManager.RequestOperation(city, BUILD, params)` with only
-- `PARAM_DISTRICT_TYPE` set returns without throwing and starts nothing, exactly
-- like the missing `PARAM_INSERT_MODE` documented in `buildParams` below — "the
-- order was never malformed in a way Lua could see; it was missing a parameter the
-- engine requires. That is the whole reason no run ever built anything."
--
-- Measured on run civvis-20260731T163924Z: the capital was ordered to build
-- DISTRICT_GOVERNMENT on 60 turns between t46 and t128, every one `applied: true`,
-- and at t130 it still had no district and three buildings. The city reported
-- `producing: DISTRICT_GOVERNMENT` the whole time — the queue took the item, the
-- engine never placed it, so the work never happened and CIVVIS re-ordered it
-- forever.
--
-- The engine will say which plots are legal if asked, which is what the shipped
-- placement UI does (`StrategicView_MapPlacement.lua`): probe with the district
-- type, read `CityOperationResults.PLOTS`, and send one back as PARAM_X/PARAM_Y.
--
-- ⚠ Returns nil rather than guessing a plot. An illegal plot is another silent
-- no-op, and a district we cannot place should fall through to the ladder so the
-- city builds something real this turn.
local function productionPlot(city, param, hash, requestedX, requestedY)
	local probe = {};
	probe[param] = hash;
	local results = try(function()
		return CityManager.GetOperationTargets(city, CityOperationTypes.BUILD, probe);
	end);
	-- ⚠ The SECOND return is how many plots the engine offered, and it is the whole
	-- difference between "this wonder is gone from the world" and "not on that tile".
	-- Zero is a real answer and must be distinguishable from the nil-results case, so
	-- both early exits return an explicit 0 rather than falling off the end.
	if results == nil then return nil, 0; end
	local plots = results[CityOperationResults.PLOTS];
	if plots == nil then return nil, 0; end
	local offered = 0;
	local first = nil;
	-- ★★★★★ AND KEEP THE COORDINATES. This loop already asks the engine for every
	-- plot it would accept, reads x and y off each one, and throws all of them
	-- away but the count. That count was enough to prove the district is
	-- placeable somewhere in this city; it can never say WHERE, so CIVVIS has no
	-- way to stop naming a plot the engine will not take.
	--
	-- It does not stop. Measured on run civvis-20260811T212652Z: 56
	-- `build_no_plot` events in 232 turns and **55 of them one pair** — a single
	-- Commercial Hub in one city, refused fifty-five times, with the engine
	-- offering plots every time. #1571 bounds how often that repeats; only the
	-- coordinates can end it.
	--
	-- ⚠ Capped. A large city can offer many plots, this rides in an event on
	-- every refusal, and the ledger is read far more often than it is written.
	-- The first few are what a chooser needs.
	local offeredPlots = {};
	-- Taken by iteration, not `plots[1]`: the shipped UI counts these with
	-- `table.count`, so the result is not promised to be a dense array.
	for _, plotIndex in pairs(plots) do
		local plot = try(function() return Map.GetPlotByIndex(plotIndex); end);
		if plot ~= nil then
			local px = try(function() return plot:GetX(); end, -1);
			local py = try(function() return plot:GetY(); end, -1);
			if px >= 0 and py >= 0 then
				offered = offered + 1;
				if #offeredPlots < (cfg.OfferedPlotsReported or 8) then
					offeredPlots[#offeredPlots + 1] = { x = px, y = py };
				end
				if requestedX ~= nil and requestedY ~= nil
						and px == requestedX and py == requestedY then
					return { x = px, y = py }, offered, offeredPlots;
				end
				if first == nil then first = { x = px, y = py }; end
			end
		end
	end
	-- A direct CIVVIS order names a plot. Substituting another legal plot would
	-- actuate a different decision; only the emergency ladder may take the first.
	if requestedX ~= nil or requestedY ~= nil then return nil, offered, offeredPlots; end
	return first, offered, offeredPlots;
end

-- ★ A PROBE, NOT A MAPPING — the measurement that decides whether repair can ship.
--
-- Civilization VI has no PROJECT_REPAIR_<district> (the only repair project in the
-- shipped Assets is PROJECT_REPAIR_OUTER_DEFENSES); a pillaged district is repaired
-- by BUILDING the district again, flagged as a repair by the engine. What kept that
-- translation unshipped is one unverified question: does GetOperationTargets for a
-- city that ALREADY HAS the pillaged district offer the existing district's plot
-- (a repair) or fresh sites (a NEW district — expensive and wrong)? 52 repair asks
-- were discarded on run civvis-20260801T184324Z while an Encampment sat pillaged.
--
-- This emits what the engine offers, once per city+district per run, and changes
-- no order. When live runs show the existing plot among `offered`, the mapping in
-- the produce arm is one line; if they show only fresh sites, it never ships.
local probedRepairs = {};
local function probeDistrictRepair(city, districtName, asked, turn)
	local key = tostring(try(function() return city:GetID(); end, -1)) .. districtName;
	if probedRepairs[key] then return; end
	probedRepairs[key] = true;
	local row = try(function() return GameInfo.Districts[districtName]; end);
	if row == nil then return; end
	local have, hx, hy, pillaged = false, -1, -1, false;
	try(function()
		local districts = city:GetDistricts();
		for _, d in districts:Members() do
			if d:GetType() == row.Index then
				have = true;
				hx = try(function() return d:GetX(); end, -1);
				hy = try(function() return d:GetY(); end, -1);
				pillaged = try(function()
					return districts:IsPillaged(row.Index);
				end, false);
			end
		end
	end);
	local offered = {};
	try(function()
		local probe = {};
		probe[CityOperationTypes.PARAM_DISTRICT_TYPE] = row.Hash;
		local results = CityManager.GetOperationTargets(
			city, CityOperationTypes.BUILD, probe);
		local plots = results and results[CityOperationResults.PLOTS];
		if plots ~= nil then
			for _, plotIndex in pairs(plots) do
				if #offered >= 12 then break; end
				local plot = try(function() return Map.GetPlotByIndex(plotIndex); end);
				if plot ~= nil then
					offered[#offered + 1] = {
						x = try(function() return plot:GetX(); end, -1),
						y = try(function() return plot:GetY(); end, -1),
					};
				end
			end
		end
	end);
	emit("repair_probe", {
		turn = turn,
		city = try(function() return city:GetID(); end, -1),
		district = districtName, asked = asked,
		has_district = have, at_x = hx, at_y = hy, pillaged = pillaged,
		offered = offered,
	});
end

-- The religion a city actually follows, by type name, and the one converting it.
--
-- ⚠ Names, not indices. An index is meaningless on the far side of the bridge and
-- would have to be re-resolved against a table the mirror does not carry -- the
-- same reason `producing` ships a name rather than the raw hash it used to send.
local function cityReligion(city)
	local id = try(function() return city:GetReligion():GetMajorityReligion(); end, -1);
	if id == nil or id < 0 then return nil; end
	local row = GameInfo.Religions[id];
	return row ~= nil and row.ReligionType or nil;
end

local function cityNextReligion(city)
	local id = try(function() return city:GetReligion():GetNextReligion(); end, -1);
	if id == nil or id < 0 then return nil; end
	local row = GameInfo.Religions[id];
	return row ~= nil and row.ReligionType or nil;
end

local function buildParams(row, city, requestedX, requestedY)
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
		local building = GameInfo.Buildings[row.Type];
		local alreadyPlaced = try(function()
			return city:GetBuildQueue():HasBeenPlaced(row.Hash);
		end, false);
		if building ~= nil and building.IsWonder and not alreadyPlaced then
			local where, offered, offeredPlots = productionPlot(city,
				CityOperationTypes.PARAM_BUILDING_TYPE, row.Hash,
				requestedX, requestedY);
			if where == nil then
				-- ★★★★★ SAY WHY. `build_no_plot` named the wonder and the city but
				-- never the reason, and the two reasons want OPPOSITE responses.
				--
				-- Measured over 45 live runs to 2026-08-03: **726 wonder refusals, and
				-- every single one named a plot that satisfies the wonder's own rule**
				-- as the shipped database states it — all 264 Great Bath asks were on
				-- Floodplains, all 160 Hanging Gardens asks on a river, all 78
				-- Stonehenge asks on legal terrain. Not one wonder was built in any
				-- run. So the plot is not the problem, and with only `x`/`y` in the
				-- event there was no way to learn what was.
				--
				-- The two cases:
				--   `offered == 0` — the engine has no target at all. A wonder is
				--     unique in the world, and a rival's cities export no buildings
				--     and no wonders, so CIVVIS cannot see that someone else already
				--     finished it. That block belongs to the WHOLE EMPIRE and forever.
				--   `offered > 0`  — the engine has ground, just not ours. That is a
				--     placement disagreement in one city and must not stop the empire.
				--
				-- `reasons` comes off the same `CanProduce(hash, false, true)` the
				-- ladder's `playable` uses, which is where "has already been built"
				-- is spelled out in words.
				local reasons = nil;
				local ok, _, results = pcall(function()
					return city:GetBuildQueue():CanProduce(row.Hash, false, true);
				end);
				if ok then reasons = productionFailureReasons(results); end
				emit("build_no_plot", {
					city = try(function() return city:GetID(); end, -1),
					building = row.Type or tostring(row.Hash),
					-- Same reason as the district emit below: without a turn,
					-- every filter downstream is a no-op.
					turn = try(function() return Game.GetCurrentGameTurn(); end, -1),
					x = requestedX, y = requestedY,
					offered = offered or 0,
					-- A positive offer names the host-valid alternatives for this
					-- specific wonder. Keep those coordinates just as we do for a
					-- district so the next CIVVIS decision can choose one instead of
					-- suppressing a usable Great Engineer activation path.
					offered_plots = offeredPlots,
					reasons = reasons,
				});
				return nil;
			end
			params[CityOperationTypes.PARAM_X] = where.x;
			params[CityOperationTypes.PARAM_Y] = where.y;
		end
	elseif row.Kind == "KIND_DISTRICT" then
		params[CityOperationTypes.PARAM_DISTRICT_TYPE] = row.Hash;
		-- The shipped ProductionPanel's ZoneDistrict path asks
		-- BuildQueue:HasBeenPlaced first.  A founded-but-incomplete district is
		-- resumed with only its type and insert mode; it is deliberately absent
		-- from GetOperationTargets because its plot is no longer a legal *new*
		-- placement. Probing anyway refused every attempt to resume that work as
		-- `no_params_DISTRICT_*` (live turns 58 and 73 of run 004000).
		local alreadyPlaced = try(function()
			return city:GetBuildQueue():HasBeenPlaced(row.Hash);
		end, false);
		local where, offered, offeredPlots = nil, 0, nil;
		if not alreadyPlaced then
			where, offered, offeredPlots = productionPlot(city,
				CityOperationTypes.PARAM_DISTRICT_TYPE, row.Hash,
				requestedX, requestedY);
		end
		if not alreadyPlaced and where == nil then
			-- ★★★★ NAME THE DISTRICT AND THE CITY. This used to send
			-- `row.DistrictType`, which does not exist on a `GameInfo.Types` row, so
			-- every one of these arrived as a bare hash -- 39 of them on run
			-- civvis-20260801T024428Z, all reading `-1743686858`, which no reader could
			-- turn back into a name.
			--
			-- ⚠ The CITY matters as much as the name. Civilization VI offers no plot
			-- either because the district is impossible ANYWHERE (Government Plaza is
			-- one per civilization) or because THIS city has no room for it, and those
			-- want opposite responses. Without the city id a consumer can only block
			-- globally, which would stop CIVVIS building Campuses everywhere the first
			-- time one city ran out of space.
			-- ⚠ The id comes off the live city object, not from `subject`: this is a
			-- top-level function and `subject` belongs to the order handler. The scope
			-- checker caught that, which is exactly what it is for.
			-- ⚠ `offered` separates the two reasons this comment already names. A
			-- Government Plaza that exists elsewhere in the empire offers ZERO plots
			-- in every city; a Campus in a full city offers zero here and plenty next
			-- door. Measured over 45 runs: 1,353 district refusals, and **1,295 of
			-- them named no plot at all**, so the count is the only signal available.
			local reasons = nil;
			local ok, _, results = pcall(function()
				return city:GetBuildQueue():CanProduce(row.Hash, false, true);
			end);
			if ok then reasons = productionFailureReasons(results); end
			emit("build_no_plot", {
				city = try(function() return city:GetID(); end, -1),
				district = row.Type or tostring(row.Hash),
			-- ⚠⚠⚠ THE TURN, WITHOUT WHICH EVERY FILTER ON THIS EVENT IS A NO-OP.
			-- `buildParams` is a top-level function and takes no turn, so this
			-- event has never carried one — and two readers silently depended on
			-- it. `refused_no_plot_through`'s replay bound (`event.turn > limit`)
			-- read the missing field as 0 and therefore never excluded anything,
			-- and #1571's staleness window read it as 0 too, which made every
			-- refusal look ancient and blocked NOTHING. Measured on run
			-- civvis-20260811T230324Z, the first to carry #1571: 40 build_no_plot
			-- events in 131 turns, `0` of them with a turn, and one Campus asked
			-- for forty times — the exact loop the TTL was meant to bound.
			-- Asked of the engine rather than threaded through the signature,
			-- which is what four other emitters in this file already do.
			turn = try(function() return Game.GetCurrentGameTurn(); end, -1),

				offered = offered or 0,
				-- ⚠ WHERE, not just how many. `offered` proves the district is
				-- placeable in this city and can never say where, so CIVVIS goes
				-- on naming the plot the engine refuses -- 55 times for one
				-- Commercial Hub on run civvis-20260811T212652Z.
				offered_plots = offeredPlots,
				reasons = reasons,
				x = requestedX, y = requestedY,
			});
			return nil;
		end
		if where ~= nil then
			params[CityOperationTypes.PARAM_X] = where.x;
			params[CityOperationTypes.PARAM_Y] = where.y;
		end
	elseif row.Kind == "KIND_PROJECT" then
		params[CityOperationTypes.PARAM_PROJECT_TYPE] = row.Hash;
	else
		-- ★★★★★ SAY WHAT WE COULD NOT BUILD. A bare `no_params` is 100 discarded
		-- decisions with no name on any of them.
		--
		-- Measured on run civvis-20260801T012454Z over 126 turns: 2070 orders, 391
		-- refused, and `no_params` was 100 of them -- the second largest reason after
		-- movement. `build_no_plot` accounted for only 5, so ~95 fell through this
		-- branch and NOTHING said which item or which Kind.
		--
		-- What that costs is not the order, it is the decision. A produce order that
		-- dies here leaves the city with nothing queued, `ENDTURN_BLOCKING_PRODUCTION`
		-- fires, and `driveProduction` picks from the hand-written ladder instead. On
		-- that run only **16 of 64 builds were CIVVIS's** -- floor 21, improve 9,
		-- develop 8, army 4, expand 3, grow 2, scout 1 -- while every turn event
		-- reported `orders_source: civvis` with `residual: []`.
		emit("build_unknown_kind", {
			item = row.Type or tostring(row.Hash),
			row_kind = row.Kind or "(nil)",
		});
		return nil;
	end
	return params;
end

-- What each city was last told to build, and on which turn. Re-sending the
-- same order every tick is how one game logged two hundred settler requests
-- in fifty turns: the queue read comes back empty for a few frames after a
-- request, so the "queue is empty" test fires again and again.
local lastBuild = {};

-- Items the host's start-now predicate rejected in a city on this turn.
--
-- ⚠ The queue does NOT reflect a BUILD request in the same tick it is made, so
-- a synchronous "did it start?" check reads false for everything and is worse
-- than no check: it made the ladder re-order all six candidates every turn.
-- This table does not infer anything from that asynchronous queue read. It records
-- only synchronous `CanProduce(..., false, true)` failures and expires at the next
-- turn, so the ladder can fall through without inventing a permanent host rule.
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
		local refused = refusedByCity[cityId];
		if refused == nil or refused.turn ~= turn then
			refused = { turn = turn };
			refusedByCity[cityId] = refused;
		end
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
			local rejected = {};
			for item, value in pairs(refused or {}) do
				if value == true then rejected[item] = true; end
			end
			local searching = true;
			while searching do
				local name, row, why = chooseProduction(
					city, counts, #cities, turn, rejected);
				if row == nil then
					searching = false;
				else
					local params = buildParams(row, city);
					if params == nil then
						-- CanProduce allows a district whose placement query has no
						-- legal plot. Reject it for this sweep and ask the ladder for
						-- its next genuine candidate instead of blocking every turn.
						rejected[name] = true;
					else
						local ok = pcall(function()
							CityManager.RequestOperation(city, CityOperationTypes.BUILD, params);
						end);
						if ok then
							issued = issued + 1;
							lastBuild[cityId] = { turn = turn, item = name };
							if civvisBuild[tostring(cityId) .. ":next"] == name then
								civvisBuild[tostring(cityId) .. ":next"] = nil;
							end
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
						searching = false;
					end
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

-- The last founding this agent asked the host for, kept until the host either
-- confirms it or is caught not doing it. `UI.RequestPlayerOperation` is
-- asynchronous, so nothing on the requesting frame can tell success from a
-- silent no-op -- and a silent no-op here costs the Great Prophet AND the
-- religion. See the `kind == "religion"` handler.
local pendingReligionFounding = nil;

-- Which city each appointed governor was posted to, kept across turns. The engine
-- has query methods for this but their names differ between builds, and guessing
-- a Civilization VI API has cost this project three failed fixes today, so the
-- assignment we made is the assignment we remember.
local governorPost = {};
-- CIVVIS appoints and posts a Governor as one semantic action. Firaxis's stock UI
-- first submits APPOINT_GOVERNOR and waits for GovernorAppointed before it opens the
-- assignment flow, so retain that target across the asynchronous engine boundary.
local pendingGovernorAssignments = {};

local function chooseGovernor(player, pid)
	-- OFF BY DEFAULT: CIVVIS owns Governor strategy. This legacy blocker fallback
	-- remains independently gated so a later config cannot silently add a second AI.
	if not (cfg.GovernorAppoint or cfg.GovernorAssign) then return nil; end
	-- ⚠ Enum members first. `params[nil] = x` throws "table index is nil".
	local govParam = try(function() return PlayerOperations.PARAM_GOVERNOR_TYPE; end);
	local cityParam = try(function() return PlayerOperations.PARAM_CITY_DEST; end);
	local playerParam = try(function() return PlayerOperations.PARAM_PLAYER_ONE; end);
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
					-- The stock GovernorPanel operation contract takes the row INDEX.
					-- This used to send Hash and produced the repeatable Game Core crash.
					params[govParam] = row.Index;
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
	if cfg.GovernorAssign and assignOp ~= nil and cityParam ~= nil and playerParam ~= nil then
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
					params[govParam] = row.Index;
					params[playerParam] = pid;
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

-- The spend order, lifted out of `chooseEnvoy` so it can be tested without a
-- running Civ 6. Everything above it is engine accessors and everything below
-- it is `UI.RequestPlayerOperation`; this is the only part with an argument.
--
-- Takes the surveyed minors and returns them cheapest-flip-first, dropping the
-- ones we already hold and the ones that cannot legally take a token. Ties go
-- to the claim we have most invested in, so a part-built claim finishes instead
-- of a new one starting, and `id` breaks the remaining ties so the order is
-- deterministic across turns rather than dependent on survey order.
--
-- ⚠ Returns the FULL order, not one target. The caller spends `need` on each in
-- turn against a live token count -- the plan says what to buy, the engine says
-- what is still affordable.
-- ⚠⚠⚠ CONCEDE A BIDDING WAR YOU ARE LOSING. `need` is recomputed every turn as
-- `most + 1 - mine`, so a city-state a rival keeps outbidding us on shows a need
-- of 1 or 2 FOREVER and therefore looks like the cheapest flip on the board
-- every single turn. It is a money pit that reads as a bargain.
--
-- Measured on the first full run with the lane working
-- (`envoy-on-20260804T015105Z`, 250 turns): **113 envoys placed and ONE
-- suzerainty held at the end.** Jakarta took **28** of them and was never ours
-- (`ours=28, most=30`); Cardiff took 10 against 12; Vatican City 8 against 13.
-- Three lost auctions consumed 46 envoys — four flips' worth — while La Venta,
-- the one nobody contested, cost 10 and was held.
--
-- So: once we have paid a flip's worth into a claim and are STILL behind, the
-- rival wants it more than the price says, and the next envoy is better spent
-- anywhere else. Conceding is the whole point — matching is what lost 46.
local ENVOY_CONTEST_STAKE = 6;
local ENVOY_CONTEST_DEFICIT = 3;

local function envoyIsALostAuction(minor)
	-- Only ever true for ground somebody else is actively holding above us.
	return (minor.mine or 0) >= ENVOY_CONTEST_STAKE
		and (minor.need or 0) >= ENVOY_CONTEST_DEFICIT;
end

-- Exposed for the offline test only. ⚠ A BARE GLOBAL, never `_G.` — the UI Lua
-- sandbox does not expose `_G` and indexing it raises at chunk load.
CivvisEnvoyIsALostAuction = envoyIsALostAuction;

local function envoySpendOrder(seen)
	local order = {};
	for _, minor in ipairs(seen) do
		if minor.takes and not minor.ours and not envoyIsALostAuction(minor) then
			order[#order + 1] = minor;
		end
	end
	table.sort(order, function(a, b)
		if a.need ~= b.need then return a.need < b.need; end
		if a.mine ~= b.mine then return a.mine > b.mine; end
		return a.id < b.id;
	end);
	-- Nothing flippable: top up somewhere legal anyway rather than forfeit the
	-- token. A held envoy expires with the game, so a partial claim beats a
	-- full purse.
	--
	-- ⚠ This is where a conceded auction may legitimately come back: if EVERY
	-- reachable city-state is one, the choice is between a bad envoy and a dead
	-- one. Sorted the same way rather than taken in survey order, so the token
	-- goes to whichever is actually closest to flipping instead of whichever the
	-- engine happened to list first.
	if #order == 0 then
		for _, minor in ipairs(seen) do
			if minor.takes then order[#order + 1] = minor; end
		end
		table.sort(order, function(a, b)
			if a.need ~= b.need then return a.need < b.need; end
			if a.mine ~= b.mine then return a.mine > b.mine; end
			return a.id < b.id;
		end);
	end
	return order;
end

-- Exposed for the offline test only; nothing in the agent reads it back.
--
-- ⚠⚠⚠ A BARE GLOBAL, NEVER `_G.`. This exact line read `_G.CivvisEnvoySpendOrder`
-- in #1047 and that is what killed the whole lane: **Civilization VI's UI Lua
-- sandbox does not expose `_G`**, so indexing it raises at CHUNK LOAD and the
-- agent never loads at all. The symptom is not an error — it is silence. No
-- `loaded` event, no seat, no orders, `Automation.log` holding nothing but the
-- autoclose shim's lines, and a game that simply sits there. #1052 reverted the
-- file on that evidence (3 failed runs against 1 passing, interleaved) without
-- being able to name the cause.
--
-- The cause was already written down twice in this repository:
-- `CivvisGrounding.lua` — "The UI Lua sandbox does not expose `_G`, so
-- `rawget(_G, ...)` raises at load, which kills the whole script" — and
-- `CivvisControlAutoClose.lua`, which ends its warning with "if anybody reaches
-- for `_G` again". Bare globals are fine and this file already uses them
-- (`encode`, `findSettleSite`); only `_G` is fatal.
CivvisEnvoySpendOrder = envoySpendOrder;

local function chooseEnvoy(player, pid, turn)
	local influence = try(function() return player:GetInfluence(); end);
	local oneParam = try(function() return PlayerOperations.PARAM_PLAYER_ONE; end);
	if influence == nil or oneParam == nil then return nil; end
	-- ⚠⚠ NEVER read or write this handle across a `UI.RequestPlayerOperation`.
	-- Step 3 below already re-fetches, and the comment there explains why: a
	-- gameplay sub-object pointer held across operations that rewrite it is the
	-- best explanation on record for the three delayed EXC_BAD_ACCESS faults
	-- that took this whole lane out of deployment. But that fix only covered
	-- step 3. Steps 1 and 2 -- the levy scan and the placement loop, the two
	-- places that actually ISSUE the operations -- kept reading through the
	-- handle fetched above, so the defect was still live on the exact path the
	-- crash was recorded on. `inf()` is the re-fetch, and every read below goes
	-- through it.
	local function inf() return try(function() return player:GetInfluence(); end); end
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
				and try(function() return inf():CanLevyMilitary(minor.id); end, false)
			then
				local cost = try(function()
					return inf():GetLevyMilitaryCost(minor.id);
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

	-- 2. Buy suzerainties cheapest-first, spending only what each flip COSTS.
	--
	-- ⚠⚠ THE OLD LOOP PUT EVERY HELD TOKEN ON ONE CITY-STATE. `for _ = 1, tokens`
	-- against a single `best` is correct only when the purse is about the size of
	-- one claim. It is not. A live game (`civvis-20260803T191900Z`, turn 231) sat
	-- on **56 unspent envoys** against four met city-states needing 14, 13, 7 and
	-- 7 to flip -- 41 for the whole map, with 15 to spare. The old loop would
	-- have bought exactly ONE suzerainty and forfeited the other 42 envoys into
	-- the same minor, which stops paying the moment the lead is safe.
	--
	-- "Concentrate, do not spread" is right about not dribbling one envoy into
	-- six minors. It is not an argument for one target: the unit of value is a
	-- FLIP, so fill the cheapest claim exactly, then take the next cheapest with
	-- what is left. Same principle, correct granularity.
	local placed, target = 0, nil;
	if giveOp ~= nil and canGive and tokens > 0 and cfg.EnvoyPlace ~= false then
		for _, minor in ipairs(envoySpendOrder(seen)) do
			-- ⚠ The purse is re-read from a FRESH handle every target, and it is
			-- the loop bound. `tokens` above was read before the levy and before
			-- our own gives, so trusting it here would both over-issue and read
			-- through the stale pointer this function's crash is blamed on.
			local live = inf();
			if live == nil then break; end
			local left = try(function() return live:GetTokensToGive(); end, 0) or 0;
			if left < 1 then break; end
			if not try(function() return live:CanGiveInfluence(); end, false) then break; end
			if try(function() return live:CanGiveTokensToPlayer(minor.id); end, false) then
				-- Spend the flip price, or everything left when the flip is out
				-- of reach -- an envoy still held at the end of the game bought
				-- nothing, so the remainder goes into the cheapest partial claim.
				local want = minor.need;
				if want < 1 or want > left then want = left; end
				for _ = 1, want do
					local params = {};
					params[oneParam] = minor.id;
					local ok = pcall(function()
						UI.RequestPlayerOperation(pid, giveOp, params);
					end);
					if not ok then break; end
					placed = placed + 1;
					if target == nil then target = minor.id; end
				end
			end
		end
	end

	-- 3. Clear the prompt whatever happened. This is the line that ends the
	--    turn, and skipping it is what left a run wedged for ten minutes.
	--
	-- ⚠⚠ THE HANDLE WAS STALE, AND THAT IS THE BEST EXPLANATION ANYONE HAS FOR
	-- THE SEGFAULT. `influence` is fetched ONCE at the top of this function and
	-- was then written through HERE — after up to `tokens` calls to
	-- `UI.RequestPlayerOperation(pid, giveOp, ...)`, every one of which mutates
	-- the very gameplay object it points at.
	--
	-- The shipped screen never does that. `UI/PartialScreens/CityStates.lua`
	-- `Close()` re-fetches in the same expression as the read and the write:
	--
	--     local localPlayer = Players[Game.GetLocalPlayer()];
	--     if (... and not localPlayer:GetInfluence():IsGivingTokensConsidered()) then
	--         localPlayer:GetInfluence():SetGivingTokensConsidered(true);
	--     end
	--
	-- So the suspected illegality was never the CONTEXT — the shipped call is a
	-- UI script too, exactly like this one. What differs is holding a pointer to
	-- a gameplay sub-object across operations that rewrite it. That matches the
	-- recorded signature far better than a bad immediate call does: three
	-- EXC_BAD_ACCESS faults in requested-seed-425255 runs, each **6-9 turns
	-- AFTER** the single envoy was placed, while 0-for-2 no-envoy runs did not
	-- crash. A delayed fault is corrupted bookkeeping.
	--
	-- ⚠ This does NOT re-enable envoys. `cfg.EnvoyEnabled` stays off and this
	-- whole function is still unreachable in deployment, so shipping this changes
	-- nothing at runtime. It removes one concrete defect so that the isolation
	-- experiment the comment in SOFT_BLOCKERS asks for has a fair chance —
	-- `EnvoyPlace` and `EnvoyConsider` already switch the two mutations
	-- independently, so that experiment is a config change, not a code change.
	if cfg.EnvoyConsider ~= false then
		pcall(function()
			-- Re-fetch: mirror the shipped screen exactly.
			local fresh = player:GetInfluence();
			if fresh ~= nil and not fresh:IsGivingTokensConsidered() then
				fresh:SetGivingTokensConsidered(true);
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
	-- The following turn's beginTurn reads the host's fresh influence object and
	-- emits `envoy_reconcile`; do not call this an immediate confirmation because
	-- the UI operation can be asynchronous and the old handle is unsafe to read.
	envoyTally.pending = { turn = turn, held_before = tokens, requested = placed };
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
	-- ⚠⚠⚠ GOVERNOR_APPOINTMENT JOINS ITS SIBLINGS, AND THIS ONE WAS MEASURED.
	-- Answering it with the old `chooseGovernor` CRASHED THE GAME CORE. Three runs, three
	-- different maps and civilizations, all with CIVVIS deciding:
	--     civvis-…T111953Z  t38      civvis-…T112719Z  t37      civvis-…T113303Z  t37
	-- Every one died with EXC_BAD_ACCESS at KERN_INVALID_ADDRESS 0x18 on the
	-- **Game Core** thread, and the faulting frames are BYTE-IDENTICAL across all
	-- three: GameCore_XP2.dll +746148, +745916, +2560108, +2565576, +2567456,
	-- +546828. In each run the very last event written is the turn record with
	-- `blocker: ENDTURN_BLOCKING_GOVERNOR_APPOINTMENT, answered: null` — the game
	-- died in the answer, before the answer could be reported. Comparing against
	-- Firaxis's shipped GovernorPanel later found the cause: the fallback supplied a
	-- Governor Hash where the operation requires its row Index, and assignment omitted
	-- PARAM_PLAYER_ONE. Both parameters are corrected above and in the CIVVIS actuator.
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
	-- This blocker remains soft because CIVVIS now spends and actuates the title from
	-- the exported roster. `GovernorAppoint` and `GovernorAssign` re-enable only the
	-- legacy heuristic fallback and should stay off in a CIVVIS-decided run.
	ENDTURN_BLOCKING_GOVERNOR_APPOINTMENT = true,
	-- ⚠⚠ GIVE_INFLUENCE_TOKEN IS BACK HERE, AND THE REASON MATTERS. Answering it
	-- with `chooseEnvoy` CRASHES THE GAME CORE. Across repeated requested seed
	-- 425255 runs with the same flags:
	--     envoy_events = 0  ->  t92, t106      no crash
	--     envoy_events = 1  ->  t44, t47, t45  EXC_BAD_ACCESS each time
	-- Three fresh SIGSEGVs in `GameCore_XP2.dll` on the `Game Core` thread, 6-9
	-- turns AFTER the single envoy was placed — a delayed fault, so corrupted
	-- state rather than a bad immediate call. Civ 6 does segfault on its own
	-- (there is a pre-envoy crash at t25), but 3-for-3 against 0-for-2 on the
	-- The real-Civ6 seed request does not pin world generation, so this is not a
	-- same-map control. The 3-for-3 versus 0-for-2 result is still a concrete
	-- crash-isolation signal; it is not evidence that the seed had any effect.
	-- ⚠ THE "WRONG CONTEXT" HYPOTHESIS IS DEAD — do not spend another cycle on it.
	-- The shipped `UI/PartialScreens/CityStates.lua` `Close()` calls
	-- `SetGivingTokensConsidered(true)` from a UI script, exactly like this agent.
	-- What differed was that `chooseEnvoy` cached `player:GetInfluence()` at the
	-- top of the function and wrote through that handle AFTER up to `tokens`
	-- `UI.RequestPlayerOperation` calls had rewritten the object underneath it,
	-- while the shipped screen re-fetches in the same expression. A stale
	-- gameplay handle fits a fault delayed 6-9 turns; a bad immediate call does
	-- not. That is now fixed, so the isolation run is worth doing.
	--
	-- Set `EnvoyEnabled` to re-enable. `EnvoyPlace` and `EnvoyConsider` already
	-- switch the two mutations independently, so isolating them is a CONFIG
	-- change, not a code change: place-only, then consider-only, across
	-- independent random-world samples.
	-- ⚠ Do it on a throwaway batch, never on a running one. Until then the
	-- known-stable skip stands, and the ten-minute wedge is the lesser failure.
	--
	-- The prize is large: the same agent headless places **18.1 envoys and holds
	-- 0.71 suzerainties** per seat (74x46, 9 city-states, 200 turns), against a
	-- live median of **1 and 0** over 36 runs.
	ENDTURN_BLOCKING_GIVE_INFLUENCE_TOKEN = true,
	ENDTURN_BLOCKING_CLAIM_GREAT_PERSON = true,
	ENDTURN_BLOCKING_ARTIFACT = true,
	ENDTURN_BLOCKING_EMERGENCY_NEEDS_ATTENTION = true,
	ENDTURN_BLOCKING_WORLD_CONGRESS_LOOK = true,
	ENDTURN_BLOCKING_WORLD_CONGRESS_SESSION = true,
	ENDTURN_BLOCKING_SPY_CHOOSE_ESCAPE_ROUTE = true,
	ENDTURN_BLOCKING_SPY_CHOOSE_DRAGNET_PRIORITY = true,
};

-- The soft blockers the ENGINE will not let a plain end-turn request past.
--
-- The shipped ActionPanel.lua special-cases exactly this trio: with one of
-- them active, its own end-turn click does not request the end of turn at all
-- -- it calls `UI.SelectNextReadyUnit()` and waits for the human. So the
-- `UI.RequestAction(ActionTypes.ACTION_ENDTURN)` at the bottom of `tick` is
-- refused while one of these is up, which no other soft blocker does. These
-- are the ones whose forfeit needs the parking pass AND the forced request.
local UNIT_BLOCKERS = {
	ENDTURN_BLOCKING_UNITS = true,
	ENDTURN_BLOCKING_UNIT_NEEDS_ORDERS = true,
	ENDTURN_BLOCKING_STACKED_UNITS = true,
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
	-- ★★★★★ THE TWO CIVVIS ALREADY DECIDES AND THE HEURISTICS ANSWERED OVER.
	--
	-- Measured on live run `civvis-20260810T040916Z` (Rome/Trajan, Settler,
	-- Online), which reported `orders_source: civvis` on 114 of 114 turns:
	--
	--     t21  ENDTURN_BLOCKING_PANTHEON         answered "BELIEF_DANCE_OF_THE_AURORA"
	--     t--  ENDTURN_BLOCKING_FILL_CIVIC_SLOT  answered "policies+3"
	--
	-- while the orders database for the same game held **1 `pantheon` order and
	-- 26 `policy_deck` orders**. CIVVIS had an opinion on both and the
	-- hand-written ladder answered first, because neither name was in this list
	-- nor in `SOFT_BLOCKERS` -- the only two ways a blocker is kept off the
	-- heuristics.
	--
	-- ⚠ AND CIVVIS ONLY WON THE PANTHEON BY LUCK. `choosePantheon` walks
	-- `GameInfo.Beliefs()` and requests the FIRST untaken pantheon belief, which
	-- is why the answer above is Dance of the Aurora -- database order, not a
	-- choice. Both requests went to the engine and CIVVIS's Divine Spark is what
	-- the t22 state shows, so the race was won rather than avoided. A race whose
	-- outcome is right is not a mechanism that works.
	--
	-- The existing semantics are exactly what these need and nothing else
	-- changes: decline while the reply is `pending`, report `civvis_complete`
	-- once it has landed, and let the soft-blocker forfeit (bounded at the
	-- SECOND sighting for a `civvis_complete` answer) clear a prompt CIVVIS
	-- turns out to have no opinion on.
	--
	ENDTURN_BLOCKING_PANTHEON = true,
	ENDTURN_BLOCKING_FILL_CIVIC_SLOT = true,
	-- ★★★ IT APPEARED. The note that stood here said this name was deliberately
	-- left out "though CIVVIS issues `government` orders too: it did not appear
	-- in this run, so adding it would be reasoning rather than measurement" —
	-- and left the measurement as the condition for adding it. Over the 14 runs
	-- to `civvis-20260817T030352Z` it fired **3 times**, every one answered by
	-- `chooseGovernment` on first sight while the orders channel carried CIVVIS's
	-- own `government` order. Small, and the same shape as the pantheon and the
	-- policy slot before it: a decision CIVVIS has an opinion on, raced and lost
	-- by a hand-written ladder because its name was not on this list.
	--
	-- The list is now checked rather than remembered: `residual_census_test.lua`
	-- requires every blocker mapped to a CIVVIS order kind to be here, so the
	-- next one cannot wait for someone to notice it in a log.
	ENDTURN_BLOCKING_CONSIDER_GOVERNMENT_CHANGE = true,
};

-- Which CIVVIS order kind answers which end-turn prompt.
--
-- ⚠ THIS EXISTS BECAUSE THE OWNED LIST ABOVE WAS MAINTAINED BY NOTICING. Every
-- entry it has gained arrived the same way: a prompt was answered by the
-- hand-written ladder for months, somebody eventually read a log, and the name
-- was added — pantheon and the civic slot in #1465, the government change in
-- this change. The join between "CIVVIS emits orders of kind K" and "prompt P
-- is CIVVIS's to answer" was never written down, so nothing could check it.
--
-- Written down, `residual_census_test.lua` can: every prompt named here must be
-- in `CIVVIS_OWNED_BLOCKERS`, so adding an order kind that answers a prompt
-- fails the gate until the prompt is claimed.
--
-- ⚠ A prompt is listed here only when CIVVIS actually emits an order that
-- ANSWERS it. `gp_recruit` and the envoy and governor kinds are deliberately
-- absent: their prompts are in `SOFT_BLOCKERS`, where CIVVIS's actuator already
-- owns the decision and the heuristic answer is a known GAME-CORE CRASH (see
-- the SIGSEGV notes on `ENDTURN_BLOCKING_GIVE_INFLUENCE_TOKEN` and
-- `ENDTURN_BLOCKING_GOVERNOR_APPOINTMENT`). Soft and owned are two different
-- ways to keep the heuristics off a decision, and only owned ones belong here.
-- ⚠ A bare global, not a local: see the 200-slot note on
-- `CivvisResidualBucket`. It is read only by `residual_census_test.lua`.
CivvisAnswersPrompt = {
	ENDTURN_BLOCKING_RESEARCH = "research",
	ENDTURN_BLOCKING_CIVIC = "civic",
	ENDTURN_BLOCKING_PRODUCTION = "produce",
	ENDTURN_BLOCKING_PANTHEON = "pantheon",
	ENDTURN_BLOCKING_FILL_CIVIC_SLOT = "policy_deck",
	ENDTURN_BLOCKING_CONSIDER_GOVERNMENT_CHANGE = "government",
};

-- Exposed for the offline test only; nothing in the agent reads these back.
-- ⚠ BARE GLOBALS, NEVER `_G.` — see the note on `CivvisEnvoySpendOrder`.
CivvisOwnedBlockers = CIVVIS_OWNED_BLOCKERS;
CivvisSoftBlockers = SOFT_BLOCKERS;

-- Answer the decision the game says it is waiting on. Returning the name of
-- what was answered (rather than a boolean) is what makes a stuck run
-- diagnosable: the log says which blocker recurred, not merely that one did.
-- Which of the three things a residual pass was, given what the ladder returned
-- and whether CIVVIS had already answered this prompt. See the long note in
-- `answerBlocker` for why one flat number was not enough.
--
--   declined     the ladder had no answer; NOTHING decided anything
--   after_civvis CIVVIS answered, the prompt stood, and the bounded escape at
--                the forfeit ladder asked for one real answer. By design.
--   unasked      CIVVIS was never consulted and the ladder decided. The leak.
--
-- ⚠⚠⚠ A BARE GLOBAL RATHER THAN A LOCAL, AND NOT BY PREFERENCE. This file
-- sits at 198 of Lua 5.1's **200 top-level local slots**, and passing that
-- limit is not a warning: the chunk fails to load, which in a live game is the
-- silent death this repository has already paid for twice (`_G` in #1047,
-- `goto` in the 5.1/5.4 gap). Three new locals in the first draft of this
-- change took it to 201 and `luac5.1 -p` refused the file. Globals cost no
-- slot. Never `_G.` — see the note on `CivvisEnvoySpendOrder`.
CivvisResidualBucket = function(answer, residual_ok)
	if answer == nil then return "declined"; end
	if residual_ok then return "after_civvis"; end
	return "unasked";
end

local function answerBlocker(player, pid, blocker, turn, residual_ok)
	local name = blockerName(blocker);
	-- A CIVVIS pass is a complete decision for the mirrored state it received.
	-- Firaxis can finish production or research later in the same turn and raise
	-- another prompt, but answering that prompt with the hand-written ladder
	-- gives control to a second AI. That is how a completed wall repair silently
	-- became a Builder in every city. End the turn and let the next fresh export
	-- ask CIVVIS what belongs in the now-empty queue.
	--
	-- ⚠ Except that a HARD blocker cannot be ended past. `ENDTURN_BLOCKING_RESEARCH`
	-- raised by a tech finishing mid-turn — after CIVVIS's reply landed, so about a
	-- queue CIVVIS's board never saw — survives `civvis_complete` by construction,
	-- and survives `dismissBlocker` because the engine re-raises what end-turn
	-- still requires. Runs civvis-20260814T215409Z (t181) and ...T223051Z (t208)
	-- both walked the whole forfeit ladder into a `wedged` report this way, every
	-- time research or a civic completed mid-turn late in the game. The forfeit
	-- arm therefore retries ONCE with `residual_ok`, which skips only this return:
	-- by then the CIVVIS reply is landed and applied (source == civvis), so the
	-- race this return exists to prevent cannot happen, and the ladder answer is
	-- counted in `residualAnswers` below like every other residual decision.
	-- CIVVIS re-decides from the next export and may override the pick.
	if not residual_ok and cfg.CivvisDecides and CIVVIS_OWNED_BLOCKERS[name]
			and (awaiting.source == "civvis" or awaiting.source == "civvis_stale") then
		return "civvis_complete";
	end

	-- While the current CIVVIS answer is still in flight, decline to answer a
	-- decision it owns. Returning nil leaves the blocker up and the loop retries
	-- after the orders database has had a chance to receive the reply.
	if cfg.CivvisDecides and awaiting.source == "pending"
			and CIVVIS_OWNED_BLOCKERS[name]
			and spend("civvis_wait_" .. name, cfg.MaxCivvisWaitPasses or 12) then
		return nil;
	end

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
	--
	-- ⚠⚠⚠ AND FOR A WHILE IT STOPPED ANSWERING THAT QUESTION. Counting HERE, before
	-- the ladder runs, cannot see three outcomes that mean opposite things:
	--
	--   * `unasked`      -- CIVVIS was never consulted for this blocker (the name is
	--                       in neither `CIVVIS_OWNED_BLOCKERS` nor `SOFT_BLOCKERS`)
	--                       and the ladder answered it. THIS is the leak the counter
	--                       exists to find: a second AI deciding under CIVVIS's name.
	--   * `after_civvis` -- CIVVIS answered, the prompt came back anyway, and the
	--                       bounded escape at the forfeit ladder asked for one real
	--                       answer rather than wedge the turn. By design, and the
	--                       design is load-bearing: without it runs sat 900 s on a
	--                       standing prompt (t178 of `civvis-20260816T115139Z`, the
	--                       seat's best game at the time).
	--   * `declined`     -- the ladder returned nil. NOTHING decided anything; the
	--                       prompt goes to the dismissal path.
	--
	-- One flat number over all three reads as the first one. On 2026-08-17 a review
	-- of 14 runs read 1,577 residuals as "1,577 decisions taken by the Lua fallback
	-- instead of CIVVIS" and had to be withdrawn: 937 were the escape hatch, ~350
	-- were declines that decided nothing, and the actual leak was THREE
	-- (`ENDTURN_BLOCKING_CONSIDER_GOVERNMENT_CHANGE`, now owned below). A number
	-- that makes a careful reader with the source open reach the wrong conclusion
	-- is a broken instrument, not a big finding.
	--
	-- So the classification moved to where the outcome is known: below the ladder,
	-- on its result. `counted` keeps the flat per-name total the ledger already
	-- reads, and the three buckets carry the meaning.
	local answer = CivvisAnswerBlockerLadder(player, pid, name, turn);
	if cfg.CivvisDecides then
		local bucket = CivvisResidualBucket(answer, residual_ok);
		residualAnswers[name] = (residualAnswers[name] or 0) + 1;
		residualAnswers[name .. "@" .. tostring(awaiting.source)] =
			(residualAnswers[name .. "@" .. tostring(awaiting.source)] or 0) + 1;
		residualAnswers[name .. "!" .. bucket] =
			(residualAnswers[name .. "!" .. bucket] or 0) + 1;
		residualAnswers["!" .. bucket] = (residualAnswers["!" .. bucket] or 0) + 1;
	end
	return answer;
end

-- The hand-written answer for one blocker, or nil when this controller has none.
--
-- Split out of `answerBlocker` so the residual census above can classify the
-- OUTCOME rather than the attempt; it is the same ladder, unchanged.
-- ⚠ A bare global for the 200-slot reason on `CivvisResidualBucket`, not
-- because anything outside this file should call it. Never `_G.`.
CivvisAnswerBlockerLadder = function(player, pid, name, turn)
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
		-- ★★★★★ THE BUDGET WAS SMALLER THAN THE EMPIRE, so past four cities the
		-- turn simply stopped answering. `ENDTURN_BLOCKING_PRODUCTION` fires once
		-- per city that needs something queued, so a fixed 4 is spent before six
		-- cities have been served and every later blocker returns nil WITHOUT
		-- TRYING. Measured on run `civvis-20260801T065721Z`:
		--
		--     turns    unanswered   answered
		--     <50               0         15
		--     50-100            0         23
		--     100-150          20         24
		--     150+             18         14
		--
		-- Zero failures until turn 102 -- four turns after the sixth city -- then
		-- 38 of them, until it was failing more often than it succeeded. It reads
		-- as "the city had nothing it could build", which is a different and much
		-- more interesting bug, and it is not that at all.
		--
		-- ⚠ Third instance of one shape in this file: `ArmyCap` at 10 units, the
		-- production floor's Ancient-only fallbacks, and this. A constant that is
		-- right for a small early empire and silently wrong for a real one. The
		-- budget exists to stop a turn taking ten minutes, and the work it bounds
		-- is PER CITY -- so the bound has to be per city too.
		local passes = math.max(cfg.MaxProductionPasses or 4, cityCount(player) + 2);
		if not spend("production", passes) then return nil; end
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
-- Per-turn forfeit state for each SOFT blocker, by name:
-- `{ sightings = <since the last forfeit>, forfeits = <this turn> }`.
--
-- `attempts` cannot carry the forfeit decision alone: it counts every blocker
-- the turn showed, so two alternating blockers would reach any small bound
-- with neither one actually stuck. The forfeit fires on the SAME soft blocker
-- surviving its own answer, and `forfeits` is what bounds the retry.
local softSeen = {};

local inTick = false;
local finished = false;


-- ------------------------------------------------- great work slot knowledge
--
-- Shared by the state export and the Great Person driver, because both were
-- wrong about the same thing in different ways and the numbers were large:
-- across all archived runs 123 Writers, 70 Artists and 58 Musicians ended
-- their game standing idle while — in the worst run, civvis-20260818T052156Z —
-- SIX empty writing slots and NINE empty art slots stood in their own empire.
--
-- Two defects, one root:
--   1. The export's class->object constant said `GREAT_WORK_OBJECT_WRITING`;
--      Firaxis's `GreatWork_ValidSubTypes` spells it `GREATWORKOBJECT_WRITING`
--      (and there is NO `..._ART` at all: Artists create SCULPTURE, PORTRAIT,
--      LANDSCAPE or RELIGIOUS works, per individual). The lookup could never
--      match, so `empty_slots` exported 0 for every cultural person and the
--      brain's driver — which stands still on `empty_slots == 0` by design —
--      froze them all for good.
--   2. Both drivers walked to the NEAREST highlighted plot. The engine's
--      `GetActivationHighlightPlots` names a cultural person's districts
--      whether or not a compatible slot is free, so eleven people stacked on
--      one slotless plot at (25,23) on run civvis-20260817T010950Z while six
--      Amphitheaters with twelve empty slots stood 2-10 tiles away, and the
--      run ended with ZERO Great Works.
--
-- A bare-global namespace (the `CivvisTrade` pattern): the main chunk is one
-- file-scope local below Lua's 200-slot ceiling and this must be visible to
-- both `exportState` above and `orderGreatPerson` far below.
CivvisGreatWorks = {
	-- Survey memo; the export and the driver run in the same turn and the
	-- board cannot change between them.
	memo = { turn = -1, survey = nil },
	-- `GameInfo.GreatWorks` individual -> the object type that person creates,
	-- built once. The class fallback below covers an individual the table does
	-- not know (a DLC person under a ruleset the map was not built for).
	objectByIndividual = nil,
	-- What each slot-consuming class can produce when the individual row is
	-- unavailable. Classes absent here (Merchants, Engineers, Scientists,
	-- Generals...) do not spend Great Work slots and keep `empty_slots` nil.
	CLASS_OBJECTS = {
		GREAT_PERSON_CLASS_WRITER = { GREATWORKOBJECT_WRITING = true },
		GREAT_PERSON_CLASS_ARTIST = {
			GREATWORKOBJECT_SCULPTURE = true,
			GREATWORKOBJECT_PORTRAIT = true,
			GREATWORKOBJECT_LANDSCAPE = true,
			GREATWORKOBJECT_RELIGIOUS = true,
		},
		GREAT_PERSON_CLASS_MUSICIAN = { GREATWORKOBJECT_MUSIC = true },
	},
};

-- The set of Great Work object types this person's activation produces, or
-- nil for a class that does not consume slots. `individualType` and
-- `classType` are the database names, exactly as `gpName` returns them.
CivvisGreatWorks.objectsFor = function(individualType, classType)
	if CivvisGreatWorks.objectByIndividual == nil then
		CivvisGreatWorks.objectByIndividual = try(function()
			local map = {};
			for row in GameInfo.GreatWorks() do
				if row.GreatPersonIndividualType ~= nil
						and row.GreatWorkObjectType ~= nil then
					map[row.GreatPersonIndividualType] = row.GreatWorkObjectType;
				end
			end
			return map;
		end, nil) or {};
	end
	local object = individualType ~= nil
		and CivvisGreatWorks.objectByIndividual[individualType] or nil;
	if object ~= nil then return { [object] = true }; end
	return classType ~= nil and CivvisGreatWorks.CLASS_OBJECTS[classType] or nil;
end

-- Every empty Great Work slot in the empire, each carrying the object types
-- its slot kind accepts (Firaxis's own `GreatWork_ValidSubTypes` — it is what
-- makes Palace slots take all non-artifact kinds) and, when the slot's
-- building hangs off a district, THAT DISTRICT'S PLOT INDEX — the tile a
-- person must stand on for the engine to take Activate. Wonders keep
-- `plot = nil`: their buildings name no `PrerequisiteDistrict`, so their tile
-- stays unknown and the walk falls back to the engine's own highlight.
--
-- `district_plots` is every completed district tile we own, any type: a
-- highlighted plot in this set WITHOUT a matching empty slot is known-full
-- and never worth walking to; a highlighted plot outside it (a wonder) stays
-- an honest unknown.
--
-- Returns nil when the slot tables are unreadable in this context, and the
-- callers keep their old behaviour — the `revealed_api` rule.
CivvisGreatWorks.survey = function(player, turn)
	local memo = CivvisGreatWorks.memo;
	if memo.turn == turn then return memo.survey; end
	local survey = nil;
	local accepts = try(function()
		local map = {};
		for row in GameInfo.GreatWork_ValidSubTypes() do
			local slotMap = map[row.GreatWorkSlotType] or {};
			slotMap[row.GreatWorkObjectType] = true;
			map[row.GreatWorkSlotType] = slotMap;
		end
		return map;
	end, nil);
	if accepts ~= nil and next(accepts) ~= nil then
		-- Unique replacements, so an Acropolis tile answers for the Theater
		-- its Amphitheater's database row names — the same swap the develop
		-- ladder's civ-unique note documents.
		local replaces = try(function()
			local map = {};
			for row in GameInfo.DistrictReplaces() do
				map[row.CivUniqueDistrictType] = row.ReplacesDistrictType;
			end
			return map;
		end, nil) or {};
		survey = { slots = {}, district_plots = {} };
		eachCity(player, function(city)
			local blds = try(function() return city:GetBuildings(); end, nil);
			if blds == nil then return; end
			-- District tile by BASE district type. Walk the city's PLOTS, not
			-- `GetDistricts()` — the plot carries type and position together
			-- and the collection's per-member accessors vary across builds
			-- (the same rule the district export follows).
			local plotByDistrict = {};
			local owned = try(function()
				return Map.GetCityPlots():GetPurchasedPlots(city);
			end, nil);
			if owned ~= nil then
				for _, plotIndex in ipairs(owned) do
					local plot = try(function()
						return Map.GetPlotByIndex(plotIndex);
					end, nil);
					local dIndex = plot ~= nil and try(function()
						return plot:GetDistrictType();
					end, -1) or -1;
					local dRow = (dIndex ~= nil and dIndex >= 0)
						and GameInfo.Districts[dIndex] or nil;
					if dRow ~= nil and dRow.DistrictType ~= nil then
						plotByDistrict[dRow.DistrictType] = plotIndex;
						if replaces[dRow.DistrictType] ~= nil then
							plotByDistrict[replaces[dRow.DistrictType]] = plotIndex;
						end
						survey.district_plots[plotIndex] = true;
					end
				end
			end
			for buildingInfo in GameInfo.Buildings() do
				if try(function()
					return blds:HasBuilding(buildingInfo.Index);
				end, false) then
					local slots = try(function()
						return blds:GetNumGreatWorkSlots(buildingInfo.Index);
					end, 0) or 0;
					for slot = 0, slots - 1 do
						local occupied = try(function()
							return blds:GetGreatWorkInSlot(buildingInfo.Index, slot);
						end, -1);
						if occupied == nil or occupied < 0 then
							local slotType = try(function()
								local index = blds:GetGreatWorkSlotType(
									buildingInfo.Index, slot);
								local row = GameInfo.GreatWorkSlotTypes[index];
								return row and row.GreatWorkSlotType or nil;
							end, nil);
							local acceptSet = slotType ~= nil
								and accepts[slotType] or nil;
							if acceptSet ~= nil then
								survey.slots[#survey.slots + 1] = {
									accepts = acceptSet,
									plot = buildingInfo.PrerequisiteDistrict ~= nil
										and plotByDistrict[
											buildingInfo.PrerequisiteDistrict]
										or nil,
								};
							end
						end
					end
				end
			end
		end);
	end
	memo.turn = turn;
	memo.survey = survey;
	return survey;
end

-- How many of the survey's empty slots take any of this person's objects, and
-- on which known tiles. Returns `nil, nil` when either side is unknown;
-- otherwise the count plus a set of plot indices a matching slot stands on.
CivvisGreatWorks.matches = function(survey, objects)
	if survey == nil or objects == nil then return nil, nil; end
	local count, plots = 0, {};
	for _, s in ipairs(survey.slots) do
		local fits = false;
		for object in pairs(objects) do
			if s.accepts[object] then fits = true; break; end
		end
		if fits then
			count = count + 1;
			if s.plot ~= nil then plots[s.plot] = true; end
		end
	end
	return count, plots;
end


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
local function exportState(player, pid, turn, frame)
	-- The six yields of one plot as the owner sees them, or nil when the read
	-- fails. Nested here rather than at file scope: the main chunk sits one
	-- local below Lua's 200-slot ceiling (see AgentChunkLocalLimitTest), and a
	-- script that crosses it fails to compile with no log line anywhere.
	-- Yield indices come from the enum the shipped UI uses; a build whose enum
	-- lacks a member returns nil for that member, which `try` turns into an
	-- absent field rather than a zero.
	local function plotYields(plot)
		return try(function()
			return {
				food = plot:GetYield(YieldTypes.FOOD),
				production = plot:GetYield(YieldTypes.PRODUCTION),
				gold = plot:GetYield(YieldTypes.GOLD),
				science = plot:GetYield(YieldTypes.SCIENCE),
				culture = plot:GetYield(YieldTypes.CULTURE),
				faith = plot:GetYield(YieldTypes.FAITH),
			};
		end);
	end
	if cfg.ExportState ~= true then return; end

	-- A met rival's detailed city list stays gated on actual map sight below.
	-- These totals are different: they are the public standings a player can
	-- compare without learning where the underlying cities, Wonders or weapons
	-- are. Keep the two channels separate so a full HUD never turns into a
	-- fog-of-war leak.
	local wonderBuildingIndices = {};
	for building in GameInfo.Buildings() do
		if building.IsWonder then
			wonderBuildingIndices[#wonderBuildingIndices + 1] = building.Index;
		end
	end
	local function publicSuzerainCounts()
		return try(function()
			local counts = {};
			for _, minor in ipairs(PlayerManager.GetAliveMinors()) do
				local minorId = minor:GetID();
				local civ = PlayerConfigurations[minorId]:GetCivilizationTypeName();
				if civ ~= "CIVILIZATION_FREE_CITIES" and civ ~= "CIVILIZATION_BARBARIAN" then
					local influence = minor:GetInfluence();
					if influence == nil then return nil; end
					local holder = influence:GetSuzerain();
					if type(holder) ~= "number" then return nil; end
					if holder >= 0 then
						counts[holder] = (counts[holder] or 0) + 1;
					end
				end
			end
			return counts;
		end, nil);
	end
	local function publicEmpireStats(subject, suzerainCounts)
		local subjectId = try(function() return subject:GetID(); end, -1);
		local stats = {
			city_count = -1, population = -1, food = -1, production = -1,
			wonder_count = -1,
			suzerain_count = suzerainCounts ~= nil and (suzerainCounts[subjectId] or 0) or -1,
			nuclear_devices = -1, thermonuclear_devices = -1,
		};
		local totals = try(function()
			local subjectsCities = subject:GetCities();
			if subjectsCities == nil then return nil; end
			local tally = {
				city_count = 0, population = 0, food = 0, production = 0, wonder_count = 0,
			};
			for _, city in subjectsCities:Members() do
				local population = city:GetPopulation();
				local food = city:GetYield(YieldTypes.FOOD);
				local production = city:GetYield(YieldTypes.PRODUCTION);
				local buildings = city:GetBuildings();
				if type(population) ~= "number" or type(food) ~= "number"
						or type(production) ~= "number" or buildings == nil then
					return nil;
				end
				tally.city_count = tally.city_count + 1;
				tally.population = tally.population + population;
				tally.food = tally.food + food;
				tally.production = tally.production + production;
				for _, buildingIndex in ipairs(wonderBuildingIndices) do
					local built = buildings:HasBuilding(buildingIndex);
					if built == nil then return nil; end
					if built then tally.wonder_count = tally.wonder_count + 1; end
				end
			end
			return tally;
		end, nil);
		if totals ~= nil then
			stats.city_count = totals.city_count;
			stats.population = totals.population;
			stats.food = totals.food;
			stats.production = totals.production;
			stats.wonder_count = totals.wonder_count;
		end
		local weapons = try(function() return subject:GetWMDs(); end, nil);
		local nuclear = try(function()
			local definition = GameInfo.WMDs["WMD_NUCLEAR_DEVICE"];
			return weapons ~= nil and definition ~= nil
				and weapons:GetWeaponCount(definition.Index) or nil;
		end, nil);
		local thermonuclear = try(function()
			local definition = GameInfo.WMDs["WMD_THERMONUCLEAR_DEVICE"];
			return weapons ~= nil and definition ~= nil
				and weapons:GetWeaponCount(definition.Index) or nil;
		end, nil);
		if type(nuclear) == "number" then stats.nuclear_devices = nuclear; end
		if type(thermonuclear) == "number" then stats.thermonuclear_devices = thermonuclear; end
		return stats;
	end

	local cities = {};
	-- Firaxis leaves a Trader unit on the map while it travels, so its position
	-- alone cannot say whether it is available for a new route.  Export the
	-- authoritative route records from the same API the stock Trade Overview
	-- uses.  CIVVIS needs both endpoint ids for capacity/yields and the unit id
	-- to keep the active Trader out of the next planning pass.
	local tradeRoutes = {};
	-- Routes OTHER players run into our cities, per destination city id. The
	-- destination earns from them under a few rules — Zhang Qian's "+2 Gold from
	-- incoming foreign routes" was live on Aquileia from t131 of run
	-- civvis-20260816T040537Z and the mirror had no way to count the routes it
	-- applied to. Foreign and domestic are told apart by origin player.
	-- There is no incoming-routes accessor; the shipped TradeOverview walks every
	-- other player's cities' OUTGOING routes and keeps those whose destination is
	-- the local player, and so does this. Keyed by destination city id.
	local incomingRoutes = {};
	pcall(function()
		for _, other in ipairs(Game.GetPlayers() or {}) do
			local otherId = try(function() return other:GetID(); end, -1);
			if otherId ~= pid then
				local theirCities = try(function() return other:GetCities(); end);
				if theirCities ~= nil then
					for _, theirCity in theirCities:Members() do
						local theirRoutes = try(function()
							return theirCity:GetTrade():GetOutgoingRoutes();
						end, {});
						for _, route in ipairs(theirRoutes or {}) do
							if try(function() return route.DestinationCityPlayer; end, -1) == pid then
								local dest = try(function() return route.DestinationCityID; end, -1);
								incomingRoutes[dest] = incomingRoutes[dest]
									or { foreign = 0, domestic = 0, origins = {} };
								incomingRoutes[dest].foreign = incomingRoutes[dest].foreign + 1;
								-- The ORIGIN city and its owner, so the mirror can seat
								-- the route on the board (owner = the rival's seat, origin
								-- = the rival city at these coordinates) instead of only
								-- counting it: alliance/treaty/Great-Merchant yields at
								-- the destination all read `game.routes`, which carried
								-- only OUR routes until run civvis-20260816T200454Z showed
								-- Cumae's "+4 from Incoming Trade Routes" (World Congress
								-- Trade Policy A) that the model could not see.
								local origins = incomingRoutes[dest].origins;
								origins[#origins + 1] = {
									x = try(function() return theirCity:GetX(); end, -1),
									y = try(function() return theirCity:GetY(); end, -1),
									player = otherId,
								};
							end
						end
					end
				end
			end
		end
	end);
	eachCity(player, function(city)
		local outgoing = try(function()
			return city:GetTrade():GetOutgoingRoutes();
		end, {});
		for _, route in ipairs(outgoing or {}) do
			local originID = try(function() return route.OriginCityID; end,
				try(function() return city:GetID(); end, -1));
			local destinationPlayer = try(function() return route.DestinationCityPlayer; end, -1);
			local destinationID = try(function() return route.DestinationCityID; end, -1);
			local destination = try(function()
				local destinationOwner = Players[destinationPlayer];
				return destinationOwner and destinationOwner:GetCities():FindID(destinationID);
			end, nil);
			if destinationPlayer == pid and destinationID ~= nil and destinationID >= 0 then
				incomingRoutes[destinationID] = incomingRoutes[destinationID]
					or { foreign = 0, domestic = 0, origins = {} };
				incomingRoutes[destinationID].domestic = incomingRoutes[destinationID].domestic + 1;
			end
			-- The Trading Posts on the route's OWN path, which is the host's
			-- pathfinder's and not a straight line: Ostia -> Aquileia (run
			-- civvis-20260816T200454Z, t144-154) read "+2 from Outgoing Trade
			-- Routes" against a model that walked the straight line and found
			-- one post — the road runs through Cumae. `GetTradeRoutePath` is
			-- what the shipped TradeRouteChooser draws; each city plot on it
			-- (the origin excluded, the destination included) is asked
			-- `HasActiveTradingPost(pid)` and filed by owner, since a post at
			-- home pays only under Rome's trait and a post abroad pays
			-- `TRADING_POST_GOLD_IN_FOREIGN_CITY`. nil when unreadable.
			local postsOwn, postsForeign = nil, nil;
			pcall(function()
				local manager = Game.GetTradeManager();
				if manager == nil then return; end
				local path = manager:GetTradeRoutePath(pid, originID, destinationPlayer, destinationID);
				if type(path) ~= "table" then return; end
				local own, foreign = 0, 0;
				for _, plotIndex in ipairs(path) do
					local plot = Map.GetPlotByIndex(plotIndex);
					if plot ~= nil and plot:IsCity() then
						local there = Cities.GetCityInPlot(plot:GetX(), plot:GetY());
						if there ~= nil
							and not (there:GetOwner() == pid and there:GetID() == originID)
							and there:GetTrade():HasActiveTradingPost(pid) then
							if there:GetOwner() == pid then own = own + 1; else foreign = foreign + 1; end
						end
					end
				end
				postsOwn, postsForeign = own, foreign;
			end);
			-- What the route PAYS its origin, summed the way the shipped
			-- TradeSupport.lua sums it for the Trade Overview: yields from the
			-- route (destination districts), from the path (Trading Posts) and
			-- from modifiers (policies, Great People, wonders), each yield under
			-- the origin player's international multiplier. The model cannot
			-- always derive this — a destination's Campus may stand on ground
			-- the seat has never seen (Ostia -> Stockholm read "+1 Science",
			-- run civvis-20260816T233226Z t177+) — so the host's figure is
			-- carried and stands in for the model's. nil when unreadable.
			local routeYields = nil;
			pcall(function()
				local manager = Game.GetTradeManager();
				if manager == nil then return; end
				local fromRoute = manager:CalculateOriginYieldsFromPotentialRoute(pid, originID, destinationPlayer, destinationID);
				local fromPath = manager:CalculateOriginYieldsFromPath(pid, originID, destinationPlayer, destinationID);
				local fromModifiers = manager:CalculateOriginYieldsFromModifiers(pid, originID, destinationPlayer, destinationID);
				if type(fromRoute) ~= "table" then return; end
				local playerTrade = destinationPlayer ~= pid and player:GetTrade() or nil;
				local out = {};
				local names = { food = "FOOD", production = "PRODUCTION", gold = "GOLD", science = "SCIENCE", culture = "CULTURE", faith = "FAITH" };
				for key, tag in pairs(names) do
					local index = YieldTypes[tag];
					if index ~= nil then
						local total = (fromRoute[index + 1] or 0)
							+ ((type(fromPath) == "table" and fromPath[index + 1]) or 0)
							+ ((type(fromModifiers) == "table" and fromModifiers[index + 1]) or 0);
						local mult = playerTrade and playerTrade:GetInternationalYieldModifier(index) or 1;
						if type(mult) ~= "number" or mult <= 0 then mult = 1; end
						out[key] = total * mult;
					end
				end
				routeYields = out;
			end);
			tradeRoutes[#tradeRoutes + 1] = {
				trader = try(function() return route.TraderUnitID; end, -1),
				origin = originID,
				destination = destinationID,
				destination_player = destinationPlayer,
				origin_x = try(function() return city:GetX(); end, -1),
				origin_y = try(function() return city:GetY(); end, -1),
				destination_x = try(function() return destination and destination:GetX(); end, -1),
				destination_y = try(function() return destination and destination:GetY(); end, -1),
				posts_own = postsOwn,
				posts_foreign = postsForeign,
				yields = routeYields,
			};
		end
		local queue = try(function()
			local q = city:GetBuildQueue();
			return q and q:GetCurrentProductionTypeHash() or 0;
		end, 0);
		-- Once per city, not three times: this runs for every city every turn and
		-- each call is three guarded engine reads.
		local loyalNow, loyalRate, loyalFallsTo = cityLoyalty(city);
		-- Same discipline: one resolve of the hash to its kind, two engine reads.
		local prodProgress, prodCost = productionProgress(city, queue);
		local defStrength, defDamage, defMax, wallDamage, wallMax = cityDefence(
			try(function() return city:GetX(); end, -1),
			try(function() return city:GetY(); end, -1));
		-- What this city has ALREADY built. Same reason as `im` in the tiles export: a
		-- city whose buildings are invisible looks empty forever, so CIVVIS keeps
		-- ordering the granary it finished twenty turns ago.
		local built = {};
		-- ★★★★★ AND WHICH OF THEM ARE PILLAGED. `HasBuilding` stays true for a
		-- pillaged Library, and a pillaged building pays nothing until it is
		-- repaired. Districts already cross with their pillage bit; buildings
		-- did not, so a raid that pillaged Antium's Library and University left
		-- the mirror paying +6 Science on a Campus the host had at +0 for
		-- twenty turns (run civvis-20260816T011314Z t147-t170: host 5.9, model
		-- 11.2), and CIVVIS could not see that "Repair Library" was the build
		-- that mattered. `IsPillaged` is the accessor the shipped CitySupport
		-- reads for the same fact. nil, not an empty list, when the collection
		-- cannot be read — "could not ask" must stay distinguishable from
		-- "nothing pillaged".
		local pillagedBuildings = nil;
		local blds = try(function() return city:GetBuildings(); end);
		if blds ~= nil then
			pillagedBuildings = {};
			for row in GameInfo.Buildings() do
				if try(function() return blds:HasBuilding(row.Index); end, false) then
					built[#built + 1] = row.BuildingType;
					if try(function() return blds:IsPillaged(row.Index); end, false) then
						pillagedBuildings[#pillagedBuildings + 1] = row.BuildingType;
					end
				end
			end
		end
		-- ★★★★★ AND WHAT IT HAS DISTRICTED, WITH THE PLOT.
		--
		-- `districts` was **null on all 23,677 city records ever exported**, across
		-- every run in `civvis-civ6-runs`. The field was in the schema and never once
		-- filled, so a district is invisible exactly the way a building used to be.
		--
		-- ⚠ THE PLOT IS THE POINT, not the name. `mirror.rs` refuses to reconstruct a
		-- district and says why: "`Item::District` carries a `pos` the export does not
		-- give, and inventing one would place a district on arbitrary ground." So
		-- `civvis_production_item` returns None for EVERY district, a city that is
		-- currently BUILDING one looks IDLE, and CIVVIS re-decides the same production
		-- next turn. Measured on run civvis-20260731T163924Z: the capital was ordered
		-- to build DISTRICT_GOVERNMENT on 60 turns between t46 and t128, every one
		-- answered `applied: true`, and it still showed pop 10 and three buildings at
		-- t130. Sixty of that run's ~91 build orders went into one district.
		--
		-- Districts are also the score gap this file already complains about ("ZERO
		-- districts, campuses, libraries ... 203 against 1088"): they are where
		-- Civilization VI's score and yields come from.
		--
		-- ⚠ Walk the city's PLOTS, not `GetDistricts()`: a plot carries the type AND
		-- the position together, and the collection's per-member accessors vary across
		-- this build. Left nil when the plots cannot be read, so "could not ask" stays
		-- distinguishable from "this city has none" -- the same rule as `built`.
		local placed = nil;
		local wonders = nil;
		-- Plot:GetDistrictType gives us placement, but not whether that placement
		-- is a finished district or a foundation still under construction.  Keep
		-- the collection's authoritative completion bit keyed by plot so the
		-- mirror does not grant yields early or erase an occupied foundation.
		local districtComplete = {};
		-- Hit points, keyed by plot exactly as completion is. The `placed` loop
		-- below walks PLOTS and has no district handle, so anything the district
		-- object knows has to be collected here. See the health block in `placed`
		-- for why this is load-bearing.
		local districtHealth = {};
		local cityDistricts = try(function() return city:GetDistricts(); end);
		if cityDistricts ~= nil then
			for _, district in cityDistricts:Members() do
				local dx = try(function() return district:GetX(); end, -1);
				local dy = try(function() return district:GetY(); end, -1);
				if dx ~= nil and dy ~= nil and dx >= 0 and dy >= 0 then
					local key = tostring(dx) .. "," .. tostring(dy);
					districtComplete[key] =
						try(function() return district:IsComplete(); end, nil);
					districtHealth[key] = {
						damage = try(function()
							return district:GetDamage(DefenseTypes.DISTRICT_GARRISON);
						end, -1),
						max_damage = try(function()
							return district:GetMaxDamage(DefenseTypes.DISTRICT_GARRISON);
						end, -1),
						wall_damage = try(function()
							return district:GetDamage(DefenseTypes.DISTRICT_OUTER);
						end, -1),
						max_wall_damage = try(function()
							return district:GetMaxDamage(DefenseTypes.DISTRICT_OUTER);
						end, -1),
					};
				end
			end
		end
		local ownedPlots = try(function()
			return Map.GetCityPlots():GetPurchasedPlots(city);
		end);
		local worked = nil;
		local specialists = nil;
		if ownedPlots ~= nil then
			placed = {};
			wonders = {};
			worked = {};
			specialists = {};
			local citizens = try(function() return city:GetCitizens(); end);
			for _, plotIndex in ipairs(ownedPlots) do
				local plot = try(function() return Map.GetPlotByIndex(plotIndex); end);
				if plot ~= nil then
					local px = try(function() return plot:GetX(); end, -1);
					local py = try(function() return plot:GetY(); end, -1);
					if citizens ~= nil and try(function()
						return citizens:IsPlotWorked(px, py);
					end, false) then
						-- ★★★★★ THE PLOT'S OWN YIELDS, SO A DRIFT CAN NAME ITS TILE.
						--
						-- The city total is exported below, and `mirror.rs`
						-- reconciles it as one number per yield per city. That
						-- says BY HOW MUCH the model disagrees, never WHERE:
						-- a +2 Food gap could be a coast tile the Lighthouse
						-- feeds, a specialist counted twice, or a resource the
						-- tile catalogue mislabels, and totals cannot tell them
						-- apart. `Plot:GetYield(index)` is what the shipped
						-- PlotInfo paints on the map, evaluated for the owner
						-- exactly as the city panel does, so it is the tile-level
						-- ledger CIVVIS's own tile model can be diffed against.
						-- Guarded per plot: a plot the read fails on carries no
						-- yields rather than zeros.
						worked[#worked + 1] = {
							x = px, y = py,
							yields = plotYields(plot),
						};
					end
					local dtype = try(function() return plot:GetDistrictType(); end, -1);
					if dtype ~= nil and dtype >= 0 then
						local info = GameInfo.Districts[dtype];
						if info ~= nil then
							local workers = try(function() return plot:GetWorkerCount(); end, 0) or 0;
							if info.DistrictType ~= "DISTRICT_CITY_CENTER"
								and info.DistrictType ~= "DISTRICT_WONDER" then
								for _ = 1, workers do
									specialists[#specialists + 1] = info.DistrictType;
								end
							end
							placed[#placed + 1] = {
								type = info.DistrictType,
								x = px,
								y = py,
								-- ★★★★★ ASK THE CITY DISTRICTS COLLECTION, AS FIRAXIS DOES.
								--
								-- `Plot:IsDistrictPillaged()` is not the stock UI API. It
								-- throws on this build, `try` converted that to `false`, and
								-- every damaged district was exported as healthy. Live turn
								-- 112 then showed Ostia's Campus as `pillaged=false` while
								-- `CanProduce` refused its Library with "The required district
								-- is damaged." The shipped PlotToolTip and MapSearchPanel both
								-- call `cityDistricts:IsPillaged(type, plotId)`.
								pillaged = try(function()
									return cityDistricts ~= nil
										and cityDistricts:IsPillaged(dtype, plotIndex);
								end, false),
								complete = districtComplete[
									tostring(try(function() return plot:GetX(); end, -1)) .. "," ..
									tostring(try(function() return plot:GetY(); end, -1))],
								-- ★★★★★ THE DISTRICT'S HEALTH, WHICH DECIDED 121 TURNS OF
								-- PRODUCTION AND WAS NEVER EXPORTED.
								--
								-- `pillaged` is a boolean and a district can be DAMAGED
								-- without being pillaged, so it cannot stand in for hit
								-- points. Nothing carried them, `mirror.rs` never set
								-- `City::encampment_hp`, and it defaults to **0**.
								--
								-- `Game::can_produce` gates `repair_encampment` on
								-- `encampment_hp < 100`, so on every mirrored board that
								-- test passed for any city holding an Encampment -- forever.
								-- The AI queued the repair every turn, `civvis_orders`
								-- correctly refuses to translate a project Civ 6 does not
								-- have, the order was discarded, and NOTHING ELSE was
								-- ordered for that city.
								--
								-- Measured on live run `civvis-20260810T040916Z`
								-- (Rome/Trajan, Settler, Online): Ravenna and Lugdunum --
								-- exactly the two cities holding an Encampment -- sat at
								-- `producing_hash 0, cost -1, progress -1` from turn 67 to
								-- turn 188, with production yields of 8 and 9 against
								-- Rome's 28. 238 discarded orders, **10.4% of every order
								-- CIVVIS issued all game**, and `ENDTURN_BLOCKING_PRODUCTION`
								-- was the dominant blocker of the run because two queues
								-- were permanently empty.
								--
								-- ⚠ AND THE RECORDED FIX WOULD HAVE MADE IT WORSE. The
								-- standing plan was to map `repair_encampment` onto a
								-- district BUILD. Both Encampments export as
								-- `pillaged=false, complete=true` -- they are UNDAMAGED --
								-- so that mapping would have rebuilt two healthy districts
								-- from scratch. The missing translation was never the bug.
								--
								-- `GetDamage`/`GetMaxDamage` on the district, keyed by
								-- `DefenseTypes`, are what the shipped CityBannerManager and
								-- PlotToolTip read; the city-level pair beside them in this
								-- same export already uses the identical shape.
								damage = (districtHealth[px .. "," .. py] or {}).damage,
								max_damage = (districtHealth[px .. "," .. py] or {}).max_damage,
								wall_damage = (districtHealth[px .. "," .. py] or {}).wall_damage,
								max_wall_damage =
									(districtHealth[px .. "," .. py] or {}).max_wall_damage,
							};
						end
					end
					-- World wonders are Building rows attached to their own map plot.
					-- `built` above proves ownership but cannot preserve placement, and
					-- putting every wonder on the City Center changes adjacency rules.
					local wonderType = try(function() return plot:GetWonderType(); end, -1);
					if wonderType ~= nil and wonderType >= 0 and
						try(function() return plot:IsWonderComplete(); end, false) then
						local wonder = GameInfo.Buildings[wonderType];
						if wonder ~= nil then
							wonders[#wonders + 1] = {
								type = wonder.BuildingType,
								x = try(function() return plot:GetX(); end, -1),
								y = try(function() return plot:GetY(); end, -1),
							};
						end
					end
				end
			end
		end
		-- Great People disappear after activation, but their Great Works are permanent
		-- state.  Export the occupied slots exactly so the next reconstruction does
		-- not forget the yield, Tourism, theming identity, or consumed slot.
		local greatWorks = nil;
		if blds ~= nil then
			greatWorks = {};
			for buildingInfo in GameInfo.Buildings() do
				if try(function() return blds:HasBuilding(buildingInfo.Index); end, false) then
					local slots = try(function()
						return blds:GetNumGreatWorkSlots(buildingInfo.Index);
					end, 0) or 0;
					for slot = 0, slots - 1 do
						local workIndex = try(function()
							return blds:GetGreatWorkInSlot(buildingInfo.Index, slot);
						end, -1);
						if workIndex ~= nil and workIndex >= 0 then
							local workType = try(function()
								return blds:GetGreatWorkTypeFromIndex(workIndex);
							end, -1);
							local info = workType ~= nil and workType >= 0
								and GameInfo.GreatWorks[workType] or nil;
							local instance = try(function()
								return Game.GetGreatWorkDataFromIndex(workIndex);
							end);
							if info ~= nil then
								greatWorks[#greatWorks + 1] = {
									type = info.GreatWorkType,
									object = info.GreatWorkObjectType,
									era = info.EraType,
									creator = instance and instance.CreatorName or "",
									building = buildingInfo.BuildingType,
									slot = slot,
								};
							end
						end
					end
				end
			end
		end
		local exactYields = try(function()
			return {
				food = city:GetYield(YieldTypes.FOOD),
				production = city:GetYield(YieldTypes.PRODUCTION),
				gold = city:GetYield(YieldTypes.GOLD),
				science = city:GetYield(YieldTypes.SCIENCE),
				culture = city:GetYield(YieldTypes.CULTURE),
				faith = city:GetYield(YieldTypes.FAITH),
			};
		end);
		-- ★★★★★ WHERE EACH YIELD COMES FROM, IN THE HOST'S OWN WORDS.
		--
		-- `City:GetYieldToolTip(yield)` is the text behind the city panel's
		-- per-yield tooltip: the same "+N from Buildings / Citizens / Districts /
		-- Trade Routes / ..." ledger a player reads. Exporting the total alone
		-- left every model-versus-host gap a guessing game (a +4 Gold step in
		-- two cities on one turn was chased through policies, religion, envoys
		-- and city-states without an answer). Raw and localized, one string per
		-- yield: the consumer parses the amounts, and nothing here has to know
		-- the wording. Absent when the accessor is missing on a build.
		local yieldSources = try(function()
			-- Compacted, not paraphrased: icon tags carry no information the
			-- yield key does not, and the markup newline becomes a real one so
			-- the record stays a few hundred bytes per yield. The amounts and
			-- the source names are untouched.
			local function compact(text)
				if text == nil then return nil; end
				text = tostring(text):gsub("%[ICON_[%w_]+%]", "");
				text = text:gsub("%[NEWLINE%]", "\n"):gsub("[ \t]+", " ");
				-- Parenthesised: gsub returns (string, count) and the caller
				-- wants exactly one value. Capped: a ledger is a dozen short
				-- lines, and a state record already reaches ~75 KB on a large
				-- empire; six of these per city must not double it.
				return (text:gsub("^%s+", ""):gsub("%s+$", "")):sub(1, 400);
			end
			return {
				food = compact(city:GetYieldToolTip(YieldTypes.FOOD)),
				production = compact(city:GetYieldToolTip(YieldTypes.PRODUCTION)),
				gold = compact(city:GetYieldToolTip(YieldTypes.GOLD)),
				science = compact(city:GetYieldToolTip(YieldTypes.SCIENCE)),
				culture = compact(city:GetYieldToolTip(YieldTypes.CULTURE)),
				faith = compact(city:GetYieldToolTip(YieldTypes.FAITH)),
			};
		end);
		local centerPlot = try(function()
			return Map.GetPlot(city:GetX(), city:GetY());
		end);
		cities[#cities + 1] = {
			districts = placed,
			wonders = wonders,
			worked = worked,
			specialists = specialists,
			great_works = greatWorks,
			yields = exactYields,
			yield_sources = yieldSources,
			incoming_routes = incomingRoutes[try(function() return city:GetID(); end, -1)],
			-- The city centre's own plot yields, which the worked list carries
			-- (Firaxis counts the centre as worked) but which CIVVIS floors to
			-- 2 Food / 1 Production before assigning citizens.
			center_yields = centerPlot ~= nil and plotYields(centerPlot) or nil,
			-- ★★★★★ WHOSE RELIGION THIS CITY FOLLOWS, AND WHAT IS CONVERTING IT.
			--
			-- `religion` was **null on all 26,954 city records ever exported**, the
			-- same shape as `districts` before it: the field is in the schema, the
			-- mod reads only the PLAYER-level religion object, and no city ever
			-- reported one. So a city can be converted away from us turn by turn and
			-- the mirror says nothing, and CIVVIS can neither pursue a religious
			-- victory nor defend against one.
			--
			-- That is not hypothetical here. Two consecutive completed games were
			-- lost to the same victory type well before the turn limit -- Greece at
			-- t124 on a Duel, Spain at t182 on a tiny map -- while CIVVIS sat on
			-- 327 and 953 unspent faith with a pantheon and no religion.
			--
			-- `next`/`turns` matter as much as the majority: a city at 100 loyalty
			-- falling fast reads identically to a stable one, and the same is true of
			-- conversion. This is the `loyalty_per_turn` lesson applied to faith.
			religion = cityReligion(city),
			religion_next = cityNextReligion(city),
			religion_turns = try(function()
				return city:GetReligion():GetTurnsToNextReligion();
			end, -1),
			pantheon_active = try(function()
				local p = city:GetReligion():GetActivePantheon();
				if p == nil or p < 0 then return nil; end
				return GameInfo.Beliefs[p] and GameInfo.Beliefs[p].BeliefType or nil;
			end),
			id = try(function() return city:GetID(); end, -1),
			-- The banner name, so the mirror can label the same city the same way.
			-- Without it the reconstruction falls back to CIVVIS's own list for
			-- whatever civilization it assigned, which is how a Persian game came
			-- out as ROME / OSTIA / ANTIUM on the left-hand screen.
			name = try(function() return Locale.Lookup(city:GetName()); end, ""),
			buildings = built,
			pillaged_buildings = pillagedBuildings,
			x = try(function() return city:GetX(); end, -1),
			y = try(function() return city:GetY(); end, -1),
			pop = try(function() return city:GetPopulation(); end, -1),
			capital = try(function() return city:IsCapital(); end, false),
			-- The NAME, not the hash. See `productionName`: shipping the raw hash
			-- meant the mirror never knew what any city was already building, so
			-- CIVVIS re-decided production every turn blind to work in progress.
			producing = productionName(queue),
			producing_hash = queue,
			-- ★★★★★ WHAT THE CITY IS BUILDING WAS EXPORTED; HOW FAST, AND HOW FAR
			-- ALONG, WERE NOT. That is the whole reason the settler stall has never
			-- been diagnosable.
			--
			-- Measured on run civvis-20260802T041527Z (Russia, Settler/Small/Online):
			-- the capital was on UNIT_SETTLER for **84 turns** — the most-produced
			-- item of the game — across 12 separate stretches of 6 to 11 turns, and
			-- **not one settler ever existed**. Zero settlers alive on any of 171
			-- turns, so the empire sat on ONE city while `settle_choice` re-picked
			-- the same site (61,12) from turn 13 to turn 141.
			--
			-- Nothing could say why, because the export carried no production yield,
			-- no accumulated progress and no turns-remaining. Nine uninterrupted
			-- turns on a settler (t130-t138) produced nothing and the stream had no
			-- field that could distinguish "the city makes 2 production a turn" from
			-- "progress is being reset" from "completion is blocked".
			--
			-- ⚠ It is also a decision input, not only a diagnostic. CIVVIS chooses
			-- what to build with no idea what the city can actually finish, which is
			-- the same class of blindness as `producing` itself once was — see the
			-- note directly above.
			--
			-- ⚠ Each is guarded separately. `GetBuildQueue` exists on this build but
			-- these accessors are exactly the shape that has silently returned nil
			-- before (see the `GetDefenseStrength` note below, which read -1 for the
			-- project's entire history). A missing one must leave -1 and not take
			-- the others with it.
			production = try(function()
				return city:GetBuildQueue():GetProductionYield();
			end, -1),
			-- ⚠ Typed accessors, not a generic one — see `productionProgress`.
			-- The obvious `GetCurrentProductionProgress()` does not exist and
			-- read -1 on every city of every turn until this was fixed.
			production_progress = prodProgress,
			production_cost = prodCost,
			production_turns = try(function()
				return city:GetBuildQueue():GetTurnsLeft();
			end, -1),
			food = try(function() return city:GetGrowth():GetFood(); end, -1),
			-- ★★★★★ THE EMPIRE'S HAPPINESS WAS NEVER ASKED FOR, AND IT MULTIPLIES
			-- EVERY YIELD ON THE BOARD.
			--
			-- Neither mod exported a single amenity, happiness or luxury field, and
			-- `mirror.rs` imported none — so CIVVIS's entire happiness picture was
			-- something it derived from its own rules on the reconstructed board and
			-- then never checked. `Game::amenity_yield_mult_for` bands that derived
			-- surplus straight into a multiplier on science, production, gold,
			-- culture and faith: +5 -> 1.20, 0 -> 1.00, -4 -> 0.80, -6 -> 0.70.
			--
			-- CIVVIS's model says the live empires are sitting at -4/-5, i.e. paying a
			-- **25-30% tax on every yield**. That may be exactly right, or it may be a
			-- number it invented; with nothing from the host there is no way to know,
			-- and the economy drift line cannot tell a real tax from a modelled one
			-- because an overestimate elsewhere would cancel it.
			--
			-- `GetHappinessNonFoodYieldModifier` is the host's OWN multiplier — the
			-- same quantity CIVVIS bands for itself — so the two can finally be
			-- compared rather than assumed.
			--
			-- ⚠ Every call here is one the shipped CityPanel makes on
			-- `city:GetGrowth()`, taken from the Assets rather than guessed: this file
			-- has already paid for `GetDistricts():GetDefenseStrength()` (a method on
			-- the collection, which read -1 on every city for the project's whole
			-- history) and for a hash lookup that segfaulted the game four times.
			-- ⚠ -1 sentinels, not 0: a city with zero amenities and a city the read
			-- failed on must not look the same to the reconstruction.
			-- Housing, and where it comes from. Population is the term every
			-- yield is a linear function of -- five completed live games put
			-- science at 1.16 x pop, with city COUNT predicting nothing -- and
			-- `housing_growth_mult` gates growth on the headroom over
			-- population: >= 2 full, >= 1 HALF, below -4 ZERO.
			--
			-- ⚠ CIVVIS has been reconstructing this from its own rules and has
			-- never been able to check it, which is exactly the position
			-- Amenities were in before #967 -- and there a claim I had made
			-- from the model turned out to be unverifiable and had to be
			-- retracted. This exports the number so the model can be CHECKED;
			-- it is not a claim that the model is wrong.
			housing = try(function() return city:GetGrowth():GetHousing(); end, -1),
			housing_from_improvements = try(function()
				return city:GetGrowth():GetHousingFromImprovements();
			end, -1),
			-- The rest of the host's housing ledger, one term each, so a mirror
			-- that disagrees with the total can say which term it got wrong.
			-- Every accessor is one the shipped CitySupport calls on
			-- `city:GetGrowth()`; -1 is the per-field "could not read" sentinel.
			housing_from_water = try(function()
				return city:GetGrowth():GetHousingFromWater();
			end, -1),
			housing_from_buildings = try(function()
				return city:GetGrowth():GetHousingFromBuildings();
			end, -1),
			housing_from_districts = try(function()
				return city:GetGrowth():GetHousingFromDistricts();
			end, -1),
			housing_from_civics = try(function()
				return city:GetGrowth():GetHousingFromCivics();
			end, -1),
			housing_from_great_people = try(function()
				return city:GetGrowth():GetHousingFromGreatPeople();
			end, -1),
			housing_from_starting_era = try(function()
				return city:GetGrowth():GetHousingFromStartingEra();
			end, -1),
			housing_from_great_works = try(function()
				return city:GetGrowth():GetHousingFromGreatWorks();
			end, -1),
			-- Growth, as the host computes it: the surplus after consumption,
			-- the next-citizen threshold, the housing/happiness multipliers and
			-- the turns the host itself forecasts. `food` above is the stockpile.
			food_surplus = try(function()
				return city:GetGrowth():GetFoodSurplus();
			end, -1),
			growth_threshold = try(function()
				return city:GetGrowth():GetGrowthThreshold();
			end, -1),
			growth_turns = try(function()
				return city:GetGrowth():GetTurnsUntilGrowth();
			end, -1),
			housing_growth_mult = try(function()
				return city:GetGrowth():GetHousingGrowthModifier();
			end, -1),
			happiness_growth_mult = try(function()
				return city:GetGrowth():GetHappinessGrowthModifier();
			end, -1),
			overall_growth_mult = try(function()
				return city:GetGrowth():GetOverallGrowthModifier();
			end, -1),
			amenities = try(function() return city:GetGrowth():GetAmenities(); end, -1),
			amenities_needed = try(function()
				return city:GetGrowth():GetAmenitiesNeeded();
			end, -1),
			happiness = try(function() return city:GetGrowth():GetHappiness(); end, -1),
			happiness_yield_mult = try(function()
				return city:GetGrowth():GetHappinessNonFoodYieldModifier();
			end, -1),
			-- WHERE the amenities come from, so a shortfall names its own repair
			-- rather than only its size. Luxuries are the lever CIVVIS can actually
			-- pull (improve one, or trade for one); entertainment is a district it
			-- can build.
			amenities_luxuries = try(function()
				return city:GetGrowth():GetAmenitiesFromLuxuries();
			end, -1),
			amenities_entertainment = try(function()
				return city:GetGrowth():GetAmenitiesFromEntertainment();
			end, -1),
			amenities_civics = try(function()
				return city:GetGrowth():GetAmenitiesFromCivics();
			end, -1),
			amenities_city_states = try(function()
				return city:GetGrowth():GetAmenitiesFromCityStates();
			end, -1),
			amenities_war_weariness = try(function()
				return city:GetGrowth():GetAmenitiesLostFromWarWeariness();
			end, -1),
			amenities_bankruptcy = try(function()
				return city:GetGrowth():GetAmenitiesLostFromBankruptcy();
			end, -1),
			-- The remaining amenity sources the shipped CitySupport reads, so the
			-- host's count decomposes completely: the six above plus these sum to
			-- `amenities`, and a model that disagrees can name the term.
			amenities_great_people = try(function()
				return city:GetGrowth():GetAmenitiesFromGreatPeople();
			end, -1),
			amenities_religion = try(function()
				return city:GetGrowth():GetAmenitiesFromReligion();
			end, -1),
			amenities_national_parks = try(function()
				return city:GetGrowth():GetAmenitiesFromNationalParks();
			end, -1),
			amenities_starting_era = try(function()
				return city:GetGrowth():GetAmenitiesFromStartingEra();
			end, -1),
			amenities_improvements = try(function()
				return city:GetGrowth():GetAmenitiesFromImprovements();
			end, -1),
			amenities_districts = try(function()
				return city:GetGrowth():GetAmenitiesFromDistricts();
			end, -1),
			amenities_natural_wonders = try(function()
				return city:GetGrowth():GetAmenitiesFromNaturalWonders();
			end, -1),
			-- ⚠ Was `GetDistricts():GetDefenseStrength()` — the method on the
			-- collection, which does not exist, so this read -1 for the whole
			-- project's history on every city on the board.
			defense = defStrength,
			damage = defDamage,
			max_damage = defMax,
			wall_damage = wallDamage,
			max_wall_damage = wallMax,
			-- ⚠ THE FIELD WHOSE ABSENCE HID THE BIGGEST DEFECT IN THE PROJECT.
			-- 45 runs of telemetry recorded cities peaking and then declining with
			-- no cause attached, because loyalty was never exported. `falls_to` is
			-- the game's own verdict on who the city is about to be lost to.
			loyalty = loyalNow,
			loyalty_per_turn = loyalRate,
			falls_to = loyalFallsTo,
		};
	end);

	-- Empty Great Work slots empire-wide, with the tiles they stand on. See
	-- `CivvisGreatWorks` for the two defects this replaces: the old
	-- class->object constant here spelt object types a way the database does
	-- not (`GREAT_WORK_OBJECT_WRITING` for `GREATWORKOBJECT_WRITING`, and an
	-- `_ART` that does not exist at all), so `empty_slots` exported 0 for
	-- every cultural person ever seen — and 0 is exactly the value the brain
	-- stands still on. Worst measured run civvis-20260818T052156Z: fourteen
	-- idle cultural people, sixteen matching empty slots, `empty_slots: 0`
	-- on every one of them.
	local gwSurvey = CivvisGreatWorks.survey(player, turn);

	local units = {};
	eachUnit(player, function(unit)
		local name = unitTypeName(unit);
		local row = GameInfo.Units[name];
		-- Great People are immediate effects in CIVVIS, but physical units in
		-- Firaxis.  Export the engine's exact activation targets so the bridge can
		-- perform that same effect without guessing which district or plot is legal.
		local greatPerson = nil;
		local gp = try(function() return unit:GetGreatPerson(); end);
		if gp ~= nil and try(function() return gp:IsGreatPerson(); end, false) then
			local individual = try(function() return gp:GetIndividual(); end, -1);
			local class = try(function() return gp:GetClass(); end, -1);
			local individualRow = GameInfo.GreatPersonIndividuals[individual];
			local classRow = GameInfo.GreatPersonClasses[class];
			local classType = classRow ~= nil and classRow.GreatPersonClassType or nil;
			-- How many empty slots the empire has that this person's work
			-- fits, and the tiles they stand on. nil for classes that do not
			-- consume slots, and nil when the slot tables were unreadable —
			-- never 0 by default, because 0 is a claim ("build capacity") and
			-- nil is an absence.
			local emptySlots, openPlots = CivvisGreatWorks.matches(gwSurvey,
				CivvisGreatWorks.objectsFor(individualRow ~= nil
					and individualRow.GreatPersonIndividualType or nil, classType));
			local activationPlots = {};
			for _, plotIndex in ipairs(try(function()
				return gp:GetActivationHighlightPlots();
			end, {}) or {}) do
				local plot = try(function() return Map.GetPlotByIndex(plotIndex); end);
				if plot ~= nil then
					local px = try(function() return plot:GetX(); end, -1);
					local py = try(function() return plot:GetY(); end, -1);
					if px >= 0 and py >= 0 then
						-- Three-valued on purpose, and only for slot consumers:
						-- true = a compatible empty slot stands here; false =
						-- one of our districts with no such slot (the tile
						-- eleven people wedged on); nil/absent = unknown (a
						-- wonder tile, or no survey). The brain must never
						-- read absence as either claim.
						local slotOpen = nil;
						if openPlots ~= nil then
							if openPlots[plotIndex] then slotOpen = true;
							elseif gwSurvey.district_plots[plotIndex] then
								slotOpen = false;
							end
						end
						activationPlots[#activationPlots + 1] = {
							x = px, y = py,
							distance = try(function()
								return Map.GetPlotDistance(unit:GetX(), unit:GetY(), px, py);
							end, 9999),
							slot_open = slotOpen,
						};
					end
				end
			end
			greatPerson = {
				individual = individualRow ~= nil
					and individualRow.GreatPersonIndividualType or nil,
				class = classType,
				empty_slots = emptySlots,
				-- The timeline moves on as soon as this person is recruited, so the
				-- current offer cannot tell the planner what district this physical
				-- unit still needs. Carry the exact per-individual database gate.
				required_district = individualRow ~= nil
					and individualRow.ActionRequiresCompletedDistrictType or nil,
				charges = try(function() return gp:GetActionCharges(); end, 0),
				can_activate = try(function()
					return UnitManager.CanStartCommand(
						unit, CMD["UNITCOMMAND_ACTIVATE_GREAT_PERSON"], nil, {});
				end, false),
				activation_plots = activationPlots,
			};
		end
		local progress = unitProgress(unit);
		CivvisLedger.kinds[tostring(try(function() return unit:GetID(); end, -1))] = name;
		units[#units + 1] = {
			id = try(function() return unit:GetID(); end, -1),
			kind = name,
			-- See `unitBaseType`: what this replaces, when it is a civ unique.
			base = unitBaseType(name),
			-- See `unitClass`: the fallback for a unique that replaces nothing.
			class = unitClass(name),
			x = try(function() return unit:GetX(); end, -1),
			y = try(function() return unit:GetY(); end, -1),
			hp = 100 - (try(function() return unit:GetDamage(); end, 0) or 0),
			moves = try(function() return unit:GetMovesRemaining(); end, -1),
			xp = progress.xp,
			level = progress.level,
			promotions = progress.promotions,
			build_charges = progress.build_charges,
			spread_charges = progress.spread_charges,
			religion = progress.religion,
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
			fortify_turns = try(function() return unit:GetFortifyTurns(); end, 0),
			-- The count is the public formation state used by the stock Unit Panel.
			-- Without it, a successfully escorted Settler is reconstructed as two
			-- loose units and CIVVIS asks to link them again on every turn.
			formation_count = try(function()
				return unit:GetFormationUnitCount();
			end, 1),
			-- Where a queued host path will carry the unit at its next turn
			-- start (nil when none), and whether it is embarked. See CivvisBoard.
			queued_dest = try(function()
				local index = UnitManager.GetQueuedDestination(unit);
				if index == nil then return nil; end
				local plot = Map.GetPlotByIndex(index);
				return plot and { plot:GetX(), plot:GetY() } or nil;
			end, nil),
			embarked = try(function() return unit:IsEmbarked(); end, nil),
			-- Attacks left this turn (`GetAttacksRemaining`, the shipped
			-- SelectedUnit read). The mirror plans a frame's second strike only
			-- for units that still have one.
			attacks_remaining = try(function() return unit:GetAttacksRemaining(); end, nil),
			great_person = greatPerson,
		};
	end);

	local suzerainCounts = publicSuzerainCounts();
	local publicStats = publicEmpireStats(player, suzerainCounts);

	-- Rivals: only what we have actually met, so the mirror never contains
	-- knowledge the seat has not earned.
	local rivals = {};
	local diplomacy = try(function() return player:GetDiplomacy(); end);
	for _, otherId in ipairs(try(function() return PlayerManager.GetAliveMajorIDs(); end, {})) do
		if otherId ~= pid and diplomacy ~= nil
				and try(function() return diplomacy:HasMet(otherId); end, false) then
			local other = Players[otherId];
			local otherPublicStats = publicEmpireStats(other, suzerainCounts);
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
						local theirDef, theirDmg, theirMax, theirWallDmg, theirWallMax =
							cityDefence(cx, cy);
						theirCities[#theirCities + 1] = {
							id = try(function() return city:GetID(); end, -1),
							x = cx, y = cy,
							name = try(function()
								return Locale.Lookup(city:GetName());
							end, ""),
							pop = try(function() return city:GetPopulation(); end, -1),
							capital = try(function() return city:IsCapital(); end, false),
							-- Defence is on the city banner when the city is
							-- visible, so this is information a human has.
							defense = theirDef,
							damage = theirDmg,
							max_damage = theirMax,
							wall_damage = theirWallDmg,
							max_wall_damage = theirWallMax,
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
							local progress = unitProgress(unit);
							theirUnits[#theirUnits + 1] = {
								id = try(function() return unit:GetID(); end, nil),
								x = ux, y = uy, kind = name,
								base = unitBaseType(name),
								class = unitClass(name),
								hp = 100 - (try(function() return unit:GetDamage(); end, 0) or 0),
								moves = try(function() return unit:GetMovesRemaining(); end, -1),
								xp = progress.xp, level = progress.level,
								promotions = progress.promotions,
								build_charges = progress.build_charges,
								spread_charges = progress.spread_charges,
								religion = progress.religion,
								combat = row ~= nil and (row.Combat or 0) or 0,
								ranged = row ~= nil and (row.RangedCombat or 0) or 0,
								fortify_turns = try(function() return unit:GetFortifyTurns(); end, 0),
								fortified = try(function()
									return (unit:GetFortifyTurns() or 0) > 0;
								end, false),
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
				-- These are the rival's public diplomacy-ribbon facts. They carry no
				-- city, unit or research detail, but without them CivVis renders every
				-- fogged rival as Normal Age with an unformed government.
				government = try(function()
					local culture = other:GetCulture();
					if culture == nil then return nil; end
					local index = culture:GetCurrentGovernment();
					if type(index) ~= "number" or index < 0 then return nil; end
					local row = GameInfo.Governments[index];
					return row ~= nil and row.GovernmentType or nil;
				end, nil),
				dark_age = try(function()
					return Game.GetEras():HasDarkAge(otherId);
				end, nil),
				golden_age = try(function()
					return Game.GetEras():HasGoldenAge(otherId);
				end, nil),
				heroic_golden_age = try(function()
					return Game.GetEras():HasHeroicGoldenAge(otherId);
				end, nil),
				at_war = try(function() return diplomacy:IsAtWarWith(otherId); end, false),
				-- Whether this rival currently grants OUR seat Open Borders —
				-- the shipped overview's "received" direction
				-- (DiplomacyActionView.lua:1429, HasOpenBordersFrom). The
				-- mirror unseals the rival's fogged border while this holds,
				-- so a passage the `buy` arm just bought is ground the
				-- planner can actually route through; it also retires the
				-- purchase trigger, so the seat never pays twice.
				open_borders = try(function()
					return diplomacy:HasOpenBordersFrom(otherId);
				end, nil),
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
				-- Diplomatic-victory points, so the denial logic can see the one
				-- victory that ended three of the last thirteen games early.
				dvp = try(function() return other:GetStats():GetDiplomaticVictoryPoints(); end, nil),
				-- ★★★★★ THE NUMBER THE WAR DECISION ACTUALLY NEEDS, and it was never
				-- exported. The old veto compared SCORE ratios, and on Settler the
				-- shipped AI always outscores this seat — so "their score is 2.59x ours"
				-- forbade every war for 190 turns while our 19 units stood on ALERT.
				-- Score counts wonders, techs and population; none of them defends a
				-- city. `GetMilitaryStrength` is what the game's own diplomacy ribbon
				-- shows a human (`DiplomacyRibbon.lua:159`), copied rather than recalled.
				military = try(function() return other:GetStats():GetMilitaryStrength(); end, -1),
				-- ★★★★★ WHY THE SCORE GAP CANNOT BE READ WITHOUT THESE.
				--
				-- Measured over 99 completed runs: CIVVIS leads in ZERO of them. Our
				-- score is a median 267 against the best rival's 1109 — a ratio of
				-- 0.26. Yet on empire SIZE we sit at 0.75-0.80 (3 cities vs 4, pop 28
				-- vs 35) and our cities are individually BIGGER (10.3 pop vs 9.4).
				--
				-- So roughly three quarters of the score gap is in components that are
				-- neither cities nor population, and the export could not name which:
				-- the rival record carried `score`, `cities`, `military` and `units`
				-- and nothing else. "We score a quarter" could not become "we score a
				-- quarter BECAUSE X".
				--
				-- Counts, not name lists: the question is how far ahead they are, and
				-- a name list per rival per turn would be tens of kilobytes of export
				-- for an answer a single integer gives. `-1` on failure, the same
				-- sentinel every other guarded accessor here uses, so a build without
				-- these APIs reads as UNKNOWN rather than as zero.
				techs = try(function()
					local t = other:GetTechs();
					if t == nil then return -1; end
					local n = 0;
					for row in GameInfo.Technologies() do
						if t:HasTech(row.Index) then n = n + 1; end
					end
					return n;
				end, -1),
				civics = try(function()
					local c = other:GetCulture();
					if c == nil then return -1; end
					local n = 0;
					for row in GameInfo.Civics() do
						if c:HasCivic(row.Index) then n = n + 1; end
					end
					return n;
				end, -1),
				-- ★★★★ THE RIVAL'S OWN ECONOMY, AS THE HOST REPORTS IT. Counts of
				-- techs and civics say how far ahead a rival is; these say how
				-- fast it is moving. Every accessor is one the shipped World
				-- Rankings and Deal screens call on OTHER players (`GetTechs():
				-- GetScienceYield`, `GetCulture():GetCultureYield`, `GetStats():
				-- GetTourism`, `GetTreasury():GetGoldBalance`), so the seat learns
				-- nothing a player at the keyboard could not read. Without them
				-- the mirror's rival science and culture were CIVVIS's own guess
				-- from a rival's visible cities — the one part of the standings a
				-- viewer could never trust. -1 on failure, as everywhere here.
				science = try(function() return other:GetTechs():GetScienceYield(); end, -1),
				culture = try(function() return other:GetCulture():GetCultureYield(); end, -1),
				tourism = try(function() return other:GetStats():GetTourism(); end, -1),
				-- ★★★★ RIVAL VICTORY PROGRESS, AS THE SHIPPED SCREEN SHOWS IT.
				--
				-- Five of the twelve runs this seat was LEADING on 2026-08-16/17
				-- ended at t229-245 by a rival completing a culture, technology
				-- or diplomatic victory the mirror could not see coming: rival
				-- space programs and tourist counts never crossed the bridge, so
				-- the victory tracker read zero for every rival on exactly the
				-- lanes that end games early. Every accessor here is one the
				-- shipped World Rankings screen calls on OTHER players
				-- (`GetNumProjectsAdvanced` per space-race project,
				-- `GetCulture():GetTouristsTo()`, `GetCulture():
				-- GetStaycationers()`, WorldRankings.lua:1674-1675), so the seat
				-- learns nothing a player at the keyboard could not read there.
				--
				-- Only the space-race milestones that screen lists — Manhattan
				-- and Ivy are strategic programs, not victory progress, and are
				-- not public the same way. nil (absent) on a build without the
				-- APIs; an empty list is a real "no milestone yet".
				science_projects = try(function()
					local stats = other:GetStats();
					if stats == nil then return nil; end
					local done = {};
					for _, projectType in ipairs({
						"PROJECT_LAUNCH_EARTH_SATELLITE",
						"PROJECT_LAUNCH_MOON_LANDING",
						"PROJECT_LAUNCH_MARS_REACTOR",
						"PROJECT_LAUNCH_MARS_HABITATION",
						"PROJECT_LAUNCH_MARS_HYDROPONICS",
						"PROJECT_LAUNCH_MARS_BASE",
						"PROJECT_LAUNCH_EXOPLANET_EXPEDITION",
					}) do
						local project = GameInfo.Projects[projectType];
						if project ~= nil
								and (stats:GetNumProjectsAdvanced(project.Index) or 0) > 0 then
							done[#done + 1] = projectType;
						end
					end
					return done;
				end, nil),
				-- The culture victory's own two numbers: tourists visiting THEM,
				-- and their staycationers (which set the bar every rival must
				-- clear). -1 on failure, as everywhere here.
				foreign_tourists = try(function()
					return other:GetCulture():GetTouristsTo();
				end, -1),
				domestic_tourists = try(function()
					return other:GetCulture():GetStaycationers();
				end, -1),
				gold = try(function() return other:GetTreasury():GetGoldBalance(); end, -1),
				-- Net, like our own `gold_per_turn` below (yield minus maintenance).
				gold_per_turn = try(function()
					local treasury = other:GetTreasury();
					return treasury:GetGoldYield() - treasury:GetTotalMaintenance();
				end, -1),
				faith = try(function() return other:GetReligion():GetFaithBalance(); end, -1),
				faith_per_turn = try(function() return other:GetReligion():GetFaithYield(); end, -1),
				public_stats = otherPublicStats,
				cities = theirCities,
				units = theirUnits,
			};
		end
	end

	-- Met city-states are first-class public actors. Treating their territory as
	-- merely "cannot settle here" hid Kabul's city, army, Envoys and Suzerain from
	-- both the mirror and the planner even while its banner was on screen.
	local minors = {};
	for _, minor in ipairs(try(function() return PlayerManager.GetAliveMinors(); end, {})) do
		pcall(function()
			local mid = minor:GetID();
			if diplomacy == nil or not diplomacy:HasMet(mid) then return; end
			local civilization = try(function()
				return PlayerConfigurations[mid]:GetCivilizationTypeName();
			end, "");
			local theirCities, theirUnits = {}, {};
			pcall(function()
				for _, city in minor:GetCities():Members() do
					local cx, cy = city:GetX(), city:GetY();
					if plotRevealed(pid, cx, cy) then
						local strength, damage, maxDamage, wallDamage, maxWallDamage =
							cityDefence(cx, cy);
						theirCities[#theirCities + 1] = {
							id = try(function() return city:GetID(); end, -1),
							x = cx, y = cy,
							name = try(function() return Locale.Lookup(city:GetName()); end, ""),
							pop = try(function() return city:GetPopulation(); end, -1),
							capital = try(function() return city:IsCapital(); end, false),
							defense = strength, damage = damage,
							max_damage = maxDamage,
							wall_damage = wallDamage,
							max_wall_damage = maxWallDamage,
						};
					end
				end
			end);
			pcall(function()
				for _, unit in minor:GetUnits():Members() do
					pcall(function()
						local ux, uy = unit:GetX(), unit:GetY();
						if PlayersVisibility[pid]:IsVisible(ux, uy) then
							local name = unitTypeName(unit);
							local row = GameInfo.Units[name];
							local progress = unitProgress(unit);
							theirUnits[#theirUnits + 1] = {
								id = try(function() return unit:GetID(); end, nil),
								x = ux, y = uy, kind = name,
								hp = 100 - (try(function() return unit:GetDamage(); end, 0) or 0),
								moves = try(function() return unit:GetMovesRemaining(); end, -1),
								xp = progress.xp, level = progress.level,
								promotions = progress.promotions,
								build_charges = progress.build_charges,
								spread_charges = progress.spread_charges,
								religion = progress.religion,
								combat = row ~= nil and (row.Combat or 0) or 0,
								ranged = row ~= nil and (row.RangedCombat or 0) or 0,
								fortify_turns = try(function() return unit:GetFortifyTurns(); end, 0),
								fortified = try(function()
									return (unit:GetFortifyTurns() or 0) > 0;
								end, false),
							};
						end
					end);
				end
			end);
			-- `GetAliveMinors()` includes the aggregate Free Cities player from
			-- turn 1 even though it is dormant. Publishing that empty placeholder
			-- used to zip it onto CIVVIS's first generated city-state and turn Kabul
			-- into an enemy. Keep it only once there is an actor to mirror.
			if civilization == "CIVILIZATION_BARBARIAN" then return; end
			if civilization == "CIVILIZATION_FREE_CITIES"
				and #theirCities == 0 and #theirUnits == 0 then return; end
			local influence = try(function() return minor:GetInfluence(); end);
			minors[#minors + 1] = {
				player = mid,
				civ = civilization,
				at_war = try(function() return diplomacy:IsAtWarWith(mid); end, false),
				score = try(function() return minor:GetScore(); end, -1),
				military = try(function() return minor:GetStats():GetMilitaryStrength(); end, -1),
				envoys = influence ~= nil and try(function()
					return influence:GetTokensReceived(pid);
				end, 0) or 0,
				most_envoys = influence ~= nil and try(function()
					return influence:GetMostTokensReceived();
				end, 0) or 0,
				suzerain = influence ~= nil and try(function()
					return influence:GetSuzerain();
				end, -1) or -1,
				cities = theirCities, units = theirUnits,
			};
		end);
	end

	-- ★★★ WITHOUT THESE, CIVVIS DECIDES IN THE ANCIENT ERA FOREVER. The
	-- reconstruction had no research at all, so `civvis-orders` on the turn-190
	-- board of run 101628Z ordered SLINGERS and `TECH_ASTROLOGY` — reasonable
	-- advice for turn 5, worthless at turn 190 against Pikemen. What a seat knows
	-- is half of what it can decide.
	--
	-- ⚠ The completed lists are deliberately different from what is merely
	-- reachable: `CanResearch` would let CIVVIS build from a tree it does not
	-- have.  The active node and its progress are separate facts, needed so a
	-- persistent reconstruction does not forget an in-flight choice each turn.
	-- ⚠⚠ THE EUREKA IS THE LARGEST SCIENCE DISCOUNT IN THE GAME AND NOBODY ASKED
	-- FOR IT. **62 of 77 technologies carry a boost** worth 40-50% of their cost,
	-- and `AdvancedAi::tech_value` already pays **+28** for a tech whose boost is
	-- triggered -- but nothing has ever sent that fact. The state export carried
	-- `techs`, `research` and `research_progress` and no boost field at all, and
	-- `mirror.rs` imported none, so in every live game the agent's
	-- `boosted_techs` is whatever its own simulation happened to derive rather
	-- than what Civilization VI actually granted.
	--
	-- Same class as the Amenity export (#967) and the Housing export (#1007):
	-- the valuation is right and the input is absent. It matters here because the
	-- median live empire ends on **30 technologies of 77** -- taking the
	-- discounted ones first is the cheapest tech-count there is.
	--
	-- `HasBoostBeenTriggered` is the shipped predicate: `TechTree.lua` reads it
	-- as `pPlayerTechs:HasBoostBeenTriggered(iTech)` and `CivicsTree.lua` the same
	-- for civics. Boosts are reported for technologies the empire does NOT yet
	-- have -- a triggered boost on a completed tech is spent and says nothing
	-- about what to research next.
	local techs, civics = {}, {};
	local boosted_techs, boosted_civics = {}, {};
	local ptechs = try(function() return player:GetTechs(); end);
	if ptechs ~= nil then
		for row in GameInfo.Technologies() do
			if try(function() return ptechs:HasTech(row.Index); end, false) then
				techs[#techs + 1] = row.TechnologyType;
			elseif try(function()
				return ptechs:HasBoostBeenTriggered(row.Index);
			end, false) then
				boosted_techs[#boosted_techs + 1] = row.TechnologyType;
			end
		end
	end
	local research, research_progress;
	if ptechs ~= nil then
		local index = try(function() return ptechs:GetResearchingTech(); end, -1);
		if index ~= nil and index >= 0 then
			local row = GameInfo.Technologies[index];
			if row ~= nil then
				research = row.TechnologyType;
				research_progress = try(function()
					return ptechs:GetResearchProgress(index);
				end, 0) or 0;
			end
		end
	end
	local pculture = try(function() return player:GetCulture(); end);
	if pculture ~= nil then
		for row in GameInfo.Civics() do
			if try(function() return pculture:HasCivic(row.Index); end, false) then
				civics[#civics + 1] = row.CivicType;
			elseif try(function()
				return pculture:HasBoostBeenTriggered(row.Index);
			end, false) then
				boosted_civics[#boosted_civics + 1] = row.CivicType;
			end
		end
	end
	local civic, civic_progress;
	if pculture ~= nil then
		local index = try(function() return pculture:GetProgressingCivic(); end, -1);
		if index ~= nil and index >= 0 then
			local row = GameInfo.Civics[index];
			if row ~= nil then
				civic = row.CivicType;
				civic_progress = try(function()
					return pculture:GetCulturalProgress(index);
				end, 0) or 0;
			end
		end
	end

	-- ★★★★★ COMPLETED PROGRAMS ARE HISTORY, NOT CITY QUEUES.
	--
	-- A `--fresh-board` decision starts from a new CIVVIS player every turn. Its
	-- `science_projects` set was therefore empty even after a finished Manhattan
	-- Project, and the live planner selected Manhattan again in city after city.
	-- City production cannot repair that: a completed project leaves every queue,
	-- but Firaxis keeps the per-player completion count in `PlayerStats`.
	--
	-- This is the exact API used by the shipped World Rankings science screen:
	-- `GetNumProjectsAdvanced(project.Index) > 0` means the milestone is done.
	-- Export only strategic one-time programs. District conversion projects are
	-- repeatable, so treating their nonzero count as a milestone would invent
	-- science-victory progress and skew the planner's project count.
	local scienceProjects = {};
	local playerStats = try(function() return player:GetStats(); end, nil);
	local function projectAdvanced(projectType)
		local project = try(function() return GameInfo.Projects[projectType]; end, nil);
		return playerStats ~= nil and project ~= nil and (try(function()
			return playerStats:GetNumProjectsAdvanced(project.Index);
		end, 0) or 0) > 0;
	end
	for _, projectType in ipairs({
		"PROJECT_MANHATTAN_PROJECT",
		"PROJECT_OPERATION_IVY",
		"PROJECT_LAUNCH_EARTH_SATELLITE",
		"PROJECT_LAUNCH_MOON_LANDING",
		-- Base Civ VI has three Mars parts; Gathering Storm replaces them with
		-- `PROJECT_LAUNCH_MARS_BASE`. The Rust mirror recognizes both shapes.
		"PROJECT_LAUNCH_MARS_REACTOR",
		"PROJECT_LAUNCH_MARS_HABITATION",
		"PROJECT_LAUNCH_MARS_HYDROPONICS",
		"PROJECT_LAUNCH_MARS_BASE",
		"PROJECT_LAUNCH_EXOPLANET_EXPEDITION",
	}) do
		if projectAdvanced(projectType) then
			scienceProjects[#scienceProjects + 1] = projectType;
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
	-- ★★★★★ FREE CITIES ARE HOSTILE AND WERE IN NO LIST AT ALL.
	--
	-- `hostiles` walked ONLY `GetAliveBarbarianIDs()`. Majors arrive in `rivals`,
	-- city-states in `minors` — and the Free Cities player is in NONE of the three.
	-- Measured on run civvis-20260802T064240Z: every hostile entry in the whole run,
	-- all 1454 of them, carried `player: 63`. Player 62 never appeared.
	--
	-- That is not a cosmetic gap. It is how the empire died:
	--   t129  five cities
	--   t131-t145  four of them lost, every one naming `falls_to: 62`
	--   t130  every rival still `at_war: false`; hostiles showed SEVEN units,
	--         two of them settlers and one a builder
	--   t125  "Researching education | worth 19, ahead of military tactics at 5"
	-- Mecca flipped to Free Cities at loyalty 1.6, and the Free City's units then
	-- took Medina, Sana'a and Hattin — all three still at loyalty 99-100, so they
	-- were CONQUERED, not disloyal. CIVVIS could not see the army that took them,
	-- so it kept valuing military tactics at 5 while its empire was dismantled.
	--
	-- ⚠ `IsFreeCities` is guarded exactly as the shipped UI guards it —
	-- GlobalResourcePopup.lua writes `if pPlayer.IsFreeCities and
	-- pPlayer:IsFreeCities()` with the comment "Not avail in base game". A build
	-- without the method must fall through, not error.
	local function addUnitsOf(bid)
		local other = Players[bid];
		if other == nil then return; end
		pcall(function()
			for _, unit in other:GetUnits():Members() do
				local ux, uy = unit:GetX(), unit:GetY();
				if plotRevealed(pid, ux, uy) then
					hostiles[#hostiles + 1] = {
						-- The host's own unit id, so a combat event and a
						-- next-frame sighting name the same unit. See CivvisLedger.
						id = try(function() return unit:GetID(); end, nil),
						x = ux, y = uy, player = bid,
						type = try(function()
							return GameInfo.Units[unit:GetUnitType()].UnitType;
						end, "?"),
					};
				end
			end
		end);
	end
	pcall(function()
		local everyone = try(function() return PlayerManager.GetAliveIDs(); end, {}) or {};
		for _, oid in ipairs(everyone) do
			local other = Players[oid];
			local free = other ~= nil and try(function()
				return other.IsFreeCities ~= nil and other:IsFreeCities() == true;
			end, false);
			if free == true then addUnitsOf(oid); end
		end
	end);
	pcall(function()
		local ids = try(function() return PlayerManager.GetAliveBarbarianIDs(); end, {}) or {};
		for _, bid in ipairs(ids) do
			local barb = Players[bid];
			if barb ~= nil then
				pcall(function()
					for _, unit in barb:GetUnits():Members() do
						local ux, uy = unit:GetX(), unit:GetY();
						if PlayersVisibility[pid]:IsVisible(ux, uy) then
							local name = try(function()
								return GameInfo.Units[unit:GetUnitType()].UnitType;
							end, "?");
							local row = GameInfo.Units[name];
							local progress = unitProgress(unit);
							hostiles[#hostiles + 1] = {
								id = try(function() return unit:GetID(); end, nil),
								x = ux, y = uy, player = bid,
								type = name,
								hp = 100 - (try(function() return unit:GetDamage(); end, 0) or 0),
								moves = try(function() return unit:GetMovesRemaining(); end, -1),
								xp = progress.xp, level = progress.level,
								promotions = progress.promotions,
								build_charges = progress.build_charges,
								spread_charges = progress.spread_charges,
								religion = progress.religion,
								combat = row ~= nil and (row.Combat or 0) or 0,
								ranged = row ~= nil and (row.RangedCombat or 0) or 0,
								fortify_turns = try(function() return unit:GetFortifyTurns(); end, 0),
								fortified = try(function()
									return (unit:GetFortifyTurns() or 0) > 0;
								end, false),
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
	-- ★★★ THE GOVERNMENTS THIS SEAT HAS ALREADY USED, current one included.
	-- Returning to one costs Anarchy; CIVVIS's engine charges that too — but
	-- only through a history a board rebuilt fresh each turn never carries, so
	-- the planner priced return switches as FREE and re-proposed them (deck
	-- and all) every turn against the brain guard's standing veto: 127 blocks
	-- in run civvis-20260815T012010Z. Derived STATELESSLY from the engine's
	-- own answer (GovernmentScreen.lua:886: a switch that would cost Anarchy
	-- turns is a return switch), so it survives reloads and needs no session
	-- table against the main chunk's register ceiling.
	--
	-- A plain ARRAY: empty is ordinary before the first government and encodes
	-- as `[]`, which the Rust Vec accepts — the OPPOSITE of the great-person
	-- maps, which must return nil. See those comments before changing this.
	local used_governments = try(function()
		local culture = player:GetCulture();
		if culture == nil then return nil; end
		local current = try(function() return culture:GetCurrentGovernment(); end, -1);
		local out = {};
		for row in GameInfo.Governments() do
			local anarchy = try(function()
				return culture:GetAnarchyTurns(row.Index);
			end, 0) or 0;
			if anarchy > 0 or (current ~= nil and current >= 0
					and row.Index == current) then
				out[#out + 1] = row.GovernmentType;
			end
		end
		return out;
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
	-- A Great Prophet is not a generic Great Person activation. Founding a
	-- religion is a player operation whose belief choices CIVVIS must make, so
	-- export both the decision gate and the worldwide availability facts.
	local playerReligion = try(function() return player:GetReligion(); end);
	local religionCreated = playerReligion ~= nil and
		try(function() return playerReligion:GetReligionTypeCreated(); end, -1) or -1;
	local prophet_pending = religionCreated < 0 and playerReligion ~= nil and
		try(function() return playerReligion:HasReligiousFoundingUnit(); end, false) or false;
	-- ★ SAY SO WHEN THE FOUNDING DID NOT TAKE. The request reports `applied`
	-- because nothing threw; only the turn AFTER can read whether a religion
	-- exists. Across 24 live runs the answer was always "no religion, and the
	-- Prophet is gone too", and nothing in the log said so.
	if pendingReligionFounding ~= nil then
		local now = try(function() return Game.GetCurrentGameTurn(); end, 0) or 0;
		if religionCreated >= 0 then
			emit("religion_founded", {
				player = pid,
				turn = now,
				requested_turn = pendingReligionFounding.turn,
				religion = pendingReligionFounding.religion,
				follower = pendingReligionFounding.follower,
				founder = pendingReligionFounding.founder,
			});
			pendingReligionFounding = nil;
		elseif now > pendingReligionFounding.turn then
			emit("religion_founding_failed", {
				player = pid,
				turn = now,
				requested_turn = pendingReligionFounding.turn,
				religion = pendingReligionFounding.religion,
				-- The two facts that separate the failure modes: whether the
				-- Prophet survived, and whether the slot is still open.
				founding_unit_left = prophet_pending,
				religions_founded = #(try(function()
					return Game.GetReligion():GetReligions(); end, {}) or {}),
			});
			pendingReligionFounding = nil;
		end
	end
	local founded_religion = nil;
	local founded_religions = {};
	local religion_beliefs = {};
	local taken_religion_beliefs = {};
	-- ★★★★ EACH RELIGION WITH ITS OWN BELIEFS AND FOUNDER. `taken_religion_beliefs`
	-- is the union and says only what is no longer available; it cannot say
	-- WHICH religion holds Divine Inspiration. A city following Catholicism gets
	-- Catholicism's follower beliefs whoever founded it, and the mirror had that
	-- belief parked on whichever seat happened to be zipped with the religion —
	-- Rome's three Wonders paid 12 Faith under Divine Inspiration for the whole
	-- of run civvis-20260816T123936Z while the model saw none of it. Same source
	-- as the Religion screen: `Game.GetReligion():GetReligions()`, each entry
	-- {Religion, Founder, Beliefs}. Empty list when nothing is founded.
	local religions = {};
	local allReligions = try(function() return Game.GetReligion():GetReligions(); end, {}) or {};
	for _, religion in ipairs(allReligions) do
		local religionRow = GameInfo.Religions[religion.Religion];
		local isPantheon = religionRow ~= nil and
			(religionRow.Pantheon == true or religionRow.Pantheon == 1);
		if religionRow ~= nil and not isPantheon then
			founded_religions[#founded_religions + 1] = religionRow.ReligionType;
			if religion.Founder == pid then
				founded_religion = religionRow.ReligionType;
			end
			local own = {};
			for _, beliefIndex in ipairs(religion.Beliefs or {}) do
				local beliefRow = GameInfo.Beliefs[beliefIndex];
				if beliefRow ~= nil then
					taken_religion_beliefs[#taken_religion_beliefs + 1] = beliefRow.BeliefType;
					own[#own + 1] = beliefRow.BeliefType;
					if religion.Founder == pid then
						religion_beliefs[#religion_beliefs + 1] = beliefRow.BeliefType;
					end
				end
			end
			table.sort(own);
			religions[#religions + 1] = {
				type = religionRow.ReligionType,
				founder = religion.Founder,
				beliefs = own,
			};
		end
	end
	table.sort(founded_religions);
	table.sort(religion_beliefs);
	table.sort(taken_religion_beliefs);
	table.sort(religions, function(a, b) return a.type < b.type; end);
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

	-- Governor Titles, appointments, promotions and assignments are authoritative
	-- host state. Completed Civics cannot reconstruct them: districts also grant
	-- titles, a title may be unspent, and appointments are player choices. Omitting
	-- this roster made CIVVIS appoint Victor and Magnus again on every replayed frame.
	local governor_points, governor_points_spent, governor_roster = nil, nil, nil;
	local governors = try(function() return player:GetGovernors(); end);
	if governors ~= nil then
		governor_points = try(function() return governors:GetGovernorPoints(); end);
		governor_points_spent = try(function() return governors:GetGovernorPointsSpent(); end);
		-- `GetGovernorList` returns two values. The generic `try` helper deliberately
		-- retains only one, so preserve the shipped API's full return tuple here.
		local okList, hasGovernors, appointed = pcall(function()
			return governors:GetGovernorList();
		end);
		if okList and type(appointed) == "table" then
			governor_roster = {};
			for _, governor in ipairs(appointed) do
				pcall(function()
					local definition = GameInfo.Governors[governor:GetType()];
					if definition == nil then return; end
					local promotions = {};
					for promotionSet in GameInfo.GovernorPromotionSets() do
						if promotionSet.GovernorType == definition.GovernorType
								and not (promotionSet.BaseAbility == true
									or promotionSet.BaseAbility == 1) then
							local promotion = GameInfo.GovernorPromotions[
								promotionSet.GovernorPromotion];
							if promotion ~= nil and try(function()
								return governor:HasPromotion(promotion.Hash);
							end, false) then
								promotions[#promotions + 1] = promotion.GovernorPromotionType;
							end
						end
					end
					table.sort(promotions);
					local city = try(function() return governor:GetAssignedCity(); end);
					governor_roster[#governor_roster + 1] = {
						type = definition.GovernorType,
						city = city ~= nil and try(function() return city:GetID(); end, -1) or -1,
						city_player = city ~= nil and try(function() return city:GetOwner(); end, -1) or -1,
						x = city ~= nil and try(function() return city:GetX(); end, -1) or -1,
						y = city ~= nil and try(function() return city:GetY(); end, -1) or -1,
						established = try(function() return governor:IsEstablished(); end, false),
						turns_on_site = try(function() return governor:GetTurnsOnSite(); end, 0),
						turns_to_establish = try(function()
							return governor:GetTurnsToEstablish();
						end, 0),
						neutralized_turns = try(function()
							return governor:GetNeutralizedTurns();
						end, 0),
						promotions = promotions,
					};
				end);
			end
			table.sort(governor_roster, function(a, b)
				return tostring(a.type) < tostring(b.type);
			end);
		end
	end
	emit("state", {
		turn = turn,
		-- 0 for the turn's opening board; N for the Nth mid-turn combat frame
		-- (see CivvisFrames). The brain re-plans the same turn on a frame.
		frame = frame or 0,
		techs = techs,
		-- Completed one-time nuclear and space milestones. This stays separate
		-- from the city's current production because completion is player-wide.
		science_projects = scienceProjects,
		-- ⚠ Boosts on technologies and civics the empire does NOT yet hold. A
		-- triggered boost on something already researched is spent and says
		-- nothing about what to take next.
		--
		-- ⚠ An empty table is CORRECT here and must stay a plain table: `encode`
		-- emits `[]` whenever `#v == n`, which `{}` satisfies, and the Rust side
		-- is a `Vec<String>` — so `[]` is exactly what serde wants. That is the
		-- opposite of the Great-Person-points field below, which is a MAP and
		-- must return `nil` rather than `{}` precisely because `{}` would go out
		-- as `[]` and take the whole StateSnapshot down with it.
		boosted_techs = boosted_techs,
		boosted_civics = boosted_civics,
		civics = civics,
		research = research,
		research_progress = research_progress,
		civic = civic,
		civic_progress = civic_progress,
		government = government,
		used_governments = used_governments,
		pantheon = pantheon,
		founded_religion = founded_religion,
		founded_religions = founded_religions,
		religion_beliefs = religion_beliefs,
		taken_religion_beliefs = taken_religion_beliefs,
		religions = religions,
		prophet_pending = prophet_pending,
		policies = policies,
		policy_slots = policy_slots,
		hostiles = hostiles,
		gold = try(function() return math.floor(player:GetTreasury():GetGoldBalance()); end, -1),
		-- ★★★★★ NET INCOME, AND WHY THE EMPIRE GOES BANKRUPT WITHOUT NOTICING.
		--
		-- `gold_per_turn` is 0 in EVERY live decision. `mirror_net_income`
		-- derives the rate from the treasury delta between CONSECUTIVE turns and
		-- keeps `last_treasury` on the `LiveMirror`, but the bridge runs
		-- `civvis_orders --serve --fresh-board`, which rebuilds that mirror every
		-- turn -- so the predecessor is never there and the rate never lands.
		-- Measured previously at 0.00 in 963 of 963 calls.
		--
		-- The cost is not academic. Live run `civvis-20260810T191050Z`
		-- (Rome/Trajan, Settler): treasury peaked at 319 on turn 60, reached
		-- **0 on turn 110 and stayed there for the remaining 75 turns**. With the
		-- bankruptcy guard blind, the empire kept units it could not pay for,
		-- Civilization VI disbanded them (`army` 12 -> 0), and the cities fell at
		-- t173, t180 and t184: **six cities became two**, final score 403 against
		-- Mongolia's 747. Tech and civics were COMPETITIVE all game (44 vs 46,
		-- 35 vs 34) -- this is the whole gap.
		--
		-- So stop deriving it. `GetGoldYield() - GetTotalMaintenance()` is the
		-- exact figure the shipped TopPanel prints beside the treasury
		-- (Expansion2/UI/Replacements/TopPanel.lua:140), it needs no history, and
		-- it survives a board rebuilt from scratch every turn.
		--
		-- ⚠ `nil` when the host does not answer, NOT 0: a real 0 is break-even
		-- and a missing answer is not, and conflating them is exactly the failure
		-- above.
		gold_per_turn = try(function()
			local treasury = player:GetTreasury();
			return treasury:GetGoldYield() - treasury:GetTotalMaintenance();
		end, nil),
		faith = try(function() return math.floor(player:GetReligion():GetFaithBalance()); end, -1),
		-- ★★★★ FAITH PER TURN, THE TOP BAR'S OWN FIGURE. `GetFaithYield()` is
		-- what TopPanel prints beside the Faith balance, and it is NOT the sum
		-- of the cities: Firaxis pays every Great Person point of a class the
		-- empire can no longer earn out again as Faith
		-- (`GetFaithFromUnusedGreatPeoplePoints` in the game core), and adds
		-- founder-belief, envoy and suzerain income at the player level. On
		-- run civvis-20260816T123936Z the balance grew 100–113 a turn from
		-- t231 while every city together made 49; without this field the
		-- mirror had no host figure to be corrected to. Same nil-not-0 rule as
		-- `gold_per_turn`: a missing answer is not break-even.
		faith_per_turn = try(function() return player:GetReligion():GetFaithYield(); end, nil),
		-- The host's own Faith ledger — "+N from Cities / Beliefs / Envoys /
		-- city-states you are Suzerain of / Other" — compacted the same way as
		-- the per-city `yield_sources`, so a gap between the two games is named
		-- rather than guessed at.
		faith_sources = try(function()
			local text = player:GetReligion():GetFaithYieldToolTip();
			if text == nil then return nil; end
			text = tostring(text):gsub("%[ICON_[%w_]+%]", "");
			text = text:gsub("%[NEWLINE%]", "\n"):gsub("[ \t]+", " ");
			return (text:gsub("^%s+", ""):gsub("%s+$", "")):sub(1, 400);
		end, nil),
		science = try(function() return player:GetTechs():GetScienceYield(); end, -1),
		culture = try(function() return player:GetCulture():GetCultureYield(); end, -1),
		public_stats = publicStats,
		score = try(function() return player:GetScore(); end, -1),
		-- Diplomatic-victory points and the Favor that buys congress votes; see
		-- `voteWorldCongress`.
		dvp = try(function() return player:GetStats():GetDiplomaticVictoryPoints(); end, nil),
		favor = try(function() return player:GetFavor(); end, nil),
		-- The World Congress standing as of the last session, recorded where
		-- the review is read. Every alive major appears, including ones this
		-- seat has not met -- the congress seats them all and shows the seat
		-- their points, and the ballot the host hands us for
		-- `WC_RES_DIPLOVICTORY` already names them as targets, which is why
		-- `voteWorldCongress` may pick a leader from the same set. Carrying it
		-- no further than the ballot is what left the victory tracker blind.
		congress_dvp = envoyTally.congress_dvp,
		-- How many Spies this empire may field, from the same accessor the
		-- shipped Espionage Overview prints. The mirror blocks Spy production
		-- outright without it, which is why the seat has never held one.
		spy_capacity = try(function()
			return player:GetDiplomacy():GetSpyCapacity();
		end, nil),
		-- Our own two culture-victory counters, same accessors as each
		-- rival's (WorldRankings.lua:1674-1675): OUR staycationers are the
		-- bar every rival's visiting tourists must clear, so the victory
		-- tracker's culture lane needs the pair on both sides of the fog.
		foreign_tourists = try(function()
			return player:GetCulture():GetTouristsTo();
		end, -1),
		domestic_tourists = try(function()
			return player:GetCulture():GetStaycationers();
		end, -1),
		-- Ours, on the same scale as each rival's, so a comparison is possible at all.
		military = try(function() return player:GetStats():GetMilitaryStrength(); end, -1),
		-- ★★★★★ THE AGE, WHICH THE BRIDGE HAS NEVER CARRIED.
		--
		-- CIVVIS models Gathering Storm's age system in full (`docs/AGES.md`):
		-- `Player::era_score`, `era_score_baseline`, `normal_age_threshold`,
		-- `golden_age_threshold`, `dedications`. None of it crossed, so on every
		-- reconstructed live board era score was 0 against the *defaults* left by
		-- `Player::default` -- a civilization permanently reading as headed for a
		-- Dark Age it might not be in, or out of one it is.
		--
		-- Two decisions run off exactly these fields and were therefore taken
		-- against fiction in every live game:
		--   * `ai::choose_dedications` picks a Dedication from
		--     `available_dedications`, which is gated on `dedication_choices`;
		--     live that is 0, so a Dedication was NEVER chosen.
		--   * `advanced.rs` filters `rules.policies[card].dark_age`, so a real
		--     Dark Age's wildcard cards were never slotted -- the same shape as
		--     the housing and loyalty cards that are never slotted.
		--
		-- Every getter below appears in the shipped Expansion2 `EraProgressPanel`
		-- on this install, so the names are read off the build rather than
		-- guessed. All are SCALARS: no empty-table hazard (see the
		-- `great_person_points` note above for why that matters).
		--
		-- `normal_age_threshold` is CIVVIS's name for the score at or above which
		-- an age is Normal rather than Dark, which is precisely Civ 6's *Dark Age
		-- threshold* -- the two names describe the same boundary from opposite
		-- sides.
		era_score = try(function()
			return Game.GetEras():GetPlayerCurrentScore(pid);
		end, -1),
		era_score_baseline = try(function()
			return Game.GetEras():GetPlayerThresholdBaseline(pid);
		end, -1),
		normal_age_threshold = try(function()
			return Game.GetEras():GetPlayerDarkAgeThreshold(pid);
		end, -1),
		golden_age_threshold = try(function()
			return Game.GetEras():GetPlayerGoldenAgeThreshold(pid);
		end, -1),
		world_era = try(function() return Game.GetEras():GetCurrentEra(); end, -1),
		dark_age = try(function() return Game.GetEras():HasDarkAge(pid); end, nil),
		golden_age = try(function() return Game.GetEras():HasGoldenAge(pid); end, nil),
		heroic_golden_age = try(function()
			return Game.GetEras():HasHeroicGoldenAge(pid);
		end, nil),
		-- ★★★★ WHICH DEDICATIONS ARE ACTIVE. The age flags above say whether a
		-- Golden Age is on; this says what it PAYS. `GetPlayerActiveCommemorations`
		-- is what the shipped EraProgressPanel lists, and every yield the mirror
		-- models for a Dedication (`Game::dedication_active`) was inert without it:
		-- Heartbeat of Steam's Campus Production ("+10 from Campus" in the host's
		-- own production ledger, run civvis-20260816T132247Z) is the whole
		-- production gap of that game's Golden Age. Type names, so the mirror
		-- maps them onto its own dedication ids without guessing an index.
		dedications = try(function()
			local names = {};
			for _, active in ipairs(Game.GetEras():GetPlayerActiveCommemorations(pid) or {}) do
				local row = GameInfo.CommemorationTypes[active];
				names[#names + 1] = row and row.CommemorationType or tostring(active);
			end
			return names;
		end, nil),
		-- ★★★★ THE WORLD CONGRESS RESOLUTIONS IN EFFECT, every turn. The
		-- `wc_outcome` event says what the last session decided; this says what
		-- is binding NOW, in the shape the mirror maps onto its own
		-- `active_congress_effects` (`Game::congress_effect_active`). Run
		-- civvis-20260816T200454Z: Trade Policy A on us for t82-101 paid Cumae
		-- "+4 from Incoming Trade Routes" per foreign route and +1 route
		-- capacity, and the model, with no Congress on the board, could not
		-- explain either. `GetResolutions(pid)` is what the shipped
		-- CityPanel_Expansion2 reads between sessions (it checks Border Control
		-- B against `ChosenOption`/`ChosenThing`); the same call returns the
		-- ballot DURING a session, whose entries have no `ChosenOption` yet, so
		-- only entries with a chosen option are in effect. `option` is 1 (A) /
		-- 2 (B) by matching the chosen LOC key against the resolution's two
		-- effect descriptions; `target` is `ChosenThing` verbatim (a player id
		-- for PLAYER-targeted resolutions, a type name otherwise).
		resolutions = try(function()
			local out = {};
			local wc = Game.GetWorldCongress();
			if wc == nil then return out; end
			for i, r in pairs(wc:GetResolutions(pid) or {}) do
				if type(i) == "number" and type(r) == "table" and r.Type ~= nil
					and type(r.ChosenOption) == "string" and r.ChosenOption ~= "" then
					local info = GameInfo.Resolutions[r.Type];
					local option = 0;
					if info ~= nil then
						if r.ChosenOption == info.Effect1Description then option = 1;
						elseif r.ChosenOption == info.Effect2Description then option = 2; end
					end
					out[#out + 1] = {
						type = tostring(info and info.ResolutionType or r.Type),
						option = option,
						target = tostring(r.ChosenThing or ""),
					};
				end
			end
			return out;
		end, nil),
		-- Turns until the next regular session, i.e. how long the resolutions
		-- above stay binding (`GetMeetingStatus().TurnsLeft`, the number the
		-- shipped congress button counts down).
		congress_turns_left = try(function()
			local status = Game.GetWorldCongress():GetMeetingStatus();
			return status and tonumber(status.TurnsLeft) or nil;
		end, nil),
		-- Great Person POINTS, not the Great People already earned. The planner
		-- prices every district project against the live race -- how close we
		-- are to the next Scientist, and how close the leading rival is -- and
		-- with this absent that whole comparison ran against all zeros in every
		-- live game.
		--
		-- ⚠ `GetPointsTotal` takes `row.Index`, NOT `row.Hash`. Passing a hash
		-- here is what crashed the game four times, once per attempt, and a
		-- pcall cannot catch that: it is a segfault inside the host, not a Lua
		-- error. Base/Assets/UI/.../GreatPeoplePopup.lua:2098 is the reference.
		-- ★★★★ THE STOCKPILES, WHICH THE BOARD HAS NEVER CARRIED. Without them
		-- CIVVIS's `strategic_stockpile` reads 0 for every resource, so no unit
		-- that costs Iron/Horses/Niter/Coal/Oil is ever producible on the live
		-- seat (the armies were AT crews, pike-and-shot and chariots) and a
		-- unit is never obsolete for want of a buildable successor: the WON game
		-- civvis-20260816T054344Z ordered a Trebuchet the host called
		-- `civvis_build_unplayable` on 29 turns across eight cities. Keyed by the
		-- host's resource type; the Rust side translates. Only the strategic
		-- class — the same filter the shipped TopPanel_Expansion2 uses.
		-- ⚠ nil, not `{}`, when nothing is stocked: see `great_person_points`.
		strategic_resources = try(function()
			local resources = player:GetResources();
			if resources == nil then return nil; end
			local out = {};
			local any = false;
			for row in GameInfo.Resources() do
				if row.ResourceClassType == "RESOURCECLASS_STRATEGIC" then
					local amount = try(function()
						return resources:GetResourceAmount(row.ResourceType);
					end, nil);
					if amount ~= nil and amount > 0 then
						out[row.ResourceType] = amount;
						any = true;
					end
				end
			end
			if not any then return nil; end
			return out;
		end, nil),
		great_person_points = try(function()
			local points = player:GetGreatPeoplePoints();
			if points == nil then return nil; end
			local out = {};
			local any = false;
			for row in GameInfo.GreatPersonClasses() do
				local total = points:GetPointsTotal(row.Index);
				if total ~= nil and total > 0 then
					out[row.GreatPersonClassType] = total;
					any = true;
				end
			end
			-- ⚠⚠ RETURN nil, NOT AN EMPTY TABLE. `encode` above counts entries and
			-- emits `[]` for any table where `#v == n`, which an empty table
			-- satisfies (0 == 0). So `{}` goes out as `[]`, the Rust field is a
			-- map, serde refuses a sequence, and **the whole StateSnapshot fails
			-- to deserialize** — not just this field.
			--
			-- That is not hypothetical. It took every live game down: three
			-- consecutive attempts on d0fdcfb reported "no revealed terrain or no
			-- state yet" and 0 orders from turn 1, stalled at turn 6 with the
			-- research prompt unanswered, and were killed by the watchdog. Every
			-- player has zero Great Person points on turn 1, so this fired in
			-- every game immediately.
			if not any then return nil; end
			return out;
		end, nil),
		-- The same classes' points PER TURN, the host's own rate: what the
		-- Great People screen prints under each class. It is the figure the
		-- Faith above is paid from once a class runs out of people, so the
		-- model's own derivation can be checked against it. Same `Index`
		-- rule and the same nil-not-`{}` rule as `great_person_points`.
		great_person_points_per_turn = try(function()
			local points = player:GetGreatPeoplePoints();
			if points == nil then return nil; end
			local out = {};
			local any = false;
			for row in GameInfo.GreatPersonClasses() do
				local rate = points:GetPointsPerTurn(row.Index);
				if rate ~= nil and rate > 0 then
					out[row.GreatPersonClassType] = rate;
					any = true;
				end
			end
			if not any then return nil; end
			return out;
		end, nil),
		-- ★★★★ THE CLASSES WITH NOBODY LEFT TO RECRUIT. Civilization VI pays
		-- every Great Person point of such a class out again as Faith, one for
		-- one (`GetFaithFromUnusedGreatPeoplePoints` in the game core; the
		-- top bar's "from Other"): the last Great Scientist anywhere claimed,
		-- and the Campus keeps paying, in Faith. Run civvis-20260816T123936Z
		-- banked 100–113 Faith a turn from t231 against 49 from every city.
		-- A class is exhausted when the timeline has no unclaimed entry for
		-- it. `great_person_costs` below already implies this (a class with
		-- points and no cost) — except on the turn EVERY class is gone, when
		-- that map is nil and cannot be told from an older export. So say it
		-- outright, and as a LIST: an empty list encodes as `[]`, which is a
		-- real answer here ("everyone still available"), unlike the maps.
		great_person_exhausted = try(function()
			local greatPeople = Game.GetGreatPeople();
			if greatPeople == nil then return nil; end
			local timeline = greatPeople:GetTimeline();
			if timeline == nil then return nil; end
			local available = {};
			for _, entry in ipairs(timeline) do
				if entry.Individual ~= nil and entry.Claimant == nil then
					local info = GameInfo.GreatPersonIndividuals[entry.Individual];
					if info ~= nil and info.GreatPersonClassType ~= nil then
						available[info.GreatPersonClassType] = true;
					end
				end
			end
			local out = {};
			for row in GameInfo.GreatPersonClasses() do
				if not available[row.GreatPersonClassType] then
					out[#out + 1] = row.GreatPersonClassType;
				end
			end
			table.sort(out);
			return out;
		end, nil),
		-- The live RECRUIT COST of each class's current unclaimed Great
		-- Person, from the same timeline the recruit order is judged by.
		-- Points without costs made the planner price the claim against
		-- CIVVIS's own market formula: 45 gp_cannot_recruit refusals in run
		-- civvis-20260815T033823Z were the recruit order crossing the bridge
		-- only to be asked years early. Min cost per class = the current
		-- individual; the timeline also lists later eras at higher prices.
		--
		-- ⚠⚠ Same serde trap as great_person_points above: RETURN nil, NEVER
		-- an empty table — `{}` encodes as `[]` and kills the whole snapshot.
		great_person_costs = try(function()
			local greatPeople = Game.GetGreatPeople();
			if greatPeople == nil then return nil; end
			local timeline = greatPeople:GetTimeline();
			if timeline == nil then return nil; end
			local out = {};
			local any = false;
			for _, entry in ipairs(timeline) do
				if entry.Individual ~= nil and entry.Claimant == nil
						and entry.Cost ~= nil then
					local info = GameInfo.GreatPersonIndividuals[entry.Individual];
					local class = info ~= nil and info.GreatPersonClassType or nil;
					if class ~= nil then
						local prior = out[class];
						if prior == nil or entry.Cost < prior then
							out[class] = entry.Cost;
							any = true;
						end
					end
				end
			end
			if not any then return nil; end
			return out;
		end, nil),
		-- The class and cost alone still leave a fatal ambiguity: a current
		-- Great Scientist can be Hildegard of Bingen, who requires a Holy Site,
		-- or Mary Leakey, who requires a Theater, while CIVVIS's generic
		-- Scientist model sees only a Campus. Export the exact named offer and
		-- Firaxis's hard completed-district prerequisite so the planner does
		-- not spend a project race on a person this empire cannot activate.
		--
		-- Keep the same min-cost/current-offer selection as `great_person_costs`
		-- directly above. The timeline includes later people in each class; its
		-- lowest-cost unclaimed entry is the one the Recruit operation would
		-- judge.
		-- An empty table encodes as `[]`; Rust deliberately accepts that shape
		-- for this field so it can distinguish an authoritative empty Great
		-- People screen from an older control mod that omitted the field. That
		-- distinction prevents the local fallback roster from claiming a class
		-- after Firaxis has exhausted every offer.
		great_person_offers = try(function()
			local greatPeople = Game.GetGreatPeople();
			if greatPeople == nil then return nil; end
			local timeline = greatPeople:GetTimeline();
			if timeline == nil then return nil; end
			local out, costs = {}, {};
			local any = false;
			for _, entry in ipairs(timeline) do
				if entry.Individual ~= nil and entry.Claimant == nil
						and entry.Cost ~= nil then
					local info = GameInfo.GreatPersonIndividuals[entry.Individual];
					local class = info ~= nil and info.GreatPersonClassType or nil;
					if class ~= nil then
						local prior = costs[class];
						if prior == nil or entry.Cost < prior then
							costs[class] = entry.Cost;
							out[class] = {
								individual = info.GreatPersonIndividualType,
								required_district = info.ActionRequiresCompletedDistrictType,
							};
							any = true;
						end
					end
				end
			end
			if not any then return {}; end
			return out;
		end, nil),
		governor_points = governor_points,
		governor_points_spent = governor_points_spent,
		governors = governor_roster,
		trade_capacity = try(function()
			return player:GetTrade():GetOutgoingRouteCapacity();
		end, -1),
		cities = cities,
		units = units,
		trade_routes = tradeRoutes,
		rivals = rivals,
		minors = minors,
		-- ★★★★★ THE ENVOYS WE ARE HOLDING BUT NEVER SPEND.
		--
		-- `minors[].envoys` says where our envoys have LANDED. Nothing has ever
		-- said how many are sitting UNSPENT, and that omission decides the whole
		-- axis: `Game::legal_actions` gates `Action::SendEnvoy` behind
		-- `if p.envoys_free > 0`, `envoys_free` is never mirrored, so on every
		-- reconstructed live board it is 0 and **CIVVIS never even enumerates
		-- sending an envoy**. That is why `SendEnvoy` appears nowhere in the
		-- skipped-action tally while `LevyMilitary` (which needs a suzerainty we
		-- never have) appears 44 times.
		--
		-- Measured over 36 live runs past turn 150, from Civ 6's own export:
		-- median envoys placed **1**, median suzerainties **0**, and 16 of 36
		-- runs end holding zero envoys anywhere. A rival was sitting on 10 at a
		-- single city-state.
		--
		-- CIVVIS models the payoff in full — `envoy_type_yields_for_count` gives a
		-- cultural city-state culture at the 1/3/6 thresholds — so the sim
		-- collects this and the live game collects none of it. Suzerainty also
		-- hands over the city-state's luxuries, which is amenities, which is the
		-- 0.70-0.80 multiplier on every yield.
		--
		-- ⚠ EXPORT ONLY. This deliberately does NOT reach `player.envoys_free` on
		-- the reconstructed board yet, because the actuation path is a KNOWN GAME
		-- CRASHER: `ENDTURN_BLOCKING_GIVE_INFLUENCE_TOKEN` answered by
		-- `chooseEnvoy` produced three SIGSEGVs in three runs on a seed that
		-- reached t92/t106 without it, and `cfg.EnvoyEnabled` is off because of
		-- it. Making CIVVIS want something the bridge cannot safely deliver would
		-- trade a silent loss for a crashed run. First measure the size of the
		-- prize; then isolate the crash.
		--
		-- `GetTokensToGive` is the same call `chooseEnvoy` already makes and
		-- appears six times in the shipped UI Lua.
		envoys_free = try(function()
			return player:GetInfluence():GetTokensToGive();
		end, -1),
		-- ★★★★ THE EMERGENCIES AND SCORED COMPETITIONS, WHICH DECIDE THE
		-- DIPLOMATIC RACE AS MUCH AS THE VOTES DO. `wc_outcome` on
		-- civvis-20260816T205104Z: the leader's +8 in one session (12→20,
		-- victory at t202) was +2 from the victory resolution and the rest from
		-- WORLD_FAIR (passed t162) and WORLD_GAMES (passed t182) resolving —
		-- competitions the seat, the production and gold leader of that game,
		-- never entered. Nothing said they were running. This is Firaxis's own
		-- crisis table (`GetEmergencyInfoTable`, what the World Crisis popup
		-- and tracker read): per emergency its type, target, turns left (<0 =
		-- completed), whether it has begun/succeeded, every member's score and
		-- tier, our own standing, the goals with their completion, and how
		-- score is earned. Read-only; membership and the aid gold are the
		-- agent's or CIVVIS's decision, made elsewhere.
		emergencies = try(function()
			local out = {};
			local crises = Game.GetEmergencyManager():GetEmergencyInfoTable(pid);
			for _, crisis in ipairs(crises or {}) do
				local members, scores = {}, {};
				local ourScore, ourTier, member = nil, nil, false;
				for _, id in ipairs(crisis.MemberIDs or {}) do
					local mid = tonumber(id) or -1;
					members[#members + 1] = mid;
					local score = tonumber((crisis.ScoresTables or {})[mid]) or 0;
					local tier = tonumber((crisis.MemberTiers or {})[mid]);
					scores[#scores + 1] = { player = mid, score = score, tier = tier };
					if mid == pid then ourScore, ourTier, member = score, tier, true; end
				end
				local goals = {};
				local goalTable = (tonumber(crisis.TargetID) == pid)
					and crisis.TargetGoalsTable or crisis.GoalsTable;
				for _, goal in ipairs(goalTable or {}) do
					if type(goal) == "table" and goal.Name ~= nil and goal.Name ~= "" then
						goals[#goals + 1] = { name = tostring(goal.Name), done = goal.Completed == true };
					end
				end
				local sources = {};
				for _, line in ipairs(crisis.ScoreSourcesTable or {}) do
					sources[#sources + 1] = tostring(line);
				end
				out[#out + 1] = {
					type = tostring(crisis.EmergencyType),
					name = tostring(crisis.NameText or ""),
					target = tonumber(crisis.TargetID) or -1,
					target_city = tostring(crisis.TargetCityName or ""),
					turns_left = tonumber(crisis.TurnsLeft) or -1,
					begun = crisis.HasBegun == true,
					success = crisis.bSuccess == true,
					members = members, scores = scores,
					ours = { member = member, target = tonumber(crisis.TargetID) == pid,
					         score = ourScore, tier = ourTier },
					goals = goals, score_sources = sources,
				};
			end
			return out;
		end, nil),
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

-- ★★★★★ THE RIVERS ON THE CIVVIS BOARD WERE INVENTED, NOT MIRRORED.
--
-- Nothing here ever exported a river and nothing on the Rust side ever wrote one
-- (`grep -c river src/mirror.rs` answered 0) — but the board was NOT river-less,
-- which is why this survived. `rebuild_game` starts from `Game::new`, which
-- generates an ordinary CIVVIS map complete with its own river network, and
-- `apply_terrain` overwrites terrain, feature, resource and improvement while
-- leaving `river_edges` untouched. So the generated world's rivers showed
-- through, in the wrong places, on every mirrored game ever played.
--
-- Measured on the live run civvis-20260731T235836Z at turn 112: a Civilization VI
-- river plot is fresh water BY DEFINITION, yet only **12 of 33** CIVVIS river
-- tiles (36.4%) were fresh water in the export, against a **25.7%** base rate —
-- chance. Meanwhile 132 of 513 revealed plots really were fresh water.
--
-- Civilization VI stores a river on three of a plot's six edges — W, NW and NE.
-- The other three edges are the same segments held by neighbouring plots. Read
-- those neighbours too: the segment itself is visible from this revealed tile,
-- even when the terrain across it has not been revealed.
--
-- Sent as one small bitmask rather than three booleans to keep this loop's per
-- plot cost down — it already runs over every plot on the map.
local RIVER_W, RIVER_NW, RIVER_NE = 1, 2, 4;
local RIVER_E, RIVER_SE, RIVER_SW = 8, 16, 32;

local function riverMask(plot)
	local mask = 0;
	if try(function() return plot:IsWOfRiver(); end, false) then mask = mask + RIVER_W; end
	if try(function() return plot:IsNWOfRiver(); end, false) then mask = mask + RIVER_NW; end
	if try(function() return plot:IsNEOfRiver(); end, false) then mask = mask + RIVER_NE; end
	-- The engine stores the other three edges on the neighbouring plot. Querying
	-- those flags reveals no hidden terrain: all three segments are visibly on
	-- this revealed plot. Without them a boundary tile can report `ri=true` and
	-- `rv=0`, so the reconstructed board necessarily loses a known river.
	local x, y = plot:GetX(), plot:GetY();
	local west = try(function()
		return Map.GetAdjacentPlot(x, y, DirectionTypes.DIRECTION_WEST);
	end);
	local northwest = try(function()
		return Map.GetAdjacentPlot(x, y, DirectionTypes.DIRECTION_NORTHWEST);
	end);
	local northeast = try(function()
		return Map.GetAdjacentPlot(x, y, DirectionTypes.DIRECTION_NORTHEAST);
	end);
	if west ~= nil and try(function() return west:IsWOfRiver(); end, false) then
		mask = mask + RIVER_E;
	end
	if northwest ~= nil and try(function() return northwest:IsNWOfRiver(); end, false) then
		mask = mask + RIVER_SE;
	end
	if northeast ~= nil and try(function() return northeast:IsNEOfRiver(); end, false) then
		mask = mask + RIVER_SW;
	end
	return mask;
end

-- Return a resource only when this seat has unlocked its reveal technology.
-- `plot:GetResourceType()` exposes the map's underlying resource even when the
-- normal UI still shows bare ground. Exporting that value let the planner use
-- Niter, Coal, Oil and Antiquity Sites before the player could know they exist.
--
-- ⚠ `IsResourceVisible` ALONE IS NOT THE GATE, and this was measured, not
-- reasoned: on run civvis-20260807T162004Z the seat held 37 techs with
-- Refining not among them, and the export still carried seven RESOURCE_OIL
-- plots — `civ6_mirror_check` flagged every one ("raw resource leak(s) hidden
-- by CIVVIS"), while the shipped database and data/resources.json agree oil
-- reveals with TECH_REFINING. So on this build the engine call answers true
-- for a resource the seat has not unlocked. The database's own PrereqTech /
-- PrereqCivic columns are checked HERE as well, which cannot be wrong about
-- reveal rules regardless of what the engine call means by "visible" — and
-- the engine call is kept, because it also hides game-mode-disabled rows.
local function visibleResourceName(player, plot)
	local index = try(function() return plot:GetResourceType(); end, -1);
	if index == nil or index < 0 then return nil; end
	return try(function()
		local row = GameInfo.Resources[index];
		local resources = player:GetResources();
		if row == nil or resources == nil
			or not resources:IsResourceVisible(row.Hash) then
			return nil;
		end
		if row.PrereqTech ~= nil then
			local tech = GameInfo.Technologies[row.PrereqTech];
			local techs = player:GetTechs();
			if tech == nil or techs == nil or not techs:HasTech(tech.Index) then
				return nil;
			end
		end
		if row.PrereqCivic ~= nil then
			local civic = GameInfo.Civics[row.PrereqCivic];
			local culture = player:GetCulture();
			if civic == nil or culture == nil or not culture:HasCivic(civic.Index) then
				return nil;
			end
		end
		return row.ResourceType;
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
						r = visibleResourceName(player, plot),
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
						-- ★★★★ AND WHETHER IT IS PILLAGED. A pillaged Farm still reads
						-- IMPROVEMENT_FARM above and pays nothing until repaired; without
						-- this bit the mirror paid every pillaged improvement in full and
						-- CIVVIS could not see there was anything to repair. The first
						-- per-plot yield export (run civvis-20260816T040537Z) showed a
						-- pastured Horses tile at the bare-terrain figure for ninety
						-- turns. `IsImprovementPillaged` is what the shipped PlotToolTip
						-- reads. Sent only where an improvement stands, so an unimproved
						-- plot costs no bytes; nil (absent) elsewhere.
						p = try(function()
							if plot:GetImprovementType() < 0 then return nil; end
							return plot:IsImprovementPillaged() and true or nil;
						end, nil),
						-- This plot's own three river edges, as a bitmask: 1 = W,
						-- 2 = NW, 4 = NE. See `riverMask` above for why the other
						-- three edges do not need sending.
						rv = riverMask(plot),
						-- Whether ANY of the six edges carries a river, which is not
						-- derivable from `rv` alone: a river running only along this
						-- plot's E, SE or SW edge is recorded on the NEIGHBOUR's
						-- flags, so `rv` is 0 here while the plot is still riverside.
						-- Carried separately so a fresh-water check does not have to
						-- wait for the neighbour to be revealed.
						ri = try(function() return plot:IsRiver(); end, false),
						-- ★★★★ WHICH LANDMASS. Without it CIVVIS read 200 of 776 tiles
						-- as carrying a continent and the rest as none — the generated
						-- world's regions showing through, the same defect as the
						-- rivers above. "Another continent" is load-bearing in the
						-- ruleset, so a seat that cannot tell one landmass from another
						-- cannot reason about overseas settling or invasion at all.
						-- ⚠ NAME, not the raw index, for the reason `typeName` exists:
						-- the index is a row into the game's own Continents table and
						-- mapping it on the Rust side would mean guessing that table's
						-- ordering.
						ct = typeName("Continents", "ContinentType",
						              try(function() return plot:GetContinentType(); end, -1)),
						-- Gathering Storm's coastal-lowland band (1-3 metres), which
						-- decides what sea-level rise floods. `TerrainManager`, not a
						-- plot method — the plot has no accessor for it.
						cl = try(function()
							return TerrainManager.GetCoastalLowlandType(plot);
						end, -1),
						-- ★★★★ THE ROAD. Never exported; the mirror wrote `road = 0`
						-- everywhere and priced every march across roadless ground.
						-- Sent by name (`GameInfo.Routes`), nil where none stands.
						rt = try(function()
							local route = plot:GetRouteType();
							if route == nil or route < 0 then return nil; end
							return typeName("Routes", "RouteType", route);
						end, nil),
						rp = try(function() return plot:IsRoutePillaged() and true or nil; end, nil),
						-- ★★★★ WHAT STANDS ON THE OTHER CIVILIZATIONS' GROUND. A rival
						-- city record is name, size, health, walls and capital; its
						-- districts and wonders were never exported, so a rival's
						-- economy and defence were modelled from population alone
						-- and the mirror could not tell an Encampment from a farm.
						-- The plot knows (`GetDistrictType`, `GetWonderType`) for
						-- any revealed ground, ours included; sent only where one
						-- stands, nil elsewhere, so an empty plot costs no bytes.
						-- Our own cities' districts still cross with the city record
						-- (with completion and pillage); the mirror reads these
						-- for rivals and city-states.
						d = try(function()
							local kind = plot:GetDistrictType();
							if kind == nil or kind < 0 then return nil; end
							local row = GameInfo.Districts[kind];
							return row and row.DistrictType or nil;
						end, nil),
						-- ★★★★ ...AND WHETHER IT IS FINISHED. `GetDistrictType` answers
						-- for a district the moment it is PLACED, and a placed
						-- district is not adjacent to anything until it is built:
						-- Puteoli's Commercial Hub read "+2" beside a placed Campus
						-- for eleven turns and "+3" the turn the Campus completed
						-- (run civvis-20260816T223457Z t108-119; Arpinum's Campus
						-- 4→6 at t140, Ostia's at t198 the same way). Ravenna's
						-- Hub read one adjacency point over the host for thirty
						-- turns beside a city-state Encampment this flag would
						-- have said was unbuilt. `CityManager.GetDistrictAt` is
						-- what the shipped CityBannerManager reads for any owner's
						-- plot; sent only beside `d`, true/false.
						dc = try(function()
							local kind = plot:GetDistrictType();
							if kind == nil or kind < 0 then return nil; end
							local district = CityManager.GetDistrictAt(x, y);
							if district == nil then return nil; end
							return district:IsComplete() and true or false;
						end, nil),
						wo = try(function()
							local kind = plot:GetWonderType();
							if kind == nil or kind < 0 then return nil; end
							local row = GameInfo.Buildings[kind];
							return row and row.BuildingType or nil;
						end, nil),
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

-- ⚠⚠ CAN THIS CONTEXT ASSIGN A CITIZEN AT ALL? ASK, DO NOT ASSUME.
--
-- Specialists are the last untouched science lane and the gap is large.
-- Measured over the 19 live runs carrying the host export, 100 end-of-game
-- cities: **53 specialist assignments in total**, of which **45.3% sit on a
-- Commercial Hub and 26.4% on a Campus**. Of the 50 cities that HAVE a Campus,
-- 28 have no specialist anywhere and **only 8 have even one ON the Campus**.
-- Every one of those placements is Civilization VI's own citizen governor,
-- because this agent READS citizens (`IsPlotWorked`, `GetWorkerCount`) and has
-- never written one.
--
-- The human city panel does it from THIS context. `PlotInfo.lua`'s
-- `OnClickCitizen`:
--
--     tParameters[CityCommandTypes.PARAM_MANAGE_CITIZEN] =
--         UI.GetInterfaceModeParameter(CityCommandTypes.PARAM_MANAGE_CITIZEN);
--     tParameters[CityCommandTypes.PARAM_X] = kPlot:GetX();
--     tParameters[CityCommandTypes.PARAM_Y] = kPlot:GetY();
--     CityManager.RequestCommand(pCity, CityCommandTypes.MANAGE, tParameters);
--
-- ⚠ The open question is `PARAM_MANAGE_CITIZEN`. The shipped UI reads it from
-- `UI.GetInterfaceModeParameter`, i.e. from being inside
-- `InterfaceModeTypes.CITY_MANAGEMENT` — which an unattended agent is not.
-- Whether the command accepts the value passed directly is UNTESTED, and this
-- project has a ledger full of requests that returned without throwing and
-- changed nothing: "the request did not throw" is not "the engine took it".
--
-- So this ASKS AND DOES NOT ACT. `CanStartCommand` is the same read-only shape
-- as the `CanProduce(hash, false, true)` predicate that settled the production
-- channel, and it is tried against several candidate parameter values so the
-- log names which one — if any — the engine accepts. Nothing here can change a
-- game: no `RequestCommand` call exists in this function.
--
-- Off unless `cfg.ProbeCitizens` is set, like `ProbeChannels` above.
local function probeCitizenSlots(turn)
	local pid = Game.GetLocalPlayer();
	if pid == nil or pid < 0 then return; end
	local player = Players[pid];
	if player == nil then return; end
	-- Candidate values for the parameter the shipped UI reads out of the
	-- interface mode. `nil` is listed first and deliberately: if the command is
	-- legal without it, nothing else here matters.
	local candidates = {
		{ name = "absent", value = nil },
		{ name = "true", value = true },
		{ name = "one", value = 1 },
		{ name = "zero", value = 0 },
	};
	local cities = try(function() return player:GetCities(); end);
	if cities == nil then return; end
	local probed = 0;
	for _, city in cities:Members() do
		if probed >= (cfg.ProbeCitizenCities or 3) then break; end
		local cityId = try(function() return city:GetID(); end, -1);
		-- ⚠ Walk the city's PLOTS, not `GetDistricts()`. This file already
		-- records why (`exportState`): a plot carries the type and the position
		-- together, and the collection's per-member accessors vary across this
		-- build. The export reads `plot:GetDistrictType()` for exactly that
		-- reason, so the probe reads it the same way.
		local ownedPlots = try(function()
			return Map.GetCityPlots():GetPurchasedPlots(city);
		end);
		if ownedPlots ~= nil then
			for _, plotIndex in ipairs(ownedPlots) do
				local plot = try(function() return Map.GetPlotByIndex(plotIndex); end);
				local dx = plot ~= nil and try(function() return plot:GetX(); end, -1) or -1;
				local dy = plot ~= nil and try(function() return plot:GetY(); end, -1) or -1;
				local dtype = nil;
				if plot ~= nil then
					local d = try(function() return plot:GetDistrictType(); end, -1);
					if d ~= nil and d >= 0 then
						dtype = try(function() return GameInfo.Districts[d].DistrictType; end);
					end
				end
				-- The City Centre takes no specialist, so it is not evidence
				-- either way and only adds noise to the log.
				if dtype ~= nil and dtype ~= "DISTRICT_CITY_CENTER" and dx >= 0 then
					local workers = try(function() return plot:GetWorkerCount(); end, -1);
					local verdicts = {};
					for _, candidate in ipairs(candidates) do
						-- ⚠⚠ EVERY PART OF BUILDING THE TABLE IS INSIDE THE
						-- `pcall`, NOT JUST THE CALL. `CityCommandTypes.PARAM_*`
						-- is a bare index: if this build does not define one of
						-- these names it is `nil`, and `params[nil] = dx` raises
						-- "table index is nil" — which is a CHUNK-KILLING error
						-- outside a protected call, and this file has already
						-- lost whole runs to a script that stopped loading with
						-- nothing in any log naming it. A probe must not be able
						-- to take the agent down.
						local ok, can = pcall(function()
							local params = {};
							params[CityCommandTypes.PARAM_X] = dx;
							params[CityCommandTypes.PARAM_Y] = dy;
							if candidate.value ~= nil then
								params[CityCommandTypes.PARAM_MANAGE_CITIZEN] =
									candidate.value;
							end
							return CityManager.CanStartCommand(
								city, CityCommandTypes.MANAGE, params, true);
						end);
						verdicts[#verdicts + 1] = candidate.name .. "="
							.. (ok and tostring(can) or "threw");
					end
					emit("civvis_citizen_probe", {
						turn = turn,
						city = cityId,
						district = dtype,
						x = dx, y = dy,
						workers = workers,
						verdicts = table.concat(verdicts, ","),
					});
				end
			end
		end
		probed = probed + 1;
	end
end

-- ⭐ PUT ONE CITIZEN IN THE CAMPUS, WHERE THE MULTIPLIER ALREADY STANDS.
--
-- The probe answered the open question. Live run `civvis-20260804T173018Z`,
-- turn 50, for every district asked:
--
--     verdicts: absent=false, true=true, one=true, zero=true
--
-- `CanStartCommand(city, MANAGE, {PARAM_MANAGE_CITIZEN=<any non-nil>, X, Y})`
-- returns TRUE, and returns FALSE only when the parameter is ABSENT. The
-- interface mode was never the gate — `UI.GetInterfaceModeParameter` merely
-- supplies a value the command needs to be PRESENT, and any non-nil value does.
--
-- The gap this closes: across 100 live end-of-game cities there are 53
-- specialist assignments in total, **45.3% of them on Commercial Hubs and only
-- 26.4% on Campuses**, and of the 50 cities holding a Campus only **8** carry a
-- specialist on it. Every one of those placements is Civilization VI's own
-- citizen governor, because this agent has never issued a citizen order.
-- Specialists are also the widest-spread metric in the corpus: the top science
-- quartile of live runs holds **9** and the bottom holds **0**.
--
-- ⚠⚠ THIS IS A REALLOCATION, NOT FREE YIELD. A citizen moved into a specialist
-- slot stops working a tile. So it is deliberately narrow:
--
--   * only a CAMPUS, and only one whose plot currently has ZERO workers;
--   * only in a city that already holds a LIBRARY, so the science the
--     specialist makes is actually multiplied — a Campus specialist in a city
--     with no Library is the weakest version of this trade;
--   * at most ONE citizen per city per turn, so a bad trade cannot compound;
--   * `CanStartCommand` is asked BEFORE `RequestCommand`, and the outcome of
--     both is emitted, because "the request did not throw" is not "the engine
--     took it" and this project keeps a ledger of exactly that mistake.
--
-- Off unless `cfg.CampusSpecialist` is set.
local function fillCampusSpecialists(turn)
	local pid = Game.GetLocalPlayer();
	if pid == nil or pid < 0 then return; end
	local player = Players[pid];
	if player == nil then return; end
	local cities = try(function() return player:GetCities(); end);
	if cities == nil then return; end
	local libraryIndex = try(function() return GameInfo.Buildings["BUILDING_LIBRARY"].Index; end);
	if libraryIndex == nil then return; end
	for _, city in cities:Members() do
		local cityId = try(function() return city:GetID(); end, -1);
		-- The multiplier gate. Without a Library the specialist's beakers are
		-- not multiplied by anything and the tile it left may well be worth more.
		local blds = try(function() return city:GetBuildings(); end);
		local hasLibrary = blds ~= nil
			and try(function() return blds:HasBuilding(libraryIndex); end, false);
		if hasLibrary then
			local ownedPlots = try(function()
				return Map.GetCityPlots():GetPurchasedPlots(city);
			end);
			for _, plotIndex in ipairs(ownedPlots or {}) do
				local plot = try(function() return Map.GetPlotByIndex(plotIndex); end);
				local dtype = nil;
				if plot ~= nil then
					local d = try(function() return plot:GetDistrictType(); end, -1);
					if d ~= nil and d >= 0 then
						dtype = try(function() return GameInfo.Districts[d].DistrictType; end);
					end
				end
				-- `DISTRICT_CAMPUS` and nothing else. The civilization uniques
				-- that REPLACE a Campus (Seowon, Observatory) carry their own
				-- type and are deliberately left for a follow-up rather than
				-- guessed at here.
				if dtype == "DISTRICT_CAMPUS"
					and try(function() return plot:GetWorkerCount(); end, -1) == 0 then
					local px = try(function() return plot:GetX(); end, -1);
					local py = try(function() return plot:GetY(); end, -1);
					local ok, can = pcall(function()
						local params = {};
						params[CityCommandTypes.PARAM_X] = px;
						params[CityCommandTypes.PARAM_Y] = py;
						params[CityCommandTypes.PARAM_MANAGE_CITIZEN] = true;
						return CityManager.CanStartCommand(
							city, CityCommandTypes.MANAGE, params, true);
					end);
					local applied = false;
					if ok and can == true then
						applied = pcall(function()
							local params = {};
							params[CityCommandTypes.PARAM_X] = px;
							params[CityCommandTypes.PARAM_Y] = py;
							params[CityCommandTypes.PARAM_MANAGE_CITIZEN] = true;
							CityManager.RequestCommand(
								city, CityCommandTypes.MANAGE, params);
						end);
					end
					emit("civvis_campus_specialist", {
						turn = turn,
						city = cityId,
						x = px, y = py,
						can = (ok and tostring(can) or "threw"),
						applied = applied,
					});
					-- One per city per turn, whatever happened. A refusal is
					-- information; a retry loop is a stall.
					break;
				end
			end
		end
	end
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
-- `frame` selects a mid-turn combat frame's answer (CivvisFrames); 0 or nil
-- is the turn's opening board. `SELECT *` on purpose: a brain that predates
-- the `frame` column answers with rows that have none, which reads as frame
-- 0, instead of a query naming a column the table lacks and failing inside
-- the pcall forever.
local function ordersReady(turn, frame)
	if not attachOrders() then return nil; end
	frame = frame or 0;
	local count = nil;
	pcall(function()
		local rows = DB.Query(string.format(
			"SELECT * FROM civvis.ready WHERE run = '%s' AND turn = %d LIMIT 1",
			sqlSafe(cfg.RunTag), turn));
		for _, row in ipairs(rows) do
			if (tonumber(row.frame) or 0) == frame then count = row.count; end
		end
	end);
	return count;
end

local function fetchOrders(turn, frame)
	frame = frame or 0;
	local out = {};
	pcall(function()
		local rows = DB.Query(string.format(
			"SELECT * FROM civvis.orders WHERE run = '%s' AND turn = %d ORDER BY seq",
			sqlSafe(cfg.RunTag), turn));
		for _, row in ipairs(rows) do
			if (tonumber(row.frame) or 0) == frame then out[#out + 1] = row; end
		end
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
--
-- CIVVIS deliberately uses stable, human-readable project IDs, while Firaxis names
-- every repeatable district project `PROJECT_ENHANCE_DISTRICT_*`. These are semantic
-- renames rather than spelling variations, so `resolveType` cannot discover them by
-- trimming. Keep the vocabulary bridge at the game boundary with the other Civ 6
-- naming adaptations.
local CIVVIS_PROJECT_TYPES = {
	PROJECT_CAMPUS_RESEARCH_GRANTS = "PROJECT_ENHANCE_DISTRICT_CAMPUS",
	PROJECT_HOLY_SITE_PRAYERS = "PROJECT_ENHANCE_DISTRICT_HOLY_SITE",
	PROJECT_COMMERCIAL_HUB_INVESTMENT = "PROJECT_ENHANCE_DISTRICT_COMMERCIAL_HUB",
	PROJECT_HARBOR_SHIPPING = "PROJECT_ENHANCE_DISTRICT_HARBOR",
	PROJECT_ENCAMPMENT_TRAINING = "PROJECT_ENHANCE_DISTRICT_ENCAMPMENT",
	PROJECT_INDUSTRIAL_ZONE_LOGISTICS = "PROJECT_ENHANCE_DISTRICT_INDUSTRIAL_ZONE",
	PROJECT_THEATER_SQUARE_FESTIVAL = "PROJECT_ENHANCE_DISTRICT_THEATER",
};

-- The Government Plaza building names exposed to players are likewise aliases;
-- Firaxis's database retains implementation names describing the intended playstyle.
local CIVVIS_GOVERNMENT_BUILDING_TYPES = {
	BUILDING_AUDIENCE_CHAMBER = "BUILDING_GOV_TALL",
	BUILDING_ANCESTRAL_HALL = "BUILDING_GOV_WIDE",
	BUILDING_WARLORDS_THRONE = "BUILDING_GOV_CONQUEST",
	BUILDING_FOREIGN_MINISTRY = "BUILDING_GOV_CITYSTATES",
	BUILDING_INTELLIGENCE_AGENCY = "BUILDING_GOV_SPIES",
	BUILDING_GRAND_MASTERS_CHAPEL = "BUILDING_GOV_FAITH",
	BUILDING_WAR_DEPARTMENT = "BUILDING_GOV_MILITARY",
	BUILDING_NATIONAL_HISTORY_MUSEUM = "BUILDING_GOV_CULTURE",
	BUILDING_ROYAL_SOCIETY = "BUILDING_GOV_SCIENCE",
};

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

local function requestGovernorAssignment(pid, governorIndex, cityOwner, cityID)
	if governorIndex == nil or cityOwner == nil or cityID == nil or cityID < 0 then
		return false;
	end
	local city = try(function() return CityManager.GetCity(cityOwner, cityID); end);
	if city == nil then return false; end
	local params = {};
	params[PlayerOperations.PARAM_GOVERNOR_TYPE] = governorIndex;
	-- Gathering Storm's assignment chooser sends the owner explicitly. City ids
	-- collide across players, so omitting this can post Amani to the wrong city.
	params[PlayerOperations.PARAM_PLAYER_ONE] = cityOwner;
	params[PlayerOperations.PARAM_CITY_DEST] = cityID;
	return pcall(function()
		UI.RequestPlayerOperation(pid, PlayerOperations.ASSIGN_GOVERNOR, params);
	end);
end

local function onGovernorAppointed(playerID, governorID)
	local pending = pendingGovernorAssignments[playerID];
	if pending == nil or pending.governor ~= governorID then return; end
	pendingGovernorAssignments[playerID] = nil;
	local ok = requestGovernorAssignment(
		playerID, governorID, pending.city_player, pending.city);
	emit("governor_assignment", {
		turn = try(function() return Game.GetCurrentGameTurn(); end, -1),
		player = playerID, governor = governorID,
		city_player = pending.city_player, city = pending.city, applied = ok,
	});
end

-- ★★★★★ THE SALE LANE'S STATE AND THE RIVAL'S ANSWER TO IT.
--
-- A `sell` order (see the arm in `applyOrder`) puts CIVVIS's surplus — a
-- duplicate luxury copy, a strategic block, idle diplomatic favor — into the
-- outgoing working deal and asks the rival's own valuation for the gold with
-- `DealProposalAction.EQUALIZE`, exactly the shipped "What would you give me
-- for this?" button (DiplomacyDealView.lua `RequestEqualizeWorkingDeal`). The
-- rival answers through `Events.DiplomacyIncomingDeal` with its balanced
-- INCOMING working deal; the shipped screen copies that over the outgoing deal
-- and, when the two are equal, sends `ACCEPTED` — which is what enacts the
-- trade (`OnProposeOrAcceptDeal`). This handler is that click, made only when
-- the answer is what was asked for: our side holds exactly the items offered,
-- their side holds gold and nothing else, and the gold clears the floor the
-- order carried. Anything else is left on the table: the outgoing deal is
-- cleared and nothing is sent, the same exit the shipped screen takes when the
-- human simply walks away from a deal they opened.
--
-- ⚠ Every incoming deal is written to the ledger (`deal_response`), whether or
-- not this lane asked for it — the peace lane above has submitted hundreds of
-- proposals without ever seeing an answer, and this is the first record of
-- what the engine says back. A deal this lane did not ask for (a rival's own
-- proposal, which arrives with `PROPOSED`) is logged and otherwise untouched:
-- that screen belongs to the harness's closers.
--
-- Bare globals, both: the main chunk sits at Lua's 200-register ceiling, and
-- the offline regression (`deal_sale_test.lua`) reads them.
CivvisTrade = { pending = {}, asked = {} };

CivvisOnIncomingDeal = function(fromPlayer, toPlayer, action)
	local pid = try(function() return Game.GetLocalPlayer(); end, -1);
	if pid == nil or pid < 0 or toPlayer ~= pid or fromPlayer == nil then return; end
	local turn = try(function() return Game.GetCurrentGameTurn(); end, -1);
	local trade = CivvisTrade;
	local pending = trade.pending[fromPlayer];
	local incoming = try(function()
		return DealManager.GetWorkingDeal(DealDirection.INCOMING, pid, fromPlayer);
	end, nil);
	-- Read both sides. On a SALE `theirs` is what the rival puts up; only
	-- gold counts and anything else marks the answer foreign. `mine` is what
	-- the answer says we give, matched against the offer by Firaxis type,
	-- value and amount, so an equalizer that touched our side cannot slip a
	-- bigger block or a different item through the accept. On a BUY the
	-- directions flip: their side must hold exactly the Open Borders
	-- agreement asked for (anything else is foreign, their gold included),
	-- and our side must hold gold and nothing else, totalled into `pay`.
	local buying = pending ~= nil and pending.direction == "buy";
	local gold, gpt, foreign, mine, offered = 0, 0, 0, {}, 0;
	local borders, payGold, payGpt = 0, 0, 0;
	local mineText = {};
	if incoming ~= nil then
		pcall(function()
			for item in incoming:Items() do
				local kind = item:GetType();
				local from = item:GetFromPlayerID();
				local duration = item:GetDuration() or 0;
				local amount = item:GetAmount() or 0;
				if from == fromPlayer then
					if kind == DealItemTypes.GOLD and not buying then
						if duration == 0 then gold = gold + amount; else gpt = gpt + amount; end
					elseif buying and kind == DealItemTypes.AGREEMENTS
							and DealAgreementTypes ~= nil
							and try(function() return item:GetSubType(); end, nil)
								== DealAgreementTypes.OPEN_BORDERS then
						borders = borders + 1;
					else
						foreign = foreign + 1;
					end
				else
					if buying and kind == DealItemTypes.GOLD then
						if duration == 0 then payGold = payGold + amount; else payGpt = payGpt + amount; end
						mineText[#mineText + 1] = "GOLD=" .. tostring(amount) .. "x" .. tostring(duration);
					else
						local key;
						if kind == DealItemTypes.FAVOR then
							key = "FAVOR";
						elseif kind == DealItemTypes.RESOURCES then
							key = "RESOURCES:" .. tostring(item:GetValueType());
						elseif kind == DealItemTypes.GREATWORK then
							-- Matched by the work INSTANCE the sale offered.
							-- A work has no amount — its presence is its
							-- quantity, and 0 here would fail the match
							-- against the `1` the ask registered.
							key = "GREATWORK:" .. tostring(item:GetValueType());
							amount = 1;
						else
							key = "OTHER:" .. tostring(kind);
						end
						mine[key] = (mine[key] or 0) + amount;
						mineText[#mineText + 1] = key .. "=" .. tostring(amount) .. "x" .. tostring(duration);
					end
				end
			end
		end);
	end
	local matches = pending ~= nil;
	if buying then
		-- The answer must be the agreement asked for and a price, nothing
		-- else in either direction — a counter that slips another item onto
		-- our side or keeps the agreement off theirs is walked away from.
		matches = borders == 1 and next(mine) == nil;
	elseif pending ~= nil then
		for key, amount in pairs(pending.gave or {}) do
			offered = offered + 1;
			if mine[key] ~= amount then matches = false; end
		end
		for key, _ in pairs(mine) do
			if (pending.gave or {})[key] == nil then matches = false; end
		end
	end
	local worth = gold + gpt * 25;
	local pay = payGold + payGpt * 25;
	local session = try(function()
		return DiplomacyManager.FindOpenSessionID(pid, fromPlayer);
	end, nil);
	local closable;
	if buying then
		closable = matches and foreign == 0
			and pay <= (pending.ceiling or 0)
			and (action == DealProposalAction.ACCEPTED or action == DealProposalAction.ADJUSTED);
	else
		closable = pending ~= nil and matches and foreign == 0 and offered > 0
			and worth >= (pending.floor or 0)
			and (action == DealProposalAction.ACCEPTED or action == DealProposalAction.ADJUSTED);
	end
	emit("deal_response", {
		turn = turn, from = fromPlayer, action = action,
		direction = buying and "buy" or "sell",
		gold = gold, gold_per_turn = gpt, worth = worth, pay = pay, foreign = foreign,
		ours = table.concat(mineText, ","),
		asked = pending ~= nil, asked_turn = pending and pending.turn or nil,
		floor = pending and pending.floor or nil,
		ceiling = pending and pending.ceiling or nil, matches = matches,
		session = session ~= nil and session or -1,
		closable = closable,
	});
	if pending == nil then return; end
	-- One answer settles one ask, whichever way it went.
	trade.pending[fromPlayer] = nil;
	if not closable then
		pcall(function() DealManager.ClearWorkingDeal(DealDirection.OUTGOING, pid, fromPlayer); end);
		emit("deal_declined", {
			turn = turn, from = fromPlayer, action = action, worth = worth,
			direction = buying and "buy" or "sell", pay = pay,
			floor = pending.floor, ceiling = pending.ceiling,
			matches = matches, foreign = foreign,
		});
		return;
	end
	local ok, sent = pcall(function()
		DealManager.CopyIncomingToOutgoingWorkingDeal(pid, fromPlayer);
		if not DealManager.AreWorkingDealsEqual(pid, fromPlayer) then return false; end
		DealManager.SendWorkingDeal(DealProposalAction.ACCEPTED, pid, fromPlayer);
		return true;
	end);
	emit("deal_closed", {
		turn = turn, from = fromPlayer, gold = gold, gold_per_turn = gpt, worth = worth,
		direction = buying and "buy" or "sell", pay = pay,
		floor = pending.floor, ceiling = pending.ceiling,
		gave = pending.verb, sent = (ok and sent) and true or false,
		threw = not ok,
	});
end

-- ------------------------------------------------------------ tactical ledger
--
-- ★★★★★ NOTHING IN A LIVE RUN'S RECORD SAID WHO KILLED WHOM. Kills, losses,
-- damage dealt and taken, captures — every reading of the live army so far was
-- a reconstruction from units vanishing between two `state` exports, or the
-- host's Hall of Fame opened by hand: twelve finished games, 343 of ours lost,
-- 61 of theirs killed, 0 cities taken. This block writes the combat record the
-- host already knows into the run's own event stream, so
-- `tools/civ6_tactics_ledger.py` reads a ledger and not a guess.
--
-- Two sources, recorded side by side because neither is verified in-game yet:
--   * `Events.CombatVisBegin/End(kVisData)` — the host's own combat
--     visualisation, fired for every combat this seat can see, carrying the
--     attacker/defender component ids (`playerID`, `componentID`,
--     `componentType`, the shape MapPinManager.lua reads). Hit points are read
--     back at Begin and at End; a defender that no longer resolves at End was
--     killed.
--   * `Events.UnitDamageChanged(player, unitId, damage)` — the core's own
--     per-unit damage change, collected while a combat is open.
--   The ledger tool prefers the damage events and falls back to the readback.
--
-- And the host's own STRIKE PREVIEW: `CombatManager.SimulateAttackInto` is what
-- the shipped UnitPanel calls to draw the combat preview, and it answers the
-- same numbers for our order before it is issued. Recorded as `strike`, and
-- joined onto the `combat` that follows, so predicted-versus-actual per strike
-- is one field apart.
--
-- ⚠ Handles are never cached across events (see the SIGSEGV note on
-- `applyOrder`); every read re-resolves through `UnitManager.GetUnit`.
-- One bare global table: the chunk is at Lua 5.1's 200-local ceiling.
CivvisLedger = { open = {}, damage = {}, pending = {}, kinds = {} };

CivvisLedger.componentKey = function(id)
	if id == nil then return nil; end
	local player = try(function() return id.playerID; end, nil);
	local comp = try(function() return id.componentID; end, nil);
	if player == nil or comp == nil then return nil; end
	return tostring(player) .. ":" .. tostring(comp);
end;

-- What a combat participant is right now: hp for a unit, garrison/wall damage
-- for a city or district; `gone` when it no longer resolves.
CivvisLedger.describe = function(id)
	if id == nil then return nil; end
	local player = tonumber(try(function() return id.playerID; end, nil));
	local comp = tonumber(try(function() return id.componentID; end, nil));
	if player == nil or comp == nil then return nil; end
	local isUnit = try(function() return id.componentType == ComponentType.UNIT; end, true);
	if isUnit ~= false then
		local unit = try(function() return UnitManager.GetUnit(player, comp); end);
		if unit == nil then return { player = player, id = comp, type = "unit", gone = true }; end
		return {
			player = player, id = comp, type = "unit",
			kind = try(function() return GameInfo.Units[unit:GetUnitType()].UnitType; end, "?"),
			x = tonumber(try(function() return unit:GetX(); end, -1)) or -1,
			y = tonumber(try(function() return unit:GetY(); end, -1)) or -1,
			hp = 100 - (tonumber(try(function() return unit:GetDamage(); end, 0)) or 0),
		};
	end
	-- A city or district defends with its garrison and its walls.
	local district = try(function() return CityManager.GetDistrict(player, comp); end);
	if district == nil then return { player = player, id = comp, type = "district", gone = true }; end
	return {
		player = player, id = comp, type = "district",
		x = tonumber(try(function() return district:GetX(); end, -1)) or -1,
		y = tonumber(try(function() return district:GetY(); end, -1)) or -1,
		hp = tonumber(try(function()
			return district:GetMaxDamage(DefenseTypes.DISTRICT_GARRISON)
				- district:GetDamage(DefenseTypes.DISTRICT_GARRISON);
		end, nil)),
		wall_hp = tonumber(try(function()
			return district:GetMaxDamage(DefenseTypes.DISTRICT_OUTER)
				- district:GetDamage(DefenseTypes.DISTRICT_OUTER);
		end, nil)),
	};
end;

-- The host's own preview of the strike about to be requested. nil when the
-- host is busy or the API is absent — an honest blank, not a zero.
CivvisLedger.preview = function(unit, verb, x, y)
	if try(function() return UI.IsGameCoreBusy(); end, false) == true then return nil; end
	local combatType = nil;
	if verb == "RANGE_ATTACK" then
		combatType = try(function()
			local ranged = unit:GetRangedCombat() or 0;
			local bombard = unit:GetBombardCombat() or 0;
			if bombard > ranged then return CombatTypes.BOMBARD; end
			return CombatTypes.RANGED;
		end, nil);
	end
	local results = try(function()
		return CombatManager.SimulateAttackInto(unit:GetComponentID(), combatType, x, y);
	end, nil);
	if results == nil then return nil; end
	local function read(side, key)
		return tonumber(try(function()
			return results[CombatResultParameters[side]][CombatResultParameters[key]];
		end, nil));
	end
	local out = {
		damage_to_defender = read("DEFENDER", "DAMAGE_TO"),
		damage_to_attacker = read("ATTACKER", "DAMAGE_TO"),
		defender_wall_damage = read("DEFENDER", "DEFENSE_DAMAGE_TO"),
		attacker_strength = read("ATTACKER", "COMBAT_STRENGTH"),
		defender_strength = read("DEFENDER", "COMBAT_STRENGTH"),
	};
	if out.damage_to_defender == nil and out.damage_to_attacker == nil then return nil; end
	return out;
end;

-- Called from `applyOrder` before a strike is requested: emit the preview and
-- remember it, so the combat this strike produces can carry it.
CivvisLedger.strike = function(unit, subject, verb, x, y, turn)
	if CivvisFrames ~= nil then CivvisFrames.noteStrike(); end
	local preview = CivvisLedger.preview(unit, verb, x, y);
	local kind = try(function() return GameInfo.Units[unit:GetUnitType()].UnitType; end, "?");
	local hp = 100 - (tonumber(try(function() return unit:GetDamage(); end, 0)) or 0);
	CivvisLedger.pending[tostring(subject)] = {
		turn = turn, verb = verb, x = x, y = y, preview = preview, hp = hp, kind = kind,
	};
	emit("strike", { turn = turn, unit = subject, unit_kind = kind, verb = verb,
	                 x = x, y = y, hp = hp, preview = preview });
end;

CivvisLedger.onCombatVisBegin = function(kVisData)
	local attacker = try(function() return kVisData[CombatVisType.ATTACKER]; end);
	local defender = try(function() return kVisData[CombatVisType.DEFENDER]; end);
	local key = CivvisLedger.componentKey(attacker);
	if key == nil then return; end
	CivvisLedger.open[key] = {
		turn = tonumber(try(function() return Game.GetCurrentGameTurn(); end, -1)) or -1,
		attacker = CivvisLedger.describe(attacker),
		defender = CivvisLedger.describe(defender),
		attacker_id = attacker, defender_id = defender,
		damage_events = {},
	};
end;

CivvisLedger.onUnitDamageChanged = function(player, unitId, damage)
	local who = tostring(player) .. ":" .. tostring(unitId);
	local previous = CivvisLedger.damage[who];
	CivvisLedger.damage[who] = tonumber(damage);
	if previous == nil or tonumber(damage) == nil then return; end
	local delta = tonumber(damage) - previous;
	for _, combat in pairs(CivvisLedger.open) do
		if CivvisLedger.componentKey(combat.attacker_id) == who
				or CivvisLedger.componentKey(combat.defender_id) == who then
			combat.damage_events[#combat.damage_events + 1] = { who = who, delta = delta };
		end
	end
end;

CivvisLedger.onCombatVisEnd = function(kVisData)
	local attacker = try(function() return kVisData[CombatVisType.ATTACKER]; end);
	local key = CivvisLedger.componentKey(attacker);
	if key == nil then return; end
	local combat = CivvisLedger.open[key];
	CivvisLedger.open[key] = nil;
	if combat == nil then return; end
	local attackerNow = CivvisLedger.describe(combat.attacker_id);
	local defenderNow = CivvisLedger.describe(combat.defender_id);
	local pid = tonumber(try(function() return Game.GetLocalPlayer(); end, -1)) or -1;
	local preview = nil;
	if combat.attacker ~= nil and combat.attacker.player == pid then
		local pending = CivvisLedger.pending[tostring(combat.attacker.id)];
		if pending ~= nil and pending.turn == combat.turn then
			preview = pending.preview;
			CivvisLedger.pending[tostring(combat.attacker.id)] = nil;
		end
	end
	local function hpOf(desc) return desc ~= nil and desc.hp or nil; end
	local function wallOf(desc) return desc ~= nil and desc.wall_hp or nil; end
	local defenderKilled = defenderNow ~= nil and defenderNow.gone == true;
	local attackerKilled = attackerNow ~= nil and attackerNow.gone == true;
	local damageToDefender, damageToAttacker = nil, nil;
	if hpOf(combat.defender) ~= nil then
		if defenderKilled then damageToDefender = hpOf(combat.defender);
		elseif hpOf(defenderNow) ~= nil then damageToDefender = hpOf(combat.defender) - hpOf(defenderNow); end
	end
	if hpOf(combat.attacker) ~= nil then
		if attackerKilled then damageToAttacker = hpOf(combat.attacker);
		elseif hpOf(attackerNow) ~= nil then damageToAttacker = hpOf(combat.attacker) - hpOf(attackerNow); end
	end
	emit("combat", {
		turn = combat.turn,
		attacker = combat.attacker, defender = combat.defender,
		attacker_hp_end = hpOf(attackerNow), defender_hp_end = hpOf(defenderNow),
		defender_wall_hp_end = wallOf(defenderNow),
		damage_to_defender = damageToDefender, damage_to_attacker = damageToAttacker,
		defender_killed = defenderKilled, attacker_killed = attackerKilled,
		damage_events = combat.damage_events,
		ours = combat.attacker ~= nil and combat.attacker.player == pid,
		against_us = combat.defender ~= nil and combat.defender.player == pid,
		preview = preview,
	});
end;

-- One of OUR units left the map — combat, disband, capture, deletion. Named
-- with the kind the last export knew, and with the treasury, so a bankruptcy
-- disband and a battlefield loss are one field apart.
CivvisLedger.onUnitRemoved = function(player, unitId)
	local pid = tonumber(try(function() return Game.GetLocalPlayer(); end, -1)) or -1;
	if tonumber(player) ~= pid then return; end
	emit("unit_lost", {
		turn = tonumber(try(function() return Game.GetCurrentGameTurn(); end, -1)) or -1,
		unit = tonumber(unitId), unit_kind = CivvisLedger.kinds[tostring(unitId)],
		gold = tonumber(try(function()
			return math.floor(Players[pid]:GetTreasury():GetGoldBalance());
		end, nil)),
	});
end;

CivvisLedger.onCityOccupationChanged = function(player, cityId)
	local pid = tonumber(try(function() return Game.GetLocalPlayer(); end, -1)) or -1;
	local city = try(function() return CityManager.GetCity(player, cityId); end);
	emit("city_occupation", {
		turn = tonumber(try(function() return Game.GetCurrentGameTurn(); end, -1)) or -1,
		player = tonumber(player), city = tonumber(cityId),
		name = try(function() return city:GetName(); end, nil),
		original_owner = try(function() return city:GetOriginalOwner(); end, nil),
		ours_now = tonumber(player) == pid,
	});
end;

local function applyOrder(player, pid, row, turn)
	-- Build and submit a major-civilization peace proposal without opening a
	-- diplomacy session.  A session displays `DiplomacyDealView`, whose only safe
	-- automatic exit is refusal; it therefore cannot carry an outbound peace offer
	-- in an unattended run.  Firaxis's own `ProposeWorkingDeal(false)` sends this
	-- same validated working deal directly with `PROPOSED`.
	--
	-- Kept inside the order handler: a file-scope local would cross Civ 6's
	-- 200-register main-chunk ceiling and make the entire mod fail to compile.
	-- Returns `(submitted, concession, reason)`.  `submitted` names an actual
	-- `SendWorkingDeal` call, deliberately not merely a `pcall` that did not throw.
	local function submitMajorPeaceDeal(subject, asked)
		if DealManager.HasPendingDeal(pid, subject) then
			return false, 0, "pending";
		end
		DealManager.ClearWorkingDeal(DealDirection.OUTGOING, pid, subject);
		local deal = DealManager.GetWorkingDeal(DealDirection.OUTGOING, pid, subject);
		if deal == nil then return false, 0, "no_working_deal"; end

		local item = deal:AddItemOfType(DealItemTypes.AGREEMENTS, pid);
		if item == nil then return false, 0, "no_peace_item"; end
		item:SetSubType(DealAgreementTypes.MAKE_PEACE);
		item:SetLocked(true);
		-- "Validate the deal, this will make sure peace is on both sides of the
		-- deal." — the shipped comment beside the UI's Make Peace action.
		deal:Validate();
		if not deal:IsValid() then return false, 0, "invalid_deal"; end

		local concession = 0;
		-- A free peace offer is the right first question.  Once the same rival
		-- remains at war through the host's retry window, it has already declined
		-- that exact white deal.  Preserve a quarter of the treasury for emergency
		-- purchases and offer the rest only on the retry; a rejected deal transfers
		-- nothing.
		if asked ~= nil then
			local tribute = deal:AddItemOfType(DealItemTypes.GOLD, pid);
			if tribute ~= nil then
				tribute:SetDuration(0);
				local balance = try(function()
					return player:GetTreasury():GetGoldBalance();
				end, 0) or 0;
				local amount = math.min(math.floor(balance * 0.75),
					tribute:GetMaxAmount() or 0);
				if amount > 0 then
					tribute:SetAmount(amount);
					if tribute:IsValid() then
						concession = amount;
					else
						deal:RemoveItemByID(tribute:GetID());
					end
				else
					deal:RemoveItemByID(tribute:GetID());
				end
			end
		end
		-- The optional Gold item changes the finished package.  Match the shipped
		-- proposal surface by validating that final package before it can be sent.
		deal:Validate();
		if not deal:IsValid() then return false, 0, "invalid_deal"; end

		-- This is the exact normal-offer call in shipped DiplomacyDealView.lua.
		-- Unlike `RequestSession(..., "MAKE_DEAL")`, it does not route through the
		-- anti-stall closer that must refuse every on-screen deal.
		DealManager.SendWorkingDeal(DealProposalAction.PROPOSED, pid, subject);
		return true, concession, "submitted";
	end

	-- Send the exact Gold amount a final-turn Aid Request needs through the
	-- ordinary deal surface.  Expansion2's emergency manager listens to normal
	-- deal Gold items for `EMERGENCY_SEND_AID` and
	-- `EMERGENCY_SEND_MILITARY_AID`; this is intentionally a one-way gift, not
	-- an EQUALIZE ask or a deal-screen session.  A partial gift is worse than no
	-- gift here: Rust asked for the smallest amount that takes first place, so
	-- if the host's current treasury or item limit cannot meet it we submit
	-- nothing and leave the competition score unchanged.
	local function submitAidGift(subject, asked)
		if DealManager.HasPendingDeal(pid, subject) then
			return false, 0, "pending";
		end
		local amount = math.max(0, math.floor(asked or 0));
		if amount <= 0 then return false, 0, "no_amount"; end
		local balance = try(function()
			return player:GetTreasury():GetGoldBalance();
		end, 0) or 0;
		if balance < amount then return false, 0, "unaffordable"; end
		DealManager.ClearWorkingDeal(DealDirection.OUTGOING, pid, subject);
		local deal = DealManager.GetWorkingDeal(DealDirection.OUTGOING, pid, subject);
		if deal == nil then return false, 0, "no_working_deal"; end
		local gift = deal:AddItemOfType(DealItemTypes.GOLD, pid);
		if gift == nil then return false, 0, "no_gold_item"; end
		gift:SetDuration(0);
		local maximum = tonumber(try(function() return gift:GetMaxAmount(); end, 0)) or 0;
		if maximum < amount then
			pcall(function() deal:RemoveItemByID(gift:GetID()); end);
			return false, 0, "gold_limit";
		end
		gift:SetAmount(amount);
		if not try(function() return gift:IsValid(); end, false) then
			pcall(function() deal:RemoveItemByID(gift:GetID()); end);
			return false, 0, "gold_invalid";
		end
		deal:Validate();
		if not deal:IsValid() then return false, 0, "invalid_deal"; end
		-- The same direct normal-offer call as Firaxis's deal UI and the peace
		-- submitter above.  The recipient's acceptance and the score change are
		-- observed in later host exports; this return value says only submitted.
		DealManager.SendWorkingDeal(DealProposalAction.PROPOSED, pid, subject);
		return true, amount, "submitted";
	end

	-- A city that is already taking fire cannot wait for the strategic planner to
	-- notice the same fact on its next board. Return only engine-visible enemies;
	-- the damage read is the fallback for the turn an attack has already landed.
	-- This is an actuation guard, not a new target-selection policy.
	--
	-- ★★★★★ ASK THE ENGINE WHY A REFUSAL HAPPENED, DO NOT INFER IT.
	--
	-- Civilization VI will say, but only if asked exactly right: the fourth
	-- argument is a BOOLEAN `bTestOnly`, the fifth is `OperationResultsTypes.ALL`
	-- (not `true`), and the reasons live under
	-- `UnitOperationResults.FAILURE_REASONS` rather than at the top level. Read
	-- out of the shipped `UnitPanel.lua:630`.
	--
	-- ⚠⚠ THAT SIGNATURE WAS GUESSED WRONG TWICE and each guess cost a whole class
	-- of diagnosis — every `found_refused` in the project's history read
	-- `can_start=false` with no reasons, because the results table was never
	-- populated. One copy now, so the next refusal that wants a cause inherits
	-- the working call instead of guessing a third time.
	--
	-- `params` must be the SAME table the refused operation was given: a build
	-- refused at a particular tile cannot be explained by asking about no tile.
	-- The reasons are LOC keys and are localised, because an untranslated key
	-- names the rule but not in words anyone reading the ledger would recognise.
	-- Only ever called on a failure path, so a normal turn pays nothing.
	--
	-- ⚠⚠⚠ AND I GUESSED THE SIGNATURE A THIRD TIME. #1536 applied the five
	-- argument FOUND_CITY form to a hash operation carrying params:
	--
	--   CanStartOperation(unit, operation, params, false, ResultsTypes.ALL)
	--
	-- which puts `params` in the PLOTS slot and `false` in the PARAMS slot. It
	-- throws, the pcall swallowed it, and every `improve_refused` in the first
	-- live run on that build read `why: "unknown"` -- six for six, the same
	-- silent ledger the event was added to end. `canOperate` a few hundred lines
	-- above has the working hash form and it is a DIFFERENT arity:
	--
	--   CanStartOperation(unit, hash, nil, params)          -- 4-arg, hash ops
	--   CanStartOperation(unit, TYPE, nil, false, ALL)      -- 5-arg, results
	--
	-- So stop guessing and do what this file already demands of itself: try each
	-- shipped form and NAME THE ONE THAT ANSWERED, exactly as `plotRevealed`
	-- does for `PlayersVisibility`. A probe that cannot say which call worked is
	-- how this went wrong three times; `why` now always carries its provenance,
	-- and a throw is reported rather than swallowed.
	--
	-- Nested for the same reason `cityWarThreat` is: see below.
	local function refusalReason(unit, operation, params)
		local attempts = {
			-- The hash form, with params where `canOperate` proves they go.
			{ form = "p4r", call = function()
				return UnitManager.CanStartOperation(
					unit, operation, nil, params or {},
					OperationResultsTypes.ALL);
			end },
			-- The shipped FOUND_CITY form, for parameterless operations.
			{ form = "t5r", call = function()
				return UnitManager.CanStartOperation(
					unit, operation, nil, false, OperationResultsTypes.ALL);
			end },
		};
		local fallback = nil;
		for _, attempt in ipairs(attempts) do
			local called, ok, results = pcall(attempt.call);
			if called then
				if results ~= nil
						and results[UnitOperationResults.FAILURE_REASONS] ~= nil then
					local parts = {};
					for _, key in ipairs(results[UnitOperationResults.FAILURE_REASONS]) do
						parts[#parts + 1] =
							try(function() return Locale.Lookup(key); end, tostring(key));
					end
					if #parts > 0 then
						return table.concat(parts, " | ") .. " [" .. attempt.form .. "]";
					end
				end
				-- Answered, but had no reasons to give. Keep it only if nothing
				-- better turns up: the other form may still carry the words.
				if fallback == nil then
					fallback = "can_start=" .. tostring(ok) .. ",no_reasons ["
						.. attempt.form .. "]";
				end
			elseif fallback == nil then
				-- ⚠ The throw is the answer when nothing else works. Swallowing it
				-- is what produced six "unknown"s and no way to tell which of the
				-- two forms this ruleset wants.
				fallback = "probe_threw[" .. attempt.form .. "]:"
					.. string.sub(tostring(ok), 1, 80);
			end
		end
		return fallback or "no_probe_answered";
	end

	-- Keep this helper inside the order handler. A file-scope local consumes one
	-- of the main chunk's 200 Lua registers; crossing that ceiling makes Civ 6
	-- silently discard the entire mod before Initialize can emit a lifecycle event.
	local function cityWarThreat(cityPlayer, cityPid, city)
		local cx = try(function() return city:GetX(); end, -1);
		local cy = try(function() return city:GetY(); end, -1);
		local _, damage, _, wallDamage, maxWallDamage = cityDefence(cx, cy);
		-- The expensive enemy roster walk is only relevant to an unwalled city.
		-- Unknown wall state stays unknown; do not invent a threat from a failed read.
		if maxWallDamage == nil or maxWallDamage > 0 then
			return false, nil, damage, wallDamage, maxWallDamage;
		end
		local diplomacy = try(function() return cityPlayer:GetDiplomacy(); end);
		local atWar = false;
		local nearestEnemy = nil;
		if diplomacy ~= nil then
			for _, otherId in ipairs(try(function()
				return PlayerManager.GetAliveMajorIDs();
			end, {}) or {}) do
				if otherId ~= cityPid and try(function()
					return diplomacy:IsAtWarWith(otherId);
				end, false) then
					atWar = true;
					local other = Players[otherId];
					local visibility = PlayersVisibility[cityPid];
					if other ~= nil and visibility ~= nil then
						pcall(function()
							for _, unit in other:GetUnits():Members() do
								pcall(function()
									local ux, uy = unit:GetX(), unit:GetY();
									if visibility:IsVisible(ux, uy) then
										local distance = plotDistance(cx, cy, ux, uy);
										if distance >= 0 and (nearestEnemy == nil
											or distance < nearestEnemy) then
											nearestEnemy = distance;
										end
									end
								end);
							end
						end);
					end
				end
			end
		end
		return atWar, nearestEnemy, damage, wallDamage, maxWallDamage;
	end

	local kind = tostring(row.kind or "");
	local verb = tostring(row.verb or "");
	local subject = tonumber(row.subject) or -1;
	local x, y = tonumber(row.x), tonumber(row.y);

	if kind == "governor_appoint" or kind == "governor_assign" then
		local governor, resolved = resolveType(GameInfo.Governors, verb);
		if governor == nil then return false, "unknown_" .. verb; end
		local governors = try(function() return player:GetGovernors(); end);
		if governors == nil then return false, "no_governors"; end
		local cityOwner = x ~= nil and x or pid;
		if try(function() return CityManager.GetCity(cityOwner, subject); end) == nil then
			return false, "governor_city_missing";
		end
		local held = try(function() return governors:HasGovernor(governor.Hash); end, false);
		if kind == "governor_assign" or held then
			if not held then return false, "governor_not_appointed"; end
			local appointed = try(function() return governors:GetGovernor(governor.Hash); end);
			if appointed ~= nil and try(function()
				return appointed:GetNeutralizedTurns();
			end, 0) > 0 then
				return false, "governor_neutralized";
			end
			local ok = requestGovernorAssignment(
				pid, governor.Index, cityOwner, subject);
			return ok, ok and resolved or "governor_assign_throw";
		end
		if not try(function() return governors:CanAppoint(); end, false) then
			return false, "no_governor_title";
		end
		if not try(function()
			return governors:CanEverAppointGovernor(governor.Hash);
		end, false) then
			return false, "governor_not_appointable";
		end
		pendingGovernorAssignments[pid] = {
			governor = governor.Index, city_player = cityOwner, city = subject,
		};
		local params = {};
		-- The shipped GovernorPanel sends row INDEXES in operation parameters.
		-- The disabled legacy path sent Hash here, which is the root cause of its
		-- repeatable game-core crash.
		params[PlayerOperations.PARAM_GOVERNOR_TYPE] = governor.Index;
		local ok = pcall(function()
			UI.RequestPlayerOperation(pid, PlayerOperations.APPOINT_GOVERNOR, params);
		end);
		if not ok then pendingGovernorAssignments[pid] = nil; end
		return ok, ok and resolved or "governor_appoint_throw";
	end

	if kind == "governor_promote" then
		local comma = string.find(verb, ",", 1, true);
		if comma == nil then return false, "governor_promotion_malformed"; end
		local governorType = string.sub(verb, 1, comma - 1);
		local promotionType = string.sub(verb, comma + 1);
		local governor, governorName = resolveType(GameInfo.Governors, governorType);
		local promotion, promotionName = resolveType(
			GameInfo.GovernorPromotions, promotionType);
		if governor == nil then return false, "unknown_" .. governorType; end
		if promotion == nil then return false, "unknown_" .. promotionType; end
		local governors = try(function() return player:GetGovernors(); end);
		if governors == nil then return false, "no_governors"; end
		if not try(function()
			return governors:CanEarnPromotion(governor.Hash, promotion.Hash);
		end, false) then
			return false, "governor_promotion_unavailable";
		end
		local params = {};
		params[PlayerOperations.PARAM_GOVERNOR_TYPE] = governor.Index;
		params[PlayerOperations.PARAM_GOVERNOR_PROMOTION_TYPE] = promotion.Index;
		local ok = pcall(function()
			UI.RequestPlayerOperation(pid, PlayerOperations.PROMOTE_GOVERNOR, params);
		end);
		return ok, ok and (governorName .. ":" .. promotionName)
			or "governor_promote_throw";
	end

	-- Claim a Great Person with banked points, or buy one outright with gold or
	-- faith. `verb` names the class (`GREAT_PERSON_CLASS_*`); WHICH individual
	-- that class offers only the live timeline knows, so it is resolved here.
	-- The shipped GreatPeoplePopup.lua is the reference for every call below:
	-- the operation takes the timeline entry's `Individual` id verbatim (not a
	-- GameInfo Index or Hash — the governor path's crash class does not apply).
	if kind == "gp_recruit" or kind == "gp_patronize"
			or kind == "gp_patronize_faith" then
		local greatPeople = try(function() return Game.GetGreatPeople(); end);
		if greatPeople == nil then return false, "no_great_people"; end
		local timeline = try(function() return greatPeople:GetTimeline(); end);
		if timeline == nil then return false, "gp_no_timeline"; end
		for _, entry in ipairs(timeline) do
			if entry.Individual ~= nil and entry.Claimant == nil then
				local info = try(function()
					return GameInfo.GreatPersonIndividuals[entry.Individual];
				end);
				if info ~= nil and info.GreatPersonClassType == verb then
					local resolved = info.GreatPersonIndividualType or verb;
					local params = {};
					params[PlayerOperations.PARAM_GREAT_PERSON_INDIVIDUAL_TYPE] =
						entry.Individual;
					if kind == "gp_recruit" then
						if not try(function()
							return greatPeople:CanRecruitPerson(pid, entry.Individual);
						end, false) then
							return false, "gp_cannot_recruit";
						end
						local ok = pcall(function()
							UI.RequestPlayerOperation(
								pid, PlayerOperations.RECRUIT_GREAT_PERSON, params);
						end);
						return ok, ok and resolved or "gp_recruit_throw";
					end
					local yield = kind == "gp_patronize_faith"
						and YieldTypes.FAITH or YieldTypes.GOLD;
					if not try(function()
						return greatPeople:CanPatronizePerson(
							pid, entry.Individual, yield);
					end, false) then
						return false, "gp_cannot_patronize";
					end
					params[PlayerOperations.PARAM_YIELD_TYPE] = yield;
					local ok = pcall(function()
						UI.RequestPlayerOperation(
							pid, PlayerOperations.PATRONIZE_GREAT_PERSON, params);
					end);
					return ok, ok and resolved or "gp_patronize_throw";
				end
			end
		end
		return false, "gp_class_not_offered";
	end

	-- A city's ranged strike. `subject` is the Firaxis city id, x/y the target
	-- plot in offset coordinates. WorldInput.lua:2545 is the reference, and it
	-- holds a trap worth naming: the CITY command takes the UNIT operation's
	-- parameter keys (`UnitOperationTypes.PARAM_X/Y`), and must be asked with
	-- `CanStartCommand` first — an unasked request fails silently.
	if kind == "city_strike" then
		local city = try(function() return CityManager.GetCity(pid, subject); end);
		if city == nil then return false, "city_strike_city_missing"; end
		if x == nil or y == nil then return false, "city_strike_no_target"; end
		local params = {};
		params[UnitOperationTypes.PARAM_X] = x;
		params[UnitOperationTypes.PARAM_Y] = y;
		if not try(function()
			return CityManager.CanStartCommand(
				city, CityCommandTypes.RANGE_ATTACK, params);
		end, false) then
			return false, "city_strike_refused";
		end
		local ok = pcall(function()
			CityManager.RequestCommand(city, CityCommandTypes.RANGE_ATTACK, params);
		end);
		return ok, ok and "CITY_STRIKE" or "city_strike_throw";
	end

	-- The encampment's strike, same shape as the city's: `subject` is the
	-- OWNING city's Firaxis id, and the command goes to the district OBJECT —
	-- WorldInput.lua:2626 hands `CityManager.CanStartCommand` the selected
	-- district with the same UNIT-keyed x/y params the city path uses. The
	-- district is found by walking the city's districts for the encampment row
	-- Index, the established pattern from the repair probe above.
	if kind == "encampment_strike" then
		local city = try(function() return CityManager.GetCity(pid, subject); end);
		if city == nil then return false, "encampment_strike_city_missing"; end
		if x == nil or y == nil then return false, "encampment_strike_no_target"; end
		local row = try(function() return GameInfo.Districts["DISTRICT_ENCAMPMENT"]; end);
		if row == nil then return false, "encampment_strike_no_district_row"; end
		local encampment = nil;
		try(function()
			for _, d in city:GetDistricts():Members() do
				if d:GetType() == row.Index then encampment = d; end
			end
		end);
		if encampment == nil then return false, "encampment_strike_no_encampment"; end
		local params = {};
		params[UnitOperationTypes.PARAM_X] = x;
		params[UnitOperationTypes.PARAM_Y] = y;
		if not try(function()
			return CityManager.CanStartCommand(
				encampment, CityCommandTypes.RANGE_ATTACK, params);
		end, false) then
			return false, "encampment_strike_refused";
		end
		local ok = pcall(function()
			CityManager.RequestCommand(
				encampment, CityCommandTypes.RANGE_ATTACK, params);
		end);
		return ok, ok and "ENCAMPMENT_STRIKE" or "encampment_strike_throw";
	end

	if kind == "war" then
		local diplomacy = try(function() return player:GetDiplomacy(); end);
		if diplomacy == nil then return false, "no_diplomacy"; end
		-- ★★★★★ AN UNMAPPED TARGET IS NOT A REFUSED WAR, AND IT USED TO LOOK LIKE ONE.
		--
		-- `subject` is `tonumber(row.subject) or -1`, and the bridge yields no
		-- subject when CIVVIS names a rival seat it has no exported rival for --
		-- CIVVIS's game has four majors while `state.rivals` carries only the ones
		-- actually met, so `rivals.get(player - 1)` can be None. That reaches here
		-- as -1, `CanDeclareWarOn(-1)` answers false, and the run records
		-- `cannot_declare` -- indistinguishable from Civilization VI refusing a
		-- legitimate declaration. Domination is the only route to a win on Settler,
		-- so the difference between "the game said no" and "we asked about nobody"
		-- is the difference between a diplomacy problem and a bridge problem.
		if subject < 0 then
			emit("war_unmapped", { turn = turn, subject = subject });
			return false, "war_target_unmapped";
		end
		if try(function() return diplomacy:IsAtWarWith(subject); end, false) then
			return false, "already_at_war";
		end
		if not try(function() return diplomacy:CanDeclareWarOn(subject); end, true) then
			-- ★★★★★ SAY WHY. Domination is the only route to a win here, and on run
			-- civvis-20260731T144251Z this refusal fired on 38 turns between t120 and
			-- t176 -- every other turn for a third of the game -- while `at_war` was
			-- false for both rivals. All the event stream carried was the word
			-- `cannot_declare`, so there was no way to tell a bad target id from a
			-- treaty, and the one decision that decides the game was undiagnosable.
			--
			-- `try` returns the call's own result, so reaching here means
			-- `CanDeclareWarOn` really answered false rather than throwing.
			emit("war_refused", {
				turn = turn,
				target = subject,
				at_war = try(function() return diplomacy:IsAtWarWith(subject); end, nil),
				has_met = try(function() return player:HasMet(subject); end, nil),
				alive = try(function() return Players[subject]:IsAlive(); end, nil),
				major = try(function() return Players[subject]:IsMajor(); end, nil),
				can_change = try(function()
					return diplomacy:CanChangeDiplomaticState(subject);
				end, nil),
			});
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

	-- ★★★★★ PEACE, WHICH NO CODE COULD EVER MAKE. CIVVIS emitted MakePeace on
	-- 93 turns of run civvis-20260801T221459Z — every turn from t118 to the end
	-- — and there was no arm for it anywhere, so the seat begged in why.log
	-- while the harness fought a war it had already lost. A war that cannot be
	-- exited turns every bad matchup into a death sentence.
	--
	-- Two shipped shapes, copied not guessed:
	-- - a MINOR takes the plain operation — CityStates.lua:818
	--   (`DIPLOMACY_MAKE_PEACE` with PARAM_PLAYER_ONE/TWO);
	-- - a MAJOR takes the deal: DiplomacyActionView.lua:434 CHOICE_MAKE_PEACE —
	--   a locked MAKE_PEACE agreement in the outgoing working deal, validated,
	--   then submitted with `DealProposalAction.PROPOSED`. The rival answers on
	--   its own turn; acceptance
	--   shows up as `at_war` dropping in a later export, and nothing here may
	--   claim more than "submitted".
	--
	-- ⚠ Re-asking every turn would rebuild the working deal against a rival who
	-- just said no. One ask per target per PeaceRetryTurns (default 5) turns.
	if kind == "peace" then
		local diplomacy = try(function() return player:GetDiplomacy(); end);
		if diplomacy == nil then return false, "no_diplomacy"; end
		if subject < 0 then
			emit("peace_unmapped", { turn = turn, subject = subject });
			return false, "peace_target_unmapped";
		end
		if not try(function() return diplomacy:IsAtWarWith(subject); end, false) then
			return false, "peace_not_at_war";
		end
		local asked = peaceAsked[subject];
		if asked ~= nil and (turn - asked) < (cfg.PeaceRetryTurns or 5) then
			return false, "peace_cooldown";
		end
		local major = try(function() return Players[subject]:IsMajor(); end, true);
		local concession = 0;
		local ok, submitted, reason;
		if major then
			local ran;
			ran, submitted, concession, reason = pcall(submitMajorPeaceDeal, subject, asked);
			if not ran then
				submitted, concession, reason = false, 0, "throw";
			end
			ok = submitted;
		else
			ok = pcall(function()
				local params = {};
				params[PlayerOperations.PARAM_PLAYER_ONE] = pid;
				params[PlayerOperations.PARAM_PLAYER_TWO] = subject;
				UI.RequestPlayerOperation(pid, PlayerOperations.DIPLOMACY_MAKE_PEACE, params);
			end);
			submitted = ok;
			reason = ok and "submitted" or "throw";
		end
		if not ok then concession = 0; end
		-- A pending offer is already inside the engine.  Retrying it on every
		-- turn cannot improve its chance and only muddies the action ledger.
		if ok or reason == "pending" then peaceAsked[subject] = turn; end
		emit("peace_request", {
			turn = turn, target = subject,
			major = major and true or false, concession = concession,
			submitted = submitted and true or false, reason = reason,
			threw = reason == "throw",
		});
		return ok, ok and "peace_submitted" or reason;
	end

	-- Delegations and embassies: diplomatic visibility and a relationship
	-- modifier for pocket change. `verb` is the session name
	-- (DIPLOMATIC_DELEGATION or RESIDENT_EMBASSY) handed to DiplomacyManager
	-- verbatim — DiplomacyActionView.lua:470 is the reference. The rival's
	-- answer arrives as a leader-scene popup the harness's clearers already
	-- close, the same accepted risk as the peace arm above, so one ask per
	-- target per cooldown window and nothing here may claim more than "asked".
	if kind == "delegation" then
		local diplomacy = try(function() return player:GetDiplomacy(); end);
		if diplomacy == nil then return false, "no_diplomacy"; end
		if subject < 0 then
			return false, "delegation_target_unmapped";
		end
		if try(function() return diplomacy:IsAtWarWith(subject); end, false) then
			return false, "delegation_at_war";
		end
		local present;
		if verb == "RESIDENT_EMBASSY" then
			present = try(function() return diplomacy:HasEmbassyAt(subject); end, false);
		else
			present = try(function() return diplomacy:HasDelegationAt(subject); end, false);
		end
		if present then return false, "delegation_already_present"; end
		-- An unaffordable session is a leader scene opened for a guaranteed
		-- no; the cost read is the shipped GetGoldCost helper's.
		local cost = try(function()
			return diplomacy:GetDiplomaticActionCost("DIPLOACTION_" .. verb);
		end, 0) or 0;
		local balance = try(function()
			return player:GetTreasury():GetGoldBalance();
		end, 0) or 0;
		if balance < cost then return false, "delegation_unaffordable"; end
		local key = verb .. subject;
		local asked = peaceAsked[key];
		if asked ~= nil and (turn - asked) < (cfg.PeaceRetryTurns or 5) then
			return false, "delegation_cooldown";
		end
		local ok = pcall(function()
			DiplomacyManager.RequestSession(pid, subject, verb);
		end);
		if ok then peaceAsked[key] = turn; end
		return ok, ok and "delegation_asked" or "throw";
	end

	-- ★★★★★ AID REQUEST FINISHER. Firaxis exposes two score routes for Aid
	-- Requests: a completed `PROJECT_SEND_AID` gives 200, and every Gold gift
	-- to the emergency target gives one. The Rust side sends this arm only when
	-- the latter is the exact bounded amount that would take the lead before
	-- the event closes. We still prove every diplomacy precondition against the
	-- host: a stale mirrored target, a war, a city-state, or another working
	-- trade may never become an unguarded deal.
	--
	-- Unlike `sell` and `buy`, this must use `PROPOSED`, not `EQUALIZE`: there
	-- is no price to negotiate and a foreign counter-offer would not be the
	-- specific one-way gift the emergency listener scores. `aid_gift_request`
	-- records a submission, never an imagined acceptance or victory point.
	if kind == "aid_gift" then
		if verb ~= "EMERGENCY_SEND_AID" and verb ~= "EMERGENCY_SEND_MILITARY_AID" then
			return false, "aid_gift_unknown_emergency";
		end
		if subject < 0 or subject == pid then return false, "aid_gift_target_unmapped"; end
		local amount = math.max(0, math.floor(x or 0));
		if amount <= 0 then return false, "aid_gift_no_amount"; end
		local diplomacy = try(function() return player:GetDiplomacy(); end);
		if diplomacy == nil then return false, "no_diplomacy"; end
		if not try(function() return diplomacy:HasMet(subject); end, false) then
			return false, "aid_gift_not_met";
		end
		if try(function() return diplomacy:IsAtWarWith(subject); end, false) then
			return false, "aid_gift_at_war";
		end
		if not try(function() return Players[subject]:IsMajor(); end, false) then
			return false, "aid_gift_not_major";
		end
		local trade = CivvisTrade;
		if trade.pending[subject] ~= nil then
			return false, "aid_gift_trade_pending";
		end
		local key = "aid_gift:" .. verb .. ":" .. subject;
		local asked = peaceAsked[key];
		if asked ~= nil and (turn - asked) < (cfg.AidGiftRetryTurns or 2) then
			return false, "aid_gift_cooldown";
		end
		local ran, submitted, paid, reason = pcall(submitAidGift, subject, amount);
		if not ran then
			submitted, paid, reason = false, 0, "throw";
		end
		-- A pending normal offer is already in the host. Keep it from being
		-- rebuilt on the next frame; a declined one can retry while the small
		-- finish window still exists.
		if submitted or reason == "pending" then peaceAsked[key] = turn; end
		emit("aid_gift_request", {
			turn = turn, target = subject, emergency = verb, amount = amount,
			paid = paid, submitted = submitted and true or false,
			reason = reason, threw = reason == "throw",
		});
		return submitted, submitted and "aid_gift_submitted" or reason;
	end

	-- ★★★★★ SURPLUS SOLD FOR GOLD, AT THE RIVAL'S OWN PRICE. `verb` is what
	-- CIVVIS lets go — `RESOURCE_DYES=1,FAVOR=10`, Firaxis's own resource type
	-- names, favor as a lump — and `x` is the gold-equivalent floor (lump plus
	-- 25× per-turn) below which the answer is declined. Built the way the
	-- shipped deal screen builds it (DiplomacyDealView_Expansion2.lua
	-- `OnClickAvailableResource` / `OnClickAvailableOneTimeFavor`): a resource
	-- item carries the type from `GetPossibleDealItems`, thirty turns for a
	-- luxury and a lump for a stockpiled (`Accumulate`) strategic, clipped to
	-- the engine's own `GetMaxAmount`; favor is a lump clipped the same way.
	-- Then EQUALIZE, not PROPOSED: the rival fills in its side and
	-- `CivvisOnIncomingDeal` above closes at or above the floor. No session is
	-- opened — the deal screen's only unattended exit is refusal.
	--
	-- One ask per rival at a time, one ask per rival per `TradeRetryTurns`;
	-- an ask the engine never answered is dropped after `TradeResponseTurns`
	-- so it cannot hold the working deal against the peace arm forever.
	-- Nothing here may claim more than "asked": the close is the handler's,
	-- and the enacted trade shows in the next export as gold up and the
	-- stock or favor down.
	if kind == "sell" then
		if subject < 0 then return false, "sell_target_unmapped"; end
		local diplomacy = try(function() return player:GetDiplomacy(); end);
		if diplomacy == nil then return false, "no_diplomacy"; end
		if not try(function() return diplomacy:HasMet(subject); end, false) then
			return false, "sell_not_met";
		end
		if try(function() return diplomacy:IsAtWarWith(subject); end, false) then
			return false, "sell_at_war";
		end
		if not try(function() return Players[subject]:IsMajor(); end, false) then
			return false, "sell_not_major";
		end
		local trade = CivvisTrade;
		local pending = trade.pending[subject];
		if pending ~= nil then
			if (turn - (pending.turn or turn)) < (cfg.TradeResponseTurns or 2) then
				return false, "sell_pending";
			end
			trade.pending[subject] = nil;
			pcall(function() DealManager.ClearWorkingDeal(DealDirection.OUTGOING, pid, subject); end);
			emit("deal_expired", { turn = turn, target = subject, asked_turn = pending.turn });
		end
		if try(function() return DealManager.HasPendingDeal(pid, subject); end, false) then
			return false, "sell_host_pending";
		end
		local asked = trade.asked[subject];
		if asked ~= nil and (turn - asked) < (cfg.TradeRetryTurns or 3) then
			return false, "sell_cooldown";
		end
		local wanted = {};
		for name, amount in string.gmatch(verb, "([%w_]+)=(%d+)") do
			wanted[#wanted + 1] = { name = name, amount = tonumber(amount) or 0 };
		end
		if #wanted == 0 then return false, "sell_no_items"; end
		local floor = math.max(0, math.floor(x or 0));
		local ran, submitted, reason, gave, gaveText = pcall(function()
			DealManager.ClearWorkingDeal(DealDirection.OUTGOING, pid, subject);
			local deal = DealManager.GetWorkingDeal(DealDirection.OUTGOING, pid, subject);
			if deal == nil then return false, "no_working_deal", {}, ""; end
			local possible = try(function()
				return DealManager.GetPossibleDealItems(pid, subject, DealItemTypes.RESOURCES, deal);
			end, nil) or {};
			local gave, text = {}, {};
			for _, want in ipairs(wanted) do
				if want.name == "FAVOR" then
					-- Favor is a Gathering Storm item; a ruleset without it has
					-- no `DealItemTypes.FAVOR` and simply nothing to sell here.
					local item = DealItemTypes.FAVOR ~= nil
						and deal:AddItemOfType(DealItemTypes.FAVOR, pid) or nil;
					if item ~= nil then
						item:SetDuration(0);
						local cap = try(function() return item:GetMaxAmount(); end, nil);
						local amount = want.amount;
						if cap ~= nil and cap < amount then amount = cap; end
						local set = amount > 0 and pcall(function() item:SetAmount(amount); end);
						if set and try(function() return item:IsValid(); end, true) then
							gave["FAVOR"] = amount;
							text[#text + 1] = "FAVOR=" .. tostring(amount);
						else
							pcall(function() deal:RemoveItemByID(item:GetID()); end);
						end
					end
				elseif string.find(want.name, "^GREATWORK_") ~= nil then
					-- A placed Great Work, sold to seat the idle person its
					-- departure makes room for (see `append_work_sale_order`
					-- on the CIVVIS side). The item contract is the shipped
					-- screen's own click, `DiplomacyDealView.lua`
					-- `OnClickAvailableGreatWork`: the possible-items entry
					-- carries the work INSTANCE as `ForType` and its
					-- `GameInfo.GreatWorks` row as `ForTypeDescriptionID`;
					-- the deal item takes SubType(description) THEN
					-- ValueType(instance), no amount and no duration — a
					-- work's presence is its quantity.
					if try(function()
						return GameInfo.GreatWorks[want.name];
					end, nil) == nil then
						return false, "unknown_great_work:" .. want.name, {}, "";
					end
					local possibleWorks = try(function()
						return DealManager.GetPossibleDealItems(
							pid, subject, DealItemTypes.GREATWORK, deal);
					end, nil) or {};
					local forType, descId = nil, nil;
					for _, entry in ipairs(possibleWorks) do
						local desc = try(function()
							return GameInfo.GreatWorks[entry.ForTypeDescriptionID];
						end, nil);
						if desc ~= nil and desc.GreatWorkType == want.name
								and entry.IsValid ~= false then
							forType, descId = entry.ForType, entry.ForTypeDescriptionID;
						end
					end
					if forType ~= nil then
						local item = deal:AddItemOfType(DealItemTypes.GREATWORK, pid);
						if item ~= nil then
							item:SetSubType(descId);
							item:SetValueType(forType);
							if try(function() return item:IsValid(); end, true) then
								gave["GREATWORK:" .. tostring(forType)] = 1;
								text[#text + 1] = want.name .. "=1";
							else
								pcall(function() deal:RemoveItemByID(item:GetID()); end);
							end
						end
					end
				else
					local row = try(function() return GameInfo.Resources[want.name]; end, nil);
					if row == nil then return false, "unknown_resource:" .. want.name, {}, ""; end
					local forType = nil;
					for _, entry in ipairs(possible) do
						local desc = try(function() return GameInfo.Resources[entry.ForType]; end, nil);
						if desc ~= nil and desc.ResourceType == want.name and entry.IsValid ~= false
								and (entry.MaxAmount or 0) > 0 then
							forType = entry.ForType;
						end
					end
					if forType ~= nil then
						local consumption = try(function()
							return GameInfo.Resource_Consumption[want.name];
						end, nil);
						local lump = consumption ~= nil
							and (consumption.Accumulate == true or consumption.Accumulate == 1);
						local item = deal:AddItemOfType(DealItemTypes.RESOURCES, pid);
						if item ~= nil then
							item:SetValueType(forType);
							item:SetDuration(lump and 0 or 30);
							local cap = try(function() return item:GetMaxAmount(); end, nil);
							local amount = want.amount;
							if cap ~= nil and cap > 0 and cap < amount then amount = cap; end
							local set = amount > 0 and pcall(function() item:SetAmount(amount); end);
							if set and try(function() return item:IsValid(); end, true) then
								gave["RESOURCES:" .. tostring(forType)] = amount;
								text[#text + 1] = want.name .. "=" .. tostring(amount)
									.. "x" .. (lump and "0" or "30");
							else
								pcall(function() deal:RemoveItemByID(item:GetID()); end);
							end
						end
					end
				end
			end
			if next(gave) == nil then return false, "nothing_tradeable", gave, ""; end
			deal:Validate();
			if not deal:IsValid() then return false, "invalid_deal", gave, table.concat(text, ","); end
			-- Registered BEFORE the ask goes out: should the engine answer
			-- synchronously, from inside this very call, the handler must
			-- already know what was offered and at what floor.
			trade.pending[subject] = {
				turn = turn, floor = floor, gave = gave, verb = table.concat(text, ","),
			};
			DealManager.SendWorkingDeal(DealProposalAction.EQUALIZE, pid, subject);
			return true, "asked", gave, table.concat(text, ",");
		end);
		if not ran then
			submitted, reason, gave, gaveText = false, "throw", {}, "";
		end
		if submitted then
			trade.asked[subject] = turn;
		elseif trade.pending[subject] ~= nil and trade.pending[subject].turn == turn then
			-- The ask itself threw after registering; nothing is in flight.
			trade.pending[subject] = nil;
		end
		if not submitted and (reason == "nothing_tradeable" or reason == "invalid_deal") then
			-- The engine has nothing to sell here right now; do not re-ask
			-- every turn for the same answer.
			trade.asked[subject] = turn;
			pcall(function() DealManager.ClearWorkingDeal(DealDirection.OUTGOING, pid, subject); end);
		end
		emit("deal_offer", {
			turn = turn, target = subject, verb = verb, floor = floor,
			submitted = submitted and true or false, reason = reason,
			gave = gaveText, threw = reason == "throw",
		});
		return submitted, submitted and "sell_asked" or reason;
	end

	-- ★★★★★ PASSAGE BOUGHT AT THE RIVAL'S OWN PRICE. The mirror image of the
	-- sale above, for the one purchase with a measured case: Open Borders,
	-- the peacetime key to a sealed border (one live run held a scout against
	-- Kongo's invisible border for 74 turns and explored 8.3% of the map).
	-- `verb` names the agreement — OPEN_BORDERS is the only one this arm
	-- buys — and `x` is the gold-equivalent ceiling (lump plus 25× per-turn)
	-- ABOVE which the answer is declined. Built the way the shipped screen
	-- adds an agreement (DiplomacyDealView.lua `OnClickAvailableAgreement`):
	-- one AGREEMENTS item FROM the rival, subtype OPEN_BORDERS, the standard
	-- thirty turns; then EQUALIZE, and `CivvisOnIncomingDeal` closes only when
	-- the rival's own balance asks gold at or under the ceiling. Same
	-- cooldowns and same one-working-deal-per-rival rule as the sale lane —
	-- `CivvisTrade.pending`/`asked` are shared deliberately, because the host
	-- holds ONE outgoing working deal per rival and a second ask would clear
	-- the first mid-flight.
	if kind == "buy" then
		if verb ~= "OPEN_BORDERS" then return false, "buy_unknown_item"; end
		if subject < 0 then return false, "buy_target_unmapped"; end
		local diplomacy = try(function() return player:GetDiplomacy(); end);
		if diplomacy == nil then return false, "no_diplomacy"; end
		if not try(function() return diplomacy:HasMet(subject); end, false) then
			return false, "buy_not_met";
		end
		if try(function() return diplomacy:IsAtWarWith(subject); end, false) then
			return false, "buy_at_war";
		end
		if not try(function() return Players[subject]:IsMajor(); end, false) then
			return false, "buy_not_major";
		end
		if try(function() return diplomacy:HasOpenBordersFrom(subject); end, false) then
			return false, "buy_already_open";
		end
		local trade = CivvisTrade;
		local pending = trade.pending[subject];
		if pending ~= nil then
			if (turn - (pending.turn or turn)) < (cfg.TradeResponseTurns or 2) then
				return false, "buy_pending";
			end
			trade.pending[subject] = nil;
			pcall(function() DealManager.ClearWorkingDeal(DealDirection.OUTGOING, pid, subject); end);
			emit("deal_expired", { turn = turn, target = subject, asked_turn = pending.turn });
		end
		if try(function() return DealManager.HasPendingDeal(pid, subject); end, false) then
			return false, "buy_host_pending";
		end
		local asked = trade.asked[subject];
		if asked ~= nil and (turn - asked) < (cfg.TradeRetryTurns or 3) then
			return false, "buy_cooldown";
		end
		local ceiling = math.max(0, math.floor(x or 0));
		if ceiling <= 0 then return false, "buy_no_ceiling"; end
		local ran, submitted, reason = pcall(function()
			DealManager.ClearWorkingDeal(DealDirection.OUTGOING, pid, subject);
			local deal = DealManager.GetWorkingDeal(DealDirection.OUTGOING, pid, subject);
			if deal == nil then return false, "no_working_deal"; end
			-- The agreement rides FROM the rival: they grant, we pay. A
			-- ruleset without the agreement type has nothing to buy here.
			if DealAgreementTypes == nil or DealAgreementTypes.OPEN_BORDERS == nil then
				return false, "no_agreement_type";
			end
			local item = deal:AddItemOfType(DealItemTypes.AGREEMENTS, subject);
			if item == nil then return false, "no_agreement_item"; end
			item:SetSubType(DealAgreementTypes.OPEN_BORDERS);
			item:SetDuration(30);
			if not try(function() return item:IsValid(); end, true) then
				pcall(function() deal:RemoveItemByID(item:GetID()); end);
				return false, "agreement_invalid";
			end
			deal:Validate();
			if not deal:IsValid() then return false, "invalid_deal"; end
			-- Registered BEFORE the ask goes out — see the sale arm above.
			trade.pending[subject] = {
				turn = turn, ceiling = ceiling, direction = "buy", verb = "OPEN_BORDERS",
			};
			DealManager.SendWorkingDeal(DealProposalAction.EQUALIZE, pid, subject);
			return true, "asked";
		end);
		if not ran then
			submitted, reason = false, "throw";
		end
		if submitted then
			trade.asked[subject] = turn;
		elseif trade.pending[subject] ~= nil and trade.pending[subject].turn == turn
				and trade.pending[subject].direction == "buy" then
			-- The ask itself threw after registering; nothing is in flight.
			trade.pending[subject] = nil;
		end
		if not submitted and (reason == "no_agreement_type" or reason == "no_agreement_item"
				or reason == "agreement_invalid" or reason == "invalid_deal") then
			-- The engine will not sell passage here right now — usually a
			-- missing Early Empire on one side; do not re-ask every turn for
			-- the same answer.
			trade.asked[subject] = turn;
			pcall(function() DealManager.ClearWorkingDeal(DealDirection.OUTGOING, pid, subject); end);
		end
		emit("deal_offer", {
			turn = turn, target = subject, verb = "OPEN_BORDERS", direction = "buy",
			ceiling = ceiling,
			submitted = submitted and true or false, reason = reason,
			threw = reason == "throw",
		});
		return submitted, submitted and "buy_asked" or reason;
	end

	-- ★★★★★ CIVVIS'S OWN ENVOY, PLACED. One order = one influence token on one
	-- met city-state, decided on the reconstructed board by `advanced_envoys`
	-- (type-aware, suzerainty-priced, denial-aware) now that `envoys_free`
	-- crosses the export into the mirror. Until this arm existed the seat
	-- finished every Settler game holding 40–70 unspent envoys with no
	-- suzerainty — the largest measured headroom in the project (an oracle
	-- suzerainty wins 56.7% against 22.7%) forfeited for free.
	--
	-- ⚠⚠ THIS IS NOT `chooseEnvoy`. That lane decides in Lua and stays behind
	-- `cfg.EnvoyEnabled` (off) as the isolation experiment its comment asks
	-- for; running both would have two deciders bidding the same purse. Here
	-- the mod places what it is told and reports what happened, nothing more.
	--
	-- ⚠⚠ A FRESH `GetInfluence()` HANDLE PER ORDER, READ THEN DROPPED. The one
	-- concrete defect ever pinned on the crash that took the old lane out of
	-- deployment was a gameplay sub-object pointer held across the
	-- `UI.RequestPlayerOperation` calls that rewrite it. Every read here goes
	-- through a handle fetched inside this order and nothing is written
	-- through it after the operation is issued. The prompt-clearing write
	-- (`SetGivingTokensConsidered`) is deliberately NOT made: with the tokens
	-- spent the `GIVE_INFLUENCE_TOKEN` blocker is not raised, and when CIVVIS
	-- keeps one back the known-stable skip in SOFT_BLOCKERS still stands.
	--
	-- Every accessor is the shipped `CityStates.lua`'s: `GetTokensToGive`,
	-- `CanGiveInfluence`, `CanGiveTokensToPlayer`, and one
	-- `GIVE_INFLUENCE_TOKEN` request per token with `PARAM_PLAYER_ONE`.
	if kind == "envoy" then
		if subject < 0 then return false, "envoy_target_unmapped"; end
		local giveOp = try(function() return PlayerOperations.GIVE_INFLUENCE_TOKEN; end);
		local oneParam = try(function() return PlayerOperations.PARAM_PLAYER_ONE; end);
		if giveOp == nil or oneParam == nil then return false, "envoy_no_operation"; end
		local influence = try(function() return player:GetInfluence(); end);
		if influence == nil then return false, "envoy_no_influence"; end
		local held = try(function() return influence:GetTokensToGive(); end, 0) or 0;
		if held < 1 then return false, "envoy_none_held"; end
		if not try(function() return influence:CanGiveInfluence(); end, false) then
			return false, "envoy_cannot_give";
		end
		if not try(function() return influence:CanGiveTokensToPlayer(subject); end, false) then
			return false, "envoy_refused_" .. tostring(subject);
		end
		local params = {};
		params[oneParam] = subject;
		local ok = pcall(function()
			UI.RequestPlayerOperation(pid, giveOp, params);
		end);
		-- ⚠ NO SAME-FRAME "AFTER" COUNT. The first version of this event read
		-- `GetTokensToGive()` again right after the request and reported it as
		-- `after`; the core applies the operation later in the frame, so it read
		-- equal to `held` on every one of the first 23 live placements
		-- (civvis-20260816T142911Z t49–t196) while the tokens WERE landing —
		-- `envoys_free` 0 in every export and 28 envoys standing on two
		-- city-states by t200. A receiving-side reading that cannot tell
		-- "applied later" from "ignored" is a false alarm waiting to fire, and
		-- it fired. The receiving side is the NEXT export: `envoys_free` there
		-- has fallen by the number placed and `minors[].envoys` has risen by
		-- it; the bridge prints both every turn (`envoys unspent N placed M`).
		if ok then
			emit("envoy", { turn = turn, target = subject, source = "civvis", held = held });
		end
		return ok, ok and "envoy_placed" or "throw";
	end

	-- ★★★★ CIVVIS'S OWN LEVY. `LevyMilitary` was the single most-skipped
	-- action of the pre-envoy bridge (44 a game) — moot while the seat held no
	-- suzerainty, live now that it places its envoys. Firaxis's own quote is
	-- the price and `CanLevyMilitary` the gate (cooldown, suzerainty, war
	-- state), exactly as `chooseEnvoy`'s levy scan reads them; one
	-- `LEVY_MILITARY` request per order, through a fresh handle, nothing
	-- written through it afterwards. The treasury check is the receiving
	-- side's, not the mirror's — a levy the plan priced from a partial view
	-- of the city-state's army is refused by name, not bought blind.
	if kind == "levy" then
		if subject < 0 then return false, "levy_target_unmapped"; end
		local levyOp = try(function() return PlayerOperations.LEVY_MILITARY; end);
		local oneParam = try(function() return PlayerOperations.PARAM_PLAYER_ONE; end);
		if levyOp == nil or oneParam == nil then return false, "levy_no_operation"; end
		local influence = try(function() return player:GetInfluence(); end);
		if influence == nil then return false, "levy_no_influence"; end
		if not try(function() return influence:CanLevyMilitary(subject); end, false) then
			return false, "levy_refused_" .. tostring(subject);
		end
		local cost = try(function() return influence:GetLevyMilitaryCost(subject); end, -1) or -1;
		local purse = try(function()
			return math.floor(player:GetTreasury():GetGoldBalance());
		end, 0) or 0;
		if cost < 0 or purse < cost then
			return false, "levy_unaffordable";
		end
		local params = {};
		params[oneParam] = subject;
		local ok = pcall(function()
			UI.RequestPlayerOperation(pid, levyOp, params);
		end);
		if ok then
			emit("levy", { turn = turn, target = subject, source = "civvis", cost = cost, purse = purse });
		end
		return ok, ok and "levy_bought" or "throw";
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
	if kind == "policy_deck" then
		local culture = try(function() return player:GetCulture(); end);
		if culture == nil then return false, "no_culture"; end
		local desired, seen = {}, {};
		for requested in string.gmatch(verb, "[^,]+") do
			local card, resolved = resolveType(GameInfo.Policies, requested);
			if card == nil then return false, "unknown_" .. requested; end
			if try(function() return culture:IsPolicyObsolete(card.Hash); end, false) then
				return false, "obsolete_" .. resolved;
			end
			if not try(function() return culture:IsPolicyUnlocked(card.Hash); end, false) then
				return false, "locked_" .. resolved;
			end
			if not seen[card.Index] then
				desired[#desired + 1] = card;
				seen[card.Index] = true;
			end
		end

		local slots = try(function() return culture:GetNumPolicySlots(); end, 0) or 0;
		if #desired > slots then return false, "policy_deck_too_large"; end
		-- ⚠⚠ NAME THE CARD AND THE SHAPE, because "does not fit" is not a
		-- diagnosis. `policy_deck_does_not_fit` fires on **601 of 2,068**
		-- policy-deck orders (29%) across the 08-04/08-05 runs, and every one
		-- of those refusals recorded nothing but that string — not the deck
		-- asked for, not the slots available, not which card had nowhere to go.
		--
		-- Two hypotheses were checked against recorded data before writing this
		-- and BOTH came back clean, which is why an instrument is the next step
		-- rather than a patch:
		--   * slot COUNT disagreeing with the host — 0 mismatches in 7,563
		--     turn records, every government
		--   * card slot TYPE disagreeing — 12 of 123 differ, but all 12 are
		--     `SLOT_GREAT_PERSON` cards CIVVIS calls `wildcard`, and the
		--     seating below already falls those back to a Wildcard slot
		-- So the cause is neither of the obvious two, and guessing a third is
		-- how #1107 got fixed in the wrong layer.
		local function deckShape()
			local want, have = {}, {};
			for _, c in ipairs(desired) do
				want[#want + 1] = tostring(c.PolicyType or "?")
					.. ":" .. tostring(c.GovernmentSlotType or "?");
			end
			for i = 0, slots - 1 do
				have[#have + 1] = tostring(try(function()
					local slotId = culture:GetSlotType(i);
					return GameInfo.GovernmentSlots[slotId].GovernmentSlotType;
				end, "?"));
			end
			return table.concat(want, ","), table.concat(have, ",");
		end
		local function noFit(card)
			local want, have = deckShape();
			emit("policy_deck_refused", {
				turn = turn,
				-- The card that had nowhere to go, which is the whole question.
				card = card ~= nil and tostring(card.PolicyType or "?") or nil,
				card_slot = card ~= nil and tostring(card.GovernmentSlotType or "?") or nil,
				wanted = want,
				slots = have,
				slot_count = slots,
			});
			-- A CIVVIS deck is a decision, but an impossible deck must not leave
			-- the game on the same civic-slot blocker forever.  Fall back to the
			-- same bounded, slot-aware filler used for a missing decision and name
			-- that loss of authority in telemetry.  The next board export gives
			-- CIVVIS a fresh chance to choose from the now-valid slate.
			local fallback = fillPolicies(player);
			emit("policy_deck_fallback", {
				turn = turn,
				card = card ~= nil and tostring(card.PolicyType or "?") or nil,
				fallback = fallback,
			});
			return fallback ~= nil, fallback or "policy_deck_does_not_fit";
		end
		local slotNames = {};
		for i = 0, slots - 1 do
			slotNames[i] = try(function()
				local slotId = culture:GetSlotType(i);
				return GameInfo.GovernmentSlots[slotId].GovernmentSlotType;
			end);
		end

		-- Seat constrained cards first. A typed card may fall back to a Wildcard
		-- slot, while a Wildcard-only card cannot fall back to a typed slot.
		local addList, used, pending = {}, {}, {};
		local function seat(card, slotType)
			for i = 0, slots - 1 do
				if not used[i] and slotNames[i] == slotType then
					addList[i] = card.Hash;
					used[i] = true;
					return true;
				end
			end
			return false;
		end
		for _, card in ipairs(desired) do
			if card.GovernmentSlotType == "SLOT_WILDCARD" then
				if not seat(card, "SLOT_WILDCARD") then
					return noFit(card);
				end
			else
				pending[#pending + 1] = card;
			end
		end
		local wildcard = {};
		for _, card in ipairs(pending) do
			if not seat(card, card.GovernmentSlotType) then
				wildcard[#wildcard + 1] = card;
			end
		end
		for _, card in ipairs(wildcard) do
			if not seat(card, "SLOT_WILDCARD") then
				return noFit(card);
			end
		end

		local clearList = {};
		for i = 0, slots - 1 do clearList[#clearList + 1] = i; end
		local ok = pcall(function()
			culture:RequestPolicyChanges(clearList, addList);
		end);
		return ok, ok and "policy_deck" or "throw";
	end
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
		-- ★★★★★ THE SLOT MUST BE CLEARED IN THE SAME REQUEST, EVEN WHEN EMPTY.
		-- Copied from the shipped GovernmentScreen.lua OnConfirmPolicies (:1560),
		-- which puts EVERY slot it writes into clearList first, with the comment
		-- "removals done first, otherwise swapping may fail ... the engine will
		-- think a policy is still active in its slot". This arm sent an empty
		-- clearList, and run civvis-20260801T221459Z (Netherlands) measured the
		-- result: from the Oligarchy switch (~t57) to t250 exactly ONE card
		-- stayed slotted while slots grew to 6 and 551 policy orders counted
		-- `applied` — accepted by pcall, discarded by the engine.
		local ok = pcall(function()
			culture:RequestPolicyChanges({ target }, addList);
		end);
		-- The policy layer had ZERO provenance events, so an accepted-but-
		-- discarded request was invisible for 140 turns. Name slot and card;
		-- the next turn's `state.policies` export is the verdict on whether it
		-- stuck (a same-tick read-back cannot be trusted — the request is
		-- processed by the game core, not synchronously by this context).
		emit("policy_request", {
			turn = turn, slot = target, policy = resolved,
			cleared = true, threw = not ok,
		});
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

	if kind == "religion" then
		local requested = {};
		for beliefType in string.gmatch(verb, "[^,]+") do
			requested[#requested + 1] = beliefType;
		end
		if #requested ~= 2 then return false, "religion_needs_two_beliefs"; end
		local follower, followerName = resolveType(GameInfo.Beliefs, requested[1]);
		local founder, founderName = resolveType(GameInfo.Beliefs, requested[2]);
		if follower == nil then return false, "unknown_" .. requested[1]; end
		if founder == nil then return false, "unknown_" .. requested[2]; end

		local playerReligion = try(function() return player:GetReligion(); end);
		if playerReligion == nil then return false, "no_religion_api"; end
		if try(function() return playerReligion:GetReligionTypeCreated(); end, -1) >= 0 then
			return false, "religion_already_founded";
		end
		if not try(function() return playerReligion:HasReligiousFoundingUnit(); end, false) then
			return false, "no_great_prophet";
		end
		local gameReligion = try(function() return Game.GetReligion(); end);
		if gameReligion == nil then return false, "no_game_religion"; end
		if try(function() return gameReligion:IsInSomeReligion(follower.Index); end, true) then
			return false, "taken_" .. followerName;
		end
		if try(function() return gameReligion:IsInSomeReligion(founder.Index); end, true) then
			return false, "taken_" .. founderName;
		end

		local used = {};
		for _, existing in ipairs(try(function() return gameReligion:GetReligions(); end, {}) or {}) do
			used[existing.Religion] = true;
		end
		local religion = nil;
		for row in GameInfo.Religions() do
			local isPantheon = row.Pantheon == true or row.Pantheon == 1;
			local needsName = row.RequiresCustomName == true or row.RequiresCustomName == 1;
			if not isPantheon and not needsName and not used[row.Index] then
				religion = row;
				break;
			end
		end
		if religion == nil then return false, "no_religion_type"; end

		local prophet = nil;
		for _, unit in player:GetUnits():Members() do
			local unitRow = GameInfo.Units[try(function() return unit:GetType(); end, -1)];
			if unitRow ~= nil and unitRow.UnitType == "UNIT_GREAT_PROPHET" then
				prophet = unit;
				break;
			end
		end
		if prophet == nil then return false, "no_great_prophet_unit"; end
		local foundOperation = GameInfo.UnitOperations["UNITOPERATION_FOUND_RELIGION"];
		if foundOperation == nil then return false, "no_found_religion_operation"; end
		local okCanOperate, canOperate = pcall(function()
			return UnitManager.CanStartOperation(prophet, foundOperation.Hash, nil, false,
				OperationResultsTypes.NO_TARGETS);
		end);
		if not (okCanOperate and canOperate == true) then
			return false, "cannot_found_religion_here";
		end

		-- Reproduce the full human path. UnitPanel starts the Prophet-specific
		-- operation that opens religion selection; ReligionScreen then founds the
		-- named religion and attaches the two selected beliefs.
		--
		-- ★★★★★ THE PLAYER OPERATION GOES FIRST, AND THAT ORDER IS THE WHOLE FIX.
		--
		-- This block used to request the UNIT operation first. Measured across the
		-- 24 completed live runs of 2026-08-07/08, that sequence has a single,
		-- perfectly repeatable outcome:
		--
		--     turn t-1   prophet 1   prophet_pending false   religion none
		--     turn t     prophet 1   prophet_pending TRUE    religion none   <- order
		--     turn t+1   prophet 0   prophet_pending false   religion NONE
		--
		-- The Great Prophet is CONSUMED and no religion is created. A religion
		-- order reached the host in 19 of 24 runs, every one of them reported
		-- `applied` with zero refusals, and a religion was founded in **0 of 24**.
		-- All four slots go to rivals in every game, a median 494 Faith banks with
		-- nothing to buy, and the religious victory lane -- which this controller
		-- wins 19 games in 50 in the headless evaluator -- is unreachable in live
		-- play by construction.
		--
		-- The comment this replaces already named the mechanism without drawing the
		-- conclusion: "omitting the first request creates the religion but leaves
		-- its Prophet occupying the Holy Site". The player operation is what FOUNDS;
		-- the unit operation only spends the Prophet. Requesting the spend first
		-- retires the founding unit before the founding it was needed for, and
		-- `HasReligiousFoundingUnit()` is false by the time the host processes it.
		--
		-- ⚠ EVERY ONE OF THESE FLAGS IS A `pcall` VERDICT -- "did not throw" -- and
		-- this file has been bitten by exactly that before. `ok` cannot mean the
		-- engine took it, because `UI.RequestPlayerOperation` is asynchronous and
		-- there is nothing to read back on this frame. `pendingReligionFounding`
		-- below is how the NEXT turn finds out, so a silent failure stops being
		-- indistinguishable from success.
		local found = {};
		found[PlayerOperations.PARAM_INSERT_MODE] = PlayerOperations.VALUE_EXCLUSIVE;
		found[PlayerOperations.PARAM_RELIGION_TYPE] = religion.Hash;
		local okFound = pcall(function()
			UI.RequestPlayerOperation(pid, PlayerOperations.FOUND_RELIGION, found);
		end);
		local function addBelief(row)
			local params = {};
			params[PlayerOperations.PARAM_BELIEF_TYPE] = row.Hash;
			params[PlayerOperations.PARAM_INSERT_MODE] = PlayerOperations.VALUE_EXCLUSIVE;
			return pcall(function()
				UI.RequestPlayerOperation(pid, PlayerOperations.ADD_BELIEF, params);
			end);
		end
		local okFollower = addBelief(follower);
		local okFounder = addBelief(founder);
		-- Spend the Prophet only after the founding has been asked for.
		local okOperation = pcall(function()
			UnitManager.RequestOperation(prophet, foundOperation.Hash);
		end);
		local ok = okFound and okFollower and okFounder and okOperation;
		if ok then
			pendingReligionFounding = {
				turn = Game.GetCurrentGameTurn(),
				religion = religion.ReligionType,
				follower = followerName,
				founder = founderName,
			};
		end
		return ok, ok and (religion.ReligionType .. ":" .. followerName .. ":" .. founderName)
			or "throw";
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

	if kind == "produce_next" then
		local city = liveCity(player, subject);
		if city == nil then return false, "no_city"; end
		verb = CIVVIS_PROJECT_TYPES[verb]
			or CIVVIS_GOVERNMENT_BUILDING_TYPES[verb]
			or verb;
		local row2, resolved = resolveType(GameInfo.Types, verb);
		if row2 == nil then return false, "unknown_" .. verb; end
		local cityId = tonumber(subject) or -1;
		civvisBuild[tostring(cityId) .. ":next"] = resolved;
		-- This is a durable handoff, not a build request.  The queue remains
		-- untouched until `driveProduction` sees the matching end-turn blocker.
		emit("build_hint", {
			turn = turn, city = cityId, item = resolved,
			production_turns = try(function()
				return city:GetBuildQueue():GetTurnsLeft();
			end, -1),
		});
		return true, "queued";
	end

	if kind == "produce" then
		local city = liveCity(player, subject);
		if city == nil then return false, "no_city"; end
		verb = CIVVIS_PROJECT_TYPES[verb]
			or CIVVIS_GOVERNMENT_BUILDING_TYPES[verb]
			or verb;
		-- ★★★★★ REMEMBER WHAT CIVVIS ASKED THIS CITY TO BUILD.
		--
		-- `ENDTURN_BLOCKING_PRODUCTION` fires when a city finishes something, which is
		-- usually AFTER CIVVIS has answered for the turn — and `driveProduction` then
		-- picks the next item ITSELF from the hand-written ladder. Measured once the
		-- residual counter was fixed: the ladder chose 76% of the build decisions on
		-- run civvis-20260731T075743Z and 44% on 070956Z, ten battering rams among
		-- them, on runs reported as 100% CIVVIS.
		--
		-- CIVVIS's own choice for that city is right here; keeping it means the
		-- fallback answers the prompt with a CIVVIS decision instead of its own.
		--
		-- ⚠ Recorded AFTER `resolveType` below, not here: `chooseProduction` looks the
		-- name up in `GameInfo.Types` and a raw verb that needed resolving would miss.
		-- ⚠ `GameInfo.Types`, NOT the per-kind tables. `buildParams` switches on
		-- `row.Kind`, and only the `Types` table carries that column — rows from
		-- `GameInfo.Units`/`Buildings`/`Districts` have no `Kind`, so `buildParams`
		-- returned nil and EVERY produce order was refused `no_params`. Measured on
		-- run civvis-20260730T111537Z: 9 refusals in 10 turns, all of them this.
		-- The built-in ladder reads `GameInfo.Types[name]` for exactly this reason.
		local row2, resolved = resolveType(GameInfo.Types, verb);
		if row2 == nil then
			-- See `probeDistrictRepair`: a repair ask names a project the game
			-- does not have; measure what the engine would offer, then refuse
			-- exactly as before.
			local wanted = string.match(tostring(verb), "^PROJECT_REPAIR_(.+)$");
			if wanted ~= nil and wanted ~= "OUTER_DEFENSES" then
				probeDistrictRepair(city, "DISTRICT_" .. wanted, verb, turn);
			end
			return false, "unknown_" .. verb;
		end
		verb = resolved;
		local cityId = tonumber(subject) or -1;
		civvisBuild[cityId] = resolved;
		-- A live city can go from healthy to lost between two CIVVIS boards.  If
		-- the engine says an unwalled city is already damaged or has a visible
		-- enemy in the neighbourhood, spend this queue on the wall immediately.
		-- Keep the original CIVVIS request in `civvisBuild`; the override is
		-- observable and does not pretend the model chose the emergency action.
		--
		-- ⚠⚠⚠ THERE WAS NO NEIGHBOURHOOD. The comment above says one thing and the
		-- test said `nearestEnemy ~= nil`, which is true whenever we are at war and
		-- can see ANY enemy unit anywhere on the map -- `cityWarThreat` walks every
		-- visible unit of every player we are at war with and bounds the distance
		-- by nothing at all.
		--
		-- Measured over the eight live runs of 2026-08-11, 160 overrides:
		--
		--     94%  fired with damage == 0
		--     70%  fired with the nearest enemy 5+ tiles away (median 6, max 14)
		--     66%  fired with BOTH -- no damage and no enemy within five tiles
		--
		-- and what they overrode was 46 Campus, 13 Library, 7 Enhance-Campus, 15
		-- Builders and 9 Settlers: science and expansion, which this project has
		-- measured to be its binding constraints, replaced by a wall it has also
		-- measured does not predict holding a city
		-- (`civ6-walls-do-not-predict-city-loss`).
		--
		-- So bound it to the claim the comment already makes. Damage is real
		-- evidence and still fires at any distance; a visible enemy has to be close
		-- enough to actually reach the city between two boards, which an enemy
		-- fourteen tiles away is not. `EmergencyWallRadius` is a knob so this is
		-- tunable and can be withheld -- set it very large to restore the old
		-- unbounded behaviour exactly.
		local atWar, nearestEnemy, damage, wallDamage, maxWallDamage =
			cityWarThreat(player, pid, city);
		local wallRadius = cfg.EmergencyWallRadius or 3;
		local emergencyWall = false;
		if resolved ~= "BUILDING_WALLS"
				and maxWallDamage ~= nil and maxWallDamage <= 0
				and ((damage ~= nil and damage > 0)
					or (nearestEnemy ~= nil and nearestEnemy <= wallRadius)) then
			local wall = GameInfo.Types["BUILDING_WALLS"];
			local wallCanOk, wallCan = false, false;
			if wall ~= nil then
				wallCanOk, wallCan = pcall(function()
					return city:GetBuildQueue():CanProduce(wall.Hash, false, true);
				end);
			end
			if wallCanOk and wallCan == true then
				row2 = wall;
				verb = "BUILDING_WALLS";
				emergencyWall = true;
				-- The radius in force goes in the record. Reading the old ledger
				-- meant knowing the gate was unbounded, which nothing stated;
				-- a threshold that is not in the event cannot be audited later.
				emit("emergency_wall_override", {
					turn = turn, city = cityId, requested = resolved,
					item = "BUILDING_WALLS", at_war = atWar,
					enemy_distance = nearestEnemy, damage = damage,
					wall_damage = wallDamage, max_wall_damage = maxWallDamage,
					radius = wallRadius,
				});
			end
		end
		local params = buildParams(row2, city, x, y);
		-- ⚠ The verb goes in the reason. `refusals` is aggregated by reason string, so
		-- a bare "no_params" collapses every distinct failure into one anonymous
		-- number -- which is exactly how 100 of them went unexplained.
		if params == nil then return false, "no_params_" .. tostring(verb); end
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
		-- Ask the same start-now predicate as Firaxis's production panel before
		-- crediting this order. A successful `pcall` only proves that Lua did not
		-- throw; the live Library loop showed that the engine can reject the build
		-- while the bridge reports it applied on every turn.
		local canOk, canStart, results = pcall(function()
			return city:GetBuildQueue():CanProduce(row2.Hash, false, true);
		end);
		if not canOk or canStart ~= true then
			local refused = refusedByCity[cityId];
			if refused == nil or refused.turn ~= turn then
				refused = { turn = turn };
				refusedByCity[cityId] = refused;
			end
			refused[verb] = true;
			emit("civvis_build_unplayable", {
				turn = turn,
				city = cityId,
				item = tostring(verb),
				reasons = productionFailureReasons(results),
			});
			return false, canOk and ("cannot_start_" .. verb) or "can_produce_throw";
		end
		local ok = pcall(function()
			CityManager.RequestOperation(city, CityOperationTypes.BUILD, params);
		end);
		return ok, ok and (emergencyWall and "BUILDING_WALLS" or verb) or "throw";
	end

	-- ★★★★ BUY. CIVVIS spends Gold or Faith and the seat sat on hundreds of it. The old
	-- tally reported 122 `Buy` and 224 `BuyBuilding` skips in one 81-turn stretch,
	-- including the since-fixed 2x diagnostic count. Worse than the waste,
	-- a purchase CIVVIS makes in its model and the bridge discards leaves it believing
	-- it owns a unit that does not exist — the same phantom that stopped it building
	-- settlers for a whole game.
	--
	-- Pattern copied from the shipped `ProductionPanel.lua`: a PURCHASE **command**,
	-- not a BUILD operation, with the item hash and the yield to pay from.
	-- The order kind carries that yield explicitly; hardcoding Gold discarded every
	-- Faith purchase even though Firaxis uses the same command for both currencies.
	-- ★★★★★ BUYING GROUND FOR A CITY.
	--
	-- `BuyPlot` had no arm at all: `civvis_orders` counted it in the `skipped` tally
	-- (25 across the runs of 2026-07-31) and the bridge threw every one away, so
	-- CIVVIS's cities only ever worked the tiles they happened to grow into. A
	-- treasury that ends a game unspent -- 1459 gold at t182 of run
	-- civvis-clean-20260731T191337Z -- is a treasury that bought no ground.
	--
	-- Pattern from the shipped `PlotInfo.lua`: a PURCHASE **command** carrying the
	-- plot flag and the plot's own X/Y, gated by `CanStartCommand`.
	--
	-- ⚠ ASK BEFORE CLAIMING, the trap this file documents at every other actuator:
	-- `pcall` succeeding means the call did not throw, not that the city bought
	-- anything. `CanStartCommand` is the engine's own answer and is checked first, so
	-- a refusal is reported as a refusal instead of becoming a phantom tile CIVVIS
	-- believes it owns.
	if kind == "buy_plot" then
		local city = liveCity(player, subject);
		if city == nil then return false, "no_city"; end
		if x == nil or y == nil then return false, "no_plot"; end
		local params = {};
		params[CityCommandTypes.PARAM_PLOT_PURCHASE] = true;
		params[CityCommandTypes.PARAM_X] = x;
		params[CityCommandTypes.PARAM_Y] = y;
		local can = try(function()
			return CityManager.CanStartCommand(city, CityCommandTypes.PURCHASE, params);
		end, false);
		if not can then
			emit("plot_refused", { turn = turn, x = x, y = y });
			return false, "cannot_buy_plot";
		end
		local ok = pcall(function()
			CityManager.RequestCommand(city, CityCommandTypes.PURCHASE, params);
		end);
		return ok, ok and "bought_plot" or "throw";
	end

	if kind == "purchase" or kind == "purchase_faith" then
		local city = liveCity(player, subject);
		if city == nil then return false, "no_city"; end
		local row2, resolved = resolveType(GameInfo.Types, verb);
		if row2 == nil then return false, "unknown_" .. verb; end
		local params = {};
		local formationForCost = nil;
		if row2.Kind == "KIND_UNIT" then
			params[CityCommandTypes.PARAM_UNIT_TYPE] = row2.Hash;
			-- STANDARD is needed by GetPurchaseCost, but it must not be sent as a
			-- military command parameter for civilian units. Civilization VI rejects
			-- Settlers and Builders carrying it even when the city and treasury are
			-- otherwise valid. Corps and Armies are the only explicit formations.
			local formation = tonumber(x) or 0;
			formationForCost = MilitaryFormationTypes.STANDARD_MILITARY_FORMATION;
			local unitRow = try(function() return GameInfo.Units[resolved]; end);
			local militaryFormation = unitRow ~= nil
				and ((unitRow.Combat or 0) > 0
				     or (unitRow.RangedCombat or 0) > 0
				     or (unitRow.Bombard or 0) > 0
				     or (unitRow.AntiAirCombat or 0) > 0);
			if formation == 0 and militaryFormation then
				-- Preserve the host's explicit standard formation for combat units;
				-- civilian and support units deliberately take the parameter-free path.
				params[CityCommandTypes.PARAM_MILITARY_FORMATION_TYPE] = formationForCost;
			elseif formation == 1 then
				formationForCost = MilitaryFormationTypes.CORPS_MILITARY_FORMATION;
				params[CityCommandTypes.PARAM_MILITARY_FORMATION_TYPE] = formationForCost;
			elseif formation == 2 then
				formationForCost = MilitaryFormationTypes.ARMY_MILITARY_FORMATION;
				params[CityCommandTypes.PARAM_MILITARY_FORMATION_TYPE] = formationForCost;
			end
		elseif row2.Kind == "KIND_BUILDING" then
			params[CityCommandTypes.PARAM_BUILDING_TYPE] = row2.Hash;
		elseif row2.Kind == "KIND_DISTRICT" then
			params[CityCommandTypes.PARAM_DISTRICT_TYPE] = row2.Hash;
			if x == nil or y == nil then return false, "no_district_plot"; end
			params[CityOperationTypes.PARAM_X] = x;
			params[CityOperationTypes.PARAM_Y] = y;
		else
			-- Same reasoning as the produce arm above: name it or it cannot be chased.
			return false, "no_params_" .. tostring(row2.Kind or verb);
		end
		local yieldName = kind == "purchase_faith" and "YIELD_FAITH" or "YIELD_GOLD";
		local currency = try(function() return GameInfo.Yields[yieldName].Index; end);
		if currency == nil then return false, "no_yield"; end
		params[CityCommandTypes.PARAM_YIELD_TYPE] = currency;
		-- Firaxis's `ComposeUnitForPurchase` asks whether a standard unit is
		-- purchasable with UNIT_TYPE and YIELD_TYPE only. `PurchaseUnit` adds
		-- STANDARD_MILITARY_FORMATION to the later request. The distinction is
		-- observable: run live-head-rome-religious-actions-20260802T173404Z had
		-- three Heavy Chariot checks rejected with the explicit standard formation,
		-- while an otherwise identical Catapult purchase succeeded. Preserve the
		-- formation on the request below, but make the eligibility predicate match
		-- the stock Production Panel. Corps and Armies remain explicit in both.
		local eligibilityParams = params;
		if row2.Kind == "KIND_UNIT" and (tonumber(x) or 0) == 0
			and params[CityCommandTypes.PARAM_MILITARY_FORMATION_TYPE] ~= nil then
			eligibilityParams = {};
			for key, value in pairs(params) do
				if key ~= CityCommandTypes.PARAM_MILITARY_FORMATION_TYPE then
					eligibilityParams[key] = value;
				end
			end
		end
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
		local canBuy, results = false, nil;
		local okCan, hostCan, hostResults = pcall(function()
			-- Exact signature from Firaxis's shipped ProductionPanel.lua. The third
			-- and fifth arguments are booleans; passing `params` as argument three
			-- made every otherwise valid purchase answer false without throwing.
			return CityManager.CanStartCommand(city, CityCommandTypes.PURCHASE,
			                                   false, eligibilityParams, true);
		end);
		if okCan then canBuy, results = hostCan == true, hostResults; end
		if not canBuy then
			local reasons = {};
			pcall(function()
				local failureReasons = results ~= nil
					and results[CityCommandResults.FAILURE_REASONS] or nil;
				if failureReasons ~= nil then
					for _, key in ipairs(failureReasons) do
						reasons[#reasons + 1] = {
							key = tostring(key),
							text = try(function() return Locale.Lookup(key); end, tostring(key)),
						};
					end
				end
			end);
			local cost = try(function()
				if row2.Kind == "KIND_UNIT" then
					return city:GetGold():GetPurchaseCost(
						currency, row2.Hash, formationForCost);
				end
				return city:GetGold():GetPurchaseCost(currency, row2.Hash);
			end, -1);
			-- ★★★★★ WHY DID THE HOST SAY NO WITH NO REASON?
			--
			-- 77% of purchase refusals come back `has_results=false` and an EMPTY
			-- reason list: 1334 of 1730 on 2026-08-03, 918 of them UNIT_MISSIONARY,
			-- and 93% of them affordable. Eight hypotheses are dead — affordability,
			-- no religion (the empire holds RELIGION_BUDDHISM and 382 faith), a
			-- missing Shrine (the city has one), the city following a foreign
			-- religion (it follows ours), the building already being built (0 of 93),
			-- unit stacking (0, and 401 had an EMPTY tile), and "faith purchases
			-- never work" (faith fell 15 times) — though NOT ONE of those drops was a
			-- unit: every one was 134-316, Great Person patronage, never the ~70 a
			-- Missionary costs, and zero religious units ever existed in that run.
			--
			-- `has_results=false` means Civilization VI returned no results TABLE at
			-- all, which is what an inapplicable command looks like rather than a
			-- rejected one — every refusal that carries a reason has one. This file
			-- has already been burned twice by exactly that: passing `params` as
			-- argument three "made every otherwise valid purchase answer false
			-- without throwing", and a stray PARAM_MILITARY_FORMATION_TYPE did the
			-- same to a Heavy Chariot while an identical Catapult succeeded.
			--
			-- So ask the same question four ways and record which shapes answer. This
			-- is READ-ONLY — `CanStartCommand` never spends anything, and nothing
			-- below changes what the agent does. It exists to turn a silent refusal
			-- into a named one.
			local probes = {};
			if results == nil then
				local shapes = {
					{ name = "as_sent", args = eligibilityParams },
					{ name = "full_params", args = params },
					{ name = "empty", args = {} },
					{ name = "nil_params", args = nil },
				};
				for _, shape in ipairs(shapes) do
					local ok, can, res = pcall(function()
						return CityManager.CanStartCommand(city, CityCommandTypes.PURCHASE,
						                                  false, shape.args, true);
					end);
					probes[#probes + 1] = {
						shape = shape.name,
						threw = not ok,
						can = ok and (can == true) or false,
						has_results = ok and (res ~= nil) or false,
					};
				end
			end
			emit("purchase_refused", {
				turn = turn, city = subject, item = resolved,
				currency = yieldName,
				probes = probes,
				balance = try(function()
					if yieldName == "YIELD_FAITH" then
						return player:GetReligion():GetFaithBalance();
					end
					return player:GetTreasury():GetGoldBalance();
				end, -1),
				cost = cost, checked = okCan, has_results = results ~= nil,
				reasons = reasons,
			});
			return false, "cannot_buy_" .. resolved;
		end
		local ok = pcall(function()
			CityManager.RequestCommand(city, CityCommandTypes.PURCHASE, params);
		end);
		return ok, ok and resolved or "throw";
	end

	if kind == "unit" then
		-- Asked for fresh, right here: the previous order in this very list may have
		-- killed or consumed it.
		--
		-- Name the unit in the refusal. Run civvis-20260815T003946Z logged 86
		-- bare `unit_gone` strings across 140 war turns, and an id-less entry
		-- cannot answer the question the ledger exists for — one leaked corpse
		-- re-ordered forever (a mirror bug) reads identically to rolling
		-- one-turn death latency (inherent). With the id, the two are one
		-- `GROUP BY` apart.
		local unit = liveUnit(pid, subject);
		if unit == nil then return false, "unit_gone:" .. tostring(subject); end
		if verb == "ACTIVATE_GREAT_PERSON" then
			local activated = commandUnit(
				unit, CMD["UNITCOMMAND_ACTIVATE_GREAT_PERSON"]);
			if not activated then
				emit("great_person_refused", {
					turn = turn, unit = subject,
					unit_kind = unitTypeName(unit),
					x = try(function() return unit:GetX(); end, -1),
					y = try(function() return unit:GetY(); end, -1),
				});
			end
			return activated, verb;
		end
		if verb == "DELETE" then
			-- CIVVIS retires a unit that can do nothing more -- today only the
			-- zero-charge Great Prophet left on the map after its religion was
			-- founded (see the Prophet branch of the Great Person routine, which
			-- does the same under the built-in ladder). Gated through
			-- CanStartCommand by `commandUnit`, so a unit the engine still values
			-- is refused rather than lost; the refusal is named so the ledger can
			-- tell "asked and declined" from "never asked".
			-- Through the shipped UnitPanel's own gate (`loose`): the strict form
			-- refused every DELETE ever asked (495 across three runs, zero
			-- retirements) and the ghost stood on its hex all game.
			local deleted, why = commandUnit(unit, CMD["UNITCOMMAND_DELETE"], true);
			if deleted then
				emit("gp", { turn = turn, unit = subject, action = "retired_by_civvis",
					kind_name = unitTypeName(unit) });
			else
				emit("delete_refused", {
					turn = turn, unit = subject,
					unit_kind = unitTypeName(unit),
					x = try(function() return unit:GetX(); end, -1),
					y = try(function() return unit:GetY(); end, -1),
					why = why,
				});
			end
			return deleted, verb;
		end
		if verb == "ENTER_FORMATION" then
			-- `x`/`y` carry the target owner/id for this non-positional command.
			-- This is the exact stock UnitPanel.lua signature.
			if x == nil or y == nil then return false, "no_formation_target"; end
			local target = liveUnit(x, y);
			if target == nil then return false, "formation_target_gone"; end
			local params = {};
			params[UnitCommandTypes.PARAM_UNIT_PLAYER] = x;
			params[UnitCommandTypes.PARAM_UNIT_ID] = y;
			local hash = CMD["UNITCOMMAND_ENTER_FORMATION"];
			if hash == nil then return false, "no_enter_formation_command"; end
			local okCan, can = pcall(function()
				return UnitManager.CanStartCommand(unit, hash, params);
			end);
			if not (okCan and can == true) then return false, "cannot_enter_formation"; end
			local ok = pcall(function()
				UnitManager.RequestCommand(unit, hash, params);
			end);
			return ok, ok and verb or "throw";
		end
		if verb == "EXIT_FORMATION" then
			return commandUnit(unit, CMD["UNITCOMMAND_EXIT_FORMATION"]), verb;
		end
		-- FOUND_CITY, MOVE_TO and RANGE_ATTACK are the three that decide a game.
		-- ⚠ There is NO attack operation on this build — the resolved list is only
		-- MOVE_TO and RANGE_ATTACK — so a melee strike IS a MOVE_TO onto the
		-- defended plot. CIVVIS's `Attack` therefore translates to MOVE_TO, and
		-- that is not a workaround: it is how Civilization VI resolves it.
		if verb == "FOUND_CITY" then
			-- ⚠ READ THE PLOT BEFORE FOUNDING. A settler founds where it STANDS, so
			-- the order carries no x/y, and the unit is consumed by the operation —
			-- afterwards there is nothing left to ask. The refusal path below can
			-- still call `unit:GetX()` precisely because it did not found.
			local atX = try(function() return unit:GetX(); end);
			local atY = try(function() return unit:GetY(); end);
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
				-- ⚠ ONE SIGNATURE WAS GUESSED AND IT ANSWERS NOTHING. Every
				-- `found_refused` this project has recorded reads `can_start=false` —
				-- 14 of 14 across every run — because the call below passed `{}` where
				-- Civilization VI wants a BOOLEAN `bTestOnly`, so the results table
				-- came back nil and the useful half of the answer was never produced.
				-- The reason this event exists is to say WHY, and it has only ever
				-- said "no".
				--
				-- Both plausible shipped forms are tried and the one that answers is
				-- named, exactly as `plotRevealed` does for `PlayersVisibility` — a
				-- silent gate is worse than a loud one, and guessing again without
				-- recording which guess worked is how this happened the first time.
				-- ★★★★★ THE SHIPPED CALL, READ OUT OF `UnitPanel.lua:630`. Two guesses
				-- before it produced `can_start=false,no_results` on every refusal in
				-- the project's history — the results table was never populated because
				-- the fifth argument is `OperationResultsTypes.ALL`, not `true`, and the
				-- reasons live under `UnitOperationResults.FAILURE_REASONS` rather than
				-- at the top level:
				--
				--   local bCanStart, tResults = UnitManager.CanStartOperation(
				--       pUnit, UnitOperationTypes.FOUND_CITY, nil, false,
				--       OperationResultsTypes.ALL);   -- No exclusion test
				--   tResults[UnitOperationResults.FAILURE_REASONS]
				--
				-- ⚠ The reasons are LOC keys, so they are localised before emitting —
				-- an untranslated key names the rule but not in words anyone reading
				-- the ledger would recognise.
				-- ★★★★★ SAY WHETHER THE SETTLER COULD STILL MOVE, because this
				-- event PERMANENTLY BLOCKS THE CITY SITE.
				--
				-- `found_refused` feeds `refused_sites` -> `blocked_city_sites`,
				-- which is extended and never cleared, so every one of these is a
				-- forever verdict on that ground. Measured across every live run of
				-- 2026-08-11, 9 found refusals: the settler had `movesRemaining ==
				-- 0` on EIGHT of them. A Civilization VI Settler needs movement
				-- left to found, so those sites were condemned for a condition that
				-- clears itself on the next turn.
				--
				-- Improvements had the identical defect and the identical cure
				-- (#1548, #1550); a city site is the more expensive one to get
				-- wrong. This project's own measurements have expansion as the
				-- binding constraint -- 36% of games end on ONE city, and the
				-- empire peaks at three -- so a legitimate site struck off the map
				-- for a spent move is not a small loss.
				--
				-- `refused_sites_of_kind_through` already drops an explicit zero,
				-- and it is shared by both refusals, so recording the reading here
				-- is the whole fix.
				local moves = try(function() return unit:GetMovesRemaining(); end, -1);
				local why = refusalReason(unit, UnitOperationTypes.FOUND_CITY, nil);
				emit("found_refused", { turn = turn, unit = subject, why = why,
				                        moves = moves,
				                        x = unit:GetX(), y = unit:GetY() });
			else
				-- ★★★★★ REPORT THE SUCCESS TOO, OR THE LEDGER ONLY KNOWS FAILURES.
				--
				-- Only the refusal was emitted, so a city that WAS founded left no
				-- event at all — the sole trace was the `cities` count moving in the
				-- next turn event. That is a state change the export does not carry,
				-- and reading a run without it is guesswork.
				--
				-- Measured cost, on run civvis-20260731T213310Z: 32 `settle_choice`
				-- and 0 `found` events, which reads as "no settler ever founded".
				-- The city count says otherwise — 0->1 at t3, 1->2 at t89, 2->1 at
				-- t93, 1->2 at t130, 2->3 at t153, 3->4 at t168, 4->3 at t172. The
				-- empire founded four cities and lost two, and the event stream could
				-- not distinguish that from settlers dying on the road. It cost this
				-- loop a wrong conclusion, stated twice.
				--
				emit("found", { turn = turn, unit = subject, x = atX, y = atY });
			end
			return placed, "found";
		end
		if verb == "MOVE_TO" or verb == "ATTACK" then
			if x == nil or y == nil then return false, "no_dest"; end
			-- ★★★★★ SEND THIS TURN'S LEG, NOT A PATH THE HOST WALKS NEXT TURN.
			-- A melee ATTACK is a MOVE_TO onto the defender and is never capped.
			-- The row's own x/y are rewritten so the queue expects the capped
			-- plot; the original destination rides in `move_capped`.
			if verb == "MOVE_TO" and cfg.CapMovesToReach ~= false then
				local capped, why = CivvisBoard.capToTurn(unit, x, y);
				if capped == false then
					CivvisBoard.stats.no_reach = CivvisBoard.stats.no_reach + 1;
					return false, "move_" .. tostring(why);
				elseif capped ~= nil then
					CivvisBoard.stats.capped = CivvisBoard.stats.capped + 1;
					emit("move_capped", { turn = turn, unit = subject,
					                      want = { x, y }, sent = { capped.x, capped.y },
					                      turns = capped.turns });
					x, y = capped.x, capped.y;
					row.x, row.y = x, y;
				end
			end
			local params = {};
			params[UnitOperationTypes.PARAM_X] = x;
			params[UnitOperationTypes.PARAM_Y] = y;
			if verb == "ATTACK" then CivvisLedger.strike(unit, subject, verb, x, y, turn); end
			local moved = operate(unit, OP["UNITOPERATION_MOVE_TO"], params);
			if not moved then
				-- ★★★★ NAME THE UNIT AND WHERE IT WOULD NOT GO. `refusals` is
				-- aggregated by REASON, so 127 `MOVE_TO` refusals on one run could not
				-- say which unit was refused even once -- and that is exactly the
				-- question that mattered.
				--
				-- Measured on run civvis-20260801T060730Z: a settler sat at (61,32) for
				-- turns 168-171 being ordered `MOVE_TO 60,32` every turn and not
				-- moving, with every adjacent plot owned by another player. `MISSED`
				-- was 0 across 2662 orders, so the move was being refused rather than
				-- lost -- but nothing could tie those refusals to that settler, so
				-- "the settler is boxed in by borders" stayed a hypothesis.
				--
				-- Same shape as `found_refused` and `improve_refused` directly above
				-- and below, and the same use: the Rust side can feed a NAMED refusal
				-- back so CIVVIS stops re-deriving the same impossible step.
				emit("move_refused", {
					turn = turn,
					unit = subject,
					-- ⚠ `unit_kind`, NOT `kind`. `emit` sets `payload.kind = <event kind>`
					-- on line 105, so a field named `kind` here is CLOBBERED — the
					-- first run with this event reported `kinds: [('move_refused', 22)]`
					-- and the unit type was gone. Any payload this file builds must
					-- avoid `kind`, `ctx` and `run` for the same reason.
					unit_kind = try(function() return unit:GetType(); end),
					from_x = try(function() return unit:GetX(); end, -1),
					from_y = try(function() return unit:GetY(); end, -1),
					x = x, y = y,
					owner = try(function()
						local plot = Map.GetPlot(x, y);
						return plot and plot:GetOwner() or -1;
					end, -1),
					-- ⚠ WHAT THE MOVE WAS AIMED AT, which nothing recorded — so the
					-- refusals could be counted and attributed but never EXPLAINED.
					--
					-- With `unit_kind` in place, run `civvis-20260801T065721Z` splits its
					-- 300 refusals as: TRADER 117 (fixed in #742), BATTERING_RAM 57,
					-- GALLEY 44, HEAVY_CHARIOT 34, ARCHER 25. Those are different bugs
					-- wearing one number, and the owner field alone cannot separate them:
					--
					--   a GALLEY aimed at dry land  -> CIVVIS pathing a ship overland
					--   a RAM aimed at a mountain   -> impassable ground
					--   any unit into foreign soil  -> the boxed-in case, `owner` says it
					--
					-- 125 of the 300 were aimed at UNOWNED ground, which rules the
					-- borders explanation out for most of them and leaves no candidate
					-- at all — the terrain is the missing half of the question.
					--
					-- `dest_domain` is the unit's own domain, not the plot's: a ship
					-- ordered onto land and a land unit ordered into the sea are the same
					-- defect and both are invisible without it.
					dest_water = try(function()
						local plot = Map.GetPlot(x, y);
						return plot ~= nil and plot:IsWater() or false;
					end, false),
					dest_impassable = try(function()
						local plot = Map.GetPlot(x, y);
						return plot ~= nil and plot:IsImpassable() or false;
					end, false),
					-- ⚠ Through `typeName`, NOT a hand-rolled `GameInfo.Terrains[i]`.
					-- `GetTerrainType` returns a ROW INDEX, and the one other place
					-- this file resolves one carries the warning: indexing the table
					-- directly means guessing its ordering, and a wrong guess reports a
					-- DIFFERENT terrain rather than failing. `typeName` sends nil when
					-- the type does not resolve, which is the honest answer.
					dest_terrain = try(function()
						local plot = Map.GetPlot(x, y);
						if plot == nil then return nil; end
						return typeName("Terrains", "TerrainType", plot:GetTerrainType());
					end),
					-- `GameInfo.Units[name]` is the proven lookup here (`row.Combat`
					-- uses it); only the Domain column is new, and an absent column
					-- reads nil rather than a plausible wrong value.
					unit_domain = try(function()
						local row = GameInfo.Units[unitTypeName(unit)];
						return row ~= nil and row.Domain or nil;
					end),
				});
			end
			return moved, verb;
		end
		if verb == "RANGE_ATTACK" then
			if x == nil or y == nil then return false, "no_dest"; end
			local params = {};
			params[UnitOperationTypes.PARAM_X] = x;
			params[UnitOperationTypes.PARAM_Y] = y;
			CivvisLedger.strike(unit, subject, verb, x, y, turn);
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
			-- ★★★★★ ASKING FOR WHAT IS ALREADY THERE IS NOT A REFUSAL.
			--
			-- #1561 put the tile's own state in the refusal, and the first run
			-- carrying it answered a question that had been open for six
			-- iterations. Run civvis-20260811T163652Z, 23 of 23 refusals: the tile
			-- OURS, the builder holding movement and charges, and the improvement
			-- already on the ground —
			--
			--     t19 MINE   owner=0 existing=IMPROVEMENT_MINE
			--     t30 QUARRY owner=0 existing=IMPROVEMENT_QUARRY
			--
			-- and the orders ledger names the mechanism exactly:
			--
			--     t18 IMPROVE:MINE ordered -> succeeded
			--     t19 IMPROVE:MINE ordered again -> refused
			--
			-- CIVVIS builds it, then orders the identical improvement on the same
			-- tile the next turn, because the tile sweep runs every four turns and
			-- its board still shows the tile bare.
			--
			-- ⚠⚠ THE REAL DAMAGE IS THE LEDGER, NOT THE ORDER. One wasted call a
			-- turn is cheap; 376 `improve_refused` in a day that are all benign
			-- duplicates is not, because it buries the refusals that mean
			-- something. It cost three PRs of mine chasing a gate that was right
			-- (#1548/#1550/#1552, corrected in #1557), and `improve_refused` is
			-- also what BLOCKS the tile in CIVVIS's planner — a duplicate must
			-- never reach it, because the tile is not dead, it is done.
			--
			-- So name it for what it is, before spending the engine call.
			-- ⚠ Only on an exact match: a Farm asked for where a Mine stands is a
			-- real disagreement and must still go to the engine.
			if row2 ~= nil then
				local here = try(function()
					local plot = Map.GetPlot(params[UnitOperationTypes.PARAM_X],
					                         params[UnitOperationTypes.PARAM_Y]);
					if plot == nil then return nil; end
					return typeName("Improvements", "ImprovementType",
					                plot:GetImprovementType());
				end);
				if here ~= nil and here == wanted then
					emit("improve_already", {
						turn = turn, unit = subject, want = wanted,
						x = params[UnitOperationTypes.PARAM_X],
						y = params[UnitOperationTypes.PARAM_Y],
					});
					return false, "already_" .. wanted;
				end
			end
			if operate(unit, OP["UNITOPERATION_BUILD_IMPROVEMENT"], params) then
				-- ★★★★★ REPORT THE SUCCESS, or the board learns four turns late.
				--
				-- The same rule `found` already follows: "REPORT THE SUCCESS TOO,
				-- OR THE LEDGER ONLY KNOWS FAILURES". Improvements had no such
				-- event, so a finished one reached CIVVIS only on the next
				-- periodic tile sweep — every four turns — and in that window
				-- CIVVIS re-ordered what it had just built. Measured on run
				-- civvis-20260811T163652Z: 23 duplicate orders, every refusal 1-3
				-- turns after a sweep, the ledger showing
				--
				--     t18 IMPROVE:MINE -> succeeded
				--     t19 IMPROVE:MINE -> refused, existing=IMPROVEMENT_MINE
				--
				-- ⚠⚠⚠ AND THE IMPROVEMENT IS NOT ON THE PLOT YET. #1565 read it back
				-- with `plot:GetImprovementType()` right here, reasoning that the
				-- engine's own answer beats the name we asked for. The engine had
				-- no answer to give: `UNITOPERATION_BUILD_IMPROVEMENT` is
				-- REQUESTED, not executed inline, so at this line the plot is still
				-- bare, `typeName` returns nil, and the emit is skipped every time.
				--
				-- Measured on the first run carrying it, civvis-20260811T183513Z:
				-- 120 turns, improve orders issued and succeeding, and `improved`
				-- fired ZERO times while `improve_already` fired 8 — the duplicates
				-- the event exists to prevent, which are themselves the proof the
				-- improvement did land, one turn later.
				--
				-- So use the name that was ASKED FOR. It is correct precisely here:
				-- this branch names the improvement, `operate` returns true only
				-- when `canOperate` accepted that exact request, and the engine
				-- builds what was named. #1565's reasoning was about the FALLBACK
				-- below, which drops the name and lets the engine choose — and that
				-- branch does not emit at all, so the concern was real and belonged
				-- to a different line.
				--
				-- ⭐ Third time recently that a reading was taken at the wrong
				-- INSTANT: a state snapshot joined by turn, a movement value
				-- sampled outside the decision, and now a plot read before the
				-- operation ran. Ask when a value becomes true, not only where it
				-- lives.
				if wanted ~= nil then
					emit("improved", {
						turn = turn,
						x = params[UnitOperationTypes.PARAM_X],
						y = params[UnitOperationTypes.PARAM_Y],
						im = wanted,
					});
				end
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
					and commandUnit(unit, CMD["UNITCOMMAND_AUTOMATE"]) then
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
			-- ⚠⚠ NAME THE TILE THE ORDER ASKED FOR, NOT THE ONE THE BUILDER IS ON.
			--
			-- `x`/`y` above carry the ORDER's target and only fall back to the unit's
			-- own tile. Reporting `unit:GetX()` here therefore named the wrong ground
			-- whenever the builder had not reached the target — and a builder that
			-- cannot reach its target is exactly the case this feedback exists for.
			--
			-- What it recorded instead was wherever the builder was stuck, usually the
			-- capital's own centre. `Game::valid_improvements` already returns nothing
			-- for a tile with a city on it, so the entry changed no decision, the real
			-- tile stayed unblocked, and CIVVIS re-derived the same target from the
			-- same board forever.
			--
			-- Measured on run civvis-20260802T041527Z: 286 `improve_refused`, of which
			-- 118 + 84 + 59 + 23 + 13 name the SAME tile (63,11) — the capital centre —
			-- across three builders, for a whole 250-turn game. Runs that expanded
			-- refuse each tile once or twice and move on, which is what the feedback
			-- looks like when it lands on the right ground.
			--
			-- ⚠ The automation rung above can move the builder before this line runs,
			-- so `unit:GetX()` is not even reliably where the refusal happened.
			-- ★★★★★ AND SAY WHY, for the same reason `found_refused` does.
			--
			-- This is the most numerous refusal the ledger records — 72 across six
			-- recent runs, 286 in one 250-turn game — and until now it named the
			-- tile and nothing else. By the time it fires, three fallbacks have
			-- already failed (the named improvement, then any improvement, then
			-- automating the builder), so "this tile is dead" is established. What
			-- was missing is the half that says WHOSE bug it is:
			--
			--   unowned tile          -> CIVVIS targeted ground it does not hold
			--   already improved      -> the mirror is stale
			--   no movement / charges -> an actuation defect, and one that has
			--                            happened before, see
			--                            `a-builder-with-no-movement-cannot-improve`
			--
			-- Those three demand different fixes and were indistinguishable in the
			-- export. `params` is the table the refused operation was given, so the
			-- engine is asked about the tile the ORDER named — the same distinction
			-- the `x`/`y` note above exists to protect.
			-- ★★★★★ RECORD THE READINGS AT THE POINT OF THE DECISION.
			--
			-- ⚠⚠⚠ AND THE FIRST ANSWER THIS FIELD GAVE CONTRADICTED WHY IT WAS
			-- ADDED. #1548 introduced `moves` on the strength of a claim that
			-- builders were out of movement on 25 of 26 refusals. That number came
			-- from matching refusals against the STATE EXPORT by turn, and a
			-- per-turn snapshot is not the same instant as an event emitted during
			-- that turn. Once the reading was taken HERE, by
			-- `GetMovesRemaining()` at the moment of the attempt, the two
			-- disagreed flatly — same turn, same unit, run
			-- civvis-20260811T134008Z:
			--
			--     turn 19  unit 327683   event moves 2   state moves 0
			--     turn 46  unit 983049   event moves 2   state moves 0
			--     ... all 25 refusals: 2, 3 or 4. NEVER zero.
			--
			-- So builders are NOT out of movement here, the refusals are genuine,
			-- and `canOperate` is right. The five-argument probe remains the
			-- misleading one — with `plots = nil` it answers "can this unit build
			-- SOMEWHERE", not "here", which is why it reports `can_start=true`
			-- against a gate that correctly reports false for this tile.
			--
			-- ⭐ Which is exactly why these two readings belong in the event and
			-- not in a later join: a snapshot matched by turn number reads as a
			-- measurement and is not one.
			local moves = try(function() return unit:GetMovesRemaining(); end, -1);
			local charges = try(function() return unit:GetBuildCharges(); end, -1);
			-- ★★★★★ AND THE TILE'S OWN STATE, READ HERE RATHER THAN JOINED LATER.
			--
			-- #1557 reopened the question this event exists to answer: the builder
			-- has movement, has charges, and stands on the ordered tile, and
			-- `canOperate` still refuses. Two ordinary explanations remain and
			-- neither is in the record — the tile is not OURS (an unowned tile
			-- cannot be improved), or it is ALREADY improved.
			--
			-- ⚠⚠⚠ AND THEY MUST BE READ AT THIS INSTANT, NOT LOOKED UP AFTERWARDS.
			-- I tried to answer the ownership half by matching refusals against the
			-- periodic tile export and it cannot be done: 23 of 25 refused tiles
			-- appear in that export as BOTH unowned and ours at different points in
			-- the same run, so the join has no defensible answer. That is the same
			-- mistake that produced a false movement measurement and three PRs
			-- resting on it (#1548/#1550/#1552, corrected in #1557) — a per-turn
			-- snapshot is not the instant of the decision.
			--
			-- Two `try` calls on a path that has already failed three fallbacks.
			-- `im` is the improvement ALREADY on the plot, named the same way the
			-- tile export names it, so an already-improved tile is legible without
			-- a second lookup.
			local plot = try(function()
				return Map.GetPlot(params[UnitOperationTypes.PARAM_X],
				                   params[UnitOperationTypes.PARAM_Y]);
			end);
			local tile_owner = plot ~= nil
				and try(function() return plot:GetOwner(); end, -1) or -1;
			local tile_improvement = plot ~= nil and typeName("Improvements",
				"ImprovementType",
				try(function() return plot:GetImprovementType(); end, -1)) or nil;
			local why = refusalReason(unit, OP["UNITOPERATION_BUILD_IMPROVEMENT"],
			                          params);
			-- ⚠⚠⚠ THE TWO FORMS OF THE SAME CALL DISAGREE, AND ONLY ONE OF THEM
			-- GATES THE WORK.
			--
			-- First live run on #1542, `civvis-20260811T094304Z`: every one of the
			-- thirteen refusals reads `can_start=true,no_reasons [p4r]`. The engine
			-- says the operation CAN start, at the exact moment we tell CIVVIS the
			-- tile is dead — and that event blocks the tile in its planner, so a
			-- wrong one poisons the map it plans from.
			--
			-- But the probe and the gate are not the same call:
			--
			--   canOperate     CanStartOperation(unit, hash, nil, params)
			--   refusalReason  CanStartOperation(unit, hash, nil, params, ALL)
			--
			-- `operate` only reaches `RequestOperation` when `canOperate` returns
			-- true, so reaching this line means the 4-arg form said FALSE while the
			-- 5-arg form says TRUE. Either the results argument changes what the
			-- engine tests, or the gate under-reports and this harness has been
			-- refusing improvements Civilization VI would have allowed.
			--
			-- ⚠ I am not claiming which. This file has been wrong three times by
			-- reasoning about an overload instead of measuring it, so record BOTH
			-- answers side by side and let the next live run say. No behaviour
			-- changes here: `can_operate` is read after every attempt has already
			-- failed, and is only written down.
			emit("improve_refused", { turn = turn, unit = subject,
			                          want = wanted or "IMPROVE", why = why,
			                          -- Every reading taken at THIS instant, which
			                          -- is the only place they mean anything.
			                          moves = moves, charges = charges,
			                          tile_owner = tile_owner,
			                          tile_improvement = tile_improvement,
			                          can_operate = canOperate(unit,
			                              OP["UNITOPERATION_BUILD_IMPROVEMENT"],
			                              params),
			                          x = params[UnitOperationTypes.PARAM_X],
			                          y = params[UnitOperationTypes.PARAM_Y] });
			return false, wanted or "IMPROVE";
		end
		-- ★★★ SEND THE TRADER SOMEWHERE. Untranslated until now, so a trader stood
		-- where it was built for the whole game: `civ6_watchdogs.py` names one in every
		-- run, motionless for 114 turns in the longest case, with its production and
		-- its maintenance already paid.
		--
		-- ⚠ THE FIRST TWO VERSIONS WERE GUESSES AND BOTH WERE WRONG — 15 refusals in
		-- one run, 87 in the next. Reporting a refusal instead of assuming success is
		-- what made that visible in one line of the status output; the shipped call
		-- below was then read out of the game's own `TradeRouteChooser.lua` rather
		-- than guessed a third time.
		if verb == "TRADE_ROUTE" then
			if x == nil or y == nil then return false, "no_dest"; end
			-- ★★★★ COPIED FROM `TradeRouteChooser.lua`, NOT GUESSED — and the two
			-- guesses before it cost 15 refusals in one run and 87 in the next, which
			-- is exactly what reporting a refusal instead of assuming success is for.
			--
			-- The shipped `RequestTradeRoute()` sends FOUR parameters, not two: the
			-- destination city as X0/Y0 and the TRADER'S OWN plot as X1/Y1. And the
			-- operation is `UnitOperationTypes.MAKE_TRADE_ROUTE`, the engine constant,
			-- not a hash looked up in `GameInfo.UnitOperations` — `MAKE_TRADE_ROUTE`
			-- also exists as an `InterfaceModeTypes`, which is what made the name look
			-- available when the row was not.
			--
			--   operationParams[PARAM_X0] = destinationCity:GetX();
			--   operationParams[PARAM_Y0] = destinationCity:GetY();
			--   operationParams[PARAM_X1] = m_selectedUnit:GetX();
			--   operationParams[PARAM_Y1] = m_selectedUnit:GetY();
			--   UnitManager.RequestOperation(unit, MAKE_TRADE_ROUTE, operationParams)
			local op = UnitOperationTypes.MAKE_TRADE_ROUTE;
			if op == nil then return false, "no_trade_route_op"; end
			local params = {};
			params[UnitOperationTypes.PARAM_X0] = x;
			params[UnitOperationTypes.PARAM_Y0] = y;
			local fromX = try(function() return unit:GetX(); end, -1);
			local fromY = try(function() return unit:GetY(); end, -1);
			params[UnitOperationTypes.PARAM_X1] = fromX;
			params[UnitOperationTypes.PARAM_Y1] = fromY;
			local routed = operate(unit, op, params);
			if not routed then
				-- Geometric range is not a route. Carry both endpoints back so the
				-- mirror can stop offering this exact unreachable pairing instead of
				-- re-sending it forever (83 consecutive turns in the Poland trace).
				emit("trade_route_refused", {
					turn = turn, unit = subject,
					from_x = fromX, from_y = fromY, x = x, y = y,
				});
			end
			return routed, verb;
		end
		if verb == "UPGRADE" then
			-- ⚠⚠ THROUGH `upgradeUnit`, NOT `commandUnit` DIRECTLY. That helper
			-- exists precisely to turn a refused upgrade into a NAMED one — it
			-- asks the engine the two-flag `CanStartCommand` and records
			-- `FAILURE_REASONS` — and until now only the mod's own automation
			-- called it. Every upgrade CIVVIS itself ordered went straight to
			-- `commandUnit` and was counted anonymously.
			--
			-- The cost of that gap, measured over the 08-04/08-05 runs:
			-- `UPGRADE` is 933 refusals, the third-largest category of all, and
			-- `upgrade_tried` / `upgrade_blocked` both read ZERO on every run
			-- because the counters were watching a path the orders never took.
			--
			-- This is the same defect the comment above `upgradeUnit` was
			-- written about: "an anonymous count ... naming it made the cause
			-- fall out immediately and both times the standing hypothesis was
			-- wrong". The helper was right; it simply was not wired here.
			local upgraded = upgradeUnit(unit);
			return upgraded ~= nil, verb;
		end
		local promotionName = string.match(tostring(verb), "^PROMOTE:(.+)$");
		if promotionName ~= nil then
			local promotion = try(function()
				return GameInfo.UnitPromotions[promotionName];
			end);
			local hash = CMD["UNITCOMMAND_PROMOTE"];
			if promotion == nil or hash == nil then
				return false, "unknown_promotion_" .. promotionName;
			end
			-- Use the stock promotion popup's own availability result. A syntactically
			-- valid promotion from another class must remain an observable refusal,
			-- never a RequestCommand we merely assume the engine accepted.
			local okCan, can, results = pcall(function()
				return UnitManager.CanStartCommand(unit, hash, true, true);
			end);
			local offered = results ~= nil
				and results[UnitCommandResults.PROMOTIONS] or nil;
			local available = false;
			if okCan and can == true and type(offered) == "table" then
				for _, index in ipairs(offered) do
					if index == promotion.Index then available = true; break; end
				end
			end
			if not available then
				-- ★★★★ NAME WHAT THE ENGINE OFFERED. The answer is already in hand
				-- three lines above and was being thrown away: `can` and the
				-- `PROMOTIONS` list are exactly what separates two refusals that
				-- want opposite responses, and the event recorded neither.
				--
				--   nothing offered  -- this unit has no promotion available at
				--     all: not enough experience, or already promoted this level.
				--     CIVVIS should not have asked, and the fix is upstream in
				--     when it asks.
				--   others offered   -- the unit can promote, just not into the
				--     tree CIVVIS named (a Recon promotion asked of a melee unit).
				--     That is a targeting bug in the choice, and the offered list
				--     names what it should have chosen from.
				--
				-- 56 of these across the eight live runs of 2026-08-11, every one
				-- carrying only the name that failed. This is the same distinction
				-- `build_no_plot` draws with `offered`, for the same reason.
				--
				-- Names, not indices: `-1743686858` in a ledger is a value no
				-- reader can turn back into a promotion, which is the exact defect
				-- the district refusal above had to be repaired for.
				local offeredNames = nil;
				if type(offered) == "table" then
					offeredNames = {};
					for _, index in ipairs(offered) do
						local row = try(function()
							return GameInfo.UnitPromotions[index];
						end);
						offeredNames[#offeredNames + 1] =
							(row ~= nil and row.UnitPromotionType) or tostring(index);
					end
				end
				emit("promotion_refused", {
					turn = turn, unit = subject, promotion = promotionName,
					can_promote = okCan and can or false,
					offered = offeredNames ~= nil and #offeredNames or 0,
					offered_promotions = offeredNames,
				});
				return false, "cannot_promote_" .. promotionName;
			end
			local params = {};
			params[UnitCommandTypes.PARAM_PROMOTION_TYPE] = promotion.Index;
			local ok = pcall(function()
				UnitManager.RequestCommand(unit, hash, params);
			end);
			return ok, ok and verb or "throw";
		end
		-- A spy mission is an operation aimed at a CITY PLOT, exactly as
		-- Firaxis' own EspionagePopup issues it: PARAM_X/PARAM_Y then
		-- `RequestOperation`. Travelling to a new city uses the same shape,
		-- which is why one branch serves both. Without the plot the host has
		-- nothing to aim at, so a missing destination is named rather than
		-- sent as an empty-parameter operation that would silently do nothing.
		if verb:sub(1, 4) == "SPY_" then
			local hash = OP["UNITOPERATION_" .. verb];
			if hash == nil then return false, "unknown_op_" .. verb; end
			if x == nil or y == nil then return false, "no_spy_target:" .. verb; end
			local params = {};
			params[UnitOperationTypes.PARAM_X] = x;
			params[UnitOperationTypes.PARAM_Y] = y;
			return operate(unit, hash, params), verb;
		end
		-- Condemning a heretic is a COMMAND, not an operation, and it is
		-- parameterless: `UnitCommands` gives the row no `InterfaceMode`, so
		-- Firaxis' own UnitPanel takes its "no mode needed" branch and calls
		-- `RequestCommand(unit, hash)` with no third argument. The target is
		-- implied by the unit standing on it -- the engine requires the same
		-- co-location -- so there is nothing to pass.
		--
		-- `loose` for the same reason DELETE uses it: the strict
		-- `CanStartCommand(unit, hash, false, true)` gate refused 495 of 495
		-- deletes while the shipped panel gated its button on the loose form
		-- and requested outright. A refusal is named rather than anonymous.
		if verb == "CONDEMN_HERETIC" then
			local hash = CMD["UNITCOMMAND_CONDEMN_HERETIC"];
			if hash == nil then return false, "unknown_cmd_" .. verb; end
			local ok, why = commandUnit(unit, hash, true);
			return ok, ok and verb or (why or "condemn_refused");
		end
		-- Anything else is a named operation from the resolved table: FORTIFY,
		-- ALERT, SKIP_TURN, HEAL, AUTOMATE_EXPLORE, BUILD_IMPROVEMENT,
		-- SPREAD_RELIGION.
		local hash = OP["UNITOPERATION_" .. verb];
		if hash == nil then return false, "unknown_op_" .. verb; end
		return operate(unit, hash, {}), verb;
	end

	return false, "unknown_kind_" .. kind;
end

-- Exposed solely for the Lua 5.1 regression.  A bare global is required: the
-- Civilization VI UI sandbox has no `_G` table.  Reusing the existing local
-- handler avoids consuming another main-chunk register.
CivvisApplyOrder = applyOrder;
CivvisResolveActions = resolveActions;
CivvisOrdersReady = ordersReady;
CivvisFetchOrders = fetchOrders;
CivvisExportState = exportState;

-- Pick the major civilization that is closest to a diplomatic victory.  The
-- World Congress vote needs this independently of the rest of the turn loop,
-- and score breaks a real DVP tie: at turn 221 of
-- `civvis-20260816T045316Z`, Sweden and America both had 17 points, but
-- America led 995 to 968 and reached 20 points first.  Player-manager order
-- is not a strategic signal, so retain it only as the final stable tie-break.
--
-- Exposed for the offline Lua regression.  This must remain a bare global:
-- another file-scope `local` would exceed Civ 6's 200-register chunk ceiling.
-- Pick the major civilization closest to ANY victory this ballot can punish.
--
-- ★★★★ THE PENALTY RESOLUTIONS WERE VOTED AT OURSELVES. Every player-targeted
-- resolution except the Diplomatic Victory one selected THIS SEAT with option
-- 1, the option that BUFFS its target -- a free vote, spent on a small bonus,
-- while the civilization about to end the game took nothing. Measured over the
-- 39 live games of 2026-08-16/17 that is 232 wasted ballots: Trade Policy 57,
-- Migration Treaty 55, Border Control 45, Public Relations 75, about six a
-- game. Meanwhile the host's own record says diplomatic (32) and culture (27)
-- victories are 83% of every game a rival ended before the turn cap.
--
-- `progress` is the share of the way to a victory, so the two lanes that
-- actually end our games are comparable on one axis: diplomatic points out of
-- twenty, and the culture race's own ratio (a leader's visiting tourists over
-- the highest domestic tourist count it must clear). Ties break on score and
-- then on id, for the same reason `CivvisSelectCongressLeader` does: player
-- manager order is not a strategic signal.
--
-- Exposed for the offline Lua regression. Must remain a bare global -- another
-- file-scope `local` would exceed Civ 6's 200-register chunk ceiling.
CivvisSelectVictoryThreat = function(candidates)
	local threat, best, bestScore = -1, -1, -1;
	for _, candidate in ipairs(candidates or {}) do
		if type(candidate) == "table" then
			local other = tonumber(candidate.id);
			local progress = tonumber(candidate.progress) or 0;
			local score = tonumber(candidate.score) or -1;
			if other ~= nil and (progress > best
				or (progress == best and score > bestScore)
				or (progress == best and score == bestScore
					and (threat < 0 or other < threat))) then
				threat, best, bestScore = other, progress, score;
			end
		end
	end
	return threat, best;
end

CivvisSelectCongressLeader = function(candidates)
	local leader, leaderPoints, leaderScore = -1, -1, -1;
	for _, candidate in ipairs(candidates or {}) do
		if type(candidate) == "table" then
			local other = tonumber(candidate.id);
			local points = tonumber(candidate.points) or 0;
			local score = tonumber(candidate.score) or -1;
			if other ~= nil and (points > leaderPoints
				or (points == leaderPoints and score > leaderScore)
				or (points == leaderPoints and score == leaderScore
					and (leader < 0 or other < leader))) then
				leader, leaderPoints, leaderScore = other, points, score;
			end
		end
	end
	return leader, leaderPoints, leaderScore;
end

-- ★★★★ THE ASK IS PRICED AGAINST BOTH TABLES THE HOST MIGHT CHARGE.
--
-- Every multi-vote ballot this seat ever sent saturated the bank the host's
-- own `GetVotesandFavorCost` table said it could afford — 14/16/18/20 votes
-- across civvis-20260819T004405Z, 13 at t162 of T175125Z — and all 17 were
-- refused whole while all 95 one-vote ballots registered. That table is the
-- ONLINE curve: the k-th extra vote costs 4k, cumulative `2n(n-1)`. The
-- Standard curve the game was written against charges 10k, cumulative
-- `5n(n-1)` — the same 780-for-13-votes this file's own #2039 comment quotes
-- from the shipped ladder. A core that CHARGES Standard while the accessor
-- REPORTS Online refuses every ask this seat has ever made as unaffordable,
-- and none of the 112 verdict rows can tell, because no ballot ever asked a
-- count small enough to fit both tables. So cap the ask by both: when the
-- theory is wrong this asks fewer votes than the bank affords on a ballot
-- that today registers ONE, which cannot lose a vote we are getting; when it
-- is right, the first session past the cap finally registers a bank. The
-- verdict's `budget` field carries both walks so the session that decides it
-- is attributable.
--
-- Exposed for the offline Lua regression. Must remain a bare global -- another
-- file-scope `local` would exceed Civ 6's 200-register chunk ceiling.
CivvisCongressVoteBudget = function(favor, costs, maxVotes)
	local bank = tonumber(favor) or 0;
	local cap = tonumber(maxVotes) or 1;
	if cap < 1 then cap = 1; end
	-- `costs[k]` is the host's cumulative price of k+1 votes; the first vote
	-- (`costs[0]`) is free on every observed table.
	local host = 1;
	while host + 1 <= cap and type(costs) == "table"
	      and tonumber(costs[host]) ~= nil
	      and tonumber(costs[host]) <= bank do
		host = host + 1;
	end
	-- Standard-speed cumulative price of n votes: 5n(n-1).
	local standard = 1;
	while standard + 1 <= cap and 5 * (standard + 1) * standard <= bank do
		standard = standard + 1;
	end
	local votes = (host < standard) and host or standard;
	return votes, host, standard;
end

-- ★★★★ GREAT PEOPLE MUST BE SPENT, NOT PARKED.
--
-- Measured on run civvis-20260801T224944Z: five Great People — three Writers and
-- an Artist stacked on the capital's own centre, a Merchant one district over —
-- each standing on ONE plot from the turn it was earned to the turn limit
-- (t70→t251, full movement every sighting). Nothing in either half of the loop
-- could act: CIVVIS's mirror drops `UNIT_GREAT_*` by design (its model banks a
-- Great Person's effect at recruit — `Action::RecruitGreatPerson` applies
-- `named_great_person_effect` with no walking unit), and this agent had zero
-- great-person code, so the units fell through every ladder to a skip.
--
-- ⚠ BE HONEST ABOUT WHAT THIS IS: walking to a legal plot and pressing Activate
-- is an actuation formality of Civilization VI — the same class as
-- FOUND_CITY-before-MOVE_TO — because the decision (acquire this Great Person)
-- was already taken upstream. The legal plots are the ENGINE's own answer
-- (`GetActivationHighlightPlots`, the call the shipped SelectedUnit.lua shades
-- the map with), so this cannot invent a target the game would refuse, and the
-- engine is asked (`CanStartCommand`) before Activate is claimed. Counted apart
-- from `applied` (`gp_activated` / `gp_moving` / `gp_idle`), so telemetry never
-- presents it as CIVVIS's work.
local gpPending = {};      -- unit id -> {x, y} last reported walk target
local gpIdleReported = {}; -- unit id -> turn the last `idle` event was emitted
local gpApiMissing = false;

local function greatPersonOf(unit)
	return try(function()
		local gp = unit:GetGreatPerson();
		if gp ~= nil and gp:IsGreatPerson() then return gp; end
		return nil;
	end, nil);
end

local function gpName(gp)
	local individual = try(function()
		local row = GameInfo.GreatPersonIndividuals[gp:GetIndividual()];
		return row and row.GreatPersonIndividualType or nil;
	end, nil);
	local class = try(function()
		local row = GameInfo.GreatPersonClasses[gp:GetClass()];
		return row and row.GreatPersonClassType or nil;
	end, nil);
	return individual or "GP_INDIVIDUAL_UNKNOWN", class or "GP_CLASS_UNKNOWN";
end

-- Drive one Great Person toward being used. Returns "activated" | "moving" |
-- "retired" | "idle", or nil when the unit is not a Great Person this code
-- should touch.
local function orderGreatPerson(player, unit, id, turn)
	local gp = greatPersonOf(unit);
	if gp == nil then
		-- ⚠ Distinguish "not a Great Person" from "the accessor is missing in
		-- this context" — the `revealed_api` lesson. Emitted once per run.
		if not gpApiMissing then
			local name = unitTypeName(unit) or "";
			if name:find("^UNIT_GREAT_") ~= nil then
				gpApiMissing = true;
				emit("gp", { turn = turn, unit = id, action = "api_missing",
					kind_name = name });
			end
		end
		return nil;
	end
	local individual, class = gpName(gp);
	-- ⚠ A Prophet's activation opens the religion chooser, a modal this harness
	-- does not answer yet — a stalled run loses more than an unspent Prophet.
	-- Deferred, visibly, until that screen has a handler.
	if class == "GREAT_PERSON_CLASS_PROPHET" then
		-- Founding consumes the Prophet's useful action, but this build can leave
		-- the zero-charge unit on the map after the religion is created.  It can no
		-- longer activate or spread; retire it only after the engine confirms both
		-- facts, through the shipped UnitPanel's own delete gate (`commandUnit`
		-- with `loose`; the strict gate refused every DELETE ever asked).
		local religionCreated = try(function()
			return player:GetReligion():GetReligionTypeCreated();
		end, -1);
		local charges = try(function() return gp:GetActionCharges(); end, -1);
		if religionCreated ~= nil and religionCreated >= 0 and charges == 0
				and commandUnit(unit, CMD["UNITCOMMAND_DELETE"], true) then
			gpPending[id] = nil;
			gpIdleReported[id] = turn;
			emit("gp", { turn = turn, unit = id, individual = individual,
				class = class, action = "retired_founded_prophet" });
			return "retired";
		end
		if gpIdleReported[id] == nil then
			gpIdleReported[id] = turn;
			emit("gp", { turn = turn, unit = id, individual = individual,
				class = class, action = "deferred_prophet" });
		end
		return "idle";
	end
	-- 1. If the engine will take Activate here and now, press it.
	if commandUnit(unit, CMD["UNITCOMMAND_ACTIVATE_GREAT_PERSON"]) then
		gpPending[id] = nil;
		emit("gp", { turn = turn, unit = id, individual = individual,
			class = class, action = "activated",
			x = try(function() return unit:GetX(); end, -1),
			y = try(function() return unit:GetY(); end, -1) });
		return "activated";
	end
	-- 2. Otherwise walk toward the nearest plot where the work can actually
	-- land. ⚠ THE ENGINE'S HIGHLIGHT IS A PLACE, NOT A PROMISE:
	-- `GetActivationHighlightPlots` names a cultural person's districts
	-- whether or not a compatible Great Work slot is free, and "nearest
	-- highlight" wedged eleven people on one slotless tile at (25,23) for the
	-- whole of run civvis-20260817T010950Z while six Amphitheaters with twelve
	-- empty slots stood 2-10 tiles away — the run ended with ZERO works. So
	-- rank the highlights with `CivvisGreatWorks`: a tile with a matching
	-- empty slot beats an unknown tile (a wonder's — its building names no
	-- district), and a district known to hold NO matching empty slot is never
	-- walked to at all. Non-cultural classes and unreadable slot tables keep
	-- the old nearest-highlight behaviour.
	local plots = try(function() return gp:GetActivationHighlightPlots(); end, nil);
	local slotCount = nil;
	if type(plots) == "table" and #plots > 0 then
		local survey = CivvisGreatWorks.survey(player, turn);
		local openPlots;
		slotCount, openPlots = CivvisGreatWorks.matches(survey,
			CivvisGreatWorks.objectsFor(individual, class));
		local ux = try(function() return unit:GetX(); end, nil);
		local uy = try(function() return unit:GetY(); end, nil);
		local bestX, bestY, bestD, bestRank = nil, nil, nil, nil;
		for _, idx in ipairs(plots) do
			local plot = try(function() return Map.GetPlotByIndex(idx); end, nil);
			if plot ~= nil and ux ~= nil then
				local rank = 1;
				if openPlots ~= nil then
					if openPlots[idx] then rank = 0;
					elseif survey.district_plots[idx] then rank = 2; end
				end
				-- A slot consumer with zero matching empty slots anywhere has
				-- no tile worth reaching: marching is motion without progress,
				-- and the mirror's needs machinery — not this walk — is what
				-- builds capacity. Fall through to the idle report instead.
				if rank < 2 and (slotCount == nil or slotCount > 0) then
					local px, py = plot:GetX(), plot:GetY();
					local d = try(function()
						return Map.GetPlotDistance(ux, uy, px, py);
					end, 9999);
					if bestRank == nil or rank < bestRank
							or (rank == bestRank and d < bestD) then
						bestX, bestY, bestD, bestRank = px, py, d, rank;
					end
				end
			end
		end
		if bestX ~= nil then
			local params = {};
			params[UnitOperationTypes.PARAM_X] = bestX;
			params[UnitOperationTypes.PARAM_Y] = bestY;
			if operate(unit, OP["UNITOPERATION_MOVE_TO"], params) then
				-- Report a walk target once, not every step of the walk. A
				-- refused MOVE_TO (e.g. a sibling Great Person holds the plot)
				-- falls through to `idle` and retries next turn.
				local pend = gpPending[id];
				if pend == nil or pend.x ~= bestX or pend.y ~= bestY then
					-- `open_slot` says whether the target tile is KNOWN to
					-- hold a matching empty slot; absent when the survey
					-- could not say (non-cultural classes, wonder tiles).
					local openKnown = nil;
					if openPlots ~= nil then openKnown = (bestRank == 0); end
					emit("gp", { turn = turn, unit = id, individual = individual,
						class = class, action = "moving", x = bestX, y = bestY,
						dist = bestD, open_slot = openKnown });
				end
				gpPending[id] = { x = bestX, y = bestY };
				return "moving";
			end
		end
	end
	-- 3. Nowhere legal to activate — no empty Great Work slot, no qualifying
	-- district built yet, or the one legal plot is occupied. A real constraint,
	-- reported sparsely; the unit stays put and is retried every turn.
	-- `empty_slots` rides along (absent when unknowable) so an idle Writer
	-- with slots on the board reads as the driver's failure, not the empire's.
	gpPending[id] = nil;
	local before = gpIdleReported[id];
	if before == nil or (turn - before) >= 25 then
		gpIdleReported[id] = turn;
		emit("gp", { turn = turn, unit = id, individual = individual,
			class = class, action = "idle",
			empty_slots = slotCount,
			x = try(function() return unit:GetX(); end, -1),
			y = try(function() return unit:GetY(); end, -1) });
	end
	return "idle";
end

-- ⚠ EMIT THE NUMERATOR AND THE DENOMINATOR. Four of this project's defects were
-- caught by a count and would have passed a boolean "is it wired up" check. An
-- `orders` event that says only "CIVVIS decided" reads identical whether every
-- order landed or every one was refused.
-- ------------------------------------------------------ per-unit order queue
--
-- ★★★★★ ONE ORDER PER UNIT PER TURN WAS THE PRICE OF ASYNCHRONOUS ACTUATION.
--
-- `UnitManager.RequestOperation` returns before the unit has moved, so a list
-- like `MOVE_TO a; RANGE_ATTACK t` applied in one callback aims the shot from
-- where the unit STOOD, not from `a`. `civvis_orders::coalesce_unit_paths`
-- answered that by sending a unit's walk and deferring every later order to the
-- next frame — correct causally, and it deferred every strike that follows a
-- step. The joint tactical search's lines are literally `[Move, Attack]`
-- (`src/ai/tactics.rs`), so on this bridge they executed as a step: the unit
-- walked into contact and stood there, unstruck, for the enemy's whole turn.
-- Measured on run civvis-20260803T005930Z: 7 melee ATTACK orders against 1546
-- MOVE_TO in 188 turns of war; 622 of 1787 military unit-turns hovering 2-4
-- hexes from a target. Twelve finished live games in the host's Hall of Fame:
-- 343 units lost, 61 killed.
--
-- This queue keeps the whole per-unit sequence and runs it IN ORDER, each
-- order once the one before it has done what it meant to do: the unit stands
-- where the move was aimed, or has no movement left, or the host says its
-- operation is over (`UnitMoveComplete` / `UnitOperationDeactivated`), or a
-- grace period ran out. Every order still passes `canOperate` inside
-- `applyOrder`, so a step the host cut short refuses the strike by name
-- rather than firing it from the wrong tile.
--
-- ⚠⚠⚠ THE UNIT HANDLE IS RE-RESOLVED ON EVERY DRAIN, NEVER CACHED — see the
-- SIGSEGV note on `applyOrder`. A queued unit that died in its own strike is a
-- named refusal (`unit_gone:<id>`), not a freed pointer.
--
-- ⚠ THE FLOOR: `settleTurn` holds the turn open while a queue is pending, but
-- only for `OrderQueueMaxTicks`. Past that the rest is refused as
-- `queue_stalled` and the turn ends. A wedged operation costs decision
-- quality, never progress.
--
-- One bare global table: the main chunk sits two slots under Lua 5.1's
-- 200-local ceiling (see the CI headroom check), so nothing here is a `local`.
CivvisQueue = {
	pending = {},   -- host unit id -> { rows, next, expect, ready, wait }
	order = {},     -- unit ids in the order they were first queued
	count = 0,
	turn = -1,
	ticks = 0,
	stats = { applied = 0, refused = 0, refusals = {}, strikes_landed = 0,
	          strikes_planned = 0, queued = 0 },
};

CivvisQueue.reset = function(turn)
	local q = CivvisQueue;
	if q.count > 0 then
		-- Leftovers from a turn that ended under us: name them, once.
		local left = 0;
		for _, entry in pairs(q.pending) do left = left + (#entry.rows - entry.next + 1); end
		q.stats.refused = q.stats.refused + left;
		q.stats.refusals.queue_turn_over = (q.stats.refusals.queue_turn_over or 0) + left;
		CivvisQueue.report(q.turn, "turn_over");
	end
	q.pending = {}; q.order = {}; q.count = 0; q.turn = turn; q.ticks = 0;
	q.stats = { applied = 0, refused = 0, refusals = {}, strikes_landed = 0,
	            strikes_planned = 0, queued = 0 };
end;

-- The position an order should leave its unit at, when the next order must
-- be issued from there. A melee ATTACK ends on the target only if the
-- defender dies, so it carries no expectation and relies on the host's
-- deactivation event or the grace period.
CivvisQueue.expectFor = function(row)
	local verb = tostring(row.verb or "");
	local x, y = tonumber(row.x), tonumber(row.y);
	if verb == "MOVE_TO" and x ~= nil and y ~= nil then return { x = x, y = y }; end
	return nil;
end;

CivvisQueue.isStrike = function(row)
	local verb = tostring(row.verb or "");
	return verb == "ATTACK" or verb == "RANGE_ATTACK";
end;

CivvisQueue.push = function(subject, row, expect)
	local q = CivvisQueue;
	local entry = q.pending[subject];
	if entry == nil then
		entry = { rows = {}, next = 1, expect = expect, ready = false, wait = 0 };
		q.pending[subject] = entry;
		q.order[#q.order + 1] = subject;
	end
	entry.rows[#entry.rows + 1] = row;
	q.count = q.count + 1;
	q.stats.queued = q.stats.queued + 1;
	if CivvisQueue.isStrike(row) then q.stats.strikes_planned = q.stats.strikes_planned + 1; end
end;

CivvisQueue.pendingCount = function() return CivvisQueue.count; end;

CivvisQueue.refuseRest = function(subject, entry, why)
	local q = CivvisQueue;
	local left = #entry.rows - entry.next + 1;
	if left > 0 then
		q.stats.refused = q.stats.refused + left;
		q.stats.refusals[why] = (q.stats.refusals[why] or 0) + left;
	end
	q.count = q.count - left;
	q.pending[subject] = nil;
end;

-- A host event for one of our units: the move finished or the operation
-- deactivated. Mark it ready; the drain on the next tick issues its next order.
CivvisQueue.noteUnitEvent = function(pid, player, unitId)
	local q = CivvisQueue;
	if q.count <= 0 or player ~= pid then return false; end
	local entry = q.pending[tonumber(unitId) or -1];
	if entry == nil then return false; end
	entry.ready = true;
	return true;
end;

CivvisQueue.report = function(turn, why)
	local q = CivvisQueue;
	emit("orders_queue", {
		turn = turn, why = why, applied = q.stats.applied, refused = q.stats.refused,
		refusals = q.stats.refusals, queued = q.stats.queued,
		strikes_planned = q.stats.strikes_planned,
		strikes_landed = q.stats.strikes_landed, waited = q.ticks,
	});
end;

-- Issue at most one queued order per unit whose previous order has settled.
-- Returns how many orders ran on this call.
CivvisQueue.drain = function(player, pid, turn)
	local q = CivvisQueue;
	if q.count <= 0 then return 0; end
	-- The host's own `UnitMoveComplete` / `UnitOperationDeactivated` mark a
	-- unit ready the moment its order settles; the grace period is only the
	-- fallback for a host that never says so, and it is deliberately long —
	-- an early strike is refused out of range, a late one merely waits.
	local grace = tonumber(cfg.OrderQueueGraceTicks) or 30;
	local ran = 0;
	for _, subject in ipairs(q.order) do
		local entry = q.pending[subject];
		if entry ~= nil then
			local unit = liveUnit(pid, subject);
			if unit == nil then
				CivvisQueue.refuseRest(subject, entry, "unit_gone:" .. tostring(subject));
			else
				entry.wait = entry.wait + 1;
				local ux = tonumber(try(function() return unit:GetX(); end, -1)) or -1;
				local uy = tonumber(try(function() return unit:GetY(); end, -1)) or -1;
				local moves = tonumber(try(function() return unit:GetMovesRemaining(); end, nil));
				local arrived = entry.expect == nil
					or (ux == entry.expect.x and uy == entry.expect.y);
				local spent = moves ~= nil and moves <= 0;
				local ready = entry.ready or arrived or spent or entry.wait >= grace;
				if ready then
					local row = entry.rows[entry.next];
					local verb = tostring(row.verb or "");
					if spent and (verb == "MOVE_TO" or CivvisQueue.isStrike(row)) then
						-- Nothing that needs movement can run; say why, don't ask.
						CivvisQueue.refuseRest(subject, entry, "queue_no_moves");
					else
						local ok, why = false, "throw";
						local safe, res1, res2 = pcall(function()
							return applyOrder(player, pid, row, turn);
						end);
						if safe then ok, why = res1, res2; end
						ran = ran + 1;
						q.count = q.count - 1;
						if ok then
							q.stats.applied = q.stats.applied + 1;
							if CivvisQueue.isStrike(row) then
								q.stats.strikes_landed = q.stats.strikes_landed + 1;
							end
						else
							q.stats.refused = q.stats.refused + 1;
							local key = tostring(why);
							q.stats.refusals[key] = (q.stats.refusals[key] or 0) + 1;
						end
						entry.next = entry.next + 1;
						if entry.next > #entry.rows then
							q.pending[subject] = nil;
						else
							entry.expect = ok and CivvisQueue.expectFor(row) or nil;
							entry.ready = false;
							entry.wait = 0;
						end
					end
				end
			end
		end
	end
	if q.count <= 0 then
		q.pending = {}; q.order = {};
		CivvisQueue.report(turn, "drained");
	end
	return ran;
end;

-- Past the cap the queue is abandoned by name and the turn may end.
CivvisQueue.giveUp = function(turn)
	local q = CivvisQueue;
	for subject, entry in pairs(q.pending) do
		CivvisQueue.refuseRest(subject, entry, "queue_stalled");
	end
	q.pending = {}; q.order = {}; q.count = 0;
	CivvisQueue.report(turn, "stalled");
end;

-- ★★★★ A HELD SOLDIER IS NOT AN IDLE ONE. `applyOrders` hands every combat
-- unit CIVVIS did not mention to `UNITOPERATION_AUTOMATE_EXPLORE`, which was
-- right for a peacetime army parked in its capital and wrong for the one
-- CIVVIS meant to hold in contact — a hold produces no order, and the host's
-- automation then walked the unit wherever it liked. Visible hostile combat
-- units and at-war cities within `ExploreGuardRadius` tiles keep the unit
-- where CIVVIS left it. Computed once per turn, only when the hand-off asks.
CivvisQueue.contactPlots = function(pid, turn)
	local q = CivvisQueue;
	if q.contactTurn == turn and q.contacts ~= nil then return q.contacts; end
	local plots = {};
	local diplomacy = try(function() return Players[pid]:GetDiplomacy(); end);
	local visible = function(x, y)
		return try(function() return PlayersVisibility[pid]:IsVisible(x, y); end, false) == true;
	end
	local combatUnit = function(unit)
		return try(function()
			local row = GameInfo.Units[unit:GetUnitType()];
			return row ~= nil and ((row.Combat or 0) > 0 or (row.RangedCombat or 0) > 0);
		end, false) == true;
	end
	local addUnits = function(other)
		pcall(function()
			for _, unit in other:GetUnits():Members() do
				local ux, uy = unit:GetX(), unit:GetY();
				if visible(ux, uy) and combatUnit(unit) then
					plots[#plots + 1] = { x = ux, y = uy };
				end
			end
		end);
	end
	pcall(function()
		for _, oid in ipairs(PlayerManager.GetAliveIDs() or {}) do
			if oid ~= pid then
				local other = Players[oid];
				local barbarian = try(function() return other:IsBarbarian(); end, false) == true;
				local free = try(function()
					return other.IsFreeCities ~= nil and other:IsFreeCities() == true;
				end, false) == true;
				local atWar = diplomacy ~= nil
					and try(function() return diplomacy:IsAtWarWith(oid); end, false) == true;
				if other ~= nil and (barbarian or free or atWar) then
					addUnits(other);
					if atWar then
						pcall(function()
							for _, city in other:GetCities():Members() do
								local cx, cy = city:GetX(), city:GetY();
								if visible(cx, cy) then plots[#plots + 1] = { x = cx, y = cy }; end
							end
						end);
					end
				end
			end
		end
	end);
	q.contacts = plots;
	q.contactTurn = turn;
	return plots;
end;

CivvisQueue.inContact = function(pid, unit, turn)
	local radius = tonumber(cfg.ExploreGuardRadius) or 4;
	local ux = tonumber(try(function() return unit:GetX(); end, -1)) or -1;
	local uy = tonumber(try(function() return unit:GetY(); end, -1)) or -1;
	if ux < 0 then return false; end
	for _, plot in ipairs(CivvisQueue.contactPlots(pid, turn)) do
		local d = tonumber(try(function() return Map.GetPlotDistance(ux, uy, plot.x, plot.y); end, -1)) or -1;
		if d >= 0 and d <= radius then return true; end
	end
	return false;
end;

-- --------------------------------------------------------- host-grounded board
--
-- ★★★★★ THE BOARD PLANNED MOVEMENT THE UNIT DID NOT HAVE. `mirror_unit_moves`
-- handed every mirrored unit its full allowance every turn, because the
-- export's `moves` had misled twice — and it misled because the host had
-- already spent the movement before the brain could act: a `MOVE_TO` whose
-- host path ran longer than CIVVIS priced was QUEUED, and the host walked the
-- unit along it at the start of the next turn, before `beginTurn` exports.
-- Turn 31 of run civvis-20260730T120107Z: 7 of 8 units at `moves: 0` at the
-- start of the turn. Measured across the recorded runs: 12.5 % of MOVE_TOs
-- did not move at all, most of them with movement showing at export.
--
-- Two rules make `moves` mean "movement available this turn", and the `seat`
-- event says so (`moves_at_turn_start`) so the mirror may trust it:
--   * every MOVE_TO is CAPPED to the furthest plot on the host's own path that
--     the unit reaches THIS turn (`UnitManager.GetMoveToPathEx(unit, dest)`
--     gives `plots` and `turns`; the shipped WorldInput draws the same path);
--     a walk that would take two turns is sent as its first turn's leg, and
--     the brain re-plans the rest from the real position next turn — no path
--     is left queued to walk the unit somewhere stale;
--   * combat units that enter the turn with a queued destination anyway (an
--     older order, the fallback ladder, explore automation) get
--     `UNITCOMMAND_CANCEL` at turn start, so the brain owns them from the next
--     turn on. Civilians keep theirs: a settler's long walk is exactly what a
--     queued path is for.
-- Both are counted (`move_capped`, `queued_paths`) so a run says how often the
-- host and the board disagreed. One bare global table (200-local ceiling).
CivvisBoard = { stats = { capped = 0, no_reach = 0 } };

CivvisBoard.reset = function()
	CivvisBoard.stats = { capped = 0, no_reach = 0 };
end;

-- The furthest plot on the host's path to (x, y) that `unit` reaches this
-- turn. Returns nil when the whole path lands this turn (no cap), false and a
-- reason when the unit cannot take even the first step, or the capped plot.
CivvisBoard.capToTurn = function(unit, x, y)
	local path = try(function()
		return UnitManager.GetMoveToPathEx(unit, Map.GetPlotIndex(x, y));
	end, nil);
	if path == nil or path.plots == nil or path.turns == nil then return nil; end
	local n = 0;
	for _ in pairs(path.plots) do n = n + 1; end
	if n <= 1 then return nil; end
	local last = tonumber(path.turns[n]);
	if last == nil or last <= 1 then return nil; end
	local reach = nil;
	for i = 2, n do
		local t = tonumber(path.turns[i]);
		if t ~= nil and t <= 1 then reach = path.plots[i]; end
	end
	if reach == nil then return false, "no_moves_this_turn"; end
	local plot = try(function() return Map.GetPlotByIndex(reach); end, nil);
	if plot == nil then return nil; end
	local cx = tonumber(try(function() return plot:GetX(); end, nil));
	local cy = tonumber(try(function() return plot:GetY(); end, nil));
	if cx == nil or cy == nil then return nil; end
	return { x = cx, y = cy, turns = last };
end;

-- Cancel queued paths on combat units at the start of our turn, and report
-- how many units entered the turn with one at all.
CivvisBoard.cancelQueuedPaths = function(player, pid, turn)
	local found, cancelled = 0, 0;
	eachUnit(player, function(unit)
		local queued = try(function() return UnitManager.GetQueuedDestination(unit); end, nil);
		if queued == nil then return; end
		found = found + 1;
		local combat = try(function()
			local row = GameInfo.Units[unit:GetUnitType()];
			return row ~= nil and ((row.Combat or 0) > 0 or (row.RangedCombat or 0) > 0);
		end, false) == true;
		if not combat then return; end
		local hash = CMD["UNITCOMMAND_CANCEL"];
		if hash == nil then return; end
		local ok = try(function()
			return UnitManager.CanStartCommand(unit, hash, false, true) == true;
		end, false);
		if ok and pcall(function() UnitManager.RequestCommand(unit, hash); end) then
			cancelled = cancelled + 1;
		end
	end);
	if found > 0 then
		emit("queued_paths", { turn = turn, found = found, cancelled = cancelled });
	end
end;

-- ------------------------------------------------------- mid-turn combat frame
--
-- ★★★★ THE PLAN IS COMPUTED ONCE, BEFORE THE HOST HAS ROLLED A SINGLE DIE.
-- Every strike of the turn is planned against the opening board with the
-- engine's own rolls; the host's roll differs (it has left "sure" kills alive
-- at 1, 3, 6, 8, 16 and 20 HP), and the next export is next turn. A combat
-- frame closes that gap once per turn: after the opening orders and their
-- per-unit queue have settled, if any strike was issued, the board is
-- exported again with `frame = 1`, the brain re-plans the SAME turn on it
-- (units that acted show the movement and attacks they have left, targets
-- show the damage they took), and the answer is applied like the opening one.
--
-- ⚠ Default OFF (`CombatFrames = 0`) until a live run has been read: a second
-- round trip per contact turn is a second place for the loop to wedge, and
-- the round trip is the one thing this file cannot test offline. The frame
-- wait has its own short budget (`CombatFramePolls`) and no fallback ladder:
-- past it the frame is abandoned by name and the turn ends as it always did.
-- One bare global table (200-local ceiling).
CivvisFrames = { current = 0, strikes = 0, exported = false };

CivvisFrames.reset = function()
	CivvisFrames.current = 0;
	CivvisFrames.strikes = 0;
	CivvisFrames.exported = false;
end;

-- Called from CivvisLedger.strike for every strike issued, opening or queued.
CivvisFrames.noteStrike = function()
	CivvisFrames.strikes = CivvisFrames.strikes + 1;
end;

CivvisFrames.max = function()
	return tonumber(cfg.CombatFrames) or 0;
end;

-- Whether another frame should open now: frames are enabled, the cap is not
-- reached, and a strike was issued since the last board went out.
CivvisFrames.wanted = function()
	return CivvisFrames.max() > 0
		and CivvisFrames.current < CivvisFrames.max()
		and CivvisFrames.strikes > 0;
end;

-- Open the next frame: export the board again, stamped, and re-arm the
-- handshake so `settleTurn` waits for this frame's answer.
CivvisFrames.begin = function(player, pid, turn)
	CivvisFrames.current = CivvisFrames.current + 1;
	local strikes = CivvisFrames.strikes;
	CivvisFrames.strikes = 0;
	awaiting.frame = CivvisFrames.current;
	awaiting.done = false;
	awaiting.polls = 0;
	awaiting.ticks = 0;
	awaiting.source = "pending";
	emit("combat_frame", { turn = turn, frame = CivvisFrames.current, strikes = strikes });
	pcall(function() exportState(player, pid, turn, CivvisFrames.current); end);
end;

local function applyOrders(player, pid, turn, rows)
	local applied, refused, deferred = 0, 0, 0;
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
			if kind == "produce_next" then
				-- A lease is accepted by the control channel but has not yet
				-- mutated the host. Keep it out of the host applied-rate numerator
				-- and denominator; the later `build` event is the actuation proof.
				deferred = deferred + 1;
			else
				applied = applied + 1;
			end
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
		return ok, why;
	end

	-- A found refused here because the settler is not on its site yet is
	-- queued again behind that settler's walk (see the queue below), so a
	-- settler with movement to spare after arriving founds THIS turn instead
	-- of standing a full enemy turn on the frontier with 0 moves left.
	local foundRetry = {};
	for index, row in ipairs(rows) do
		if tostring(row.kind or "") == "unit" and tostring(row.verb or "") == "FOUND_CITY" then
			local ok = runOrder(index, row);
			local subject = tonumber(row.subject);
			if not ok and subject ~= nil then foundRetry[subject] = row; end
		end
	end
	-- ★★★★★ CHANGE GOVERNMENT BEFORE SLOTTING THE DECK THAT FITS IT. Like
	-- FOUND_CITY above, this is an ACTUATION RULE of Civilization VI and not a
	-- decision: a government defines the slot SHAPE, so a deck chosen for the
	-- new government is illegal under the old one and the host refuses the
	-- whole thing.
	--
	-- Caught by the `policy_deck_refused` instrument (#1222) on its first live
	-- game, turn 183:
	--
	--   card with nowhere to go : POLICY_WISSELBANKEN (SLOT_DIPLOMATIC)
	--   slots the host had      : MILITARY,MILITARY,ECONOMIC,ECONOMIC,
	--                             DIPLOMATIC,WILDCARD        <- Theocracy
	--   deck CIVVIS asked for   : ECONOMIC x3, MILITARY x1, DIPLOMATIC x2
	--                                                        <- Merchant Republic
	--
	-- and the export shows the government flipping THEOCRACY -> MERCHANT_REPUBLIC
	-- on turn 184, one turn later. CIVVIS was right about both; the two orders
	-- were simply applied in the wrong order.
	--
	-- ⚠ Not a CIVVIS-side bug. `policies_fit` and `revise_policy_deck` both seat
	-- correctly, and `data/governments.json` matches `Government_SlotCounts` for
	-- all 13 governments — checked before touching anything, because the obvious
	-- read is that the deck chooser is broken and it is not.
	for index, row in ipairs(rows) do
		if not ordered[index] and tostring(row.kind or "") == "government" then
			runOrder(index, row);
		end
	end
	-- ★★★★★ THE FIRST ORDER PER UNIT RUNS NOW; EVERY LATER ONE IS QUEUED behind
	-- it and issued once the earlier order has done what it meant to do — see
	-- `CivvisQueue`. Before this, `civvis_orders` deferred those follow-ups to
	-- the next turn, which is how a planned move-then-strike became a move.
	-- A unit whose first order the host refused gets no follow-up: the rest of
	-- its sequence was planned from a tile it never reached, and is refused by
	-- name rather than fired from the wrong one.
	local queueOn = cfg.OrderQueue ~= false;
	local firstRun, firstRefused = {}, {};
	for index, row in ipairs(rows) do
		if not ordered[index] then
			local subject = tonumber(row.subject);
			local isUnit = tostring(row.kind or "") == "unit" and subject ~= nil;
			if queueOn and isUnit and firstRun[subject] then
				ordered[index] = true;
				if firstRefused[subject] then
					refused = refused + 1;
					whyNot.queue_prior_refused = (whyNot.queue_prior_refused or 0) + 1;
				else
					CivvisQueue.push(subject, row, firstRun[subject].expect);
				end
			else
				local ok = runOrder(index, row);
				if isUnit then
					firstRun[subject] = { expect = ok and CivvisQueue.expectFor(row) or nil };
					if not ok then firstRefused[subject] = true; end
					if queueOn and ok and foundRetry[subject] ~= nil
							and tostring(row.verb or "") == "MOVE_TO" then
						CivvisQueue.push(subject, foundRetry[subject], firstRun[subject].expect);
						foundRetry[subject] = nil;
					end
				end
			end
		end
	end

	-- Great People go first, before the explore handoff: they cannot explore,
	-- and CIVVIS cannot mention them — the mirror drops `UNIT_GREAT_*` by
	-- design. See `orderGreatPerson` for what this is and is not.
	local gpActivated, gpMoving, gpRetired, gpIdle = 0, 0, 0, 0;
	local gpHandled = {};
	if cfg.GreatPeopleUse ~= false then
		eachUnit(player, function(unit)
			local id = try(function() return unit:GetID(); end, -1);
			if id == -1 then return; end
			local acted = orderGreatPerson(player, unit, id, turn);
			if acted == nil then return; end
			gpHandled[id] = true;
			if acted == "activated" then gpActivated = gpActivated + 1;
			elseif acted == "moving" then gpMoving = gpMoving + 1;
			elseif acted == "retired" then gpRetired = gpRetired + 1;
			else gpIdle = gpIdle + 1; end
		end);
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
	local explored, guarded = 0, 0;
	-- ⚠ NEVER on a combat frame: every unit not named by the frame's answer
	-- was ordered by the opening board and is exactly where CIVVIS left it.
	if cfg.ExploreUnassigned ~= false and (awaiting.frame or 0) == 0 then
		local mentioned = {};
		for _, row in ipairs(rows) do
			if tostring(row.kind or "") == "unit" then
				mentioned[tonumber(row.subject) or -1] = true;
			end
		end
		eachUnit(player, function(unit)
			local id = try(function() return unit:GetID(); end, -1);
			if id == -1 or mentioned[id] or gpHandled[id] then return; end
			local name = unitTypeName(unit);
			-- Civilians cannot explore, and a settler that wanders is a settler that
			-- never founds — this project has already paid for both.
			local gp = try(function() return unit:GetGreatPerson(); end);
			if name == "UNIT_SETTLER" or name == "UNIT_BUILDER"
					or name == "UNIT_TRADER"
					or (gp ~= nil and try(function() return gp:IsGreatPerson(); end, false)) then
				return;
			end
			-- A held soldier stays held: see `CivvisQueue.contactPlots`.
			if cfg.ExploreGuard ~= false and CivvisQueue.inContact(pid, unit, turn) then
				guarded = guarded + 1;
				return;
			end
			if operate(unit, OP["UNITOPERATION_AUTOMATE_EXPLORE"], {}) then
				explored = explored + 1;
			end
		end);
	end

	emit("orders", {
		turn = turn, frame = awaiting.frame or 0, source = "civvis", seen = #rows - deferred,
		applied = applied, refused = refused, by = byKind, refusals = whyNot,
		deferred = deferred,
		-- Not part of `applied`: these are units CIVVIS said nothing about.
		explored = explored,
		-- Unmentioned combat units kept off the explore automation because a
		-- hostile stood within `ExploreGuardRadius`; see `CivvisQueue.inContact`.
		explore_guarded = guarded,
		-- MOVE_TOs sent as this turn's leg of a longer host path, and moves
		-- refused because the unit could not take even the first step this
		-- turn. See CivvisBoard.
		move_capped = CivvisBoard.stats.capped,
		move_no_reach = CivvisBoard.stats.no_reach,
		-- Follow-up orders waiting in the per-unit queue; their outcome lands
		-- in this turn's `orders_queue` event, not in `applied` above.
		queued = CivvisQueue.pendingCount(),
		-- Also not part of `applied`: Great People driven to their own use —
		-- an actuation formality, not a CIVVIS decision. See `orderGreatPerson`.
		gp_activated = gpActivated,
		gp_moving = gpMoving,
		gp_retired = gpRetired,
		gp_idle = gpIdle,
		-- Orders the engine ACCEPTED that left the unit farther from where it was
		-- sent. Counted apart from `refused` on purpose: a refusal is the bridge
		-- working and being told no; this is the bridge reporting success for
		-- something that did not happen.
		missed = missed,
		-- See `upgradeUnit`. `tried` is every combat unit reached this turn and
		-- `blocked` is what the engine would not upgrade, by unit type, so a run that
		-- fields Ancient units in 1100 AD can finally say WHY. Gold rides along
		-- because "no gold" is the first hypothesis and the cheapest to eliminate.
		upgrade_tried = upgradeTried,
		upgrade_blocked = upgradeBlocked,
		-- See `upgradeUnit`: the engine's own FAILURE_REASONS per blocked type.
		upgrade_blocked_why = upgradeBlockedWhy,
		upgrade_gold = try(function()
			return math.floor(player:GetTreasury():GetGoldBalance());
		end, -1),
	});
	upgradeTried, upgradeBlocked, upgradeBlockedWhy = 0, {}, {};

	-- ⚠⚠ A CIVVIS TURN MUST STILL EMIT A `turn` RECORD. The full one lives at the
	-- end of `playTurn`, which no longer runs when CIVVIS is deciding — so run
	-- civvis-20260730T110209Z produced ZERO turn records and every progress check,
	-- mine and the harness's, went blind. The harness's `[turn N]` lines come from
	-- this event, so its absence also disabled the stall watchdog's only clock.
	--
	-- Leaner than the heuristic path's record on purpose: the fields it omits
	-- (`war_blocked`) describe built-ins that did not run.
	-- One turn record per turn: a combat frame's answer is part of the same
	-- turn, and the harness reads `turn` records as its clock.
	if (awaiting.frame or 0) > 0 then return applied; end
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
		orders_seen = #rows - deferred,
		orders_applied = applied,
		orders_refused = refused,
		orders_deferred = deferred,
		orders_polls = awaiting.polls,
		residual = residualAnswers,
		blocker = blockerName(currentBlocker(pid)),
	});
	return applied;
end
-- Exposed for the offline order-queue regression (order_queue_test.lua).
CivvisApplyOrders = applyOrders;

-- Publish the board and open the window in which CIVVIS answers.
--
-- Split out of `playTurn` because the export must happen whether or not CIVVIS
-- replies: it is the only thing the brain has to read, so skipping it on a
-- fallback turn would guarantee the next turn falls back too.
-- Emit a `city_lost` for every city that was ours last turn and is not now,
-- carrying the condition it was in when we last held it.
--
-- ⚠ Deliberately compares against the PREVIOUS TURN's roster rather than a
-- periodic export. A city taken between two exports is invisible to the diff
-- that was being used before; at turn resolution it cannot be.
--
-- ⚠ `loyalty` and `damage` are the condition ONE TURN BEFORE the loss, which is
-- the honest thing to report: the turn it fell we no longer own it and cannot
-- read it. A city at loyalty 100 and undamaged the turn before it went was
-- almost certainly taken by force — half of all losses look like that.
local function reportLostCities(player, pid, turn)
	local cities = try(function() return player:GetCities(); end);
	if cities == nil then return; end
	local now = {};
	for _, city in cities:Members() do
		local id = try(function() return city:GetID(); end);
		if id ~= nil then
			local x = try(function() return city:GetX(); end, -1);
			local y = try(function() return city:GetY(); end, -1);
			-- The file's own accessors, not recalled ones. `cityLoyalty` returns
			-- nil on a Vanilla ruleset with no `GetCulturalIdentity`, and
			-- `cityDefence` reads damage off the DISTRICT at the plot — a city
			-- has no `GetDamage`, and calling one that does not exist is how
			-- every wall was once exported as pristine.
			local loyalty, perTurn, fallsTo = cityLoyalty(city);
			local _, damage, _, wallDamage = cityDefence(x, y);
			now[id] = {
				name = try(function() return Locale.Lookup(city:GetName()); end, "?"),
				pop = try(function() return city:GetPopulation(); end, -1),
				loyalty = loyalty, per_turn = perTurn, falls_to = fallsTo,
				damage = damage, wall = wallDamage,
				-- ⭐ Were we at war with ANYONE while we still held it. Damage
				-- says the city was hit; this says an enemy was entitled to hit
				-- it, and the pair is what separates a storming from a flip
				-- without inferring either. `-1` for "the accessor is missing"
				-- rather than `false`, so a broken read can never be mistaken
				-- for a peaceful game — the `#1184` convention.
				--
				-- ⚠⚠ AND THAT CONVENTION IMMEDIATELY EARNED ITS KEEP. This was
				-- first written as `GetDiplomacy():IsAtWar()`, which does not exist
				-- on the Diplomacy object, and the field came back `-1` on all SEVEN
				-- city losses of the first run that carried it. Had the fallback been
				-- `false`, seven stormings would have been recorded as "lost while at
				-- peace" — precisely the wrong conclusion, and indistinguishable from
				-- a real one.
				--
				-- The working form is the one this file already uses in four other
				-- places: `IsAtWarWith(id)` over the alive majors.
				at_war = try(function()
					local diplomacy = player:GetDiplomacy();
					if diplomacy == nil then return -1; end
					for _, otherId in ipairs(PlayerManager.GetAliveMajorIDs()) do
						if otherId ~= pid and diplomacy:IsAtWarWith(otherId) then
							return true;
						end
					end
					return false;
				end, -1),
				turn = turn,
			};
		end
	end
	-- Only report once a roster has been recorded, or turn 1 announces the loss
	-- of every city we never had.
	if next(lastRoster) ~= nil then
		for id, was in pairs(lastRoster) do
			if now[id] == nil then
				-- ⛔⛔ `falls_to` DOES NOT DISCRIMINATE, and the comment that
				-- stood here said it did. It is
				-- `GetCulturalIdentity():GetPotentialTransferPlayer()` — the
				-- player a city WOULD transfer to if its loyalty ever reached
				-- zero, which the engine fills in whenever any neighbour exerts
				-- pressure. It is not the banner's "will fall to" forecast.
				--
				-- Measured over every `city_lost` recorded to date: `falls_to`
				-- is **62 on all 16 of them**, and the state export carries the
				-- same 62 for Rome on turn 3 sitting at loyalty 100 gaining
				-- +21 a turn. It is a constant. Any split computed from it
				-- reads 100% loyalty by construction.
				--
				-- ⭐ The fields that DO discriminate are already in this event:
				-- `damage_when_held` / `wall_damage_when_held` (a city cannot be
				-- stormed without being hit) against `loyalty_when_held` and
				-- `loyalty_rate_when_held` (a loyalty flip runs the loyalty
				-- DOWN — a negative rate — and needs no damage at all). On the
				-- 16 recorded losses those two signals separate cleanly:
				-- loyalty 100 with a +12..+28 rate and 57-166 damage is a
				-- storming; 1-4 loyalty on a negative rate with zero damage is
				-- a flip.
				--
				-- ⚠ `falls_to_when_held` is still emitted, because removing a
				-- field silently is worse than recording a known-useless one —
				-- but do NOT classify with it.
				emit("city_lost", {
					turn = turn,
					city = id,
					name = was.name,
					held_until = was.turn,
					pop_when_held = was.pop,
					loyalty_when_held = was.loyalty,
					loyalty_rate_when_held = was.per_turn,
					falls_to_when_held = was.falls_to,
					damage_when_held = was.damage,
					wall_damage_when_held = was.wall,
					at_war_when_held = was.at_war,
				});
			end
		end
	end
	lastRoster = now;
end

local function beginTurn(player, pid, turn)
	-- First, before anything else this turn can change the roster.
	pcall(function() reportLostCities(player, pid, turn); end);
	if cfg.ProbeChannels then probeChannels(turn); end
	-- Every `ProbeCitizenEvery` turns rather than every turn: the answer cannot
	-- change within a game, and one line per district per city per turn would
	-- drown the log the rest of the run has to be read out of.
	if cfg.ProbeCitizens
		and (turn % (cfg.ProbeCitizenEvery or 25)) == 0 then
		probeCitizenSlots(turn);
	end
	-- Every `CampusSpecialistEvery` turns rather than every turn: a citizen the
	-- game's own governor moves back is not worth fighting over each turn, and
	-- the emit would drown the log the rest of the run is read out of.
	if cfg.CampusSpecialist
		and (turn % (cfg.CampusSpecialistEvery or 10)) == 0 then
		fillCampusSpecialists(turn);
	end
	-- ⚠⚠⚠ ENVOYS ARE SPENT ON THE TURN, NOT ON THE PROMPT. `chooseEnvoy` was
	-- reachable only from the `ENDTURN_BLOCKING_GIVE_INFLUENCE_TOKEN` handler,
	-- and that blocker is far rarer than the comment above `chooseEnvoy`
	-- assumes: an isolation run on 2026-08-04 with `EnvoyEnabled` on went **58
	-- turns without it firing once**, while holding five free envoys with four
	-- city-states met. Zero `envoy` events, so the lane could not have been
	-- measured even though it was switched on.
	--
	-- That also explains the deployment symptom on its own terms: runs finish
	-- holding 55, 56 and 69 unspent envoys. Waiting to be asked is not a
	-- strategy when the question is not reliably asked.
	--
	-- ⚠ This does NOT enable the lane. `chooseEnvoy` still returns immediately
	-- unless `cfg.EnvoyEnabled` is set, which is still off by default because
	-- the three SIGSEGVs have not been cleared. What it changes is that the
	-- isolation run can now actually exercise the code it is testing.
	--
	-- ⚠ `beginTurn`, not `playTurn`: deployment runs with `--civvis-decides`, so
	-- `playTurn` never executes. A call added there would have been inert in
	-- exactly the configuration that matters — the same trap the `come_ashore`
	-- comment records for the tactical path.
	--
	-- Read back the previous turn's host token count before planning another
	-- spend. The immediate `envoy` event is intentionally issuing-side: the
	-- gameplay object can lag while `UI.RequestPlayerOperation` is resolving.
	-- This next-frame record is the first authoritative host value, and the
	-- lower bound makes token generation between frames explicit rather than
	-- mistaking a changed purse for a failed order.
	pcall(function()
		local pending = envoyTally.pending;
		if pending == nil then return; end
		local fresh = player:GetInfluence();
		if fresh == nil then return; end
		local heldAfter = try(function() return fresh:GetTokensToGive(); end);
		if heldAfter == nil then return; end
		local heldBefore = tonumber(pending.held_before) or 0;
		local requested = tonumber(pending.requested) or 0;
		emit("envoy_reconcile", {
			turn = turn,
			requested_turn = pending.turn,
			held_before = heldBefore,
			requested = requested,
			held_after = heldAfter,
			minimum_after = math.max(0, heldBefore - requested),
		});
		envoyTally.pending = nil;
	end);
	if cfg.EnvoyEnabled then
		pcall(function() chooseEnvoy(player, pid, turn); end);
	end
	-- Refreshed here rather than in the fallback so that the export, CIVVIS and the
	-- built-ins all describe the same war picture.
	warTarget = findWarTarget(player, pid);
	CivvisBoard.reset();
	if cfg.CancelQueuedPaths ~= false then CivvisBoard.cancelQueuedPaths(player, pid, turn); end
	exportState(player, pid, turn);
	exportTiles(player, pid, turn);
	-- ★★★★ WHAT THE LAST WORLD CONGRESS SESSION DECIDED, AND WHO GAINED FROM IT.
	--
	-- Seven Settler games of 2026-08-16 ended early on a rival's DIPLOMATIC
	-- victory, the last one (civvis-20260816T184500Z, 1058 vs 1087 at t222, the
	-- lane's best game) probably a win otherwise. The voter's own ledger says
	-- what WE cast (`wc_vote`: votes and favor against the leader) and the state
	-- export says the leader's DVP afterwards, but nothing says what the session
	-- RESOLVED: which option won each resolution and for whom, who voted how,
	-- which emergencies and aid requests passed against whom, and every civ's
	-- points across the session. Canada went 6→9→13→14→18→22 at successive
	-- sessions whether we cast the free vote or twelve; without the outcome
	-- there is no telling whether the +3/+4 was the victory resolution, a
	-- resolution it led the votes on, an aid competition, or all three — and
	-- so no telling whether option B against the leader is the right vote.
	--
	-- `Game.GetWorldCongress():GetReview(pid)` is what the shipped
	-- `WorldCongressPopup.lua` "Last Results" tab reads (`PopulateReview` /
	-- `PopulateReviewProposals`): `.Resolutions[i]` carries `Type`,
	-- `ChosenOption`, `ChosenThing`, `ChosenLabel`, `RejectedLabel`,
	-- `TargetType`, `IsNew`, `PlayerSelections[{PlayerID, OptionChosen,
	-- Votes}]`; `.Discussions[proposalType].ProposalsOfType[]` carries
	-- `TypeName`, `TargetPlayer`, `PlayerVotes[{PlayerType, Votes}]` (negative
	-- = against).
	-- The review is stable between sessions, so it is emitted once per change
	-- of content — i.e. once per session, on the first turn after it — keyed
	-- by a signature hung off `envoyTally` (a file-scope local would cross the
	-- 200-register main-chunk ceiling). DVP per alive major is read the way the
	-- voter already reads it. Read-only; nothing here votes.
	pcall(function()
		local wc = Game.GetWorldCongress();
		if wc == nil then return; end
		local review = wc:GetReview(pid);
		if type(review) ~= "table" then return; end
		local resolutions, signature = {}, {};
		for i, r in pairs(review.Resolutions or {}) do
			if type(i) == "number" and type(r) == "table" and r.Type ~= nil then
				local info = GameInfo.Resolutions[r.Type];
				local rtype = tostring(info and info.ResolutionType or r.Type);
				local a, b, ours, voters = 0, 0, nil, {};
				for _, sel in pairs(r.PlayerSelections or {}) do
					if type(sel) == "table" then
						local who = tonumber(sel.PlayerID) or -1;
						local votes = tonumber(sel.Votes) or 0;
						local option = tonumber(sel.OptionChosen) or 0;
						if option == 1 then a = a + votes; else b = b + votes; end
						voters[#voters + 1] = { player = who, option = option, votes = votes };
						if who == pid then ours = { option = option, votes = votes }; end
					end
				end
				local won = a > b and 1 or (b > a and 2 or 0);
				resolutions[#resolutions + 1] = {
					type = rtype,
					target_type = tostring(r.TargetType or ""),
					target = tostring(r.ChosenThing or ""),
					chosen = tostring(r.ChosenLabel or ""),
					won = won, a = a, b = b,
					ours = ours, voters = voters,
					new = (r.IsNew ~= nil and r.IsNew ~= 0 and r.IsNew ~= false) or false,
				};
				signature[#signature + 1] = rtype .. ":" .. tostring(r.ChosenThing or "") .. ":" .. a .. "/" .. b;
			end
		end
		local proposals = {};
		for ptype, category in pairs(review.Discussions or {}) do
			if type(category) == "table" then
				for _, prop in pairs(category.ProposalsOfType or {}) do
					if type(prop) == "table" then
						-- `PlayerVotes` entries carry `PlayerType` (the voter's id)
						-- and a signed `Votes` (negative = against), as the
						-- shipped review reads them.
						local up, down, ours = 0, 0, nil;
						for _, v in pairs(prop.PlayerVotes or {}) do
							if type(v) == "table" then
								local votes = tonumber(v.Votes) or 0;
								if votes > 0 then up = up + votes; elseif votes < 0 then down = down - votes; end
								if tonumber(v.PlayerType) == pid then ours = votes; end
							end
						end
						local target = tonumber(prop.TargetPlayer) or -1;
						local ptypeName = tostring(prop.TypeName or ptype);
						proposals[#proposals + 1] = {
							type = ptypeName, target = target,
							passed = up > down, up = up, down = down, ours = ours,
						};
						signature[#signature + 1] = ptypeName .. "@" .. target .. ":" .. up .. "/" .. down;
					end
				end
			end
		end
		if #resolutions == 0 and #proposals == 0 then return; end
		table.sort(signature);
		local key = table.concat(signature, "|");
		if envoyTally.wc_review_signature == key then return; end
		envoyTally.wc_review_signature = key;
		local dvp = {};
		for _, other in ipairs(try(function() return PlayerManager.GetAliveMajorIDs(); end, {})) do
			local id = tonumber(other);
			if id ~= nil then
				dvp[#dvp + 1] = { player = id, points = tonumber(try(function()
					return Players[id]:GetStats():GetDiplomaticVictoryPoints();
				end, 0)) or 0 };
			end
		end
		local favorNow = tonumber(try(function() return player:GetFavor(); end, 0)) or 0;
		emit("wc_outcome", {
			turn = turn, resolutions = resolutions, proposals = proposals, dvp = dvp,
			favor = favorNow,
		});
		-- ★★★★★ AND THE HALF THAT MAKES THE BALLOT A CHECK INSTEAD OF A CLAIM.
		--
		-- `wc_vote` has always reported what the ballot INTENDED -- `cast 3,
		-- spent 612` -- and nothing ever compared that with what the host went
		-- on to record. It recorded one vote, every time, for 961 resolutions
		-- across 29 runs, and the Favor was never taken. An operation that is
		-- queued and then silently declined is indistinguishable from one that
		-- worked, unless something reads the result back.
		--
		-- This is that read. `PlayerSelections` above is the host's own count
		-- for our seat; the ask is what this turn's ballot sent. `registered`
		-- being false is the defect reporting itself, per resolution, with the
		-- Favor readings that priced it -- which is what the next repair needs
		-- and what no run so far has had.
		if type(envoyTally.ballot_ask) == "table" then
			for _, r in ipairs(resolutions) do
				local ask = envoyTally.ballot_ask[r.type];
				if type(ask) == "table" and type(r.ours) == "table" then
					local recorded = tonumber(r.ours.votes) or 0;
					local asked = tonumber(ask.votes) or 0;
					emit("wc_ballot_verdict", {
						turn = turn, resolution = r.type,
						asked = asked, recorded = recorded,
						registered = recorded >= asked,
						option_asked = ask.option, option_recorded = r.ours.option,
						favor_at_ballot = envoyTally.ballot_favor_now,
						favor_entering_congress = envoyTally.ballot_favor_entering,
						favor_now = favorNow,
						max_votes = envoyTally.ballot_max_votes,
						costs = envoyTally.ballot_costs,
						votes_sent = (type(envoyTally.ballot_sent) == "table")
							and envoyTally.ballot_sent[r.type] or nil,
						-- Both affordability walks behind the ask (host table
						-- and Standard-priced), so the session that finally
						-- registers a bank says which table the core charges.
						budget = (type(envoyTally.ballot_budget) == "table")
							and envoyTally.ballot_budget or nil,
					});
				end
			end
			envoyTally.ballot_ask = {};
		end
		-- ★★★★★ AND THE SAME TABLE, KEPT FOR THE STATE EXPORT.
		--
		-- `wc_outcome` is a log event: nothing on the CIVVIS side has ever read
		-- it. Meanwhile the per-turn rival export is met-gated, so the victory
		-- tracker's diplomatic lane only ever saw the civilizations this seat
		-- had already contacted. Measured over the 50 runs carrying a congress
		-- table, 40 of them (80%) had a congress DVP standing HIGHER than any
		-- rival the decider could see, and in five the gap crossed the denial
		-- alarm: `civvis-20260818T103630Z` lost a diplomatic victory to a
		-- player sitting at 22 DVP while the tracker's best visible rival read
		-- 14, so `urgent_victory_threat` never fired once all game.
		--
		-- This is the congress standing and nothing more: the table the seat is
		-- shown when it votes, stamped with the turn it was shown. It is not a
		-- live per-turn read of an uncontacted empire — between sessions it
		-- goes stale exactly the way a human's memory of the last session does.
		--
		-- ⚠ Guarded on a non-empty list: an empty Lua table encodes as `{}`, not
		-- `[]`, and a `points` object where the schema wants an array fails the
		-- whole state parse rather than this one field. `GetAliveMajorIDs`
		-- cannot be empty while this seat is playing, so the guard is insurance
		-- against a host that answered oddly, not an expected branch.
		if #dvp > 0 then
			envoyTally.congress_dvp = { turn = turn, points = dvp };
		end
	end);
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

	-- ★★★★★ REPORT THE PREVIOUS TURN'S RESIDUAL BEFORE CLEARING IT.
	--
	-- ⚠⚠ `residual` HAS BEEN STRUCTURALLY MEANINGLESS ON THE CIVVIS PATH, and it is
	-- the field this project has been quoting as proof that nothing but CIVVIS
	-- decides anything. The order of a turn is: reset here, the game core fires its
	-- end-turn blockers and `answerBlocker` counts them, and `applyOrders` emits the
	-- turn record. But `applyOrders` runs when CIVVIS's orders ARRIVE, which is
	-- BEFORE most blockers fire — so the tally it copied was almost always empty, and
	-- the entries that arrived after it were wiped by this reset without ever being
	-- emitted. Every run of this ladder reported `residual: none`.
	--
	-- What it was hiding, measured on run `civvis-20260731T075743Z`: 29
	-- `ENDTURN_BLOCKING_PRODUCTION` answers, each one a call to `driveProduction`,
	-- which picks the item ITSELF from the hand-written ladder. CIVVIS issued 11
	-- produce orders that run; the heuristic issued 33 builds, including TEN battering
	-- rams in a game with one rival met and no war. On attempt 4 it was 338 against
	-- 270. That is not a pass re-running CIVVIS's own order — it is a different
	-- program choosing what the empire makes.
	--
	-- Emitted as its own event for the turn that just ended, so the count is taken
	-- after everything that can add to it and no longer races the turn record.
	if next(residualAnswers) ~= nil then
		emit("residual", { turn = awaiting.turn, counts = residualAnswers });
	end

	awaiting.turn = turn;
	awaiting.ticks = 0;
	awaiting.polls = 0;
	awaiting.done = false;
	awaiting.source = "pending";
	awaiting.frame = 0;
	CivvisFrames.reset();
	-- Per turn, like everything else in this handshake: a queue that outlived
	-- its turn is reported and dropped, never carried into a board CIVVIS has
	-- not seen.
	CivvisQueue.reset(turn);
	-- Per turn, or the tally becomes cumulative and unreadable.
	residualAnswers = {};
	-- The same table carries direct choices and deferred `city:next` leases.
	-- Keeping one table is necessary because Lua 5.1's main chunk has only
	-- 200 registers and this file already uses the practical ceiling.
	civvisBuild = {};
end

-- Poll for CIVVIS's answer. Returns true once the turn's decisions are settled,
-- by CIVVIS or by the fallback; the caller must not end the turn before then.
local function settleTurn(player, pid, turn, playFallback)
	if awaiting.turn ~= turn then return true; end
	if awaiting.done then
		-- The decisions are in; the per-unit follow-ups may still be
		-- draining. Hold the turn for them, bounded — see `CivvisQueue`.
		if CivvisQueue.pendingCount() > 0 then
			CivvisQueue.ticks = CivvisQueue.ticks + 1;
			if CivvisQueue.ticks > (tonumber(cfg.OrderQueueMaxTicks) or 240) then
				CivvisQueue.giveUp(turn);
				return true;
			end
			CivvisQueue.drain(player, pid, turn);
			if CivvisQueue.pendingCount() > 0 then return false; end
		end
		-- Everything issued has settled. If a strike went out this frame and
		-- frames are enabled, open the next one: the brain re-plans the same
		-- turn on the board as it now stands. See CivvisFrames.
		if CivvisFrames.wanted() then
			CivvisFrames.begin(player, pid, turn);
			return false;
		end
		return true;
	end
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

	local frame = awaiting.frame or 0;
	local ready = ordersReady(turn, frame);
	if ready ~= nil and ready >= 0 then
		local rows = fetchOrders(turn, frame);
		-- `ready.count` is the transaction boundary, including for an empty
		-- decision. Requiring a positive row count wedged every turn where CIVVIS
		-- correctly chose no action: the brain had durably written count=0, but the
		-- game waited until the legacy fallback budget expired. Match the declared
		-- count so zero completes immediately while a partially visible batch still
		-- cannot be actuated.
		if #rows == ready then
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

	-- A combat frame has its own short budget and no fallback: past it the
	-- frame is abandoned by name and the turn ends as it always did. The
	-- opening board's stale-answer and built-in ladders below never apply
	-- to a frame — a stale answer is the very board this frame replaces.
	if frame > 0 then
		if awaiting.polls >= (tonumber(cfg.CombatFramePolls) or 20) then
			awaiting.done = true;
			awaiting.source = "civvis";
			CivvisFrames.strikes = 0;
			emit("combat_frame_timeout", { turn = turn, frame = frame, polls = awaiting.polls });
			return true;
		end
		return false;
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
		actions = lastActions,
		ticks_seen = ticksSeen, ticks_taken = ticksTaken,
		blocker = blockerName(currentBlocker(pid)),
	});
end

local function tick()
	if finished or inTick or cfg.Play == false then return; end
	inTick = true;
	local ok, err = pcall(function()
		-- ★★★★★ THE SEAT FORFEITED EVERY WORLD CONGRESS SESSION. The session is a
		-- SOFT blocker, so the ladder below dismissed it: run
		-- civvis-20260816T021044Z logged `dismissed … "forfeit": 1` for all
		-- nineteen sessions, cast no vote in 242 turns, and ended on turn 242
		-- to a rival's DIPLOMATIC victory -- the third of the last thirteen
		-- games to end that way (t224–t242), every one before the turn limit
		-- the score race needs. Its Diplomatic Favor was never spent either.
		--
		-- This is the shipped WorldCongressPopup's own submission path
		-- (DLC/Expansion2/UI/Additions/WorldCongressPopup.lua, OnAccept):
		-- one WORLD_CONGRESS_RESOLUTION_VOTE per resolution, then
		-- WORLD_CONGRESS_SUBMIT_TURN. `GetResolutions(pid)` is a numbered list
		-- (plus a "Stage" key) of {Type, TargetType, PossibleTargets};
		-- `GetVotesandFavorCost(pid)` is `[k] = cumulative favor for k+1
		-- votes` with a `MaxVotes` cap; PossibleTargets of a PlayerType
		-- resolution are player ids and PARAM_RESOLUTION_SELECTION is the
		-- 0-based index into them.
		--
		-- The votes: on WC_RES_DIPLOVICTORY (Effect1 = +2 points to the
		-- target, Effect2 = −2), every vote favor affords AGAINST the current
		-- diplomatic-victory leader -- the same choice Firaxis' own AI makes
		-- ("PlayerOrDiploLeader"), which is what makes piling on it count.
		-- On the other PLAYER-targeted resolutions the free vote goes to the
		-- option that buffs the target, with us as the target. Elsewhere the
		-- free vote goes to the option that is not a ban. Nothing here is a
		-- claim about winning a resolution: the point is that a session the
		-- seat sat out is now one it takes part in, cheaply, every time.
		local function voteWorldCongress(pid)
			local wc = Game.GetWorldCongress();
			if wc == nil then return 0, 0, "no_congress"; end
			local resolutions = wc:GetResolutions(pid);
			local costs = wc:GetVotesandFavorCost(pid);
			if type(resolutions) ~= "table" then return 0, 0, "no_resolutions"; end
			if type(costs) ~= "table" then costs = {}; end
			-- ★★★★★ NINE HUNDRED AND SIXTY-ONE RESOLUTIONS, ONE VOTE EVERY TIME.
			--
			-- The ballot below has reported `spent 264-924` on every session
			-- since #1766, and the host's own review has recorded our seat at
			-- `votes = 1` on 961 of 961 resolutions across 29 recorded runs --
			-- never once above the free vote -- while Favor rose monotonically
			-- through every "spend" (run civvis-20260818T091159Z: 584, 676, 681,
			-- 809, 816, 952 across four of them). Rivals cast 5 to 14. The
			-- OPTION we choose does register, and changes when we change it, so
			-- the operation reaches the core and only the extra votes are
			-- refused: what is failing is the purchase, not the ballot.
			--
			-- The one difference from the shipped screen is the budget it is
			-- priced against. `WorldCongressPopup.lua` reads
			-- `GetFavorEnteringCongress()` for every stage of a live session and
			-- `GetFavor()` only after it ends (line 466); this asked for as many
			-- votes as CURRENT Favor could buy. Asking beyond what the core will
			-- sell is a coherent explanation for a purchase that fails whole and
			-- leaves the free vote standing.
			--
			-- So price it the way the screen does, and -- because that is a
			-- hypothesis and not a finding -- record both readings and what the
			-- host went on to record, rather than assuming this fixed it. The
			-- lower of the two cannot lose a vote we are already not getting.
			-- `wc_ballot_verdict`, emitted beside the next `wc_outcome`, is the
			-- half that settles it.
			local favorNow = tonumber(try(function() return Players[pid]:GetFavor(); end, 0)) or 0;
			local favorEntering = tonumber(try(function()
				return Players[pid]:GetFavorEnteringCongress();
			end, nil));
			local favor = favorNow;
			if favorEntering ~= nil and favorEntering >= 0 and favorEntering < favor then
				favor = favorEntering;
			end
			envoyTally.ballot_favor_now = favorNow;
			envoyTally.ballot_favor_entering = favorEntering;
			envoyTally.ballot_ask = {};
			envoyTally.ballot_budget = nil;
			-- ★★★★★ AND WHAT THE VOTES WERE PRICED AGAINST, BECAUSE THE FIRST
			-- READBACK RULED OUT THE REASON IT WAS BUILT TO TEST.
			--
			-- #2013 priced the ballot against `GetFavorEnteringCongress` on the
			-- theory that it was the smaller number and that asking past it
			-- refused the purchase whole. The first decisive session says
			-- otherwise: run `civvis-20260818T161918Z` turn 162 reports
			-- `favor_at_ballot 439, favor_entering_congress 439` -- the same
			-- value -- with `asked 15, recorded 1, registered false`, while the
			-- one-vote ballot on the SAME resolution one turn later reports
			-- `asked 1, recorded 1, registered true`. A controlled pair inside
			-- one session: the free vote lands, the purchase does not, and the
			-- budget was never the difference.
			--
			-- What is left unobserved is the pricing itself. `n` came out at
			-- FIFTEEN votes for 420 Favor, and this file's own comment says
			-- twelve votes cost 780 on the shipped ladder -- so either the cost
			-- table is not the cumulative-per-vote-count array the loop below
			-- assumes, or `MaxVotes` is not the cap being read. Both are
			-- answerable, and neither is answerable from the outside: the
			-- numbers never leave this function.
			--
			-- So they leave it now. `GetVotesandFavorCost` is a small array;
			-- carrying it and the vote count it produced turns the next session
			-- into the experiment instead of the one after that. This changes
			-- no decision -- it is the same ballot, reported.
			envoyTally.ballot_costs = {};
			for index = 0, 20 do
				if costs[index] ~= nil then
					envoyTally.ballot_costs[#envoyTally.ballot_costs + 1] =
						{ index = index, cost = tonumber(costs[index]) };
				end
			end
			envoyTally.ballot_max_votes = tonumber(costs.MaxVotes);
			envoyTally.ballot_sent = {};
			-- The diplomatic-victory leader among the others.  Keep this sampling
			-- beside the actual vote: it is the host API contract and exposes a
			-- useful DVP/score snapshot to the pure selector below.
			local candidates = {};
			for _, other in ipairs(try(function() return PlayerManager.GetAliveMajorIDs(); end, {})) do
				local otherID = tonumber(other);
				if otherID ~= nil and otherID ~= pid then
					candidates[#candidates + 1] = {
						id = otherID,
						points = tonumber(try(function()
							return Players[otherID]:GetStats():GetDiplomaticVictoryPoints();
						end, 0)) or 0,
						score = tonumber(try(function() return Players[otherID]:GetScore(); end, -1)) or -1,
					};
				end
			end
			-- Equal DVP totals use score rather than arbitrary PlayerManager order.
			local leader, leaderPoints, leaderScore = CivvisSelectCongressLeader(candidates);
			-- ★★★ AND WHO THE PENALTY BALLOTS SHOULD PUNISH. The culture race is
			-- a rival's visiting tourists over the highest domestic count it has
			-- to clear, which is the engine's own formula and reads off the same
			-- two accessors the state export already calls. Both lanes are
			-- expressed as a percentage so one selector ranks them.
			if cfg.CounterResolutions ~= false then
				local bar = 0;
				for _, c in ipairs(candidates) do
					local dom = tonumber(try(function()
						return Players[c.id]:GetCulture():GetStaycationers();
					end, 0)) or 0;
					c.domestic = dom;
					if dom > bar then bar = dom; end
				end
				local ourDom = tonumber(try(function()
					return Players[pid]:GetCulture():GetStaycationers();
				end, 0)) or 0;
				for _, c in ipairs(candidates) do
					-- The bar a rival must clear excludes its own domestic count.
					local against = ourDom;
					for _, other in ipairs(candidates) do
						if other.id ~= c.id and (other.domestic or 0) > against then
							against = other.domestic or 0;
						end
					end
					local tourists = tonumber(try(function()
						return Players[c.id]:GetCulture():GetTouristsTo();
					end, 0)) or 0;
					local culture = (against > 0) and (100 * tourists / against) or 0;
					local diplo = 100 * (tonumber(c.points) or 0) / 20;
					c.progress = (culture > diplo) and culture or diplo;
					if c.progress > 100 then c.progress = 100; end
				end
			end
			local threat, threatProgress = CivvisSelectVictoryThreat(candidates);
			-- Option 1 of these resolutions is the ban; the free vote goes to 2.
			local BAN_FIRST = {
				WC_RES_MERCENARY_COMPANIES = true, WC_RES_GLOBAL_ENERGY_TREATY = true,
				WC_RES_DEFORESTATION_TREATY = true, WC_RES_PUBLIC_RELATIONS = true,
			};
			-- Diplomatic victory first, so its votes get the favor. `Type` may
			-- arrive as the row name or its hash; the row's own ResolutionType
			-- is the name either way, and a resolution the DB does not know is
			-- left alone rather than voted blind.
			local list = {};
			for i, r in pairs(resolutions) do
				if type(i) == "number" and type(r) == "table" and r.Type ~= nil then
					local info = GameInfo.Resolutions[r.Type];
					if info ~= nil then
						list[#list + 1] = { r = r, info = info,
							rtype = tostring(info.ResolutionType or r.Type) };
					end
				end
			end
			table.sort(list, function(a, b)
				local da = a.rtype == "WC_RES_DIPLOVICTORY" and 1 or 0;
				local db = b.rtype == "WC_RES_DIPLOVICTORY" and 1 or 0;
				if da ~= db then return da > db; end
				return a.rtype < b.rtype;
			end);
			local cast, spent = 0, 0;
			-- How the Diplomatic Victory ballot was cast, for `wc_vote`:
			-- "claim" (A for us with the bank), "deny" (B against the leader
			-- with the bank), "free" (the one free vote against the leader).
			local mode = nil;
			for _, entry in ipairs(list) do
				local r, info, rtype = entry.r, entry.info, entry.rtype;
				do
					local option, selection, votes = 1, 1, 1;
					local targets = r.PossibleTargets or {};
					if rtype == "WC_RES_DIPLOVICTORY" then
						option = 2;
						for idx, t in pairs(targets) do
							if tonumber(t) == leader then selection = idx; end
						end
						-- ★★★★ FAVOR IS BANKED UNTIL A LEADER IS WITHIN REACH.
						--
						-- Run `civvis-20260816T123936Z` ended at turn 239 on a rival's
						-- Diplomatic Victory -- the sixth early diplomatic loss on the
						-- Settler seat -- and the ledger of this voter reads: 180 Favor
						-- spent at t161 against a leader on 8 points, 220 at t181 (11),
						-- 220 at t201 (14), 264 at t221 (15); the leader then went 15
						-- -> 19 -> 20 in the two sessions that decided it while the
						-- treasury it faced was whatever had trickled in since the
						-- last spend. Extra votes cost Favor on a rising ladder, so the
						-- same Favor buys the most votes when spent at once, and it
						-- only matters at the sessions a leader can win from. Below
						-- `DiploVictoryVoteFloor` points (12: four sessions of +2 from
						-- twenty) the free vote is still cast against the leader and
						-- nothing is spent; from there every session spends the bank.
						local floor = cfg.DiploVictoryVoteFloor or 12;
						local maxVotes = tonumber(costs.MaxVotes) or 1;
						-- Both walks live in `CivvisCongressVoteBudget` (see its
						-- comment): the host-table bank and the Standard-priced
						-- bank a mispricing core would charge. The ask takes the
						-- smaller; the verdict records both.
						local budget, budgetHost, budgetStandard =
							CivvisCongressVoteBudget(favor, costs, maxVotes);
						envoyTally.ballot_budget =
							{ host = budgetHost, standard = budgetStandard };
						local n = 1;
						if (tonumber(leaderPoints) or 0) >= floor then
							n = budget;
							mode = (n > 1) and "deny" or "free";
						elseif cfg.CongressVoteProbe ~= false and budget > 1 then
							-- ★★ A THREE-VOTE PROBE AT EVERY SESSION BELOW THE FLOOR.
							--
							-- The floor banks Favor until a leader is within reach,
							-- which also postpones the first multi-vote ballot to
							-- t160+ — one or two sessions before a diplomatic loss,
							-- far too late to learn whether the purchase registers
							-- at all. Three votes cost 12 Favor on the Online table
							-- (30 if the core charges Standard) out of a bank that
							-- ends games in the hundreds: every session now reports
							-- a verdict on the purchase path, and lands two extra
							-- votes against the leader when it works. Its own mode
							-- string keeps a 12-Favor probe from reading like a
							-- bank-scale deny in the ledger.
							n = (budget < 3) and budget or 3;
							mode = "probe";
						else
							mode = "free";
						end
						-- ★★★★ WHEN THE BANK OUTVOTES EVERY RIVAL'S BLOCK, CLAIM THE +2.
						--
						-- The resolution's winner is the option with more votes, and
						-- its target the player with the most votes ON that option.
						-- Decoded `wc_outcome` rows (runs T205104Z, T223457Z): option
						-- A ("+2 to the target") wins every session by 23–43 votes to
						-- ≤15 because each rival votes A for ITSELF with 6–11 votes,
						-- and the target is simply the rival with the biggest block —
						-- so a B ballot against the leader changes nothing until the
						-- rivals themselves turn on a leader at 17. Meanwhile our
						-- Favor sat at 640–1441 unspent. Twelve votes (780 Favor on
						-- the shipped ladder) beat every block seen; when the bank
						-- affords that many, vote A with all of them targeting US: the
						-- +2 lands on this seat and the leader gets nothing that
						-- session. `DiploVictoryClaimVotes` is that bar; below it the
						-- floor rule above stands.
						local claim = tonumber(cfg.DiploVictoryClaimVotes) or 12;
						local ourIdx = nil;
						for idx, t in pairs(targets) do
							if tonumber(t) == pid then ourIdx = idx; end
						end
						if ourIdx ~= nil and budget >= claim then
							option = 1;
							selection = ourIdx;
							n = budget;
							mode = "claim";
						end
						votes = n;
						local cost = (n > 1 and costs[n - 1]) or 0;
						favor = favor - cost;
						spent = spent + cost;
					elseif r.TargetType == "PlayerType" then
						-- ⚠⚠ THE THREE RESOLUTIONS BELOW CARRY A REAL PENALTY ON
						-- OPTION 2, AND WE SPENT THEM ON OURSELVES. Read from the
						-- host's own Expansion2_Congress.xml rather than guessed:
						--   Trade Policy       effect 2 = APPLY_INTERNATIONAL_MAJOR_TRADE_ROUTES_DISABLED
						--   Border Control     effect 2 = APPLY_NO_CULTURE_BORDER_EXPANSION_TO_PLAYER
						--   Migration Treaty   effect 2 = APPLY_GROWTH_PENALTY_TO_PLAYER
						-- The trade embargo is the one that matters most here: a
						-- live trade route is +25% Tourism toward its partner, so
						-- disabling a culture leader's international routes cuts
						-- the tourism it draws from EVERY civilization -- the one
						-- culture counter that does not require us to hold the
						-- domestic-tourist bar ourselves, which a post-mortem of
						-- the first instrumented run measured us holding for 0 of
						-- 153 contested turns.
						--
						-- The free vote costs no Favor, so this spends nothing the
						-- Diplomatic Victory ballot above wants: it changes only
						-- WHO the vote names. Below the bar the old self-buff
						-- stands, because naming a rival who is not close is worth
						-- less than the bonus.
						local counter = cfg.CounterResolutions ~= false
							and threat ~= nil and threat >= 0
							and (tonumber(threatProgress) or 0)
								>= (tonumber(cfg.CounterResolutionBar) or 60)
							and (rtype == "WC_RES_TRADE_TREATY"
								or rtype == "WC_RES_BORDER_CONTROL"
								or rtype == "WC_RES_MIGRATION_TREATY");
						if counter then
							for idx, t in pairs(targets) do
								if tonumber(t) == threat then selection = idx; end
							end
							option = 2;
						else
							for idx, t in pairs(targets) do
								if tonumber(t) == pid then selection = idx; end
							end
							if BAN_FIRST[rtype] then option = 2; end
						end
					elseif BAN_FIRST[rtype] then
						option = 2;
					end
					local params = {};
					params[PlayerOperations.PARAM_RESOLUTION_TYPE] = info.Hash;
					params[PlayerOperations.PARAM_WORLD_CONGRESS_VOTES] = votes;
					params[PlayerOperations.PARAM_RESOLUTION_OPTION] = option;
					params[PlayerOperations.PARAM_RESOLUTION_SELECTION] = selection - 1;
					-- ★★★★★ NINETY-FIVE OF NINETY-FIVE ONE-VOTE BALLOTS REGISTER.
					-- SEVENTEEN OF SEVENTEEN MULTI-VOTE BALLOTS DO NOT.
					--
					-- 112 `wc_ballot_verdict` rows over four runs, and the split
					-- is perfect in both directions: no ballot asking one vote
					-- was ever refused, and no ballot asking more than one was
					-- ever recorded above one. It does not depend on the moment
					-- (one-vote ballots register from the `stage1` trigger and
					-- from the `popup` trigger alike), on the option (both
					-- register and flip as asked), or on affordability -- run
					-- `civvis-20260818T175125Z` t162 asked thirteen votes at a
					-- charged 312 Favor holding 352, inside `MaxVotes = 13`, and
					-- the host recorded one.
					--
					-- Every explanation the mod controls is now eliminated:
					-- parameters match the shipped `OnAccept` exactly, both
					-- triggers fire, the option registers, the budget is not the
					-- difference (#2039: `favor_entering_congress` equals
					-- `GetFavor` on every row), and the ask is affordable and
					-- within the cap. What is left is the count parameter
					-- itself: `PARAM_WORLD_CONGRESS_VOTES > 1` is never honoured
					-- through this path.
					--
					-- #2045 tried one vote per operation, repeated, on the theory
					-- that the core might accumulate them. The experiment came
					-- back on run civvis-20260819T004405Z: `votes_sent 20,
					-- recorded 1` on every multi-vote session — the operation
					-- SETS the seat's ballot rather than adding to it, so a
					-- repeat leaves the LAST write's single vote standing. Back
					-- to one operation carrying the whole count, exactly as the
					-- shipped `OnAccept` sends it; what changed instead is the
					-- count itself, now priced by `CivvisCongressVoteBudget`
					-- against both tables the host might charge.
					local sent = pcall(function()
						UI.RequestPlayerOperation(pid,
							PlayerOperations.WORLD_CONGRESS_RESOLUTION_VOTE, params);
					end);
					if sent then cast = cast + 1; end
					envoyTally.ballot_sent = envoyTally.ballot_sent or {};
					envoyTally.ballot_sent[rtype] = sent and 1 or 0;
					-- What this ballot ASKED for, per resolution, so the next
					-- review can be compared with it rather than trusted. `pcall`
					-- reports only that the call did not raise; the host's own
					-- `PlayerSelections` is the only thing that reports a vote.
					envoyTally.ballot_ask[rtype] = { votes = votes, option = option };
				end
			end
			pcall(function()
				UI.RequestPlayerOperation(pid, PlayerOperations.WORLD_CONGRESS_SUBMIT_TURN, {});
			end);
			return cast, spent, nil, leader, leaderPoints, leaderScore, mode;
		end
		local player, pid = localPlayer();
		if player == nil then return; end
		-- ★★★★★ THE BALLOT IS CAST WHEN THE POPUP ASKS, NOT WHEN THE BLOCKER
		-- APPEARS. `voteWorldCongress` below is also called from the blocker
		-- ladder, and that call has never registered a vote: `wc_vote` says
		-- `spent 760` at t201 of civvis-20260816T184500Z and Favor reads
		-- 822→829→836 across it; `wc_outcome` shows our selection on every
		-- resolution as the core's default `option 1, votes 1` — the free vote
		-- cast FOR the diplomatic leader. The shipped screen votes from inside
		-- the WorldCongressPopup in stage 1; the autoclose shim standing in
		-- front of that popup raises `LuaEvents.CivvisCongressBallot` right
		-- before its `OnAccept`, and this is the handler. Registered once, from
		-- inside `tick` because `voteWorldCongress` is nested here (a file-scope
		-- local would cross the 200-register ceiling); the flag hangs off
		-- `envoyTally` for the same reason. `source` on the event tells the two
		-- call sites apart in the ledger; the popup one is the one that counts.
		-- ⚠⚠ AND THE FIRST POPUP-MOMENT ATTEMPT NEVER FIRED EITHER: batch-9
		-- game civvis-20260816T223457Z has no `source:"popup"` row at all —
		-- the shim's WorldCongressPopup ladder runs its `OnPass` rung, not the
		-- `OnAccept` one the event was raised in. Two triggers now, either of
		-- which casts once per turn: the game core's own
		-- `Events.WorldCongressStage1(playerID)` — the very event the shipped
		-- popup opens on, i.e. the earliest moment a person could vote — and
		-- the shim's ballot event, now raised from the rung that runs.
		-- `castBallot` is shared; `envoyTally.ballot_turn` is the once-per-turn
		-- latch and is only set when something was cast, so a trigger that
		-- arrives before the resolutions are readable does not spend the turn.
		-- The blocker path below defers to these and only falls back a forfeit
		-- cycle later. Every ballot reports its trigger, the core's `Stage`,
		-- and Favor before, so the next `wc_outcome` says which moment took.
		local function castBallot(trigger)
			local ballotPlayer, ballotPid = localPlayer();
			if ballotPlayer == nil then return; end
			local ballotTurn = try(function() return Game.GetCurrentGameTurn(); end, -1);
			if envoyTally.ballot_turn == ballotTurn then
				emit("wc_vote", { turn = ballotTurn, source = trigger, cast = 0, spent = 0,
				                  why = "already_cast" });
				return;
			end
			local stage = try(function()
				local r = Game.GetWorldCongress():GetResolutions(ballotPid);
				return r and r.Stage or nil;
			end, nil);
			-- `GetResolutions().Stage` has read INT_MAX on every recorded cast,
			-- from both triggers, so it does not say whether the cast landed
			-- inside the congress turn segment — the window the shipped popup
			-- votes in (`CheckShouldOpen` gates on `Game.GetCurrentTurnSegment`).
			-- Read the segment itself, so the moment theory finally has a
			-- measurement instead of a sentinel.
			local segment = try(function() return Game.GetCurrentTurnSegment(); end, nil);
			local inCongressSegment = try(function()
				if segment == nil then return nil; end
				return segment == DB.MakeHash("TURNSEG_WORLDCONGRESS_1")
					or segment == DB.MakeHash("TURNSEG_WORLDCONGRESS_2")
					or segment == DB.MakeHash("TURNSEG_WORLDCONGRESS_RESOLUTION");
			end, nil);
			local before = tonumber(try(function() return ballotPlayer:GetFavor(); end, -1)) or -1;
			local cast, spent, why, leader, leaderPoints, leaderScore, mode = voteWorldCongress(ballotPid);
			if (cast or 0) > 0 then envoyTally.ballot_turn = ballotTurn; end
			emit("wc_vote", { turn = ballotTurn, cast = cast, spent = spent,
			                  why = why, leader = leader,
			                  leader_points = leaderPoints,
			                  leader_score = leaderScore, source = trigger,
			                  stage = stage, favor_before = before, mode = mode,
			                  segment = segment,
			                  in_congress_segment = inCongressSegment });
		end
		if not envoyTally.ballot_hooked then
			envoyTally.ballot_hooked = true;
			local hookedLua = pcall(function()
				LuaEvents.CivvisCongressBallot.Add(function() castBallot("popup"); end);
			end);
			local hookedStage = pcall(function()
				Events.WorldCongressStage1.Add(function(playerID)
					if tonumber(playerID) == pid then castBallot("stage1"); end
				end);
			end);
			emit("wc_ballot_hooked", { turn = try(function() return Game.GetCurrentGameTurn(); end, -1),
			                           popup = hookedLua, stage1 = hookedStage });
		end
		if not try(function() return player:IsTurnActive(); end, false) then return; end

		local turn = try(function() return Game.GetCurrentGameTurn(); end, -1);
		if turn ~= lastTurnSeen then
			lastTurnSeen = turn;
			turnsPlayed = turnsPlayed + 1;
			-- ⚠ ONCE PER TURN, HERE, NOT IN `countUnits`. Counting runs several
			-- times a turn -- the comment on `countUnits` says so and an
			-- `upgradeUnit` spliced into it once issued orders from a counting
			-- pass. An idle streak incremented there would count passes, not
			-- turns, and every settler would read stranded within one turn.
			trackIdleUnits(player);
			attempts = 0;
			softSeen = {};
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
				if cfg.CivvisDecides then
					-- CIVVIS has already made and applied its complete unit-order
					-- pass in settleTurn. A soft blocker is only a UI reminder; the
					-- legacy unit AI must not invent orders here. In particular, it
					-- previously moved a Settler out of a safe capital and into a
					-- visible barbarian capture zone after CIVVIS chose to wait.
					answered = "civvis_complete";
				else
					-- Bounded per turn. The order pass is the expensive one, and a
					-- soft blocker that will not clear -- a unit the engine keeps
					-- listing as ready -- would otherwise run it on every batch of
					-- game-core events for the rest of the turn.
					if spend("soft", cfg.MaxSoftPasses or 3) then
						orderUnits(player, pid, turn);
					end
					answered = "units";
				end
			else
				answered = answerBlocker(player, pid, blocker, turn);
			end
			if attempts == 1 or attempts % (cfg.BlockerReportEvery or 25) == 0 then
				emit("blocked", { turn = turn, blocker = name,
				                  attempts = attempts, answered = answered });
			end
			-- ★★★ A SOFT BLOCKER THAT SURVIVES ITS ANSWER IS FORFEITED EARLY,
			-- not left to the MaxBlockedAttempts hammer below. Run
			-- civvis-20260807T190903Z (issue #1374), turn 39:
			-- `ENDTURN_BLOCKING_UNITS` answered `civvis_complete`, attempts:1 --
			-- then the turn sat 900 s until the outside watchdog killed the
			-- attempt. Two mechanisms compound there:
			--
			--   * With a units blocker up, the plain `ACTION_ENDTURN` request at
			--     the bottom of this function is refused -- the shipped
			--     ActionPanel.lua shows the engine's contract: its own end-turn
			--     click selects the next ready unit instead of requesting the
			--     end of turn.
			--   * A board waiting on input publishes almost no game-core events,
			--     so this very function nearly stops running and the 40-attempt
			--     hammer is wall-clock unreachable. The forfeit therefore has to
			--     fire on an early re-sighting after the answer, not on a big
			--     count.
			--
			-- Sighting 1 is the honest answer. The SAME soft blocker coming back
			-- says that answer did not clear it, so: park the still-ready units
			-- (units family only -- position-preserving orders, never the
			-- legacy movement AI), dismiss the notification exactly as the
			-- hammer would, and force the end of turn through the same
			-- `{ REASON = "UserForced" }` request the shipped UI sends for
			-- SHIFT+ENTER, which is the only end-turn form that is not refused
			-- while a units blocker is up.
			-- ⚠⚠⚠ `or answered == "civvis_complete"` REPAIRS A REGRESSION FROM #1465.
			--
			-- That change added `ENDTURN_BLOCKING_PANTHEON` and
			-- `ENDTURN_BLOCKING_FILL_CIVIC_SLOT` to `CIVVIS_OWNED_BLOCKERS` --
			-- correctly, because CIVVIS issues `pantheon` and `policy_deck`
			-- orders and `choosePantheon`/`fillPolicies` were racing them. But
			-- `civvis_complete` had until then been reachable ONLY from the
			-- `SOFT_BLOCKERS` arm above, so this forfeit was gated on that table
			-- and the two new names are not in it. `answerBlocker` returned
			-- `civvis_complete`, nothing dismissed the notification, and the
			-- prompt stood forever.
			--
			-- Measured. Run `civvis-20260810T040916Z` (before) saw
			-- `FILL_CIVIC_SLOT` **once**, answered `policies+3` by the
			-- heuristic. Run `civvis-20260810T050558Z` (after) saw it **ten
			-- times**, every one `civvis_complete`, and **the attempt wedged on
			-- it at turn 224** and was killed by the outside watchdog after
			-- 900 s while scoring 524. The stall watchdogs did their job; the
			-- turn loop should not have needed them.
			--
			-- Gating on the ANSWER rather than on a second table is what keeps
			-- this from happening again: any blocker that is ever answered
			-- "CIVVIS has already decided this" now has a forfeit, including one
			-- added to `CIVVIS_OWNED_BLOCKERS` later by someone who does not know
			-- this paragraph exists. `civvis_complete` is only ever produced
			-- under `cfg.CivvisDecides`, so non-decider runs are untouched, and
			-- `UNIT_BLOCKERS[name]` stays false for these two so they are
			-- dismissed without the unit-parking pass or the forced end-turn.
			if SOFT_BLOCKERS[name] or answered == "civvis_complete" then
				local seen = softSeen[name] or { sightings = 0, forfeits = 0 };
				softSeen[name] = seen;
				seen.sightings = seen.sightings + 1;
				-- How many sightings prove the answer did nothing. A
				-- `civvis_complete` answer changed the board by construction
				-- NOT AT ALL -- it means "CIVVIS has already ordered this
				-- board, keep the legacy AI off it" -- so the second sighting
				-- is already proof and a third only spends wall clock we do
				-- not have. The legacy answer really does run `orderUnits`,
				-- bounded by `MaxSoftPasses`, so it gets that budget first.
				local bound = cfg.SoftBlockerForfeitAttempts
					or (answered == "civvis_complete" and 2 or (cfg.MaxSoftPasses or 3) + 1);
				local cap = cfg.MaxSoftBlockerForfeits or 3;
				if seen.sightings >= bound then
					seen.sightings = 0;
					-- A `civvis_complete` that has now proven inert twice is not
					-- an answer, and dismissing a HARD blocker's notification
					-- does not satisfy the engine (see the paragraph on the
					-- early return in `answerBlocker`). Ask the ladder for ONE
					-- real answer first — bounded by its own per-name `spend`
					-- budgets, counted in `residualAnswers`, overridable by
					-- CIVVIS at the next export. Only a pass that produced
					-- nothing falls through to the dismissal ladder below.
					-- ⚠⚠⚠ AND THE RESIDUAL ANSWER IS BOUNDED TOO, OR IT IS THE WEDGE.
					--
					-- Run `civvis-20260816T115139Z` -- the seat's best game, 804
					-- against 715 and the score leader -- wedged at turn 178 on
					-- `ENDTURN_BLOCKING_UNITS`: `civvis_complete`, then the ladder's
					-- `units` answer, `residual_unblock ... forfeits 0`, and the same
					-- blocker back again -- SEVEN times, because a residual answer
					-- that is not nil and not `civvis_complete` reset `attempts` and
					-- never reached the forfeit below, so neither the parking pass,
					-- the dismissal, the forced end turn nor the 40-attempt drop ever
					-- ran. The outside watchdog killed the attempt after 900 s.
					-- The ladder's own per-name `spend` budgets did not bound it,
					-- and must not be relied on to. Two residual answers that leave
					-- the blocker standing are the proof; the third sighting-pair
					-- forfeits like any other inert answer.
					local residual_pick = nil;
					if answered == "civvis_complete"
							and (seen.residuals or 0) < (cfg.MaxResidualAnswers or 2) then
						residual_pick = answerBlocker(player, pid, blocker, turn, true);
					end
					local residual_taken = false;
					if residual_pick ~= nil and residual_pick ~= "civvis_complete" then
						seen.residuals = (seen.residuals or 0) + 1;
						emit("residual_unblock", { turn = turn, blocker = name,
						                           answered = residual_pick,
						                           forfeits = seen.forfeits,
						                           residuals = seen.residuals });
						attempts = 0;
						residual_taken = true;
					end
					-- ⚠⚠⚠ A UNITS BLOCKER FORFEITS IN THE SAME PASS AS ITS RESIDUAL
					-- ANSWER, because a quiet board never ticks again. Run
					-- `civvis-20260816T151716Z` wedged at turn 111 with the bound
					-- above in place: `civvis_complete`, residual `units`,
					-- `residuals: 1`, then `residuals: 2` -- and then NOTHING. The
					-- residual pass had no unit left to order, so it issued no
					-- request, the game core published no event, and this function
					-- -- driven only by game-core events, a per-frame update does
					-- not run in this context (see the note above `onGameCoreTick`)
					-- -- was never called again to reach the forfeit that the third
					-- sighting-pair was supposed to bring. The outside watchdog
					-- killed the attempt after 900 s, at 342 against 403. For the
					-- units family the forfeit therefore runs in THIS pass, after
					-- the residual answer's own requests are queued: park what is
					-- still ready, dismiss, and force the end of turn. Research and
					-- production keep the two-step, since their answers always
					-- move the board and a forced end turn is refused under them.
					if (not residual_taken or UNIT_BLOCKERS[name]) and seen.forfeits < cap then
						-- BOUNDED RETRY. Re-arming after `bound` more sightings
						-- covers a forfeit that did not stick -- the engine can
						-- raise a fresh units notification for the same units --
						-- without looping the expensive parking pass on every
						-- batch of game-core events.
						seen.forfeits = seen.forfeits + 1;
						-- The congress session gets its votes before it is
						-- forfeited: see `voteWorldCongress`. Once per turn;
						-- a session the engine keeps up after the submit falls
						-- through to the dismissal exactly as before.
						if name == "ENDTURN_BLOCKING_WORLD_CONGRESS_SESSION"
								and seen.voted_turn ~= turn then
							-- ⚠ THE BLOCKER IS SEEN BEFORE THE SESSION IS OPEN FOR
							-- VOTING (its ballot never registered; see `castBallot`
							-- above), so it defers: if the stage-1/popup ballot has
							-- cast this turn there is nothing to do, and otherwise
							-- it waits one forfeit cycle for those triggers before
							-- falling back to the old vote-and-submit, so a session
							-- that neither trigger reaches still ends.
							if envoyTally.ballot_turn == turn then
								seen.voted_turn = turn;
								emit("wc_vote", { turn = turn, cast = 0, spent = 0,
								                  why = "cast_at_stage1", source = "blocker" });
							elseif seen.forfeits >= 2 then
								seen.voted_turn = turn;
								local cast, spent, why, leader, leaderPoints, leaderScore, mode = voteWorldCongress(pid);
								emit("wc_vote", { turn = turn, cast = cast, spent = spent,
								                  why = why, leader = leader,
								                  leader_points = leaderPoints,
								                  leader_score = leaderScore, source = "blocker",
								                  mode = mode });
							end
						end
						local parked = UNIT_BLOCKERS[name] and parkReadyUnits(player) or 0;
						local dropped = dismissBlocker(pid, blocker);
						emit("dismissed", { turn = turn, blocker = name,
						                    dismissed = dropped, attempts = attempts,
						                    answered = answered, parked = parked,
						                    forfeit = seen.forfeits,
						                    forced = UNIT_BLOCKERS[name] == true });
						attempts = 0;
						if UNIT_BLOCKERS[name] then
							pcall(function()
								UI.RequestAction(ActionTypes.ACTION_ENDTURN,
								                 { REASON = "UserForced" });
							end);
						end
					elseif not residual_taken and seen.forfeits == cap then
						-- ⚠ THE RETRY IS SPENT AND THE TURN IS STILL NOT MOVING.
						-- Say so, once, in the run's own event log. Issue #1374
						-- died as 900 s of silence that read as a slow machine;
						-- an outside watchdog killing an attempt should be able
						-- to point at the prompt that did it. Bumping past `cap`
						-- is what makes this report once per turn per blocker
						-- rather than on every later sighting.
						seen.forfeits = cap + 1;
						emit("wedged", { turn = turn, blocker = name,
						                 forfeits = cap, attempts = attempts,
						                 answered = answered });
					end
				end
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

-- A blocker-state change ticks DIRECTLY, not through the 1-in-16 divider.
--
-- `EndTurnBlockingChanged` fires a handful of times per turn -- when a blocker
-- is answered, dismissed, or replaced by the next one -- and each of those is
-- precisely the moment the loop has something to do. Routing it through
-- `onGameCoreTick` made progress wait for fifteen more publish batches, and a
-- board that is sitting on a blocker is exactly the board that publishes
-- almost nothing (issue #1374): after the forfeit above dismisses one
-- notification, the divider could starve the pass that should answer the next
-- one. `tick` is reentrancy-guarded and cheap when there is nothing to do, so
-- taking this event undivided costs a few extra passes per turn at most.
local function onEndTurnBlockingChanged()
	ensureStarted();
	tick();
end

-- The host says one of our units finished moving or its operation ended.
-- If that unit has queued follow-ups, this is the moment to issue the next
-- one — undivided, like `EndTurnBlockingChanged`, because a board whose only
-- remaining work is a queued strike publishes almost nothing on its own.
CivvisQueue.onUnitSettled = function(player, unitId)
	if CivvisQueue.count <= 0 then return; end
	local pid = try(function() return Game.GetLocalPlayer(); end, -1);
	if CivvisQueue.noteUnitEvent(pid, player, unitId) then
		ensureStarted();
		tick();
	end
end;

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

-- ★★★★★ JOIN A SCORABLE WORLD CRISIS BEFORE TRYING TO WIN IT.
--
-- The bridge already knows how to take an Aid Request's first-place score and
-- Climate Accords' power-plant decommission score. Both paths require
-- membership, while the prior controller merely let the World Crisis prompt
-- wait for a person. Firaxis's own WorldCrisisPopup handles
-- `Events.EmergencyAvailable` by issuing this exact ACCEPT_EMERGENCY operation
-- with PARAM_OTHER_PLAYER and PARAM_EMERGENCY_TYPE. Take that same operation,
-- but only for competitions with a priced path in the bridge. Other emergencies
-- can create wars or commit production that this event has not priced, so they
-- remain untouched.
--
-- This is a bare global for the offline regression and because the main chunk
-- is at its local-register ceiling. `peaceAsked` is the existing turn-scoped
-- submission ledger; a string key prevents a synchronous repeat of this event
-- from submitting a second accept before the host updates MemberIDs.
CivvisOnAidEmergencyAvailable = function(targetPlayerID, emergencyType)
	if finished or cfg.Play == false then return; end
	local pid = try(function() return Game.GetLocalPlayer(); end, -1);
	local target = tonumber(targetPlayerID);
	local emergency = tonumber(emergencyType);
	local definition = emergency ~= nil and try(function()
		return GameInfo.EmergencyAlliances[emergency];
	end, nil) or nil;
	local kind = definition and tostring(definition.EmergencyType or "") or "";
	local aid = kind == "EMERGENCY_SEND_AID" or kind == "EMERGENCY_SEND_MILITARY_AID";
	local climate = kind == "EMERGENCY_CLIMATE_ACCORDS";
	if not climate and cfg.AutoJoinAidRequests == false then return; end
	if climate and cfg.AutoJoinClimateAccords == false then return; end
	local turn = try(function() return Game.GetCurrentGameTurn(); end, -1);
	local function report(reason, submitted)
		emit(climate and "climate_accords_join" or "aid_emergency_join", {
			turn = turn, target = target or -1,
			emergency = kind ~= "" and kind or tostring(emergencyType or ""),
			submitted = submitted and true or false, reason = reason,
		});
	end
	if pid == nil or pid < 0 or target == nil or emergency == nil then
		report("invalid_event", false);
		return;
	end
	if not aid and not climate then
		report("not_aid_request", false);
		return;
	end
	if climate then
		-- Climate Accords has NoTarget=true. The shipped popup sends -1 through
		-- PARAM_OTHER_PLAYER; a real player ID would be a mismatched event.
		if target ~= -1 then
			report("unexpected_target", false);
			return;
		end
	else
		if target < 0 then
			report("invalid_event", false);
			return;
		end
		if target == pid then
			report("target_is_local", false);
			return;
		end
	end

	-- Match the shipped popup's tracker lookup before issuing anything. An old
	-- availability notification is not authority to join a different emergency.
	local crises = try(function()
		return Game.GetEmergencyManager():GetEmergencyInfoTable(pid);
	end, nil);
	local live = nil;
	if type(crises) == "table" then
		for _, crisis in ipairs(crises) do
			if crisis.EmergencyType == emergency and tonumber(crisis.TargetID) == target then
				live = crisis;
				break;
			end
		end
	end
	if live == nil then
		report("missing_emergency", false);
		return;
	end
	if live.HasBegun == true then
		report("already_begun", false);
		return;
	end
	for _, member in ipairs(live.MemberIDs or {}) do
		if tonumber(member) == pid then
			report("already_member", false);
			return;
		end
	end

	if aid then
		-- Firaxis gives both Aid Request types an empty member-requirement set: the
		-- project route can score even when we have not met the recipient, or when
		-- it is not a major civilization. Do not accidentally impose the direct-Gold
		-- deal's stricter contact/major gates here. War is different: the shipped
		-- score sources deduct 30 (ordinary Aid) or 200 (military Aid) while at war,
		-- so decline that actively losing membership.
		local player = try(function() return Players[pid]; end, nil);
		local diplomacy = player and try(function() return player:GetDiplomacy(); end, nil);
		if diplomacy == nil then
			report("no_diplomacy", false);
			return;
		end
		if try(function() return diplomacy:IsAtWarWith(target); end, false) then
			report("at_war", false);
			return;
		end
	end

	local otherParam = try(function() return PlayerOperations.PARAM_OTHER_PLAYER; end);
	local typeParam = try(function() return PlayerOperations.PARAM_EMERGENCY_TYPE; end);
	local accept = try(function() return PlayerOperations.ACCEPT_EMERGENCY; end);
	if otherParam == nil or typeParam == nil or accept == nil then
		report("api_unavailable", false);
		return;
	end
	local key = climate and ("climate_join:" .. kind)
		or ("aid_join:" .. kind .. ":" .. target);
	if turn >= 0 and peaceAsked[key] == turn then
		report("duplicate", false);
		return;
	end
	local parameters = {};
	parameters[otherParam] = target;
	parameters[typeParam] = emergency;
	-- Set the one-turn guard before the engine call: RequestPlayerOperation can
	-- publish synchronously. A Lua exception clears it so a later valid event
	-- can still retry; a normal return is only a submitted operation, not a
	-- claimed membership or score change.
	if turn >= 0 then peaceAsked[key] = turn; end
	local submitted = pcall(function()
		UI.RequestPlayerOperation(pid, accept, parameters);
	end);
	if not submitted and turn >= 0 and peaceAsked[key] == turn then
		peaceAsked[key] = nil;
	end
	report(submitted and "submitted" or "throw", submitted);
end;

function Initialize()
	emit("loaded", { version = 2, play = cfg.Play ~= false });
	for name, handler in pairs({
		LocalPlayerTurnBegin = onLocalPlayerTurnBegin,
		GameCoreEventPublishComplete = onGameCoreTick,
		EndTurnBlockingChanged = onEndTurnBlockingChanged,
		UnitMoveComplete = function(player, unitId) CivvisQueue.onUnitSettled(player, unitId); end,
		UnitOperationDeactivated = function(player, unitId) CivvisQueue.onUnitSettled(player, unitId); end,
		CityAddedToMap = onGameCoreTick,
		UnitAddedToMap = onGameCoreTick,
		CityProductionCompleted = onGameCoreTick,
		GovernorAppointed = onGovernorAppointed,
		DiplomacyIncomingDeal = CivvisOnIncomingDeal,
		EmergencyAvailable = CivvisOnAidEmergencyAvailable,
		LoadGameViewStateDone = ensureStarted,
		TeamVictory = onTeamVictory,
		PlayerDefeat = onPlayerDefeat,
		-- The tactical ledger: see CivvisLedger.
		CombatVisBegin = CivvisLedger.onCombatVisBegin,
		CombatVisEnd = CivvisLedger.onCombatVisEnd,
		UnitDamageChanged = CivvisLedger.onUnitDamageChanged,
		UnitRemovedFromMap = CivvisLedger.onUnitRemoved,
		CityOccupationChanged = CivvisLedger.onCityOccupationChanged,
	}) do
		pcall(function() Events[name].Add(handler); end);
	end
end

Initialize();
