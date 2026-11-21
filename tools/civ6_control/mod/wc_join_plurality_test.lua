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

-- Run: lua5.1 tools/civ6_control/mod/wc_join_plurality_test.lua
-- Opt-in (`WcJoinPlurality`): a seat racing the Congress votes the Migration
-- Treaty, World Ideology and Mercenary Companies with the block predicted to
-- carry them (`CivvisWcJoinPlurality`); the reading is recorded either way
-- as `wc_ballot_context.wc_prediction`. Loads the actual agent: the pure
-- choice, the real ballot unarmed and armed, the paths it leaves alone, a
-- failing arm, and the recorded resolutions it changes, replayed with the
-- recorded rival votes deciding.
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

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print("ok " .. name)
	end
end

local racer = { VictoryLane = "science", ScienceCongressRacer = true, WcJoinPlurality = true }
local online = { [0] = 0, [1] = 4, MaxVotes = 1 }
local function yield(name) return "LOC_YIELD_" .. name .. "_NAME" end
local function gov(name) return "LOC_GOVERNMENT_" .. name .. "_NAME" end
local function choose(q)
	q.config = q.config or racer
	q.costs = q.costs or online
	q.pid = q.pid or 0
	return CivvisWcJoinPlurality(q)
end

-- 1. The session's resolutions on which rivals buy votes.
check("bought kinds are counted", CivvisWcMultiVote({ "WC_RES_DIPLOVICTORY",
	{ rtype = "WC_RES_MIGRATION_TREATY" }, "WC_RES_BORDER_CONTROL" }), 2)
check("an all-one-vote session counts one", CivvisWcMultiVote({ "WC_RES_SOVEREIGNTY" }), 1)
check("no list counts one", CivvisWcMultiVote(nil), 1)

