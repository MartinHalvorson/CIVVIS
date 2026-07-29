-- Configure and start a Civilization VI game from the main menu, exactly.
--
-- The old harness reached a game by clicking: Single Player, then Play Now,
-- then BEGIN GAME, at screen fractions measured once and hoped to hold. That
-- works and it starts *a* game -- always the default one. It cannot express
-- "Settler difficulty, duel map, seed 424242", and difficulty is the whole
-- point of a milestone ladder.
--
-- This does what the game's own automated tests do instead. Every setting
-- below is written straight into GameConfiguration/MapConfiguration and then
-- Network.HostGame is called, which is the same sequence
-- Automation_StandardTests.lua's "PlayGame" runs. No screen coordinate is
-- involved, so nothing here breaks when a window moves.
--
-- The run's settings arrive as a `CivvisControlConfig` table that the
-- installer prepends to this file. They are prepended rather than `include`d
-- because a file listed under <Files> in a .modinfo is not on the include
-- path, so the include fails silently and every setting falls back to its
-- default -- a run configured for Settler that quietly plays Prince.

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

-- Where a line lands is build-dependent: this macOS build writes no Lua.log,
-- so `print` alone makes a working script look dead. Every available sink gets
-- the line; the redundancy is the point.
local function emit(kind, payload)
	payload = payload or {};
	payload.kind = kind;
	payload.ctx = "setup";
	payload.run = cfg.RunTag or "unset";
	local line = PREFIX .. encode(payload);
	pcall(function() print(line); end);
	pcall(function() Automation.Log(line); end);
	pcall(function() UI.DataError(line); end);
end

local function try(fn, fallback)
	local ok, result = pcall(fn);
	if ok then return result; end
	return fallback;
end

-- ------------------------------------------------------------- the settings

-- Player counts follow the map, not the other way round: setting a map size
-- without republishing the counts leaves a duel map configured for six.
local function updatePlayerCounts()
	local size = MapConfiguration.GetMapSize();
	local def = GameInfo.Maps[size];
	if def == nil or def.DefaultPlayers == nil then return; end
	MapConfiguration.SetMaxMajorPlayers(def.DefaultPlayers);
	GameConfiguration.SetParticipatingPlayerCount(
		def.DefaultPlayers + GameConfiguration.GetHiddenPlayerCount());
end

-- Exactly one seat is ours; every other major is the shipped AI. Slot status
-- has to be settled before hosting, because it is what decides whether the
-- game waits for orders or plays itself.
local function seatHuman(count)
	count = count or 1;
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
	return count - needed;
end

