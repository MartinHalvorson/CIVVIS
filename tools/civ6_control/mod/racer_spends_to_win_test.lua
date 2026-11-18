-- ⚠ lupa's Lua 5.1.5 (the local runner) barriers the CLOSURE in
-- lua_setupvalue, not the shared UpVal; a full collection first leaves
-- nothing marked (see congress_claim_denial_test.lua). Harmless under the
-- real lua5.1 CI runs.
do
	local setupvalue = debug.setupvalue
	debug.setupvalue = function(...)
		collectgarbage("collect")
		return setupvalue(...)
	end
end

-- Run: lua5.1 tools/civ6_control/mod/racer_spends_to_win_test.lua
-- The Diplomatic racer's last stretch: the brace from 16, the bank at a
-- session with no claim to save for, and the vote that goes with a field
-- voting as one. The five sessions below are live ballots of 2026-11-18,
-- with every rival's recorded vote: the production ballot is cast into each
-- and scored the way the Congress scores it (the option with more votes,
-- then the target with the most votes on it, a tie a loss; one point for a
-- matching option and target, +2 / -2 to the Diplomatic Victory target).
local here = arg[0]:match("(.*)/[^/]*$") or "."
local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end
setmetatable(_G, { __index = function() return stub() end })
assert(pcall(assert(loadfile(here .. "/CivvisControlAgent.lua"))))
local function upvalue(fn, key, replacement)
	for i = 1, 100 do
		local name, value = debug.getupvalue(fn, i)
		if name == nil then break end
		if name == key then
			if replacement ~= nil then debug.setupvalue(fn, i, replacement) end
			return value
		end
	end
	error("missing upvalue " .. key)
end
local tick = upvalue(CivvisQueue.onUiPulse, "tick")
-- The live seat's configuration: the Diplomatic lane, the counter
-- resolutions on (the default), `congress-guards-the-leader` armed.
local config = { Play = true, CivvisDecides = false, VictoryLane = "diplomatic",
	DiploVictoryGuardLeader = true }
upvalue(tick, "cfg", config)
local hooks = {}
Events = setmetatable({ WorldCongressStage1 = { Add = function(fn) hooks.stage = fn end } },
	{ __index = function() return stub() end })
LuaEvents = setmetatable({ CivvisCongressBallot = { Add = function(fn) hooks.popup = fn end } },
	{ __index = function() return stub() end })
Automation = { Log = function() end }

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print("ok " .. name)
	end
end

local DD, FASCISM = "LOC_GOVERNMENT_DIGITAL_DEMOCRACY_NAME", "LOC_GOVERNMENT_FASCISM_NAME"
local SYNTH, MONARCHY = "LOC_GOVERNMENT_SYNTHETIC_TECHNOCRACY_NAME", "LOC_GOVERNMENT_MONARCHY_NAME"
local DEMOCRACY, OLIGARCHY = "LOC_GOVERNMENT_DEMOCRACY_NAME", "LOC_GOVERNMENT_OLIGARCHY_NAME"
local GOVERNMENTS = { "LOC_GOVERNMENT_AUTOCRACY_NAME", OLIGARCHY, "LOC_GOVERNMENT_CLASSICAL_REPUBLIC_NAME",
	MONARCHY, "LOC_GOVERNMENT_THEOCRACY_NAME", "LOC_GOVERNMENT_MERCHANT_REPUBLIC_NAME", FASCISM,
	"LOC_GOVERNMENT_COMMUNISM_NAME", DEMOCRACY, "LOC_GOVERNMENT_CORPORATE_LIBERTARIANISM_NAME", DD, SYNTH }
local PLAYERS = { 0, 1, 2, 3, 4, 5 }
local PLANTS = { "LOC_BUILDING_POWER_PLANT_EXPANSION2_NAME", "LOC_BUILDING_COAL_POWER_PLANT_NAME",
	"LOC_BUILDING_FOSSIL_FUEL_POWER_PLANT_NAME" }
local YIELDS = { "LOC_YIELD_PRODUCTION_NAME", "LOC_YIELD_GOLD_NAME", "LOC_YIELD_FAITH_NAME" }
local LUXURIES = { "LOC_RESOURCE_COCOA_NAME", "LOC_RESOURCE_DYES_NAME", "LOC_RESOURCE_FURS_NAME",
	"LOC_RESOURCE_INCENSE_NAME", "LOC_RESOURCE_MARBLE_NAME", "LOC_RESOURCE_SILK_NAME",
	"LOC_RESOURCE_SILVER_NAME", "LOC_RESOURCE_SUGAR_NAME", "LOC_RESOURCE_TEA_NAME",
	"LOC_RESOURCE_WINE_NAME", "LOC_RESOURCE_AMBER_NAME", "LOC_RESOURCE_TURTLES_NAME",
	"LOC_RESOURCE_HONEY_NAME" }