-- 2. Migration Treaty: a rival's block is a quarter of its spend.
local players = { 0, 1, 2, 3, 4 }
local function spenders(spend)
	local out = {}
	for id = 1, 4 do
		out[#out + 1] = spend[id] ~= nil and { id = id, favor = 3, favor_entering = 3 + spend[id] }
			or { id = id, favor = 3 }
	end
	return out
end
-- Rival 2 paid 288: a quarter is 72, six votes; rival 4 paid 40: ten, two.
local field = spenders({ [1] = 24, [2] = 288, [3] = 0, [4] = 40 })
local o, s, n, rec = choose({ rtype = "WC_RES_MIGRATION_TREATY", targets = players,
	candidates = field, option = 1, selection = 1, votes = 1 })
check("a lone vote under a block of six joins", rec.choice, "join")
check("join is A", o, 1)
check("join names the top spender", players[s], 2)
check("join is the free vote", n, 1)
check("the block is recorded", rec.block, 6)
check("the winner is recorded", rec.winner, 2)
check("what it replaced", rec.before_option .. "/" .. rec.before_target .. "/" .. rec.before_votes, "1/0/1")
o, s, n, rec = choose({ rtype = "WC_RES_MIGRATION_TREATY", targets = players,
	candidates = field, option = 1, selection = 1, votes = 6 })
check("a claim of six against six stands", o, nil)
check("and says so", rec.why, "claim_carries")
o, s, n, rec = choose({ rtype = "WC_RES_MIGRATION_TREATY", targets = players,
	candidates = field, option = 1, selection = 1, votes = 5, config = { VictoryLane = "science",
	ScienceCongressRacer = true, WcJoinPlurality = true, WcJoinSpendShare = 0.1 } })
check("the share is a setting (a tenth of 288 is four votes)", rec.block, 4)
check("and a claim of five then stands", o, nil)
o, s, n, rec = choose({ rtype = "WC_RES_MIGRATION_TREATY", targets = players,
	candidates = field, option = 1, selection = 1, votes = 1, row = { DisfavoredPlayerIDs = { 2 } } })
check("a disfavoring rival votes B and is left out", rec.winner ~= 2, true)
check("rivals 1 and 4 tie on two: the lower id, over our one vote", players[s], 1)
o, s, n, rec = choose({ rtype = "WC_RES_MIGRATION_TREATY", targets = players,
	candidates = spenders({ [1] = 40, [2] = 40 }), option = 1, selection = 1, votes = 1 })
check("a tie goes to the lower id", players[s], 1)
o, s, n, rec = choose({ rtype = "WC_RES_MIGRATION_TREATY", targets = players,
	candidates = spenders({}), option = 1, selection = 1, votes = 1 })
check("no Favor entering read, no prediction", o, nil)
check("and says why", rec.why, "no_spend")
o, s, n, rec = choose({ rtype = "WC_RES_MIGRATION_TREATY", targets = players,
	candidates = spenders({ [1] = 2, [2] = 0 }), option = 1, selection = 1, votes = 1 })
check("a spend that buys nobody a vote predicts nothing", rec.why, "no_spend")
o, s, n, rec = choose({ rtype = "WC_RES_MIGRATION_TREATY", targets = players,
	candidates = field, option = 2, selection = 4, votes = 1 })
check("the counter's B on a threat stands", o, nil)
check("it is not a claim", rec.why, "not_a_claim")
o, s, n, rec = choose({ rtype = "WC_RES_MIGRATION_TREATY", targets = players,
	candidates = field, option = 1, selection = 3, votes = 1 })
check("A on someone else is not the claim either", rec.why, "not_a_claim")
o, s, n, rec = choose({ rtype = "WC_RES_MIGRATION_TREATY", targets = { 0, 1, 3, 4 },
	candidates = field, option = 1, selection = 1, votes = 1 })
check("a winner the ballot does not offer is not joined", o, nil)
check("and says why", rec.why, "not_offered")
o, s, n, rec = choose({ rtype = "WC_RES_MIGRATION_TREATY", targets = players, candidates = field,
	option = 1, selection = 1, votes = 1, config = { VictoryLane = "science", ScienceCongressRacer = true } })
check("unarmed: the ballot stands", o, nil)
check("unarmed: what it would cast", rec.would, "join")
check("unarmed: no choice", rec.choice, nil)
check("unarmed: recorded unarmed", rec.armed, false)
o, s, n, rec = choose({ rtype = "WC_RES_MIGRATION_TREATY", targets = players, candidates = field,
	option = 1, selection = 1, votes = 1, config = { VictoryLane = "culture", WcJoinPlurality = true } })
check("a seat not racing the Congress: nothing", o, nil)
check("and no record", rec, nil)
for _, other in ipairs({ "WC_RES_LUXURY", "WC_RES_TRADE_TREATY", "WC_RES_PATRONAGE",
		"WC_RES_PUBLIC_WORKS", "WC_RES_DIPLOVICTORY", "WC_RES_WORLD_RELIGION" }) do
	o, s, n, rec = choose({ rtype = other, targets = players, candidates = field,
		option = 1, selection = 1, votes = 1 })
	check(other .. " is left alone", o == nil and rec == nil, true)
end

-- 3. World Ideology: the government with the largest spend-weighted block.
local ideologies = { gov("FASCISM"), gov("DEMOCRACY"), gov("COMMUNISM") }
local held = { [1] = "COMMUNISM", [2] = "DEMOCRACY", [3] = "COMMUNISM", [4] = "COMMUNISM" }
local function governmentOf(id) return held[id] and gov(held[id]) or nil end
local four = { "WC_RES_DIPLOVICTORY", "WC_RES_MERCENARY_COMPANIES", "WC_RES_MIGRATION_TREATY",
	"WC_RES_WORLD_IDEOLOGY" }
-- Spread over four bought resolutions: rival 2 buys six (72), rivals 1 and 4
-- two each, rival 3 one: Democracy 6 against Communism 5.
o, s, n, rec = choose({ rtype = "WC_RES_WORLD_IDEOLOGY", targets = ideologies, candidates = field,
	option = 1, selection = 3, votes = 1, entries = four, governmentOf = governmentOf })
check("the bigger spender's government over three heads", ideologies[s or 0], gov("DEMOCRACY"))
check("A", o, 1)
check("the ballot's count stands", n, 1)
check("its block", rec.block, 6)
check("four bought resolutions", rec.multi, 4)
o, s, n, rec = choose({ rtype = "WC_RES_WORLD_IDEOLOGY", targets = ideologies, candidates = field,
	option = 1, selection = 3, votes = 3, entries = { "WC_RES_WORLD_IDEOLOGY" }, governmentOf = governmentOf })
check("the whole spend on one resolution: Democracy 12 over Communism", ideologies[s or 0], gov("DEMOCRACY"))
check("three votes stay three", n, 3)
o, s, n, rec = choose({ rtype = "WC_RES_WORLD_IDEOLOGY", targets = ideologies, candidates = spenders({}),
	option = 1, selection = 1, votes = 1, entries = four, governmentOf = governmentOf })
check("spend unread: one vote a head", ideologies[s or 0], gov("COMMUNISM"))
o, s, n, rec = choose({ rtype = "WC_RES_WORLD_IDEOLOGY", targets = ideologies, candidates = field,
	option = 1, selection = 2, votes = 1, entries = four, governmentOf = governmentOf })
check("already on it: nothing to change", o, nil)
check("and says so", rec.why, "already")
o, s, n, rec = choose({ rtype = "WC_RES_WORLD_IDEOLOGY", targets = ideologies, candidates = field,
	option = 2, selection = 1, votes = 1, entries = four, governmentOf = governmentOf })
check("a B vote stands", rec.why, "not_a_claim")
o, s, n, rec = choose({ rtype = "WC_RES_WORLD_IDEOLOGY", targets = { gov("FASCISM") }, candidates = field,
	option = 1, selection = 1, votes = 1, entries = four, governmentOf = governmentOf })
check("governments the ballot does not offer are not counted", rec.why, "no_government")
o, s, n, rec = choose({ rtype = "WC_RES_WORLD_IDEOLOGY", targets = ideologies, candidates = field,
	option = 1, selection = 1, votes = 1, entries = four, governmentOf = function() error("unmet") end })
check("no government read: no prediction", o, nil)
-- Equal blocks: the target listed first.
held = { [1] = "DEMOCRACY", [2] = "COMMUNISM" }
o, s, n, rec = choose({ rtype = "WC_RES_WORLD_IDEOLOGY", targets = ideologies,
	candidates = spenders({ [1] = 40, [2] = 40 }), option = 1, selection = 1, votes = 1,
	entries = { "WC_RES_WORLD_IDEOLOGY" }, governmentOf = governmentOf })
check("a tie goes to the target listed first", ideologies[s or 0], gov("DEMOCRACY"))
held = { [1] = "COMMUNISM", [2] = "DEMOCRACY", [3] = "COMMUNISM", [4] = "COMMUNISM" }

-- 4. Mercenary Companies: the last winning option and target again.
local yields = { yield("PRODUCTION"), yield("GOLD"), yield("FAITH") }
local last = { WC_RES_MERCENARY_COMPANIES = { option = 2, target = yield("GOLD") } }
o, s, n, rec = choose({ rtype = "WC_RES_MERCENARY_COMPANIES", targets = yields, option = 2,
	selection = 1, votes = 2, last = last })
check("B on Gold, as last time", o .. "/" .. yields[s] .. "/" .. n, "2/" .. yield("GOLD") .. "/2")
check("recorded", rec.choice, "repeat")
o, s, n, rec = choose({ rtype = "WC_RES_MERCENARY_COMPANIES", targets = yields, option = 2,
	selection = 1, votes = 2 })
check("no history: today's vote", o, nil)
check("and says why", rec.why, "no_history")
o, s, n, rec = choose({ rtype = "WC_RES_MERCENARY_COMPANIES", targets = yields, option = 2,
	selection = 2, votes = 2, last = last })
check("already on it", rec.why, "already")
o, s, n, rec = choose({ rtype = "WC_RES_MERCENARY_COMPANIES", targets = yields, option = 2,
	selection = 1, votes = 2, last = { WC_RES_MERCENARY_COMPANIES = { option = 0, target = yield("GOLD") } } })
check("a tied last session is no history", rec.why, "no_history")
o, s, n, rec = choose({ rtype = "WC_RES_MERCENARY_COMPANIES", targets = { yield("PRODUCTION") },
	option = 2, selection = 1, votes = 2, last = last })
check("a winner no longer offered", rec.why, "not_offered")

-- 5. The real ballot: a Science seat racing the Congress, four rivals, a
-- session with the Diplomatic Victory and the three kinds.
local tick = upvalue(CivvisQueue.onUiPulse, "tick")
local config = { Play = true, CivvisDecides = false, CounterResolutions = false,
	VictoryLane = "science", ScienceCongressRacer = true }
upvalue(tick, "cfg", config)
local hooks = {}
Events = setmetatable({ WorldCongressStage1 = { Add = function(fn) hooks.stage = fn end } },
	{ __index = function() return stub() end })
LuaEvents = setmetatable({ CivvisCongressBallot = { Add = function(fn) hooks.popup = fn end } },
	{ __index = function() return stub() end })
Automation = { Log = function() end }
local bank = 40
local rivals = {}
local disfavored = {}
local session = four
local offered = {
	WC_RES_DIPLOVICTORY = { "PlayerType", players },
	WC_RES_MERCENARY_COMPANIES = { "YieldType", yields },
	WC_RES_MIGRATION_TREATY = { "PlayerType", players },
	WC_RES_WORLD_IDEOLOGY = { "GovernmentType", ideologies },
}
local wc = {
	GetVotesandFavorCost = function()
		local costs = { [0] = 0, [1] = 4 }
		local max = 1
		while 2 * (max + 1) * max <= bank do max = max + 1 end
		for k = 1, max do costs[k] = 2 * (k + 1) * k end
		costs.MaxVotes = max
		return costs
	end,
	GetResolutions = function()
		local list = { Stage = 2147483647 }
		for i, name in ipairs(session) do
			list[i] = { Type = name, TargetType = offered[name][1], PossibleTargets = offered[name][2],
				DisfavoredPlayerIDs = name == "WC_RES_MIGRATION_TREATY" and disfavored or {} }
		end
		return list
	end,
	GetReview = function() return nil end,
}
Game = {
	GetLocalPlayer = function() return 0 end,
	GetCurrentGameTurn = function() return 152 end,
	GetCurrentTurnSegment = function() return "TURNSEG_WORLDCONGRESS_1" end,
	GetWorldCongress = function() return wc end,
}
DB = { MakeHash = function(name) return name end }
local function culture(id)
	return { GetStaycationers = function() return 0 end, GetTouristsTo = function() return 0 end,
		GetCurrentGovernment = function() return held[id] and ("GOVERNMENT_" .. held[id]) or nil end }
end
Players = { [0] = {
	IsTurnActive = function() return false end,
	GetFavor = function() return bank end,
	GetFavorEnteringCongress = function() return bank end,
	GetStats = function() return { GetDiplomaticVictoryPoints = function() return 6 end } end,
	GetScore = function() return 900 end,
	GetDiplomacy = function() return { HasMet = function() return true end } end,
	GetCulture = function() return culture(0) end,
} }
for id = 1, 4 do
	local player = id
	Players[id] = {
		GetStats = function() return {
			GetDiplomaticVictoryPoints = function() return rivals[player].points end,
			GetScienceVictoryPoints = function() return 0 end,
		} end,
		GetScore = function() return 800 + player end,
		GetFavor = function() return rivals[player].now end,
		GetFavorEnteringCongress = function() return rivals[player].entering end,
		GetCulture = function() return culture(player) end,
	}
end
PlayerManager = { GetAliveMajorIDs = function() return { 0, 1, 2, 3, 4 } end }
GameInfo = { Resolutions = {}, Governments = {} }
for _, name in ipairs(four) do GameInfo.Resolutions[name] = { ResolutionType = name, Hash = name } end
for _, name in ipairs({ "FASCISM", "DEMOCRACY", "COMMUNISM" }) do
	GameInfo.Governments["GOVERNMENT_" .. name] = { Name = gov(name) }
end
PlayerOperations = {
	PARAM_RESOLUTION_TYPE = "type", PARAM_WORLD_CONGRESS_VOTES = "votes",
	PARAM_RESOLUTION_OPTION = "option", PARAM_RESOLUTION_SELECTION = "selection",
	WORLD_CONGRESS_RESOLUTION_VOTE = "vote", WORLD_CONGRESS_SUBMIT_TURN = "submit",
}
local requests, submitted = {}, 0
UI = { RequestPlayerOperation = function(pid, operation, params)
	assert(pid == 0)
	if operation == "vote" then requests[params.type] = params end
	if operation == "submit" then submitted = submitted + 1 end
end }
local contexts = {}
local context = CivvisCongressChoiceContext
CivvisCongressChoiceContext = function(...)
	local row = context(...)
	contexts[row.resolution] = row
	return row
end
tick()
local cast = upvalue(hooks.popup, "castBallot")
local vote = upvalue(cast, "voteWorldCongress")
local tally = upvalue(vote, "envoyTally")
local function sameField()
	return {
		[1] = { points = 7, entering = 27, now = 3 },
		[2] = { points = 8, entering = 300, now = 12 },
		[3] = { points = 5, entering = 9, now = 9 },
		[4] = { points = 6, entering = 40, now = 0 },
	}
end
local function run(favor, f, lastWin)
	requests, contexts, submitted = {}, {}, 0
	bank, rivals = favor, f
	tally.wc_rival_block, tally.wc_rival_block_prev, tally.wc_review_favor = nil, nil, nil
	tally.wc_last_win = lastWin
	vote(0)
	local out = {}
	for _, name in ipairs(four) do
		local q = requests[name]
		local t = q and offered[name][2][q.selection + 1]
		out[name] = q and (q.option .. "/" .. tostring(t) .. "/" .. q.votes) or "none"
	end
	return out
end
local mig, ideo, merc = "WC_RES_MIGRATION_TREATY", "WC_RES_WORLD_IDEOLOGY", "WC_RES_MERCENARY_COMPANIES"
local gold = { [merc] = { option = 2, target = yield("GOLD") } }

-- Unarmed: today's ballot, the reading recorded.
local today = run(40, sameField(), gold)
check("unarmed Migration Treaty: the free vote A on us", today[mig], "1/0/1")
check("unarmed World Ideology: the rivals' landslide, Communism", today[ideo], "1/" .. gov("COMMUNISM") .. "/1")
check("unarmed Mercenary Companies: B on Production", today[merc], "2/" .. yield("PRODUCTION") .. "/2")
local p = contexts[mig].wc_prediction
check("the Migration Treaty row records the reading", p and p.kind, "claim_or_join")
check("its winner", p and p.winner, 2)
check("its block", p and p.block, 6)
check("what an armed arm would cast", p and p.would, "join")
check("unarmed", p and p.armed, false)
check("World Ideology would take Democracy", contexts[ideo].wc_prediction.would, "government")
check("Mercenary Companies would repeat", contexts[merc].wc_prediction.would, "repeat")
check("the Diplomatic Victory row carries none", contexts.WC_RES_DIPLOVICTORY.wc_prediction, nil)
check("four votes and the submission", submitted, 1)

-- Armed: the same session.
config.WcJoinPlurality = true
local armed = run(40, sameField(), gold)
check("armed Migration Treaty: the free vote A on rival 2", armed[mig], "1/2/1")
check("armed World Ideology: Democracy", armed[ideo], "1/" .. gov("DEMOCRACY") .. "/1")
check("armed Mercenary Companies: B on Gold, its two votes", armed[merc], "2/" .. yield("GOLD") .. "/2")
check("the Diplomatic Victory ballot is untouched", armed.WC_RES_DIPLOVICTORY, today.WC_RES_DIPLOVICTORY)
check("armed records the choice", contexts[mig].wc_prediction.choice, "join")
check("the context names the joined target", contexts[mig].requested.target, 2)
check("still submitted", submitted, 1)

-- A claim the spend says loses joins and its Favor comes back. Without the
-- Diplomatic Victory on the ballot, 300 Favor buys the treaty its six.
session = { merc, mig, ideo }
local rich = sameField()
rich[2].entering = 612 -- 600 paid: a quarter buys nine
config.WcJoinPlurality = nil
local claim = run(300, rich)
local before = contexts[mig].requested
config.WcJoinPlurality = true
local joined = run(300, rich)
local after = contexts[mig].requested
check("unarmed: a claim of six on us", claim[mig], "1/0/6")
check("armed: the free vote on rival 2", joined[mig], "1/2/1")
check("its 60 Favor stays in the bank", after.favor_after_allocations - before.favor_after_allocations, 60)
check("and off the session's spend", before.spent_requested - after.spent_requested, 60)
-- A claim at least the block stands.
local modest = sameField()
modest[2].entering = 112 -- 100 paid: a quarter buys four
local kept = run(300, modest)
check("a claim of six over a block of four stands", kept[mig], "1/0/6")
check("and says so", contexts[mig].wc_prediction.why, "claim_carries")
session = four

-- The Culture counter's B on a threat stands (a racer keeps it only with
-- `RacerJoinsTheField = false`): rival 1 on 12 points is the threat.
config.CounterResolutions, config.RacerJoinsTheField = true, false
local threatened = sameField()
threatened[1].points = 12
config.WcJoinPlurality = nil
local counter = run(40, threatened)
config.WcJoinPlurality = true
local counterArmed = run(40, threatened)
check("the counter votes B on the threat", counter[mig], "2/1/1")
check("armed, the counter stands", counterArmed[mig], "2/1/1")
check("and the reading says why", contexts[mig].wc_prediction.why, "not_a_claim")
config.CounterResolutions, config.RacerJoinsTheField = false, nil

