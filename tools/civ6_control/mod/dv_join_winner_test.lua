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

-- Run: lua5.1 tools/civ6_control/mod/dv_join_winner_test.lua
-- The Diplomatic Victory ballot reads who takes the +2 from what the rivals
-- spent this session (`CivvisDvPredict`, recorded every session as
-- `wc_ballot_context.dv_prediction`) and, opt-in (`DvJoinWinner`), votes A on
-- that rival with the free vote when our bank cannot outvote it
-- (`CivvisDvJoinWinner`). Loads the actual agent and drives the real ballot.
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

-- The host's Online table: `costs[k]` is the price of k+1 votes, and it stops
-- at the votes the bank affords (`MaxVotes`).
local function hostCosts(bank)
	local costs = { [0] = 0, [1] = 4 }
	local max = 1
	while 2 * (max + 1) * max <= bank do max = max + 1 end
	for k = 1, max do costs[k] = 2 * (k + 1) * k end
	costs.MaxVotes = max
	return costs
end

-- `CivvisDvVotesForFavor`: the votes a spend buys, past the host's table too.
local online = hostCosts(40)
check("the host table stops at five votes for 40 Favor", online.MaxVotes, 5)
check("nothing spent is the free vote", CivvisDvVotesForFavor(0, online), 1)
check("4 Favor is two votes", CivvisDvVotesForFavor(4, online), 2)
check("11 Favor is still two", CivvisDvVotesForFavor(11, online), 2)
check("40 Favor is five", CivvisDvVotesForFavor(40, online), 5)
check("past the table: 312 Favor is thirteen (4 * 12 * 13 / 2)", CivvisDvVotesForFavor(312, online), 13)
check("past the table: 311 Favor is twelve", CivvisDvVotesForFavor(311, online), 12)
local standard = { [0] = 0, [1] = 10, [2] = 30, MaxVotes = 2 }
check("Standard prices: 60 Favor is four votes", CivvisDvVotesForFavor(60, standard), 4)
check("no price for a second vote reads nothing", CivvisDvVotesForFavor(60, { [0] = 0 }), nil)
check("no spend reads nothing", CivvisDvVotesForFavor(nil, online), nil)
check("a negative spend reads nothing", CivvisDvVotesForFavor(-4, online), nil)

-- The real ballot: a Science seat racing the Congress, four rivals.
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
local bank, ourPoints = 40, 6
-- Each rival: points, Favor entering the session, Favor at our ballot.
local rivals = {}
local session = { "WC_RES_DIPLOVICTORY", "WC_RES_BORDER_CONTROL", "WC_RES_TRADE_TREATY" }
local review = nil
local wc = {
	GetVotesandFavorCost = function() return hostCosts(bank) end,
	GetResolutions = function()
		local list = { Stage = 2147483647 }
		for i, name in ipairs(session) do
			list[i] = { Type = name, TargetType = "PlayerType", PossibleTargets = { 0, 1, 2, 3, 4 } }
		end
		return list
	end,
	GetReview = function() return review end,
}
Game = {
	GetLocalPlayer = function() return 0 end,
	GetCurrentGameTurn = function() return 152 end,
	GetCurrentTurnSegment = function() return "TURNSEG_WORLDCONGRESS_1" end,
	GetWorldCongress = function() return wc end,
}
DB = { MakeHash = function(name) return name end }
Players = { [0] = {
	IsTurnActive = function() return false end,
	GetFavor = function() return bank end,
	GetFavorEnteringCongress = function() return bank end,
	GetStats = function() return {
		GetDiplomaticVictoryPoints = function() return ourPoints end,
	} end,
	GetScore = function() return 900 end,
} }
local enteringReadable = true
for id = 1, 4 do
	local player = id
	Players[id] = {
		GetStats = function() return {
			GetDiplomaticVictoryPoints = function() return rivals[player].points end,
		} end,
		GetScore = function() return 800 + player end,
		GetFavor = function() return rivals[player].now end,
		GetFavorEnteringCongress = function()
			if not enteringReadable then error("not exposed for a rival") end
			return rivals[player].entering
		end,
	}
end
PlayerManager = { GetAliveMajorIDs = function() return { 0, 1, 2, 3, 4 } end }
GameInfo = { Resolutions = {} }
for index, name in ipairs({ "WC_RES_DIPLOVICTORY", "WC_RES_BORDER_CONTROL", "WC_RES_TRADE_TREATY" }) do
	GameInfo.Resolutions[name] = { ResolutionType = name, Hash = index }