local WORKS = { "LOC_GREAT_WORK_OBJECT_SCULPTURE_NAME", "LOC_GREAT_WORK_OBJECT_PORTRAIT_NAME",
	"LOC_GREAT_WORK_OBJECT_LANDSCAPE_NAME", "LOC_GREAT_WORK_OBJECT_RELIGIOUS_NAME",
	"LOC_GREAT_WORK_OBJECT_ARTIFACT_NAME", "LOC_GREAT_WORK_OBJECT_WRITING_NAME",
	"LOC_GREAT_WORK_OBJECT_MUSIC_NAME", "LOC_GREAT_WORK_OBJECT_RELIC_NAME" }
local WRITING = "LOC_GREAT_WORK_OBJECT_WRITING_NAME"

-- Each rival vote is { player, option, votes, target }.
local FIXTURES = {
	-- civvis-20261118T055703Z (Gaul, Deity, lost on 18 of 20 at t191).
	-- t148: 10 points, level with the best rival; the claim carried.
	t148 = { favor = 160, maxVotes = 9, points = { [0] = 10, 10, 4, 6, 5, 5 },
		score = { 1049, 1085, 751, 1104, 798 }, culture = { 37.1, 40.7, 17.9, 18.9, 7.1 },
		government = { DD, SYNTH, FASCISM, SYNTH, MONARCHY }, science = {},
		tally = { wc_rival_block = 5, wc_rival_block_prev = 7, wc_b_on_us = 0, wc_b_passed_on_us = 0,
			wc_dvp_won = 1, wc_review_favor = 14 },
		session = {
			{ "WC_RES_DIPLOVICTORY", "PlayerType", PLAYERS, { { 1, 1, 2, 1 }, { 2, 1, 2, 2 },
				{ 3, 1, 4, 3 }, { 4, 1, 1, 4 }, { 5, 1, 6, 5 } } },
			{ "WC_RES_GLOBAL_ENERGY_TREATY", "BuildingType", PLANTS, { { 1, 1, 1, PLANTS[2] },
				{ 2, 1, 1, PLANTS[2] }, { 3, 2, 1, PLANTS[1] }, { 4, 1, 1, PLANTS[3] }, { 5, 1, 1, PLANTS[2] } } },
			{ "WC_RES_MERCENARY_COMPANIES", "YieldType", YIELDS, { { 1, 2, 6, YIELDS[1] },
				{ 2, 2, 6, YIELDS[1] }, { 3, 2, 9, YIELDS[1] }, { 4, 2, 1, YIELDS[2] }, { 5, 2, 8, YIELDS[2] } } },
		} },
	-- t168: 16 points and leading; four rivals cast 39 B votes on us. Live:
	-- 11 A votes (220 Favor asked) lost 13 to 39 and both other resolutions
	-- got the free vote: -2.
	t168 = { favor = 289, maxVotes = 12, points = { [0] = 16, 13, 7, 7, 5, 6 },
		score = { 1144, 1236, 841, 1342, 841 }, culture = { 64.6, 60.8, 14.0, 45.8, 8.9 },
		government = { DD, DD, FASCISM, DEMOCRACY, MONARCHY }, science = {},
		tally = { wc_rival_block = 6, wc_rival_block_prev = 5, wc_b_on_us = 0, wc_b_passed_on_us = 0,
			wc_dvp_won = 1, wc_review_favor = 100 },
		session = {
			{ "WC_RES_DIPLOVICTORY", "PlayerType", PLAYERS, { { 1, 2, 6, 0 }, { 2, 2, 9, 0 },
				{ 3, 1, 2, 3 }, { 4, 2, 13, 0 }, { 5, 2, 11, 0 } } },
			{ "WC_RES_LUXURY", "ResourceType", LUXURIES, { { 1, 2, 4, "LOC_RESOURCE_INCENSE_NAME" },
				{ 2, 1, 4, "LOC_RESOURCE_SILK_NAME" }, { 3, 2, 6, "LOC_RESOURCE_AMBER_NAME" },
				{ 4, 1, 1, "LOC_RESOURCE_SUGAR_NAME" }, { 5, 1, 5, "LOC_RESOURCE_TURTLES_NAME" } } },
			{ "WC_RES_TRADE_TREATY", "PlayerType", PLAYERS, { { 1, 1, 6, 2 }, { 2, 1, 4, 1 },
				{ 3, 1, 1, 3 }, { 4, 1, 1, 4 }, { 5, 2, 1, 2 } } },
		} },
	-- t188: 14 points, a rival on 15 with its Exoplanet Expedition in flight;
	-- three rivals cast 18 B votes on it. Live: 11 A votes carried 19 to 18,
	-- World Ideology's free vote went to Fascism against 37 votes for Digital
	-- Democracy, and 343 Favor stayed in the bank: +4.
	t188 = { favor = 587, maxVotes = 17, points = { [0] = 14, 15, 11, 7, 8, 8 },
		score = { 1234, 1455, 868, 1515, 829 }, culture = { 60.2, 63.6, 14.5, 69.5, 11.0 },
		government = { DD, DD, FASCISM, DD, FASCISM }, science = { 16, 12, 0, 14, 0 },
		tally = { wc_rival_block = 2, wc_rival_block_prev = 6, wc_b_on_us = 39, wc_b_passed_on_us = 1,
			wc_dvp_won = 2, wc_review_favor = 446 },
		session = {
			{ "WC_RES_ARMS_CONTROL", "PlayerType", PLAYERS, { { 1, 1, 1, 4 }, { 2, 1, 1, 2 },
				{ 3, 1, 1, 2 }, { 4, 1, 1, 0 }, { 5, 1, 1, 5 } } },
			{ "WC_RES_DIPLOVICTORY", "PlayerType", PLAYERS, { { 1, 1, 4, 1 }, { 2, 2, 7, 1 },
				{ 3, 1, 4, 3 }, { 4, 2, 7, 1 }, { 5, 2, 4, 1 } } },
			{ "WC_RES_WORLD_IDEOLOGY", "GovernmentType", GOVERNMENTS, { { 1, 1, 8, DD }, { 2, 1, 14, DD },
				{ 3, 1, 10, FASCISM }, { 4, 1, 15, DD }, { 5, 2, 1, DD } } },
		} },
	-- civvis-20261118T061033Z (Hungary, Deity, WON on 21 points at t183).
	-- t174: 13 points, level with a rival on 13; 13 A votes carried.
	won174 = { favor = 329, maxVotes = 13, points = { [0] = 13, 5, 5, 7, 13, 9 },
		score = { 961, 1014, 1276, 1177, 1318 }, culture = { 33.7, 2.2, 88.4, 15.7, 18.8 },
		government = { DD, DD, DD, MONARCHY, DD }, science = {},
		tally = { wc_rival_block = 5, wc_rival_block_prev = 11, wc_b_on_us = 0, wc_b_passed_on_us = 0,
			wc_dvp_won = 1, wc_review_favor = 57 },
		session = {
			{ "WC_RES_DIPLOVICTORY", "PlayerType", PLAYERS, { { 1, 1, 1, 1 }, { 2, 1, 3, 2 },
				{ 3, 1, 4, 3 }, { 4, 1, 10, 4 }, { 5, 1, 2, 5 } } },
			{ "WC_RES_MIGRATION_TREATY", "PlayerType", PLAYERS, { { 1, 1, 1, 0 }, { 2, 1, 3, 2 },
				{ 3, 1, 1, 3 }, { 4, 1, 4, 4 }, { 5, 1, 1, 5 } } },
			{ "WC_RES_WORLD_IDEOLOGY", "GovernmentType", GOVERNMENTS, { { 1, 1, 7, DD }, { 2, 1, 7, DD },
				{ 3, 1, 13, DD }, { 4, 1, 10, DEMOCRACY }, { 5, 1, 12, DD } } },
		} },
	-- civvis-20261118T063654Z (Gaul, Deity, lost on 15 at t173 with 381 Favor
	-- in hand). t171: 15 points, warned by 19 B votes the session before;
	-- all five rivals cast B on us. Live: the brace, a Heritage denial of
	-- four votes that lost to the rivals' five, 0 net.
	lost171 = { favor = 359, maxVotes = 13, points = { [0] = 15, 7, 9, 12, 8, 8 },
		score = { 1064, 839, 1317, 1358, 1273 }, culture = { 5.2, 4.1, 39.7, 47.2, 28.1 },
		government = { FASCISM, OLIGARCHY, SYNTH, SYNTH, DD }, science = { 0, 0, 0, 0, 8 },
		heritage = { [WRITING] = 100 },
		tally = { wc_rival_block = 9, wc_rival_block_prev = 3, wc_b_on_us = 19, wc_b_passed_on_us = 0,
			wc_dvp_won = 1, wc_review_favor = 79 },
		session = {
			{ "WC_RES_DIPLOVICTORY", "PlayerType", PLAYERS, { { 1, 2, 4, 0 }, { 2, 2, 2, 0 },
				{ 3, 2, 7, 0 }, { 4, 2, 4, 0 }, { 5, 2, 7, 0 } } },
			{ "WC_RES_HERITAGE_ORG", "GreatWorkObjectType", WORKS, { { 1, 1, 1, WORKS[8] },
				{ 2, 1, 1, WORKS[3] }, { 3, 1, 1, WRITING }, { 4, 1, 1, WRITING }, { 5, 1, 1, WRITING } } },
			{ "WC_RES_MERCENARY_COMPANIES", "YieldType", YIELDS, { { 1, 2, 10, YIELDS[1] },
				{ 2, 2, 3, YIELDS[1] }, { 3, 1, 12, YIELDS[3] }, { 4, 2, 9, YIELDS[1] }, { 5, 2, 18, YIELDS[1] } } },
		} },
}