-- A failing arm leaves today's ballot and still submits it.
local real = CivvisWcJoinPlurality
CivvisWcJoinPlurality = function() error("host table changed shape") end
local broken = run(40, sameField(), gold)
check("a failed arm: today's Migration Treaty", broken[mig], today[mig])
check("today's World Ideology", broken[ideo], today[ideo])
check("today's Mercenary Companies", broken[merc], today[merc])
check("no reading recorded", contexts[mig].wc_prediction, nil)
check("the ballot is still submitted", submitted, 1)
CivvisWcJoinPlurality = real

-- A seat not racing the Congress: today's ballot, nothing recorded.
config.ScienceCongressRacer = nil
local plain = run(40, sameField(), gold)
check("not racing: no reading", contexts[mig].wc_prediction, nil)
check("not racing: Migration Treaty as without the arm", plain[mig] ~= "1/2/1", true)
config.ScienceCongressRacer = true

-- The switch reaches `loaded`, and the review keeps each kind's last winner.
local source = assert(io.open(here .. "/CivvisControlAgent.lua")):read("*a")
check("loaded records the arm",
	source:find("wc_join_plurality = cfg.WcJoinPlurality == true", 1, true) ~= nil, true)
check("the review records the last winner", source:find(
	'envoyTally.wc_last_win[rtype] = { option = won, target = tostring(r.ChosenThing or "") };', 1, true) ~= nil, true)
