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
-- Capital city id -> true while the live opening's second Settler is still
-- committed in the host queue.  Unlike `civvisBuild`, this deliberately spans
-- turns: the first Settler may found city two between two fresh CIVVIS boards,
-- while the second is still building in the capital.  Keep this a bare global:
-- the control chunk is at Lua 5.1's file-local limit, and a local here can make
-- the entire game-side agent refuse to load.
CivvisOpeningSettlerLocks = {};
-- A full policy request is an asynchronous host transaction.  `pcall` can
-- succeed while the game applies only part of it (the live Communism trace
-- repeatedly kept the old Liberalism card and omitted the newly requested New
-- Deal).  Keep the last request on a global table so the next export can name
-- the per-slot readback and the next turn can repair one missing card without
-- racing another same-turn policy transaction.  Globals are intentional: this
-- chunk is at Lua 5.1's 200-local limit.
CivvisPolicy = {
	signature = nil,
	sent_signature = nil,
	sent_turn = -1,
	attempt_turn = -1,
	pending = nil,
};
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
	-- Mod-side write time in seconds on whichever clock this Lua sandbox offers
	-- (none of them is guaranteed; a build with none simply omits `t`). The
	-- harness stamps receipt as `utc`; the two together separate the log's
	-- delivery delay from the agent's own cadence, which receipt alone could not
	-- (lines reach the harness in batches, so 13 polls read as one instant).
	pcall(function()
		local t = Automation.GetTime();
		if type(t) == "number" then payload.t = t; end
	end);
	if payload.t == nil then
		pcall(function()
			local t = UI.GetElapsedTime();
			if type(t) == "number" then payload.t = t; end
		end);
	end
	if payload.t == nil then
		pcall(function()
			local t = os.clock();
			if type(t) == "number" then payload.t = t; end
		end);
	end
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