local F = FIXTURES.t148
local wc = {
	GetVotesandFavorCost = function()
		-- The Online table, as every recorded `wc_ballot_verdict.costs` reads.
		local costs = { MaxVotes = F.maxVotes }
		for k = 0, F.maxVotes do costs[k] = 2 * (k + 1) * k end
		return costs
	end,
	GetResolutions = function()
		local list = { Stage = 2147483647 }
		for i, row in ipairs(F.session) do
			list[i] = { Type = row[1], TargetType = row[2], PossibleTargets = row[3] }
		end
		return list
	end,
}
Game = {
	GetLocalPlayer = function() return 0 end,
	GetCurrentGameTurn = function() return 168 end,
	GetCurrentTurnSegment = function() return "TURNSEG_WORLDCONGRESS_1" end,
	GetWorldCongress = function() return wc end,
}
DB = { MakeHash = function(name) return name end }
Players = {}
for id = 0, 5 do
	local who = id
	Players[id] = {
		IsTurnActive = function() return false end,
		GetFavor = function() return who == 0 and F.favor or nil end,
		GetFavorEnteringCongress = function() return F.favor end,
		GetStats = function() return {
			GetDiplomaticVictoryPoints = function() return F.points[who] end,
			GetScienceVictoryPoints = function() return F.science[who] or 0 end,
		} end,
		GetScore = function() return who == 0 and 0 or F.score[who] end,
		-- The culture race as the ballot computes it: tourists over the
		-- highest domestic count the rival must clear (ours, 1000).
		GetCulture = function() return {
			GetStaycationers = function() return who == 0 and 1000 or 0 end,
			GetTouristsTo = function() return who == 0 and 0 or F.culture[who] * 10 end,
			GetCurrentGovernment = function() return who end,
		} end,
		GetDiplomacy = function() return { HasMet = function() return true end } end,
	}