config.WcJoinPlurality = nil

local RECORDED = {
	{ game = "20261120T051850Z", turn = 135, kind = "MERCENARY_COMPANIES",
		targets = { "PRODUCTION", "GOLD", "FAITH" },
		ours = { 2, "PRODUCTION", 2 },
		last = { option = 2, target = "GOLD" },
		voters = { {1, 1, "FAITH", 1}, {2, 2, "GOLD", 7}, {3, 1, "FAITH", 1}, {4, 2, "GOLD", 5}, {5, 2, "PRODUCTION", 5} },
		want = { 2, "GOLD", 2 } },
	{ game = "20261120T053354Z-cont1", turn = 152, kind = "MERCENARY_COMPANIES",
		targets = { "PRODUCTION", "GOLD", "FAITH" },
		ours = { 2, "PRODUCTION", 2 },
		last = { option = 2, target = "GOLD" },
		voters = { {1, 2, "GOLD", 5}, {2, 2, "GOLD", 8}, {3, 2, "GOLD", 3}, {4, 2, "GOLD", 7}, {5, 2, "PRODUCTION", 5} },
		want = { 2, "GOLD", 2 } },
	{ game = "20261120T081130Z-cont1", turn = 173, kind = "MERCENARY_COMPANIES",
		targets = { "PRODUCTION", "GOLD", "FAITH" },
		ours = { 2, "PRODUCTION", 1 },
		last = { option = 2, target = "GOLD" },
		voters = { {1, 2, "PRODUCTION", 9}, {2, 2, "GOLD", 7}, {3, 1, "FAITH", 8}, {4, 1, "FAITH", 1}, {5, 2, "GOLD", 4} },
		want = { 2, "GOLD", 1 } },
	{ game = "20261120T043819Z", turn = 132, kind = "MIGRATION_TREATY",
		targets = { "0", "1", "2", "3", "4", "5" },
		ours = { 1, "0", 1 }, spends = { [1] = 72, [2] = 232, [3] = 240, [4] = 96, [5] = 44 },
		disfavored = {  },
		voters = { {1, 1, "1", 3}, {2, 1, "2", 3}, {3, 1, "3", 6}, {4, 1, "4", 3}, {5, 1, "0", 2} },
		want = { 1, "3", 1 } },
	{ game = "20261120T045254Z", turn = 153, kind = "MIGRATION_TREATY",
		targets = { "0", "1", "2", "3", "4", "5" },
		ours = { 1, "0", 3 }, spends = { [1] = 304, [2] = 12, [3] = 172, [4] = 24, [5] = 52 },
		disfavored = {  },
		voters = { {1, 1, "4", 5}, {2, 1, "2", 1}, {3, 1, "3", 2}, {4, 1, "4", 1}, {5, 1, "5", 1} },
		want = { 1, "1", 1 } },
	{ game = "20261120T050623Z", turn = 112, kind = "MIGRATION_TREATY",
		targets = { "0", "2", "3", "4", "5" },
		ours = { 1, "0", 1 }, spends = { [2] = 228, [3] = 0, [4] = 36, [5] = 136 },
		disfavored = {  },
		voters = { {2, 1, "2", 7}, {3, 1, "3", 1}, {4, 1, "4", 3}, {5, 1, "5", 4} },
		want = { 1, "2", 1 } },
	{ game = "20261120T051850Z", turn = 155, kind = "MIGRATION_TREATY",
		targets = { "0", "1", "2", "3", "4", "5" },
		ours = { 1, "0", 6 }, spends = { [1] = 456, [2] = 72, [3] = 72, [4] = 44, [5] = 204 },
		disfavored = {  },
		voters = { {1, 1, "1", 9}, {2, 1, "1", 3}, {3, 1, "3", 3}, {4, 1, "4", 2}, {5, 1, "3", 6} },
		want = { 1, "1", 1 } },
	{ game = "20261120T061018Z", turn = 132, kind = "MIGRATION_TREATY",
		targets = { "0", "1", "2", "3", "4", "5" },
		ours = { 1, "0", 1 }, spends = { [1] = 28, [2] = 184, [3] = 184, [4] = 64, [5] = 192 },
		disfavored = {  },
		voters = { {1, 1, "1", 2}, {2, 1, "2", 5}, {3, 1, "3", 5}, {4, 1, "4", 4}, {5, 1, "5", 3} },
		want = { 1, "2", 1 } },
	{ game = "20261120T062249Z", turn = 133, kind = "MIGRATION_TREATY",
		targets = { "0", "1", "3", "4", "5" },
		ours = { 1, "0", 5 }, spends = { [1] = 68, [2] = 124, [3] = 260, [4] = 24, [5] = 68 },
		disfavored = {  },
		voters = { {1, 1, "1", 2}, {2, 1, "2", 3}, {3, 1, "3", 5}, {4, 1, "4", 1}, {5, 1, "5", 2} },
		want = { 1, "3", 1 } },
	{ game = "20261120T063234Z-cont1", turn = 172, kind = "MIGRATION_TREATY",
		targets = { "0", "1", "2", "3", "4", "5" },
		ours = { 1, "0", 3 }, spends = { [1] = 216, [2] = 128, [3] = 336, [4] = 44, [5] = 96 },
		disfavored = {  },
		voters = { {1, 1, "1", 3}, {2, 1, "2", 2}, {3, 1, "3", 3}, {4, 1, "4", 1}, {5, 1, "5", 3} },
		want = { 1, "3", 1 } },
	{ game = "20261120T071809Z", turn = 174, kind = "MIGRATION_TREATY",
		targets = { "0", "1", "2", "3", "4", "5" },
		ours = { 1, "0", 1 }, spends = { [1] = 44, [2] = 240, [3] = 352, [4] = 376, [5] = 148 },
		disfavored = {  },
		voters = { {1, 1, "1", 2}, {2, 1, "1", 6}, {3, 1, "3", 5}, {4, 1, "4", 3}, {5, 1, "5", 2} },
		want = { 1, "3", 1 } },
	{ game = "20261120T083921Z", turn = 132, kind = "MIGRATION_TREATY",
		targets = { "0", "1", "2", "3", "4", "5" },
		ours = { 1, "0", 1 }, spends = { [1] = 44, [2] = 52, [3] = 124, [4] = 64, [5] = 152 },
		disfavored = {  },
		voters = { {1, 1, "0", 2}, {2, 1, "2", 3}, {3, 1, "3", 5}, {4, 1, "4", 2}, {5, 1, "5", 5} },
		want = { 1, "3", 1 } },
	{ game = "20261120T083921Z", turn = 172, kind = "MIGRATION_TREATY",
		targets = { "0", "2", "3", "4", "5" },
		ours = { 1, "0", 3 }, spends = { [2] = 112, [3] = 448, [4] = 172, [5] = 296 },
		disfavored = {  },
		voters = { {2, 1, "2", 3}, {3, 1, "3", 5}, {4, 1, "4", 2}, {5, 1, "5", 5} },
		want = { 1, "3", 1 } },
	{ game = "20261120T094935Z", turn = 134, kind = "MIGRATION_TREATY",
		targets = { "0", "1", "2", "3", "4", "5" },
		ours = { 1, "0", 1 }, spends = { [1] = 52, [2] = 72, [3] = 28, [4] = 72, [5] = 28 },
		disfavored = {  },
		voters = { {1, 1, "1", 3}, {2, 1, "2", 3}, {3, 1, "3", 2}, {4, 1, "0", 3}, {5, 1, "5", 2} },
		want = { 1, "1", 1 } },
	{ game = "20261120T094935Z", turn = 174, kind = "MIGRATION_TREATY",
		targets = { "0", "1", "2", "3", "4", "5" },
		ours = { 1, "0", 1 }, spends = { [1] = 292, [2] = 188, [3] = 140, [4] = 8, [5] = 56 },
		disfavored = {  },
		voters = { {1, 1, "1", 3}, {2, 1, "2", 2}, {3, 1, "0", 2}, {4, 1, "0", 1}, {5, 1, "5", 2} },
		want = { 1, "1", 1 } },
	{ game = "20261120T100612Z", turn = 132, kind = "MIGRATION_TREATY",
		targets = { "0", "1", "2", "4", "5" },
		ours = { 1, "0", 1 }, spends = { [1] = 32, [2] = 112, [3] = 40, [4] = 24, [5] = 132 },
		disfavored = { 4 },
		voters = { {1, 1, "1", 2}, {2, 1, "2", 2}, {3, 1, "2", 2}, {4, 2, "0", 1}, {5, 1, "5", 3} },
		want = { 1, "2", 1 } },
	{ game = "20261120T101840Z", turn = 114, kind = "MIGRATION_TREATY",
		targets = { "0", "1", "2", "3", "4", "5" },
		ours = { 1, "0", 1 }, spends = { [1] = 24, [2] = 28, [3] = 16, [4] = 100, [5] = 0 },
		disfavored = {  },
		voters = { {1, 1, "4", 3}, {2, 1, "1", 2}, {3, 1, "3", 2}, {4, 1, "4", 5}, {5, 1, "5", 1} },
		want = { 1, "4", 1 } },
	{ game = "20261120T103014Z", turn = 92, kind = "MIGRATION_TREATY",
		targets = { "0", "1", "3", "5" },
		ours = { 1, "0", 6 }, spends = { [1] = 420, [2] = 72, [3] = 52, [4] = 88, [5] = 52 },
		disfavored = {  },
		voters = { {1, 1, "1", 15}, {2, 1, "2", 6}, {3, 1, "3", 3}, {4, 1, "4", 2}, {5, 1, "5", 3} },
		want = { 1, "1", 1 } },
	{ game = "20261120T103014Z", turn = 132, kind = "MIGRATION_TREATY",
		targets = { "0", "1", "2", "3", "4", "5" },
		ours = { 1, "0", 1 }, spends = { [1] = 232, [2] = 116, [3] = 84, [4] = 124, [5] = 172 },
		disfavored = {  },
		voters = { {1, 1, "1", 3}, {2, 1, "2", 2}, {3, 1, "4", 4}, {4, 1, "0", 5}, {5, 1, "5", 6} },
		want = { 1, "1", 1 } },
	{ game = "20261120T103014Z", turn = 192, kind = "MIGRATION_TREATY",
		targets = { "0", "1", "2", "3", "4", "5" },
		ours = { 1, "0", 2 }, spends = { [1] = 468, [2] = 100, [3] = 48, [4] = 300, [5] = 96 },
		disfavored = {  },
		voters = { {1, 1, "1", 3}, {2, 1, "2", 2}, {3, 1, "3", 2}, {4, 1, "4", 3}, {5, 1, "5", 1} },
		want = { 1, "1", 1 } },
	{ game = "20261120T130355Z", turn = 92, kind = "MIGRATION_TREATY",
		targets = { "0", "2", "3", "4" },
		ours = { 1, "0", 1 }, spends = { [1] = 64, [2] = 36, [3] = 156, [4] = 64, [5] = 72 },
		disfavored = {  },
		voters = { {1, 1, "1", 2}, {2, 1, "2", 3}, {3, 1, "3", 3}, {4, 1, "3", 2}, {5, 1, "5", 3} },
		want = { 1, "3", 1 } },
	{ game = "20261120T130355Z", turn = 172, kind = "MIGRATION_TREATY",
		targets = { "0", "1", "2", "3", "4", "5" },
		ours = { 1, "0", 3 }, spends = { [1] = 176, [2] = 196, [3] = 88, [4] = 208, [5] = 372 },
		disfavored = {  },
		voters = { {1, 1, "1", 5}, {2, 1, "0", 3}, {3, 1, "3", 3}, {4, 1, "4", 5}, {5, 1, "5", 7} },
		want = { 1, "5", 1 } },
	{ game = "20261120T134042Z", turn = 152, kind = "MIGRATION_TREATY",
		targets = { "0", "1", "2", "3", "4", "5" },
		ours = { 1, "0", 1 }, spends = { [1] = 96, [2] = 144, [3] = 176, [4] = 588, [5] = 64 },
		disfavored = {  },
		voters = { {1, 1, "0", 4}, {2, 1, "0", 6}, {3, 1, "3", 5}, {4, 1, "4", 7}, {5, 1, "5", 3} },
		want = { 1, "4", 1 } },
	{ game = "20261120T143159Z", turn = 172, kind = "MIGRATION_TREATY",
		targets = { "0", "1", "2", "3", "4", "5" },
		ours = { 1, "0", 1 }, spends = { [1] = 332, [2] = 500, [3] = 200, [4] = 276, [5] = 88 },
		disfavored = {  },
		voters = { {1, 1, "1", 2}, {2, 1, "2", 3}, {3, 1, "3", 2}, {4, 1, "5", 3}, {5, 1, "5", 2} },
		want = { 1, "2", 1 } },
	{ game = "20261120T143159Z", turn = 192, kind = "MIGRATION_TREATY",
		targets = { "0", "1", "2", "3", "4", "5" },
		ours = { 1, "0", 8 }, spends = { [1] = 484, [2] = 808, [3] = 460, [4] = 0, [5] = 252 },
		disfavored = {  },
		voters = { {1, 1, "2", 5}, {2, 1, "2", 8}, {3, 1, "3", 6}, {4, 1, "0", 1}, {5, 1, "5", 3} },
		want = { 1, "2", 1 } },
	{ game = "20261120T145009Z", turn = 133, kind = "MIGRATION_TREATY",
		targets = { "0", "2", "4", "5" },
		ours = { 1, "0", 1 }, spends = { [1] = 132, [2] = 356, [3] = 132, [4] = 40, [5] = 56 },
		disfavored = {  },
		voters = { {1, 1, "4", 4}, {2, 1, "2", 8}, {3, 1, "3", 4}, {4, 1, "4", 2}, {5, 1, "5", 2} },
		want = { 1, "2", 1 } },
	{ game = "20261120T145009Z", turn = 173, kind = "MIGRATION_TREATY",
		targets = { "0", "1", "2", "3", "4", "5" },
		ours = { 1, "0", 3 }, spends = { [1] = 100, [2] = 100, [3] = 76, [4] = 36, [5] = 136 },
		disfavored = {  },
		voters = { {1, 1, "2", 2}, {2, 1, "0", 2}, {3, 1, "3", 2}, {4, 1, "4", 1}, {5, 1, "5", 1} },
		want = { 1, "1", 1 } },
	{ game = "20261121T023459Z", turn = 155, kind = "MIGRATION_TREATY",
		targets = { "0", "1", "2", "3", "4", "5" },
		ours = { 1, "0", 6 }, spends = { [1] = 64, [2] = 228, [3] = 556, [4] = 112, [5] = 36 },
		disfavored = {  },
		voters = { {1, 1, "1", 3}, {2, 1, "2", 4}, {3, 1, "3", 10}, {4, 1, "4", 5}, {5, 1, "0", 3} },
		want = { 1, "3", 1 } },
	{ game = "20261121T033617Z", turn = 133, kind = "MIGRATION_TREATY",
		targets = { "0", "2", "3", "4" },
		ours = { 1, "0", 1 }, spends = { [1] = 88, [2] = 244, [3] = 108, [4] = 76, [5] = 180 },
		disfavored = {  },
		voters = { {1, 1, "1", 4}, {2, 1, "2", 5}, {3, 1, "3", 3}, {4, 1, "4", 4}, {5, 1, "5", 4} },
		want = { 1, "2", 1 } },
	{ game = "20261121T041557Z", turn = 112, kind = "MIGRATION_TREATY",
		targets = { "0", "1", "2", "3", "5" },
		ours = { 1, "0", 1 }, spends = { [1] = 192, [2] = 148, [3] = 0, [4] = 100, [5] = 108 },
		disfavored = {  },
		voters = { {1, 1, "1", 4}, {2, 1, "2", 4}, {3, 1, "3", 1}, {4, 1, "4", 3}, {5, 1, "5", 3} },
		want = { 1, "1", 1 } },
	{ game = "20261121T043307Z", turn = 153, kind = "MIGRATION_TREATY",
		targets = { "0", "1", "2", "3", "4", "5" },
		ours = { 1, "0", 1 }, spends = { [1] = 84, [2] = 176, [3] = 108, [4] = 72, [5] = 132 },
		disfavored = {  },
		voters = { {1, 1, "1", 3}, {2, 1, "0", 5}, {3, 1, "3", 4}, {4, 2, "0", 1}, {5, 1, "3", 4} },
		want = { 1, "2", 1 } },
	{ game = "20261121T044558Z", turn = 172, kind = "MIGRATION_TREATY",
		targets = { "0", "1", "2", "3", "4", "5" },
		ours = { 1, "0", 7 }, spends = { [1] = 188, [2] = 328, [3] = 340, [4] = 208, [5] = 636 },
		disfavored = {  },
		voters = { {1, 1, "1", 2}, {2, 1, "1", 7}, {3, 1, "3", 2}, {4, 1, "4", 2}, {5, 1, "5", 3} },
		want = { 1, "5", 1 } },
	{ game = "20261120T045254Z", turn = 113, kind = "WORLD_IDEOLOGY",
		targets = { "AUTOCRACY", "OLIGARCHY", "CLASSICAL_REPUBLIC", "MONARCHY", "THEOCRACY", "MERCHANT_REPUBLIC", "FASCISM", "COMMUNISM", "DEMOCRACY", "CORPORATE_LIBERTARIANISM", "DIGITAL_DEMOCRACY", "SYNTHETIC_TECHNOCRACY" },
		ours = { 1, "FASCISM", 1 }, spends = { [1] = 72, [2] = 100, [3] = 144, [4] = 408, [5] = 108 },
		governments = { [1] = "CLASSICAL_REPUBLIC", [2] = "FASCISM", [3] = "FASCISM", [4] = "MONARCHY", [5] = "MONARCHY" }, session = { "WORLD_IDEOLOGY", "PUBLIC_RELATIONS", "DIPLOVICTORY" },
		voters = { {1, 1, "CLASSICAL_REPUBLIC", 3}, {2, 1, "FASCISM", 5}, {3, 1, "FASCISM", 6}, {4, 1, "MONARCHY", 9}, {5, 1, "MONARCHY", 4} },
		want = { 1, "MONARCHY", 1 } },
	{ game = "20261120T061018Z", turn = 172, kind = "WORLD_IDEOLOGY",
		targets = { "AUTOCRACY", "OLIGARCHY", "CLASSICAL_REPUBLIC", "MONARCHY", "THEOCRACY", "MERCHANT_REPUBLIC", "FASCISM", "COMMUNISM", "DEMOCRACY", "CORPORATE_LIBERTARIANISM", "DIGITAL_DEMOCRACY", "SYNTHETIC_TECHNOCRACY" },
		ours = { 1, "DIGITAL_DEMOCRACY", 1 }, spends = { [1] = 480, [2] = 64, [3] = 224, [4] = 344, [5] = 196 },
		governments = { [1] = "SYNTHETIC_TECHNOCRACY", [2] = "MERCHANT_REPUBLIC", [3] = "DIGITAL_DEMOCRACY", [4] = "MONARCHY", [5] = "FASCISM" }, session = { "TRADE_TREATY", "WORLD_IDEOLOGY", "DIPLOVICTORY" },
		voters = { {1, 1, "DIGITAL_DEMOCRACY", 11}, {2, 1, "MERCHANT_REPUBLIC", 3}, {3, 1, "DIGITAL_DEMOCRACY", 8}, {4, 1, "MONARCHY", 5}, {5, 1, "FASCISM", 10} },
		want = { 1, "SYNTHETIC_TECHNOCRACY", 1 } },
	{ game = "20261120T071809Z", turn = 114, kind = "WORLD_IDEOLOGY",
		targets = { "AUTOCRACY", "OLIGARCHY", "CLASSICAL_REPUBLIC", "MONARCHY", "THEOCRACY", "MERCHANT_REPUBLIC", "FASCISM", "COMMUNISM", "DEMOCRACY", "CORPORATE_LIBERTARIANISM", "DIGITAL_DEMOCRACY", "SYNTHETIC_TECHNOCRACY" },
		ours = { 1, "FASCISM", 1 }, spends = { [1] = 4, [2] = 172, [3] = 4, [4] = 256, [5] = 204 },
		governments = { [3] = "OLIGARCHY", [4] = "MONARCHY", [5] = "FASCISM" }, session = { "WORLD_IDEOLOGY", "PUBLIC_RELATIONS", "DIPLOVICTORY" },
		voters = { {1, 1, "OLIGARCHY", 2}, {2, 1, "MONARCHY", 8}, {3, 1, "OLIGARCHY", 2}, {4, 1, "MONARCHY", 8}, {5, 1, "FASCISM", 9} },
		want = { 1, "MONARCHY", 1 } },
	{ game = "20261120T073344Z", turn = 152, kind = "WORLD_IDEOLOGY",
		targets = { "AUTOCRACY", "OLIGARCHY", "CLASSICAL_REPUBLIC", "MONARCHY", "THEOCRACY", "MERCHANT_REPUBLIC", "FASCISM", "COMMUNISM", "DEMOCRACY", "CORPORATE_LIBERTARIANISM", "DIGITAL_DEMOCRACY", "SYNTHETIC_TECHNOCRACY" },
		ours = { 1, "FASCISM", 5 }, spends = { [1] = 220, [2] = 44, [3] = 152, [4] = 24, [5] = 192 },
		governments = { [1] = "SYNTHETIC_TECHNOCRACY", [2] = "DEMOCRACY", [4] = "FASCISM", [5] = "DIGITAL_DEMOCRACY" }, session = { "HERITAGE_ORG", "WORLD_IDEOLOGY", "DIPLOVICTORY" },
		voters = { {1, 1, "SYNTHETIC_TECHNOCRACY", 10}, {2, 1, "DEMOCRACY", 5}, {3, 1, "FASCISM", 8}, {4, 1, "FASCISM", 3}, {5, 1, "DIGITAL_DEMOCRACY", 10} },
		want = { 1, "DIGITAL_DEMOCRACY", 5 } },
	{ game = "20261120T075546Z", turn = 116, kind = "WORLD_IDEOLOGY",
		targets = { "AUTOCRACY", "OLIGARCHY", "CLASSICAL_REPUBLIC", "MONARCHY", "THEOCRACY", "MERCHANT_REPUBLIC", "FASCISM", "COMMUNISM", "DEMOCRACY", "CORPORATE_LIBERTARIANISM", "DIGITAL_DEMOCRACY", "SYNTHETIC_TECHNOCRACY" },
		ours = { 1, "FASCISM", 1 }, spends = { [1] = 144, [2] = 104, [3] = 60, [4] = 60, [5] = 288 },
		governments = { [1] = "FASCISM", [2] = "OLIGARCHY", [3] = "CLASSICAL_REPUBLIC", [4] = "CLASSICAL_REPUBLIC", [5] = "OLIGARCHY" }, session = { "WORLD_IDEOLOGY", "LUXURY", "DIPLOVICTORY" },
		voters = { {1, 1, "FASCISM", 6}, {2, 1, "OLIGARCHY", 5}, {3, 1, "CLASSICAL_REPUBLIC", 4}, {4, 1, "CLASSICAL_REPUBLIC", 4}, {5, 1, "OLIGARCHY", 6} },
		want = { 1, "OLIGARCHY", 1 } },
	{ game = "20261120T075546Z", turn = 156, kind = "WORLD_IDEOLOGY",
		targets = { "AUTOCRACY", "OLIGARCHY", "CLASSICAL_REPUBLIC", "MONARCHY", "THEOCRACY", "MERCHANT_REPUBLIC", "FASCISM", "COMMUNISM", "DEMOCRACY", "CORPORATE_LIBERTARIANISM", "DIGITAL_DEMOCRACY", "SYNTHETIC_TECHNOCRACY" },
		ours = { 1, "FASCISM", 6 }, spends = { [1] = 408, [2] = 48, [3] = 48, [4] = 44, [5] = 0 },
		governments = { [1] = "CORPORATE_LIBERTARIANISM", [2] = "FASCISM", [3] = "FASCISM", [4] = "SYNTHETIC_TECHNOCRACY", [5] = "OLIGARCHY" }, session = { "WORLD_IDEOLOGY", "MILITARY_ADVISORY", "DIPLOVICTORY" },
		voters = { {1, 1, "CORPORATE_LIBERTARIANISM", 12}, {2, 1, "FASCISM", 4}, {3, 1, "MONARCHY", 4}, {4, 1, "SYNTHETIC_TECHNOCRACY", 5}, {5, 1, "OLIGARCHY", 1} },
		want = { 1, "CORPORATE_LIBERTARIANISM", 6 } },
	{ game = "20261120T103014Z", turn = 192, kind = "WORLD_IDEOLOGY",
		targets = { "AUTOCRACY", "OLIGARCHY", "CLASSICAL_REPUBLIC", "MONARCHY", "THEOCRACY", "MERCHANT_REPUBLIC", "FASCISM", "COMMUNISM", "DEMOCRACY", "CORPORATE_LIBERTARIANISM", "DIGITAL_DEMOCRACY", "SYNTHETIC_TECHNOCRACY" },
		ours = { 1, "FASCISM", 1 }, spends = { [1] = 468, [2] = 100, [3] = 48, [4] = 300, [5] = 96 },
		governments = { [1] = "DEMOCRACY", [2] = "SYNTHETIC_TECHNOCRACY", [3] = "DIGITAL_DEMOCRACY", [4] = "SYNTHETIC_TECHNOCRACY", [5] = "DIGITAL_DEMOCRACY" }, session = { "WORLD_IDEOLOGY", "MIGRATION_TREATY", "DIPLOVICTORY" },
		voters = { {1, 1, "DEMOCRACY", 13}, {2, 1, "SYNTHETIC_TECHNOCRACY", 7}, {3, 1, "DIGITAL_DEMOCRACY", 5}, {4, 1, "SYNTHETIC_TECHNOCRACY", 12}, {5, 1, "DIGITAL_DEMOCRACY", 7} },
		want = { 1, "SYNTHETIC_TECHNOCRACY", 1 } },
	{ game = "20261120T110805Z", turn = 113, kind = "WORLD_IDEOLOGY",
		targets = { "AUTOCRACY", "OLIGARCHY", "CLASSICAL_REPUBLIC", "MONARCHY", "THEOCRACY", "MERCHANT_REPUBLIC", "FASCISM", "COMMUNISM", "DEMOCRACY", "CORPORATE_LIBERTARIANISM", "DIGITAL_DEMOCRACY", "SYNTHETIC_TECHNOCRACY" },
		ours = { 1, "FASCISM", 1 }, spends = { [1] = 112, [2] = 68, [3] = 168, [4] = 88, [5] = 204 },
		governments = { [1] = "OLIGARCHY", [2] = "MERCHANT_REPUBLIC", [4] = "CLASSICAL_REPUBLIC" }, session = { "WORLD_IDEOLOGY", "LUXURY", "DIPLOVICTORY" },
		voters = { {1, 1, "OLIGARCHY", 6}, {2, 1, "MERCHANT_REPUBLIC", 2}, {3, 1, "MONARCHY", 4}, {4, 1, "CLASSICAL_REPUBLIC", 5}, {5, 1, "OLIGARCHY", 7} },
		want = { 1, "OLIGARCHY", 1 } },
	{ game = "20261120T130355Z", turn = 152, kind = "WORLD_IDEOLOGY",
		targets = { "AUTOCRACY", "OLIGARCHY", "CLASSICAL_REPUBLIC", "MONARCHY", "THEOCRACY", "MERCHANT_REPUBLIC", "FASCISM", "COMMUNISM", "DEMOCRACY", "CORPORATE_LIBERTARIANISM", "DIGITAL_DEMOCRACY", "SYNTHETIC_TECHNOCRACY" },
		ours = { 1, "FASCISM", 1 }, spends = { [1] = 36, [2] = 148, [3] = 268, [4] = 124, [5] = 160 },
		governments = { [1] = "SYNTHETIC_TECHNOCRACY", [2] = "CLASSICAL_REPUBLIC", [3] = "THEOCRACY", [4] = "DIGITAL_DEMOCRACY", [5] = "MONARCHY" }, session = { "WORLD_IDEOLOGY", "HERITAGE_ORG", "DIPLOVICTORY" },
		voters = { {1, 1, "SYNTHETIC_TECHNOCRACY", 3}, {2, 1, "CLASSICAL_REPUBLIC", 8}, {3, 1, "THEOCRACY", 9}, {4, 1, "DIGITAL_DEMOCRACY", 6}, {5, 1, "MONARCHY", 8} },
		want = { 1, "THEOCRACY", 1 } },
	{ game = "20261120T132528Z-cont1", turn = 154, kind = "WORLD_IDEOLOGY",
		targets = { "AUTOCRACY", "OLIGARCHY", "CLASSICAL_REPUBLIC", "MONARCHY", "THEOCRACY", "MERCHANT_REPUBLIC", "FASCISM", "COMMUNISM", "DEMOCRACY", "CORPORATE_LIBERTARIANISM", "DIGITAL_DEMOCRACY", "SYNTHETIC_TECHNOCRACY" },
		ours = { 1, "FASCISM", 1 }, spends = { [1] = 116, [2] = 108, [3] = 188, [4] = 80, [5] = 96 },
		governments = { [1] = "SYNTHETIC_TECHNOCRACY", [3] = "SYNTHETIC_TECHNOCRACY", [4] = "FASCISM" }, session = { "WORLD_IDEOLOGY", "LUXURY", "DIPLOVICTORY" },
		voters = { {1, 1, "DIGITAL_DEMOCRACY", 5}, {2, 1, "FASCISM", 6}, {3, 1, "SYNTHETIC_TECHNOCRACY", 4}, {4, 1, "FASCISM", 2}, {5, 1, "MONARCHY", 4} },
		want = { 1, "SYNTHETIC_TECHNOCRACY", 1 } },
	{ game = "20261120T143159Z", turn = 112, kind = "WORLD_IDEOLOGY",
		targets = { "AUTOCRACY", "OLIGARCHY", "CLASSICAL_REPUBLIC", "MONARCHY", "THEOCRACY", "MERCHANT_REPUBLIC", "FASCISM", "COMMUNISM", "DEMOCRACY", "CORPORATE_LIBERTARIANISM", "DIGITAL_DEMOCRACY", "SYNTHETIC_TECHNOCRACY" },
		ours = { 1, "FASCISM", 1 }, spends = { [1] = 112, [2] = 76, [3] = 40, [4] = 92, [5] = 8 },
		governments = { [1] = "MONARCHY", [2] = "CLASSICAL_REPUBLIC", [3] = "CLASSICAL_REPUBLIC", [4] = "MONARCHY" }, session = { "ESPIONAGE_PACT", "WORLD_IDEOLOGY", "DIPLOVICTORY" },
		voters = { {1, 1, "MONARCHY", 5}, {2, 1, "CLASSICAL_REPUBLIC", 6}, {3, 1, "CLASSICAL_REPUBLIC", 4}, {4, 1, "MONARCHY", 5}, {5, 1, "AUTOCRACY", 2} },
		want = { 1, "MONARCHY", 1 } },
	{ game = "20261120T150010Z", turn = 112, kind = "WORLD_IDEOLOGY",
		targets = { "AUTOCRACY", "OLIGARCHY", "CLASSICAL_REPUBLIC", "MONARCHY", "THEOCRACY", "MERCHANT_REPUBLIC", "FASCISM", "COMMUNISM", "DEMOCRACY", "CORPORATE_LIBERTARIANISM", "DIGITAL_DEMOCRACY", "SYNTHETIC_TECHNOCRACY" },
		ours = { 1, "FASCISM", 1 }, spends = { [1] = 52, [2] = 52, [3] = 320, [4] = 132, [5] = 4 },
		governments = { [2] = "FASCISM", [4] = "CLASSICAL_REPUBLIC", [5] = "OLIGARCHY" }, session = { "WORLD_IDEOLOGY", "MIGRATION_TREATY", "DIPLOVICTORY" },
		voters = { {1, 1, "MONARCHY", 3}, {2, 1, "FASCISM", 4}, {3, 1, "MONARCHY", 8}, {4, 1, "CLASSICAL_REPUBLIC", 6}, {5, 1, "OLIGARCHY", 1} },
		want = { 1, "CLASSICAL_REPUBLIC", 1 } },
}

