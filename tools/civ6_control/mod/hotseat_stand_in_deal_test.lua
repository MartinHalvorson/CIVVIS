-- The check games' stand-in deals (`CivvisTrade.standInDeals`,
-- `CivvisTrade.standInHolds`): config keys only `tools/civ6_hotseat.py
-- --stand-in-deal`, `--stand-in-deal-answer` and `--stand-in-deal-hold`
-- write, and only with `--auto-human`.
--
-- Pinned here: with no key nothing happens; a listed deal goes once, on the
-- stand-in's first turn at or after its turn, through the CIVVIS seats' own
-- `deal` order (a MAKE_DEAL session, the shipped screen's PROPOSED), to the
-- seat named or (`*`) the lowest CIVVIS seat it may ask, and is tried again
-- only for a refusal that passes with time; a deal another seat put to the
-- stand-in is answered only on a later turn than the one it was first seen
-- on, once; the end of turn is held the configured seconds over a deal
-- waiting for the stand-in, and never otherwise; and a CIVVIS seat's own
-- context does none of it.
--
-- Run: lua5.1 tools/civ6_control/mod/hotseat_stand_in_deal_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end
setmetatable(_G, { __index = function(_, key)
	if key == "CivvisTrade" or key == "CivvisApplyOrder" or key == "CivvisOnDiplomacyStatement" then
		return rawget(_G, key)
	end
	return stub()
end })

-- This is the stand-in's context, seat 0 (`--auto-human gated`: the person's
-- seat, played by the test stub); 1, 2 and 3 are CIVVIS seats, 4 a Firaxis
-- AI, 5 a seat not met.
CivvisControlConfig = { Hotseat = true, HotseatSeat = 0, HotseatSeats = { 3, 1, 2 },
	HotseatHumanSeats = { 0 }, HotseatStubSeats = { 0 }, CivvisDecides = false,
	TradeRetryTurns = 3, TradeResponseTurns = 2 }