end
PlayerManager = { GetAliveMajorIDs = function() return { 0, 1, 2, 3, 4, 5 } end }
GameInfo = {
	Resolutions = setmetatable({}, { __index = function(_, name)
		return { ResolutionType = name, Hash = name }
	end }),
	Governments = setmetatable({}, { __index = function(_, who)
		return F.government[who] and { Name = F.government[who] } or nil
	end }),
}
PlayerOperations = {
	PARAM_RESOLUTION_TYPE = "type", PARAM_WORLD_CONGRESS_VOTES = "votes",
	PARAM_RESOLUTION_OPTION = "option", PARAM_RESOLUTION_SELECTION = "selection",
	WORLD_CONGRESS_RESOLUTION_VOTE = "vote", WORLD_CONGRESS_SUBMIT_TURN = "submit",
}
local requests = {}
UI = { RequestPlayerOperation = function(pid, operation, params)
	assert(pid == 0)
	if operation == "vote" then requests[params.type] = params end
end }
tick()
local cast = upvalue(hooks.popup, "castBallot")
local vote = upvalue(cast, "voteWorldCongress")
local tally = upvalue(vote, "envoyTally")
-- The culture leader's tourism by Great Work type is read from its cities:
-- held as the live denial chose (civvis-20261118T063654Z t171: Writing).
CivvisHeritageTourismByType = function() return F.heritage or {} end