-- 6. The recorded resolutions the arm changes. Live Deity, 2026-11-19 12:00Z
-- to 11-20, 746 regular resolutions of 55 games: the arm changes these 44.
-- Each is re-run with the recorded rival votes deciding and our vote
-- replaced by the arm's (a tie for the top block scores nothing; one of
-- these, civvis-20261120T103014Z t132, was a tie the host broke for us).
-- Spends are what each rival paid that session (recorded where the log has
-- `favor_entering`, else rebuilt from its votes at the Online prices).
local function full(kind, t)
	if kind == "MERCENARY_COMPANIES" then return yield(t) end
	if kind == "WORLD_IDEOLOGY" then return gov(t) end
	return tostring(tonumber(t))
end
local function winner(voters, ours)
	local a, b, blocks = 0, 0, {}
	local function add(option, target, votes)
		if option == 1 then a = a + votes else b = b + votes end
		local key = option .. "|" .. target
		blocks[key] = (blocks[key] or 0) + votes
	end
	for _, v in ipairs(voters) do add(v[1], v[2], v[3]) end
	add(ours[1], ours[2], ours[3])
	if a == b then return nil end
	local option = a > b and 1 or 2
	local top, best, tied = -1, nil, false
	for key, votes in pairs(blocks) do
		if tonumber(key:sub(1, 1)) == option then
			if votes > top then top, best, tied = votes, key, false
			elseif votes == top then tied = true end
		end
	end
	return (not tied) and best or nil