local events = {}
local realPrint = print
print = function(line)
	if type(line) == "string" and (line:find('"kind":"deal_', 1, true)
			or line:find('"kind":"stand_in_deal', 1, true)) then
		events[#events + 1] = line
	end
end

DealDirection = { OUTGOING = "outgoing", INCOMING = "incoming" }
DealItemTypes = { RESOURCES = "resources", FAVOR = "favor", GOLD = "gold",
	AGREEMENTS = "agreements", GREATWORK = "greatwork", CAPTIVE = "captive", CITIES = "cities" }
DealAgreementTypes = { OPEN_BORDERS = "open_borders", JOINT_WAR = "joint_war" }
DealProposalAction = { PROPOSED = "PROPOSED", ACCEPTED = "ACCEPTED", REJECTED = "REJECTED",
	ADJUSTED = "ADJUSTED", CLOSED = "CLOSED", EQUALIZE = "EQUALIZE" }
local resourceRows = {
	RESOURCE_DYES = { ResourceType = "RESOURCE_DYES", Index = 12, ResourceClassType = "RESOURCECLASS_LUXURY" },
	RESOURCE_IRON = { ResourceType = "RESOURCE_IRON", Index = 44, ResourceClassType = "RESOURCECLASS_STRATEGIC" },
	RESOURCE_SILK = { ResourceType = "RESOURCE_SILK", Index = 30, ResourceClassType = "RESOURCECLASS_LUXURY" },
}
GameInfo = {
	Resources = setmetatable({}, { __index = function(_, key)
		if type(key) == "number" then
			for _, row in pairs(resourceRows) do if row.Index == key then return row end end
			return nil
		end
		return resourceRows[key]
	end, __call = function() local rows, i = {}, 0
		for _, row in pairs(resourceRows) do rows[#rows + 1] = row end
		return function() i = i + 1; return rows[i] end
	end }),
	GreatWorks = { [61] = { GreatWorkType = "GREATWORK_POE_1", GreatWorkObjectType = "GREATWORKOBJECT_WRITING" } },
	Resource_Consumption = {},
}

local turn = 30
Game = {
	GetLocalPlayer = function() return 0 end,
	GetCurrentGameTurn = function() return turn end,
}
local humans = { [0] = true, [1] = true, [2] = true, [3] = true, [4] = false, [5] = true }
Players = setmetatable({}, { __index = function(_, id)
	return { IsHuman = function() return humans[id] end, IsMajor = function() return true end }
end })
PlayerManager = { GetAliveMajorIDs = function() return { 0, 1, 2, 3, 4, 5 } end }
local met = { [1] = true, [2] = true, [3] = true, [4] = true }
local diplomacy = {
	HasMet = function(_, other) return met[other] == true end,
	IsAtWarWith = function() return false end,
}
local player = { GetDiplomacy = function() return diplomacy end }

-- The deal manager. `pending[a][b]` = HasPendingDeal(a, b); working deals are
-- item lists by direction and pair; `possible[from]` lists what `from` can
-- put on the table. Sends move the pending flags the way the screen expects.
local pending, working, possible, log = {}, {}, {}, {}
local opts = {}
local function note(entry) log[#log + 1] = entry end
local function deal(dir, a, b)
	working[dir] = working[dir] or {}
	working[dir][a] = working[dir][a] or {}
	working[dir][a][b] = working[dir][a][b] or {}
	local list = working[dir][a][b]
	local d = {}
	function d:Items()
		local i = 0
		return function()
			i = i + 1
			local item = list[i]
			if item == nil then return nil end
			return {
				GetType = function() return item.kind end,
				GetFromPlayerID = function() return item.from end,
				GetDuration = function() return item.duration or 0 end,
				GetAmount = function() return item.amount end,
				GetValueType = function() return item.value end,
				GetSubType = function() return item.sub end,
			}
		end
	end
	function d:GetItemCount() return #list end
	function d:AddItemOfType(kind, from)
		local item = { kind = kind, from = from, id = #list + 1 }
		list[#list + 1] = item
		return {
			SetDuration = function(_, v) item.duration = v end,
			SetAmount = function(_, v) item.amount = v end,
			SetValueType = function(_, v) item.value = v end,
			SetSubType = function(_, v) item.sub = v end,
			GetMaxAmount = function() return (opts.max or {})[kind] or 999 end,
			IsValid = function() return opts.itemValid ~= false end,
			GetID = function() return item.id end,
		}
	end
	function d:RemoveItemByID(id)
		for i, item in ipairs(list) do if item.id == id then table.remove(list, i); return end end
	end
	function d:Validate() note("validate") end
	function d:IsValid() return opts.dealValid ~= false end
	return d
end
local function items(dir, a, b) return ((working[dir] or {})[a] or {})[b] or {} end
DealManager = {
	HasPendingDeal = function(a, b) return (pending[a] or {})[b] == true end,
	GetWorkingDeal = function(dir, a, b) return deal(dir, a, b) end,
	ClearWorkingDeal = function(dir, a, b)
		note("clear:" .. a .. ">" .. b)
		working[dir] = working[dir] or {}
		working[dir][a] = working[dir][a] or {}
		working[dir][a][b] = {}
	end,
	GetPossibleDealItems = function(from, to, kind) return ((possible[from] or {})[kind]) or {} end,
	CopyIncomingToOutgoingWorkingDeal = function(a, b)
		note("copy:" .. a .. ">" .. b)
		local copy = {}
		for i, item in ipairs(items("incoming", a, b)) do copy[i] = item end
		working.outgoing = working.outgoing or {}
		working.outgoing[a] = working.outgoing[a] or {}
		working.outgoing[a][b] = copy
	end,
	AreWorkingDealsEqual = function(a, b)
		if opts.unequal then return false end
		return #items("incoming", a, b) == #items("outgoing", a, b)
	end,
	SendWorkingDeal = function(action, a, b)
		note("send:" .. tostring(action) .. ":" .. a .. ">" .. b)
		if action == "PROPOSED" or action == "ADJUSTED" then
			pending[a] = pending[a] or {}; pending[a][b] = true
		elseif action == "CLOSED" then
			if pending[a] then pending[a][b] = nil end
		else
			if pending[b] then pending[b][a] = nil end
		end
	end,
}
local openStatement = true
DiplomacyManager = {
	GetKeyName = function(v) return v end,
	FindOpenSessionID = function() return nil end,
	HasQueuedSession = function() return false end,
	RequestSession = function(a, b, kind)
		note("session:" .. a .. ">" .. b .. ":" .. kind)
		-- Our own opening statement, delivered inside the request as the
		-- engine may.
		if openStatement then
			CivvisOnDiplomacyStatement(a, b, { SessionID = 55, StatementType = "MAKE_DEAL" })
		end
	end,
	CloseSession = function(id) note("close:" .. tostring(id)) end,
}
NotificationTypes = { DIPLOMACY_SESSION = 91 }
NotificationManager = { GetList = function() return {} end, FindType = function() return nil end }

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtimeErr = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtimeErr))
local applyOrder = rawget(_G, "CivvisApplyOrder")
local trade = rawget(_G, "CivvisTrade")
assert(type(applyOrder) == "function" and type(trade.seatOrder) == "function")
for i = #events, 1, -1 do events[i] = nil end

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		realPrint(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		realPrint(string.format("ok   %s = %s", name, tostring(got)))
	end
end
local function reset()
	for i = #log, 1, -1 do log[i] = nil end
	for i = #events, 1, -1 do events[i] = nil end
end
local function logged(prefix)
	local n = 0
	for _, entry in ipairs(log) do if entry:sub(1, #prefix) == prefix then n = n + 1 end end
	return n
end
local function count(kind)
	local n = 0
	for _, line in ipairs(events) do
		if line:find('"kind":"' .. kind .. '"', 1, true) then n = n + 1 end
	end
	return n
end
local function last(kind)
	for i = #events, 1, -1 do
		if events[i]:find('"kind":"' .. kind .. '"', 1, true) then return events[i] end
	end
end
local function first(kind)
	for _, line in ipairs(events) do
		if line:find('"kind":"' .. kind .. '"', 1, true) then return line end
	end
end
local function field(line, key)
	if line == nil then return nil end
	local quoted = line:match('"' .. key .. '":"([^"]*)"')
	if quoted ~= nil then return quoted end
	return line:match('"' .. key .. '":([^,}]*)')
end
local clock = 100
CivvisClock.now = function() return clock end
local cfg = CivvisControlConfig
local function play(t)
	turn = t
	reset()
	trade.seatBoard(player, 0, t)
	trade.standInDeals(player, 0, t)
end

check("the stand-in's context stands in", CivvisSeat.standsIn(), true)

-- No key: nothing is proposed, answered or held.
pending = { [1] = { [0] = true } }
play(3)
check("no key, no session", logged("session:"), 0)
check("no key, no record", count("stand_in_deal"), 0)
check("no key, no hold", trade.holdsEndTurn(3), false)
check("no key, no hold record", count("stand_in_deal_hold"), 0)
pending = {}

cfg.StandInDeals = {
	{ turn = 5, to = -1, verb = "PROPOSE;give=GOLD=25;get=" },
	{ turn = 7, to = 3, verb = "PROPOSE;give=GOLD=5;get=GOLD=4@30" },
	{ turn = 7, to = 4, verb = "PROPOSE;give=GOLD=5;get=" },
}
met = {}
play(4)
check("before its turn, nothing", count("stand_in_deal"), 0)
play(5)
check("no CIVVIS seat met: says so", field(last("stand_in_deal"), "why"), "deal_no_seat")
check("and tries again later", field(last("stand_in_deal"), "done"), "false")
check("no session yet", logged("session:"), 0)
met = { [2] = true, [3] = true, [4] = true }
pending = { [0] = { [3] = true } }
play(6)
check("the lowest seat it may ask: 2 (3 has our deal pending)", logged("session:0>2:MAKE_DEAL"), 1)
check("sent as the deal screen sends it", logged("send:PROPOSED:0>2"), 1)
check("one send", logged("send:"), 1)
check("the record names the seat", field(last("stand_in_deal"), "to"), "2")
check("ok", field(last("stand_in_deal"), "ok"), "true")
check("done", field(last("stand_in_deal"), "done"), "true")
check("the session closed", logged("close:55"), 1)
check("the send says how long its session took", field(last("deal_answer"), "held_ms"), "0")
trade.standInDeals(player, 0, 6)
check("once a turn", logged("session:"), 1)
-- Turn 7: the second deal waits for the deal already pending with seat 3,
-- the third is a Firaxis AI's lane and ends.
play(7)
check("a pending deal in the way is tried again", field(first("stand_in_deal"), "why"), "deal_pending_exists")
check("two tries that turn", count("stand_in_deal"), 2)
check("a Firaxis AI ends it", field(last("stand_in_deal"), "why"), "deal_not_human")
check("no session for either", logged("session:"), 0)
pending = {}
play(8)
check("the second deal goes once the way is clear", logged("session:0>3:MAKE_DEAL"), 1)
check("with its own terms", logged("send:PROPOSED:0>3"), 1)
check("the first is not sent again", logged("session:0>2"), 0)
play(9)
check("then nothing more", logged("session:"), 0)
check("and no record", count("stand_in_deal"), 0)
-- Two sendable on one turn (both outside the mod's five-turn cooldown):
-- one session a turn, the other the next.
trade.standIn.done = {}
cfg.StandInDeals = {
	{ turn = 9, to = 1, verb = "PROPOSE;give=GOLD=5;get=" },
	{ turn = 9, to = 2, verb = "PROPOSE;give=GOLD=6;get=" },
}
met[1] = true
pending = {}
play(12)
check("one deal sent a turn", logged("session:"), 1)
check("the first listed", logged("session:0>1:MAKE_DEAL"), 1)
pending = {}
play(13)
check("the other the next turn", logged("session:0>2:MAKE_DEAL"), 1)
check("only it", logged("session:"), 1)
cfg.StandInDeals = nil
pending = {}

-- A deal seat 1 put to the stand-in: seen on turn 20, answered on turn 21.
cfg.StandInDealAnswer = "ACCEPT"
met[1] = true
pending = { [1] = { [0] = true } }
working.incoming = { [0] = { [1] = {
	{ kind = "gold", from = 1, amount = 30, duration = 0 },
} } }
play(20)
check("not on the turn it is first seen", logged("session:"), 0)
check("seen", trade.seat.seen[1] ~= nil, true)
local seenId = trade.seat.seen[1].id
play(21)
check("answered the next turn", logged("session:0>1:MAKE_DEAL"), 1)
check("accepted as the screen accepts", logged("send:ACCEPTED:0>1"), 1)
check("the record carries the deal's id", tonumber(field(last("stand_in_deal"), "id")), seenId)
check("and the answer", field(last("stand_in_deal"), "answer"), "ACCEPT")
pending = { [1] = { [0] = true } }
play(22)
check("the same deal is answered once", logged("session:"), 0)
cfg.StandInDealAnswer = nil
pending = {}

-- The hold: 6 s from the first end-turn request of a turn with a deal waiting.
cfg.StandInDealHoldSeconds = 6
turn = 23
reset()
clock = 200
check("nothing waiting: no hold", trade.holdsEndTurn(23), false)
check("and no record", count("stand_in_deal_hold"), 0)
pending = { [2] = { [0] = true } }
check("a deal that came later the same turn changes nothing", trade.holdsEndTurn(23), false)
turn = 24
clock = 300
check("a deal waiting: held", trade.holdsEndTurn(24), true)
check("holding record", field(last("stand_in_deal_hold"), "phase"), "holding")
check("from seat 2", field(last("stand_in_deal_hold"), "from"), "2")
clock = 305.5
check("still held", trade.holdsEndTurn(24), true)
check("one holding record", count("stand_in_deal_hold"), 1)
clock = 306.2
check("released after its seconds", trade.holdsEndTurn(24), false)
check("released record", field(last("stand_in_deal_hold"), "phase"), "released")
check("and stays released", trade.holdsEndTurn(24), false)
check("released once", count("stand_in_deal_hold"), 2)

-- A CIVVIS seat's context (deciding, not the stub's seat) does none of it.
trade.standIn.done, trade.standIn.answered, trade.standIn.hold = {}, {}, nil
pending = { [1] = { [0] = true } }
cfg.CivvisDecides = true
cfg.StandInDeals = { { turn = 1, to = 2, verb = "PROPOSE;give=GOLD=5;get=" } }
cfg.StandInDealAnswer = "ACCEPT"
check("a deciding seat does not stand in", CivvisSeat.standsIn(), false)
turn = 25
clock = 400
play(25)
check("it proposes nothing", logged("session:"), 0)
check("it holds nothing", trade.holdsEndTurn(25), false)
check("no stand-in record", count("stand_in_deal") + count("stand_in_deal_hold"), 0)

print = realPrint
if failures > 0 then
	print(string.format("%d stand-in deal check(s) failed", failures))
	os.exit(1)
end
print("all stand-in deal checks passed")