-- The Congress's own count.
local function resolve(voters)
	local a, b = 0, 0
	for _, v in ipairs(voters) do
		if v[2] == 1 then a = a + v[3] else b = b + v[3] end
	end
	if a == b then return 0, nil end
	local option, by, top, winner, tied = (a > b) and 1 or 2, {}, 0, nil, false
	for _, v in ipairs(voters) do
		if v[2] == option then by[v[4]] = (by[v[4]] or 0) + v[3] end
	end
	for target, n in pairs(by) do
		if n > top then top, winner, tied = n, target, false
		elseif n == top then tied = true end
	end
	if tied then return option, nil end
	return option, winner
end

-- Cast the production ballot into a fixture. Returns the points it gains,
-- the Favor it asks for and pays (a matched vote in full, the winning option
-- on another target half, a losing option nothing), the mode and the votes.
local function play(name, switches)
	F = FIXTURES[name]
	for key in pairs(tally) do
		if string.sub(key, 1, 3) == "wc_" then tally[key] = nil end
	end
	for key, value in pairs(F.tally) do tally[key] = value end
	for key, value in pairs(switches or {}) do config[key] = value end
	requests = {}
	local _, spent, _, _, _, _, mode = vote(0)
	for key in pairs(switches or {}) do config[key] = nil end
	local out = { mode = mode, asked = spent, points = 0, paid = 0 }
	for _, row in ipairs(F.session) do
		local q = requests[row[1]]
		local target = row[3][q.selection + 1]
		local voters = { { 0, q.option, q.votes, target } }
		for _, v in ipairs(row[4]) do voters[#voters + 1] = v end
		local option, winner = resolve(voters)
		local matched = option == q.option and winner == target
		local price = 2 * q.votes * (q.votes - 1)
		if matched then
			out.points = out.points + 1
			out.paid = out.paid + price
		elseif option == q.option then
			out.paid = out.paid + price / 2
		end
		if row[1] == "WC_RES_DIPLOVICTORY" and winner == 0 then
			out.points = out.points + ((option == 1) and 2 or -2)
		end
		out[row[1]] = string.format("%s%d>%s%s", (q.option == 1) and "A" or "B", q.votes,
			tostring(target), matched and "*" or "")
	end
	return out
end

-- ★ t168, the gang. The free vote goes with the outcome that wins (B on
-- us), the claim's 220 Favor is not asked for, and the bank carries the
-- Trade Treaty (twelve votes; seven were needed against a block of six).
-- The Luxury resolution is a long shot and takes what that leaves.
local r = play("t168")
check("t168: the ballot braces", r.mode, "brace")
check("t168: B on ourselves with the free vote, matched", r.WC_RES_DIPLOVICTORY, "B1>0*")
check("t168: the Trade Treaty is carried, A on us", r.WC_RES_TRADE_TREATY, "A12>0*")
check("t168: the long shot takes what the treaty leaves", r.WC_RES_LUXURY, "A4>LOC_RESOURCE_COCOA_NAME")
check("t168: 0 net, where the live ballot lost 2", r.points, 0)
check("t168: no more Favor asked than the bank holds", r.asked <= 289, true)
check("t168: Favor asked", r.asked, 264 + 24)
-- With the brace at its old level the live ballot comes back: -2.
r = play("t168", { DiploRacerGangSelf = 18, RacerEndgame = false, RacerJoinsTheField = false })
check("t168, old rules: the claim into the gang", r.WC_RES_DIPLOVICTORY, "A11>0")
check("t168, old rules: the lone counter vote", r.WC_RES_TRADE_TREATY, "B1>1")
check("t168, old rules: -2", r.points, -2)
-- The brace alone (the bank and the field's vote off) is -1, the Favor kept.
r = play("t168", { RacerEndgame = false, RacerJoinsTheField = false })
check("t168, the brace alone: -1", r.points, -1)
check("t168, the brace alone: nothing asked", r.asked, 0)

-- ★ t188, a rival is the one the gang is on. The claim takes the bank less
-- the Arms Control count (17 votes; 11 carried it by one), and World
-- Ideology's free vote goes to the government three of five rivals hold.
r = play("t188")
check("t188: the ballot claims", r.mode, "race")
check("t188: the claim takes the bank and carries", r.WC_RES_DIPLOVICTORY, "A17>0*")
check("t188: Arms Control carried", r.WC_RES_ARMS_CONTROL, "A5>0*")
check("t188: World Ideology joins the landslide with the free vote",
	r.WC_RES_WORLD_IDEOLOGY, "A1>" .. DD .. "*")
check("t188: +5, where the live ballot made +4", r.points, 5)
check("t188: no more Favor asked than the bank holds", r.asked <= 587, true)
r = play("t188", { DiploRacerGangSelf = 18, RacerEndgame = false, RacerJoinsTheField = false })
check("t188, old rules: eleven votes", r.WC_RES_DIPLOVICTORY, "A11>0*")
check("t188, old rules: Fascism", r.WC_RES_WORLD_IDEOLOGY, "A1>" .. FASCISM)
check("t188, old rules: +4", r.points, 4)

-- ★ t148, an ordinary session: nothing changes but the Global Energy vote,
-- which goes with the rivals' option (A; three votes are what 16 Favor buys,
-- and tie the rivals' three for the Coal Power Plant: a loss here).
r = play("t148")
check("t148: the same claim", r.WC_RES_DIPLOVICTORY, "A9>0*")
check("t148: the same Mercenary Companies vote", r.WC_RES_MERCENARY_COMPANIES, "B2>LOC_YIELD_PRODUCTION_NAME*")
check("t148: Global Energy votes with the field", r.WC_RES_GLOBAL_ENERGY_TREATY, "A3>" .. PLANTS[1])
check("t148: +4, as live", r.points, 4)
check("t148: no more Favor asked than the bank holds", r.asked <= 160, true)

-- ★ The game that was won (Hungary, t174): the claim that carried it is
-- cast unchanged, and World Ideology's free vote now matches.
r = play("won174")
check("won t174: the same thirteen-vote claim", r.WC_RES_DIPLOVICTORY, "A13>0*")
check("won t174: World Ideology joins the landslide", r.WC_RES_WORLD_IDEOLOGY, "A1>" .. DD .. "*")
check("won t174: the lone counter vote is the racer's own vote now", r.WC_RES_MIGRATION_TREATY, "A1>0")
check("won t174: +4, where the live ballot made +3", r.points, 4)

-- ★ A warned brace at 15 (lost t171): the denial of four votes that five
-- rivals outvote is not cast; the free vote goes with them, on Writing.
r = play("lost171")
check("lost t171: the ballot braces", r.mode, "brace")
check("lost t171: Heritage joins the rivals on Writing", r.WC_RES_HERITAGE_ORG, "A1>" .. WRITING .. "*")
check("lost t171: +1, where the live ballot made 0", r.points, 1)
r = play("lost171", { RacerJoinsTheField = false })
check("lost t171, switch off: the four-vote denial", r.WC_RES_HERITAGE_ORG, "B4>" .. WRITING)

-- The pure pieces.
local racer = { VictoryLane = "diplomatic" }
local function none() return 0 end
check("endgame: 14 and 14 is not the last stretch",
	CivvisRacerEndgame(14, 14, { { id = 1 } }, racer, none), false)
check("endgame: our 15", CivvisRacerEndgame(15, 8, {}, racer, none), true)
check("endgame: a rival's 15", CivvisRacerEndgame(8, 15, {}, racer, none), true)
check("endgame: a rival's Exoplanet Expedition in flight",
	CivvisRacerEndgame(8, 9, { { id = 1 }, { id = 2 } }, racer,
		function(id) return id == 2 and 6 or 0 end), true)
check("endgame: a reading that raises is no flight",
	CivvisRacerEndgame(8, 9, { { id = 1 } }, racer, function() error("no stats") end), false)
check("endgame: its own level", CivvisRacerEndgame(15, 8, {}, { VictoryLane = "diplomatic",
	DiploVictoryGangFloor = 16 }, none), false)
check("endgame: off by its switch", CivvisRacerEndgame(19, 19, {}, { VictoryLane = "diplomatic",
	RacerEndgame = false }, none), false)
check("endgame: another lane", CivvisRacerEndgame(19, 19, {}, { VictoryLane = "culture" }, none), false)
check("endgame: a Science seat racing the Congress", CivvisRacerEndgame(15, 8, {},
	{ VictoryLane = "science", ScienceCongressRacer = true }, none), true)

local five = { { id = 1 }, { id = 2 }, { id = 3 }, { id = 4 }, { id = 5 } }
local function governments(list)
	return function(id) return list[id] end
end
check("landslide: the government most rivals hold", CivvisRivalsGovernment("WC_RES_WORLD_IDEOLOGY",
	five, 0, racer, governments({ DD, DD, FASCISM, DD, FASCISM })), DD)
check("landslide: two of five is still the most", CivvisRivalsGovernment("WC_RES_WORLD_IDEOLOGY",
	five, 0, racer, governments({ SYNTH, SYNTH, FASCISM, DD, MONARCHY })), SYNTH)
check("landslide: two governments level is none", CivvisRivalsGovernment("WC_RES_WORLD_IDEOLOGY",
	five, 0, racer, governments({ DD, DD, FASCISM, FASCISM, SYNTH })), nil)
check("landslide: a rival not read is not counted", CivvisRivalsGovernment("WC_RES_WORLD_IDEOLOGY",
	five, 0, racer, governments({ DD, nil, FASCISM, nil, nil })), nil)
check("landslide: a reading that raises is none", CivvisRivalsGovernment("WC_RES_WORLD_IDEOLOGY",
	five, 0, racer, function() error("no culture") end), nil)
check("landslide: our own seat is not a rival", CivvisRivalsGovernment("WC_RES_WORLD_IDEOLOGY",
	{ { id = 0 }, { id = 1 } }, 0, racer, governments({ [0] = DD, FASCISM })), FASCISM)
check("landslide: only World Ideology", CivvisRivalsGovernment("WC_RES_PATRONAGE",
	five, 0, racer, governments({ DD, DD, DD, DD, DD })), nil)
check("landslide: off by its switch", CivvisRivalsGovernment("WC_RES_WORLD_IDEOLOGY", five, 0,
	{ VictoryLane = "diplomatic", RacerJoinsTheField = false }, governments({ DD, DD, DD, DD, DD })), nil)
check("landslide: another lane", CivvisRivalsGovernment("WC_RES_WORLD_IDEOLOGY", five, 0,
	{ VictoryLane = "culture" }, governments({ DD, DD, DD, DD, DD })), nil)

local function pick(rtype, targets, cfg, option, selection, landslide)
	local o, s = CivvisRacerUsualWinner(rtype, targets, cfg, option or 1, selection or 1, landslide)
	return tostring(o) .. "/" .. tostring(s)
end
check("usual winner: the landslide over Fascism", pick("WC_RES_WORLD_IDEOLOGY", GOVERNMENTS, racer, 1, 1, DD), "1/11")
check("usual winner: no landslide, Fascism", pick("WC_RES_WORLD_IDEOLOGY", GOVERNMENTS, racer, 1, 1, nil), "1/7")
check("usual winner: a landslide not on the ballot, Fascism",
	pick("WC_RES_WORLD_IDEOLOGY", GOVERNMENTS, racer, 1, 1, "LOC_GOVERNMENT_CHIEFDOM_NAME"), "1/7")
check("usual winner: Heritage names Writing", pick("WC_RES_HERITAGE_ORG", WORKS, racer), "1/6")
check("usual winner: a Heritage denial keeps its B", pick("WC_RES_HERITAGE_ORG", WORKS, racer, 2, 7), "2/7")
check("usual winner: Global Energy takes option A on the target it had",
	pick("WC_RES_GLOBAL_ENERGY_TREATY", PLANTS, racer, 2, 1), "1/1")
check("usual winner: Deforestation takes option A on the Rainforest",
	pick("WC_RES_DEFORESTATION_TREATY", { "LOC_FEATURE_FOREST_NAME", "LOC_FEATURE_JUNGLE_NAME",
		"LOC_FEATURE_MARSH_NAME" }, racer, 2, 1), "1/2")
check("usual winner: Deforestation without the Rainforest, option A on the target it had",
	pick("WC_RES_DEFORESTATION_TREATY", { "LOC_FEATURE_FOREST_NAME", "LOC_FEATURE_MARSH_NAME" }, racer, 2, 2), "1/2")
check("usual winner: Mercenary Companies keeps its B", pick("WC_RES_MERCENARY_COMPANIES", YIELDS, racer, 2, 1), "2/1")
local off = { VictoryLane = "diplomatic", RacerJoinsTheField = false }
check("usual winner, switch off: Fascism over the landslide", pick("WC_RES_WORLD_IDEOLOGY", GOVERNMENTS, off, 1, 1, DD), "1/7")
check("usual winner, switch off: Heritage keeps the first target", pick("WC_RES_HERITAGE_ORG", WORKS, off), "1/1")
check("usual winner, switch off: Global Energy keeps its B", pick("WC_RES_GLOBAL_ENERGY_TREATY", PLANTS, off, 2, 1), "2/1")
check("usual winner: another lane keeps Global Energy's B",
	pick("WC_RES_GLOBAL_ENERGY_TREATY", PLANTS, { VictoryLane = "culture" }, 2, 1), "2/1")

check("denial: four votes against five rivals is not cast", CivvisRacerKeepsDenial(4, five, racer), false)
check("denial: five against five is not cast", CivvisRacerKeepsDenial(5, five, racer), false)
check("denial: six against five stands", CivvisRacerKeepsDenial(6, five, racer), true)
check("denial: another lane's is its own", CivvisRacerKeepsDenial(4, five, { VictoryLane = "science" }), true)
check("denial: the switch keeps it", CivvisRacerKeepsDenial(4, five, off), true)

local costs = { MaxVotes = 20 }
for k = 0, 20 do costs[k] = 2 * (k + 1) * k end
local function count(rtype, favor, cfg)
	local v, p = CivvisRacerResolutionVotes(rtype, favor, costs, cfg or {})
	return v .. "x" .. p
end
check("counts: Sovereignty takes four", count("WC_RES_SOVEREIGNTY", 100), "4x24")
check("counts: Urban Development takes four", count("WC_RES_URBAN_DEVELOPMENT", 100), "4x24")
check("counts: a bank short of four buys three", count("WC_RES_GLOBAL_ENERGY_TREATY", 16), "3x12")
check("counts: or two", count("WC_RES_ARMS_CONTROL", 5), "2x4")
check("counts: or the free vote", count("WC_RES_ARMS_CONTROL", 3), "1x0")
check("counts: three short of its price buys two", count("WC_RES_PUBLIC_RELATIONS", 8), "2x4")
check("counts: a treaty short of its six still keeps the free vote", count("WC_RES_TRADE_TREATY", 50), "1x0")

local function bank(rtype, entries, favor, points, spends, rivals, want, price)
	local v, p = CivvisRacerMatchPointVotes(rtype, entries, rtype, favor, costs, racer, points,
		want or 1, price or 0, spends, rivals)
	return v .. "x" .. p
end
local PAIR = { "WC_RES_LUXURY", "WC_RES_TRADE_TREATY" }
check("bank: below match point and not spending, unchanged", bank("WC_RES_TRADE_TREATY", PAIR, 289, 16, false, 5, 6, 60), "6x60")
check("bank: a spending session takes the bank below match point", bank("WC_RES_TRADE_TREATY", PAIR, 289, 16, true, 5, 6, 60), "12x264")
check("bank: a long shot keeps back twice the later treaty's count", bank("WC_RES_LUXURY", PAIR, 289, 16, true, 5), "4x24")
check("bank: a long shot with nothing after it takes the bank",
	bank("WC_RES_LUXURY", { "WC_RES_LUXURY" }, 289, 16, true, 5), "12x264")
check("bank: a one-vote resolution stops one vote past the rivals",
	bank("WC_RES_ARMS_CONTROL", { "WC_RES_ARMS_CONTROL" }, 300, 18, false, 5, 4, 24), "6x60")
check("bank: without the rivals it stops at twice its count",
	bank("WC_RES_ARMS_CONTROL", { "WC_RES_ARMS_CONTROL" }, 300, 18, false, nil, 4, 24), "8x112")
check("bank: a treaty is not held to the rivals' number",
	bank("WC_RES_MIGRATION_TREATY", { "WC_RES_MIGRATION_TREATY" }, 1000, 18, false, 5, 6, 60), "12x264")
check("bank: never below its own count", bank("WC_RES_ARMS_CONTROL", { "WC_RES_ARMS_CONTROL" }, 300, 18, false, 2, 1, 0), "4x24")

check("claim: all in on the flag", CivvisRacerClaimVotes(587, 14, 6, costs, 20, {}, true), 17)
check("claim: paced without it", CivvisRacerClaimVotes(587, 14, 6, costs, 20, {}), 11)
check("claim: all in buys nothing from an empty bank", CivvisRacerClaimVotes(3, 14, 6, costs, 20, {}, true), nil)
check("reserve: nothing beside a paced claim", CivvisRacerReserve({ "WC_RES_DIPLOVICTORY",
	"WC_RES_ARMS_CONTROL", "WC_RES_WORLD_IDEOLOGY" }, costs, {}), 0)
check("reserve: the ordinary counts beside a claim that takes the bank", CivvisRacerReserve({
	"WC_RES_DIPLOVICTORY", "WC_RES_ARMS_CONTROL", "WC_RES_WORLD_IDEOLOGY" }, costs, {}, true), 24)

if failures > 0 then
	print(failures .. " failure(s)")
	os.exit(1)
end
print("racer spends to win: all ok")