end
local replay = {}
local matched = 0
for _, row in ipairs(RECORDED) do
	local kind = row.kind
	local targets, selection = {}, nil
	for i, t in ipairs(row.targets) do
		targets[i] = kind == "MIGRATION_TREATY" and tonumber(t) or full(kind, t)
		if full(kind, t) == full(kind, row.ours[2]) then selection = i end
	end
	local candidates, seen = {}, {}
	for _, v in ipairs(row.voters) do
		if not seen[v[1]] then
			seen[v[1]] = true
			local paid = row.spends and row.spends[v[1]]
			candidates[#candidates + 1] = paid and { id = v[1], favor = 0, favor_entering = paid } or { id = v[1] }
		end
	end
	local entries = {}
	for i, name in ipairs(row.session or {}) do entries[i] = "WC_RES_" .. name end
	local o, s, n = choose({ rtype = "WC_RES_" .. kind, targets = targets, candidates = candidates,
		option = row.ours[1], selection = selection, votes = row.ours[3], entries = entries,
		row = { DisfavoredPlayerIDs = row.disfavored or {} },
		governmentOf = function(id) return row.governments and row.governments[id] and gov(row.governments[id]) end,
		last = row.last and { ["WC_RES_" .. kind] = { option = row.last.option,
			target = full(kind, row.last.target) } } or nil })
	local got = o and (o .. "/" .. tostring(targets[s]) .. "/" .. n) or "unchanged"
	local want = row.want[1] .. "/" .. full(kind, row.want[2]) .. "/" .. row.want[3]
	if got == want then matched = matched + 1 else
		check(row.game .. " t" .. row.turn .. " " .. kind, got, want)
	end
	local voters = {}
	for _, v in ipairs(row.voters) do voters[#voters + 1] = { v[2], full(kind, v[3]), v[4] } end
	local was = { row.ours[1], full(kind, row.ours[2]), row.ours[3] }
	local now = o and { o, tostring(targets[s]), n } or was
	local r = replay[kind] or { n = 0, before = 0, after = 0 }
	replay[kind] = r
	r.n = r.n + 1
	if winner(voters, was) == was[1] .. "|" .. was[2] then r.before = r.before + 1 end
	if winner(voters, now) == now[1] .. "|" .. now[2] then r.after = r.after + 1 end
end
check("every recorded choice is the arm's", matched, #RECORDED)
check("44 recorded resolutions changed", #RECORDED, 44)
local function line(kind)
	local r = replay[kind] or {}
	return (r.n or 0) .. ": " .. (r.before or 0) .. " -> " .. (r.after or 0)
end
check("Migration Treaty, 29 changed: 4 points -> 21", line("MIGRATION_TREATY"), "29: 4 -> 21")
check("World Ideology, 12 changed: 3 -> 9", line("WORLD_IDEOLOGY"), "12: 3 -> 9")
check("Mercenary Companies, 3 changed: 0 -> 3", line("MERCENARY_COMPANIES"), "3: 0 -> 3")

if failures > 0 then
	print(failures .. " failure(s)")
	os.exit(1)
end
print("wc join plurality: all ok")
