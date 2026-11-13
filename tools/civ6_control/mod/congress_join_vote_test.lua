-- Offline regression for the World Congress vote that joins an Aid Request or
-- a Scored Competition.
--
-- Gathering Storm admits a civilization to those by its vote in favour of the
-- proposal ("If the Discussion passes, you will join the resulting Scored
-- Competition. Your first vote is free."). The seat's ballot sent resolution
-- votes only, so it was never a member of any. This loads the actual agent and
-- drives `CivvisCongressJoin.choose` (which proposals) and
-- `CivvisCongressJoin.cast` (the operations sent) against a recorded host.
--
-- Run: lua5.1 tools/civ6_control/mod/congress_join_vote_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end

setmetatable(_G, { __index = function(_, key)
	if key == "CivvisCongressJoin" then return rawget(_G, key) end
	return stub()
end })

local logs = {}
Automation = { Log = function(line) logs[#logs + 1] = line end }
UI = { DataError = function() end }
CivvisControlConfig = { Play = true, VictoryLane = "diplomatic" }
local config = CivvisControlConfig

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtimeErr = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtimeErr))

local join = rawget(_G, "CivvisCongressJoin")
assert(type(join) == "table" and type(join.choose) == "function" and type(join.cast) == "function",
	"CivvisControlAgent.lua did not export CivvisCongressJoin.choose / .cast")

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", name, tostring(got)))
	end
end

local PID = 0

-- `GetProposals(pid).Proposals` as the shipped screen reads it: keyed by
-- proposal type, each holding `ProposalsOfType`.
local function session(...)
	local categories = {}
	for index, prop in ipairs({ ... }) do
		categories[100 + index] = { ProposalsOfType = { prop } }
	end
	return categories
end

local function names(rows)
	local out = {}
	for _, row in ipairs(rows) do
		out[#out + 1] = row.type:gsub("^WC_EMERGENCY_", "") .. "@" .. tostring(row.target)
			.. (row.why and (":" .. row.why) or "")
	end
	return table.concat(out, " ")
end

local function choose(lane, categories, opts)
	opts = opts or {}
	local cfgRow = { VictoryLane = lane }
	for key, value in pairs(opts.config or {}) do cfgRow[key] = value end
	local asks, skipped = join.choose(PID, categories, cfgRow,
		function(target) return opts.war ~= nil and opts.war[target] == true end,
		function(restriction) return opts.blockers ~= nil and opts.blockers[restriction] or nil end)
	return names(asks), names(skipped)
end

local every = function()
	return session(
		{ TypeName = "WC_EMERGENCY_REQUEST_AID", Sender = 3, Target = 3 },
		{ TypeName = "WC_EMERGENCY_MILITARY_AID_REQUEST", Sender = 4, Target = 4 },
		{ TypeName = "WC_EMERGENCY_CLIMATE_ACCORDS", Target = -1 },
		{ TypeName = "WC_EMERGENCY_WORLD_FAIR", Target = -1 },
		{ TypeName = "WC_EMERGENCY_WORLD_GAMES", Target = -1 },
		{ TypeName = "WC_EMERGENCY_SPACE_STATION", Target = -1 },
		{ TypeName = "WC_EMERGENCY_NOBEL_PRIZE_LITERATURE", Target = -1 },
		{ TypeName = "WC_EMERGENCY_NOBEL_PRIZE_PEACE", Target = -1 },
		{ TypeName = "WC_EMERGENCY_NOBEL_PRIZE_PHYSICS", Target = -1 },
		-- The war emergencies and the ordinary discussions are never ours to join.
		{ TypeName = "WC_EMERGENCY_MILITARY", Sender = 2, Target = 5 },
		{ TypeName = "WC_EMERGENCY_CITY_STATE", Sender = 2, Target = 5 },
		{ TypeName = "WC_EMERGENCY_RELIGIOUS", Sender = 2, Target = 5 },
		{ TypeName = "WC_EMERGENCY_NUCLEAR", Sender = 2, Target = 5 },
		{ TypeName = "WC_EMERGENCY_BACKSTAB", Sender = 2, Target = 5 },
		{ TypeName = "WC_DENOUNCE_GRIEVANCES", Sender = 2, Target = 5 })
end

-- 1. A Diplomatic seat joins every scored kind and no war emergency.
do
	local asks, skipped = choose("diplomatic", every())
	check("diplomatic seat joins all nine", asks,
		"CLIMATE_ACCORDS@-1 MILITARY_AID_REQUEST@4 NOBEL_PRIZE_LITERATURE@-1 NOBEL_PRIZE_PEACE@-1 "
		.. "NOBEL_PRIZE_PHYSICS@-1 REQUEST_AID@3 SPACE_STATION@-1 WORLD_FAIR@-1 WORLD_GAMES@-1")
	check("diplomatic seat skips nothing of those kinds", skipped, "")
end

-- 2. A Science seat joins the Space Station and the Physics prize only.
do
	local asks, skipped = choose("science", every())
	check("science seat joins two", asks, "NOBEL_PRIZE_PHYSICS@-1 SPACE_STATION@-1")
	check("science seat leaves the rest", skipped,
		"CLIMATE_ACCORDS@-1:not_this_lane MILITARY_AID_REQUEST@4:not_this_lane "
		.. "NOBEL_PRIZE_LITERATURE@-1:not_this_lane NOBEL_PRIZE_PEACE@-1:not_this_lane "
		.. "REQUEST_AID@3:not_this_lane WORLD_FAIR@-1:not_this_lane WORLD_GAMES@-1:not_this_lane")
end

-- 3. A Culture seat joins the Fair, the Games and the Literature prize.
check("culture seat joins three", (choose("culture", every())),
	"NOBEL_PRIZE_LITERATURE@-1 WORLD_FAIR@-1 WORLD_GAMES@-1")

-- 4. Every other seat, and a run with no lane, votes as before.
for _, lane in ipairs({ "domination", "score", "religious", "civvis" }) do
	check(lane .. " seat joins none", (choose(lane, every())), "")
end
check("no lane joins none", (choose(nil, every())), "")

-- 5. The existing AutoJoin flags turn a kind off.
do
	local asks, skipped = choose("diplomatic", session(
		{ TypeName = "WC_EMERGENCY_REQUEST_AID", Sender = 3, Target = 3 },
		{ TypeName = "WC_EMERGENCY_WORLD_GAMES", Target = -1 }),
		{ config = { AutoJoinAidRequests = false } })
	check("flag off: the other kind still joins", asks, "WORLD_GAMES@-1")
	check("flag off: the kind is reported disabled", skipped, "REQUEST_AID@3:disabled")
end

-- 6. What the host forbids, our own request, and a rival at war are left alone.
do
	local blockers = {
		[11] = { NoUpvote = true, NoDownvote = true },   -- have not met the target
		[12] = { NoDownvote = true },                    -- only the down vote is barred
	}
	local asks, skipped = choose("diplomatic", session(
		{ TypeName = "WC_EMERGENCY_REQUEST_AID", Sender = 1, Target = 1, VotingRestriction = 11 },
		{ TypeName = "WC_EMERGENCY_REQUEST_AID", Sender = 2, Target = 2, VotingRestriction = 12 },
		{ TypeName = "WC_EMERGENCY_REQUEST_AID", Sender = 3, Target = 3, Disabled = true },
		{ TypeName = "WC_EMERGENCY_REQUEST_AID", Sender = PID, Target = PID },
		{ TypeName = "WC_EMERGENCY_REQUEST_AID", Sender = 4, Target = 4 },
		{ TypeName = "WC_EMERGENCY_MILITARY_AID_REQUEST", Sender = 5, Target = 5 }),
		{ blockers = blockers, war = { [4] = true } })
	check("only the open requests join", asks, "MILITARY_AID_REQUEST@5 REQUEST_AID@2")
	check("each other one says why", skipped,
		"REQUEST_AID@0:target_is_local REQUEST_AID@1:no_upvote "
		.. "REQUEST_AID@3:proposal_disabled REQUEST_AID@4:at_war")
end

-- 7. `cast` sends the shipped operation: Sender, Target, one vote in favour;
-- once a turn; and reports it.
local requests = {}
PlayerOperations = {
	WORLD_CONGRESS_DISCUSSION_VOTE = "DISCUSSION_VOTE",
	PARAM_PLAYER_ONE = "ONE", PARAM_PLAYER_TWO = "TWO",
	PARAM_WORLD_CONGRESS_VOTES = "VOTES",
}
UI = {
	DataError = function() end,
	RequestPlayerOperation = function(pid, operation, params)
		requests[#requests + 1] = { pid = pid, operation = operation, params = params }
	end,
}
local live = nil
Game = {
	GetWorldCongress = function()
		return { GetProposals = function(_, pid) return { Proposals = live } end }
	end,
}
Players = setmetatable({}, { __index = function()
	return { GetDiplomacy = function()
		return { IsAtWarWith = function(_, target) return target == 4 end }
	end }
end })
GameInfo = { VotingBlockers = { [11] = { NoUpvote = true, NoDownvote = true } } }

local function lastJoinVote()
	for i = #logs, 1, -1 do
		if logs[i]:find('"kind":"wc_join_vote"', 1, true) then return logs[i] end
	end
	return nil
end

do
	config.VictoryLane = "diplomatic"
	live = session(
		{ TypeName = "WC_EMERGENCY_REQUEST_AID", Sender = 3, Target = 3 },
		{ TypeName = "WC_EMERGENCY_REQUEST_AID", Sender = 4, Target = 4 },
		{ TypeName = "WC_EMERGENCY_REQUEST_AID", Sender = 1, Target = 1, VotingRestriction = 11 },
		{ TypeName = "WC_EMERGENCY_WORLD_FAIR", Target = -1 },
		{ TypeName = "WC_EMERGENCY_MILITARY", Sender = 2, Target = 5 })
	check("cast returns the votes sent", join.cast(PID, 115), 2)
	check("two operations", #requests, 2)
	table.sort(requests, function(a, b) return a.params.TWO > b.params.TWO end)
	check("aid: operation", requests[1].operation, "DISCUSSION_VOTE")
	check("aid: player", requests[1].pid, PID)
	check("aid: sender", requests[1].params.ONE, 3)
	check("aid: target", requests[1].params.TWO, 3)
	check("aid: one vote in favour", requests[1].params.VOTES, 1)
	check("fair: no sender", requests[2].params.ONE, nil)
	check("fair: target", requests[2].params.TWO, -1)
	check("fair: one vote in favour", requests[2].params.VOTES, 1)
	local line = lastJoinVote()
	check("reported", line ~= nil, true)
	check("report counts the votes", line ~= nil and line:find('"sent":2', 1, true) ~= nil, true)
	check("report names the lane", line ~= nil and line:find('"lane":"diplomatic"', 1, true) ~= nil, true)
	check("report says why the war rival was skipped",
		line ~= nil and line:find('"why":"at_war"', 1, true) ~= nil, true)
	check("report says why the unmet target was skipped",
		line ~= nil and line:find('"why":"no_upvote"', 1, true) ~= nil, true)
	-- A second call in the same turn (the blocker fallback after the popup
	-- ballot) sends nothing more.
	check("same turn: nothing more", join.cast(PID, 115), 0)
	check("same turn: still two operations", #requests, 2)
end

-- 8. A Science seat's regular session: the Space Station only; the World
-- Games proposal beside it is left at the host's default.
do
	requests, logs = {}, {}
	config.VictoryLane = "science"
	live = session(
		{ TypeName = "WC_EMERGENCY_SPACE_STATION", Target = -1 },
		{ TypeName = "WC_EMERGENCY_WORLD_GAMES", Target = -1 })
	check("science cast", join.cast(PID, 175), 1)
	check("science: one operation", #requests, 1)
	check("science: it is the Space Station (no sender, target -1)",
		requests[1].params.TWO, -1)
	local line = lastJoinVote()
	check("science: report names the kind",
		line ~= nil and line:find('"type":"WC_EMERGENCY_SPACE_STATION"', 1, true) ~= nil, true)
end

-- 9. Nothing of these kinds on the table: no operation and no row.
do
	requests, logs = {}, {}
	config.VictoryLane = "diplomatic"
	live = session({ TypeName = "WC_EMERGENCY_MILITARY", Sender = 2, Target = 5 })
	check("war emergency only: nothing sent", join.cast(PID, 196), 0)
	check("war emergency only: no operation", #requests, 0)
	check("war emergency only: no row", lastJoinVote(), nil)
	live = nil
	check("no proposals table: nothing sent", join.cast(PID, 197), 0)
end

-- 10. A host that throws on the operation is reported, and the turn is not
-- latched, so a later call can still try.
do
	requests, logs = {}, {}
	UI.RequestPlayerOperation = function() error("refused") end
	live = session({ TypeName = "WC_EMERGENCY_WORLD_GAMES", Target = -1 })
	check("throw: nothing counted", join.cast(PID, 215), 0)
	local line = lastJoinVote()
	check("throw: reported", line ~= nil and line:find('"why":"throw"', 1, true) ~= nil, true)
	UI.RequestPlayerOperation = function(pid, operation, params)
		requests[#requests + 1] = { pid = pid, operation = operation, params = params }
	end
	check("throw: the same turn may try again", join.cast(PID, 215), 1)
end

-- 11. The ballot casts them before it submits the congress turn: the submit
-- closes the session, and a vote sent after it is not counted.
do
	local handle = assert(io.open(here .. "/CivvisControlAgent.lua", "r"))
	local source = handle:read("*a")
	handle:close()
	local start = source:find("local function voteWorldCongress(pid, ballotTurn)", 1, true)
	local finish = source:find("local function castBallot(trigger)", 1, true)
	check("ballot function found", start ~= nil and finish ~= nil and start < finish, true)
	local body = source:sub(start or 1, finish or 1)
	local castAt = body:find("pcall(CivvisCongressJoin.cast, pid, ballotTurn)", 1, true)
	local submitAt = body:find("PlayerOperations.WORLD_CONGRESS_SUBMIT_TURN, {}", 1, true)
	check("the ballot casts the join votes", castAt ~= nil, true)
	check("before it submits the turn", castAt ~= nil and submitAt ~= nil and castAt < submitAt, true)
end

if failures > 0 then
	print(string.format("%d FAILED", failures))
	os.exit(1)
end
print("all congress join vote checks passed")