end
PlayerOperations = {
	PARAM_RESOLUTION_TYPE = "type", PARAM_WORLD_CONGRESS_VOTES = "votes",
	PARAM_RESOLUTION_OPTION = "option", PARAM_RESOLUTION_SELECTION = "selection",
	WORLD_CONGRESS_RESOLUTION_VOTE = "vote", WORLD_CONGRESS_SUBMIT_TURN = "submit",
}
local requests = {}
UI = { RequestPlayerOperation = function(pid, operation, params)
	assert(pid == 0)
	if operation == "vote" then requests[#requests + 1] = params end
end }
-- What the ballot records: `CivvisCongressChoiceContext` is a global, read at
-- the call, so a wrapper sees every `wc_ballot_context` row.
local contexts = {}
local context = CivvisCongressChoiceContext
CivvisCongressChoiceContext = function(...)
	local row = context(...)
	contexts[#contexts + 1] = row
	return row
end
tick()
local cast = upvalue(hooks.popup, "castBallot")
local vote = upvalue(cast, "voteWorldCongress")
local tally = upvalue(vote, "envoyTally")

local function run(favor, points, field)
	requests, contexts = {}, {}
	bank, ourPoints, rivals = favor, points, field
	tally.wc_rival_block, tally.wc_rival_block_prev = nil, nil
	tally.wc_review_favor = nil
	local _, _, _, _, _, _, mode = vote(0)
	return mode, requests[1], contexts[1]
end
local function field()
	-- Rival 2 spent 288 (twelve votes for that alone, nine spread over the
	-- Diplomatic Victory and Trade Treaty); the rest little.
	return {
		[1] = { points = 7, entering = 30, now = 6 },
		[2] = { points = 8, entering = 300, now = 12 },
		[3] = { points = 5, entering = 9, now = 9 },
		[4] = { points = 6, entering = 40, now = 0 },
	}
end

-- 1. Unarmed: the ballot is today's, and the prediction is recorded.
local mode, dv, row = run(40, 6, field())
check("unarmed: the small bank casts the free vote", mode, "free")
check("unarmed: B as today", dv.option, 2)
check("unarmed: on the leader as today", dv.selection, 2)
check("unarmed: one vote", dv.votes, 1)
local p = row.dv_prediction
check("the Diplomatic Victory row records the prediction", type(p), "table")
check("predicted winner is the biggest spender", p and p.winner, 2)
check("its spend buys twelve votes", p and p.votes_max, 12)
check("spread over two bought resolutions, nine", p and p.votes_split, 9)
check("two resolutions on which rivals buy votes", p and p.multi_vote_resolutions, 2)
check("read from the spend", p and p.source, "spend")
check("our claim bank buys five", p and p.afford, 5)
check("the winner may be joined", p and p.eligible, true)
check("unarmed recorded", p and p.armed, false)
check("unarmed changes nothing", p and p.choice, nil)
local r2 = nil
for _, r in ipairs(row.rivals) do if r.id == 2 then r2 = r end end
check("rivals record their Favor entering", r2 and r2.favor_entering, 300)
check("rivals record their spend", r2 and r2.favor_spent, 288)
check("rivals record the votes it buys", r2 and r2.predicted_votes_max, 12)
check("and spread", r2 and r2.predicted_votes_split, 9)
check("the ballot row's fields are recorded", p and p.row_keys,
	"PossibleTargets,TargetType,Type")
check("no review, no review voters", p and p.review_voters, "")
local others = 0
for _, c in ipairs(contexts) do if c.dv_prediction ~= nil then others = others + 1 end end
check("only the Diplomatic Victory row carries it", others, 1)

-- 2. Armed, the same session: the free vote goes to A on rival 2.
config.DvJoinWinner = true
mode, dv, row = run(40, 6, field())
check("join mode", mode, "join")
check("join is option A", dv.option, 1)
check("join names the predicted winner", dv.selection, 2)
check("join is the free vote", dv.votes, 1)
check("join recorded", row.dv_prediction.choice, "join")
check("join records what it replaced", row.dv_prediction.before_mode, "free")
check("join records the old target", row.dv_prediction.before_target, 2)
check("join records the old option", row.dv_prediction.before_option, 2)
check("the context names the joined target", row.requested.target, 2)

-- 3. A bank that beats the whole spend outbids it, two votes over (not all in).
local f3 = field()
f3[2].entering = 124 -- 112 spent: eight votes
mode, dv, row = run(300, 6, f3)
check("outbid over a block of eight", mode, "outbid")
check("outbid claims A on us", dv.option .. "/" .. dv.selection, "1/0")
check("ten votes, not the bank's twelve", dv.votes, 10)
check("before was a race of twelve", row.dv_prediction.before_votes, 12)
check("the margin is a setting",
	select(3, CivvisDvJoinWinner({ winner = 2, votes_max = 8, votes_split = 8, afford = 12,
		eligible = true }, { 0, 1, 2 }, 0, 1, 1, 12, "race", false,
		{ DvJoinWinner = true, DvJoinMargin = 0 })), 9)
check("never past the bank",
	select(3, CivvisDvJoinWinner({ winner = 2, votes_max = 8, votes_split = 8, afford = 9,
		eligible = true }, { 0, 1, 2 }, 0, 1, 1, 9, "race", false, config)), 9)

-- 4. A bank over the even split but not the whole spend keeps today's claim.
mode, dv, row = run(220, 6, field())
check("the claim stands", mode, "race")
check("the claim is A on us", dv.option .. "/" .. dv.selection, "1/0")
check("the claim keeps its eleven", dv.votes, 11)
check("no choice recorded", row.dv_prediction.choice, nil)
-- The same band without a claim: the bank claims.
check("a free vote in that band claims with the bank",
	select(3, CivvisDvJoinWinner({ winner = 2, votes_max = 12, votes_split = 9, afford = 10,
		eligible = true }, { 0, 1, 2 }, 0, 2, 3, 1, "free", false, config)), 10)
check("a claim at the even split of a spend keeps its votes",
	select(3, CivvisDvJoinWinner({ winner = 2, votes_max = 12, votes_split = 9, afford = 9,
		eligible = true, source = "spend" }, { 0, 1, 2 }, 0, 1, 1, 9, "race", false, config)), 9)
check("a claim that only ties votes already cast joins (a tie loses)",
	select(4, CivvisDvJoinWinner({ winner = 2, votes_max = 9, votes_split = 9, afford = 9,
		eligible = true, source = "review", rival_a = 20, rival_b = 0 }, { 0, 1, 2 }, 0, 1, 1, 9,
		"race", false, config)), "join")
check("a free vote at the even split joins",
	select(4, CivvisDvJoinWinner({ winner = 2, votes_max = 12, votes_split = 9, afford = 9,
		eligible = true }, { 0, 1, 2 }, 0, 2, 3, 1, "free", false, config)), "join")

-- 5. A claim the spend says loses becomes the join, and keeps the Favor.
local f5 = field()
f5[2].entering = 600 -- 588 spent: seventeen votes, twelve spread
mode, dv, row = run(220, 6, f5)
check("losing claim joins", mode, "join")
check("on the winner", dv.option .. "/" .. dv.selection .. "/" .. dv.votes, "1/2/1")
check("replacing a race", row.dv_prediction.before_mode, "race")

-- 6. Paths left alone: the brace, a ganged leader, the deny-floor leader, a
-- winner past the ceiling, a winner the ballot does not offer, no reading.
mode, dv = run(40, 16, field())
check("brace stays", mode, "brace")
check("brace is B on us", dv.option .. "/" .. dv.selection .. "/" .. dv.votes, "2/0/1")
local f6 = field()
f6[2].points = 16
mode, dv = run(40, 6, f6)
check("a leader on the gang threshold is not joined", dv.option .. "/" .. dv.selection, "2/2")
local f7 = field()
f7[2].points = 13
mode, dv, row = run(40, 6, f7)
check("the leader the ballot would deny is not joined", dv.option .. "/" .. dv.selection, "2/2")
check("and not eligible", row.dv_prediction.eligible, false)
-- Tied with a rival we may join: that one, and our vote decides the tie.
local f8 = field()
f8[2].points = 13
f8[1].entering = 6 + 288
mode, dv, row = run(40, 6, f8)
check("a tie goes to the rival we may join", dv.option .. "/" .. dv.selection, "1/1")
check("the tie is recorded", row.dv_prediction.tied, 2)
check("a winner whose +2 reaches the ceiling is not eligible",
	CivvisDvPredict({}, { { id = 1, points = 16, favor = 0, favor_entering = 100 } }, 0,
		online, {}, 0, 1, {}, nil, 2, 5).eligible, false)
check("the ceiling is a setting",
	CivvisDvPredict({}, { { id = 1, points = 16, favor = 0, favor_entering = 100 } }, 0,
		online, {}, 0, 1, { DvJoinCeiling = 20 }, nil, 2, 5).eligible, true)
check("a winner off the ballot is not joined",
	select(4, CivvisDvJoinWinner({ winner = 4, votes_max = 12, votes_split = 9, afford = 2,
		eligible = true }, { 0, 1, 2 }, 0, 2, 3, 1, "free", false, config)), "free")
for _, m in ipairs({ "brace", "deny", "redirect" }) do
	check(m .. " is never replaced",
		select(4, CivvisDvJoinWinner({ winner = 2, votes_max = 12, votes_split = 9, afford = 2,
			eligible = true }, { 0, 1, 2 }, 0, 2, 3, 3, m, false, config)), m)
end
check("a ganged session is never replaced",
	select(4, CivvisDvJoinWinner({ winner = 2, votes_max = 12, votes_split = 9, afford = 2,
		eligible = true }, { 0, 1, 2 }, 0, 2, 3, 1, "free", true, config)), "free")
check("off unless armed",
	select(4, CivvisDvJoinWinner({ winner = 2, votes_max = 12, votes_split = 9, afford = 2,
		eligible = true }, { 0, 1, 2 }, 0, 2, 3, 1, "free", false, {})), "free")
enteringReadable = false
mode, dv, row = run(40, 6, field())
check("no Favor entering, no prediction", row.dv_prediction.winner, nil)
check("no prediction, today's ballot", dv.option .. "/" .. dv.selection .. "/" .. dv.votes, "2/2/1")
check("no prediction, no choice", mode, "free")
enteringReadable = true
-- An entering balance that reads as the current one: no rival bought a
-- vote, so nobody is named and a three-vote claim does not chase phantoms.
local flat = field()
for _, r in pairs(flat) do r.entering = r.now end
mode, dv, row = run(300, 6, flat)
check("no spend names nobody", row.dv_prediction.winner, nil)
check("and says why", row.dv_prediction.why, "no_spend")
check("no spend, today's race", mode, "race")
check("no spend, today's twelve votes", dv.votes, 12)

-- 7. Votes already cast, when the host shows them, are exact. Last session's
-- review carries our vote and is not read as this session's.
review = { Resolutions = { { Type = "WC_RES_DIPLOVICTORY", PlayerSelections = {
	{ PlayerID = 0, OptionChosen = 1, ResolutionTarget = 0, Votes = 9 },
	{ PlayerID = 3, OptionChosen = 1, ResolutionTarget = 3, Votes = 4 },
} } } }
mode, dv, row = run(40, 6, field())
check("last session's review is not this one", row.dv_prediction.source, "spend")
check("and it is recorded", row.dv_prediction.review_voters, "0:1:0:9 3:1:3:4")
check("with our vote in it", row.dv_prediction.review_has_ours, true)
review = { Resolutions = { { Type = "WC_RES_DIPLOVICTORY", PlayerSelections = {
	{ PlayerID = 1, OptionChosen = 1, ResolutionTarget = 1, Votes = 3 },
	{ PlayerID = 3, OptionChosen = 1, ResolutionTarget = 3, Votes = 4 },
	{ PlayerID = 2, OptionChosen = 2, ResolutionTarget = 3, Votes = 2 },
} } } }
mode, dv, row = run(40, 6, field())
check("a review without our vote is this session's", row.dv_prediction.source, "review")
check("its block is exact", row.dv_prediction.votes_max, 4)
check("its winner is exact", row.dv_prediction.winner, 3)
check("our five votes outbid a block of four", mode, "outbid")
check("with the block plus two", dv.votes, 5)
check("rivals record what they cast", row.rivals[3].cast_votes, 4)
review = { Resolutions = { { Type = "WC_RES_DIPLOVICTORY", PlayerSelections = {
	{ PlayerID = 1, OptionChosen = 1, ResolutionTarget = 1, Votes = 3 },
	{ PlayerID = 2, OptionChosen = 2, ResolutionTarget = 0, Votes = 9 },
} } } }
mode, dv = run(40, 6, field())
check("cast votes that give option B: today's ballot", mode, "free")
review = nil
check("selections on the ballot row are read first",
	CivvisDvPredict({ PlayerSelections = { { PlayerID = 1, OptionChosen = 1, ResolutionTarget = 1,
		Votes = 6 } } }, { { id = 1, points = 3 } }, 0, online, {}, 0, 1, {}, nil, 1, 3).source, "ballot")

-- 8. The arm's switch reaches the `loaded` row.
local source = assert(io.open(here .. "/CivvisControlAgent.lua")):read("*a")
check("loaded records the arm", source:find("dv_join_winner = cfg.DvJoinWinner == true", 1, true) ~= nil, true)

config.DvJoinWinner = nil
if failures > 0 then
	print(failures .. " failure(s)")
	os.exit(1)
end
print("dv join winner: all ok")