-- The resource types the shipped diplomacy screen currently puts on a
-- rival's side of a working deal, narrowed to luxuries this seat actually
-- lacks. Kept as a bare global because the control agent is already at the
-- Lua 5.1 main-chunk local ceiling; the exporter and its offline regression
-- both call the same predicate instead of maintaining two trade catalogues.
CivvisTradeableLuxuries = function(player, pid, otherId)
	local deal = DealManager.GetWorkingDeal(DealDirection.OUTGOING, pid, otherId);
	if deal == nil then return nil; end
	local possible = DealManager.GetPossibleDealItems(
		otherId, pid, DealItemTypes.RESOURCES, deal) or {};
	local resources = player:GetResources();
	if resources == nil then return nil; end
	local out = {};
	for _, entry in ipairs(possible) do
		if entry.IsValid ~= false and (entry.MaxAmount or 0) > 0 then
			local row = try(function() return GameInfo.Resources[entry.ForType]; end, nil);
			local owned = try(function()
				return resources:GetResourceAmount(entry.ForType);
			end, nil);
			if row ~= nil and row.ResourceClassType == "RESOURCECLASS_LUXURY"
				and owned == 0 then
				local key = "RESOURCES:" .. tostring(entry.ForType);
				local selling = false;
				local trade = CivvisTrade;
				if trade ~= nil and trade.pending ~= nil then
					for _, pending in pairs(trade.pending) do
						if pending.gave ~= nil and pending.gave[key] ~= nil then
							selling = true;
						end
					end
				end
				if not selling then out[#out + 1] = row.ResourceType; end
			end
		end
	end
	table.sort(out);
	return out;
end;


-- --------------------------------------------------------------- action ids
--
-- Operations are looked up in GameInfo, not on the UnitOperationTypes table.
-- The named table is a convenience list and it is *not* complete: on this
-- build it has no SKIP_TURN or SLEEP, while the database defines both.
-- Reading a missing name off the enum yields nil,
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
		-- Two adjacent friendly units trading hexes. `VisibleInUI="false"` in
		-- `Base/Assets/Gameplay/Data/UnitOperations.xml:115`: the shipped UI
		-- never shows a button for it, it requests it from `RequestMoveOperation`
		-- (`Base/Assets/UI/Civ6Common.lua:160-161`) whenever the destination
		-- plot holds a friendly unit. `applyOrder` takes verb `SWAP`.
		"UNITOPERATION_SWAP_UNITS",
		-- The air verbs. `Base/Assets/Gameplay/Data/UnitOperations.xml:66`
		-- (`UNITOPERATION_AIR_ATTACK`, `InterfaceMode="INTERFACEMODE_AIR_ATTACK"`),
		-- `:93` (`UNITOPERATION_REBASE`) and `:72` (`UNITOPERATION_DEPLOY`).
		-- There is no `AIR_PATROL` row on this build: a fighter's patrol IS the
		-- shipped "Deploy" — it flies to a plot within range and intercepts from
		-- there — so `applyOrder`'s verb `PATROL` resolves to DEPLOY. All three
		-- take `{PARAM_X, PARAM_Y}`, the way `WorldInput.lua:2077`, `:2418` and
		-- `:2486` request them.
		"UNITOPERATION_AIR_ATTACK", "UNITOPERATION_REBASE", "UNITOPERATION_DEPLOY",
		"UNITOPERATION_FORTIFY", "UNITOPERATION_ALERT",
		"UNITOPERATION_SKIP_TURN", "UNITOPERATION_SLEEP",
		"UNITOPERATION_HEAL",
		"UNITOPERATION_BUILD_IMPROVEMENT", "UNITOPERATION_REPAIR", "UNITOPERATION_RANGE_ATTACK",
		-- Pillage was never resolved, so `Action::Pillage` had no host verb and
		-- light cavalry's pillage-before-combat could not happen on the live
		-- seat. Parameterless, like FORTIFY: the unit pillages the tile it is on.
		"UNITOPERATION_PILLAGE",
		"UNITOPERATION_HARVEST_RESOURCE", "UNITOPERATION_REST_REPAIR",
		"UNITOPERATION_MAKE_TRADE_ROUTE", "UNITOPERATION_SPREAD_RELIGION",
		-- This begins the Apostle's native belief-selection prompt. The order
		-- handler keeps CIVVIS's selected belief and completes that prompt with
		-- the same ADD_BELIEF player operation the shipped ReligionScreen uses.
		"UNITOPERATION_EVANGELIZE_BELIEF",
		-- These entries have no InterfaceMode in Firaxis' UnitOperations table,
		-- so UnitPanel requests each directly with no parameters, like spreading.
		--
		-- Read off the installed game, not recalled. In
		-- `Base/Assets/Gameplay/Data/UnitOperations.xml`:
		--   :24 / :83   UNITOPERATION_LAUNCH_INQUISITION
		--   :36 / :95   UNITOPERATION_REMOVE_HERESY
		--   :62 / :121  UNITOPERATION_RELIGIOUS_HEAL
		--   :12 / :71   UNITOPERATION_CONVERT_BARBARIANS
		-- none of which carries an `InterfaceMode` attribute; the shipped
		-- `Base/Assets/UI/Panels/UnitPanel.lua:2518-2535` then takes its
		-- "No mode needed, just do the operation" branch and calls
		-- `UnitManager.RequestOperation(pSelectedUnit, actionHash)` with no
		-- parameter table at all. The order layer may still use `{}` as its
		-- empty logical parameter table; `operate` omits it at this host boundary.
		"UNITOPERATION_LAUNCH_INQUISITION", "UNITOPERATION_REMOVE_HERESY",
		"UNITOPERATION_RELIGIOUS_HEAL", "UNITOPERATION_CONVERT_BARBARIANS",
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
		-- ★★★ CORPS AND ARMY, THE OTHER CONSOLIDATION AND THE ONLY ONE NEVER
		-- WIRED. `ENTER_FORMATION` above links an ESCORT (a support unit riding
		-- with a combat unit); merging two identical units into one stronger one
		-- is a different pair of commands entirely, and CIVVIS decided it 10,015
		-- times across the live archive without a verb to send. Read off the
		-- installed game at `Base/Assets/Gameplay/Data/UnitCommands.xml`:
		--   :20 / :44  UNITCOMMAND_FORM_CORPS (PrereqCivic CIVIC_NATIONALISM)
		--   :21 / :45  UNITCOMMAND_FORM_ARMY  (PrereqCivic CIVIC_MOBILIZATION)
		"UNITCOMMAND_FORM_CORPS", "UNITCOMMAND_FORM_ARMY",
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
		-- Host-side bridge repair keeps an unambiguously co-located combat guard
		-- with a moving settler even when the planner omitted that guard's row.
		settler_escort_sync = cfg.SettlerEscortCapSync ~= false,
		-- Mid-turn replan frames: after the opening orders settle, a board
		-- with newly revealed ground and movement left to spend on it (or a
		-- strike) is exported again and the same turn re-planned, up to
		-- `ReplanFrames` times.
		replan_frames = (tonumber(cfg.ReplanFrames) or 0) > 0,
		-- Newly revealed plots cross every turn and every frame as `tiles`
		-- deltas, not only with the periodic sweep. See CivvisTiles.
		tile_delta = cfg.TileDelta ~= false,
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

	-- Optional game modes survive SetToDefaults on this build.  The in-game
	-- rehost is the capture-free recovery route, so it must establish the same
	-- exact mode contract as the FrontEnd setup context rather than inheriting a
	-- previous throwaway game's Dramatic Ages (or any other) setting.
	for _, mode in ipairs(GAME_MODES) do
		local wanted = (cfg.GameModes or {})[mode];
		GameConfiguration.SetValue(mode, wanted and true or false);
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

-- ★★★ THE MILITARY FORMATION TIER, WITHOUT WHICH ARMY IS UNREACHABLE LIVE.
--
-- #2373 wired `Action::CombineUnits` to Firaxis' TWO merge commands --
-- `UNITCOMMAND_FORM_CORPS` and `UNITCOMMAND_FORM_ARMY` -- and `civvis_orders`
-- picks which one to send from the MIRROR's `Unit::formation`. But the live seat
-- runs `--fresh-board`: the mirror is rebuilt from this export every turn, this
-- export carried no tier at all, so every unit read back as STANDARD and the
-- seat could only ever ask for FORM_CORPS. The Army half of the whole
-- unit-consolidation layer was unreachable live for exactly that reason.
--
-- ⚠ NOT `GetFormationUnitCount`, which this file also exports as
-- `formation_count`. That is Firaxis' ESCORT stack size -- a Settler riding with
-- a Warrior reports 2 -- and it is what `LinkUnits` reconstructs. A Corps is ONE
-- unit and reports a count of 1. Two different mechanisms, both exported.
--
-- Read off the installed game, not recalled. The accessor is
-- `Unit:GetMilitaryFormation()`, which the shipped UI calls at
-- `Base/Assets/UI/WorldTracker.lua:507`,
-- `Base/Assets/UI/Panels/UnitPanel.lua:2259` and `:4018`, and
-- `Base/Assets/UI/Screens/ReportScreen.lua:314`. It is a real binding on this
-- build, not just a Windows one: the name is present in
-- `Civ6.app/Contents/MacOS/GameCore_Base.dll`, and the Win64 map for the same
-- build names its Lua binding at
-- `Assets/DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.map:50977`
-- (`?lGetMilitaryFormation@IUnit@Lua@GameCore@@`).
--
-- ⚠⚠⚠ THE ENUM IS REGISTERED TWICE, UNDER ONE NAME, WITH DIFFERENT MEMBERS.
-- Civilization VI builds two Lua virtual machines and each contributes globals
-- to this script. Both register a table called `MilitaryFormationTypes`, and the
-- member names DO NOT MATCH. Read straight off the installed binaries, as
-- `strings -a -t d` byte offsets:
--
--   Civ6.app/Contents/MacOS/GameCore_Base.dll   (gameplay bindings: Unit,
--   Players, UnitManager, DefenseTypes, UnitCommandTypes -- the 75 gameplay
--   enum reads in this file)
--     12606080  MilitaryFormationTypes
--     12606103  STANDARD_FORMATION
--     12606122  CORPS_FORMATION
--     12606138  ARMY_FORMATION
--
--   Civ6.app/Contents/MacOS/Civ6_Exe_Child     (the UI framework: ContextPtr,
--   LuaEvents, UIManager -- this script is an `AddUserInterfaces` context)
--     26900226  MilitaryFormationTypes
--     26900271  STANDARD_MILITARY_FORMATION
--     26900299  CORPS_MILITARY_FORMATION
--     26900324  ARMY_MILITARY_FORMATION
--
-- Neither binary contains the other's spelling, and Firaxis' own shipped UI uses
-- BOTH: `WorldTracker.lua:512-520`, `ReportScreen.lua:317-321`,
-- `UnitPanel.lua:4022-4030` and `CitySupport.lua:248-259` compare against the
-- SHORT names, while `CitySupport.lua:87-89`, `ToolTipHelper.lua:585-593` and
-- `ProductionPanel.lua:314-456` write the LONG ones. At least one of those two
-- families is comparing against `nil` in any given context and is dead code.
--
-- ⚠ So this asks for BOTH and does not bet on either. Picking one and being
-- wrong would classify every Corps as "not one of the three" forever, silently
-- -- the same nil-literal failure family as the guessed operation name #2373
-- avoided, and unresolvable from here because the ladder is halted and no live
-- game can be asked which VM wins.
--
-- ⚠ THE FAILURE MUST NOT READ AS STANDARD. `try(..., 0)` would hand back
-- "standard" on a build where the accessor is missing or renamed -- which is
-- exactly the sentinel trap `GetDefenseStrength` fell into for the whole
-- project's life (see `cityDefence`), where the fallback was indistinguishable
-- from an answer. Here it would be worse, because 0 is a LEGAL tier: the board
-- would assert that every unit is a plain unit and keep asking for a Corps with
-- nothing anywhere to show it was guessing. An unreadable tier is exported as
-- -1, and the mirror leaves the board's own value alone. That is the same
-- three-valued convention `envoys_free` uses: a real reading, or an explicit
-- "asked, could not answer".
--
-- ⚠ HUNG OFF A GLOBAL, NOT DECLARED AS A FILE-SCOPE `local`. The main chunk is
-- one Lua function and it sits within single digits of Lua's 200-local ceiling;
-- crossing it is a parse error, and a mod script that fails to parse writes
-- NOTHING to any log -- the run looks exactly like one where CIVVIS never
-- decided anything. `test_main_chunk_locals_stay_under_the_limit` refuses the
-- next file-scope local, and this is the shape it asks for. It doubles as the
-- offline test's entry point; ⚠ a bare global, never `_G.`, which the UI sandbox
-- does not expose.
CivvisMilitaryFormation = function(unit)
	return try(function()
		local tier = unit:GetMilitaryFormation();
		if tier == nil or MilitaryFormationTypes == nil then return -1; end
		local tiers = MilitaryFormationTypes;
		-- ⚠ Both spellings, most-specific tier first. A missing member is nil and
		-- `tier` is a number, so an absent spelling simply never matches.
		if tier == tiers.ARMY_FORMATION
				or tier == tiers.ARMY_MILITARY_FORMATION then
			return 2;
		end
		if tier == tiers.CORPS_FORMATION
				or tier == tiers.CORPS_MILITARY_FORMATION then
			return 1;
		end
		if tier == tiers.STANDARD_FORMATION
				or tier == tiers.STANDARD_MILITARY_FORMATION then
			return 0;
		end
		-- A tier this build names and CIVVIS does not model, or a table with
		-- neither spelling. Unknown, not standard: a value we cannot place must
		-- never become a claim.
		return -1;
	end, -1);
end;

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
-- ⚠⚠⚠ `best` IS THE BEST RIVAL WE HAVE MET, NOT THE LEADER. The operator's
-- abandon rule is "under 60% of the leader's score at turn 150 or later", and it
-- reads this number — but a seat that has met two of five majors is being
-- compared against the best of two. Measured over the twelve abandons of
-- 2026-08-30, rivals MET at turn 150 were:
--
--     3, 2, 5, 2, 3, 4, 4, 4, 3, 2, 1, 2      (of five)
--
-- Exactly one run had met the whole field and one had met a single rival. So the
-- rule is systematically LENIENT: the true leader is often unmet and uncounted,
-- our recorded ratio flatters us, and games play on that the rule would have
-- called. The error is in the safe direction — it never abandons a game it
-- should not — but it is not what the rule says.
--
-- `allBest` is the same maximum over every alive major, met or not. It is
-- REPORTING ONLY: nothing decides on it yet, and CIVVIS never sees it, so this
-- cannot leak unmet-civ knowledge into gameplay. It exists so the gap between
-- "the best we have met" and "the leader" can be measured before anyone changes
-- a rule on top of it.
local function rivalBest(player, pid)
	local diplomacy = try(function() return player:GetDiplomacy(); end);
	if diplomacy == nil then return nil, 0, nil, 0; end
	local best, met = nil, 0;
	local allBest, majors = nil, 0;
	for _, otherId in ipairs(try(function() return PlayerManager.GetAliveMajorIDs(); end, {})) do
		if otherId ~= pid then
			majors = majors + 1;
			local score = try(function() return Players[otherId]:GetScore(); end, -1) or -1;
			if score >= 0 and (allBest == nil or score > allBest) then allBest = score; end
			if try(function() return diplomacy:HasMet(otherId); end, false) then
				met = met + 1;
				if score >= 0 and (best == nil or score > best) then best = score; end
			end
		end
	end
	return best, met, allBest, majors;
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
-- and the engine quietly declined to move anything. Parameterless operations
-- are a different overload: the shipped UnitPanel checks them with the strict
-- five-argument form `(unit, hash, nil, false, false)`.
local function canOperate(unit, hash, params)
	if hash == nil then return false; end
	local ok, result = pcall(function()
		if params == nil or next(params) == nil then
			return UnitManager.CanStartOperation(unit, hash, nil, false, false);
		end
		return UnitManager.CanStartOperation(unit, hash, nil, params);
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
	if not canOperate(unit, hash, params) then return false; end
	if params == nil or next(params) == nil then
		-- UnitPanel.lua:2535 calls parameterless operations with exactly two
		-- arguments. Passing `{}` as a third argument is not equivalent on the
		-- live host: the request can return without throwing while FORTIFY,
		-- SKIP_TURN and the other in-place operations never change the unit.
		return pcall(function() UnitManager.RequestOperation(unit, hash); end);
	end
	return pcall(function() UnitManager.RequestOperation(unit, hash, params); end);
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
-- falling back to host-chosen wandering, which is why a game reached turn 50 with
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
		-- neither found nor move, and ended on SKIP_TURN. Measured at turn 20 of
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
	-- No safe site is a reason to wait, not a license for the host to choose a
	-- destination.  The next CIVVIS board will reconsider with fresh state.
	return firstOperation(unit, { "UNITOPERATION_SKIP_TURN" });
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
-- ★ WHY GENERIC EXPLORATION WAS NOT ENOUGH. `findWarTarget` needs a rival city
-- plot to be revealed, and generic frontier wandering did not deliver one:
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
-- Generic frontier wandering charts terrain with no reason to walk toward anybody.
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

local function orderMilitary(unit, player, probeTo)
	-- A probe with somewhere to be walks there. Finding a rival's city means
	-- choosing the rival's border explicitly rather than delegating a route.
	if probeTo ~= nil then
		local params = {};
		params[UnitOperationTypes.PARAM_X] = probeTo.x;
		params[UnitOperationTypes.PARAM_Y] = probeTo.y;
		if operate(unit, OP["UNITOPERATION_MOVE_TO"], params) then
			return "probe";
		end
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
	-- The assault only ever aims at a declared target, so the war-state
	-- question (`CivvisLedger.warStarters`) answers empty here; it is asked
	-- anyway, because a declaration the host refused would otherwise turn
	-- the first volley into a silent surprise war.
	if CivvisLedger ~= nil and CivvisLedger.refuseWarStarter(
			unit, try(function() return unit:GetID(); end), "RANGE_ATTACK",
			warTarget.x, warTarget.y, turn) ~= nil then
		return nil;
	end
	local params = {};
	params[UnitOperationTypes.PARAM_X] = warTarget.x;
	params[UnitOperationTypes.PARAM_Y] = warTarget.y;
	-- Ranged legality is target-specific. The parameterless preflight used to
	-- ask a different question from the request below, so a valid shot could be
	-- rejected before the host ever saw its target. `operate` performs the one
	-- correctly-parameterised check and request.
	if operate(unit, OP["UNITOPERATION_RANGE_ATTACK"], params) then
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
		-- The fallback still names a destination itself; it never asks the host
		-- to invent one. `frontierGround` is memoized for the turn, so scouts
		-- fan toward the same known map edge rather than each receiving a blind
		-- autonomous route.
		return orderMilitary(unit, player, frontierGround(player, pid, turn)) or orderIdle(unit);
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
				probesOut = probesOut + 1;
			end
		end
		return orderMilitary(unit, player, probeTo);
	end
	return nil;
end

-- Work the game's own ready list rather than every unit the player owns.
--
-- Iterating all units and re-issuing an order to each looks equivalent and is
-- not: a unit with a host operation still in progress is still owned, still has
-- movement, and reissuing an operation can restart it, so the ready list
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

	-- ★ OUTSIDE A GOLDEN AGE THE DEDICATION IS ITS QUEST. The `GA_` grants above
	-- apply only in a Golden Age; in a Normal or Dark age the choice grants an
	-- era-score quest instead, and the quest the seat can actually score is
	-- SCIENTIFIC's (Free Inquiry: +1 era score per Eureka). Measured on nine
	-- live King runs: the seat earned 3–12 Eurekas per Classical era against
	-- 1–4 specialty districts, took INFRASTRUCTURE every time, and fell into 15
	-- Dark Ages, ten of them by five points or fewer. In a Golden Age the
	-- expansion grants are real and the order above stands.
	local golden = try(function()
		return Game.GetEras():HasGoldenAge(pid) or Game.GetEras():HasHeroicGoldenAge(pid);
	end, false);
	local order = DEDICATION_ORDER;
	if not golden then
		order = { "COMMEMORATION_SCIENTIFIC" };
		for _, name in ipairs(DEDICATION_ORDER) do
			if name ~= "COMMEMORATION_SCIENTIFIC" then order[#order + 1] = name; end
		end
	end

	local taken = 0;
	for _, preferred in ipairs(order) do
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

-- The last founding or Apostle enhancement this agent asked the host for,
-- kept until the next export confirms it or catches its failure. Both paths
-- cross asynchronous player operations, so a `pcall` verdict only means that
-- the request did not throw; it cannot prove the host applied the choice.
local pendingReligionChoice = nil;

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

-- Mark a city-state token prompt as considered without spending anything. This
-- is the shipped CityStates.lua Close() operation, kept behind a fresh
-- GetInfluence() handle because GIVE_INFLUENCE_TOKEN rewrites that gameplay
-- object. A CIVVIS order may deliberately leave a token unspent; that is not
-- a reason to let the native prompt stop the turn.
--
-- A bare global is intentional: the agent's main chunk is at Lua's local-slot
-- ceiling, and the same small operation is needed by the CIVVIS blocker arm
-- below without adding another file-scope local.
CivvisMarkEnvoyConsidered = function(player)
	local ran, marked = pcall(function()
		local fresh = player:GetInfluence();
		if fresh == nil then return false; end
		if not fresh:IsGivingTokensConsidered() then
			fresh:SetGivingTokensConsidered(true);
		end
		return true;
	end);
	return ran and marked == true;
end

local function currentBlocker(pid)
	return try(function()
		return NotificationManager.GetFirstEndTurnBlocking(pid);
	end);
end

-- The shipped `EspionageEscape.lua` answers this prompt with a
-- `SET_ESCAPE_ROUTE` player operation.  The notification is not guaranteed to
-- open that popup in an unattended seat, so use the same native operation from
-- the agent.  City Center is the fourth shipped choice and is always enabled;
-- this is a route choice, not a movement order, so it cannot move a unit or
-- alter the mirrored position while it clears the blocker.
CivvisChooseSpyEscapeRoute = function(pid)
	return try(function()
		local params = {};
		params[PlayerOperations.PARAM_DISTRICT_TYPE] =
			GameInfo.Districts["DISTRICT_CITY_CENTER"].Index;
		UI.RequestPlayerOperation(pid, PlayerOperations.SET_ESCAPE_ROUTE, params);
		return true;
	end, false);
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
	-- no-spend policy stands; the blocker arm safely marks any leftover prompt
	-- considered and advances the turn without invoking this crash-prone lane.
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
	-- Escape-route choice is answered by the native operation in the soft arm
	-- below; it stays here so the game does not treat it as an ordinary hard
	-- decision while the operation settles.
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
	-- CIVVIS chooses the exact dedication on its era-boundary board. Keep the
	-- native ladder from spending a different choice while that operation is
	-- still crossing the asynchronous player-operation boundary.
	ENDTURN_BLOCKING_COMMEMORATION_AVAILABLE = true,
	-- An Apostle's EVANGELIZE_BELIEF operation raises this prompt. The pending
	-- order stores the exact CIVVIS choice, and `answerBlocker` supplies it
	-- instead of allowing a generic chooser to race the operation.
	ENDTURN_BLOCKING_BELIEF = true,
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
	ENDTURN_BLOCKING_COMMEMORATION_AVAILABLE = "dedication",
	ENDTURN_BLOCKING_BELIEF = "unit",
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
	-- Firaxis's UnitPanel starts the Apostle operation, then ReligionScreen
	-- confirms the selected belief with ADD_BELIEF. Keep those two asynchronous
	-- steps together through a pending record so the exact CIVVIS choice reaches
	-- the prompt and a non-throwing request is still verified by exportState.
	if name == "ENDTURN_BLOCKING_BELIEF"
			and pendingReligionChoice ~= nil
			and pendingReligionChoice.mode == "evangelize" then
		if pendingReligionChoice.add_requested then return "civvis_complete"; end
		local params = {};
		params[PlayerOperations.PARAM_BELIEF_TYPE] = pendingReligionChoice.belief_hash;
		params[PlayerOperations.PARAM_INSERT_MODE] = PlayerOperations.VALUE_EXCLUSIVE;
		local ok = pcall(function()
			UI.RequestPlayerOperation(pid, PlayerOperations.ADD_BELIEF, params);
		end);
		if ok then pendingReligionChoice.add_requested = true; end
		return ok and "evangelize_belief" or nil;
	end
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
	-- arm therefore needs one residual answer, which skips only this return:
	-- by then the CIVVIS reply is landed and applied (source == civvis), so the
	-- race this return exists to prevent cannot happen, and the ladder answer is
	-- counted in `residualAnswers` below like every other residual decision.
	--
	-- Do it in this callback for research, civics, and commemoration. These are
	-- hard blockers, and after Firaxis raises one it can stop publishing Game
	-- Core ticks before the ordinary "second sighting" escalation below gets a
	-- chance to run. That was the turn-230 Conservation wedge: the safe residual
	-- choice was made only after a later event happened to arrive, then the game
	-- sat at Please Wait. A live host that has not exported its dedication
	-- allowance gives CIVVIS no dedication order to apply; once its completed
	-- reply leaves the prompt standing, the native chooser is the safe bridge.
	-- Re-entering with `residual_ok` retains the normal spend cap and accounting;
	-- it merely avoids relying on a tick that the blocked engine need not emit.
	-- CIVVIS receives a fresh board next turn and may override this bridge pick.
	if not residual_ok and cfg.CivvisDecides and CIVVIS_OWNED_BLOCKERS[name]
			and (awaiting.source == "civvis" or awaiting.source == "civvis_stale") then
		if name == "ENDTURN_BLOCKING_RESEARCH" or name == "ENDTURN_BLOCKING_CIVIC"
				or name == "ENDTURN_BLOCKING_COMMEMORATION_AVAILABLE" then
			return answerBlocker(player, pid, blocker, turn, true);
		end
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
-- ★★★★★ THE HOST'S OWN PRODUCTION AND PURCHASE MENUS, PER CITY.
--
-- Production is the highest-frequency decision the board makes, and until this
-- block it made that decision from its own catalogue and learned legality only
-- from refusals -- `civvis_build_unplayable` AFTER the order, one item per city
-- per turn, never the set. The predicate that decides was already in this
-- file: `city:GetBuildQueue():CanProduce(hash, false, true)` inside the chooser
-- above and `city:GetGold():GetPurchaseCost(currency, hash)` inside the purchase
-- actuator below. Neither was ever exported.
--
-- The loops are the shipped `UI/Panels/ProductionPanel.lua`'s, family by
-- family (districts 1898-2003, buildings 2019-2100, units 2107-2203, projects
-- 2207-2237): `CanProduce(hash, true)` is the EXCLUSION test that decides
-- whether an item is listed at all, `CanProduce(hash, false, true)` whether it
-- can be STARTED now, and the typed cost accessors take `row.Index` -- see
-- `productionProgress` for what the hash form does. Purchases follow
-- `ComposeUnitForPurchase` / `ComposeBldgForPurchase` /
-- `ComposeDistrictForPurchase` (1632-1820): `CityManager.CanStartCommand(city,
-- PURCHASE, true, params, false)` lists, the same call with `(false, params,
-- true)` decides, `GetPurchaseCost(yield, hash)` prices. The queue behind the
-- head is `ProductionHelper.lua:185` -- `GetAt(i)` for i >= 1, each entry a
-- `Directive` and a typed index.
--
-- Only what the host says can START now crosses; a listed-but-disabled item is
-- not a choice. Compact on purpose, because it rides on every state export of
-- every city: one `{t, c, p}` per startable item, `f` = 1/2 for a Corps/Army
-- the results table says the city may train, `n`/`s` the number of plots the
-- engine offers a district and up to sixteen of them (the mirror trusts the
-- list only when it is complete). Corps/Army PURCHASES are not priced.
--
-- ⚠ Each family is guarded on its own and the whole record under `try` at the
-- call site: a nil answer is "unknown" on the mirror (no gate), never "nothing
-- buildable". A global table, not a file-scope local -- the main chunk is at
-- Lua's 200-local ceiling.
CivvisMenus = {};

-- `AdjacencyBonusSupport.lua:280`: the engine's own placement offer for a
-- district. Returns the count and up to sixteen of the plots.
CivvisMenus.plots = function(city, param, hash)
	local plots = try(function()
		local probe = {};
		probe[param] = hash;
		local results = CityManager.GetOperationTargets(
			city, CityOperationTypes.BUILD, probe);
		return results and results[CityOperationResults.PLOTS] or nil;
	end);
	if plots == nil then return nil, nil; end
	local offered, out = 0, {};
	for _, plotIndex in pairs(plots) do
		local plot = try(function() return Map.GetPlotByIndex(plotIndex); end);
		local px = plot and try(function() return plot:GetX(); end, -1) or -1;
		local py = plot and try(function() return plot:GetY(); end, -1) or -1;
		if px >= 0 and py >= 0 then
			offered = offered + 1;
			if #out < 16 then out[#out + 1] = { x = px, y = py }; end
		end
	end
	return offered, out;
end;

CivvisMenus.buildable = function(city)
	local queue = city:GetBuildQueue();
	if queue == nil then return nil; end
	local function listed(arg)
		return try(function() return queue:CanProduce(arg, true); end, false) == true;
	end
	-- The three-argument form returns (can, results); a raise is "no".
	local function startable(arg)
		local ok, can, results = pcall(function()
			return queue:CanProduce(arg, false, true);
		end);
		if ok and can == true then return true, results; end
		return false, nil;
	end
	local out = {};
	for row in GameInfo.Districts() do
		if not row.InternalOnly and listed(row.Hash) and startable(row.Hash) then
			local offered, plots = CivvisMenus.plots(
				city, CityOperationTypes.PARAM_DISTRICT_TYPE, row.Hash);
			out[#out + 1] = {
				t = row.DistrictType,
				c = try(function() return queue:GetDistrictCost(row.Index); end, -1),
				p = try(function() return queue:GetTurnsLeft(row.DistrictType); end, -1),
				n = offered,
				s = plots,
			};
		end
	end
	for row in GameInfo.Buildings() do
		if not row.MustPurchase and listed(row.Hash) and startable(row.Hash) then
			out[#out + 1] = {
				t = row.BuildingType,
				c = try(function() return queue:GetBuildingCost(row.Index); end, -1),
				p = try(function() return queue:GetTurnsLeft(row.Hash); end, -1),
			};
		end
	end
	-- Both spellings of the tier enum, for the reason `CivvisMilitaryFormation`
	-- records: the two Lua VMs name the members differently.
	local tiers = MilitaryFormationTypes or {};
	local standard = tiers.STANDARD_MILITARY_FORMATION or tiers.STANDARD_FORMATION;
	local corps = tiers.CORPS_MILITARY_FORMATION or tiers.CORPS_FORMATION;
	local army = tiers.ARMY_MILITARY_FORMATION or tiers.ARMY_FORMATION;
	for row in GameInfo.Units() do
		if not row.MustPurchase then
			local arg = row.Hash;
			if standard ~= nil then
				arg = { UnitType = row.Hash, MilitaryFormationType = standard };
			end
			if listed(arg) then
				local can, results = startable(arg);
				if can then
					out[#out + 1] = {
						t = row.UnitType,
						c = try(function() return queue:GetUnitCost(row.Index); end, -1),
						p = try(function() return queue:GetTurnsLeft(row.Hash); end, -1),
					};
					-- `ProductionPanel.lua:2150-2172`: the results table says whether
					-- a Corps or Army may be trained here; each tier is asked again.
					local canCorps = try(function()
						return results[CityOperationResults.CAN_TRAIN_CORPS] == true;
					end, false);
					if corps ~= nil and canCorps
							and startable({ UnitType = row.Hash, MilitaryFormationType = corps }) then
						out[#out + 1] = {
							t = row.UnitType,
							f = 1,
							c = try(function() return queue:GetUnitCorpsCost(row.Index); end, -1),
							p = try(function() return queue:GetTurnsLeft(row.Hash, corps); end, -1),
						};
					end
					local canArmy = try(function()
						return results[CityOperationResults.CAN_TRAIN_ARMY] == true;
					end, false);
					if army ~= nil and canArmy
							and startable({ UnitType = row.Hash, MilitaryFormationType = army }) then
						out[#out + 1] = {
							t = row.UnitType,
							f = 2,
							c = try(function() return queue:GetUnitArmyCost(row.Index); end, -1),
							p = try(function() return queue:GetTurnsLeft(row.Hash, army); end, -1),
						};
					end
				end
			end
		end
	end
	for row in GameInfo.Projects() do
		if listed(row.Hash) and startable(row.Hash) then
			out[#out + 1] = {
				t = row.ProjectType,
				c = try(function() return queue:GetProjectCost(row.Index); end, -1),
				p = try(function() return queue:GetTurnsLeft(row.ProjectType); end, -1),
			};
		end
	end
	return out;
end;

CivvisMenus.purchasable = function(city)
	local gold = try(function() return GameInfo.Yields["YIELD_GOLD"].Index; end);
	local faith = try(function() return GameInfo.Yields["YIELD_FAITH"].Index; end);
	if gold == nil or faith == nil then return nil; end
	local wallet = try(function() return city:GetGold(); end);
	if wallet == nil then return nil; end
	-- `ComposeUnitForPurchase`: UNIT_TYPE and YIELD_TYPE only. The purchase
	-- actuator below records why the formation parameter must stay off the
	-- eligibility question.
	local function price(param, hash, currency)
		local params = {};
		params[param] = hash;
		params[CityCommandTypes.PARAM_YIELD_TYPE] = currency;
		local shown = try(function()
			return CityManager.CanStartCommand(
				city, CityCommandTypes.PURCHASE, true, params, false);
		end, false);
		if shown ~= true then return nil; end
		local ok, can = pcall(function()
			return CityManager.CanStartCommand(
				city, CityCommandTypes.PURCHASE, false, params, true);
		end);
		if not ok or can ~= true then return nil; end
		local cost = try(function() return wallet:GetPurchaseCost(currency, hash); end);
		if type(cost) ~= "number" or cost < 0 then return nil; end
		return cost;
	end
	local out, seen = {}, {};
	local function record(name, g, f)
		if name == nil or (g == nil and f == nil) then return; end
		local entry = seen[name];
		if entry == nil then
			entry = { t = name };
			seen[name] = entry;
			out[#out + 1] = entry;
		end
		if g ~= nil then entry.g = g; end
		if f ~= nil then entry.f = f; end
	end
	for row in GameInfo.Units() do
		local g, f = nil, nil;
		if row.PurchaseYield == "YIELD_GOLD" then
			g = price(CityCommandTypes.PARAM_UNIT_TYPE, row.Hash, gold);
		end
		if row.PurchaseYield == "YIELD_FAITH" or try(function()
			return wallet:IsUnitFaithPurchaseEnabled(row.Hash);
		end, false) == true then
			f = price(CityCommandTypes.PARAM_UNIT_TYPE, row.Hash, faith);
		end
		record(row.UnitType, g, f);
	end
	for row in GameInfo.Buildings() do
		local g, f = nil, nil;
		if row.PurchaseYield == "YIELD_GOLD" then
			g = price(CityCommandTypes.PARAM_BUILDING_TYPE, row.Hash, gold);
		end
		if row.PurchaseYield == "YIELD_FAITH" or try(function()
			return wallet:IsBuildingFaithPurchaseEnabled(row.Hash);
		end, false) == true then
			f = price(CityCommandTypes.PARAM_BUILDING_TYPE, row.Hash, faith);
		end
		record(row.BuildingType, g, f);
	end
	for row in GameInfo.Districts() do
		if not row.InternalOnly then
			record(row.DistrictType,
				price(CityCommandTypes.PARAM_DISTRICT_TYPE, row.Hash, gold),
				price(CityCommandTypes.PARAM_DISTRICT_TYPE, row.Hash, faith));
		end
	end
	return out;
end;

-- The queue BEHIND the head. The head crosses as `producing`; until this the
-- rest of a real multi-item queue never did.
CivvisMenus.queue = function(city)
	local queue = city:GetBuildQueue();
	if queue == nil then return nil; end
	local size = try(function() return queue:GetSize(); end, 0) or 0;
	if size <= 1 then return nil; end
	local directives = CityProductionDirectives;
	if directives == nil then return nil; end
	local tiers = MilitaryFormationTypes or {};
	local out = {};
	for i = 1, size - 1 do
		local entry = try(function() return queue:GetAt(i); end);
		local directive = type(entry) == "table" and entry.Directive or nil;
		local rec = nil;
		if directive == nil then
			rec = nil;
		elseif directive == directives.TRAIN then
			local row = try(function() return GameInfo.Units[entry.UnitType]; end);
			if row ~= nil then
				rec = {
					t = row.UnitType,
					pr = try(function() return queue:GetUnitProgress(row.Index); end),
				};
				local tier = entry.MilitaryFormationType;
				if tier ~= nil and (tier == tiers.CORPS_FORMATION
						or tier == tiers.CORPS_MILITARY_FORMATION) then
					rec.f = 1;
				elseif tier ~= nil and (tier == tiers.ARMY_FORMATION
						or tier == tiers.ARMY_MILITARY_FORMATION) then
					rec.f = 2;
				end
			end
		elseif directive == directives.CONSTRUCT then
			local row = try(function() return GameInfo.Buildings[entry.BuildingType]; end);
			if row ~= nil then
				rec = {
					t = row.BuildingType,
					pr = try(function() return queue:GetBuildingProgress(row.Index); end),
				};
			end
		elseif directive == directives.ZONE then
			local row = try(function() return GameInfo.Districts[entry.DistrictType]; end);
			if row ~= nil then
				rec = {
					t = row.DistrictType,
					pr = try(function() return queue:GetDistrictProgress(row.Index); end),
				};
			end
		elseif directive == directives.PROJECT then
			local row = try(function() return GameInfo.Projects[entry.ProjectType]; end);
			if row ~= nil then
				rec = {
					t = row.ProjectType,
					pr = try(function() return queue:GetProjectProgress(row.Index); end),
				};
			end
		end
		if rec ~= nil then out[#out + 1] = rec; end
	end
	return out;
end;

-- `GetActivationHighlightPlots` returns every matching district the host UI
-- can highlight, including districts behind a closed border.  Great People
-- can activate only in our empire, so crossing a foreign highlight would
-- make the bridge issue the same MOVE_TO forever while the host correctly
-- refuses to move the unit.  Keep this ownership gate at the host export
-- boundary; the planner should never have to infer legality from a failed
-- movement order.
CivvisGreatPersonActivationPlots = function(unit, gp, pid, gwSurvey, openPlots)
	local activationPlots = {};
	local districtPlots = gwSurvey ~= nil and gwSurvey.district_plots or nil;
	for _, plotIndex in ipairs(try(function()
		return gp:GetActivationHighlightPlots();
	end, {}) or {}) do
		local plot = try(function() return Map.GetPlotByIndex(plotIndex); end);
		local owner = plot ~= nil and try(function() return plot:GetOwner(); end, -1) or -1;
		if plot ~= nil and owner == pid then
			local px = try(function() return plot:GetX(); end, -1);
			local py = try(function() return plot:GetY(); end, -1);
			if px >= 0 and py >= 0 then
				-- Three-valued on purpose, and only for slot consumers:
				-- true = a compatible empty slot stands here; false =
				-- one of our districts with no such slot; nil/absent =
				-- unknown (a wonder tile, or no survey). The brain must
				-- never read absence as either claim.
				local slotOpen = nil;
				if openPlots ~= nil then
					if openPlots[plotIndex] then slotOpen = true;
					elseif districtPlots ~= nil and districtPlots[plotIndex] then
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
	return activationPlots;
end;

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
	-- ★★★ THE CAPTURED CITY THE HOST IS WAITING ON. Civilization VI blocks the
	-- turn with ENDTURN_BLOCKING_CONSIDER_RAZE_CITY until the conqueror keeps,
	-- razes or liberates a city taken this turn; this controller lists that
	-- blocker as soft and ends the turn over it, so the host's default — keep
	-- — decided every capture and the board never saw a decision to make. The
	-- shipped popup finds the city with `GetNextCapturedCity()`
	-- (`Base/Assets/UI/Popups/RazeCity.lua:71`) and the loser with
	-- `GetJustConqueredFrom()` (`:86`); the city record below carries both, on
	-- that one city, so the mirror can set `captured_from` and the board
	-- offers the same three choices the popup does. The `city` order kind in
	-- `applyOrder` carries the answer back.
	local pendingCaptureId = try(function()
		local pending = player:GetCities():GetNextCapturedCity();
		return pending and pending:GetID() or nil;
	end, nil);
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
			-- See `pendingCaptureId`: the Firaxis player this city was just taken
			-- from, present exactly while the host waits on its disposition; and
			-- its founder (`GetOriginalOwner`, `RazeCity.lua:85`), whom LIBERATE
			-- hands it to.
			captured_from = (pendingCaptureId ~= nil
				and pendingCaptureId == try(function() return city:GetID(); end, nil))
				and try(function() return city:GetJustConqueredFrom(); end, nil) or nil,
			original_owner = try(function() return city:GetOriginalOwner(); end, nil),
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
			-- ★ THE HOST'S MENUS -- see `CivvisMenus` above. What this city can
			-- START now, with the engine's cost and turns; what it can BUY now,
			-- with the engine's price; and the queue behind `producing`. Each
			-- is nil when the read fails, which the mirror treats as unknown.
			buildable = try(function() return CivvisMenus.buildable(city); end),
			purchasable = try(function() return CivvisMenus.purchasable(city); end),
			queue = try(function() return CivvisMenus.queue(city); end),
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
			local activationPlots = CivvisGreatPersonActivationPlots(
				unit, gp, pid, gwSurvey, openPlots);
			greatPerson = {
				individual = individualRow ~= nil
					and individualRow.GreatPersonIndividualType or nil,
				class = classType,
				empty_slots = emptySlots,
				-- The timeline moves on as soon as this person is recruited, so the
				-- current offer cannot tell the planner which exact city conditions
				-- this physical unit still needs. Carry all three per-individual
				-- database gates: completed district, missing building, and Great Work
				-- object. `ActionRequiresMissingBuildingType` is intentionally kept
				-- separate from a normal prerequisite because James of St. George
				-- supplies the missing Medieval Walls himself.
				required_district = individualRow ~= nil
					and individualRow.ActionRequiresCompletedDistrictType or nil,
				required_missing_building = individualRow ~= nil
					and individualRow.ActionRequiresMissingBuildingType or nil,
				required_great_work = individualRow ~= nil
					and individualRow.ActionRequiresCityGreatWorkObjectType or nil,
				charges = try(function() return gp:GetActionCharges(); end, 0),
				can_activate = try(function()
					return UnitManager.CanStartCommand(
						unit, CMD["UNITCOMMAND_ACTIVATE_GREAT_PERSON"], nil, {});
				end, false),
				activation_plots = activationPlots,
			};
		end
		local progress = unitProgress(unit);
		-- ★★★ THE HOST'S OWN UPGRADE VERDICT, MAINTENANCE, ACTIVITY AND SPY
		-- STATE, ONE-TO-ONE (docs/FIDELITY.md, "The one-to-one map", item 9).
		-- The board derived every one of these from its own rules: `UPGRADE`
		-- was 933 refusals over the 08-04/08-05 runs because the board's
		-- successor, territory and price rules disagreed with the host's, and
		-- `SPY_GAIN_SOURCES` was refused 195 of 862 times in one run because
		-- the board re-tasked a Spy the host already had on an operation.
		--
		-- Upgrade, the shipped UnitPanel.lua:470-483 read: the loose
		-- `CanStartCommand(pUnit, UPGRADE, true, true)` returns the results
		-- table whose `UnitCommandResults.UNIT_TYPE` names the successor, the
		-- strict `(false, true)` says whether it can start NOW, and
		-- `pUnit:GetUpgradeCost()` is the bill. The strict call's
		-- `FAILURE_REASONS` is the table `upgradeUnit` already reads; its
		-- first entry crosses as `upgrade_blocked_reason`, and a block the
		-- host would not name crosses as "unnamed" so the verdict still
		-- stands. A unit with no successor at all exports none of the three.
		local upgradeTo, upgradeCost, upgradeBlocked = nil, nil, nil;
		try(function()
			local hash = CMD["UNITCOMMAND_UPGRADE"];
			if hash == nil then return; end
			local now, strict = UnitManager.CanStartCommand(unit, hash, false, true);
			local ever, loose = UnitManager.CanStartCommand(unit, hash, true, true);
			if ever ~= true and now ~= true then return; end
			local keys = UnitCommandResults;
			local kind = nil;
			if keys ~= nil then
				if type(strict) == "table" then kind = strict[keys.UNIT_TYPE]; end
				if kind == nil and type(loose) == "table" then
					kind = loose[keys.UNIT_TYPE];
				end
			end
			local target = kind ~= nil and GameInfo.Units[kind] or nil;
			upgradeTo = target ~= nil and target.UnitType or nil;
			upgradeCost = unit:GetUpgradeCost();
			if now == true then return; end
			local reasons = (keys ~= nil and type(strict) == "table")
				and strict[keys.FAILURE_REASONS] or nil;
			upgradeBlocked = (type(reasons) == "table" and reasons[1] ~= nil)
				and tostring(reasons[1]) or "unnamed";
		end);
		-- Activity: `UnitManager.GetActivityType`, the WorldTracker.lua:544
		-- status read. The stock UnitActivities.artdef supplies the named
		-- activity values below; `NO_ACTIVITY` is also exposed by ActivityTypes.
		-- UnitPanel.lua:4054-4062 treats every non-AWAKE activity with fortify
		-- turns as defense, so an omitted enum becomes a misleading raw hash in
		-- the mirror precisely while a unit is standing sentry or healing.
		-- Named through the enum, never the raw integer, for the same reason
		-- `CivvisMilitaryFormation` is. Keep the raw fallback only for a future
		-- host value that is genuinely unknown to this exporter.
		local activity = try(function()
			local kind = UnitManager.GetActivityType(unit);
			if kind == nil then return nil; end
			for _, label in ipairs({
				"SLEEP", "HOLD", "OPERATION", "AWAKE",
				"HEAL", "SENTRY", "INTERCEPT", "NO_ACTIVITY",
				"BUILD", "DIG", "CUT", "REPAIR",
				"SPREAD_RELIGION", "LAUNCH_INQUISITION",
				"EVANGELIZE_BELIEF", "EXCAVATE", "DESIGNATE_PARK",
				"FOUND_RELIGION",
			}) do
				local enum = label == "NO_ACTIVITY"
					and ActivityTypes.NO_ACTIVITY
					or ActivityTypes["ACTIVITY_" .. label];
				if enum ~= nil and enum == kind then
					return string.lower(label);
				end
			end
			return tostring(kind);
		end, nil);
		-- Spy: `GetSpyOperation()` is -1 when idle, else the UnitOperations
		-- row (EspionageOverview.lua:659-676, with `GetSpyOperationEndTurn`).
		-- The missions the host would let an IDLE Spy start from the city it
		-- stands in are the EspionageChooser.lua:199-213 loop — counterspy in
		-- our own city, `CategoryInUI == "OFFENSIVESPY"` in anyone else's —
		-- names only. ⚠ nil, not `{}`, when there are none (see
		-- `great_person_points`), and nil when the Spy is busy: the mirror
		-- filters the board's mission menu only on a list with entries.
		local spyOperation, spyEnds, spyMissions = nil, nil, nil;
		if name == "UNIT_SPY" then
			try(function()
				local op = unit:GetSpyOperation();
				if op == nil or op == -1 then return; end
				local opRow = GameInfo.UnitOperations[op];
				spyOperation = opRow ~= nil and opRow.OperationType or tostring(op);
				spyEnds = unit:GetSpyOperationEndTurn();
			end);
			if spyOperation == nil then
				spyMissions = try(function()
					local here = Map.GetPlot(unit:GetX(), unit:GetY());
					local city = Cities.GetPlotPurchaseCity(here);
					if city == nil then return nil; end
					local cityPlot = Map.GetPlot(city:GetX(), city:GetY());
					local ours = city:GetOwner() == pid;
					local names = {};
					for operation in GameInfo.UnitOperations() do
						local wanted = (ours
							and operation.OperationType == "UNITOPERATION_SPY_COUNTERSPY")
							or ((not ours) and operation.CategoryInUI == "OFFENSIVESPY");
						if wanted and UnitManager.CanStartOperation(
								unit, operation.Hash, cityPlot, false, true) == true then
							names[#names + 1] = operation.OperationType;
						end
					end
					if #names == 0 then return nil; end
					table.sort(names);
					return names;
				end, nil);
			end
		end
		local unitId = try(function() return unit:GetID(); end, -1);
		local unitX = try(function() return unit:GetX(); end, -1);
		local unitY = try(function() return unit:GetY(); end, -1);
		CivvisLedger.kinds[tostring(unitId)] = name;
		if unitId >= 0 and unitX >= 0 and unitY >= 0 then
			CivvisLedger.positions[tostring(unitId)] = { x = unitX, y = unitY };
		end
		units[#units + 1] = {
			id = unitId,
			kind = name,
			-- See `unitBaseType`: what this replaces, when it is a civ unique.
			base = unitBaseType(name),
			-- See `unitClass`: the fallback for a unique that replaces nothing.
			class = unitClass(name),
			x = unitX,
			y = unitY,
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
			-- ★★★ CORPS AND ARMY. 0 standard, 1 Corps/Fleet, 2 Army/Armada, and
			-- -1 for "asked, could not answer" -- see `CivvisMilitaryFormation` for the
			-- accessor's citations, for which spelling of the enum actually
			-- exists on this build, and for why the sentinel is -1 rather than 0.
			--
			-- ⚠ This and `formation_count` below are DIFFERENT MECHANISMS with
			-- confusingly similar names. This one is the merge tier that
			-- `FORM_CORPS`/`FORM_ARMY` raise; the count below is the escort stack
			-- that `ENTER_FORMATION` builds. A Corps reports a count of 1.
			formation = CivvisMilitaryFormation(unit),
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
			-- The unit's live range (`Unit:GetRange()`, the shipped
			-- `UnitPanel.lua:2250` read). For an aircraft this is its
			-- operational range from the plot it stands on — which IS its base:
			-- a Civilization VI aircraft sits on its airfield, city or carrier
			-- between sorties, so `x`/`y` name the base and this names how far
			-- an AIR_ATTACK, REBASE or PATROL may reach from it.
			range = try(function() return unit:GetRange(); end, nil),
			-- The host's upgrade verdict: see the block above `units[#units + 1]`.
			-- `upgrade_to` is the successor's UnitType, `upgrade_cost` the Gold
			-- the host would charge, `upgrade_blocked_reason` present exactly
			-- when a successor exists and the command cannot start this turn.
			upgrade_to = upgradeTo,
			upgrade_cost = upgradeCost,
			upgrade_blocked_reason = upgradeBlocked,
			-- The per-type bill the shipped ReportScreen.lua:314-334 sums and
			-- ToolTipHelper.lua:705 prints, by formation the way that screen
			-- reads it: `GetUnitArmyMaintenance` for an Army/Armada,
			-- `GetUnitCorpsMaintenance` for a Corps/Fleet, `GetUnitMaintenance`
			-- otherwise. BEFORE the player's `GetMaintDiscountPerUnit`, which
			-- ReportScreen.lua:338 subtracts afterwards and the board keeps
			-- as its own policy term. ⚠ nil when the tier could not be read
			-- (`CivvisMilitaryFormation` -1): a Corps billed as a plain unit
			-- would read as a discount the host never gave.
			maintenance = try(function()
				if row == nil then return nil; end
				local tier = CivvisMilitaryFormation(unit);
				if tier == 2 then return UnitManager.GetUnitArmyMaintenance(row.Hash); end
				if tier == 1 then return UnitManager.GetUnitCorpsMaintenance(row.Hash); end
				if tier == 0 then return UnitManager.GetUnitMaintenance(row.Hash); end
				return nil;
			end, nil),
			-- UnitPanel.lua:2257 and :2242, the panel's own stat reads.
			religious_strength = try(function() return unit:GetReligiousStrength(); end, nil),
			max_moves = try(function() return unit:GetMaxMoves(); end, nil),
			activity = activity,
			spy_operation = spyOperation,
			spy_operation_end_turn = spyEnds,
			spy_missions_available = spyMissions,
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
						local name = unitTypeName(unit);
						-- `IsVisible`, not `IsRevealed`: a remembered tile does not show
						-- who stands on it now. Confirmed on a player-indexed
						-- `PlayersVisibility` handle, which is the only form that
						-- answers in a gameplay context. A visible tile is not a
						-- detection result, though: `other:GetUnits()` still contains
						-- a foreign Spy while its operation remains secret.
						if name ~= "UNIT_SPY" and PlayersVisibility[pid]:IsVisible(ux, uy) then
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
								-- The same three enemy combat facts `hostiles[]` carries, for a rival's unit;
								-- see the note in `addUnitsOf` below for the shipped reads and
								-- for why an unreadable value stays nil / -1.
								attacks_remaining = try(function() return unit:GetAttacksRemaining(); end, nil),
								formation = CivvisMilitaryFormation(unit),
								embarked = try(function() return unit:IsEmbarked(); end, nil),
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
				-- Whether THEIR border exists at all. Civilization VI shuts a
				-- civilization's territory to foreign units only once it holds
				-- Early Empire ("unlocks the abilities to enforce borders and
				-- grant Open Borders"; CIVIC_ENFORCE_BORDERS hangs off it). The
				-- mirror does not model a rival's civics and used to answer
				-- "free passage" for every rival whose city it could see: run
				-- 184456Z sent 37 military steps into a rival's closed border
				-- and none arrived. nil when the host cannot be asked; the
				-- mirror reads nil as enforced.
				enforces_borders = try(function()
					local civic = GameInfo.Civics["CIVIC_EARLY_EMPIRE"];
					if civic == nil then return nil; end
					return other:GetCulture():HasCivic(civic.Index);
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
				-- ★★★★★ THE RELATIONSHIP ITSELF, ONE-TO-ONE. `can_declare` says a war is
				-- LEGAL; nothing said whether it was ruinous. Every war, peace,
				-- denounce and alliance decision on the board was taken blind to the
				-- host's diplomatic state, grievance ledger, alliance, missions,
				-- promises and visibility (docs/FIDELITY.md, "The one-to-one map",
				-- item 1). Each accessor is one the shipped screens call on the same
				-- objects in this same InGame context:
				--   DiplomacyActionView.lua:870,1473  GetDiplomaticAI():GetDiplomaticStateIndex
				--                                     -> GameInfo.DiplomaticStates[i].StateType
				--   DiplomacyActionView.lua:1486-1511 GetDenounceTurn (both sides),
				--                                     GetDeclaredFriendshipTurn,
				--                                     Game.GetGameDiplomacy():GetDenounceTimeLimit
				--   DiplomacyActionView_WorldCongressTab.lua:42,78
				--                                     GetGrievancesAgainst is ONE SIGNED BALANCE:
				--                                     > 0 ours against them, < 0 theirs against
				--                                     us; GetGrievanceChangePerTurn(them, us)
				--   DiplomacyRibbon_Expansion1.lua:24-27 GetAllianceType (-1 = none) /
				--                                     GetAllianceLevel
				--   DiplomacyActionView_AllianceTab.lua:44 GetAllianceTurnsUntilExpiration
				--   DiplomacyActionView.lua:992       GetVisibilityOn (GameInfo.Visibilities index)
				--   DiplomacyActionView_AllianceRow.lua:61-70 IsPromiseMade(other, PromiseTypes.*)
				-- `nil` wherever the host cannot be asked; the mirror keeps its
				-- `can_declare` fallback whenever `diplomatic_state` is absent.
				diplomatic_state = try(function()
					local index = other:GetDiplomaticAI():GetDiplomaticStateIndex(pid);
					local row = GameInfo.DiplomaticStates[index];
					return row ~= nil and row.StateType or nil;
				end, nil),
				our_grievances_against_them = try(function()
					local balance = diplomacy:GetGrievancesAgainst(otherId);
					return balance > 0 and balance or 0;
				end, nil),
				grievances_against_us = try(function()
					local theirs = try(function()
						return other:GetDiplomacy():GetGrievancesAgainst(pid);
					end, nil);
					if type(theirs) == "number" then
						return theirs > 0 and theirs or 0;
					end
					local balance = diplomacy:GetGrievancesAgainst(otherId);
					return balance < 0 and -balance or 0;
				end, nil),
				grievance_change_per_turn = try(function()
					return Game.GetGameDiplomacy():GetGrievanceChangePerTurn(otherId, pid);
				end, nil),
				alliance_type = try(function()
					local kind = diplomacy:GetAllianceType(otherId);
					if type(kind) ~= "number" or kind < 0 then return nil; end
					local row = GameInfo.Alliances[kind];
					return row ~= nil and row.AllianceType or nil;
				end, nil),
				alliance_level = try(function()
					if diplomacy:GetAllianceType(otherId) < 0 then return nil; end
					return diplomacy:GetAllianceLevel(otherId);
				end, nil),
				alliance_turns_left = try(function()
					if diplomacy:GetAllianceType(otherId) < 0 then return nil; end
					return diplomacy:GetAllianceTurnsUntilExpiration(otherId);
				end, nil),
				our_denounce_turn = try(function()
					return diplomacy:GetDenounceTurn(otherId);
				end, nil),
				their_denounce_turn = try(function()
					return other:GetDiplomacy():GetDenounceTurn(pid);
				end, nil),
				friendship_turn = try(function()
					return diplomacy:GetDeclaredFriendshipTurn(otherId);
				end, nil),
				denounce_time_limit = try(function()
					return Game.GetGameDiplomacy():GetDenounceTimeLimit();
				end, nil),
				visibility = try(function()
					return diplomacy:GetVisibilityOn(otherId);
				end, nil),
				their_visibility_on_us = try(function()
					return other:GetDiplomacy():GetVisibilityOn(pid);
				end, nil),
				-- The grant WE make (their overview's "received" direction).
				open_borders_granted = try(function()
					return other:GetDiplomacy():HasOpenBordersFrom(pid);
				end, nil),
				delegation_at = try(function()
					return diplomacy:HasDelegationAt(otherId);
				end, nil),
				embassy_at = try(function()
					return diplomacy:HasEmbassyAt(otherId);
				end, nil),
				their_delegation = try(function()
					return other:GetDiplomacy():HasDelegationAt(pid);
				end, nil),
				their_embassy = try(function()
					return other:GetDiplomacy():HasEmbassyAt(pid);
				end, nil),
				-- Promise-type names as the shipped Alliance row lists them. The
				-- enum member is named from the REQUESTER's side ("near ME"), so
				-- `ours:IsPromiseMade(them, kind)` is a promise WE made to them and
				-- `theirs:IsPromiseMade(us, kind)` one they made to us.
				promises_made = try(function()
					local made = {};
					for _, name in ipairs({ "DONT_SETTLE_NEAR_ME", "DONT_SPY_ON_ME",
							"DONT_CONVERT_MY_CITIES", "DONT_DIG_UP_MY_ARTIFACTS" }) do
						local kind = PromiseTypes[name];
						if kind ~= nil and diplomacy:IsPromiseMade(otherId, kind) then
							made[#made + 1] = name;
						end
					end
					return made;
				end, nil),
				promises_received = try(function()
					local theirs = other:GetDiplomacy();
					local received = {};
					for _, name in ipairs({ "DONT_SETTLE_NEAR_ME", "DONT_SPY_ON_ME",
							"DONT_CONVERT_MY_CITIES", "DONT_DIG_UP_MY_ARTIFACTS" }) do
						local kind = PromiseTypes[name];
						if kind ~= nil and theirs:IsPromiseMade(pid, kind) then
							received[#received + 1] = name;
						end
					end
					return received;
				end, nil),
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
				-- ★★★★ THE RIVAL'S TREE BY NAME, NOT ONLY BY COUNT. The counts say
				-- how far ahead a rival is; the names say WHAT it holds, which is
				-- what the board needs to derive its era (`player_era` reads the
				-- seat's own tech and civic sets), the units it can field, and
				-- whether its border is enforced at all (Early Empire —
				-- `enforces_borders` above exports that one civic as one bit; with
				-- the tree on the seat the board derives it natively and keeps the
				-- bit as the override). Same `HasTech`/`HasCivic` loops as the
				-- counts, collected instead of summed: ~70 + ~50 names of ~20
				-- bytes per met rival, a few kilobytes per export. nil (absent)
				-- when the host cannot be asked; an empty list is a real "nothing
				-- yet", which serialises as `[]` and reads as an empty list.
				tech_names = try(function()
					local t = other:GetTechs();
					if t == nil then return nil; end
					local names = {};
					for row in GameInfo.Technologies() do
						if t:HasTech(row.Index) then names[#names + 1] = row.TechnologyType; end
					end
					return names;
				end, nil),
				civic_names = try(function()
					local c = other:GetCulture();
					if c == nil then return nil; end
					local names = {};
					for row in GameInfo.Civics() do
						if c:HasCivic(row.Index) then names[#names + 1] = row.CivicType; end
					end
					return names;
				end, nil),
				-- The shipped World Rankings overview's own lane numbers for this
				-- rival (`g_victoryData`, WorldRankings.lua:27,44,55): the science
				-- lane's `GetNumTechsResearched`, the domination lane's
				-- `GetMilitaryStrengthWithoutTreasury` (the army alone; `military`
				-- above is the ribbon figure with the treasury folded in) and the
				-- religion lane's `GetNumCitiesFollowingReligion` — the
				-- religious-victory progress the board could only guess from the
				-- rival cities it happened to have seen. nil when unreadable.
				techs_researched = try(function()
					return other:GetStats():GetNumTechsResearched();
				end, nil),
				military_no_treasury = try(function()
					return other:GetStats():GetMilitaryStrengthWithoutTreasury();
				end, nil),
				cities_following_religion = try(function()
					return other:GetStats():GetNumCitiesFollowingReligion();
				end, nil),
				-- The religion a majority of this rival's cities follow — the
				-- exact test the shipped religion tab runs to mark a civilization
				-- converted (WorldRankings.lua:2049, `GetReligionInMajorityOfCities`
				-- against each founder's `GetReligionTypeCreated`). nil when no
				-- religion holds a majority or the host cannot be asked.
				religion = try(function()
					local index = other:GetReligion():GetReligionInMajorityOfCities();
					if type(index) ~= "number" or index < 0 then return nil; end
					local row = GameInfo.Religions[index];
					return row ~= nil and row.ReligionType or nil;
				end, nil),
				-- Tourists visiting US from this rival: the culture tab's
				-- "Visiting us" column (WorldRankings.lua:1766, `GetTouristsFrom(
				-- playerID)` on the LOCAL player's culture). The top-level
				-- `foreign_tourists` is the sum of these over every rival; this
				-- is the per-rival term, the one the culture race is scored on.
				tourists_visiting_us = try(function()
					return Players[pid]:GetCulture():GetTouristsFrom(otherId);
				end, nil),
				-- The rival's Era Score, from the same `GetPlayerCurrentScore` the
				-- top-level `era_score` reads for us: the age flags above say
				-- which age it is in, this says how close the next one is.
				era_score = try(function()
					return Game.GetEras():GetPlayerCurrentScore(otherId);
				end, nil),
				-- The rival's outgoing trade routes, by endpoint. The same
				-- `city:GetTrade():GetOutgoingRoutes()` walk `incomingRoutes` makes
				-- for routes INTO our cities (TradeOverview.lua:60-78 reads the
				-- same rows), kept only when BOTH ends stand on ground revealed
				-- to us — the reading a player at the keyboard has, since a
				-- Trader's path is drawn on the map it walks and a route between
				-- two cities never seen is not. The mirror seats each as a route
				-- the rival's seat owns (`restore_rival_outgoing_routes`), so the
				-- rival's own route income stops being a guess from a bare city.
				trade_routes = try(function()
					local routes = {};
					for _, city in other:GetCities():Members() do
						local cx, cy = city:GetX(), city:GetY();
						if plotRevealed(pid, cx, cy) then
							for _, route in ipairs(try(function()
								return city:GetTrade():GetOutgoingRoutes();
							end, {}) or {}) do
								local destinationPlayer = try(function()
									return route.DestinationCityPlayer;
								end, -1);
								local destination = try(function()
									return Players[destinationPlayer]:GetCities():FindID(route.DestinationCityID);
								end);
								if destination ~= nil then
									local dx, dy = destination:GetX(), destination:GetY();
									if plotRevealed(pid, dx, dy) then
										routes[#routes + 1] = {
											origin_x = cx,
											origin_y = cy,
											destination_x = dx,
											destination_y = dy,
											destination_player = destinationPlayer,
										};
									end
								end
							end
						end
					end
					return routes;
				end, nil),
				-- The same resource catalogue the shipped diplomacy deal screen
				-- shows on the rival's side, narrowed to luxury TYPES the seat
				-- actually lacks. This is deliberately not a hidden map location
				-- or total stock: a met player can inspect this offer column, while
				-- the mirror only needs to avoid asking a rich rival whose available
				-- items contain no useful luxury.
				-- `Some([])` is a real no-stock answer; the outer `try` leaves the
				-- field nil when this build cannot query a working deal.
				tradeable_luxuries = try(function()
					return CivvisTradeableLuxuries(player, pid, otherId);
				end, nil),
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
				-- The exact post-launch race, including the actual active effect of
				-- Lagrange and Terrestrial Laser Stations. World Rankings' expansion
				-- replacement reads points and points-per-turn from EACH player
				-- (`WorldRankings_Expansion2.lua:587-588`), while its game-speed
				-- target comes from the local player's stats (`:589`). Keep the
				-- repeatable stations out of `science_projects`: a count cannot say
				-- whether a terrestrial station is powered this turn.
				science_victory_points = try(function()
					local stats = other:GetStats();
					if stats == nil then return -1; end
					return stats:GetScienceVictoryPoints();
				end, -1) or -1,
				science_victory_points_per_turn = try(function()
					local stats = other:GetStats();
					if stats == nil then return -1; end
					return stats:GetScienceVictoryPointsPerTurn();
				end, -1) or -1,
				science_victory_points_needed = try(function()
					local stats = player:GetStats();
					if stats == nil then return -1; end
					return stats:GetScienceVictoryPointsTotalNeeded();
				end, -1) or -1,
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
								-- The same three enemy combat facts `hostiles[]` carries, for a city-state's unit;
								-- see the note in `addUnitsOf` below for the shipped reads and
								-- for why an unreadable value stays nil / -1.
								attacks_remaining = try(function() return unit:GetAttacksRemaining(); end, nil),
								formation = CivvisMilitaryFormation(unit),
								embarked = try(function() return unit:IsEmbarked(); end, nil),
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
				-- ★★★ WHO ELSE HOLDS ENVOYS HERE, AND HOW MANY. `envoys` and
				-- `most_envoys` said ours and the leader's; the board seeded a
				-- rival's delegation as the minimum that elects the Suzerain the
				-- host names, so it could never tell one envoy from five, nor
				-- see that it stood one short of taking a city-state. The
				-- shipped panel reads every alive major's count off the same
				-- object (Base/Assets/UI/PartialScreens/CityStates.lua:1458
				-- `GetTokensReceived(iInfluencePlayer)`). Every major, zeros
				-- included, so a lapsed delegation clears; a list rather than a
				-- map because `encode` writes an empty table as `[]`.
				envoys_by_player = influence ~= nil and try(function()
					local out = {};
					for _, otherId in ipairs(PlayerManager.GetAliveMajorIDs()) do
						out[#out + 1] = {
							player = otherId,
							envoys = try(function() return influence:GetTokensReceived(otherId); end, 0) or 0,
						};
					end
					return out;
				end, nil) or nil,
				-- ★★★ WHAT THE CITY-STATE IS ASKING US FOR. The board rolled its
				-- own quest for every pair from its own hash and paid itself the
				-- Envoy when its model said so; the host's actual request never
				-- crossed, so the `quest-*` genes priced a request nobody made.
				-- CityStates.lua:257-266 is the accessor set:
				-- `HasActiveQuestFromPlayer` over `GameInfo.Quests()`, then
				-- `GetActiveQuestName` / `GetActiveQuestDescription` (both
				-- already localized text; the panel prints them as they are).
				-- The host names the quest's TARGET only inside that
				-- description (`LOC_QUEST_TRAIN_UNIT_TYPE_INSTANCE_DESCRIPTION`
				-- is "Train {1_UnitName} military unit"), so the target type is
				-- the row of the quest's own family whose localized name occurs
				-- in it, longest match first (Warrior Monk over Warrior). nil
				-- when the manager is absent; an empty list is a city-state
				-- asking nothing.
				quests = try(function()
					local manager = Game.GetQuestsManager();
					if manager == nil then return nil; end
					local out = {};
					for questInfo in GameInfo.Quests() do
						if manager:HasActiveQuestFromPlayer(pid, mid, questInfo.Index) then
							local description = try(function()
								return tostring(manager:GetActiveQuestDescription(pid, mid, questInfo.Index));
							end, "") or "";
							local family = ({
								QUEST_TRAIN_UNIT_TYPE = { "Units", "UnitType" },
								QUEST_ZONE_DISTRICT_TYPE = { "Districts", "DistrictType" },
								QUEST_TRIGGER_TECH_BOOST = { "Technologies", "TechnologyType" },
								QUEST_TRIGGER_CIVIC_BOOST = { "Civics", "CivicType" },
								QUEST_RECRUIT_GREAT_PERSON_CLASS = { "GreatPersonClasses", "GreatPersonClassType" },
							})[questInfo.QuestType];
							local target, best = nil, 0;
							if family ~= nil and description ~= "" then
								for row in GameInfo[family[1]]() do
									local name = try(function() return Locale.Lookup(row.Name); end, nil);
									if type(name) == "string" and #name > best
										and description:find(name, 1, true) then
										target, best = row[family[2]], #name;
									end
								end
							end
							out[#out + 1] = {
								type = questInfo.QuestType,
								target = target,
								name = try(function()
									return tostring(manager:GetActiveQuestName(pid, mid, questInfo.Index));
								end, nil),
							};
						end
					end
					return out;
				end, nil),
				suzerain = influence ~= nil and try(function()
					return influence:GetSuzerain();
				end, -1) or -1,
				-- Whether the city-state's border exists: Early Empire, the same
				-- civic as a major's. A city-state's land is shut to everyone but
				-- its Suzerain once it holds it — run 184456Z sent 122 military
				-- steps into non-suzerain city-state land and 4% arrived, 51%
				-- where we were Suzerain. nil when the host cannot be asked; the
				-- mirror reads nil as enforced.
				enforces_borders = try(function()
					local civic = GameInfo.Civics["CIVIC_EARLY_EMPIRE"];
					if civic == nil then return nil; end
					return minor:GetCulture():HasCivic(civic.Index);
				end, nil),
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
	-- World Rankings' science lane calls its position and target accessors at
	-- `WorldRankings_Expansion2.lua:373-374`, and the per-player rate at :588.
	-- They are the host's real progress, target, and present rate, so laser
	-- stations never have to be guessed from their repeatable completion counts.
	local scienceVictoryPoints = try(function()
		if playerStats == nil then return -1; end
		return playerStats:GetScienceVictoryPoints();
	end, -1) or -1;
	local scienceVictoryPointsPerTurn = try(function()
		if playerStats == nil then return -1; end
		return playerStats:GetScienceVictoryPointsPerTurn();
	end, -1) or -1;
	local scienceVictoryPointsNeeded = try(function()
		if playerStats == nil then return -1; end
		return playerStats:GetScienceVictoryPointsTotalNeeded();
	end, -1) or -1;

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
	-- ★★★★ ONE RECORD FOR BOTH PLAYERS, BEHIND ONE GATE. Free Cities units used
	-- to cross with id/x/y/player/type only, behind `plotRevealed`, while a
	-- barbarian carried hp, moves, xp, promotions, charges, strength and fortify
	-- state behind `PlayersVisibility[pid]:IsVisible` — and the mirror then
	-- planted every entry as a full-health barbarian (docs/FIDELITY.md, "The
	-- one-to-one map", item 3). The builder below serves both walks; `free`
	-- marks a Free Cities unit so the mirror can seat it without knowing
	-- Firaxis's player index for that aggregate (nil, not false, elsewhere: an
	-- absent key costs no bytes and reads as a barbarian on every mirror).
	local function addUnitsOf(bid, isFree)
		local other = Players[bid];
		if other == nil then return; end
		pcall(function()
			for _, unit in other:GetUnits():Members() do
				local ux, uy = unit:GetX(), unit:GetY();
				if PlayersVisibility[pid]:IsVisible(ux, uy) then
					local name = try(function()
						return GameInfo.Units[unit:GetUnitType()].UnitType;
					end, "?");
					local row = GameInfo.Units[name];
					local progress = unitProgress(unit);
					hostiles[#hostiles + 1] = {
						-- The host's own unit id, so a combat event and a
						-- next-frame sighting name the same unit. See CivvisLedger.
						id = try(function() return unit:GetID(); end, nil),
						x = ux, y = uy, player = bid,
						type = name,
						free = isFree or nil,
						hp = 100 - (try(function() return unit:GetDamage(); end, 0) or 0),
						moves = try(function() return unit:GetMovesRemaining(); end, -1),
						-- Foreign units use the same fresh-turn movement fact as
						-- our own units.  The mirror's threat flood must not infer
						-- a barbarian's allowance from an ordinary stock unit when
						-- the host has already answered it.
						max_moves = try(function() return unit:GetMaxMoves(); end, nil),
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
						-- ★★★ THE THREE FACTS THE PLANNER WAS BLIND TO ON AN ENEMY.
						-- Same accessors, same guards as the seat's own units above:
						-- `GetAttacksRemaining` (the shipped SelectedUnit.lua:62 read),
						-- the merge tier through `CivvisMilitaryFormation`
						-- (`GetMilitaryFormation`, UnitPanel.lua:2259) and `IsEmbarked`
						-- (UnitFlagManager.lua:603). Without them an enemy Corps/Army
						-- was priced on the board as a plain unit, an enemy that had
						-- struck this turn as one that could still strike, and its
						-- embarkation was read off its tile. nil / -1 is "could not
						-- read", never a guess: the mirror keeps its own rule then.
						-- The rival and city-state unit walks above emit the same three.
						attacks_remaining = try(function() return unit:GetAttacksRemaining(); end, nil),
						formation = CivvisMilitaryFormation(unit),
						embarked = try(function() return unit:IsEmbarked(); end, nil),
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
			if free == true then addUnitsOf(oid, true); end
		end
	end);
	pcall(function()
		local ids = try(function() return PlayerManager.GetAliveBarbarianIDs(); end, {}) or {};
		for _, bid in ipairs(ids) do addUnitsOf(bid, false); end
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
	-- ★ SAY SO WHEN A RELIGIOUS CHOICE DID NOT TAKE. The request reports
	-- `applied` because nothing threw; only the turn AFTER can read whether the
	-- player's own religion carries the selected belief or exists at all.
	if pendingReligionChoice ~= nil then
		local now = try(function() return Game.GetCurrentGameTurn(); end, 0) or 0;
		if pendingReligionChoice.mode == "evangelize" then
			local enhanced = false;
			for _, religion in ipairs(try(function()
				return Game.GetReligion():GetReligions();
			end, {}) or {}) do
				if religion.Founder == pid then
					for _, beliefIndex in ipairs(religion.Beliefs or {}) do
						if beliefIndex == pendingReligionChoice.belief_index then
							enhanced = true;
							break;
						end
					end
				end
				if enhanced then break; end
			end
			if enhanced then
				emit("religion_enhanced", {
					player = pid,
					turn = now,
					requested_turn = pendingReligionChoice.turn,
					unit = pendingReligionChoice.unit,
					belief = pendingReligionChoice.belief,
				});
				pendingReligionChoice = nil;
			elseif now > pendingReligionChoice.turn then
				emit("religion_enhancement_failed", {
					player = pid,
					turn = now,
					requested_turn = pendingReligionChoice.turn,
					unit = pendingReligionChoice.unit,
					belief = pendingReligionChoice.belief,
					belief_taken = try(function()
						return Game.GetReligion():IsInSomeReligion(
							pendingReligionChoice.belief_index);
					end, false),
				});
				pendingReligionChoice = nil;
			end
		elseif religionCreated >= 0 then
			emit("religion_founded", {
				player = pid,
				turn = now,
				requested_turn = pendingReligionChoice.turn,
				religion = pendingReligionChoice.religion,
				follower = pendingReligionChoice.follower,
				founder = pendingReligionChoice.founder,
			});
			pendingReligionChoice = nil;
		elseif now > pendingReligionChoice.turn then
			emit("religion_founding_failed", {
				player = pid,
				turn = now,
				requested_turn = pendingReligionChoice.turn,
				religion = pendingReligionChoice.religion,
				-- The two facts that separate the failure modes: whether the
				-- Prophet survived, and whether the slot is still open.
				founding_unit_left = prophet_pending,
				religions_founded = #(try(function()
					return Game.GetReligion():GetReligions(); end, {}) or {}),
			});
			pendingReligionChoice = nil;
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
	local policies, policy_slots, policy_slot_details = {}, 0, {};
	local pcult = try(function() return player:GetCulture(); end);
	if pcult ~= nil then
		policy_slots = try(function() return pcult:GetNumPolicySlots(); end, 0) or 0;
		for i = 0, policy_slots - 1 do
			local index = try(function() return pcult:GetSlotPolicy(i); end, -1);
			local slot_type = try(function()
				local slot_id = pcult:GetSlotType(i);
				local slot = GameInfo.GovernmentSlots[slot_id];
				return slot ~= nil and slot.GovernmentSlotType or nil;
			end, nil);
			local policy_type = nil;
			if index ~= nil and index >= 0 then
				local row = GameInfo.Policies[index];
				if row ~= nil then
					policy_type = row.PolicyType;
					policies[#policies + 1] = policy_type;
				end
			end
			policy_slot_details[#policy_slot_details + 1] = {
				slot = i, slot_type = slot_type, policy = policy_type,
			};
		end
	end
	-- A same-tick read is not a verdict: Firaxis applies policy changes after
	-- this Lua context returns.  The next opening export is the first reliable
	-- observation, and it carries both the requested deck and the actual slot
	-- contents so a partial host transaction cannot remain anonymous.
	local pending_policy = CivvisPolicy.pending;
	if pending_policy ~= nil and turn > (pending_policy.turn or turn) then
		emit("policy_deck_readback", {
			turn = turn, requested_turn = pending_policy.turn,
			mode = pending_policy.mode, desired = pending_policy.desired,
			actual = policies, slots = policy_slot_details,
		});
		CivvisPolicy.pending = nil;
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
		-- The authoritative World Rankings race. The rate includes active
		-- Lagrange and Terrestrial Laser Station effects this turn.
		science_victory_points = scienceVictoryPoints,
		science_victory_points_per_turn = scienceVictoryPointsPerTurn,
		science_victory_points_needed = scienceVictoryPointsNeeded,
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
		-- The bill by source, the TopPanel's own breakdown
		-- (ToolTipHelper_PlayerYields.lua:26-30): `GetUnitMaintenance`,
		-- `GetBuildingMaintenance` and `GetDistrictMaintenance` on the
		-- treasury. The board simulates its own bill from its rules for every
		-- plan it prices; these let the mirrored seat's forecast carry the
		-- host's components instead. Same nil-not-0 rule as `gold_per_turn`.
		unit_maintenance_total = try(function()
			return player:GetTreasury():GetUnitMaintenance();
		end, nil),
		building_maintenance_total = try(function()
			return player:GetTreasury():GetBuildingMaintenance();
		end, nil),
		district_maintenance_total = try(function()
			return player:GetTreasury():GetDistrictMaintenance();
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
		-- Our own tourism per turn, the accessor each rival's `tourism` already
		-- uses (GetStats():GetTourism()). The board prefers it to its model and
		-- the divergence instrument scores the model against it; until it
		-- crossed the `tourism` row read "no pairs" on every run.
		tourism_per_turn = try(function()
			return player:GetStats():GetTourism();
		end, nil),
		-- The religion lane's own number for us, the same accessor as each
		-- rival's `cities_following_religion` (WorldRankings.lua:44): cities
		-- anywhere following OUR religion, including ones the seat has never
		-- seen. nil when the host cannot be asked.
		cities_following_religion = try(function()
			return player:GetStats():GetNumCitiesFollowingReligion();
		end, nil),
		-- Ours, on the same scale as each rival's, so a comparison is possible at all.
		military = try(function() return player:GetStats():GetMilitaryStrength(); end, -1),
		-- ★★★★★ THE AGE, WHICH THE BRIDGE HAS NEVER CARRIED.
		--
		-- CIVVIS models Gathering Storm's age system in full (`docs/AGES.md`):
		-- `Player::era_score`, `era_score_baseline`, `normal_age_threshold`,
		-- `golden_age_threshold`, `dedications`, `dedication_choices`. None of it crossed, so on every
		-- reconstructed live board era score was 0 against the *defaults* left by
		-- `Player::default` -- a civilization permanently reading as headed for a
		-- Dark Age it might not be in, or out of one it is.
		--
		-- Two decisions run off exactly these fields and were therefore taken
		-- against fiction in every live game:
		--   * `ai::choose_dedications` picks a Dedication from
		--     `available_dedications`, which is gated on `dedication_choices`;
		--     carry the native allowance so an era-boundary board can choose one.
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
		-- A real zero says the current era has no pending dedication; -1 is the
		-- distinct "this host could not answer" value. Without this count the
		-- mirror leaves `Player::dedication_choices` at its default zero, so the
		-- CIVVIS dedication order is never emitted and an owned prompt stands.
		dedication_choices = try(function()
			return Game.GetEras():GetPlayerNumAllowedCommemorations(pid);
		end, -1),
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
		-- ★★★★ THE CLIMATE, WHICH THE BOARD HAS NEVER CARRIED. `cl` (the
		-- coastal-lowland band) has crossed per plot since the yield export
		-- with no phase to read it against: the mirror's `climate_phase` sat
		-- at 0 on every live turn, so the Flood Barrier price, the clean-power
		-- premium and the flooding of the very bands `cl` was exported for
		-- all ran under a pre-industrial sky. Every accessor is the one the
		-- shipped ClimateScreen calls (DLC/Expansion2/UI/Additions/
		-- ClimateScreen.lua:128 `GetTotalCO2Footprint`, :399
		-- `GetPlayerCO2Footprint(id, false)`, :410 `GetTemperatureChange`,
		-- :425/:428/:440 the storm, flood and drought percent chances, :445
		-- `GetTilesFlooded`, :447 `GetNextSeaLevelRiseTurns`, :1046
		-- `GetClimateChangeLevel`). Each field is guarded on its own, so a
		-- ruleset missing one accessor still sends the rest; the whole object
		-- is nil where `GameClimate` is absent or answers nothing (⚠ never an
		-- empty table: `encode` writes that as `[]`, which is not an object).
		climate = try(function()
			if GameClimate == nil then return nil; end
			local out = {
				level = try(function() return GameClimate.GetClimateChangeLevel(); end, nil),
				temperature = try(function() return GameClimate.GetTemperatureChange(); end, nil),
				co2_total = try(function() return GameClimate.GetTotalCO2Footprint(); end, nil),
				co2_ours = try(function() return GameClimate.GetPlayerCO2Footprint(pid, false); end, nil),
				sea_level_turns = try(function() return GameClimate.GetNextSeaLevelRiseTurns(); end, nil),
				tiles_flooded = try(function() return GameClimate.GetTilesFlooded(); end, nil),
				storm_pct = try(function() return GameClimate.GetStormPercentChance(); end, nil),
				flood_pct = try(function() return GameClimate.GetFloodPercentChance(); end, nil),
				drought_pct = try(function() return GameClimate.GetDroughtPercentChance(); end, nil),
			};
			for _ in pairs(out) do return out; end
			return nil;
		end, nil),
		-- ★★★★ WHERE A TRADER COULD GO AND WHAT EACH ROUTE WOULD PAY, priced
		-- by the host. `trade_routes[].yields` has carried the host's figure
		-- for ACTIVE routes; the choice of the NEXT route was priced by the
		-- model alone, which cannot see a destination's districts in fog
		-- (Ostia -> Stockholm read "+1 Science", run civvis-20260816T233226Z
		-- t177+). The shipped chooser's own legality and yield calls
		-- (Base/Assets/UI/Choosers/TradeRouteChooser.lua:227 `CanStartRoute`,
		-- :864 `CalculateOriginYieldFromPotentialRoute`) over every city of
		-- every player from each of our cities, only while a route slot is
		-- open (`GetOutgoingRouteCapacity` above the active count, the gate
		-- under which a Trader can start one at all), the 12 richest per
		-- origin so the record stays bounded. Yields are summed the way the
		-- active-route export sums them (route + path + modifiers under the
		-- international multiplier). nil when nothing can be started.
		route_options = try(function()
			local trade = player:GetTrade();
			if trade == nil then return nil; end
			local capacity = trade:GetOutgoingRouteCapacity() or 0;
			if capacity <= #tradeRoutes then return nil; end
			local manager = Game.GetTradeManager();
			if manager == nil then return nil; end
			local names = { food = "FOOD", production = "PRODUCTION", gold = "GOLD", science = "SCIENCE", culture = "CULTURE", faith = "FAITH" };
			local out = {};
			for _, origin in player:GetCities():Members() do
				local originID = origin:GetID();
				local options = {};
				for _, other in ipairs(Game.GetPlayers() or {}) do
					local otherId = try(function() return other:GetID(); end, -1);
					pcall(function()
						for _, city in other:GetCities():Members() do
							local cityID = city:GetID();
							if not (otherId == pid and cityID == originID)
								and manager:CanStartRoute(pid, originID, otherId, cityID) then
								local fromRoute = manager:CalculateOriginYieldsFromPotentialRoute(pid, originID, otherId, cityID);
								if type(fromRoute) == "table" then
									local fromPath = manager:CalculateOriginYieldsFromPath(pid, originID, otherId, cityID);
									local fromModifiers = manager:CalculateOriginYieldsFromModifiers(pid, originID, otherId, cityID);
									local yields, total = {}, 0;
									for key, tag in pairs(names) do
										local index = YieldTypes[tag];
										if index ~= nil then
											local sum = (fromRoute[index + 1] or 0)
												+ ((type(fromPath) == "table" and fromPath[index + 1]) or 0)
												+ ((type(fromModifiers) == "table" and fromModifiers[index + 1]) or 0);
											local mult = otherId ~= pid and trade:GetInternationalYieldModifier(index) or 1;
											if type(mult) ~= "number" or mult <= 0 then mult = 1; end
											yields[key] = sum * mult;
											total = total + yields[key];
										end
									end
									options[#options + 1] = {
										origin = originID,
										origin_x = origin:GetX(), origin_y = origin:GetY(),
										dest = cityID, dest_player = otherId,
										dest_x = city:GetX(), dest_y = city:GetY(),
										yields = yields, total = total,
									};
								end
							end
						end
					end);
				end
				table.sort(options, function(a, b) return a.total > b.total; end);
				for i = 1, math.min(12, #options) do
					options[i].total = nil;
					out[#out + 1] = options[i];
				end
			end
			if #out == 0 then return nil; end
			return out;
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
								required_missing_building = info.ActionRequiresMissingBuildingType,
								required_great_work = info.ActionRequiresCityGreatWorkObjectType,
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

-- ★★★★ THE MAP CROSSED EVERY 25 TURNS, AND A SCOUT LEARNS SOMETHING EVERY
-- STEP. Between two sweeps, everything a unit uncovered — coast, a rival's
-- border, a barbarian camp, the pass through the hills — was known to the
-- host and unknown to the brain, which kept planning on the board as it was
-- at the last sweep. `CivvisTiles.known` remembers which plots have crossed
-- (and under which owner); `sweep` sends only what is new or changed hands
-- since, as a `tiles` chunk stamped `delta = true`, every turn and on every
-- mid-turn frame. The Rust side merges chunks cumulatively already; the
-- stamp only keeps a delta from being mistaken for a fresh sweep. The full
-- sweep keeps its cadence (resources, improvements and pillage refresh there)
-- and re-primes `known`. `TileDelta = false` withholds the deltas.
-- One bare global table (200-local ceiling).
CivvisTiles = { known = {} };

local function exportTiles(player, pid, turn, frame, deltaOnly)
	if cfg.ExportState ~= true then return; end
	local every = cfg.TileExportEvery or 25;
	-- ⚠ TURN 1 MUST EXPORT, whatever the cadence. `turn % 25` is false for turns
	-- 1..24, so CIVVIS spent the whole opening with NO MAP: `civvis-orders` on run
	-- smoke-20260730T105241Z answered "no revealed terrain yet" every turn to turn 9
	-- and would have to turn 24. The opening is where settling and the first army
	-- are decided, so that is precisely the window that cannot be handed to a
	-- fallback. Export on the first turn, then on the cadence; between sweeps
	-- (and on a frame) send the delta — see `CivvisTiles`.
	local full = not deltaOnly and (turn <= 1 or turn % every == 0);
	if not full and cfg.TileDelta == false then return 0; end
	local width = try(function() return Map.GetGridSize(); end, 0) or 0;
	local height = 0;
	pcall(function() width, height = Map.GetGridSize(); end);
	if width <= 0 or height <= 0 then return; end

	local chunk, chunks, index, fresh = {}, 0, 0, 0;
	local function flush()
		if #chunk == 0 then return; end
		chunks = chunks + 1;
		emit("tiles", {
			turn = turn, width = width, height = height,
			chunk = chunks, plots = chunk,
			-- nil (absent) on a full sweep, so an older reader sees no change.
			delta = (not full) and true or nil,
			frame = (not full) and (frame or 0) or nil,
		});
		chunk = {};
	end

	-- ★★★★★ WHAT THE HOST SAYS THIS PLOT PAYS, WHICH IS NOT DERIVABLE FROM THE
	-- NAMES ABOVE.
	--
	-- Terrain, feature, resource and improvement are all the export ever
	-- carried, and CIVVIS re-derived the plot's yields from its own catalogue.
	-- That sum is short by every term the ground holds and no row of the
	-- ruleset names — above all the permanent fertility a flood or an eruption
	-- leaves behind (`RandomEvent_Yields` in `Expansion2_RandomEvents.xml`:
	-- Food AND Production, up to 75% and 35% per plot on a Megacolossal
	-- eruption), which the host stores on the plot and exposes through no
	-- accessor but this one. Volcanic Soil itself has NO yields of its own —
	-- `Feature_YieldChanges` has not a single row for it — so a mirror that
	-- reads the feature and not the plot sees a bare Grassland where the game
	-- shows 3 Food 3 Production.
	--
	-- The state export has carried `worked[].yields` for a while, but only for
	-- the handful of plots a city is working THIS turn. Everything a Builder,
	-- a Settler or the citizen governor is choosing BETWEEN was read from the
	-- catalogue, which is exactly the set that has to be right.
	--
	-- ⚠ BUDGET. Six reads per plot, and only on the plots this sweep was
	-- already going to send. Land only: the model's own fertility skips water
	-- (`Game::fertilize_tile`) and a coast tile's catalogue row is not the one
	-- that drifts, so two thirds of a Continents map costs nothing. Absent
	-- where every yield is zero, and absent entirely under `TileYields=false`.
	local function plotYieldTuple(plot)
		if cfg.TileYields == false then return nil; end
		return try(function()
			local out = {
				plot:GetYield(YieldTypes.FOOD),
				plot:GetYield(YieldTypes.PRODUCTION),
				plot:GetYield(YieldTypes.GOLD),
				plot:GetYield(YieldTypes.SCIENCE),
				plot:GetYield(YieldTypes.CULTURE),
				plot:GetYield(YieldTypes.FAITH),
			};
			local any = false;
			for i = 1, 6 do
				-- A build whose enum lacks a member reads nil; send nothing
				-- rather than a tuple with a hole a reader would take for zero.
				if out[i] == nil then return nil; end
				if out[i] ~= 0 then any = true; end
			end
			return any and out or nil;
		end);
	end

	local known = CivvisTiles.known;
	for y = 0, height - 1 do
		for x = 0, width - 1 do
			local plot = try(function() return Map.GetPlot(x, y); end);
			if plot ~= nil then
				local revealed = plotRevealed(pid, x, y);
				-- Unrevealed ground is deliberately sent as a hole rather than
				-- as its true terrain: the mirror must not know more than the
				-- seat does, or the simulator would plan on stolen information.
				--
				-- ★★★★ WHAT IS ON THE PLOT, NOT ONLY WHO HOLDS IT.
				--
				-- The delta used to re-send a plot when it was newly revealed or
				-- when it CHANGED HANDS, and on nothing else. A volcano that
				-- buries four tiles in Volcanic Soil changes no owner, and
				-- neither does a chopped Forest, a drained Marsh or a flood —
				-- so the mirror kept the old feature, and the yields derived
				-- from it, until the next full sweep came round. The feature
				-- index is one integer read and it closes that window.
				local mark = nil;
				if revealed then
					local owner = try(function() return plot:GetOwner(); end, -1) or -1;
					local feature = try(function() return plot:GetFeatureType(); end, -1) or -1;
					-- ★★★★ AND WHAT WAS BUILT, BURNT OR PAVED ON IT. Owner and
					-- feature closed the volcano's window; a Builder finishing a
					-- Farm, a raider pillaging it, a road laid, a district placed
					-- or completed, a wonder finished and a barbarian camp planted
					-- or cleared change neither, so the board kept the old plot
					-- until the next full sweep — up to TileExportEvery-1 turns of
					-- a camp that is not there, or one that is. Every field the
					-- record carries joins the signature; no new locals, the
					-- chunk is at its ceiling.
					mark = (owner * 1024 + feature) .. ":"
						.. (try(function() return plot:GetImprovementType(); end, -1) or -1) .. ":"
						.. (try(function() return plot:IsImprovementPillaged(); end, false) and 1 or 0) .. ":"
						.. (try(function() return plot:GetRouteType(); end, -1) or -1) .. ":"
						.. (try(function() return plot:IsRoutePillaged(); end, false) and 1 or 0) .. ":"
						.. (try(function() return plot:GetDistrictType(); end, -1) or -1) .. ":"
						.. (try(function()
							local district = CityManager.GetDistrictAt(x, y);
							return district ~= nil and district:IsComplete();
						end, false) and 1 or 0) .. ":"
						.. (try(function() return plot:GetWonderType(); end, -1) or -1) .. ":"
						-- Appeal moves when a NEIGHBOUR changes and sight moves
						-- every turn; both join the signature so the record the
						-- board reads between sweeps is this turn's.
						.. (try(function() return plot:GetAppeal(); end, 0) or 0) .. ":"
						.. (try(function() return PlayersVisibility[pid]:IsVisible(x, y); end, false) and 1 or 0);
				end
				local key = y * width + x;
				local changed = mark ~= nil and (full or known[key] ~= mark);
				if changed then
					known[key] = mark;
					if not full then fresh = fresh + 1; end
					index = index + 1;
					local water = try(function() return plot:IsWater(); end, false);
					chunk[index] = {
						x = x, y = y,
						-- The host's own six yields for this plot; see
						-- `plotYieldTuple`. Land only, absent where the read
						-- fails or every yield is zero. ⚠ Named `yl`, not `y`:
						-- `y` is this plot's row coordinate two fields up, and
						-- a duplicate key in one Lua constructor silently keeps
						-- the last one written.
						yl = (not water) and plotYieldTuple(plot) or nil,
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
						w = water,
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
						-- ★★★ APPEAL, AS THE HOST COUNTS IT. The board derived appeal
						-- from its own six neighbours and could not see a wonder's
						-- +2 in fog, a Governor's promotion or a rival's district;
						-- Neighborhood housing, Seaside and Ski Resorts and
						-- National Parks are all priced on it. The shipped
						-- PlotToolTip reads `plot:GetAppeal()` (Base/Assets/UI/
						-- ToolTips/PlotToolTip.lua:641) and `IsNationalPark`
						-- (:743). Sent on every revealed plot; absent only when the
						-- read fails, so the mirror keeps its derivation there.
						ap = try(function() return plot:GetAppeal(); end, nil),
						np = try(function() return plot:IsNationalPark() and true or nil; end, nil),
						-- ★★★ IN SIGHT NOW, not merely revealed once. `IsRevealed`
						-- gates the record; the board re-derived sight from its own
						-- radii on a reconstructed map and disagreed with the host
						-- on the tiles that mattered (`Game::host_observed`). The
						-- same call the shipped combat preview makes
						-- (Base/Assets/UI/Civ6Common.lua:115). true or absent.
						vis = try(function()
							return PlayersVisibility[pid]:IsVisible(x, y) and true or nil;
						end, nil),
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
	if full then
		emit("tiles_done", { turn = turn, chunks = chunks, width = width, height = height });
		return nil;
	end
	if fresh > 0 then
		emit("tiles_delta", { turn = turn, frame = frame or 0, plots = fresh, chunks = chunks });
	end
	return fresh;
end

-- The delta alone, whatever the cadence: what this seat revealed (or saw
-- change hands) since the last board went out. Returns the plot count.
CivvisTiles.sweep = function(player, pid, turn, frame)
	return exportTiles(player, pid, turn, frame, true);
end;

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
-- wrong. So: the turn WAITS for orders, but only for `OrdersWaitPolls` (40) and
-- then `OrdersFallbackPolls` (120); past those the built-in heuristics run and
-- the turn is recorded as `fallback`.
--
-- ⚠ THAT KNOB WAS NAMED `OrdersWaitTicks` HERE AND IN `civ6_play.py`, AND NO SUCH
-- KEY EXISTS. It is read nowhere in the tree — the two mentions were both
-- comments describing each other. Setting it would have done nothing, which is
-- the sort of thing an operator finds out during an incident.
--
-- ⚠⚠ AND THE FLOOR HAS NEVER ONCE HELD. Across the fifteen runs recorded on
-- 2026-08-27/28 the string `fallback` appears in ZERO events, and every one of
-- the 100 turns of run civvis-20260828T173743Z reads `orders_source: "civvis"`.
-- Partly that is good news — the brain answers in ~0.12 s and has never needed
-- rescuing. But that run then WEDGED at turn 100 with the brain silent, and the
-- fallback did not fire, because it cannot: every poll above is driven by
-- `awaiting.ticks`, which advances only when the Game Core thread runs. The
-- wedge sample (#2698) shows that thread parked in `__psynch_cvwait` for 1433
-- of 1433 samples while Metal kept rendering. A floor that depends on the tick
-- it is meant to rescue is not a floor for the case where ticks stop.
--
-- So the promise above is true for a brain that is SLOW or answers badly, and
-- false for a game whose Game Core has parked. The wedge watchdog
-- (`tools/ops/civvis-agent-wedge-watchdog.sh`) is what actually covers the
-- second case today, at the cost of the whole attempt. Why the thread parks is
-- not yet explained and is deliberately not guessed at here.
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
	if pending == nil or pending.kind ~= "appoint" or pending.governor ~= governorID then return; end
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
CivvisTrade = { pending = {}, asked = {}, sessions = {}, unanswered = 0, disabled = false };

-- ★★★★★ THE ANSWER ONLY EVER COMES INSIDE A SESSION. Over 42 live runs this
-- lane sent 636 EQUALIZE asks and the peace arm 253 proposals, and not ONE
-- `DiplomacyIncomingDeal` arrived — the handler above never ran. The shipped
-- screens show why: a rival evaluates a working deal as a diplomacy
-- STATEMENT inside a `MAKE_DEAL` session (DiplomacyActionView.lua
-- `MakeDeal_ApplyStatement`: "The AI will send, ACCEPT, REJECT, etc. as the
-- automatic evaluation of the deal occurs" — it arrives through
-- `Events.DiplomacyStatement` with a `DealAction`, and the view relays it as
-- `DiploPopup_DealUpdated`). A `SendWorkingDeal` with no session open is
-- never evaluated. Firaxis's own peace flow builds the locked working deal
-- FIRST and then `RequestSession(..., "MAKE_DEAL")` — the session does not
-- clear it — so every arm here now builds its deal as before and then asks
-- through `CivvisTrade.ask`: the session opens, our own opening statement
-- says it is live, the question goes out, the rival's statement carries the
-- verdict, the existing closer accepts or walks away, and the session is
-- closed by us. `CivvisControlAutoClose` is told to hold its hand for
-- `DealSessionHoldSeconds` through `LuaEvents.CivvisDealSession`; a session
-- the rival never answers is closed by that ladder as before and counted,
-- and after `DealSessionStandDown` of those the lane stands down for the
-- run rather than opening a screen a fourth time. `DealSessions = false`
-- restores the direct send.
CivvisTrade.ask = function(pid, subject, action, kind, turn)
	local trade = CivvisTrade;
	if cfg.DealSessions == false or trade.disabled then
		DealManager.SendWorkingDeal(DealProposalAction[action], pid, subject);
		return "direct";
	end
	trade.sessions[subject] = { kind = kind, action = action, turn = turn, sent = false };
	pcall(function() LuaEvents.CivvisDealSession(subject, true, cfg.DealSessionHoldSeconds or 4); end);
	DiplomacyManager.RequestSession(pid, subject, "MAKE_DEAL");
	emit("deal_session", { turn = turn, target = subject, kind = kind, action = action, phase = "opening" });
	return "session";
end;

CivvisTrade.close = function(pid, subject, why, already_closed)
	local trade = CivvisTrade;
	local session = trade.sessions[subject];
	trade.sessions[subject] = nil;
	if not already_closed and session ~= nil and session.sessionID ~= nil then
		pcall(function() DiplomacyManager.CloseSession(session.sessionID); end);
	end
	pcall(function() LuaEvents.CivvisDealSession(subject, false, 0); end);
	emit("deal_session", {
		turn = try(function() return Game.GetCurrentGameTurn(); end, -1),
		target = subject, phase = "closed", why = why,
		kind = session and session.kind or nil,
	});
end;

-- `SendWorkingDeal(ACCEPTED)` is an asynchronous request.  A successful Lua
-- call is not proof that the treaty landed: on run
-- `civvis-20260903T021106Z`, the rival answered ACCEPTED and this call returned
-- successfully, but the next host frame still reported `at_war = true`.  The
-- shipped deal screen likewise sends ACCEPTED and waits for the engine's next
-- statement/frame before treating the deal as finished.
CivvisTrade.peaceAtWar = function(pid, subject)
	return try(function()
		return Players[pid]:GetDiplomacy():IsAtWarWith(subject);
	end, nil);
end;

-- Record the only authoritative peace outcome available to this context: the
-- host diplomacy flag.  `peace_response` remains the rival's answer and the
-- submission attempt; this follow-up says whether Civilization VI actually
-- ended the war.  Keeping the two records separate prevents a pcall success
-- from masquerading as an enacted treaty.
CivvisTrade.settlePeace = function(pid, subject, why, already_closed)
	local atWar = CivvisTrade.peaceAtWar(pid, subject);
	local enacted = atWar == false;
	local turn = try(function() return Game.GetCurrentGameTurn(); end, -1);
	emit("peace_result", {
		turn = turn, target = subject, accepted = true, enacted = enacted,
		at_war = atWar, reason = why,
	});
	CivvisTrade.close(pid, subject, why, already_closed);
	return enacted;
end;

-- The deal engine may apply ACCEPTED after the statement callback returns.
-- Poll only while one accepted peace is outstanding; the ordinary game-core
-- tick and turn-begin hooks call this, so the hot path pays no diplomacy query.
CivvisTrade.pollPeace = function()
	local trade = CivvisTrade;
	local pid = try(function() return Game.GetLocalPlayer(); end, -1);
	if pid == nil or pid < 0 then return; end
	local turn = try(function() return Game.GetCurrentGameTurn(); end, -1);
	local pending = {};
	for subject, session in pairs(trade.sessions) do
		if session.peace_pending ~= nil then
			pending[#pending + 1] = { subject = subject, session = session };
		end
	end
	for _, entry in ipairs(pending) do
		local subject = entry.subject;
		local session = entry.session;
		local atWar = trade.peaceAtWar(pid, subject);
		if atWar == false then
			trade.settlePeace(pid, subject, "enacted");
		elseif atWar == true and turn > (session.peace_pending.turn or turn) then
			-- Give the host the rest of the submission turn to apply the deal;
			-- if the next turn still says war, leave it to the normal five-turn
			-- retry gate rather than holding the diplomacy session indefinitely.
			trade.settlePeace(pid, subject, "still_at_war");
		end
	end
end;

-- A session the rival never answered: the ask is dead, and a third such
-- session in a run stands the lane down. Called from the closed-session
-- event and from the arms' own response-window expiry.
CivvisTrade.abandon = function(subject, why)
	local trade = CivvisTrade;
	local session = trade.sessions[subject];
	if session == nil then return false; end
	trade.sessions[subject] = nil;
	trade.pending[subject] = nil;
	trade.unanswered = trade.unanswered + 1;
	pcall(function() LuaEvents.CivvisDealSession(subject, false, 0); end);
	local turn = try(function() return Game.GetCurrentGameTurn(); end, -1);
	emit("deal_session", { turn = turn, target = subject, phase = "unanswered", why = why,
		kind = session.kind, unanswered = trade.unanswered });
	if not trade.disabled and trade.unanswered >= (cfg.DealSessionStandDown or 3) then
		trade.disabled = true;
		emit("deal_sessions_stood_down", { turn = turn, unanswered = trade.unanswered });
	end
	return true;
end;

-- Every diplomacy statement that touches the local seat. Ours opens the
-- question; the rival's carries the verdict. Bare global for the offline
-- regression (`deal_session_test.lua`).
CivvisOnDiplomacyStatement = function(fromPlayer, toPlayer, kVariants)
	local pid = try(function() return Game.GetLocalPlayer(); end, -1);
	if pid == nil or pid < 0 or (fromPlayer ~= pid and toPlayer ~= pid) then return; end
	local other = (fromPlayer == pid) and toPlayer or fromPlayer;
	local trade = CivvisTrade;
	local session = trade.sessions[other];
	if session == nil then return; end
	local turn = try(function() return Game.GetCurrentGameTurn(); end, -1);
	local sessionID = type(kVariants) == "table" and kVariants.SessionID or nil;
	if session.sessionID == nil and sessionID ~= nil then session.sessionID = sessionID; end
	if not session.sent then
		-- The session is live: put the question. `sent` goes first so an
		-- answer delivered from inside the send is read as the answer.
		session.sent = true;
		local ok = pcall(function()
			DealManager.SendWorkingDeal(DealProposalAction[session.action], pid, other);
		end);
		emit("deal_session", { turn = turn, target = other, kind = session.kind,
			action = session.action, session = sessionID or -1, phase = "asked", sent = ok });
		if not ok then CivvisTrade.close(pid, other, "send_threw"); end
		return;
	end
	if fromPlayer ~= other then return; end
	local dealAction = type(kVariants) == "table" and kVariants.DealAction or nil;
	trade.unanswered = 0;
	emit("deal_session", { turn = turn, target = other, kind = session.kind, phase = "answered",
		session = session.sessionID or -1, deal_action = tostring(dealAction) });
	if session.kind == "peace" then
		-- The rival's ACCEPTED is its verdict.  Match the shipped
		-- `DiplomacyDealView.OnProposeOrAcceptDeal` guard: it sends ACCEPTED
		-- only when both working deals are equal, otherwise it sends ADJUSTED
		-- and waits for the rival to reconcile the package.
		local accepted = dealAction == DealProposalAction.ACCEPTED;
		local dealEqual = nil;
		local equalityChecked = false;
		local submitted = false;
		local submittedAction = nil;
		if accepted then
			equalityChecked = pcall(function()
				dealEqual = DealManager.AreWorkingDealsEqual(pid, other);
			end) and type(dealEqual) == "boolean";
			if equalityChecked then
				submittedAction = dealEqual and "ACCEPTED" or "ADJUSTED";
				submitted = pcall(function()
					DealManager.SendWorkingDeal(DealProposalAction[submittedAction], pid, other);
				end);
			end
		end
		emit("peace_response", { turn = turn, target = other, accepted = accepted,
			deal_equal = dealEqual, equality_checked = equalityChecked,
			submitted = submitted, submitted_action = submittedAction,
			-- This is deliberately false until `pollPeace` observes the host
			-- diplomacy state; a pcall only proves that Lua accepted the request.
			enacted = false, deal_action = tostring(dealAction) });
		if accepted and equalityChecked and dealEqual and submitted then
			session.peace_pending = { turn = turn };
			pcall(function()
				LuaEvents.CivvisDealSession(other, true, cfg.DealSessionHoldSeconds or 4);
			end);
			CivvisTrade.pollPeace();
			return;
		elseif accepted and equalityChecked and not dealEqual and submitted then
			-- Keep the session alive just as the shipped deal view does after
			-- ADJUSTED; the next rival statement can be accepted after the
			-- working deals converge.
			session.peace_adjusted = true;
			emit("deal_session", { turn = turn, target = other, kind = session.kind,
				phase = "adjusted", action = submittedAction, deal_equal = false });
			pcall(function()
				LuaEvents.CivvisDealSession(other, true, cfg.DealSessionHoldSeconds or 4);
			end);
			return;
		elseif accepted and not equalityChecked then
			CivvisTrade.close(pid, other, "deal_equality_unreadable");
			return;
		end
	else
		CivvisOnIncomingDeal(other, pid, dealAction);
	end
	CivvisTrade.close(pid, other, "answered");
end;

CivvisOnDealSessionClosed = function(sessionID)
	for subject, session in pairs(CivvisTrade.sessions) do
		if session.sessionID == sessionID or (session.sessionID == nil and sessionID == nil) then
			local pid = try(function() return Game.GetLocalPlayer(); end, -1);
			if session.peace_pending ~= nil then
				CivvisTrade.settlePeace(pid, subject, "session_closed", true);
			elseif session.peace_adjusted then
				-- ADJUSTED means the rival answered, but no equal package was
				-- accepted. Closing that screen is not evidence of a treaty and
				-- must not produce a synthetic accepted peace result.
				CivvisTrade.close(pid, subject, "session_closed", true);
			else
				CivvisTrade.abandon(subject, "session_closed");
			end
			return;
		end
	end
end;

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
	-- directions flip: their side is tallied into `theirs` by the same keys
	-- and must hold exactly the one item the ask registered as
	-- `pending.want` — the Open Borders agreement, or the luxury copy —
	-- (anything else is foreign, their gold included), and our side must
	-- hold gold and nothing else, totalled into `pay`.
	local buying = pending ~= nil and pending.direction == "buy";
	local gold, gpt, foreign, mine, offered = 0, 0, 0, {}, 0;
	local theirs, payGold, payGpt = {}, 0, 0;
	local mineText, theirsText = {}, {};
	if incoming ~= nil then
		pcall(function()
			-- One key per item, the shape `pending.gave` (a sale) and
			-- `pending.want` (a purchase) are written in, so either side of
			-- an answer is matched against what was asked for by Firaxis
			-- type and value, never by position.
			local function keyOf(item, kind, amount)
				if kind == DealItemTypes.FAVOR then return "FAVOR", amount; end
				if kind == DealItemTypes.RESOURCES then
					return "RESOURCES:" .. tostring(item:GetValueType()), amount;
				end
				if kind == DealItemTypes.GREATWORK then
					-- Matched by the work INSTANCE the sale offered. A work
					-- has no amount — its presence is its quantity, and 0
					-- here would fail the match against the `1` the ask
					-- registered.
					return "GREATWORK:" .. tostring(item:GetValueType()), 1;
				end
				if kind == DealItemTypes.AGREEMENTS and DealAgreementTypes ~= nil
						and try(function() return item:GetSubType(); end, nil)
							== DealAgreementTypes.OPEN_BORDERS then
					-- An agreement has no amount either.
					return "OPEN_BORDERS", 1;
				end
				return "OTHER:" .. tostring(kind), amount;
			end
			for item in incoming:Items() do
				local kind = item:GetType();
				local from = item:GetFromPlayerID();
				local duration = item:GetDuration() or 0;
				local amount = item:GetAmount() or 0;
				if from == fromPlayer then
					if kind == DealItemTypes.GOLD then
						-- Their gold is the price of a sale and foreign to a
						-- purchase, whatever else the answer holds.
						if buying then
							foreign = foreign + 1;
						elseif duration == 0 then
							gold = gold + amount;
						else
							gpt = gpt + amount;
						end
					elseif buying then
						local key, count = keyOf(item, kind, amount);
						theirs[key] = (theirs[key] or 0) + count;
						theirsText[#theirsText + 1] = key .. "=" .. tostring(count) .. "x" .. tostring(duration);
					else
						foreign = foreign + 1;
					end
				else
					if buying and kind == DealItemTypes.GOLD then
						if duration == 0 then payGold = payGold + amount; else payGpt = payGpt + amount; end
						mineText[#mineText + 1] = "GOLD=" .. tostring(amount) .. "x" .. tostring(duration);
					else
						local key, count = keyOf(item, kind, amount);
						mine[key] = (mine[key] or 0) + count;
						mineText[#mineText + 1] = key .. "=" .. tostring(count) .. "x" .. tostring(duration);
					end
				end
			end
		end);
	end
	local matches = pending ~= nil;
	if buying then
		-- The answer must be the one item asked for — the passage, or the
		-- luxury copy the ask registered as `want` — and a price, nothing
		-- else in either direction: a counter that slips another item onto
		-- our side, swaps the copy for another, doubles it, or keeps it off
		-- theirs is walked away from.
		local want = pending.want or "OPEN_BORDERS";
		matches = theirs[want] == 1 and next(mine) == nil;
		for key, _ in pairs(theirs) do
			if key ~= want then matches = false; end
		end
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
		theirs = table.concat(theirsText, ","),
		want = pending and pending.want or nil,
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
			floor = pending.floor, ceiling = pending.ceiling, want = pending.want,
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
		floor = pending.floor, ceiling = pending.ceiling, want = pending.want,
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
CivvisLedger = {
	open = {}, damage = {}, pending = {}, kinds = {}, positions = {}, expected_gp_activation = {}
};

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
	-- The shipped UnitPanel.lua:3905-3916 names a combat type ONLY in the
	-- RANGE_ATTACK interface mode; in AIR_ATTACK mode (and every other) it
	-- hands `SimulateAttackInto` nil and lets the engine pick, so an
	-- `AIR_ATTACK` preview deliberately leaves `combatType` nil here.
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

-- ★★★★★ A MELEE ATTACK IS A MOVE_TO **WITH THE ATTACK MODIFIER**, AND
-- WITHOUT IT NOTHING EVER ATTACKS.
--
-- Measured across every control run this machine holds: 8,828 melee ATTACK
-- orders were issued and 89 combats came back — a 1.0% landing rate — while
-- RANGE_ATTACK, which needs no modifier, landed 520 of 841 (61.8%). On run
-- civvis-20260821T130446Z the seat ordered 208 melee attacks in 104 turns and
-- fought exactly ZERO of them: a barbarian Slinger (combat strength 5, our
-- preview promising 63 damage) sat on the same plot at (65,25) from t36 to
-- t40 being "attacked" every single turn and walked away untouched, and the
-- empire lost EIGHT Settlers, two Builders, two Warriors, a Slinger, a Scout
-- and an Archer to raiders it could not hit back.
--
-- ★★★ AND IT IS ALSO THE ONLY ROUTE TO THEOLOGICAL COMBAT. There is no
-- `UNITOPERATION_THEOLOGICAL_ATTACK` on this build and no `UNITCOMMAND_` for
-- one: `Base/Assets/Gameplay/Data/UnitOperations.xml` lists 57 operations and
-- none of them is a religious strike. The shipped Civilopedia says why, at
-- `Base/Assets/Text/en_US/Civilopedia_Concepts_Text.xml:636` — "Theological
-- combat works just like combat with military units, just attack one Religious
-- unit with another." An Apostle or Inquisitor therefore attacks through this
-- same branch, and `Action::TheologicalAttack` translates to the ordinary
-- `ATTACK` verb for exactly that reason. Religious units have no ranged
-- combat, so `RequestMoveOperation`'s `GetRangedCombat() > GetCombat()` test
-- below is false for them and they take the melee path unchanged.
--
-- The reason is one parameter. Firaxis's own `Civ6Common.lua:RequestMoveOperation`
-- (`Base/Assets/UI/Civ6Common.lua:137-169`, the melee branch at :152-163)
-- — the shipped path behind every melee attack a human ever makes — sets
--
--   tParameters[UnitOperationTypes.PARAM_MODIFIERS] =
--       UnitOperationMoveModifiers.ATTACK
--       + UnitOperationMoveModifiers.MOVE_IGNORE_UNEXPLORED_DESTINATION;
--   UnitManager.RequestOperation( kUnit, UnitOperationTypes.MOVE_TO, tParameters );
--
-- before requesting MOVE_TO. Without `ATTACK` the engine reads a plain move,
-- the pathfinder will not enter a plot an enemy is standing on, and the
-- request resolves to "walk next to it and stop". `CanStartOperation` still
-- answers TRUE — the unit genuinely can start moving that way — so `operate`
-- reported every one of those 8,828 orders as given. This is the same trap
-- `canOperate` was written for, one level deeper: the parameters were passed,
-- but not all of them.
--
-- ⚠ Resolved defensively and ONCE. `UnitOperationMoveModifiers` is a UI-context
-- global; if a build does not expose it, `nil` is returned and the caller sends
-- the parameter table unchanged — the historical behaviour — rather than
-- throwing on every attack in the game.
-- ⚠ Hung on `CivvisLedger`, not declared as a main-chunk local: this file sits
-- ONE slot under Lua's 199-local ceiling for the main chunk and
-- `test_main_chunk_locals_stay_under_the_limit` fails the build at 199. The
-- resolution is cached in an upvalue so the enum is read once per game.
CivvisLedger.attackModifiers = nil;
do
	local resolved = false;
	local cached = nil;
	CivvisLedger.attackModifiers = function()
		if resolved then return cached; end
		resolved = true;
		cached = try(function()
			local attack = UnitOperationMoveModifiers.ATTACK;
			if attack == nil then return nil; end
			local ignore = UnitOperationMoveModifiers.MOVE_IGNORE_UNEXPLORED_DESTINATION;
			if ignore == nil then return attack; end
			return attack + ignore;
		end, nil);
		return cached;
	end
end

-- ★★★★★ A STRIKE THE ENGINE WOULD TURN INTO A WAR IS REFUSED, NOT SENT.
--
-- Measured across the 46 King runs since 2026-08-26: 28 wars carried the
-- signature of a war WE started and never chose — 150 grievances against
-- us, none of ours, no `war` order and no journal line — in 19 games, 14 of
-- them before turn 100. Twenty-one were Suzerain Wars: a strike (or a
-- CAPTURE aimed at what the mirror took for a barbarian) landed on a
-- city-state's unit, and its suzerain joined at 100 + 50 grievances
-- (`GRIEVANCES_SUZERAIN_CITY_STATE_DOW` + `GRIEVANCES_HAVE_ENVOYS_CITY_STATE_DOW`,
-- `DLC/Expansion2/Data/Expansion2_GlobalParameters.xml`); the other seven
-- hit a major's unit directly. Run civvis-20260829T090147Z drew Nubia that
-- way at t67 and lost the game at t106; civvis-20260829T094737Z drew
-- Scotland at t59 with no leader screen open at all.
--
-- A human never does this by accident: `Base/Assets/UI/WorldInput.lua:2067`
-- asks `CombatManager.IsAttackChangeWarState(componentID, x, y)` before every
-- attack and raises "Declare Surprise War?" when the answer names a player.
-- This file requests the operation directly and never asked, so the engine
-- declared silently — and nothing logged it, because `war` is emitted only
-- on the agent's own declare path. The same question is asked here, and a
-- strike whose answer is non-empty is refused with the players it would have
-- drawn in. The agent's `war` order (`DIPLOMACY_DECLARE_WAR`) stays the only
-- route to a war, as designed: once that has been declared the check answers
-- empty and the strike goes through unchanged.
--
-- ⚠ An absent or failing API answers nil and the strike proceeds — the
-- historical behaviour — never a blanket refusal.
-- ⚠ `#results` and `rawget`, not `ipairs`: the answer is a host table and
-- the test harness stands in stubs whose `__index` invents members.
CivvisLedger.warStarters = function(actor, x, y)
	if actor == nil or x == nil or y == nil then return nil; end
	local results = try(function()
		return CombatManager.IsAttackChangeWarState(actor:GetComponentID(), x, y);
	end, nil);
	if type(results) ~= "table" then return nil; end
	local players = {};
	for i = 1, #results do
		local id = tonumber(rawget(results, i));
		if id ~= nil then players[#players + 1] = id; end
	end
	if #players == 0 then return nil; end
	return players;
end;

-- The refusal, named for the ledger — `would_declare_war:<player ids>` —
-- and a `war_refused` event carrying the actor, the verb, the plot and its
-- owner, so the decider's half of the mistake (a strike aimed at a unit the
-- mirror took for a barbarian) can be read off the next run.
CivvisLedger.refuseWarStarter = function(actor, subject, verb, x, y, turn)
	local players = CivvisLedger.warStarters(actor, x, y);
	if players == nil then return nil; end
	local names = {};
	for i = 1, #players do names[i] = tostring(players[i]); end
	emit("war_refused", {
		turn = turn, unit = subject, verb = verb, x = x, y = y, players = players,
		target_owner = try(function()
			local plot = Map.GetPlot(x, y);
			return plot and plot:GetOwner() or -1;
		end, -1),
	});
	return "would_declare_war:" .. table.concat(names, ",");
end;

-- Called from `applyOrder` before a strike is requested: emit the preview and
-- remember it, so the combat this strike produces can carry it.
CivvisLedger.strike = function(unit, subject, verb, x, y, turn)
	if CivvisFrames ~= nil then CivvisFrames.noteStrike(); end
	-- `StrikePreview = false` keeps the host's combat simulation out of the
	-- turn entirely; the strike is still recorded, without a prediction.
	local preview = nil;
	if cfg.StrikePreview ~= false then preview = CivvisLedger.preview(unit, verb, x, y); end
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

-- One of OUR units left the map — combat, disband, capture, deletion, or a
-- successful Great Person activation. Named with the kind the last export
-- knew, and with the treasury, so a bankruptcy disband and a battlefield loss
-- are one field apart. Activation consumes the physical Great Person unit;
-- keep the historical `unit_lost` witness, but mark that non-loss disposition
-- so a ledger cannot mistake a successful science/culture action for a kill.
CivvisLedger.onUnitRemoved = function(player, unitId)
	local pid = tonumber(try(function() return Game.GetLocalPlayer(); end, -1)) or -1;
	if tonumber(player) ~= pid then return; end
	local turn = tonumber(try(function() return Game.GetCurrentGameTurn(); end, -1)) or -1;
	local key = tostring(unitId);
	local activationTurn = CivvisLedger.expected_gp_activation[key];
	CivvisLedger.expected_gp_activation[key] = nil;
	CivvisLedger.positions[key] = nil;
	emit("unit_lost", {
		turn = turn, unit = tonumber(unitId), unit_kind = CivvisLedger.kinds[key],
		cause = activationTurn == turn and "great_person_activation" or nil,
		gold = tonumber(try(function()
			return math.floor(Players[pid]:GetTreasury():GetGoldBalance());
		end, nil)),
	});
end;

-- One of OUR units was TAKEN, not killed: the game's own word for it. Modelled
-- on `Base/Assets/UI/Popups/UnitCaptured.lua:8`
-- (`function OnUnitCaptured( currentUnitOwner, unit, owningPlayer, capturingPlayer )`),
-- registered there at `:49` (`Events.UnitCaptured.Add(OnUnitCaptured);`) and
-- filtered at `:11` on `localPlayer == currentUnitOwner`. `unit_lost` above
-- still fires for the same removal, and it reads exactly like a settler
-- founding a city — 24 settlers went to the barbarians in ten runs on
-- 2026-08-28 and no ledger column could tell. This line names the captor.
CivvisLedger.onUnitCaptured = function(currentUnitOwner, unitId, owningPlayer, capturingPlayer)
	local pid = tonumber(try(function() return Game.GetLocalPlayer(); end, -1)) or -1;
	if tonumber(currentUnitOwner) ~= pid then return; end
	local captor = tonumber(capturingPlayer);
	local barbarian = captor ~= nil and try(function()
		return Players[captor]:IsBarbarian() == true;
	end, false) == true;
	emit("unit_captured", {
		turn = tonumber(try(function() return Game.GetCurrentGameTurn(); end, -1)) or -1,
		unit = tonumber(unitId), unit_kind = CivvisLedger.kinds[tostring(unitId)],
		owner = tonumber(currentUnitOwner), original_owner = tonumber(owningPlayer),
		captor = captor, captor_is_barbarian = barbarian,
	});
end;

-- ★★★★★ THE REVERSE LEGALITY WITNESS. The bridge normally sends only actions
-- CIVVIS already calls legal, so it can discover "CIVVIS allowed a host refusal"
-- but never "the host allowed a CIVVIS refusal". `Events.UnitMoved` is the
-- shipped event for the latter direction: TutorialUIRoot.lua:2733 receives
-- `(player, unit, x, y, locallyVisible, stateChange)` after each movement.
--
-- Keep the previous location from the state export, then advance it after every
-- event. A first sighting is intentionally only a seed: without a host-observed
-- source coordinate it is not a legal-action comparison. The Rust audit marks
-- that absence as uncomparable instead of inventing a path.
CivvisLedger.onUnitMoved = function(player, unitId, x, y, locallyVisible, stateChange)
	local pid = tonumber(try(function() return Game.GetLocalPlayer(); end, -1)) or -1;
	if tonumber(player) ~= pid then return; end
	local id = tonumber(unitId);
	local toX, toY = tonumber(x), tonumber(y);
	if id == nil or toX == nil or toY == nil then return; end
	local key = tostring(id);
	local from = CivvisLedger.positions[key];
	CivvisLedger.positions[key] = { x = toX, y = toY };
	if from == nil or from.x == toX and from.y == toY then return; end
	emit("host_move", {
		turn = tonumber(try(function() return Game.GetCurrentGameTurn(); end, -1)) or -1,
		unit = id, unit_kind = CivvisLedger.kinds[key],
		from_x = from.x, from_y = from.y, x = toX, y = toY,
		locally_visible = locallyVisible == true,
		state_change = stateChange == true,
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

-- ★★★★★ THE RECEIVING SIDE OF THE BRIDGE. `applied = true` below means an arm's
-- request did not throw, and that has never meant the host did anything: a
-- Settler was requested on 83 consecutive turns with `applied = true` and
-- nothing built, a purchase whose `pcall` did not throw bought nothing, and a
-- `MOVE_TO (14,11)` ended at (12,9). `civvis_orders` now checks every order it
-- issued against the NEXT `state` frame and sends the verdicts back through the
-- orders channel as rows of kind `order_verified` / `order_failed` /
-- `turn_verified`. This file is the ledger's only writer, so it re-emits them
-- as events. They are not orders: `applyOrders` keeps them out of
-- `orders_seen` and `orders_applied`, and `turn_verified` lays this file's own
-- return-code count for the verified turn (`orders_reported`) beside the
-- decider's verified count (`orders_applied`) on one line of the ledger.
--
-- Hung on a global table, not file-scope locals: the main chunk sits at Lua's
-- 200-register ceiling.
CivvisVerify = { reported = {} };
CivvisVerify.isVerdict = function(kind)
	return kind == "order_verified" or kind == "order_failed" or kind == "turn_verified";
end;
-- What this file counted for a turn, kept until the decider's verdict for that
-- turn arrives with the next turn's orders.
CivvisVerify.remember = function(turn, seen, applied)
	CivvisVerify.reported[tostring(turn)] = { seen = seen, applied = applied };
end;
-- Re-emit one verdict row as a ledger event. The order row has no spare
-- column, so the verified turn rides in `x` for a per-order verdict and in
-- `subject` for the tally; the tally's counts ride in `verb` as `name=N`.
CivvisVerify.record = function(kind, subject, verb, x, turn)
	if kind == "turn_verified" then
		local counted = CivvisVerify.reported[tostring(subject)] or {};
		CivvisVerify.reported[tostring(subject)] = nil;
		local function count(name)
			return tonumber(string.match(verb, name .. "=(%d+)")) or 0;
		end
		emit("turn_verified", {
			turn = subject, checked_on = turn,
			orders_issued = count("issued"),
			orders_applied = count("verified"),
			orders_failed = count("failed"),
			orders_unverifiable = count("unverifiable"),
			orders_seen = counted.seen,
			orders_reported = counted.applied,
		});
		return true, "verdict";
	end
	local label, reason = string.match(verb, "^(%S+)%s*(.*)$");
	local orderKind, orderVerb = string.match(label or verb, "^([^:]*):?(.*)$");
	-- `kind` is the event's own name in every ledger record, so the order's
	-- kind travels as `order_kind`.
	local payload = {
		turn = x, checked_on = turn, order_kind = orderKind,
		verb = (orderVerb ~= nil and orderVerb ~= "") and orderVerb or nil,
		subject = (subject ~= nil and subject >= 0) and subject or nil,
	};
	if kind == "order_failed" then
		payload.reason = (reason ~= nil and reason ~= "") and reason or "unknown";
	end
	emit(kind, payload);
	return true, "verdict";
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
	local function submitMajorPeaceDeal(subject, asked, cap)
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
		-- that exact white deal.  A tribute rides the retry only when the planner
		-- said the front is lost: the order's `x` is the most Gold it may carry
		-- (Rust: `peace_tribute_cap`, a quarter of the treasury at most), and a
		-- white offer carries 0.  Until 2026-08-24 every retry put three quarters
		-- of the treasury on the table whatever the reason for the offer; the
		-- ledger counted 142 such tributes at a median 116 Gold, and Civilization
		-- VI prices a gift at nothing.  A rejected deal transfers nothing.
		cap = math.max(0, math.floor(tonumber(cap) or 0));
		if asked ~= nil and cap > 0 then
			local tribute = deal:AddItemOfType(DealItemTypes.GOLD, pid);
			if tribute ~= nil then
				tribute:SetDuration(0);
				local balance = try(function()
					return player:GetTreasury():GetGoldBalance();
				end, 0) or 0;
				local amount = math.min(cap, math.floor(balance * 0.75),
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

		-- `CivvisTrade.ask`: the offer goes out inside a MAKE_DEAL session, the
		-- way the shipped CHOICE_MAKE_PEACE does (locked deal first, then the
		-- session), because a session-less PROPOSED is never evaluated — 253 of
		-- them were submitted over 42 runs without one answer. With
		-- `DealSessions = false` this is the direct send it used to be.
		CivvisTrade.ask(pid, subject, "PROPOSED", "peace", turn);
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

	-- A verdict on an earlier turn's order, for the ledger. See CivvisVerify.
	if CivvisVerify.isVerdict(kind) then
		return CivvisVerify.record(kind, subject, verb, x, turn);
	end

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
			-- Firaxis accepts an assignment request before the Governor roster
			-- export reflects it.  Replan frames can therefore replay the same
			-- semantic order several times in one turn.  A second request is not
			-- a useful retry: it only produces `not_assigned` verdicts while the
			-- first request is still in flight.  Keep the request for this turn;
			-- if the next authoritative export is still unassigned, the next turn
			-- is a real retry window.
			local pending = pendingGovernorAssignments[pid];
			if kind == "governor_assign"
					and pending ~= nil
					and pending.kind == "assign"
					and pending.governor == governor.Index
					and pending.city_player == cityOwner
					and pending.city == subject
					and pending.turn == turn then
				return false, "governor_assign_pending";
			end
			local ok = requestGovernorAssignment(
				pid, governor.Index, cityOwner, subject);
			if ok and kind == "governor_assign" then
				pendingGovernorAssignments[pid] = {
					kind = "assign", governor = governor.Index,
					city_player = cityOwner, city = subject, turn = turn,
				};
			end
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
			kind = "appoint", governor = governor.Index,
			city_player = cityOwner, city = subject,
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

	-- ★★★ THE CAPTURED CITY'S DISPOSITION. `subject` is the Firaxis city id;
	-- `verb` is KEEP, RAZE or LIBERATE. Civilization VI has ONE command for all
	-- three, `CityCommandTypes.DESTROY`, told apart by a directive in
	-- `PARAM_FLAGS` — the shipped `Base/Assets/UI/Popups/RazeCity.lua` buttons:
	--
	--   :39  tParameters[UnitOperationTypes.PARAM_FLAGS] = CityDestroyDirectives.KEEP;
	--   :48  tParameters[UnitOperationTypes.PARAM_FLAGS] = CityDestroyDirectives.RAZE;
	--   :17  tParameters[UnitOperationTypes.PARAM_FLAGS] = CityDestroyDirectives.LIBERATE_FOUNDER;
	--   :18  if (CityManager.CanStartCommand( g_pSelectedCity, CityCommandTypes.DESTROY, tParameters)) then
	--   :20      CityManager.RequestCommand( g_pSelectedCity, CityCommandTypes.DESTROY, tParameters);
	--
	-- LIBERATE is the FOUNDER button (`:17`; the host shows it only when
	-- `CanLiberateCityTo(GetOriginalOwner())` and the founder is not the
	-- loser, `:90`), because the engine's `do_liberate_city` returns the city
	-- to `original_owner`; the owner-before-occupation button (`:28`) has no
	-- board action. `CanStartCommand` with the SAME table gates the request,
	-- so a directive the host will not take (RAZE on a capital or on the
	-- founder's own city, LIBERATE with nobody to hand it to) is the named
	-- refusal `cannot_<verb>`, never a silent no-op. Until this branch every
	-- one of these decisions was untranslated and the host's default — keep
	-- — took every city.
	if kind == "city" then
		local city = try(function() return CityManager.GetCity(pid, subject); end);
		if city == nil then return false, "city_missing:" .. tostring(subject); end
		local directiveName = ({
			KEEP = "KEEP", RAZE = "RAZE", LIBERATE = "LIBERATE_FOUNDER",
		})[verb];
		if directiveName == nil then return false, "unknown_city_verb_" .. verb; end
		local directive = try(function() return CityDestroyDirectives[directiveName]; end, nil);
		if directive == nil then return false, "no_destroy_directive_" .. directiveName; end
		local params = {};
		params[UnitOperationTypes.PARAM_FLAGS] = directive;
		if not try(function()
			return CityManager.CanStartCommand(city, CityCommandTypes.DESTROY, params);
		end, false) then
			return false, "cannot_" .. string.lower(verb);
		end
		local ok = pcall(function()
			CityManager.RequestCommand(city, CityCommandTypes.DESTROY, params);
		end);
		emit("city_disposition", {
			turn = turn, city = subject, verb = verb, requested = ok,
			name = try(function() return Locale.Lookup(city:GetName()); end, nil),
			conquered_from = try(function() return city:GetJustConqueredFrom(); end, nil),
			original_owner = try(function() return city:GetOriginalOwner(); end, nil),
			pop = try(function() return city:GetPopulation(); end, nil),
		});
		return ok, ok and verb or ("city_" .. string.lower(verb) .. "_throw");
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
		local warRefusal = CivvisLedger.refuseWarStarter(city, subject, "CITY_STRIKE", x, y, turn);
		if warRefusal ~= nil then return false, warRefusal; end
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
		local warRefusal = CivvisLedger.refuseWarStarter(
			encampment, subject, "ENCAMPMENT_STRIKE", x, y, turn);
		if warRefusal ~= nil then return false, warRefusal; end
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
			ran, submitted, concession, reason = pcall(submitMajorPeaceDeal, subject, asked, x);
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

	-- ★★★★ A DENOUNCEMENT IS A SESSION, LIKE A DELEGATION. The shipped view
	-- opens it with DiplomacyManager.RequestSession(local, other, "DENOUNCE")
	-- (DiplomacyActionView.lua:469); there is no PlayerOperations verb. The
	-- host's own record of it (GetDenounceTurn) crosses on every rival, which
	-- is how the verdict reads the next frame, and how a second ask inside
	-- the shipped time limit is refused here before it opens a leader scene.
	if kind == "denounce" then
		local diplomacy = try(function() return player:GetDiplomacy(); end);
		if diplomacy == nil then return false, "no_diplomacy"; end
		if subject < 0 then return false, "denounce_target_unmapped"; end
		if try(function() return diplomacy:IsAtWarWith(subject); end, false) then
			return false, "denounce_at_war";
		end
		local since = try(function() return diplomacy:GetDenounceTurn(subject); end, -1) or -1;
		local limit = try(function() return Game.GetGameDiplomacy():GetDenounceTimeLimit(); end, 0) or 0;
		if since >= 0 and (turn - since) < limit then
			return false, "denounce_already";
		end
		local key = "DENOUNCE" .. subject;
		local asked = peaceAsked[key];
		if asked ~= nil and (turn - asked) < (cfg.PeaceRetryTurns or 5) then
			return false, "denounce_cooldown";
		end
		local ok = pcall(function()
			DiplomacyManager.RequestSession(pid, subject, "DENOUNCE");
		end);
		if ok then peaceAsked[key] = turn; end
		return ok, ok and "denounce_asked" or "throw";
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
			CivvisTrade.abandon(subject, "expired");
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
					-- ★ NEVER SELL THE LAST COPY OF A LUXURY. Each distinct luxury
					-- gives an Amenity to four cities; a second copy gives nothing
					-- but its trade value, so the second is surplus and the first
					-- is not. The planner's count includes a suzerain's copy, so
					-- run civvis-20260829T040540Z sold its only Mercury three
					-- times (t102, t126, t150) and dropped four Amenities each
					-- time. Ask the host how many we actually hold; a stub that
					-- answers nothing is not a number and leaves the sale alone.
					if forType ~= nil then
						local luxury = row.ResourceClassType == "RESOURCECLASS_LUXURY";
						local owned = try(function()
							return player:GetResources():GetResourceAmount(forType);
						end, nil);
						if luxury and type(owned) == "number" and owned <= want.amount then
							return false, "sole_copy:" .. want.name, {}, "";
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
			CivvisTrade.ask(pid, subject, "EQUALIZE", "sell", turn);
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
	-- `verb` names the agreement — OPEN_BORDERS — and `x` is the
	-- gold-equivalent ceiling (lump plus 25× per-turn) ABOVE which the answer
	-- is declined. Built the way the shipped screen adds an agreement
	-- (DiplomacyDealView.lua `OnClickAvailableAgreement`): one AGREEMENTS
	-- item FROM the rival, subtype OPEN_BORDERS, the standard thirty turns;
	-- then EQUALIZE, and `CivvisOnIncomingDeal` closes only when the rival's
	-- own balance asks gold at or under the ceiling. Same cooldowns and same
	-- one-working-deal-per-rival rule as the sale lane —
	-- `CivvisTrade.pending`/`asked` are shared deliberately, because the host
	-- holds ONE outgoing working deal per rival and a second ask would clear
	-- the first mid-flight.
	--
	-- ★★★★★ THE LUXURY THE SEAT LACKS, BOUGHT THE SAME WAY. The second
	-- purchase on this arm: `LUXURY_ANY` (or a `RESOURCE_*` name) asks the
	-- rival for one copy of a luxury the seat holds none of, thirty turns,
	-- at the rival's own price. The seat runs Displeased on 48% of its
	-- city-turns — −10% on every yield, ≈1,150 yield points a game — and
	-- luxuries are 60% of its amenities, while rivals visibly hold 2–8
	-- improved luxuries and asked 2–14 Gold a turn per copy in the nine
	-- recorded sales; one copy is +1 Amenity in four cities. The rival's own
	-- tradeable list (`GetPossibleDealItems` with the RIVAL as owner, the
	-- shipped screen's "their available resources" column) is the catalogue;
	-- a luxury is one whose row is `RESOURCECLASS_LUXURY`; the seat lacks it
	-- when the host's own count says zero (the planner's count includes a
	-- suzerain's copy, see the sale arm); a copy this lane is selling to
	-- anyone is not bought back. Their RESOURCES item, one copy, thirty
	-- turns, nothing on our side; EQUALIZE; the handler closes only at or
	-- under the ceiling, and only when their side is exactly that copy.
	if kind == "buy" then
		local luxury = verb == "LUXURY_ANY" or string.find(verb, "^RESOURCE_") ~= nil;
		if verb ~= "OPEN_BORDERS" and not luxury then return false, "buy_unknown_item"; end
		if luxury and verb ~= "LUXURY_ANY"
				and try(function() return GameInfo.Resources[verb]; end, nil) == nil then
			return false, "buy_unknown_item";
		end
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
		if not luxury and try(function() return diplomacy:HasOpenBordersFrom(subject); end, false) then
			return false, "buy_already_open";
		end
		local trade = CivvisTrade;
		local pending = trade.pending[subject];
		if pending ~= nil then
			if (turn - (pending.turn or turn)) < (cfg.TradeResponseTurns or 2) then
				return false, "buy_pending";
			end
			trade.pending[subject] = nil;
			CivvisTrade.abandon(subject, "expired");
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
		local ran, submitted, reason, wantName = pcall(function()
			DealManager.ClearWorkingDeal(DealDirection.OUTGOING, pid, subject);
			local deal = DealManager.GetWorkingDeal(DealDirection.OUTGOING, pid, subject);
			if deal == nil then return false, "no_working_deal"; end
			local want, name = "OPEN_BORDERS", "OPEN_BORDERS";
			if luxury then
				-- Owner first: the RIVAL's tradeable resources, the column the
				-- shipped screen fills for their side of the table.
				local possible = try(function()
					return DealManager.GetPossibleDealItems(subject, pid, DealItemTypes.RESOURCES, deal);
				end, nil) or {};
				local resources = try(function() return player:GetResources(); end, nil);
				local forType = nil;
				for _, entry in ipairs(possible) do
					if forType == nil and entry.IsValid ~= false and (entry.MaxAmount or 0) > 0 then
						local row = try(function() return GameInfo.Resources[entry.ForType]; end, nil);
						if row ~= nil and row.ResourceClassType == "RESOURCECLASS_LUXURY"
								and (verb == "LUXURY_ANY" or row.ResourceType == verb) then
							-- The host's own count; a stub that answers
							-- nothing is not zero and is left alone.
							local owned = resources ~= nil and try(function()
								return resources:GetResourceAmount(entry.ForType);
							end, nil) or nil;
							local key = "RESOURCES:" .. tostring(entry.ForType);
							local selling = false;
							for _, other in pairs(trade.pending) do
								if other.gave ~= nil and other.gave[key] ~= nil then selling = true; end
							end
							if owned == 0 and not selling then
								forType, name = entry.ForType, row.ResourceType;
							end
						end
					end
				end
				if forType == nil then return false, "buy_no_luxury"; end
				local item = deal:AddItemOfType(DealItemTypes.RESOURCES, subject);
				if item == nil then return false, "no_resource_item"; end
				item:SetValueType(forType);
				item:SetDuration(30);
				item:SetAmount(1);
				if not try(function() return item:IsValid(); end, true) then
					pcall(function() deal:RemoveItemByID(item:GetID()); end);
					return false, "resource_invalid";
				end
				want = "RESOURCES:" .. tostring(forType);
			else
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
			end
			deal:Validate();
			if not deal:IsValid() then return false, "invalid_deal", name; end
			-- Registered BEFORE the ask goes out — see the sale arm above.
			-- `want` is the key the handler matches their side against.
			trade.pending[subject] = {
				turn = turn, ceiling = ceiling, direction = "buy", verb = name, want = want,
			};
			CivvisTrade.ask(pid, subject, "EQUALIZE", "buy", turn);
			return true, "asked", name;
		end);
		if not ran then
			submitted, reason, wantName = false, "throw", nil;
		end
		if submitted then
			trade.asked[subject] = turn;
		elseif trade.pending[subject] ~= nil and trade.pending[subject].turn == turn
				and trade.pending[subject].direction == "buy" then
			-- The ask itself threw after registering; nothing is in flight.
			trade.pending[subject] = nil;
		end
		if not submitted and (reason == "no_agreement_type" or reason == "no_agreement_item"
				or reason == "agreement_invalid" or reason == "invalid_deal"
				or reason == "buy_no_luxury" or reason == "no_resource_item"
				or reason == "resource_invalid") then
			-- The engine will not sell passage here right now — usually a
			-- missing Early Empire on one side — or has no luxury the seat
			-- lacks on its table; do not re-ask every turn for the same
			-- answer.
			trade.asked[subject] = turn;
			pcall(function() DealManager.ClearWorkingDeal(DealDirection.OUTGOING, pid, subject); end);
		end
		emit("deal_offer", {
			turn = turn, target = subject, verb = verb, direction = "buy",
			ceiling = ceiling, want = wantName,
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
	-- (`SetGivingTokensConsidered`) is deliberately NOT made in this order arm:
	-- with the token spent the `GIVE_INFLUENCE_TOKEN` blocker is not raised, and
	-- when CIVVIS keeps one back the blocker arm below marks the prompt considered
	-- through a fresh handle before forcing the turn forward.
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
		local desired, desiredNames, seen = {}, {}, {};
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
				desiredNames[#desiredNames + 1] = resolved;
				seen[card.Index] = true;
			end
		end

		local slots = try(function() return culture:GetNumPolicySlots(); end, 0) or 0;
		if #desired > slots then return false, "policy_deck_too_large"; end
		local signature = table.concat(desiredNames, ",");
		local samePlan = CivvisPolicy.signature == signature;
		CivvisPolicy.signature = signature;
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
		local slotNames, currentBySlot, currentSet = {}, {}, {};
		local currentCount = 0;
		for i = 0, slots - 1 do
			slotNames[i] = try(function()
				local slotId = culture:GetSlotType(i);
				return GameInfo.GovernmentSlots[slotId].GovernmentSlotType;
			end);
			local current = try(function() return culture:GetSlotPolicy(i); end, -1);
			currentBySlot[i] = current;
			if current ~= nil and current >= 0 then
				currentSet[current] = true;
				currentCount = currentCount + 1;
			end
		end
		local desiredSet = {};
		for _, card in ipairs(desired) do desiredSet[card.Index] = true; end
		local function currentName(index)
			if index == nil or index < 0 then return nil; end
			local row = GameInfo.Policies[index];
			return row ~= nil and row.PolicyType or tostring(index);
		end
		local function fits(card, slotType)
			-- The stock UI treats great-person policy slots as wildcards.  The
			-- normal path rarely sees them, but the repair must use the same rule.
			if slotType == "SLOT_GREAT_PERSON" then slotType = "SLOT_WILDCARD"; end
			local cardType = card.GovernmentSlotType;
			if cardType == "SLOT_GREAT_PERSON" then cardType = "SLOT_WILDCARD"; end
			return slotType == nil or slotType == "SLOT_WILDCARD" or slotType == cardType;
		end
		local function exactDeck()
			if currentCount ~= #desired then return false; end
			for _, card in ipairs(desired) do
				if not currentSet[card.Index] then return false; end
			end
			return true;
		end
		local function request(clearList, addList, mode, repaired)
			local layout = {};
			for i = 0, slots - 1 do
				layout[#layout + 1] = {
					slot = i, slot_type = slotNames[i],
					current = currentName(currentBySlot[i]), add = addList[i],
				};
			end
			-- Only one policy transaction may be in flight for a host turn.  The
			-- same-turn frame/replan requests otherwise race the first transaction;
			-- a later export, not another pcall, is the useful feedback signal.
			CivvisPolicy.attempt_turn = turn;
			local ok, result = pcall(function()
				return culture:RequestPolicyChanges(clearList, addList);
			end);
			emit("policy_deck_request", {
				turn = turn, mode = mode, repaired = repaired,
				desired = desiredNames, clear = clearList, slots = layout,
				pcall_ok = ok, pcall_result = ok and result or nil,
			});
			if ok then
				CivvisPolicy.sent_signature = signature;
				CivvisPolicy.sent_turn = turn;
				CivvisPolicy.pending = {
					turn = turn, mode = mode, desired = desiredNames,
				};
			end
			return ok;
		end
		if exactDeck() then return true, "policy_deck_already_applied"; end
		if CivvisPolicy.attempt_turn == turn then
			emit("policy_deck_deferred", {
				turn = turn, desired = desiredNames,
				why = "same_turn_transaction_in_flight",
			});
			return false, "policy_deck_same_turn";
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

		-- If the same deck was sent on an earlier turn but the next export still
		-- lacks a card, replace one unwanted/empty slot at a time.  This mirrors
		-- the proven single-card arm and gives the host a fresh transaction after
		-- a partial full-deck apply; it also keeps the retry bounded to one card
		-- per turn instead of flooding the same asynchronous request.
		if samePlan and CivvisPolicy.sent_signature == signature
				and CivvisPolicy.sent_turn < turn then
			local missing = {};
			for _, card in ipairs(desired) do
				if not currentSet[card.Index] then missing[#missing + 1] = card; end
			end
			for _, card in ipairs(missing) do
				local target = nil;
				for i = 0, slots - 1 do
					if (currentBySlot[i] == nil or currentBySlot[i] < 0)
							and fits(card, slotNames[i]) then
						target = i;
						break;
					end
				end
				if target == nil then
					for i = 0, slots - 1 do
						local held = currentBySlot[i];
						if held ~= nil and held >= 0 and not desiredSet[held]
							and fits(card, slotNames[i]) then
							target = i;
							break;
						end
					end
				end
				if target ~= nil then
					local repairAdd = {};
					repairAdd[target] = card.Hash;
					local repairClear = { target };
					local ok = request(repairClear, repairAdd, "repair", card.PolicyType);
					return ok, ok and "policy_deck_repair" or "throw";
				end
			end
		end

		local clearList = {};
		for i = 0, slots - 1 do clearList[#clearList + 1] = i; end
		local ok = request(clearList, addList, "full", nil);
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

	-- CIVVIS names the dedication it selected; the host operation accepts the
	-- choice value returned by Game.GetEras(), which may be an index/hash rather
	-- than the visible `COMMEMORATION_*` type name. Match the requested name
	-- against the currently offered choices before submitting it. Reusing the
	-- first offered choice here would recreate the exact ownership leak this arm
	-- is meant to close.
	if kind == "dedication" then
		local param = try(function()
			return PlayerOperations.PARAM_COMMEMORATION_TYPE;
		end);
		local operation = try(function() return PlayerOperations.COMMEMORATE; end);
		if param == nil or operation == nil then
			return false, "dedication_api_unavailable";
		end
		local eras = try(function() return Game.GetEras(); end);
		if eras == nil then return false, "dedication_no_eras"; end
		local allowed = tonumber(try(function()
			return eras:GetPlayerNumAllowedCommemorations(pid);
		end, 0)) or 0;
		if allowed <= 0 then return false, "dedication_none_allowed"; end
		local choices = try(function()
			return eras:GetPlayerCommemorateChoices(pid);
		end);
		if choices == nil then return false, "dedication_no_choices"; end
		local selected;
		for _, choice in ipairs(choices) do
			local choiceRow = GameInfo.CommemorationTypes[choice];
			local choiceName = choiceRow and choiceRow.CommemorationType
				or tostring(choice);
			if choiceName == verb then
				selected = choice;
				break;
			end
		end
		if selected == nil then
			return false, "dedication_not_offered_" .. verb;
		end
		local params = {};
		params[param] = selected;
		local ok = pcall(function()
			UI.RequestPlayerOperation(pid, operation, params);
		end);
		return ok, ok and verb or "dedication_throw";
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
			pendingReligionChoice = {
				mode = "found",
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
		-- Read the queue once before considering any emergency replacement.  A
		-- defender one turn from completion is an immediate answer to a siege;
		-- replacing it with Walls that need several turns leaves the city with
		-- neither defense.  `GetTurnsLeft` is the shipped production-panel
		-- accessor (Base/Assets/UI/Panels/ProductionPanel.lua:1881).
		local current = try(function()
			local q = city:GetBuildQueue();
			return q and q:GetCurrentProductionTypeHash() or 0;
		end, 0) or 0;
		local currentTurns = tonumber(try(function()
			return city:GetBuildQueue():GetTurnsLeft();
		end, -1)) or -1;
		-- An opening Settler is a commitment, not a provisional queue suggestion.
		-- CIVVIS receives a fresh board every turn and can otherwise replace it
		-- with a newly preferred Scout before either item completes.  The live
		-- land-grab policy starts a second Settler while the first walks.  The
		-- first can then found city two before that second queue finishes; using
		-- only the current city count released the queue on that exact frame and
		-- replaced it with the deferred opening Warrior.  Remember a Settler that
		-- was already protected in the one-city opening, and release that exact
		-- city lock only after its host queue changes away from Settler.  A later
		-- two-city Settler never acquires the lock, so ordinary replacement stays
		-- available once the opening pipeline has actually completed.
		local currentOpening = current;
		local settlerRow = GameInfo.Types["UNIT_SETTLER"];
		if currentOpening ~= 0 and settlerRow ~= nil
				and currentOpening == settlerRow.Hash then
			local cityCount = 0;
			eachCity(player, function() cityCount = cityCount + 1; end);
			if cityCount == 1 then CivvisOpeningSettlerLocks[cityId] = true; end
			if resolved ~= "UNIT_SETTLER"
					and (cityCount == 1 or CivvisOpeningSettlerLocks[cityId]) then
				emit("opening_settler_preserved", {
					turn = turn, city = cityId, requested = resolved,
					cities = cityCount,
					pipeline = CivvisOpeningSettlerLocks[cityId] == true,
				});
				return false, "opening_settler_in_progress";
			end
		else
			CivvisOpeningSettlerLocks[cityId] = nil;
		end
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
		local immediateThreat = maxWallDamage ~= nil and maxWallDamage <= 0
			and ((damage ~= nil and damage > 0)
				or (nearestEnemy ~= nil and nearestEnemy <= wallRadius));
		local currentUnit = current ~= 0 and try(function()
			return GameInfo.Units[current];
		end) or nil;
		local finishingDefender = currentTurns >= 0 and currentTurns <= 1
			and currentUnit ~= nil
			and ((currentUnit.Combat or 0) > 0
				or (currentUnit.RangedCombat or 0) > 0
				or (currentUnit.Bombard or 0) > 0);
		-- Preserve the response the city can field this turn before recording a
		-- replacement intent.  In civvis-20260826T091338Z, replacing a one-turn
		-- Archer with four-turn Walls let the attacker take Ostia before either
		-- defense existed.  Returning here keeps both the current queue and the
		-- fallback's remembered build untouched for the finishing turn.
		if immediateThreat and finishingDefender then
			emit("emergency_defender_preserved", {
				turn = turn, city = cityId, requested = resolved,
				current = currentUnit.UnitType or tostring(current),
				current_turns = currentTurns, at_war = atWar,
				enemy_distance = nearestEnemy, damage = damage,
				wall_damage = wallDamage, max_wall_damage = maxWallDamage,
				radius = wallRadius,
			});
			return true, "finishing_defender_preserved";
		end
		civvisBuild[cityId] = resolved;
		local emergencyWall = false;
		if resolved ~= "BUILDING_WALLS"
				and immediateThreat then
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
		if ok and verb == "UNIT_SETTLER" then
			local cityCount = 0;
			eachCity(player, function() cityCount = cityCount + 1; end);
			if cityCount == 1 then CivvisOpeningSettlerLocks[cityId] = true; end
		end
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
			--
			-- ⚠ BOTH ENUM SPELLINGS, because the same table name is registered by
			-- two binaries with different members and this script sees globals
			-- from both -- see `CivvisMilitaryFormation` for the byte offsets.
			-- These three reads asked only for the LONG spelling, which is the
			-- one `Civ6_Exe_Child` registers; if the gameplay VM's table wins
			-- here instead, every one of them is `nil`, the parameter is simply
			-- never set (assigning nil to a Lua table key is not an assignment),
			-- and CIVVIS could never buy a Corps or an Army with no error
			-- anywhere. `or` is free when the first spelling resolves -- the
			-- values are 0/1/2 and 0 is TRUTHY in Lua, so a real STANDARD is
			-- never mistaken for a missing member.
			local formation = tonumber(x) or 0;
			formationForCost = MilitaryFormationTypes.STANDARD_MILITARY_FORMATION
				or MilitaryFormationTypes.STANDARD_FORMATION;
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
				formationForCost = MilitaryFormationTypes.CORPS_MILITARY_FORMATION
					or MilitaryFormationTypes.CORPS_FORMATION;
				params[CityCommandTypes.PARAM_MILITARY_FORMATION_TYPE] = formationForCost;
			elseif formation == 2 then
				formationForCost = MilitaryFormationTypes.ARMY_MILITARY_FORMATION
					or MilitaryFormationTypes.ARMY_FORMATION;
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

	-- ★★★ PREVIEW: a strike the decider wants PRICED, not fought. `subject` is
	-- the unit, `verb` ATTACK or RANGE_ATTACK, `x`/`y` the target plot, and the
	-- answer is a `preview` event carrying the host's own combat simulation.
	-- `CivvisLedger.preview` wraps the same call the shipped combat preview
	-- panel makes -- `Base/Assets/UI/Panels/UnitPanel.lua:3924`:
	--
	--   m_combatResults = CombatManager.SimulateAttackInto( attacker, eCombatType, locX, locY );
	--
	-- with `eCombatType` nil for melee and, for a ranged unit (:3908-3913),
	-- `CombatTypes.RANGED` or `BOMBARD` "if (pUnit:GetBombardCombat() >
	-- pUnit:GetRangedCombat())". Until now that simulation ran only inside
	-- `CivvisLedger.strike`, at issue time, so the decider could read the
	-- host's price of a blow only after committing to it. NOTHING IS
	-- REQUESTED here: no operation, no strike-ledger entry, no war-starter
	-- check -- the simulation has no side effect, and a war the strike would
	-- start is the strike's refusal, not the preview's. A host that cannot
	-- simulate (Game Core busy, no result table) is the named refusal
	-- `preview_unavailable`, so an unanswered ask is never silent.
	if kind == "preview" then
		local unit = liveUnit(pid, subject);
		if unit == nil then return false, "unit_gone:" .. tostring(subject); end
		if verb ~= "ATTACK" and verb ~= "RANGE_ATTACK" then
			return false, "preview_unknown_verb_" .. verb;
		end
		if x == nil or y == nil then return false, "preview_no_target"; end
		local preview = CivvisLedger.preview(unit, verb, x, y);
		if preview == nil then return false, "preview_unavailable"; end
		emit("preview", {
			turn = turn,
			frame = (CivvisFrames ~= nil and CivvisFrames.current) or 0,
			unit = subject, verb = verb, x = x, y = y,
			preview = preview,
		});
		return true, "PREVIEW";
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
		-- ★★★ FORM A CORPS OR AN ARMY. Two commands, not one, and the tier is
		-- the caller's choice: `civvis_orders::translate` picks it from
		-- `Game::can_combine_units`'s own rule and names it in the verb, so this
		-- side never guesses which merge was decided.
		--
		-- Request shape read off the installed game, not recalled. The shipped
		-- `Base/Assets/UI/WorldInput.lua` builds both from the OTHER unit's
		-- owner and id -- `FormCorps` at 2879-2882, `FormArmy` at 2949-2952:
		--
		--   tParameters[UnitCommandTypes.PARAM_UNIT_PLAYER] = pUnit:GetOwner();
		--   tParameters[UnitCommandTypes.PARAM_UNIT_ID]     = pUnit:GetID();
		--   if (UnitManager.CanStartCommand(pSelectedUnit,
		--         UnitCommandTypes.FORM_CORPS, tParameters)) then
		--     UnitManager.RequestCommand(pSelectedUnit,
		--         UnitCommandTypes.FORM_CORPS, tParameters);
		--
		-- which is the identical non-positional owner/id pair ENTER_FORMATION
		-- already carries in `x`/`y`, so the order shape needs nothing new.
		--
		-- ⚠ Both rows DO carry an `InterfaceMode` (INTERFACEMODE_FORM_CORPS /
		-- _FORM_ARMY, UnitCommands.xml:44-45), unlike CONDEMN_HERETIC. That mode
		-- exists only so a human can CLICK the partner: the handler above is
		-- what the mode's click lands in, and it requests the command outright
		-- once it holds the pair. CIVVIS has already chosen the partner, so
		-- entering the mode would be asking the UI a question we answered.
		--
		-- A refusal is named per tier. `cannot_form_army` against a unit CIVVIS
		-- believes is already a Corps is precisely the signal that the mirror's
		-- formation tier and the host's have diverged -- the export carries no
		-- military formation today, so that divergence has to be observable.
		if verb == "FORM_CORPS" or verb == "FORM_ARMY" then
			-- `x`/`y` carry the partner's owner and id, as for ENTER_FORMATION.
			if x == nil or y == nil then return false, "no_formation_target"; end
			if liveUnit(x, y) == nil then return false, "formation_target_gone"; end
			local hash = CMD["UNITCOMMAND_" .. verb];
			if hash == nil then return false, "unknown_cmd_" .. verb; end
			local params = {};
			params[UnitCommandTypes.PARAM_UNIT_PLAYER] = x;
			params[UnitCommandTypes.PARAM_UNIT_ID] = y;
			local okCan, can = pcall(function()
				return UnitManager.CanStartCommand(unit, hash, params);
			end);
			if not (okCan and can == true) then
				return false, "cannot_" .. string.lower(verb);
			end
			local ok = pcall(function()
				UnitManager.RequestCommand(unit, hash, params);
			end);
			return ok, ok and verb or "throw";
		end
		-- FOUND_CITY, MOVE_TO and RANGE_ATTACK are the three that decide a game.
		-- ⚠ There is NO attack operation on this build — the resolved list is only
		-- MOVE_TO and RANGE_ATTACK — so a melee strike IS a MOVE_TO onto the
		-- defended plot. CIVVIS's `Attack` therefore translates to MOVE_TO, and
		-- that is not a workaround: it is how Civilization VI resolves it.
		if verb == "FOUND_CITY" then
			-- ⚠ READ THE PLOT BEFORE FOUNDING. A settler founds where it STANDS, so
			-- the operation takes no x/y, and the unit is consumed by it —
			-- afterwards there is nothing left to ask. The refusal path below can
			-- still call `unit:GetX()` precisely because it did not found.
			local atX = try(function() return unit:GetX(); end);
			local atY = try(function() return unit:GetY(); end);
			-- ★★★★★ FOUND ONLY ON THE SITE THE BRAIN CHOSE. The row now carries
			-- the site (the hex the planned walk ends on). `applyOrders` runs
			-- every FOUND_CITY row BEFORE the settler's own MOVE_TO and re-queues
			-- a refused one behind the walk, which was right while the host
			-- refused an off-site found — but Civilization VI refuses a found
			-- only where founding is ILLEGAL, and the hex one step short of a
			-- chosen site is legal far more often than not (`CITY_MIN_RANGE` 3).
			-- So a planned "step, then settle" founded on the hex BEFORE the
			-- step, and a walk the host capped short founded on the capped hex
			-- once the settler arrived there with movement to spare. A row
			-- without a site (an older brain) keeps the old behaviour. The
			-- miss is named, not blocked: `found_refused` feeds the brain's
			-- permanent `blocked_city_sites`, and standing one hex off is not
			-- a verdict on the ground.
			if x ~= nil and y ~= nil and (atX ~= x or atY ~= y) then
				return false, "found_off_site";
			end
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
		-- CAPTURE is a MOVE_TO onto an enemy civilian with the attack modifier
		-- and without a strike ledger entry: no combat follows, the unit simply
		-- arrives and the civilian is ours. Without the modifier the host walks
		-- next to the civilian and stops (see `attackModifiers`); measured on
		-- the 273 live runs that carried #2075: 65 bare MOVE_TO orders aimed at
		-- an unguarded barbarian-held settler, zero captures.
		-- ★★★ SWAP. Two adjacent friendly units trade hexes; `x`/`y` is the
		-- PARTNER'S plot, exactly the positional pair the shipped UI hands the
		-- operation. `Base/Assets/UI/Civ6Common.lua:160-161`
		-- (`RequestMoveOperation`, the branch a human's move takes when the
		-- destination holds a friendly unit):
		--
		--   if (UnitManager.CanStartOperation( kUnit, UnitOperationTypes.SWAP_UNITS, nil, tParameters) ) then
		--       UnitManager.RequestOperation(kUnit, UnitOperationTypes.SWAP_UNITS, tParameters);
		--
		-- with `tParameters[PARAM_X]`/`[PARAM_Y]` = the destination plot
		-- (`WorldInput.lua:940-943` asks the same question the same way to draw
		-- the swap path). `canOperate` is that four-argument
		-- `CanStartOperation(unit, hash, nil, params)`, asked with the SAME
		-- params the request then carries; a host that declines (not adjacent,
		-- no friendly unit there, a different stacking layer, no movement left)
		-- is a NAMED refusal, `cannot_swap`, never a silent no-op. Not a war
		-- starter — both units are ours — and never reach-capped: the partner
		-- is adjacent by construction.
		if verb == "SWAP" then
			if x == nil or y == nil then return false, "no_dest"; end
			local hash = OP["UNITOPERATION_SWAP_UNITS"];
			if hash == nil then return false, "unknown_op_" .. verb; end
			local params = {};
			params[UnitOperationTypes.PARAM_X] = x;
			params[UnitOperationTypes.PARAM_Y] = y;
			if not canOperate(unit, hash, params) then return false, "cannot_swap"; end
			return operate(unit, hash, params), verb;
		end
		if verb == "MOVE_TO" or verb == "ATTACK" or verb == "CAPTURE" then
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
			-- See `warStarters`: an order the engine would answer with a war
			-- the agent never declared is refused before it is sent — and
			-- that includes the PLAIN MOVE. Run civvis-20260829T105710Z, the
			-- last game before #2755: Nubia read DIPLO_STATE_FRIENDLY at t139
			-- frame 0 and DIPLO_STATE_WAR with 150 grievances against us at
			-- frame 1, with two MOVE_TO orders and a FORTIFY the only thing
			-- applied in between, no strike and no `war` order. A move onto a
			-- plot a peaceful civilian stands on is a capture, and the engine
			-- declares the surprise war to make it — which is exactly why the
			-- shipped `WorldInput.lua:2067` asks this question before EVERY
			-- move, not only before an attack. The leg is checked at the plot
			-- it will be sent to, after the reach cap.
			local warRefusal = CivvisLedger.refuseWarStarter(unit, subject, verb, x, y, turn);
			if warRefusal ~= nil then return false, warRefusal; end
			local params = {};
			params[UnitOperationTypes.PARAM_X] = x;
			params[UnitOperationTypes.PARAM_Y] = y;
			if verb == "ATTACK" or verb == "CAPTURE" then
				-- See `attackModifiers`: MOVE_TO without this flag is a walk, not
				-- a strike, and the whole army has been swinging at air.
				local modifiers = CivvisLedger.attackModifiers();
				if modifiers ~= nil then
					params[UnitOperationTypes.PARAM_MODIFIERS] = modifiers;
				end
				if verb == "ATTACK" then
					CivvisLedger.strike(unit, subject, verb, x, y, turn);
				end
			end
			-- See `CivvisBoard.moveNoop`: the plot and movement the unit had
			-- when the leg was requested are what the queue compares against
			-- to tell a leg the host accepted and never walked from one it is
			-- still walking.
			local fromX = tonumber(try(function() return unit:GetX(); end, nil));
			local fromY = tonumber(try(function() return unit:GetY(); end, nil));
			local movesBefore = tonumber(try(function() return unit:GetMovesRemaining(); end, nil));
			local moved = operate(unit, OP["UNITOPERATION_MOVE_TO"], params);
			if moved and verb == "MOVE_TO" then
				CivvisBoard.noteMoveAttempt(subject, turn, fromX, fromY, x, y, movesBefore);
			end
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
				-- ★★★★ A LEG THE HOST WILL NOT START IS ANSWERED IN THE SAME
				-- PASS. The refusal above is the ledger's; this is the unit's.
				-- See `CivvisBoard.fallbackStep`: the nearest legal neighbour
				-- toward the wanted plot is sent instead of leaving the unit
				-- where it stands for a turn — which, for a wounded unit inside
				-- a raider's reach, is the turn it dies on.
				if verb == "MOVE_TO" and fromX ~= nil and fromY ~= nil then
					local sent = CivvisBoard.fallbackStep(player, pid, unit, subject,
					                                      fromX, fromY, x, y, turn, "cannot_start");
					if sent ~= nil then
						row.x, row.y = sent.x, sent.y;
						return true, verb;
					end
				end
			end
			return moved, verb;
		end
		if verb == "RANGE_ATTACK" then
			if x == nil or y == nil then return false, "no_dest"; end
			local warRefusal = CivvisLedger.refuseWarStarter(unit, subject, verb, x, y, turn);
			if warRefusal ~= nil then return false, warRefusal; end
			local params = {};
			params[UnitOperationTypes.PARAM_X] = x;
			params[UnitOperationTypes.PARAM_Y] = y;
			CivvisLedger.strike(unit, subject, verb, x, y, turn);
			local accepted = operate(unit, OP["UNITOPERATION_RANGE_ATTACK"], params);
			if not accepted then
				-- The simulator can preview a shot that the host rejects for a
				-- target-specific reason (LOS, range, diplomatic state, etc.). Keep
				-- the decision and the host verdict separate so the next run can
				-- repair the right side of the bridge instead of guessing from the
				-- aggregate RANGE_ATTACK refusal count.
				emit("range_attack_refused", {
					turn = turn, unit = subject, unit_kind = unitTypeName(unit),
					unit_x = try(function() return unit:GetX(); end, -1),
					unit_y = try(function() return unit:GetY(); end, -1),
					x = x, y = y,
					moves = try(function() return unit:GetMovesRemaining(); end, -1),
					attacks = try(function() return unit:GetAttacksRemaining(); end, -1),
					activity = try(function()
						return UnitManager.GetActivityType(unit);
					end, nil),
					why = refusalReason(unit, OP["UNITOPERATION_RANGE_ATTACK"], params),
				});
			end
			return accepted, verb;
		end
		-- ★★★ THE AIR VERBS. `AIR_ATTACK` (x/y = the target plot), `REBASE`
		-- (x/y = the new base plot) and `PATROL` (x/y = the plot to fly to and
		-- intercept from) — until this branch every one fell through Rust's
		-- `translate` as `unit_action_untranslated`, so no aircraft the seat
		-- built ever flew. Each is one shipped operation requested with the
		-- plot as the positional pair, exactly as `WorldInput.lua` does:
		--
		--   :2077  if (UnitManager.CanStartOperation( pSelectedUnit, UnitOperationTypes.AIR_ATTACK, nil, tParameters)) then
		--   :2078      UnitManager.RequestOperation( pSelectedUnit, UnitOperationTypes.AIR_ATTACK, tParameters);
		--   :2418  if (UnitManager.CanStartOperation( pSelectedUnit, UnitOperationTypes.DEPLOY, nil, tParameters)) then
		--   :2486  if (UnitManager.CanStartOperation( pSelectedUnit, UnitOperationTypes.REBASE, nil, tParameters)) then
		--
		-- with `tParameters[PARAM_X]`/`[PARAM_Y]` = `plot:GetX()`/`GetY()`.
		-- `canOperate` is that four-argument `CanStartOperation(unit, hash,
		-- nil, params)`, asked with the SAME table the request then carries; a
		-- host that declines (out of range, no base slot there, no target,
		-- attack spent) is the NAMED refusal `cannot_<verb>`, never a silent
		-- no-op. `AIR_ATTACK` is a strike: `refuseWarStarter` holds it back
		-- when the engine would answer with an undeclared war (the shipped
		-- `WorldInput.lua:2067` asks `IsAttackChangeWarState` before the same
		-- request), and `CivvisLedger.strike` records it so the preview and
		-- the combat frame count follow, as they do for RANGE_ATTACK.
		-- `REBASE` and `PATROL` move an aircraft between friendly plots and
		-- start no war. Never reach-capped: an air operation is one hop.
		if verb == "AIR_ATTACK" or verb == "REBASE" or verb == "PATROL" then
			if x == nil or y == nil then return false, "no_dest"; end
			local opName = "UNITOPERATION_AIR_ATTACK";
			if verb == "REBASE" then opName = "UNITOPERATION_REBASE"; end
			if verb == "PATROL" then opName = "UNITOPERATION_DEPLOY"; end
			local hash = OP[opName];
			if hash == nil then return false, "unknown_op_" .. verb; end
			local params = {};
			params[UnitOperationTypes.PARAM_X] = x;
			params[UnitOperationTypes.PARAM_Y] = y;
			if verb == "AIR_ATTACK" then
				local warRefusal = CivvisLedger.refuseWarStarter(unit, subject, verb, x, y, turn);
				if warRefusal ~= nil then return false, warRefusal; end
			end
			if not canOperate(unit, hash, params) then
				if verb == "AIR_ATTACK" then
					-- The same event RANGE_ATTACK files, so the decider's
					-- `blocked_strikes` keeps the refused pair off the next frame.
					emit("range_attack_refused", {
						turn = turn, unit = subject, unit_kind = unitTypeName(unit),
						verb = verb,
						unit_x = try(function() return unit:GetX(); end, -1),
						unit_y = try(function() return unit:GetY(); end, -1),
						x = x, y = y,
						moves = try(function() return unit:GetMovesRemaining(); end, -1),
						attacks = try(function() return unit:GetAttacksRemaining(); end, -1),
						why = refusalReason(unit, hash, params),
					});
				end
				return false, "cannot_" .. string.lower(verb);
			end
			if verb == "AIR_ATTACK" then
				CivvisLedger.strike(unit, subject, verb, x, y, turn);
			end
			return operate(unit, hash, params), verb;
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
			-- This is a POLICY. It does not pick a tile or an improvement — Civ 6's own builder
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
		local beliefName = string.match(tostring(verb), "^EVANGELIZE_BELIEF:(.+)$");
		if beliefName ~= nil then
			if pendingReligionChoice ~= nil then return false, "religion_choice_pending"; end
			local belief, resolved = resolveType(GameInfo.Beliefs, beliefName);
			if belief == nil then return false, "unknown_belief_" .. beliefName; end
			local playerReligion = try(function() return player:GetReligion(); end);
			if playerReligion == nil then return false, "no_religion_api"; end
			if try(function() return playerReligion:GetReligionTypeCreated(); end, -1) < 0 then
				return false, "religion_not_founded";
			end
			local gameReligion = try(function() return Game.GetReligion(); end);
			if gameReligion == nil then return false, "no_game_religion"; end
			if try(function() return gameReligion:IsInSomeReligion(belief.Index); end, true) then
				return false, "taken_" .. resolved;
			end
			-- The operation itself opens the selection prompt; the blocker handler
			-- above supplies the selected belief only after that native state exists.
			local started = operate(unit, OP["UNITOPERATION_EVANGELIZE_BELIEF"], {});
			if not started then return false, "cannot_evangelize_" .. resolved; end
			pendingReligionChoice = {
				mode = "evangelize",
				turn = turn,
				unit = subject,
				belief = resolved,
				belief_index = belief.Index,
				belief_hash = belief.Hash,
				add_requested = false,
			};
			return true, "EVANGELIZE_BELIEF:" .. resolved;
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
		-- ALERT, SKIP_TURN, HEAL, BUILD_IMPROVEMENT,
		-- SPREAD_RELIGION, REMOVE_HERESY, RELIGIOUS_HEAL, LAUNCH_INQUISITION,
		-- CONVERT_BARBARIANS, PILLAGE. All parameterless -- `operate` selects
		-- the shipped UnitPanel's two-argument request for these.
		local hash = OP["UNITOPERATION_" .. verb];
		if hash == nil then return false, "unknown_op_" .. verb; end
		-- ⚠ NAME THE REFUSAL, NOT THE VERB. This tail returned `verb` for BOTH
		-- outcomes, so a REMOVE_HERESY the engine declined and a REMOVE_HERESY
		-- it accepted reached the queue's `refusals` table under the same key --
		-- the anonymous-count trap this file names everywhere else, one level
		-- in. The ledger reads only `why` when `ok` is false, so the two are
		-- worth telling apart: `cannot_REMOVE_HERESY` is the host declining
		-- outright (wrong tile, no rival religion present, charges spent), and
		-- it is a completely different repair from the request raising.
		--
		-- `operate` asks `canOperate` again on the line below; that is a cheap
		-- repeat and deliberately not inlined, so the parameterless request still
		-- passes through the same signature-aware helper as every other operation.
		if not canOperate(unit, hash, {}) then return false, "cannot_" .. verb; end
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
CivvisExportTiles = exportTiles;

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
-- ★★★★★ 2026-08-23: THE PROBE CAME BACK AND THE THEORY IS DEAD.
--
-- #2108 pre-registered its own falsifier -- "watch the first
-- `wc_ballot_verdict` with `asked = 3`; `recorded 1` kills the affordability
-- theory too" -- and then nobody read it. Read now, over every run under
-- `~/civvis-civ6-runs/control/`: **802 verdict rows, 139 multi-vote asks, 0
-- registered.** Twenty-three of those are the three-vote probe, across nine
-- separate post-#2108 runs, and every one recorded ONE. Three votes cost 12
-- Favor on the Online table and 30 on the Standard one against banks of
-- 169-427, with `MaxVotes` 9-15 and this walk's own budget reading host 9-15
-- and standard 6-9 -- affordable on BOTH tables at once, which
-- is the exact ask no ballot had ever made when the theory was written. A
-- core charging Standard while reporting Online would have honoured it.
--
-- Fourteen of the thirty-one probe ballots were cast with
-- `in_congress_segment = true`, from inside `TURNSEG_WORLDCONGRESS_*`, so
-- the moment theory is dead beside it. And the option is NOT what is being
-- refused: `option_asked == option_recorded` on 82.8% of one-vote rows and
-- 73.4% of multi-vote rows, so the ballot registers and only its COUNT is
-- clamped.
--
-- The dual-table cap below is therefore known to be answering a question
-- with a settled negative answer. It is kept, not removed, for the reason
-- its own paragraph gives: when the theory is wrong the cap asks fewer votes
-- on a ballot that registers one either way, so removing it would change no
-- outcome and would only churn a file that cannot be tested without a live
-- game. What is NOT kept is the impression that the question is open.
--
-- ⚠ What remains is host-side and unreachable from this file. The run that
-- would settle it is a single live game with the popup driven by hand for
-- one resolution -- a human clicking two votes -- next to an agent ballot
-- asking two on the same seat, comparing `wc_outcome`. That needs the live
-- harness, which is under an operator halt; do not start one to answer this.
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
-- was already taken upstream. The legal activation plots are the ENGINE's own
-- answer (`GetActivationHighlightPlots`, the call the shipped SelectedUnit.lua
-- shades the map with), but that list is an eligibility highlight, not a
-- route: the host can accept a MOVE_TO to a highlighted plot behind a closed
-- border and leave the unit at its origin. The bridge therefore checks the
-- host pathfinder before choosing a target. Counted apart from `applied`
-- (`gp_activated` / `gp_moving` / `gp_idle`), so telemetry never presents it
-- as CIVVIS's work.
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
	-- UnitRemovedFromMap is the host's completion witness for this command;
	-- retain the turn BEFORE requesting it because a host event may fire
	-- synchronously inside commandUnit.
	local activationKey = tostring(id);
	CivvisLedger.expected_gp_activation[activationKey] = turn;
	if commandUnit(unit, CMD["UNITCOMMAND_ACTIVATE_GREAT_PERSON"]) then
		gpPending[id] = nil;
		emit("gp", { turn = turn, unit = id, individual = individual,
			class = class, action = "activated",
			x = try(function() return unit:GetX(); end, -1),
			y = try(function() return unit:GetY(); end, -1) });
		return "activated";
	end
	CivvisLedger.expected_gp_activation[activationKey] = nil;
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
		-- `GetActivationHighlightPlots` is what the UI shades, not a promise that
		-- this unit can walk to every highlighted tile. In particular, the nearest
		-- natural-wonder tile may belong to a civilization with no Open Borders.
		-- Return nil when the path API is unavailable so old hosts retain the
		-- historical nearest-highlight behavior; return false only when the host
		-- explicitly proves that the destination cannot be reached.
		local function activationReachable(x, y)
			local pathFinder = try(function() return UnitManager.GetMoveToPathEx; end, nil);
			local plotIndexer = try(function() return Map.GetPlotIndex; end, nil);
			if type(pathFinder) ~= "function" or type(plotIndexer) ~= "function" then
				return nil;
			end
			local destination = try(function() return plotIndexer(x, y); end, nil);
			if destination == nil then return false; end
			local path = try(function() return pathFinder(unit, destination); end, nil);
			if path == nil or type(path.plots) ~= "table" then return false; end
			local n = 0;
			for _ in pairs(path.plots) do n = n + 1; end
			-- A one-entry path is the host's no-progress/no-route result. Also
			-- require the endpoint to be the requested plot: a partial route or
			-- stale path must not turn into another endless MOVE_TO.
			if n <= 1 or path.plots[n] ~= destination then return false; end
			return true;
		end
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
					if activationReachable(px, py) ~= false then
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
-- step. A move followed by an action therefore executed as a step: the unit
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
	pending = {},   -- host unit id -> { rows, next, expect, ready, wait, settle_passes }
	order = {},     -- unit ids in the order they were first queued
	count = 0,
	-- Units whose OPENING walk is still in flight and have nothing queued
	-- behind it: a rows-less entry that only holds the turn until the walk
	-- has landed. See `CivvisQueue.watch`.
	watching = 0,
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
	q.pending = {}; q.order = {}; q.count = 0; q.watching = 0; q.turn = turn; q.ticks = 0;
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
	-- A SWAP lands the unit on the partner's plot, the same expectation a
	-- MOVE_TO carries for its destination; a REBASE lands the aircraft on
	-- its new base plot.
	if (verb == "MOVE_TO" or verb == "SWAP" or verb == "REBASE") and x ~= nil and y ~= nil then
		return { x = x, y = y };
	end
	return nil;
end;

CivvisQueue.isStrike = function(row)
	local verb = tostring(row.verb or "");
	return verb == "ATTACK" or verb == "RANGE_ATTACK" or verb == "AIR_ATTACK";
end;

CivvisQueue.push = function(subject, row, expect)
	local q = CivvisQueue;
	local entry = q.pending[subject];
	if entry == nil then
		entry = {
			rows = {}, next = 1, expect = expect, ready = false, wait = 0,
			settle_passes = 0,
		};
		q.pending[subject] = entry;
		q.order[#q.order + 1] = subject;
	elseif #entry.rows == 0 then
		-- A watch on the opening walk becomes a real queue entry.
		q.watching = q.watching - 1;
	end
	entry.rows[#entry.rows + 1] = row;
	q.count = q.count + 1;
	q.stats.queued = q.stats.queued + 1;
	if CivvisQueue.isStrike(row) then q.stats.strikes_planned = q.stats.strikes_planned + 1; end
end;

CivvisQueue.pendingCount = function() return CivvisQueue.count + CivvisQueue.watching; end;

-- ★★★★ HOLD THE TURN UNTIL THE OPENING WALK HAS LANDED, EVEN WITH NOTHING
-- QUEUED BEHIND IT. `settleTurn` decides whether to open a replan frame the
-- first time the queue is empty — and a unit whose whole order was one
-- MOVE_TO never entered the queue, so that decision was taken while the
-- unit was still walking: nothing revealed yet, no frame, the turn latched
-- settled, and the host could open no replan frame from the landed board.
-- A watch is a rows-less entry: it settles like any queued order
-- (arrival, no movement left, the host's own event, or the grace period)
-- and is dropped; it never issues anything and names no refusal.
CivvisQueue.watch = function(subject, expect, origin)
	local q = CivvisQueue;
	if q.pending[subject] ~= nil then return; end
	q.pending[subject] = {
		rows = {}, next = 1, expect = expect, origin = origin, ready = false, wait = 0,
		settle_passes = 0,
	};
	q.order[#q.order + 1] = subject;
	q.watching = q.watching + 1;
end;

CivvisQueue.dropWatch = function(subject, entry)
	local q = CivvisQueue;
	if #entry.rows > 0 then return false; end
	q.pending[subject] = nil;
	q.watching = q.watching - 1;
	return true;
end;

CivvisQueue.refuseRest = function(subject, entry, why)
	local q = CivvisQueue;
	if CivvisQueue.dropWatch(subject, entry) then return; end
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
	if q.count + q.watching <= 0 then return 0; end
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
				-- A host operation can settle on a different plot than the
				-- requested coordinate while an asynchronous walk is in flight.
				-- The rows-less watch exists only to wait for that opening
				-- operation; once the unit has demonstrably left its origin,
				-- release the watch and let the brain re-plan from the actual
				-- board instead of pinning the turn until the grace timeout.
				-- Do not apply this to a real queued follow-up: its origin belongs
				-- to the opening walk and must not make the next row run before its
				-- own expectation settles.
				local moved_from_origin = #entry.rows == 0
					and entry.origin ~= nil
					and (ux ~= entry.origin.x or uy ~= entry.origin.y);
				-- Arrival is not the same as settlement on the live host. Civ VI can
				-- place a unit on the requested plot while its MOVE_TO operation is
				-- still active; a follow-up RequestOperation then returns successfully
				-- but is ignored by the in-flight path. This was visible in the live
				-- trace as MOVE_TO -> FORTIFY: the warrior reached its plot, FORTIFY
				-- was counted as applied, and the next turn's stale-operation cleanup
				-- cancelled the still-active move with the unit unfortified.
				--
				-- Keep the opening rows-less watch's early release: it has no dependent
				-- order to protect and the next board can re-plan from the landed unit.
				-- A real queued follow-up must wait for the host operation to deactivate,
				-- even when the unit has arrived, spent its movement, or raised an event.
				local active_operation = #entry.rows > 0 and try(function()
					return ActivityTypes ~= nil
						and ActivityTypes.ACTIVITY_OPERATION ~= nil
						and UnitManager.GetActivityType(unit) == ActivityTypes.ACTIVITY_OPERATION;
				end, false) == true;
				-- A MOVE_TO can leave the unit on its exact destination while Civ VI
				-- keeps the path operation active until the next turn.  Waiting for the
				-- deactivation event is normally correct, but turn-start ownership
				-- cleanup can cancel that operation first and discard the dependent row.
				-- Once the expected plot is reached, cancel only that landed operation
				-- through the same shipped command used by start-of-turn cleanup.  A
				-- unit still short of its destination remains protected by the active
				-- operation guard above.
				if active_operation and entry.expect ~= nil and arrived then
					local cancel = CMD["UNITCOMMAND_CANCEL"];
					local can_cancel = cancel ~= nil and try(function()
						return UnitManager.CanStartCommand(unit, cancel, false, true) == true;
					end, false) == true;
					if can_cancel and pcall(function() UnitManager.RequestCommand(unit, cancel); end) then
						-- RequestCommand can itself be asynchronous. Re-read the host
						-- activity instead of assuming the cancel returned after the
						-- operation deactivated; otherwise the dependent operation can
						-- lose the same race this guard is meant to prevent.
						local deactivated = try(function()
							return ActivityTypes ~= nil
								and ActivityTypes.ACTIVITY_OPERATION ~= nil
								and UnitManager.GetActivityType(unit) ~= ActivityTypes.ACTIVITY_OPERATION;
						end, false) == true;
						active_operation = not deactivated;
						emit("queue_operation_cancelled", {
							turn = turn, unit = subject, x = ux, y = uy,
							reason = "landed_before_deactivation",
						});
					end
				end
				local ready = (entry.ready or arrived or spent or moved_from_origin
					or entry.wait >= grace) and not active_operation;
				-- A path can report its destination before Civ VI has finished
				-- deactivating the asynchronous MOVE_TO. On the live host the
				-- activity read can briefly say "awake" in that window, so an
				-- immediately accepted FORTIFY is then cancelled by next-turn
				-- cleanup without ever granting fortification. Give only this
				-- MOVE_TO -> FORTIFY handoff one additional drain pass; strikes
				-- and useful actions retain their existing latency.
				if ready and (entry.settle_passes or 0) > 0 then
					entry.settle_passes = entry.settle_passes - 1;
					ready = false;
				end
				-- See `CivvisBoard.moveNoop`: a leg the host accepted, whose unit
				-- is still on the plot it was sent from with its movement intact
				-- once the watch runs out, is a no-op. It is named and answered
				-- here, before the watch is dropped — the drop was the silence.
				local atOrigin = entry.origin ~= nil and ux == entry.origin.x and uy == entry.origin.y;
				local noop = ready and entry.expect ~= nil and not arrived and not spent and atOrigin;
				if noop and CivvisBoard.moveNoop(player, pid, subject, unit, entry, turn, ux, uy, moves) then
					-- A legal neighbour step went out in its place and the watch
					-- was re-armed on that plot; nothing else to do this tick.
				elseif ready and CivvisQueue.dropWatch(subject, entry) then
					-- The opening walk has landed; nothing follows it.
				elseif ready then
					-- A MOVE_TO can report an operation-ended event, spend its movement,
					-- or hit the grace cap without reaching the requested plot.  None of
					-- those outcomes makes a follow-up safe: an IMPROVE would otherwise
					-- run at the old tile (the live trace showed farms requested at a city
					-- centre), and a strike could fire from the wrong tile.  The opening
					-- walk has no follow-up and is allowed to release for re-planning, but
					-- a real queued sequence must either arrive at its expectation or
					-- refuse the remaining rows by name.
					if entry.expect ~= nil and not arrived then
						CivvisQueue.refuseRest(subject, entry, "queue_prior_not_arrived");
					else
						local row = entry.rows[entry.next];
						local verb = tostring(row.verb or "");
						if spent and (verb == "MOVE_TO" or verb == "CAPTURE" or verb == "SWAP"
								or verb == "REBASE" or verb == "PATROL"
								or CivvisQueue.isStrike(row)) then
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
								entry.settle_passes = ok
									and verb == "MOVE_TO"
									and tostring(entry.rows[entry.next].verb or "") == "FORTIFY"
									and 1 or 0;
							end
						end
					end
				end
			end
		end
	end
	if q.count + q.watching <= 0 then
		q.pending = {}; q.order = {};
		-- A turn that only watched walks land has nothing to report.
		if q.stats.queued > 0 then CivvisQueue.report(turn, "drained"); end
	end
	return ran;
end;

-- Past the cap the queue is abandoned by name and the turn may end.
CivvisQueue.giveUp = function(turn)
	local q = CivvisQueue;
	for subject, entry in pairs(q.pending) do
		CivvisQueue.refuseRest(subject, entry, "queue_stalled");
	end
	q.pending = {}; q.order = {}; q.count = 0; q.watching = 0;
	CivvisQueue.report(turn, "stalled");
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
--     older order, the fallback ladder, or a stale host operation) get
--     `UNITCOMMAND_CANCEL` at turn start, so the brain owns them from the next
--     turn on. Civilians keep theirs: a settler's long walk is exactly what a
--     queued path is for.
-- Both are counted (`move_capped`, `queued_paths`) so a run says how often the
-- host and the board disagreed. One bare global table (200-local ceiling).
CivvisBoard = { stats = { capped = 0, no_reach = 0, escort_cap_synced = 0,
	                         escort_cap_unresolved = 0, escort_shadow_injected = 0,
	                         escort_shadow_applied = 0, escort_shadow_refused = 0,
	                         escort_shadow_held = 0,
	                         settler_scout_capture_held = 0,
	                         settler_scout_guard_held = 0,
	                         settler_barbarian_combat_capture_held = 0,
	                         settler_barbarian_combat_guard_held = 0,
	                         settler_barbarian_combat_guard_rescued = 0,
	                         builder_barbarian_capture_held = 0,
	                         builder_capture_escaped = 0,
	                         active_fire_civilian_held = 0,
	                         move_noop = 0, move_fallback = 0 },
	                escortHolds = {}, moveAttempts = {} };

CivvisBoard.reset = function()
	CivvisBoard.stats = { capped = 0, no_reach = 0, escort_cap_synced = 0,
	                     escort_cap_unresolved = 0, escort_shadow_injected = 0,
	                     escort_shadow_applied = 0, escort_shadow_refused = 0,
	                     escort_shadow_held = 0,
	                     settler_scout_capture_held = 0,
	                     settler_scout_guard_held = 0,
	                     settler_barbarian_combat_capture_held = 0,
	                     settler_barbarian_combat_guard_held = 0,
	                     settler_barbarian_combat_guard_rescued = 0,
	                     builder_barbarian_capture_held = 0,
	                     builder_capture_escaped = 0,
	                     active_fire_civilian_held = 0,
	                     move_noop = 0, move_fallback = 0 };
	CivvisBoard.moveAttempts = {};
	CivvisBoard.escortHolds = {};
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
	-- A path object with only the unit's current plot is Civ VI's explicit
	-- no-route/no-progress answer. Treating it as an unknown API result sends
	-- the original MOVE_TO anyway; the host accepts that request and its path
	-- worker retries forever with Distance: 2147483647. A missing path object
	-- above remains compatibility-unknown, but a present one-entry path is a
	-- proven refusal and must be surfaced to the order ledger.
	if n <= 1 then return false, "no_path"; end
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

-- Read the host path all the way through the requested plot.  A nil cap is
-- normally a same-turn path, but it can also mean that WorldInput had no path
-- data at all.  An escort may be synthesized only in the former case.
-- ═══ THE EVACUATION LANDS ═══════════════════════════════════════════════════
--
-- Measured on the 32 ledger runs of 2026-08-30..09-01 that reached turn 100
-- (461 combat deaths of our units, 408 to barbarians): the victim had been
-- ordered `MOVE_TO` on the turn it died or the turn before in the great
-- majority of cases, and 383 of the 461 made NO host move on the turn before
-- they died. The decider had chosen the evacuation. The host had accepted the
-- request — `canOperate` said yes, `RequestOperation` returned — and the unit
-- stood where it was until the next raider blow. Nothing in this file could
-- tell that leg from one still walking: the queue watched it for `grace`
-- ticks and then dropped the watch in silence, and the Rust side discovered
-- the standstill a turn later as `did_not_move` (5,873 of them across those
-- 32 runs), by which time the unit was usually dead.
--
-- Three things change. A `MOVE_TO` records the plot and movement it left
-- from (`noteMoveAttempt`). A watched leg whose unit is still on that plot
-- with its movement intact when the watch runs out is a no-op and is named
-- by probing the host (`classifyNoop`) — `cannot_start`, `no_path`,
-- `beyond_turn`, `occupied`, `hostile_on_plot`, `zoc`, `hostile_adjacent`,
-- `no_moves`, `unknown` — as `move_noop`. And in the same pass the nearest
-- legal neighbour toward the wanted plot is sent instead (`fallbackStep`,
-- `move_fallback`), once per unit per turn, so the unit does not spend the
-- turn standing in reach. A leg the host refuses outright (`operate` false)
-- takes the same fallback at once.
--
-- Every host call is the shipped UI's own:
--   `UnitManager.GetMoveToPathEx( kUnit, endPlotId )`     WorldInput.lua:961
--   `UnitManager.CanStartOperation( pUnit, UnitOperationTypes.MOVE_TO, …)`
--                                                          Panels/UnitPanel.lua:609
--   `UnitManager.RequestOperation(kUnit, UnitOperationTypes.MOVE_TO, tParameters)`
--                                                          Civ6Common.lua:163
--   `Map.GetAdjacentPlot( x, y, direction)`                AdjacencyBonusSupport.lua:95
--   `Map.GetPlotDistance(…)`                               TradeOverview.lua:436
--   `kUnit:GetMovesRemaining()`                            WorldInput.lua:971
--   `pLocalPlayer:GetDiplomacy():IsAtWarWith( iPlayer )`   PartialScreens/CityStates.lua:1508
--   `playerInfo:IsBarbarian()`                             EndGame/EndGameReplayLogic.lua:74
--
-- `MoveFallback = false` in the mod config keeps every part of this off (the
-- A/B arm; `mod_arms.MoveFallback` on the run summary says which it was).

CivvisBoard.noteMoveAttempt = function(subject, turn, fromX, fromY, wantX, wantY, moves)
	if subject == nil then return; end
	local previous = CivvisBoard.moveAttempts[subject];
	local fallback = previous ~= nil and previous.turn == turn and previous.fallback == true;
	CivvisBoard.moveAttempts[subject] = {
		turn = turn, fromX = fromX, fromY = fromY, wantX = wantX, wantY = wantY,
		moves = moves, fallback = fallback,
	};
end;

-- The six plots around (x, y), in direction order, skipping the map edge.
CivvisBoard.adjacentPlots = function(x, y)
	local out = {};
	pcall(function()
		for direction = 0, DirectionTypes.NUM_DIRECTION_TYPES - 1 do
			local plot = Map.GetAdjacentPlot(x, y, direction);
			if plot ~= nil then out[#out + 1] = plot; end
		end
	end);
	return out;
end;

-- Every plot a combat unit of a player we are at war with stands on, the
-- Barbarian seat included, keyed "x,y".
CivvisBoard.hostilePlots = function(pid)
	local plots = {};
	local me = try(function() return Players[pid]; end, nil);
	local diplomacy = nil;
	if me ~= nil then diplomacy = try(function() return me:GetDiplomacy(); end, nil); end
	pcall(function()
		for _, otherId in ipairs(PlayerManager.GetAliveIDs() or {}) do
			if otherId ~= pid then
				local other = Players[otherId];
				local hostile = other ~= nil
					and (try(function() return other:IsBarbarian(); end, false) == true
						or (diplomacy ~= nil
							and try(function() return diplomacy:IsAtWarWith(otherId); end, false) == true));
				if hostile then
					eachUnit(other, function(unit)
						if not CivvisBoard.isCombatEscort(unit) then return; end
						local x = tonumber(try(function() return unit:GetX(); end, nil));
						local y = tonumber(try(function() return unit:GetY(); end, nil));
						if x ~= nil and y ~= nil then plots[x .. "," .. y] = true; end
					end);
				end
			end
		end
	end);
	return plots;
end;

-- Our own units by plot, as the set of Domain values standing there: a land
-- unit cannot stack on a land unit, but it can share a plot with a ship.
CivvisBoard.ownDomainsAt = function(player)
	local plots = {};
	eachUnit(player, function(unit)
		local x = tonumber(try(function() return unit:GetX(); end, nil));
		local y = tonumber(try(function() return unit:GetY(); end, nil));
		local domain = try(function()
			local row = GameInfo.Units[unitTypeName(unit)];
			return row ~= nil and row.Domain or nil;
		end, nil);
		if x ~= nil and y ~= nil and domain ~= nil then
			local key = x .. "," .. y;
			plots[key] = plots[key] or {};
			plots[key][domain] = true;
		end
	end);
	return plots;
end;

CivvisBoard.unitDomain = function(unit)
	return try(function()
		local row = GameInfo.Units[unitTypeName(unit)];
		return row ~= nil and row.Domain or nil;
	end, nil);
end;

-- How many of the plots around (x, y) hold a hostile combat unit.
CivvisBoard.hostileAdjacent = function(hostile, x, y)
	local n = 0;
	for _, plot in ipairs(CivvisBoard.adjacentPlots(x, y)) do
		local px = tonumber(try(function() return plot:GetX(); end, nil));
		local py = tonumber(try(function() return plot:GetY(); end, nil));
		if px ~= nil and py ~= nil and hostile[px .. "," .. py] then n = n + 1; end
	end
	return n;
end;

-- Why a leg the host accepted did not happen, asked of the host itself.
CivvisBoard.classifyNoop = function(player, pid, unit, fromX, fromY, wantX, wantY, moves)
	if moves ~= nil and moves <= 0 then return "no_moves"; end
	local params = {};
	params[UnitOperationTypes.PARAM_X] = wantX;
	params[UnitOperationTypes.PARAM_Y] = wantY;
	if not canOperate(unit, OP["UNITOPERATION_MOVE_TO"], params) then return "cannot_start"; end
	local destination = try(function() return Map.GetPlotIndex(wantX, wantY); end, nil);
	local path = nil;
	if destination ~= nil then
		path = try(function() return UnitManager.GetMoveToPathEx(unit, destination); end, nil);
	end
	local n = 0;
	if path ~= nil and path.plots ~= nil then
		for _ in pairs(path.plots) do n = n + 1; end
	end
	if path == nil or n <= 1 then return "no_path"; end
	local last = nil;
	if path.turns ~= nil then last = tonumber(path.turns[n]); end
	if last ~= nil and last > 1 then return "beyond_turn"; end
	local domain = CivvisBoard.unitDomain(unit);
	local own = CivvisBoard.ownDomainsAt(player)[wantX .. "," .. wantY];
	if own ~= nil and domain ~= nil and own[domain] == true then return "occupied"; end
	local hostile = CivvisBoard.hostilePlots(pid);
	if hostile[wantX .. "," .. wantY] then return "hostile_on_plot"; end
	if CivvisBoard.hostileAdjacent(hostile, fromX, fromY) > 0 then
		if CivvisBoard.hostileAdjacent(hostile, wantX, wantY) > 0 then return "zoc"; end
		return "hostile_adjacent";
	end
	return "unknown";
end;

CivvisBoard.fallbackBetter = function(candidate, best)
	local dc = candidate.distance;
	local db = best.distance;
	if dc < 0 then dc = 99; end
	if db < 0 then db = 99; end
	if dc ~= db then return dc < db; end
	if candidate.exposed ~= best.exposed then return candidate.exposed < best.exposed; end
	return candidate.index < best.index;
end;

-- The nearest legal neighbour toward the wanted plot, sent in place of a leg
-- that will not happen: closest to the destination first, then the plot with
-- the fewest hostile neighbours, then direction order. The wanted plot
-- itself is never re-tried (it just failed), a plot a hostile stands on is
-- an attack, and a plot our own unit of the same domain holds is a stack.
-- Once per unit per turn. Returns the plot sent, or nil.
CivvisBoard.fallbackStep = function(player, pid, unit, subject, fromX, fromY, wantX, wantY, turn, why)
	if cfg.MoveFallback == false then return nil; end
	local attempt = CivvisBoard.moveAttempts[subject];
	if attempt ~= nil and attempt.turn == turn and attempt.fallback == true then return nil; end
	local hostile = CivvisBoard.hostilePlots(pid);
	local own = CivvisBoard.ownDomainsAt(player);
	local domain = CivvisBoard.unitDomain(unit);
	local best = nil;
	for index, plot in ipairs(CivvisBoard.adjacentPlots(fromX, fromY)) do
		local px = tonumber(try(function() return plot:GetX(); end, nil));
		local py = tonumber(try(function() return plot:GetY(); end, nil));
		if px ~= nil and py ~= nil and not (px == wantX and py == wantY) then
			local key = px .. "," .. py;
			local stacked = own[key] ~= nil and domain ~= nil and own[key][domain] == true;
			if not hostile[key] and not stacked then
				local params = {};
				params[UnitOperationTypes.PARAM_X] = px;
				params[UnitOperationTypes.PARAM_Y] = py;
				if canOperate(unit, OP["UNITOPERATION_MOVE_TO"], params) then
					local distance = tonumber(try(function()
						return Map.GetPlotDistance(px, py, wantX, wantY);
					end, -1)) or -1;
					local candidate = {
						x = px, y = py, index = index, params = params,
						distance = distance,
						exposed = CivvisBoard.hostileAdjacent(hostile, px, py),
					};
					if best == nil or CivvisBoard.fallbackBetter(candidate, best) then
						best = candidate;
					end
				end
			end
		end
	end
	if best == nil then return nil; end
	local sent = operate(unit, OP["UNITOPERATION_MOVE_TO"], best.params);
	if not sent then return nil; end
	CivvisBoard.stats.move_fallback = CivvisBoard.stats.move_fallback + 1;
	CivvisBoard.moveAttempts[subject] = {
		turn = turn, fromX = fromX, fromY = fromY, wantX = wantX, wantY = wantY,
		moves = nil, fallback = true,
	};
	emit("move_fallback", {
		turn = turn, unit = subject, unit_kind = unitTypeName(unit),
		from = { fromX, fromY }, want = { wantX, wantY }, sent = { best.x, best.y },
		why = why, distance = best.distance, exposed = best.exposed,
	});
	return { x = best.x, y = best.y };
end;

-- The queue's answer to a watched leg that never left its origin. Names the
-- no-op, sends the fallback step, and re-arms the watch on the plot sent.
-- Returns true when a step went out (the caller leaves the entry alone), false
-- when the existing drop / `queue_prior_not_arrived` handling should run.
CivvisBoard.moveNoop = function(player, pid, subject, unit, entry, turn, ux, uy, moves)
	if cfg.MoveFallback == false then return false; end
	if entry == nil or entry.expect == nil then return false; end
	local wantX, wantY = entry.expect.x, entry.expect.y;
	local attempt = CivvisBoard.moveAttempts[subject];
	-- Movement spent since the request means the host is walking the leg;
	-- that is a slow leg, not a no-op, and it is left alone.
	if attempt ~= nil and attempt.turn == turn and attempt.moves ~= nil and moves ~= nil
			and moves < attempt.moves then
		return false;
	end
	local afterFallback = attempt ~= nil and attempt.turn == turn and attempt.fallback == true;
	local why = CivvisBoard.classifyNoop(player, pid, unit, ux, uy, wantX, wantY, moves);
	CivvisBoard.stats.move_noop = CivvisBoard.stats.move_noop + 1;
	emit("move_noop", {
		turn = turn, unit = subject, unit_kind = unitTypeName(unit),
		from = { ux, uy }, want = { wantX, wantY }, moves = moves,
		ticks = entry.wait, why = why, after_fallback = afterFallback,
	});
	if afterFallback then return false; end
	local sent = CivvisBoard.fallbackStep(player, pid, unit, subject, ux, uy, wantX, wantY, turn, why);
	if sent == nil then return false; end
	entry.expect = { x = sent.x, y = sent.y };
	entry.origin = { x = ux, y = uy };
	entry.ready = false;
	entry.wait = 0;
	return true;
end;

CivvisBoard.reachesThisTurn = function(unit, x, y)
	local path = try(function()
		return UnitManager.GetMoveToPathEx(unit, Map.GetPlotIndex(x, y));
	end, nil);
	if path == nil or path.plots == nil or path.turns == nil then
		return false, "path_unknown";
	end
	local n = 0;
	for _ in pairs(path.plots) do n = n + 1; end
	local destination = try(function() return Map.GetPlotIndex(x, y); end, nil);
	if n <= 0 or destination == nil or path.plots[n] ~= destination then
		return false, "path_destination_unknown";
	end
	local last = tonumber(path.turns[n]);
	if last == nil then return false, "path_turn_unknown"; end
	if last > 1 then return false, "guard_still_capped"; end
	local params = {};
	params[UnitOperationTypes.PARAM_X] = x;
	params[UnitOperationTypes.PARAM_Y] = y;
	if not canOperate(unit, OP["UNITOPERATION_MOVE_TO"], params) then
		return false, "guard_cannot_move";
	end
	return true, nil;
end;

-- Return the six in-map neighbours of a plot.  The Civ VI map API owns the
-- staggered-hex coordinate rules (including wrap seams), so do not recreate
-- them with x/y arithmetic here.  Keeping this as a board method also avoids
-- another file-scope local in the host's 200-register chunk.
CivvisBoard.adjacentPlots = function(x, y)
	local result, seen = {}, {};
	local names = {
		"DIRECTION_WEST", "DIRECTION_EAST", "DIRECTION_NORTHWEST",
		"DIRECTION_NORTHEAST", "DIRECTION_SOUTHWEST", "DIRECTION_SOUTHEAST",
	};
	for _, name in ipairs(names) do
		local direction = try(function() return DirectionTypes[name]; end, nil);
		local plot = direction ~= nil and try(function()
			return Map.GetAdjacentPlot(x, y, direction);
		end, nil) or nil;
		if plot ~= nil then
			local px = tonumber(try(function() return plot:GetX(); end, nil));
			local py = tonumber(try(function() return plot:GetY(); end, nil));
			if px ~= nil and py ~= nil then
				local key = px .. ":" .. py;
				if not seen[key] then
					seen[key] = true;
					result[#result + 1] = { x = px, y = py };
				end
			end
		end
	end
	return result;
end;

-- When a visible hostile already covers a Settler's CURRENT tile, merely
-- refusing its planned leg is not a safety action: the hostile phase still
-- captures the stationary civilian.  Find a destination within two hexes that
-- the host can actually execute this turn and that every supplied hostile cannot
-- reach under the same conservative host-side predicate.  Two hexes is the
-- ordinary Settler allowance; the host path query remains the authority, so a
-- blocked second step is simply ignored.  The caller owns the threat predicate
-- because scouts use their measured geometric floor, while combat units prefer
-- their path query and then a BaseMoves fallback.
CivvisBoard.findSettlerCaptureEscape = function(settler, fromX, fromY, wantX, wantY,
		threats, threatReaches)
	local candidates = {};
	local frontier = { { x = fromX, y = fromY } };
	local seen = { [fromX .. ":" .. fromY] = true };
	-- Keep this bounded. Enumerating the whole map through GetMoveToPathEx on
	-- every safety pass would make a rare emergency expensive, while two rings
	-- cover a normal Settler's full fresh-turn movement and the live failure that
	-- exposed this gap.
	for _ = 1, 2 do
		local nextFrontier = {};
		for _, origin in ipairs(frontier) do
			for _, plot in ipairs(CivvisBoard.adjacentPlots(origin.x, origin.y)) do
				local key = plot.x .. ":" .. plot.y;
				if not seen[key] then
					seen[key] = true;
					nextFrontier[#nextFrontier + 1] = plot;
					if CivvisBoard.reachesThisTurn(settler, plot.x, plot.y) then
						local safe = true;
						for _, threat in ipairs(threats) do
							local reaches = threatReaches(threat, plot.x, plot.y);
							if reaches then safe = false; break; end
						end
						if safe then
							local distance = tonumber(try(function()
								return Map.GetPlotDistance(plot.x, plot.y, wantX, wantY);
							end, 9999)) or 9999;
							candidates[#candidates + 1] = {
								x = plot.x, y = plot.y, distance = distance,
							};
						end
					end
				end
			end
		end
		frontier = nextFrontier;
	end
	table.sort(candidates, function(a, b)
		if a.distance ~= b.distance then return a.distance < b.distance; end
		if a.x ~= b.x then return a.x < b.x; end
		return a.y < b.y;
	end);
	return candidates[1];
end;

CivvisBoard.isCombatEscort = function(unit)
	return try(function()
		local definition = GameInfo.Units[unit:GetUnitType()];
		return definition ~= nil
			and ((tonumber(definition.Combat) or 0) > 0
				or (tonumber(definition.RangedCombat) or 0) > 0);
	end, false) == true;
end

-- A setter and its guard can be co-located while the planner exports only the
-- setter's MOVE_TO.  That happened on the failed opening: the planner's model
-- mirrored the stacked guard, whereas the host received no row for it and the
-- settler was captured next turn.  Reconcile a matching explicit row as before,
-- and additionally synthesize one host-only row when exactly one co-located
-- combat unit is otherwise unmentioned.  This is still bridge repair, not an
-- escort policy: ambiguous, separately ordered, or host-unreachable guards are
-- left alone and named in the ledger.
CivvisBoard.syncCappedSettlerEscorts = function(pid, turn, rows)
	if cfg.SettlerEscortCapSync == false or cfg.CapMovesToReach == false then return; end
	local first, firstRow, settling, setters, claimed = {}, {}, {}, {}, {};
	for index, row in ipairs(rows) do
		if tostring(row.kind or "") == "unit" then
			local subject = tonumber(row.subject);
			if subject ~= nil then
				if first[subject] == nil then
					first[subject], firstRow[subject] = index, row;
				end
				if tostring(row.verb or "") == "FOUND_CITY" then settling[subject] = true; end
			end
		end
	end
	-- Keep references to the original settler rows.  A synthesized guard is
	-- inserted before its setter, so raw indices move during this pass.
	for index, row in ipairs(rows) do
		local setterId = tonumber(row.subject);
		local wantX, wantY = tonumber(row.x), tonumber(row.y);
		if tostring(row.kind or "") == "unit" and tostring(row.verb or "") == "MOVE_TO"
				and setterId ~= nil and wantX ~= nil and wantY ~= nil
				and first[setterId] == index and not settling[setterId] then
			setters[#setters + 1] = { row = row, id = setterId, x = wantX, y = wantY };
		end
	end
	local owner = try(function() return Players[pid]; end, nil);
	local function unresolved(setterId, guardId, reason, wantX, wantY, sentX, sentY, candidates)
		CivvisBoard.stats.escort_cap_unresolved = CivvisBoard.stats.escort_cap_unresolved + 1;
		emit("escort_cap_unresolved", {
			turn = turn, settler = setterId, guard = guardId, reason = reason,
			candidates = candidates, want = { wantX, wantY }, sent = { sentX, sentY },
		});
	end
	for _, planned in ipairs(setters) do
		local setter = liveUnit(pid, planned.id);
		if setter ~= nil and unitTypeName(setter) == "UNIT_SETTLER" then
			local capped = CivvisBoard.capToTurn(setter, planned.x, planned.y);
			if capped ~= false then
				local sentX, sentY = planned.x, planned.y;
				if type(capped) == "table" then sentX, sentY = capped.x, capped.y; end
				local setterReaches = CivvisBoard.reachesThisTurn(setter, sentX, sentY);
				local sx = tonumber(try(function() return setter:GetX(); end, nil));
				local sy = tonumber(try(function() return setter:GetY(); end, nil));
				local candidates = {};
				if setterReaches and owner ~= nil and sx ~= nil and sy ~= nil then
					eachUnit(owner, function(candidate)
						local guardId = tonumber(try(function() return candidate:GetID(); end, nil));
						local gx = tonumber(try(function() return candidate:GetX(); end, nil));
						local gy = tonumber(try(function() return candidate:GetY(); end, nil));
						if guardId ~= nil and guardId ~= planned.id and not claimed[guardId]
								and gx == sx and gy == sy and CivvisBoard.isCombatEscort(candidate) then
							candidates[#candidates + 1] = { id = guardId, unit = candidate };
						end
					end);
				end
				if #candidates == 1 then
					local guardId, guard = candidates[1].id, candidates[1].unit;
					local guardRow = firstRow[guardId];
					local sameGoal = guardRow ~= nil and tostring(guardRow.verb or "") == "MOVE_TO"
						and tonumber(guardRow.x) == planned.x and tonumber(guardRow.y) == planned.y;
					if guardRow == nil or sameGoal then
						local reaches, why = CivvisBoard.reachesThisTurn(guard, sentX, sentY);
						if reaches then
							claimed[guardId] = true;
							CivvisBoard.escortHolds[guardId] = nil;
							if guardRow == nil then
								local insertAt = nil;
								for i, current in ipairs(rows) do
									if current == planned.row then insertAt = i; break; end
								end
								table.insert(rows, insertAt or (#rows + 1), {
									kind = "unit", subject = guardId, verb = "MOVE_TO", x = sentX, y = sentY,
									_civvis_escort_shadow = true,
								});
								CivvisBoard.stats.escort_shadow_injected = CivvisBoard.stats.escort_shadow_injected + 1;
								emit("escort_shadow_injected", {
									turn = turn, settler = planned.id, guard = guardId,
									want = { planned.x, planned.y }, sent = { sentX, sentY },
								});
							else
								guardRow.x, guardRow.y = sentX, sentY;
								CivvisBoard.stats.escort_cap_synced = CivvisBoard.stats.escort_cap_synced + 1;
								emit("escort_cap_synced", {
									turn = turn, settler = planned.id, guard = guardId,
									want = { planned.x, planned.y }, sent = { sentX, sentY },
								});
							end
						else
							-- A missing guard row must not fall through to explore
							-- automation while the settler leaves it behind.  Holding the
							-- soldier is the conservative bridge outcome when the host
							-- cannot prove it can make the same leg this turn.
							if guardRow == nil then
								CivvisBoard.escortHolds[guardId] = true;
								CivvisBoard.stats.escort_shadow_held = CivvisBoard.stats.escort_shadow_held + 1;
							end
							unresolved(planned.id, guardId, why, planned.x, planned.y, sentX, sentY, 1);
						end
					else
						unresolved(planned.id, guardId, "guard_has_order", planned.x, planned.y, sentX, sentY, 1);
					end
				elseif #candidates > 1 then
					unresolved(planned.id, nil, "ambiguous_guards", planned.x, planned.y,
						sentX, sentY, #candidates);
				end
			end
		end
	end
end;

-- A live host observed the new Rome settler leave its city for a tile beside a
-- visible barbarian scout, then disappear before the next state export
-- (civvis-20260826T153014Z, turn 10).  Do not turn that one observation into a
-- general claim about every scout's behaviour.  Instead, keep this bridge
-- When an unguarded settler's *actual host leg* would end within two plots of
-- a visible barbarian UNIT_SCOUT, hold the leg for this turn.  This applies to
-- every travel leg, not only its first departure from a city: the scout can
-- capture the civilian after any exposed step.  The two-plot floor is the
-- measured live capture reach, rather than a blanket freeze around every
-- visible scout.
--
-- The actual leg is read through `UnitManager.GetMoveToPathEx`, the same host
-- path query the shipped WorldInput.lua uses to draw a unit route
-- (Base/Assets/UI/WorldInput.lua:961).  A co-located combat unit that the host
-- has proved can share that leg remains a valid escort.  This is an actuation
-- floor, not a genome or route-selection policy: the planner still owns the
-- destination and can retry it after the immediate capture geometry clears.
CivvisBoard.holdVisibleScoutCaptureLegs = function(pid, turn, rows)
	local player = try(function() return Players[pid]; end, nil);
	if player == nil then return; end
	local visible = function(x, y)
		return try(function() return PlayersVisibility[pid]:IsVisible(x, y); end, false) == true;
	end
	local scouts = {};
	pcall(function()
		for _, otherId in ipairs(PlayerManager.GetAliveIDs() or {}) do
			if otherId ~= pid then
				local other = Players[otherId];
				local barbarian = other ~= nil
					and try(function() return other:IsBarbarian(); end, false) == true;
				if barbarian then
					eachUnit(other, function(unit)
						if unitTypeName(unit) ~= "UNIT_SCOUT" then return; end
						local x = tonumber(try(function() return unit:GetX(); end, nil));
						local y = tonumber(try(function() return unit:GetY(); end, nil));
						if x ~= nil and y ~= nil and visible(x, y) then
							scouts[#scouts + 1] = {
								id = tonumber(try(function() return unit:GetID(); end, nil)), x = x, y = y,
							};
						end
					end);
				end
			end
		end
	end);
	if #scouts == 0 then return; end

	-- `syncCappedSettlerEscorts` runs immediately before this floor.  It has
	-- already inserted or rewritten a guard row only when the host can prove
	-- that guard reaches the setter's exact leg.  Recheck that proof here for
	-- explicit rows too, rather than treating an intended guard route as cover.
	local function guardedLeg(settlerId, fromX, fromY, sentX, sentY)
		local guarded = false;
		eachUnit(player, function(candidate)
			if guarded or not CivvisBoard.isCombatEscort(candidate) then return; end
			local guardId = tonumber(try(function() return candidate:GetID(); end, nil));
			local guardX = tonumber(try(function() return candidate:GetX(); end, nil));
			local guardY = tonumber(try(function() return candidate:GetY(); end, nil));
			if guardId == nil or guardId == settlerId or guardX ~= fromX or guardY ~= fromY then return; end
			local ordered = false;
			for _, row in ipairs(rows) do
				if tostring(row.kind or "") == "unit" and tonumber(row.subject) == guardId
						and tostring(row.verb or "") == "MOVE_TO"
						and tonumber(row.x) == sentX and tonumber(row.y) == sentY then
					ordered = true;
					break;
				end
			end
			if ordered and CivvisBoard.reachesThisTurn(candidate, sentX, sentY) then
				guarded = true;
			end
		end);
		return guarded;
	end

	local held = {};
	for _, row in ipairs(rows) do
		local settlerId = tonumber(row.subject);
		local wantX, wantY = tonumber(row.x), tonumber(row.y);
		if tostring(row.kind or "") == "unit" and tostring(row.verb or "") == "MOVE_TO"
				and settlerId ~= nil and wantX ~= nil and wantY ~= nil and held[settlerId] == nil then
			local settler = liveUnit(pid, settlerId);
			if settler ~= nil and unitTypeName(settler) == "UNIT_SETTLER" then
				local fromX = tonumber(try(function() return settler:GetX(); end, nil));
				local fromY = tonumber(try(function() return settler:GetY(); end, nil));
				if fromX ~= nil and fromY ~= nil then
					local capped = CivvisBoard.capToTurn(settler, wantX, wantY);
					if capped ~= false then
						local sentX, sentY = wantX, wantY;
						if type(capped) == "table" then sentX, sentY = capped.x, capped.y; end
						if CivvisBoard.reachesThisTurn(settler, sentX, sentY)
								and not guardedLeg(settlerId, fromX, fromY, sentX, sentY) then
							for _, scout in ipairs(scouts) do
								local distance = tonumber(try(function()
									return Map.GetPlotDistance(sentX, sentY, scout.x, scout.y);
								end, -1)) or -1;
								if distance >= 0 and distance <= 2 then
									held[settlerId] = {
										settler = settlerId, fromX = fromX, fromY = fromY,
										wantX = wantX, wantY = wantY, sentX = sentX, sentY = sentY,
										scout = scout, row = row,
									};
									break;
								end
							end
						end
					end
				end
			end
		end
	end
	-- A hold is only useful when the Settler is safe where it is.  If the
	-- visible scout already covers the CURRENT tile, take a proven one-step
	-- retreat outside every visible scout's floor instead of waiting for the
	-- hostile phase to capture the stationary civilian.  This is a host-only
	-- actuation repair; the original planner target remains in the event so a
	-- subsequent turn can resume the intended route.
	for settlerId, heldLeg in pairs(held) do
		local function scoutReaches(scout, x, y)
			local distance = tonumber(try(function()
				return Map.GetPlotDistance(x, y, scout.x, scout.y);
			end, -1)) or -1;
			return distance >= 0 and distance <= 2;
		end
		local currentThreat = false;
		for _, scout in ipairs(scouts) do
			if scoutReaches(scout, heldLeg.fromX, heldLeg.fromY) then
				currentThreat = true;
				break;
			end
		end
		if currentThreat then
			local escape = CivvisBoard.findSettlerCaptureEscape(
				liveUnit(pid, settlerId), heldLeg.fromX, heldLeg.fromY,
				heldLeg.wantX, heldLeg.wantY, scouts, scoutReaches);
			if escape ~= nil then
				heldLeg.row.x, heldLeg.row.y = escape.x, escape.y;
				emit("settler_capture_escape", {
					turn = turn, settler = settlerId,
					from = { heldLeg.fromX, heldLeg.fromY },
					want = { heldLeg.wantX, heldLeg.wantY },
					sent = { escape.x, escape.y },
					threat_kind = "UNIT_SCOUT", threat = heldLeg.scout.id,
					threat_pos = { heldLeg.scout.x, heldLeg.scout.y },
				});
				held[settlerId] = nil;
			end
		end
	end
	-- A planner may send a follow-up MOVE_TO for the same settler.  Mark each
	-- one, so the queue cannot turn a held first leg into the same exposed walk.
	for settlerId, heldLeg in pairs(held) do
		CivvisBoard.stats.settler_scout_capture_held =
			CivvisBoard.stats.settler_scout_capture_held + 1;
		emit("settler_scout_capture_hold", {
			turn = turn, settler = settlerId,
			from = { heldLeg.fromX, heldLeg.fromY },
			want = { heldLeg.wantX, heldLeg.wantY }, sent = { heldLeg.sentX, heldLeg.sentY },
			scout = heldLeg.scout.id, scout_pos = { heldLeg.scout.x, heldLeg.scout.y },
		});
	end
	-- A planner may send a follow-up MOVE_TO for the same setter.  Mark each
	-- one, so the queue cannot turn a held first leg into the same exposed walk.
	for _, row in ipairs(rows) do
		if held[tonumber(row.subject)] ~= nil and tostring(row.kind or "") == "unit"
				and tostring(row.verb or "") == "MOVE_TO" then
			row._civvis_settler_scout_hold = true;
		end
	end
	-- Holding only the civilian can make the host create the exact capture it
	-- meant to prevent.  In live game `civvis-20260827T183146Z`, turn 47, a
	-- barbarian scout stood two plots from both a travelling Settler and its
	-- planned leg.  The bridge refused the Settler's leg, then accepted a
	-- co-located warrior's unrelated MOVE_TO.  The warrior walked away and the
	-- scout captured the lone Settler before the next export.  A shared combat
	-- unit blocks that capture, but only while it remains shared.
	--
	-- Retain each co-located combat unit when the named scout also covers the
	-- Settler's CURRENT tile.  A scout that threatens only the rejected
	-- destination does not justify freezing the guard on otherwise safe ground.
	-- Mark every departure verb, not just MOVE_TO: a melee attack or CAPTURE
	-- likewise leaves the civilian alone.  `escortHolds` additionally prevents
	-- the unmentioned-unit explore fallback from walking the guard away.
	for settlerId, heldLeg in pairs(held) do
		local settler = liveUnit(pid, settlerId);
		if settler ~= nil then
			local fromX = heldLeg.fromX;
			local fromY = heldLeg.fromY;
			local currentDistance = tonumber(try(function()
				return Map.GetPlotDistance(fromX, fromY, heldLeg.scout.x, heldLeg.scout.y);
			end, -1)) or -1;
			if fromX ~= nil and fromY ~= nil and currentDistance >= 0 and currentDistance <= 2 then
				eachUnit(player, function(candidate)
					if not CivvisBoard.isCombatEscort(candidate) then return; end
					local guardId = tonumber(try(function() return candidate:GetID(); end, nil));
					local guardX = tonumber(try(function() return candidate:GetX(); end, nil));
					local guardY = tonumber(try(function() return candidate:GetY(); end, nil));
					if guardId == nil or guardId == settlerId or guardX ~= fromX or guardY ~= fromY then return; end
					CivvisBoard.escortHolds[guardId] = true;
					CivvisBoard.stats.settler_scout_guard_held =
						CivvisBoard.stats.settler_scout_guard_held + 1;
					emit("settler_scout_guard_hold", {
						turn = turn, settler = settlerId, guard = guardId,
						at = { fromX, fromY }, scout = heldLeg.scout.id,
						scout_pos = { heldLeg.scout.x, heldLeg.scout.y },
					});
					for _, row in ipairs(rows) do
						local verb = tostring(row.verb or "");
						if tostring(row.kind or "") == "unit" and tonumber(row.subject) == guardId
								and (verb == "MOVE_TO" or verb == "ATTACK" or verb == "CAPTURE") then
							row._civvis_settler_scout_guard_hold = true;
						end
					end
				end);
			end
		end
	end
end;

-- A proven escort may make an exposed scout leg, but a lone travelling
-- settler cannot: the same capture geometry applies after it has left home.
-- That proof is not sufficient against an actual barbarian combat unit.  In
-- civvis-20260827T081925Z, turns 82 and 83, a barbarian musketeer and
-- man-at-arms each killed the synchronized single escort and captured the
-- settler in the same hostile turn.  The AI had ordered both units onto the
-- same tile, and `escort_cap_synced` recorded that the host sent both there;
-- the bridge must therefore not treat one shared guard as an exemption here.
--
-- Hold a leg whose *actual host destination* a visible non-scout barbarian
-- combat unit can reach on its next turn.  The first version only checked for
-- adjacency to the destination.  That was too narrow: a horse archer two
-- plots away (the host's staggered-hex coordinates made its one-turn leg read
-- as distance two) moved onto the newly exposed tile and captured the
-- Settler before the next export.  Use the host pathfinder when it can answer
-- the question, then a conservative base-move distance fallback when the
-- enemy's current turn has already spent its movement.  This uses only the
-- local player's ordinary visibility and applies while the Settler is
-- travelling as well as on its first city departure.  Its matching escort row
-- is held too, so refusing the civilian cannot leave the soldier walking into
-- the threat or being moved by the host.
CivvisBoard.holdVisibleBarbarianCombatCaptureLegs = function(pid, turn, rows)
	local player = try(function() return Players[pid]; end, nil);
	if player == nil then return; end
	local visible = function(x, y)
		return try(function() return PlayersVisibility[pid]:IsVisible(x, y); end, false) == true;
	end
	local threats = {};
	pcall(function()
		for _, otherId in ipairs(PlayerManager.GetAliveIDs() or {}) do
			if otherId ~= pid then
				local other = Players[otherId];
				local barbarian = other ~= nil
					and try(function() return other:IsBarbarian(); end, false) == true;
				if barbarian then
					eachUnit(other, function(unit)
						local name = unitTypeName(unit);
						if name == "UNIT_SCOUT" or not CivvisBoard.isCombatEscort(unit) then return; end
						local x = tonumber(try(function() return unit:GetX(); end, nil));
						local y = tonumber(try(function() return unit:GetY(); end, nil));
						if x ~= nil and y ~= nil and visible(x, y) then
							threats[#threats + 1] = {
								id = tonumber(try(function() return unit:GetID(); end, nil)),
								unit = unit,
								name = name, x = x, y = y,
							};
						end
					end);
				end
			end
		end
	end);
	if #threats == 0 then return; end

	-- Hostile units are observed after their own turn, so GetMovesRemaining()
	-- may be zero even though they receive a fresh allowance before the next
	-- capture attempt.  Prefer the host path's turn count (which includes that
	-- next-turn allowance); if the path is unavailable or refuses a route to a
	-- civilian-occupied plot, fall back to the unit definition's BaseMoves and
	-- the real hex distance.  The fallback is deliberately conservative: a
	-- false positive holds a Settler for one turn, while a false negative loses
	-- it before the next export.
	local function threatReaches(threat, x, y)
		local destination = try(function() return Map.GetPlotIndex(x, y); end, nil);
		local path = try(function()
			return UnitManager.GetMoveToPathEx(threat.unit, destination);
		end, nil);
		if destination ~= nil and path ~= nil and path.plots ~= nil and path.turns ~= nil then
			local n = 0;
			for _ in pairs(path.plots) do n = n + 1; end
			local last = tonumber(path.turns[n]);
			if n > 0 and path.plots[n] == destination and last ~= nil and last <= 1 then
				return true, "path";
			end
		end
		local baseMoves = tonumber(try(function()
			local definition = GameInfo.Units[threat.unit:GetUnitType()];
			return definition ~= nil and definition.BaseMoves;
		end, nil)) or 2;
		local distance = tonumber(try(function()
			return Map.GetPlotDistance(x, y, threat.x, threat.y);
		end, -1)) or -1;
		if distance >= 0 and distance <= baseMoves then
			return true, "base_moves";
		end
		return false, nil;
	end;

	-- A combat escort can be issued in an earlier replan frame than its
	-- Settler.  If the Settler is already standing inside a visible combat
	-- threat, letting that escort leave in the earlier frame exposes the
	-- civilian before the later Settler row can be held.  Remember which
	-- Settlers have a row in this frame: an exposed Settler with no row is
	-- waiting, so a co-located escort must stay; an exposed Settler with a row
	-- is handled below by the leg hold/escape decision.
	local exposedSettlers, currentSettlerRows = {}, {};
	for _, row in ipairs(rows) do
		if tostring(row.kind or "") == "unit" and tostring(row.verb or "") == "MOVE_TO" then
			local id = tonumber(row.subject);
			if id ~= nil then
				local unit = liveUnit(pid, id);
				if unit ~= nil and unitTypeName(unit) == "UNIT_SETTLER" then
					currentSettlerRows[id] = row;
				end
			end
		end
	end
	eachUnit(player, function(unit)
		if unitTypeName(unit) ~= "UNIT_SETTLER" then return; end
		local id = tonumber(try(function() return unit:GetID(); end, nil));
		local x = tonumber(try(function() return unit:GetX(); end, nil));
		local y = tonumber(try(function() return unit:GetY(); end, nil));
		if id == nil or x == nil or y == nil then return; end
		for _, threat in ipairs(threats) do
			local reaches = threatReaches(threat, x, y);
			if reaches then
				exposedSettlers[id] = { x = x, y = y, threat = threat };
				break;
			end
		end
	end);

	local held = {};
	for _, row in ipairs(rows) do
		local settlerId = tonumber(row.subject);
		local wantX, wantY = tonumber(row.x), tonumber(row.y);
		if tostring(row.kind or "") == "unit" and tostring(row.verb or "") == "MOVE_TO"
				and row._civvis_settler_scout_hold ~= true
				and settlerId ~= nil and wantX ~= nil and wantY ~= nil and held[settlerId] == nil then
			local settler = liveUnit(pid, settlerId);
			if settler ~= nil and unitTypeName(settler) == "UNIT_SETTLER" then
				local fromX = tonumber(try(function() return settler:GetX(); end, nil));
				local fromY = tonumber(try(function() return settler:GetY(); end, nil));
				local capped = CivvisBoard.capToTurn(settler, wantX, wantY);
				if fromX ~= nil and fromY ~= nil and capped ~= false then
					local sentX, sentY = wantX, wantY;
					if type(capped) == "table" then sentX, sentY = capped.x, capped.y; end
					if CivvisBoard.reachesThisTurn(settler, sentX, sentY) then
						for _, threat in ipairs(threats) do
							local reaches, reachKind = threatReaches(threat, sentX, sentY);
							if reaches then
								held[settlerId] = {
									settler = settlerId, fromX = fromX, fromY = fromY,
									wantX = wantX, wantY = wantY,
									sentX = sentX, sentY = sentY, threat = threat,
									reachKind = reachKind,
									row = row,
								};
								break;
							end
						end
					end
				end
			end
		end
	end
	-- Refusing a leg does not save a Settler that is already standing in the
	-- hostile's envelope.  Before falling back to a hold, use the host's own
	-- one-step path answers to retreat outside every visible combat threat.
	for settlerId, heldLeg in pairs(held) do
		local currentThreat = false;
		for _, threat in ipairs(threats) do
			if threatReaches(threat, heldLeg.fromX, heldLeg.fromY) then
				currentThreat = true;
				break;
			end
		end
		if currentThreat then
			local escape = CivvisBoard.findSettlerCaptureEscape(
				liveUnit(pid, settlerId), heldLeg.fromX, heldLeg.fromY,
				heldLeg.wantX, heldLeg.wantY, threats, threatReaches);
			if escape ~= nil then
				heldLeg.row.x, heldLeg.row.y = escape.x, escape.y;
				emit("settler_capture_escape", {
					turn = turn, settler = settlerId,
					from = { heldLeg.fromX, heldLeg.fromY },
					want = { heldLeg.wantX, heldLeg.wantY },
					sent = { escape.x, escape.y },
					reach = heldLeg.reachKind,
					threat_kind = heldLeg.threat.name, threat = heldLeg.threat.id,
					threat_pos = { heldLeg.threat.x, heldLeg.threat.y },
				});
				held[settlerId] = nil;
			end
		end
	end
	-- Keep a co-located combat escort from leaving an exposed Settler in an
	-- earlier frame.  The Settler and escort can be queued in different frames
	-- (for example, the escort at frame 1 and the Settler's held retreat at
	-- frame 2), so the normal matching-row check below cannot see both at once.
	-- Only a MOVE_TO is shadow-held: an attack or other combat action may clear
	-- the threat, while an unrelated move would abandon the civilian before the
	-- next host export.  A Settler with a same-frame row is held here only when
	-- that row remains in `held`; a proven safe escape does not freeze its escort.
	for settlerId, exposed in pairs(exposedSettlers) do
		if currentSettlerRows[settlerId] == nil or held[settlerId] ~= nil then
			eachUnit(player, function(candidate)
				local guardId = tonumber(try(function() return candidate:GetID(); end, nil));
				local guardX = tonumber(try(function() return candidate:GetX(); end, nil));
				local guardY = tonumber(try(function() return candidate:GetY(); end, nil));
				if guardId == nil or guardId == settlerId or guardX ~= exposed.x
						or guardY ~= exposed.y or not CivvisBoard.isCombatEscort(candidate) then
					return;
				end
				for _, row in ipairs(rows) do
					if tostring(row.kind or "") == "unit" and tostring(row.verb or "") == "MOVE_TO"
							and tonumber(row.subject) == guardId
							and row._civvis_settler_barbarian_combat_hold ~= true then
						row._civvis_settler_barbarian_combat_hold = true;
						CivvisBoard.escortHolds[guardId] = true;
						CivvisBoard.stats.settler_barbarian_combat_guard_held =
							CivvisBoard.stats.settler_barbarian_combat_guard_held + 1;
						emit("settler_barbarian_combat_guard_hold", {
							turn = turn, settler = settlerId, guard = guardId,
							at = { exposed.x, exposed.y }, hostile = exposed.threat.id,
							hostile_type = exposed.threat.name,
							hostile_pos = { exposed.threat.x, exposed.threat.y },
						});
						break;
					end
				end
			end);
		end
	end
	for settlerId, heldLeg in pairs(held) do
		local reachKind = heldLeg.reachKind;
		CivvisBoard.stats.settler_barbarian_combat_capture_held =
			CivvisBoard.stats.settler_barbarian_combat_capture_held + 1;
		emit("settler_barbarian_combat_capture_hold", {
			turn = turn, settler = settlerId,
			from = { heldLeg.fromX, heldLeg.fromY },
			want = { heldLeg.wantX, heldLeg.wantY }, sent = { heldLeg.sentX, heldLeg.sentY },
			hostile = heldLeg.threat.id, hostile_type = heldLeg.threat.name,
			hostile_pos = { heldLeg.threat.x, heldLeg.threat.y },
			hostile_reach = reachKind,
		});
	end
	if next(held) == nil then return; end

	-- Refuse all follow-up moves for the held settler, then any co-located
	-- combat row that the synchronization pass proved would share this exact
	-- leg.  The latter includes host-only shadow rows.
	for _, row in ipairs(rows) do
		local heldLeg = held[tonumber(row.subject)];
		if heldLeg ~= nil and tostring(row.kind or "") == "unit"
				and tostring(row.verb or "") == "MOVE_TO" then
			row._civvis_settler_barbarian_combat_hold = true;
		end
	end
	for _, heldLeg in pairs(held) do
		for _, row in ipairs(rows) do
			local guardId = tonumber(row.subject);
			if tostring(row.kind or "") == "unit" and tostring(row.verb or "") == "MOVE_TO"
					and guardId ~= nil and guardId ~= heldLeg.settler
					and tonumber(row.x) == heldLeg.sentX and tonumber(row.y) == heldLeg.sentY then
				local guard = liveUnit(pid, guardId);
				local guardX = tonumber(try(function() return guard:GetX(); end, nil));
				local guardY = tonumber(try(function() return guard:GetY(); end, nil));
				if guard ~= nil and guardX == heldLeg.fromX and guardY == heldLeg.fromY
						and CivvisBoard.isCombatEscort(guard)
						and CivvisBoard.reachesThisTurn(guard, heldLeg.sentX, heldLeg.sentY) then
					row._civvis_settler_barbarian_combat_hold = true;
					CivvisBoard.escortHolds[guardId] = true;
				end
			end
		end
	end

	-- Refusing the civilian's leg is not enough when a combat escort is nearby
	-- but not already on its tile.  In the live loss at t73 the warrior was one
	-- hex away and had an unrelated MOVE_TO, so the settler stayed exposed after
	-- the hold and the hostile captured it before the next export.  Rescue only
	-- the narrow, host-grounded case: the visible combat threat is adjacent to
	-- the settler's CURRENT tile, exactly one unmentioned combat unit is within
	-- two hexes and can reach that tile this turn, and no existing order has to
	-- be overwritten.  The synthetic row is a shadow actuation, so it does not
	-- change CIVVIS's decision counts; it merely co-locates the guard before the
	-- held settler's row is processed.
	local mentioned = {};
	for _, row in ipairs(rows) do
		if tostring(row.kind or "") == "unit" then
			local id = tonumber(row.subject);
			if id ~= nil then mentioned[id] = true; end
		end
	end
	for _, heldLeg in pairs(held) do
		local currentDistance = tonumber(try(function()
			return Map.GetPlotDistance(heldLeg.fromX, heldLeg.fromY,
				heldLeg.threat.x, heldLeg.threat.y);
		end, -1)) or -1;
		if currentDistance >= 0 and currentDistance <= 1 then
			local candidates = {};
			eachUnit(player, function(candidate)
				local guardId = tonumber(try(function() return candidate:GetID(); end, nil));
				local guardX = tonumber(try(function() return candidate:GetX(); end, nil));
				local guardY = tonumber(try(function() return candidate:GetY(); end, nil));
				if guardId == nil or guardId == heldLeg.settler or mentioned[guardId]
						or CivvisBoard.escortHolds[guardId]
						or not CivvisBoard.isCombatEscort(candidate)
						or guardX == nil or guardY == nil then return; end
				local distance = tonumber(try(function()
					return Map.GetPlotDistance(guardX, guardY, heldLeg.fromX, heldLeg.fromY);
				end, -1)) or -1;
				if distance < 0 or distance > 2 then return; end
				local reaches = CivvisBoard.reachesThisTurn(candidate, heldLeg.fromX, heldLeg.fromY);
				if reaches then candidates[#candidates + 1] = {
					id = guardId, unit = candidate,
				}; end
			end);
			if #candidates == 1 then
				local guardId = candidates[1].id;
				local insertAt = nil;
				for i, row in ipairs(rows) do
					if row == heldLeg.row then insertAt = i; break; end
				end
				table.insert(rows, insertAt or (#rows + 1), {
					kind = "unit", subject = guardId, verb = "MOVE_TO",
					x = heldLeg.fromX, y = heldLeg.fromY,
					_civvis_escort_shadow = true,
				});
				mentioned[guardId] = true;
				CivvisBoard.escortHolds[guardId] = true;
				CivvisBoard.stats.escort_shadow_injected =
					CivvisBoard.stats.escort_shadow_injected + 1;
				CivvisBoard.stats.settler_barbarian_combat_guard_rescued =
					CivvisBoard.stats.settler_barbarian_combat_guard_rescued + 1;
				emit("settler_barbarian_combat_guard_rescue", {
					turn = turn, settler = heldLeg.settler, guard = guardId,
					at = { heldLeg.fromX, heldLeg.fromY },
					hostile = heldLeg.threat.id,
					hostile_pos = { heldLeg.threat.x, heldLeg.threat.y },
				});
			end
		end
	end
end;

-- Builders are civilians too.  One live Civ 6 run lost a Builder immediately
-- after CIVVIS moved it onto `FEATURE_BURNING_FOREST`; there was no combat or
-- capture callback, and the tile became `FEATURE_BURNT_FOREST` two turns later.
-- The normal civilian bridge only protected Settlers, so the Builder
-- disappeared before the next export.  Keep this repair narrow: inspect only
-- visible barbarian units, only a host-proven same-turn Builder leg, and only
-- refuse the hand-off when no co-located combat escort is proven to take that
-- exact leg.  The planner still owns the destination and the existing opt-in
-- advanced Builder gene remains independent of this actuation floor.
CivvisBoard.holdVisibleBuilderCaptureLegs = function(pid, turn, rows)
	local player = try(function() return Players[pid]; end, nil);
	if player == nil then return; end
	local visible = function(x, y)
		return try(function() return PlayersVisibility[pid]:IsVisible(x, y); end, false) == true;
	end
	local threats = {};
	pcall(function()
		for _, otherId in ipairs(PlayerManager.GetAliveIDs() or {}) do
			if otherId ~= pid then
				local other = Players[otherId];
				local barbarian = other ~= nil
					and try(function() return other:IsBarbarian(); end, false) == true;
				if barbarian then
					eachUnit(other, function(unit)
						local name = unitTypeName(unit);
						-- Barb scouts have a measured two-plot civilian-capture
						-- floor; other combat units use the host path below.
						if name ~= "UNIT_SCOUT" and not CivvisBoard.isCombatEscort(unit) then return; end
						local x = tonumber(try(function() return unit:GetX(); end, nil));
						local y = tonumber(try(function() return unit:GetY(); end, nil));
						if x ~= nil and y ~= nil and visible(x, y) then
							threats[#threats + 1] = {
								id = tonumber(try(function() return unit:GetID(); end, nil)),
								unit = unit, name = name, x = x, y = y,
								scout = name == "UNIT_SCOUT",
							};
						end
					end);
				end
			end
		end
	end);
	if #threats == 0 then return; end

	local function threatReaches(threat, x, y)
		if threat.scout then
			local distance = tonumber(try(function()
				return Map.GetPlotDistance(x, y, threat.x, threat.y);
			end, -1)) or -1;
			return distance >= 0 and distance <= 2, "scout_distance";
		end
		local destination = try(function() return Map.GetPlotIndex(x, y); end, nil);
		local path = try(function()
			return UnitManager.GetMoveToPathEx(threat.unit, destination);
		end, nil);
		if destination ~= nil and path ~= nil and path.plots ~= nil and path.turns ~= nil then
			local n = 0;
			for _ in pairs(path.plots) do n = n + 1; end
			local last = tonumber(path.turns[n]);
			if n > 0 and path.plots[n] == destination and last ~= nil and last <= 1 then
				return true, "path";
			end
		end
		local baseMoves = tonumber(try(function()
			local definition = GameInfo.Units[threat.unit:GetUnitType()];
			return definition ~= nil and definition.BaseMoves;
		end, nil)) or 2;
		local distance = tonumber(try(function()
			return Map.GetPlotDistance(x, y, threat.x, threat.y);
		end, -1)) or -1;
		return distance >= 0 and distance <= baseMoves, "base_moves";
	end

	local function onOwnCity(x, y)
		local found = false;
		eachCity(player, function(city)
			if found then return; end
			local cx = tonumber(try(function() return city:GetX(); end, nil));
			local cy = tonumber(try(function() return city:GetY(); end, nil));
			if cx == x and cy == y then found = true; end
		end);
		return found;
	end

	-- `reachesThisTurn` is the strongest proof, but Civ VI can return no path
	-- for a civilian destination occupied by one of our own units even though
	-- the subsequent MOVE_TO request is accepted.  The live t32 Builder loss
	-- had exactly that shape: the Builder, Warrior, and Settler shared the
	-- origin/destination envelope, the Slinger was visible, and the host moved
	-- the Builder after the path probe returned unknown.  Once the requested
	-- leg is within the Builder's geometric movement allowance, use that
	-- conservative fallback.  A false positive holds one turn; a false
	-- negative loses the civilian before the next export.
	local function builderReachesThisTurn(builder, fromX, fromY, x, y)
		local reaches, why = CivvisBoard.reachesThisTurn(builder, x, y);
		if reaches then return true, "path"; end
		local baseMoves = tonumber(try(function()
			local definition = GameInfo.Units[builder:GetUnitType()];
			return definition ~= nil and definition.BaseMoves;
		end, nil)) or 2;
		local distance = tonumber(try(function()
			return Map.GetPlotDistance(fromX, fromY, x, y);
		end, -1)) or -1;
		if distance >= 0 and distance <= baseMoves then
			return true, "distance_fallback";
		end
		return false, why;
	end

	-- A Builder can travel with a combat unit.  Preserve that explicit escort
	-- contract, but require the host to prove both the co-location and the
	-- exact same-turn destination before allowing the exposed civilian leg.
	local function guardedLeg(builderId, fromX, fromY, sentX, sentY)
		local guarded = false;
		eachUnit(player, function(candidate)
			if guarded or not CivvisBoard.isCombatEscort(candidate) then return; end
			local guardId = tonumber(try(function() return candidate:GetID(); end, nil));
			local guardX = tonumber(try(function() return candidate:GetX(); end, nil));
			local guardY = tonumber(try(function() return candidate:GetY(); end, nil));
			if guardId == nil or guardId == builderId or guardX ~= fromX or guardY ~= fromY then return; end
			for _, row in ipairs(rows) do
				if tostring(row.kind or "") == "unit" and tonumber(row.subject) == guardId
						and tostring(row.verb or "") == "MOVE_TO"
						and tonumber(row.x) == sentX and tonumber(row.y) == sentY
						and CivvisBoard.reachesThisTurn(candidate, sentX, sentY) then
					guarded = true;
					break;
				end
			end
		end);
		return guarded;
	end

	local held = {};
	for _, row in ipairs(rows) do
		local builderId = tonumber(row.subject);
		local wantX, wantY = tonumber(row.x), tonumber(row.y);
		if tostring(row.kind or "") == "unit" and tostring(row.verb or "") == "MOVE_TO"
				and row._civvis_builder_barbarian_capture_hold ~= true
				and builderId ~= nil and wantX ~= nil and wantY ~= nil and held[builderId] == nil then
			local builder = liveUnit(pid, builderId);
			if builder ~= nil and unitTypeName(builder) == "UNIT_BUILDER" then
				local fromX = tonumber(try(function() return builder:GetX(); end, nil));
				local fromY = tonumber(try(function() return builder:GetY(); end, nil));
				local capped = CivvisBoard.capToTurn(builder, wantX, wantY);
				if fromX ~= nil and fromY ~= nil and capped ~= false then
					local sentX, sentY = wantX, wantY;
					if type(capped) == "table" then sentX, sentY = capped.x, capped.y; end
					local builderReaches, builderReachKind = builderReachesThisTurn(
						builder, fromX, fromY, sentX, sentY);
					if (sentX ~= fromX or sentY ~= fromY)
							and builderReaches
							and not guardedLeg(builderId, fromX, fromY, sentX, sentY) then
						for _, threat in ipairs(threats) do
							local reaches, reachKind = threatReaches(threat, sentX, sentY);
							if reaches then
								held[builderId] = {
									builder = builderId, fromX = fromX, fromY = fromY,
									wantX = wantX, wantY = wantY,
									sentX = sentX, sentY = sentY, threat = threat,
									reachKind = reachKind, builderReachKind = builderReachKind, row = row,
								};
								break;
							end
						end
					end
				end
			end
		end
	end

	-- If an already travelling Builder is inside the visible envelope, try the
	-- same bounded host-proven escape used by Settlers.  A Builder in one of our
	-- own city centers is treated as a safe origin: moving it out of the city is
	-- not required to answer a threatened destination, and the city itself is a
	-- separate defense problem.
	for builderId, heldLeg in pairs(held) do
		if not onOwnCity(heldLeg.fromX, heldLeg.fromY) then
			local currentThreat = false;
			for _, threat in ipairs(threats) do
				if threatReaches(threat, heldLeg.fromX, heldLeg.fromY) then
					currentThreat = true;
					break;
				end
			end
			if currentThreat then
				local escape = CivvisBoard.findSettlerCaptureEscape(
					liveUnit(pid, builderId), heldLeg.fromX, heldLeg.fromY,
					heldLeg.wantX, heldLeg.wantY, threats, threatReaches);
				if escape ~= nil then
					heldLeg.row.x, heldLeg.row.y = escape.x, escape.y;
					CivvisBoard.stats.builder_capture_escaped =
						CivvisBoard.stats.builder_capture_escaped + 1;
					emit("builder_capture_escape", {
						turn = turn, builder = builderId,
						from = { heldLeg.fromX, heldLeg.fromY },
						want = { heldLeg.wantX, heldLeg.wantY },
						sent = { escape.x, escape.y },
						reach = heldLeg.reachKind,
						threat_kind = heldLeg.threat.name, threat = heldLeg.threat.id,
						threat_pos = { heldLeg.threat.x, heldLeg.threat.y },
					});
					held[builderId] = nil;
				end
			end
		end
	end
	-- A Builder can receive a follow-up MOVE_TO in a later replan frame.  Mark
	-- every move for a held Builder, not only the first row that established the
	-- threat, so a second frame cannot reintroduce the same exposed leg.
	for builderId in pairs(held) do
		for _, row in ipairs(rows) do
			if tostring(row.kind or "") == "unit" and tostring(row.verb or "") == "MOVE_TO"
					and tonumber(row.subject) == builderId then
				row._civvis_builder_barbarian_capture_hold = true;
			end
		end
	end
	for builderId, heldLeg in pairs(held) do
		CivvisBoard.stats.builder_barbarian_capture_held =
			CivvisBoard.stats.builder_barbarian_capture_held + 1;
		emit("builder_barbarian_capture_hold", {
			turn = turn, builder = builderId,
			from = { heldLeg.fromX, heldLeg.fromY },
			want = { heldLeg.wantX, heldLeg.wantY },
			sent = { heldLeg.sentX, heldLeg.sentY },
			hostile = heldLeg.threat.id, hostile_type = heldLeg.threat.name,
			hostile_pos = { heldLeg.threat.x, heldLeg.threat.y },
			hostile_reach = heldLeg.reachKind,
			builder_reach = heldLeg.builderReachKind,
		});
	end
end;

-- Gathering Storm's forest-fire damage table is more dangerous than the
-- feature's ordinary movement/defense row suggests: turns 0-2 carry a 101%
-- `UNIT_KILLED_CIVILIAN` damage row for both forest and jungle fires.  The
-- board already exports the host's authoritative feature name, but a CIVVIS
-- MOVE_TO can be applied between exports.  Refuse a civilian's exposed handoff
-- into an active fire tile at the last host-leg boundary.  This is deliberately
-- host-only: the feature may start or spread after the model's last observation,
-- and no escort makes a civilian immune to a random-event kill.
CivvisBoard.holdActiveFireCivilianLegs = function(pid, turn, rows)
	local function activeFireAt(x, y)
		local plot = try(function() return Map.GetPlot(x, y); end, nil);
		if plot == nil then return nil; end
		local feature = typeName("Features", "FeatureType",
			try(function() return plot:GetFeatureType(); end, -1));
		if feature == "FEATURE_BURNING_FOREST" or feature == "FEATURE_BURNING_JUNGLE" then
			return feature;
		end
		return nil;
	end
	local function civilian(unit)
		local row = try(function() return GameInfo.Units[unitTypeName(unit)]; end, nil);
		if row == nil then return false; end
		return (tonumber(row.Combat) or 0) <= 0
			and (tonumber(row.RangedCombat) or 0) <= 0
			and (tonumber(row.Bombard) or 0) <= 0
			and (tonumber(row.AntiAirCombat) or 0) <= 0;
	end
	local function alreadyHeld(row)
		return row._civvis_builder_barbarian_capture_hold == true
			or row._civvis_settler_barbarian_combat_hold == true
			or row._civvis_settler_scout_hold == true;
	end
	local held = {};
	for _, row in ipairs(rows) do
		local subject = tonumber(row.subject);
		local wantX, wantY = tonumber(row.x), tonumber(row.y);
		if tostring(row.kind or "") == "unit" and tostring(row.verb or "") == "MOVE_TO"
				and not alreadyHeld(row) and subject ~= nil
				and wantX ~= nil and wantY ~= nil and held[subject] == nil then
			local unit = liveUnit(pid, subject);
			if unit ~= nil and civilian(unit) then
				local fromX = tonumber(try(function() return unit:GetX(); end, nil));
				local fromY = tonumber(try(function() return unit:GetY(); end, nil));
				local capped = CivvisBoard.capToTurn(unit, wantX, wantY);
				if fromX ~= nil and fromY ~= nil and capped ~= false then
					local sentX, sentY = wantX, wantY;
					if type(capped) == "table" then sentX, sentY = capped.x, capped.y; end
					local feature = activeFireAt(sentX, sentY);
					if feature ~= nil and (sentX ~= fromX or sentY ~= fromY) then
						held[subject] = {
							unit = subject, unit_kind = unitTypeName(unit),
							fromX = fromX, fromY = fromY,
							wantX = wantX, wantY = wantY,
							sentX = sentX, sentY = sentY, feature = feature,
						};
					end
				end
			end
		end
	end
	-- A later replan frame must not put the same civilian back onto the fire
	-- after the first row was held.  Keep the original decision visible in the
	-- order ledger; only the host hand-off is declined.
	for subject in pairs(held) do
		for _, row in ipairs(rows) do
			if tostring(row.kind or "") == "unit" and tostring(row.verb or "") == "MOVE_TO"
					and tonumber(row.subject) == subject then
				row._civvis_active_fire_civilian_hold = true;
			end
		end
	end
	for subject, heldLeg in pairs(held) do
		CivvisBoard.stats.active_fire_civilian_held =
			CivvisBoard.stats.active_fire_civilian_held + 1;
		emit("active_fire_civilian_hold", {
			turn = turn, unit = subject, unit_kind = heldLeg.unit_kind,
			from = { heldLeg.fromX, heldLeg.fromY },
			want = { heldLeg.wantX, heldLeg.wantY },
			sent = { heldLeg.sentX, heldLeg.sentY }, feature = heldLeg.feature,
		});
	end
end;

-- Cancel stale host movement on combat units before the board owns them.
--
-- `GetQueuedDestination` catches an ordinary multi-turn MOVE_TO, but it is
-- deliberately nil while a stale host operation is active: the unit exports
-- as `activity = "operation"` instead.  That left a second driver on
-- a unit CIVVIS had just planned.  In run civvis-20260831T195447Z the Slinger
-- was sent to (38,14) on replan frame 1, the operation instead walked it to
-- (38,11), and a later frame had no movement left to leave a Spearman plus
-- Quadrireme envelope.  `UNITCOMMAND_CANCEL` is the host's in-place cancel
-- command (UnitCommands.xml:36); use it for either stale shape.
--
-- `only` narrows a mid-turn reconciliation to units CIVVIS actually named.
-- Start-of-turn callers omit it so a leftover operation can never walk a
-- tactical unit before the next board export.
CivvisBoard.cancelQueuedPaths = function(player, pid, turn, only)
	local found, cancelled, activeOperations = 0, 0, 0;
	eachUnit(player, function(unit)
		local id = try(function() return unit:GetID(); end, -1);
		if only ~= nil and not only[id] then return; end
		local queued = try(function() return UnitManager.GetQueuedDestination(unit); end, nil);
		if queued ~= nil then found = found + 1; end
		local combat = try(function()
			local row = GameInfo.Units[unit:GetUnitType()];
			return row ~= nil and ((row.Combat or 0) > 0 or (row.RangedCombat or 0) > 0);
		end, false) == true;
		if not combat then return; end
		local activeOperation = try(function()
			return ActivityTypes ~= nil
				and ActivityTypes.ACTIVITY_OPERATION ~= nil
				and UnitManager.GetActivityType(unit) == ActivityTypes.ACTIVITY_OPERATION;
		end, false) == true;
		if queued == nil and not activeOperation then return; end
		if queued == nil then found = found + 1; end
		if activeOperation then activeOperations = activeOperations + 1; end
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
		emit("queued_paths", {
			turn = turn, found = found, cancelled = cancelled,
			active_operations = activeOperations,
		});
	end
end;

-- ------------------------------------------------ mid-turn frames (combat + replan)
--
-- ★★★★ THE PLAN IS COMPUTED ONCE, BEFORE THE HOST HAS ROLLED A SINGLE DIE.
-- Every strike of the turn is planned against the opening board with the
-- engine's own rolls; the host's roll differs (it has left "sure" kills alive
-- at 1, 3, 6, 8, 16 and 20 HP), and the next export is next turn. A combat
-- frame closes that gap: after the opening orders and their per-unit queue
-- have settled, if any strike was issued, the board is exported again with
-- `frame = N`, the brain re-plans the SAME turn on it (units that acted show
-- the movement and attacks they have left, targets show the damage they
-- took), and the answer is applied like the opening one.
--
-- ★★★★ AND THE BOARD WAS COMPUTED ONCE, BEFORE ANY UNIT HAD LOOKED. The same
-- shape holds for ground: a scout ordered three hexes into the fog reveals the
-- coast, the rival border or the barbarian camp on its first step and walks
-- the other two regardless, and the map it uncovered reached the brain only
-- with the next `tiles` sweep — every `TileExportEvery` turns. A REPLAN frame
-- (`ReplanFrames = N`, the cap per turn) opens whenever the settled board has
-- something new to say and somebody left to say it to: plots were revealed
-- since the board went out (`CivvisTiles.sweep`, which also sends them as a
-- `tiles` delta so the re-plan SEES them) and at least one unit still has
-- movement, or a strike went out.
--
-- ⚠ `CombatFrames` keeps its old meaning (strike-opened frames only, default
-- 0). `ReplanFrames` opens on strikes OR revealed ground. Each frame waits
-- with its own short budget (`CombatFramePolls`) and no fallback ladder:
-- past it the turn's remaining frames are abandoned by name and the turn
-- ends as it always did. Every frame re-arms the per-unit queue's tick
-- budget, so a turn with N frames may hold up to (N+1) x OrderQueueMaxTicks.
-- One bare global table (200-local ceiling).
CivvisFrames = { current = 0, strikes = 0, revealed = 0, movers = 0, reason = nil, settled = false };

CivvisFrames.reset = function()
	CivvisFrames.current = 0;
	CivvisFrames.strikes = 0;
	CivvisFrames.revealed = 0;
	CivvisFrames.movers = 0;
	CivvisFrames.reason = nil;
	-- True once the turn declined its next frame: `settleTurn` is called
	-- again on every later tick of the turn (blockers, end-turn retries),
	-- and the sweep must not run on each of them.
	CivvisFrames.settled = false;
end;

-- Called from CivvisLedger.strike for every strike issued, opening or queued.
CivvisFrames.noteStrike = function()
	CivvisFrames.strikes = CivvisFrames.strikes + 1;
end;

CivvisFrames.combatMax = function()
	return tonumber(cfg.CombatFrames) or 0;
end;

CivvisFrames.replanMax = function()
	return tonumber(cfg.ReplanFrames) or 0;
end;

CivvisFrames.max = function()
	return math.max(CivvisFrames.combatMax(), CivvisFrames.replanMax());
end;

-- Look at the settled board once, before asking `wanted`: how many plots this
-- seat revealed since the last board went out (sent as a `tiles` delta in the
-- same breath, so the frame's re-plan has them), and how many units could
-- still act on them. Pure bookkeeping when frames are off.
CivvisFrames.observe = function(player, pid, turn)
	if CivvisFrames.replanMax() <= 0 then return; end
	CivvisFrames.revealed = CivvisTiles.sweep(player, pid, turn, CivvisFrames.current + 1) or 0;
	local movers = 0;
	eachUnit(player, function(unit)
		local moves = try(function() return unit:GetMovesRemaining(); end, 0) or 0;
		if moves > 0 then movers = movers + 1; end
	end);
	CivvisFrames.movers = movers;
end;

-- Why another frame should open now, or nil: frames are enabled, the cap is
-- not reached, and either a strike was issued since the last board went out
-- (combat or replan frames) or ground was revealed with movement left to
-- spend on it (replan frames only).
CivvisFrames.why = function()
	local current = CivvisFrames.current;
	if CivvisFrames.strikes > 0 and current < CivvisFrames.max() then
		return "strike";
	end
	if current < CivvisFrames.replanMax()
			and CivvisFrames.revealed > 0 and CivvisFrames.movers > 0 then
		return "revealed";
	end
	return nil;
end;

CivvisFrames.wanted = function()
	return CivvisFrames.why() ~= nil;
end;

-- Open the next frame: export the board again, stamped, and re-arm the
-- handshake so `settleTurn` waits for this frame's answer.
CivvisFrames.begin = function(player, pid, turn)
	local reason = CivvisFrames.why() or "strike";
	CivvisFrames.current = CivvisFrames.current + 1;
	CivvisFrames.reason = reason;
	CivvisFrames.settled = false;
	local strikes, revealed = CivvisFrames.strikes, CivvisFrames.revealed;
	CivvisFrames.strikes = 0;
	CivvisFrames.revealed = 0;
	awaiting.frame = CivvisFrames.current;
	awaiting.done = false;
	awaiting.polls = 0;
	awaiting.ticks = 0;
	awaiting.source = "pending";
	-- Each frame's follow-ups get their own queue budget; see the header.
	CivvisQueue.ticks = 0;
	-- `combat_frame` keeps its name for the readers that count it; a frame
	-- opened by revealed ground is a `replan_frame`. Both carry the reason.
	emit(reason == "strike" and "combat_frame" or "replan_frame", {
		turn = turn, frame = CivvisFrames.current, reason = reason,
		strikes = strikes, revealed = revealed, movers = CivvisFrames.movers,
	});
	pcall(function() exportState(player, pid, turn, CivvisFrames.current); end);
end;

local function applyOrders(player, pid, turn, rows)
	local applied, refused, deferred, verdicts = 0, 0, 0, 0;
	local byKind, whyNot = {}, {};
	-- Per kind, beside the per-turn totals: how many orders of each kind were
	-- counted, and each kind's refusal reasons. `by` is applied-only and
	-- `refusals` is reason-only, so "which kind is being refused, and why" could
	-- not be read off the event; `civ6_ladder.orders_by_kind` sums these into
	-- `summary.orders` and `tools/live_actuation.py` floors them. `seen_by`
	-- counts accepted `produce_next` leases too (they are deferred out of `seen`),
	-- so its sum is `seen + deferred`.
	local seenByKind, refusedByKind = {}, {};
	local function countRefusal(kind, why)
		refused = refused + 1;
		whyNot[why] = (whyNot[why] or 0) + 1;
		seenByKind[kind] = (seenByKind[kind] or 0) + 1;
		local perKind = refusedByKind[kind];
		if perKind == nil then perKind = {}; refusedByKind[kind] = perKind; end
		perKind[why] = (perKind[why] or 0) + 1;
	end
	-- The board may answer an intra-turn replan while a stale host operation is
	-- still active. Preempt it before applying an order for the same combat
	-- unit; otherwise the request can be accepted and still lose the race to
	-- the host's prior operation. Do not scan
	-- unmentioned units here: their disposition remains the opening-frame
	-- fallback below, while named units are unequivocally CIVVIS's to drive.
	if cfg.CancelQueuedPaths ~= false then
		local named = {};
		for _, row in ipairs(rows) do
			if tostring(row.kind or "") == "unit" then
				local subject = tonumber(row.subject);
				if subject ~= nil then named[subject] = true; end
			end
		end
		if next(named) ~= nil then CivvisBoard.cancelQueuedPaths(player, pid, turn, named); end
	end
	-- Match the guard to the settler's actual host leg before either row is
	-- applied.  A host-only shadow row is deliberately outside the CIVVIS order
	-- counts and verdict: it is an actuation safety repair, not a new decision.
	CivvisBoard.syncCappedSettlerEscorts(pid, turn, rows);
	CivvisBoard.holdVisibleScoutCaptureLegs(pid, turn, rows);
	CivvisBoard.holdVisibleBarbarianCombatCaptureLegs(pid, turn, rows);
	CivvisBoard.holdVisibleBuilderCaptureLegs(pid, turn, rows);
	CivvisBoard.holdActiveFireCivilianLegs(pid, turn, rows);
	local shadowRows = 0;
	for _, row in ipairs(rows) do
		if row._civvis_escort_shadow == true then shadowRows = shadowRows + 1; end
	end

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
		local shadow = row._civvis_escort_shadow == true;
		local wantX, wantY = tonumber(row.x), tonumber(row.y);
		if row._civvis_builder_barbarian_capture_hold == true then
			countRefusal(kind, "builder_barbarian_capture_hold");
			ordered[index] = true;
			return false, "builder_barbarian_capture_hold";
		end
		if row._civvis_active_fire_civilian_hold == true then
			countRefusal(kind, "active_fire_civilian_hold");
			ordered[index] = true;
			return false, "active_fire_civilian_hold";
		end
		-- This move is still a CIVVIS decision and must remain in the order
		-- accounting.  The bridge declined only its exposed host hand-off; emit
		-- the named refusal instead of silently deleting a planned expansion leg.
		if row._civvis_settler_barbarian_combat_hold == true then
			if shadow then
				CivvisBoard.stats.escort_shadow_refused = CivvisBoard.stats.escort_shadow_refused + 1;
			else
				countRefusal(kind, "settler_barbarian_combat_capture_hold");
			end
			ordered[index] = true;
			return false, "settler_barbarian_combat_capture_hold";
		end
		if row._civvis_settler_scout_hold == true then
			countRefusal(kind, "settler_scout_capture_hold");
			ordered[index] = true;
			return false, "settler_scout_capture_hold";
		end
		if row._civvis_settler_scout_guard_hold == true then
			countRefusal(kind, "settler_scout_guard_hold");
			ordered[index] = true;
			return false, "settler_scout_guard_hold";
		end
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
			if shadow then
				CivvisBoard.stats.escort_shadow_applied = CivvisBoard.stats.escort_shadow_applied + 1;
			elseif kind == "produce_next" then
				-- A lease is accepted by the control channel but has not yet
				-- mutated the host. Keep it out of the host applied-rate numerator
				-- and denominator; the later `build` event is the actuation proof.
				deferred = deferred + 1;
			elseif CivvisVerify.isVerdict(kind) then
				-- A verdict on an earlier turn is the ledger's, not the host's:
				-- neither seen nor applied.
				verdicts = verdicts + 1;
			else
				applied = applied + 1;
			end
			if not shadow and not CivvisVerify.isVerdict(kind) then
				byKind[kind] = (byKind[kind] or 0) + 1;
				seenByKind[kind] = (seenByKind[kind] or 0) + 1;
			end
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
			if shadow then
				CivvisBoard.stats.escort_shadow_refused = CivvisBoard.stats.escort_shadow_refused + 1;
			else
				countRefusal(kind, tostring(why));
			end
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
					countRefusal("unit", "queue_prior_refused");
				else
					CivvisQueue.push(subject, row, firstRun[subject].expect);
				end
			else
				local ok = runOrder(index, row);
				if isUnit then
					firstRun[subject] = { expect = ok and CivvisQueue.expectFor(row) or nil };
					if not ok then firstRefused[subject] = true; end
					-- Hold the turn until this walk lands (see CivvisQueue.watch);
					-- a queued follow-up for the unit replaces the watch.
					if queueOn and ok and firstRun[subject].expect ~= nil then
						local watched = liveUnit(pid, subject);
						local origin = watched ~= nil and {
							x = tonumber(try(function() return watched:GetX(); end, -1)),
							y = tonumber(try(function() return watched:GetY(); end, -1)),
						} or nil;
						CivvisQueue.watch(subject, firstRun[subject].expect, origin);
					end
					if queueOn and ok and foundRetry[subject] ~= nil
							and tostring(row.verb or "") == "MOVE_TO" then
						CivvisQueue.push(subject, foundRetry[subject], firstRun[subject].expect);
						foundRetry[subject] = nil;
					end
				end
			end
		end
	end

	-- Great People go first, before the unmentioned-unit holding pass: they
	-- cannot be represented on the planner's unit board, and the mirror drops
	-- `UNIT_GREAT_*`, so CIVVIS cannot mention them. See `orderGreatPerson` for
	-- what this is and is not.
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

	-- ★★★★ CIVVIS OWNS EVERY UNIT MOVEMENT.
	--
	-- A missing row means the planner chose no movement on this board; it does
	-- not delegate a destination to Civilization VI.  This is especially
	-- important for a unit whose apparent safety depends on retaining movement:
	-- the live Slinger that crossed into a combined Spearman + Quadrireme
	-- envelope did so under a host-selected route and could not retreat after
	-- the board saw the threat.  Explicit movement comes only from CIVVIS rows.
	-- Everything else gets a position-preserving order so it cannot block the
	-- turn, and is reconsidered from a fresh board next turn.
	local unmentionedHeld, unmentionedHeldApplied = 0, 0;
	local civiliansSkipped = 0;
	-- NEVER on a combat frame: every unit not named by the frame's answer was
	-- already dispositioned by the opening board and is exactly where CIVVIS
	-- left it.
	if (awaiting.frame or 0) == 0 then
		local mentioned = {};
		for _, row in ipairs(rows) do
			if tostring(row.kind or "") == "unit" then
				mentioned[tonumber(row.subject) or -1] = true;
			end
		end
		eachUnit(player, function(unit)
			local id = try(function() return unit:GetID(); end, -1);
			if id == -1 or mentioned[id] or gpHandled[id] then return; end
			if CivvisBoard.escortHolds[id] then return; end
			local name = unitTypeName(unit);
			-- Civilians receive SKIP_TURN and nothing else. Fortify would be
			-- wrong for a civilian and Alert wrong for a Trader. Skipping changes
			-- no gameplay — the unit stays exactly where CIVVIS left it and is
			-- re-planned next turn — it only tells the engine the turn may end.
			local gp = try(function() return unit:GetGreatPerson(); end);
			unmentionedHeld = unmentionedHeld + 1;
			if name == "UNIT_SETTLER" or name == "UNIT_BUILDER"
					or name == "UNIT_TRADER"
					or (gp ~= nil and try(function() return gp:IsGreatPerson(); end, false)) then
				if operate(unit, OP["UNITOPERATION_SKIP_TURN"], {}) then
					unmentionedHeldApplied = unmentionedHeldApplied + 1;
					civiliansSkipped = civiliansSkipped + 1;
				end
				return;
			end
			-- A held soldier must still receive an operation: leaving it ready
			-- produces ENDTURN_BLOCKING_UNITS and can wedge the run. Fortify
			-- preserves position with defense first, Alert second, Skip last.
			if firstOperation(unit, { "UNITOPERATION_FORTIFY",
					"UNITOPERATION_ALERT", "UNITOPERATION_SKIP_TURN" }) then
				unmentionedHeldApplied = unmentionedHeldApplied + 1;
			end
		end);
	end

	emit("orders", {
		turn = turn, frame = awaiting.frame or 0, source = "civvis",
		seen = #rows - deferred - verdicts - shadowRows,
		applied = applied, refused = refused, by = byKind, refusals = whyNot,
		seen_by = seenByKind, refused_by = refusedByKind,
		deferred = deferred,
		-- Verdict rows on an earlier turn's orders, re-emitted as events; see
		-- CivvisVerify. Not orders, so not in `seen`.
		verdicts = verdicts,
		-- Retained as a literal zero for readers of historical events: this mod
		-- never asks the host to choose an exploration route.
		explored = 0,
		-- Every unmentioned unit is held by the bridge rather than moved by the
		-- host. A gap between these two is a unit the engine may still report as
		-- ready, so the end-turn parking pass can diagnose it.
		unmentioned_held = unmentionedHeld,
		unmentioned_held_applied = unmentionedHeldApplied,
		-- Unmentioned civilians told to skip so the turn can end; this is the
		-- civilian portion of `unmentioned_held_applied`.
		civilians_skipped = civiliansSkipped,
		-- MOVE_TOs sent as this turn's leg of a longer host path, and moves
		-- refused because the unit could not take even the first step this
		-- turn. See CivvisBoard.
		move_capped = CivvisBoard.stats.capped,
		move_no_reach = CivvisBoard.stats.no_reach,
		move_noop = CivvisBoard.stats.move_noop,
		move_fallback = CivvisBoard.stats.move_fallback,
		-- Reconciliation of a co-located guard with the settler's actual host
		-- leg.  The shadow counters are host safety operations, not CIVVIS rows.
		escort_cap_synced = CivvisBoard.stats.escort_cap_synced,
		escort_cap_unresolved = CivvisBoard.stats.escort_cap_unresolved,
		escort_shadow_injected = CivvisBoard.stats.escort_shadow_injected,
		escort_shadow_applied = CivvisBoard.stats.escort_shadow_applied,
		escort_shadow_refused = CivvisBoard.stats.escort_shadow_refused,
		escort_shadow_held = CivvisBoard.stats.escort_shadow_held,
		-- Settler legs held because their actual host destination was adjacent
		-- to a visible barbarian scout and no proven guard could share it.
		settler_scout_capture_held = CivvisBoard.stats.settler_scout_capture_held,
		-- A scout-held Settler's current tile was also inside capture reach, so
		-- a combat unit already sharing that tile was kept from departing.
		settler_scout_guard_held = CivvisBoard.stats.settler_scout_guard_held,
		-- A visible non-scout barbarian combat unit can remove a synchronized
		-- single guard and capture its settler in one hostile turn, so both
		-- matching legs are held instead of counting the guard as coverage.
		settler_barbarian_combat_capture_held =
			CivvisBoard.stats.settler_barbarian_combat_capture_held,
		-- A co-located guard was queued in an earlier frame while its exposed
		-- Settler had no row yet; keep that guard from leaving before the later
		-- Settler safety decision is actuated.
		settler_barbarian_combat_guard_held =
			CivvisBoard.stats.settler_barbarian_combat_guard_held,
		-- A nearby unmentioned combat unit was moved onto a held settler's
		-- current tile when the host could prove the rescue leg this turn.
		settler_barbarian_combat_guard_rescued =
			CivvisBoard.stats.settler_barbarian_combat_guard_rescued,
		-- Builders are civilians as well.  These are visible barbarian capture
		-- legs held by the host bridge, plus proven escapes for Builders that were
		-- already exposed before the current order frame.
		builder_barbarian_capture_held = CivvisBoard.stats.builder_barbarian_capture_held,
		builder_capture_escaped = CivvisBoard.stats.builder_capture_escaped,
		-- Civilian MOVE_TO legs refused because the host's actual destination is
		-- an active forest/jungle fire.  Random-event escorts do not prevent the
		-- shipped `UNIT_KILLED_CIVILIAN` outcome.
		active_fire_civilian_held = CivvisBoard.stats.active_fire_civilian_held,
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
	local rivalTop, metCount, rivalTopAll, majorCount = rivalBest(player, pid);
	local ourScore = try(function() return player:GetScore(); end, -1);
	local cityCount = 0;
	eachCity(player, function() cityCount = cityCount + 1; end);
	-- Kept for the decider's verdict on this turn, which arrives with the next
	-- turn's orders and is emitted as `turn_verified`; see CivvisVerify.
	CivvisVerify.remember(turn, #rows - shadowRows - deferred - verdicts, applied);
	emit("turn", {
		turn = turn,
		score = ourScore,
		rival_best = rivalTop,
		met = metCount,
		-- Reporting only; see `rivalBest`. The abandon rule still reads
		-- `rival_best`, so this changes no decision.
		rival_best_all = rivalTopAll,
		majors = majorCount,
		lead = (rivalTop ~= nil and ourScore >= 0) and (ourScore - rivalTop) or nil,
		cities = cityCount,
		units = counts.total or counts.military,
		army = counts.military,
		gold = try(function() return math.floor(player:GetTreasury():GetGoldBalance()); end, -1),
		orders_source = awaiting.source,
		orders_seen = #rows - shadowRows - deferred - verdicts,
		-- ⚠ `orders_applied` here is the RETURN-CODE count — arms whose request
		-- did not throw — kept under its old name for the readers that clock on
		-- this record. `orders_reported` is the same number under its honest
		-- name; the verified count for this turn lands in the next turn's
		-- `turn_verified` event, and `civ6_ladder.orders_totals` sums that one
		-- as the summary's `orders_applied`.
		orders_applied = applied,
		orders_reported = applied,
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

-- Exposed for the offline step-turn-actions regression (see CivvisSettleTurn).
CivvisBeginTurn = beginTurn;

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
		-- Everything issued has settled. If a strike went out this frame, or
		-- ground was revealed that somebody can still act on, and frames are
		-- enabled, open the next one: the brain re-plans the same turn on the
		-- board as it now stands. See CivvisFrames.
		if not CivvisFrames.settled then
			CivvisFrames.observe(player, pid, turn);
			if CivvisFrames.wanted() then
				CivvisFrames.begin(player, pid, turn);
				return false;
			end
			CivvisFrames.settled = true;
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
	-- Polling every `OrdersPollTicks` ticks bounds the queries — but note what a
	-- tick IS here: `settleTurn` runs from `tick()`, which `onGameCoreTick` calls
	-- once per `TickEvery` (16) publish batches. Thirty of those was 480 publish
	-- batches per poll, and on the live Emperor lane (run 195234Z, receipt-stamped)
	-- that was **3.8 s** between the board going out and the first poll, paid on
	-- the opening board AND on every replan frame while the brain had answered
	-- 50 ms after the board reached it: state → orders median 3.95 s, replan →
	-- frame state median 3.84 s, on turns of 13–20 s. Four ticks is 64 publish
	-- batches per poll — still an order of magnitude short of the every-publish
	-- query that deadlocked 20260730T110209Z — and the poll budgets below are
	-- scaled by the same factor so every wall-clock allowance is unchanged.
	local every = cfg.OrdersPollTicks or 2;
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
			-- ★★★★★ NOT `return true` — THAT ENDED THE TURN ON THE TICK THE
			-- OPENING ORDERS WENT OUT. The caller requests `ACTION_ENDTURN` the
			-- moment this returns true, and on this tick every unit has just
			-- been handed its FIRST order: the walk is in flight, the strike,
			-- the found and the second step are still on the per-unit queue,
			-- and no replan frame has been considered. Whenever the host took
			-- that request at once (nothing blocking: every unit busy walking),
			-- the turn ended under the queue — its leftovers refused as
			-- `queue_turn_over` — and the frame never opened: a unit stepped,
			-- stood, and kept the rest of its movement. The `awaiting.done`
			-- branch above is the one written to drain the queue, open the
			-- frame and only then release the turn; it just never got a tick.
			-- One more tick is all this costs. The same holds for a frame's
			-- answer, which arrives through this branch too.
			return false;
		end
	end

	-- A combat frame has its own short budget and no fallback: past it the
	-- frame is abandoned by name and the turn ends as it always did. The
	-- opening board's stale-answer and built-in ladders below never apply
	-- to a frame — a stale answer is the very board this frame replaces.
	if frame > 0 then
		if awaiting.polls >= (tonumber(cfg.CombatFramePolls) or 300) then
			awaiting.done = true;
			awaiting.source = "civvis";
			-- Every trigger, and the cap: a brain that could not answer this
			-- frame in time is not asked again this turn.
			CivvisFrames.strikes = 0;
			CivvisFrames.revealed = 0;
			CivvisFrames.current = CivvisFrames.max();
			emit("combat_frame_timeout", { turn = turn, frame = frame, polls = awaiting.polls,
			                               reason = CivvisFrames.reason });
			return true;
		end
		return false;
	end

	-- Past the wait, prefer CIVVIS's most recent answer over the built-ins.
	if awaiting.polls >= (cfg.OrdersWaitPolls or 600) then
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
				-- As above: the queue drains on the next tick, then the turn ends.
				return false;
			end
		end
	end
	-- ⚠ THE FLOOR. A brain that is slow, crashed, or has not been started must cost
	-- decision QUALITY, never progress: three regressions in this project came from
	-- a mechanism given authority with no floor for being wrong. Past the budget the
	-- built-in heuristics run and the turn is recorded as `fallback`, which is a
	-- number to watch — a run that is mostly fallback is not a measurement of CIVVIS.
	if awaiting.polls >= (cfg.OrdersFallbackPolls or 1800) then
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

-- Exposed for the offline step-turn-actions regression
-- (step_turn_actions_test.lua), with `beginTurn` below it.
CivvisSettleTurn = settleTurn;
CivvisSurvey = survey;

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
	local rivalTop, metCount, rivalTopAll, majorCount = rivalBest(player, pid);
	local ourScore = try(function() return player:GetScore(); end, -1);
	emit("turn", {
		policies = policies,
		war = war,
		target = warTarget and (warTarget.capital and "capital" or "city") or nil,
		turn = turn,
		score = ourScore,
		rival_best = rivalTop,
		met = metCount,
		-- Reporting only; see `rivalBest`. The abandon rule still reads
		-- `rival_best`, so this changes no decision.
		rival_best_all = rivalTopAll,
		majors = majorCount,
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

-- ★★★★ THE TURN-START BOARD READ EVERY UNIT'S MOVEMENT BEFORE THE ENGINE
-- RESTORED IT.
--
-- `tick` reaches `beginTurn` from `LocalPlayerTurnBegin` with `IsTurnActive()`
-- already true, and `GetMovesRemaining()` still answers LAST turn's leftover:
-- a Warrior that spent both moves reads 0, a Scout that spent one of three
-- reads 2. The board trusts that export (`Seat::moves_at_turn_start`), plans
-- the unit as it stands, and leaves it unmentioned, so the unit is skipped.
-- Measured on `civvis-20260901T212354Z`: the first Settler read 0 on turns 3
-- and 5, walked on 2, 4 and 6 only, and founded the capital on turn 6 for a
-- two-hex walk. Across turns 20-60 of a mature run a quarter of frame-0 unit
-- rows were 0-move rows; the frame published later the same turn read full
-- movement for 290 of 293 of them.
--
-- So the turn is not opened until every unit reads its full allowance, or
-- `MOVES_RESTORED_PATIENCE` ticks have passed — a unit the engine walked on a
-- queued path before `cancelQueuedPaths` ran is legitimately short and must
-- not hold the turn. `lastTurnSeen` is left at the previous turn so the next
-- tick retries; ticks come undivided from `EndTurnBlockingChanged` and 1-in-16
-- from `GameCoreEventPublishComplete`, so the wait is a fraction of a second.
-- On `CivvisBoard` rather than as locals: the main chunk is at Lua's
-- 199-local ceiling (`test_main_chunk_locals_stay_under_the_limit`).
CivvisBoard.MOVES_RESTORED_PATIENCE = 12;
CivvisBoard.movesRestoredWait = { turn = -1, ticks = 0 };
function CivvisBoard.movementNotYetRestored(player, turn)
	local movesRestoredWait = CivvisBoard.movesRestoredWait;
	if movesRestoredWait.turn ~= turn then
		movesRestoredWait.turn = turn;
		movesRestoredWait.ticks = 0;
	end
	local short = 0;
	eachUnit(player, function(unit)
		local max = tonumber(try(function() return unit:GetMaxMoves(); end, 0)) or 0;
		local left = tonumber(try(function() return unit:GetMovesRemaining(); end, -1)) or -1;
		if max > 0 and left >= 0 and left < max then short = short + 1; end
	end);
	if short == 0 then
		if movesRestoredWait.ticks > 0 then
			emit("moves_restored", { turn = turn, ticks = movesRestoredWait.ticks });
		end
		return false;
	end
	movesRestoredWait.ticks = movesRestoredWait.ticks + 1;
	if movesRestoredWait.ticks > CivvisBoard.MOVES_RESTORED_PATIENCE then
		emit("moves_restored", { turn = turn, ticks = movesRestoredWait.ticks,
		                         short = short, gave_up = true });
		return false;
	end
	return true;
end

local function tick()
	if finished or inTick or cfg.Play == false then return; end
	inTick = true;
	local ok, err = pcall(function()
		-- ★★★★ RETIRE, WHICH IS HOW A QUIT GAME GETS A RESULT AT ALL.
		--
		-- Killing the harness leaves the game unfinished: no `TeamVictory`, no
		-- defeat, and nothing for `tools/civ6_ladder.py` to record — an attempt
		-- that was genuinely lost reads exactly like one that crashed. The
		-- operator asked for the shipped Retire instead, and it is one call: the
		-- stock `InGameTopOptionsMenu.lua` `OnReallyRetire` does exactly
		-- `UI.RequestAction(ActionTypes.ACTION_RETIRE)`, and everything else in
		-- that function closes its own menu and plays a sound. No pause menu and
		-- no confirm dialog — which matters, because that dialog is a `PopupDialog`
		-- this controller would then have to find and click blind.
		--
		-- ⚠ Polled here rather than handled in `applyOrder`. That only ever sees
		-- the rows for the turn and frame the tick is fetching, so a request made
		-- at a moment nobody scheduled would sit unread until a frame happened to
		-- match. Matching on the run alone means one row anywhere is enough.
		--
		-- ⚠ Asked ONCE, latched on `CivvisBoard` rather than a new file-scope
		-- local: this main chunk is at Civ 6's 200-register ceiling and three more
		-- locals here stop the whole mod compiling. `RequestAction` is also
		-- asynchronous, so the tick keeps running for a few frames afterwards and
		-- re-asking would queue a pile of retires behind the first.
		-- ⚠⚠⚠ SAY ONCE THAT THIS RAN, because three abandons in a row wrote the
		-- row and got no answer, and nothing in the log could tell whether the
		-- poll executed, whether the attach failed, or whether the query simply
		-- found nothing. Every explanation was unfalsifiable — the same trap the
		-- wedge sampler was added to close.
		--
		-- Measured 2026-08-30, run civvis-20260830T055337Z: abandoned at t150,
		-- `retire_requested: true`, the row present as
		-- `150|99000|retire|below_leader_score|990` with a run tag byte-identical
		-- to the decider's own rows, and NO `retired` event. The mod was alive —
		-- it emitted `orders` and `turn` for t150 — so it should have polled.
		--
		-- One event per game, on the first poll only: enough to separate "never
		-- ran" from "ran and saw nothing", and cheap enough to keep afterwards.
		if not CivvisBoard.retireAsked and attachOrders() then
			-- ⚠⚠ ONCE PER GAME ANSWERED THE WRONG QUESTION. The first version
			-- latched, so it said "retire_poll at turn 1" and nothing more —
			-- proving only that the poll runs at game START. What matters is
			-- whether it is still running at the moment the harness writes the
			-- row, which is turn 150 or later, and the latch could never say.
			--
			-- That distinction is the whole question. A parked Game Core stops
			-- publishing, and this poll runs on `GameCoreEventPublishComplete`,
			-- so a game that parked BEFORE the abandon fired cannot answer a
			-- retire row however correct the row is. Periodic reporting
			-- distinguishes "the poll stopped when the game parked" from "the
			-- poll was running and did not see the row".
			--
			-- Every 25 turns: about six lines a game, one of them near t150.
			local pollTurn = try(function()
				return Game.GetCurrentGameTurn();
			end, -1);
			if pollTurn >= 0 and CivvisBoard.retirePollAt ~= pollTurn
					and (pollTurn % 25 == 0 or pollTurn == 1) then
				CivvisBoard.retirePollAt = pollTurn;
				emit("retire_poll", { turn = pollTurn });
			end
			local wanted = false;
			pcall(function()
				local rows = DB.Query(string.format(
					"SELECT count(*) AS n FROM civvis.orders WHERE run = '%s' " ..
					"AND kind = 'retire'", sqlSafe(cfg.RunTag)));
				for _, row in ipairs(rows) do wanted = (tonumber(row.n) or 0) > 0; end
			end);
			if wanted then
				CivvisBoard.retireAsked = true;
				local action = try(function() return ActionTypes.ACTION_RETIRE; end);
				if action == nil then
					emit("retire_failed", { why = "no_action_type" });
				else
					local asked = pcall(function() UI.RequestAction(action); end);
					emit(asked and "retired" or "retire_failed",
					     { why = asked and "requested" or "refused" });
				end
				return;
			end
		end

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
			-- ★★★★★ `spent` IS WHAT THE BALLOT ASKED FOR. IT IS NOT WHAT WAS TAKEN.
			--
			-- `voteWorldCongress` returns its own model of the stake: it walks
			-- the host's cost table, decrements a local bank, and adds the
			-- charge for every vote it requested. The host charges for the
			-- votes it RECORDS, and it has never recorded more than one --
			-- 139 of 139 multi-vote asks came back `recorded 1` across the
			-- whole `wc_ballot_verdict` corpus. So every `spent` above zero
			-- this ledger has ever carried is Favor that never moved, and the
			-- reader had to join two sessions of `wc_ballot_verdict` to find
			-- that out.
			--
			-- `favor_before` was already read here; reading the bank back
			-- costs one more accessor and makes the row self-describing.
			-- ⚠ A player operation is queued, not applied inline, so a real
			-- charge may land after this read: treat `favor_after` as a lower
			-- bound on what was taken, and `wc_ballot_verdict.favor_now` at
			-- the next review as the settled figure. The two together are
			-- still strictly more than `spent` alone, which is a forecast.
			local after = tonumber(try(function() return ballotPlayer:GetFavor(); end, -1)) or -1;
			emit("wc_vote", { turn = ballotTurn, cast = cast, spent = spent,
			                  favor_asked = spent,
			                  why = why, leader = leader,
			                  leader_points = leaderPoints,
			                  leader_score = leaderScore, source = trigger,
			                  stage = stage, favor_before = before, mode = mode,
			                  favor_after = after,
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
			-- See `movementNotYetRestored`: the board waits for the engine to
			-- hand the units their turn's movement, not the previous turn's dregs.
			if cfg.CivvisDecides and CivvisBoard.movementNotYetRestored(player, turn) then return; end
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
		local same_pass_forced = false;
		if blocker ~= nil and blocker ~= none then
			local name = blockerName(blocker);
			attempts = attempts + 1;
			local answered;
			if SOFT_BLOCKERS[name] then
				-- `EspionageEscape` is a soft blocker only in the sense that it is
				-- not a CIVVIS decision. It is still a native prompt the engine will
				-- not clear by dismissing its notification; answer it with the same
				-- operation the shipped fourth button sends.
				if name == "ENDTURN_BLOCKING_SPY_CHOOSE_ESCAPE_ROUTE" then
					answered = CivvisChooseSpyEscapeRoute(pid)
						and "escape_route:city_center" or nil;
				elseif cfg.CivvisDecides then
					-- CIVVIS has already made and applied its complete unit-order
					-- pass in settleTurn. A soft blocker is only a UI reminder; the
					-- legacy unit AI must not invent orders here. In particular, it
					-- previously moved a Settler out of a safe capital and into a
					-- visible barbarian capture zone after CIVVIS chose to wait.
					--
					-- ⚠⚠⚠ "COMPLETE" WAS A CLAIM, AND THE ENGINE DISAGREED.
					-- The pass is complete for units CIVVIS MENTIONED. A unit can
					-- go ready again inside the same turn — it finishes the walk
					-- the opening board gave it, and a REPLAN FRAME does not
					-- re-run the unassigned pass (it is skipped whenever `frame > 0`,
					-- because the opening frame already parked each unmentioned unit).
					-- Nothing then dispositions it, `ENDTURN_BLOCKING_UNITS`
					-- stands, and answering "civvis_complete" told us it was
					-- handled while the turn died.
					--
					-- Measured 2026-08-28, run civvis-20260828T190111Z at turn 113
					-- — 7 cities, 33 units, the furthest game of the day:
					--     frame 0  expl=20 civskip=3   (everything dispositioned)
					--     frame 1  6 movers replanned, expl=0
					--     blocked ENDTURN_BLOCKING_UNITS answered="civvis_complete"
					-- and the game never advanced again.
					--
					-- So PARK the units the engine still calls ready, and only
					-- then claim completion. `parkReadyUnits` is `orderIdle`:
					-- SKIP_TURN, FORTIFY, ALERT, SLEEP. Every one holds position —
					-- none is a move, so the Settler-into-barbarians case this
					-- branch was written for cannot recur. A unit CIVVIS ordered
					-- is not ready and is never touched.
					--
					-- `UNIT_BLOCKERS` already existed for the forfeit path,
					-- whose own comment says these are "the ones whose forfeit
					-- needs the parking pass". The pass was simply not reached
					-- until the forfeit, by which point the turn had already
					-- spent its attempts. This runs it at the answer instead.
					-- ⚠ THIS BRANCH DELIBERATELY HAS NO ALTERNATIVE ARM, AND
					-- THE WORD FOR ONE MUST NOT APPEAR HERE AT ALL.
					-- `test_civvis_soft_blockers_do_not_invoke_legacy_unit_ai`
					-- splits this handler on the first occurrence of that
					-- keyword to tell the CIVVIS arm from the legacy one, so
					-- either the keyword or a comment naming it cuts the arm in
					-- half and the guard then reads the wrong code. Both
					-- mistakes were made here in turn; the base answer is
					-- assigned first and the park only appends to it.
					answered = "civvis_complete";
					if UNIT_BLOCKERS[name] then
						local parked = parkReadyUnits(player);
						if parked > 0 then
							answered = answered .. "+parked:" .. parked;
						end
						-- A units blocker can mask an empty city queue.  The live run
						-- civvis-20260903T135954Z reached t40 with a pending Ravenna
						-- production request, then reported only ENDTURN_BLOCKING_UNITS;
						-- forcing the turn here skipped the production repair arm below.
						-- Retry the same forced production ladder after parking units,
						-- so a Civvis request that has not appeared in the host queue yet
						-- can still be handed to the game before this pass ends.
						local set = driveProduction(player, turn, true) or 0;
						answered = answered .. "+production_retry:" .. set;
						-- A parked answer can leave the Game Core waiting without publishing
						-- the second sighting that the bounded forfeit below used to need.
						-- Dismiss and force now, after the position-preserving parking pass;
						-- this is the same accepted SHIFT+ENTER request used by the later
						-- forfeit, and it keeps a quiet first response from wedging the turn.
						local dropped = dismissBlocker(pid, blocker);
						answered = answered .. "+forced";
						emit("dismissed", { turn = turn, blocker = name,
						                    dismissed = dropped, attempts = attempts,
						                    answered = answered, parked = parked,
						                    forfeit = 0, forced = true, same_pass = true });
						same_pass_forced = true;
						pcall(function()
							UI.RequestAction(ActionTypes.ACTION_ENDTURN,
							                 { REASON = "UserForced" });
						end);
					end
					-- ⚠⚠⚠ THE SAME CLAIM-NOT-CHECK DEFECT, ON THE POLICY SLOT.
					-- Parking the ready units repaired it for `ENDTURN_BLOCKING_UNITS`;
					-- `FILL_CIVIC_SLOT` was left claiming completion over a slot that is
					-- still open. Dismissing cannot help here: an empty slot is something
					-- end-turn genuinely requires, so the engine raises it straight back.
					--
					-- Measured 2026-08-29, run civvis-20260829T022749Z at turn 114 -- 8
					-- cities, the strongest empire of the day:
					--     blocked   FILL_CIVIC_SLOT  answered civvis_complete   1
					--     blocked   FILL_CIVIC_SLOT  answered civvis_complete  25
					--     dismissed FILL_CIVIC_SLOT                            40
					-- and that cycle repeated unchanged until the watchdog killed the
					-- game. The forfeit ladder ran every time and dismissed every time.
					--
					-- A policy deck request is an asynchronous transaction. On the live
					-- 2026-09-01 run `civvis-20260901T230916Z`, turns 184--208 kept
					-- returning `FILL_CIVIC_SLOT` after the full deck request succeeded:
					-- one Economic slot was still empty, while the same-turn replan was
					-- correctly deferred as `same_turn_transaction_in_flight`. Calling
					-- `fillPolicies` here would submit a second transaction against that
					-- in-flight request. The first `civvis_complete` answer then stopped
					-- the next board publication, so the old second-sighting forfeit could
					-- never run and the game stayed on the same turn.
					--
					-- Force this hard blocker in the same pass after CIVVIS has answered.
					-- The forced end turn gives the engine a fresh turn in which the
					-- policy transaction can settle and the targeted repair can run. Keep
					-- the filler for the non-racing path, where CIVVIS has not answered
					-- this turn and an open slot can still be filled safely.
					if name == "ENDTURN_BLOCKING_FILL_CIVIC_SLOT" then
						if answered == "civvis_complete" then
							local dropped = dismissBlocker(pid, blocker);
							answered = answered .. "+forced";
							emit("dismissed", { turn = turn, blocker = name,
							                    dismissed = dropped, attempts = attempts,
							                    answered = answered, parked = 0,
							                    forfeit = 0, forced = true, same_pass = true });
							same_pass_forced = true;
							pcall(function()
								UI.RequestAction(ActionTypes.ACTION_ENDTURN,
								                 { REASON = "UserForced" });
							end);
						else
							local filled = fillPolicies(player);
							if filled then
								answered = answered .. "+" .. tostring(filled);
							end
						end
					end
					-- ⚠⚠⚠ AND THE SAME THING AGAIN ON PRODUCTION, WHICH PARKS THE
					-- WHOLE GAME RATHER THAN ONE TURN.
					--
					-- A city with nothing queued is something end-turn genuinely
					-- requires, so `civvis_complete` — "CIVVIS has already decided
					-- this board" — is a claim the engine does not accept. Unlike the
					-- policy slot it is not merely re-raised: the Game Core stops
					-- publishing while it waits, the agent is driven ONLY by
					-- `GameCoreEventPublishComplete`, and so it never ticks again.
					-- Nothing recovers that from inside or outside the process (see
					-- `civ6_nudge_end_turn.py`: an external forced end turn was
					-- measured and ignored, twice).
					--
					-- Measured 2026-08-30, run civvis-20260830T074021Z, parked at
					-- t87 on `ENDTURN_BLOCKING_PRODUCTION` answered `civvis_complete`
					-- at attempts=1 — the forfeit ladder never even ran. The last
					-- board shows why:
					--     Rome      producing BUILDING_PETRA        turns 11
					--     Ravenna   producing nil                   turns -1
					--     Lugdunum  producing BUILDING_CONSULATE    turns 18
					-- One city with nothing to build ended the run.
					--
					-- `driveProduction` is the same call the ordinary production arm
					-- makes; forced, so a city the ranking left empty gets something
					-- rather than nothing. It returns how many cities it set, so a
					-- board that was already complete costs one pass and says so.
					if name == "ENDTURN_BLOCKING_PRODUCTION" then
						local set = driveProduction(player, turn, true) or 0;
						if set > 0 then
							answered = answered .. "+produced:" .. set;
						else
							-- ⚠⚠⚠ THE REPAIR HAS NEVER ONCE FIRED, AND NOTHING SAID SO.
							-- Measured over the twelve runs of 2026-08-30: 87
							-- `ENDTURN_BLOCKING_PRODUCTION` blockers answered, **zero**
							-- carrying `+produced:`, and **40 of them answered while a
							-- city genuinely had an empty queue** (`producing_hash == 0`
							-- in the same turn's exported state). Run
							-- civvis-20260830T104408Z parked at t88 with Lugdunum on
							-- `hash = 0`.
							--
							-- `set == 0` is the right answer on a board that is already
							-- complete and the WRONG one on a board with an empty city,
							-- and the ledger could not tell those apart — so the fix
							-- above was unfalsifiable from the outside. Count what the
							-- drive left behind and say it. Read-only: `+empty:N` beside
							-- a `civvis_complete` on this blocker is the repair failing,
							-- and its absence is a board that needed nothing.
							local empty = 0;
							eachCity(player, function(city)
								local hash = try(function()
									local queue = city:GetBuildQueue();
									return queue and queue:GetCurrentProductionTypeHash() or 0;
								end, 0);
								if hash == nil or hash == 0 then empty = empty + 1; end
							end);
							if empty > 0 then
								answered = answered .. "+empty:" .. empty;
							end
						end
					end
					-- A CIVVIS envoy pass may intentionally keep a token for a
					-- better claim later. The native consideration prompt is
					-- optional, but leaving it unanswered can stop publishing the
					-- next turn. Mark it through a fresh handle, then use the same
					-- forced request as the unit repair so this first sighting is
					-- sufficient even when the Game Core goes quiet.
					if name == "ENDTURN_BLOCKING_GIVE_INFLUENCE_TOKEN" then
						local considered = CivvisMarkEnvoyConsidered(player);
						answered = answered .. (considered
							and "+considered" or "+considered_unavailable");
						local dropped = dismissBlocker(pid, blocker);
						answered = answered .. "+forced";
						emit("dismissed", { turn = turn, blocker = name,
						                    dismissed = dropped, attempts = attempts,
						                    answered = answered, parked = 0,
						                    forfeit = 0, forced = true, same_pass = true });
						same_pass_forced = true;
						pcall(function()
							UI.RequestAction(ActionTypes.ACTION_ENDTURN,
							                 { REASON = "UserForced" });
						end);
					end
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
				if same_pass_forced then attempts = 0; end
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
			if (SOFT_BLOCKERS[name] or answered == "civvis_complete")
				and not same_pass_forced then
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
						-- ⚠⚠⚠ ONE BLOCKER MUST NOT BE FORCED PAST YET. The congress session
						-- defers its ballot by one forfeit cycle on purpose (the vote arm just
						-- above): forfeit 1 waits for the stage-1/popup ballot to land, and only
						-- forfeit 2 falls back to vote-and-submit. Forcing the turn at forfeit 1
						-- ends it before either can happen, so the session is dismissed unvoted
						-- every time -- and this seat plays for a DIPLOMATIC victory, where those
						-- votes are the win condition, not a side decision worth forfeiting.
						-- Once the ballot is cast for this turn the session is a spent blocker
						-- like any other and is forced with the rest.
						local holdForVote = name == "ENDTURN_BLOCKING_WORLD_CONGRESS_SESSION"
							and seen.voted_turn ~= turn;
						local dropped = dismissBlocker(pid, blocker);
						emit("dismissed", { turn = turn, blocker = name,
						                    dismissed = dropped, attempts = attempts,
						                    answered = answered, parked = parked,
						                    forfeit = seen.forfeits,
						                    forced = not holdForVote });
						attempts = 0;
						-- ⚠⚠⚠ EVERY FORFEITED BLOCKER GETS THE FORCED END TURN, NOT ONLY
						-- THE UNIT ONES. Reaching this point means the ladder is spent: the
						-- answer was tried, the blocker survived it, and the notification has
						-- just been dismissed. Dismissal does not stick for anything end-turn
						-- genuinely requires -- the engine raises it straight back -- and
						-- `ACTION_ENDTURN` without a reason is refused while it stands. So the
						-- turn could not end, and the run died holding a decision it had
						-- already given up.
						--
						-- Measured across the 2026-08-28/29 ladder runs, dismissals by blocker:
						--     26  forced   ENDTURN_BLOCKING_UNITS
						--     39  NOT      ENDTURN_BLOCKING_WORLD_CONGRESS_SESSION
						--     24  NOT      ENDTURN_BLOCKING_FILL_CIVIC_SLOT
						--     12  NOT      ENDTURN_BLOCKING_GIVE_INFLUENCE_TOKEN
						--      6  NOT      ENDTURN_BLOCKING_CLAIM_GREAT_PERSON
						-- Run civvis-20260829T032446Z shows both halves in one game: t88
						-- dismissed UNITS with `parked=0` and `forced=true` and the turn
						-- advanced; t94 dismissed GIVE_INFLUENCE_TOKEN with `forced=false` and
						-- the game never played another turn. Run ...T182156Z died the same way
						-- on the same prompt at t85.
						--
						-- The trade is the one this ladder already chose: forfeiting one
						-- decision beats losing every turn after it. `parkReadyUnits` above
						-- still runs for the unit blockers alone, because only those have units
						-- to park.
						if not holdForVote then
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

		if not same_pass_forced then
			pcall(function()
				if UI.GetInterfaceMode() ~= InterfaceModeTypes.SELECTION then
					UI.SetInterfaceMode(InterfaceModeTypes.SELECTION);
				end
				UI.RequestAction(ActionTypes.ACTION_ENDTURN);
			end);
		end
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
	CivvisTrade.pollPeace();
	ticksSeen = ticksSeen + 1;
	if ticksSeen % (cfg.TickEvery or 16) ~= 0 then return; end
	ticksTaken = ticksTaken + 1;
	tick();
end

local function onLocalPlayerTurnBegin()
	ensureStarted();
	CivvisTrade.pollPeace();
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
-- The bridge already knows how to take an Aid Request's first-place score,
-- Climate Accords' power-plant decommission score, and World's Fair's Great
-- Person point score. World Games' 50-point athlete project is likewise
-- already priced. International Space Station's 30-point astronaut project is
-- too. Nobel Literature and Physics score the matching Great Person points,
-- while Nobel Peace scores generated Favor. All paths require membership,
-- while the prior controller merely let the World Crisis prompt wait for a
-- person. Firaxis's own WorldCrisisPopup handles
-- `Events.EmergencyAvailable` by issuing this exact ACCEPT_EMERGENCY operation
-- with PARAM_OTHER_PLAYER and PARAM_EMERGENCY_TYPE. Take that same operation,
-- but only for competitions with a priced path that this controller has
-- explicitly approved. Other emergencies can create wars or commit production
-- that this event has not priced, so they remain untouched.
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
	local worldsFair = kind == "EMERGENCY_WORLDS_FAIR";
	local worldGames = kind == "EMERGENCY_WORLD_GAMES";
	local spaceStation = kind == "EMERGENCY_SPACE_STATION";
	local nobel = kind == "EMERGENCY_NOBEL_PRIZE_LITERATURE"
		or kind == "EMERGENCY_NOBEL_PRIZE_PEACE"
		or kind == "EMERGENCY_NOBEL_PRIZE_PHYSICS";
	local targetFree = climate or worldsFair or worldGames or spaceStation or nobel;
	if not targetFree and cfg.AutoJoinAidRequests == false then return; end
	if climate and cfg.AutoJoinClimateAccords == false then return; end
	if worldsFair and cfg.AutoJoinWorldsFair == false then return; end
	if worldGames and cfg.AutoJoinWorldGames == false then return; end
	if spaceStation and cfg.AutoJoinSpaceStation == false then return; end
	if nobel and cfg.AutoJoinNobelPrizes == false then return; end
	local turn = try(function() return Game.GetCurrentGameTurn(); end, -1);
	local function report(reason, submitted)
		emit(climate and "climate_accords_join"
			or worldsFair and "worlds_fair_join"
			or worldGames and "world_games_join"
			or spaceStation and "space_station_join"
			or nobel and "nobel_prize_join" or "aid_emergency_join", {
			turn = turn, target = target or -1,
			emergency = kind ~= "" and kind or tostring(emergencyType or ""),
			submitted = submitted and true or false, reason = reason,
		});
	end
	if pid == nil or pid < 0 or target == nil or emergency == nil then
		report("invalid_event", false);
		return;
	end
	if not aid and not targetFree then
		report("not_aid_request", false);
		return;
	end
	if targetFree then
		-- Climate Accords, World's Fair, World Games, International Space Station,
		-- and all Nobel Prize competitions have NoTarget=true. The shipped popup
		-- sends -1 through
		-- PARAM_OTHER_PLAYER; a real player ID is mismatched.
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
		or worldsFair and ("worlds_fair_join:" .. kind)
		or worldGames and ("world_games_join:" .. kind)
		or spaceStation and ("space_station_join:" .. kind)
		or nobel and ("nobel_prize_join:" .. kind)
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
		DiplomacyStatement = CivvisOnDiplomacyStatement,
		DiplomacySessionClosed = CivvisOnDealSessionClosed,
		EmergencyAvailable = CivvisOnAidEmergencyAvailable,
		LoadGameViewStateDone = ensureStarted,
		TeamVictory = onTeamVictory,
		PlayerDefeat = onPlayerDefeat,
		-- The tactical ledger: see CivvisLedger.
		CombatVisBegin = CivvisLedger.onCombatVisBegin,
		CombatVisEnd = CivvisLedger.onCombatVisEnd,
		UnitDamageChanged = CivvisLedger.onUnitDamageChanged,
		UnitMoved = CivvisLedger.onUnitMoved,
		UnitRemovedFromMap = CivvisLedger.onUnitRemoved,
		UnitCaptured = CivvisLedger.onUnitCaptured,
		CityOccupationChanged = CivvisLedger.onCityOccupationChanged,
	}) do
		pcall(function() Events[name].Add(handler); end);
	end
end

Initialize();