local function applyConfiguration()
	GameConfiguration.SetToDefaults();
	-- SetToDefaults pins the ruleset to standard. Clearing it lets the setup
	-- logic pick the best default, which on this install is Gathering Storm;
	-- an explicit RuleSet below overrides that again.
	GameConfiguration.SetValue("RULESET", nil);

	if cfg.RuleSet then GameConfiguration.SetRuleSet(cfg.RuleSet); end
	if cfg.MapScript then
		MapConfiguration.SetScript(cfg.MapScript);
		updatePlayerCounts();
	end
	if cfg.MapSize then
		MapConfiguration.SetMapSize(cfg.MapSize);
		updatePlayerCounts();
	end

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

	-- Free-form passthrough for anything without a named setter above, using
	-- the same "Game.<key>" / "Map.<key>" convention the shipped tests use. A
	-- leading '#' means the value is a type name to be hashed.
	for key, value in pairs(cfg.GameValues or {}) do
		if type(value) == "string" and value:sub(1, 1) == "#" then
			value = DB.MakeHash(value:sub(2));
		end
		GameConfiguration.SetValue(key, value);
	end
	for key, value in pairs(cfg.MapValues or {}) do
		if type(value) == "string" and value:sub(1, 1) == "#" then
			value = DB.MakeHash(value:sub(2));
		end
		MapConfiguration.SetValue(key, value);
	end

	-- Pin the leader so every rung is climbed by the same civilization.
	--
	-- A random leader changes the whole game — Rome's free monument and road
	-- on every founding is a different opening from Russia's tundra faith —
	-- and comparing two attempts across two civilizations compares nothing.
	-- ⚠ This only takes effect if the FrontEnd/Setup context actually hosts
	-- the game. On this install it usually does not, and the harness falls
	-- back to driving the Create Game screen by sight, which leaves the leader
	-- at whatever that screen defaults to. `seat` reports the leader the game
	-- actually gave us, so the log says which happened rather than assuming.
	if cfg.Leader then
		for _, id in ipairs(GameConfiguration.GetHumanPlayerIDs()) do
			local ok = pcall(function()
				PlayerConfigurations[id]:SetLeaderTypeName(cfg.Leader);
			end);
			emit("leader", { requested = cfg.Leader, applied = ok, slot = id });
		end
	end

	-- Read back rather than report what was asked for. A setter that silently
	-- rejects a value produces a game at the wrong difficulty, and the whole
	-- ladder this exists to climb is measured in difficulty.
	emit("configured", {
		requested = {
			ruleset = cfg.RuleSet, map = cfg.MapScript, size = cfg.MapSize,
			difficulty = cfg.Difficulty, speed = cfg.GameSpeed,
			map_seed = cfg.MapSeed, game_seed = cfg.GameSeed,
			leader = cfg.Leader,
			max_turns = cfg.MaxTurns, humans = cfg.HumanPlayers or 1,
		},
		applied = {
			ruleset = try(function() return GameConfiguration.GetRuleSet(); end),
			map = try(function() return MapConfiguration.GetScript(); end),
			size = try(function() return MapConfiguration.GetMapSize(); end),
			difficulty = try(function() return GameConfiguration.GetHandicapType(); end),
			speed = try(function() return GameConfiguration.GetGameSpeedType(); end),
			max_turns = try(function() return GameConfiguration.GetMaxTurns(); end),
			humans = try(function() return GameConfiguration.GetHumanPlayerCount(); end),
			leader = try(function()
				local ids = GameConfiguration.GetHumanPlayerIDs();
				return ids[1] and PlayerConfigurations[ids[1]]:GetLeaderTypeName() or nil;
			end),
			participants = try(function()
				return GameConfiguration.GetParticipatingPlayerCount();
			end),
			seated = seated,
		},
	});
end

-- ------------------------------------------------------------------- firing

local fired = false;
local frames = 0;

-- Waiting a fixed number of frames rather than hooking a "menu ready" event.
-- The obvious event, LoadGameViewStateDone, is raised before a mod context
-- exists, so a handler added here never runs and everything behind it silently
-- never happens. A frame count cannot miss an event it was too late to hear.
local function onUpdate()
	if fired then return; end
	frames = frames + 1;
	if frames < (cfg.StartDelayFrames or 240) then return; end
	fired = true;

	if cfg.AutoStart == false then
		emit("idle", { reason = "AutoStart is false" });
		return;
	end

	-- Whether this context runs at all is not observable from the main menu:
	-- there is no Lua.log on this build, and Automation.log stays empty until
	-- a game exists, so a context that never loaded and one that loaded and
	-- threw look identical -- both are a game sitting at the menu.
	--
	-- So the outcome is carried *into* the game instead. GameConfiguration
	-- values survive into the session, the agent reads this one back on its
	-- first turn, and the three cases separate cleanly: no game at all means
	-- the context never ran; a game whose marker says "ok" means it ran and
	-- configured; a game whose marker carries an error means it ran, failed to
	-- configure, and hosted anyway -- which is why hosting is not conditional
	-- on the configuration succeeding.
	local ok, err = pcall(applyConfiguration);
	-- Not `ok and nil or tostring(err)`: nil is falsy, so that idiom falls
	-- through to the second branch on success and reports pcall's return value
	-- as an error message.
	local failure = nil;
	if not ok then failure = tostring(err); end
	pcall(function()
		GameConfiguration.SetValue("CIVVIS_SETUP", ok and "ok" or failure);
	end);
	if not ok then
		emit("error", { where = "applyConfiguration", error = failure });
	end
	local hosted, hostErr = pcall(function()
		Network.HostGame(ServerType.SERVER_TYPE_NONE);
	end);
	local hostFailure = nil;
	if not hosted then hostFailure = tostring(hostErr); end
	emit("host", { started = hosted, configured = ok, error = hostFailure });
end

function Initialize()
	emit("loaded", { version = 1, autostart = cfg.AutoStart ~= false });
	ContextPtr:SetUpdate(onUpdate);
end

Initialize();
