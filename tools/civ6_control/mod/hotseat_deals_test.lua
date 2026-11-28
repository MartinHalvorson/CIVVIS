-- Hotseat deals v1, the reading half: `CivvisTrade.seatBoard` (the board's
-- `deals`, with `deal_incoming`, `deal_outcome` and `deal_probe`) and
-- `CivvisTrade.seatHeard` (the answer to our proposal, heard on the other
-- seat's turn). The contract is
-- ~/civvis-civ6-runs/hotseat/findings/trade-deals-design.md.
--
-- Loads the shipped agent and drives the real functions against a deal
-- manager fixture: a pending deal from another seat is listed with both sides
-- and a stable id, read from our INCOMING working deal or, when that is empty,
-- from their OUTGOING one; a working deal nobody has proposed is not; our own
-- pending deal is tracked to its outcome; nothing is ever sent; outside
-- hotseat the board is nil and the ledger silent.
--
-- Run: lua5.1 tools/civ6_control/mod/hotseat_deals_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end
setmetatable(_G, { __index = function(_, key)
	if key == "CivvisTrade" or key == "CivvisSeat" then return rawget(_G, key) end
	return stub()
end })

-- Seat 2 is this context's; 1, 2 and 3 are CIVVIS seats, 0 is the person and
-- 4 a Firaxis AI.
CivvisControlConfig = { Hotseat = true, HotseatSeat = 2, HotseatSeats = { 1, 2, 3 },
	HotseatHumanSeats = { 0 } }

local events = {}
local realPrint = print
print = function(line)
	if type(line) == "string" and line:find('"kind":"deal_', 1, true) then
		events[#events + 1] = line
	end
end

DealDirection = { OUTGOING = "outgoing", INCOMING = "incoming" }
DealItemTypes = { RESOURCES = "resources", FAVOR = "favor", GOLD = "gold",
	AGREEMENTS = "agreements", GREATWORK = "greatwork", CAPTIVE = "captive", CITIES = "cities" }
DealAgreementTypes = { OPEN_BORDERS = "open_borders", JOINT_WAR = "joint_war" }
DealProposalAction = { PROPOSED = 4, ACCEPTED = 1, REJECTED = 2, ADJUSTED = 3, CLOSED = 8 }
NotificationTypes = { DIPLOMACY_SESSION = 91 }
GameInfo = {
	Resources = { [12] = { ResourceType = "RESOURCE_DYES" }, [44] = { ResourceType = "RESOURCE_IRON" } },
	GreatWorks = { [61] = { GreatWorkType = "GREATWORK_POE_1" } },
}

local turn = 20
local localPlayer = 2
Game = {
	GetLocalPlayer = function() return localPlayer end,
	GetCurrentGameTurn = function() return turn end,
}
local humans = { [0] = true, [1] = true, [2] = true, [3] = true, [4] = false }
Players = setmetatable({}, { __index = function(_, id)
	return { IsHuman = function() return humans[id] end }
end })
PlayerManager = { GetAliveMajorIDs = function() return { 0, 1, 2, 3, 4 } end }

-- The fixture: `pending[from][to]` is HasPendingDeal(from, to); `working[dir]
-- [a][b]` is the item list of GetWorkingDeal(dir, a, b). Every send counts.
local pending, working, sends = {}, {}, 0
local notifications = {}
local function deal(list)
	local d = {}
	function d:Items()
		local i = 0
		return function()
			i = i + 1
			local spec = list[i]
			if spec == nil then return nil end
			return {
				GetType = function() return spec.kind end,
				GetFromPlayerID = function() return spec.from end,
				GetDuration = function() return spec.duration or 0 end,
				GetAmount = function() return spec.amount end,
				GetValueType = function() return spec.value end,
				GetSubType = function() return spec.sub end,
			}
		end
	end
	function d:GetItemCount() return #list end
	return d
end
DealManager = {
	HasPendingDeal = function(a, b) return (pending[a] or {})[b] == true end,
	GetWorkingDeal = function(dir, a, b)
		return deal(((working[dir] or {})[a] or {})[b] or {})
	end,
	SendWorkingDeal = function() sends = sends + 1 end,
	ClearWorkingDeal = function() sends = sends + 1 end,
	CopyIncomingToOutgoingWorkingDeal = function() sends = sends + 1 end,
}
DiplomacyManager = {
	FindOpenSessionID = function() return nil end,
	HasQueuedSession = function() return false end,
	RequestSession = function() sends = sends + 1 end,
	CloseSession = function() sends = sends + 1 end,
}
NotificationManager = {
	GetList = function() local ids = {}; for id in pairs(notifications) do ids[#ids + 1] = id end; return ids end,
	Find = function(_, id) return { GetTypeName = function() return notifications[id] end } end,
	FindType = function(kind)
		for _, name in pairs(notifications) do
			if kind == 91 and name == "NOTIFICATION_DIPLOMACY_SESSION" then return {} end
		end
		return nil
	end,
}
local player = { GetDiplomacy = function()
	return { HasMet = function() return true end }
end }

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtimeErr = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtimeErr))
local trade = rawget(_G, "CivvisTrade")
assert(type(trade) == "table" and type(trade.seatBoard) == "function", "CivvisTrade.seatBoard")
-- What the load itself wrote (`deal_action_names`) is not this lane's.
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
local function board() return trade.seatBoard(player, localPlayer, turn) end

-- Who plays whom.
check("seat 1 is a CIVVIS seat", trade.seatKind(1), "civvis")
check("seat 0 is the person", trade.seatKind(0), "person")
check("a non-human major is a Firaxis AI", trade.seatKind(4), "ai")

-- Outside hotseat: nil, silent.
CivvisControlConfig.Hotseat = false
pending[0] = { [2] = true }
check("no board outside hotseat", board(), nil)
check("no ledger outside hotseat", #events, 0)
CivvisControlConfig.Hotseat = true

-- The person proposes: Dyes for 30 turns against 5 Gold a turn from us.
local personDeal = {
	{ kind = "resources", from = 0, value = 12, amount = 1, duration = 30 },
	{ kind = "gold", from = 2, amount = 5, duration = 30 },
}
working.incoming = { [2] = { [0] = personDeal } }
notifications = { [7] = "NOTIFICATION_DIPLOMACY_SESSION", [8] = "NOTIFICATION_CITY_GROWTH" }
local b = board()
check("hotseat board version", b.v, 1)
check("one incoming deal", #b.incoming, 1)
local inc = b.incoming[1]
check("from the person", inc.from, 0)
check("from_kind person", inc.from_kind, "person")
check("read from our incoming working deal", inc.source, "incoming")
check("we give one item", #inc.give, 1)
check("we give gold", inc.give[1].type, "GOLD")
check("5 gold", inc.give[1].amount, 5)
check("per turn for 30", inc.give[1].duration, 30)
check("we get a resource", inc.get[1].type, "RESOURCE")
check("the resource by type name", inc.get[1].name, "RESOURCE_DYES")
check("not a gift", inc.gift, false)
check("pending since this turn", inc.pending_turns, 0)
check("deal_incoming once", count("deal_incoming"), 1)
check("deal_probe once", count("deal_probe"), 1)
check("probe sees the session notification", last("deal_probe"):find('"diplomacy_session":true', 1, true) ~= nil, true)
check("probe lists diplomacy notifications only",
	last("deal_probe"):find('"diplomacy_notifications":["NOTIFICATION_DIPLOMACY_SESSION"]', 1, true) ~= nil, true)
check("probe sees their pending deal",
	last("deal_probe"):find('"pending_from_them":true', 1, true) ~= nil, true)
local firstId = inc.id
check("an id", type(firstId), "number")
check("the id fits 31 bits", firstId >= 0 and firstId < 2147483647, true)

-- A second frame of the same turn: same deal, no new records.
b = board()
check("same id on the next frame", b.incoming[1].id, firstId)
check("still one deal_incoming", count("deal_incoming"), 1)
check("still one probe this turn", count("deal_probe"), 1)

-- The next turn: still pending, one turn old, probed again.
turn = 21
b = board()
check("pending one turn", b.incoming[1].pending_turns, 1)
check("first seen turn kept", b.incoming[1].first_seen_turn, 20)
check("probe once per turn", count("deal_probe"), 2)

-- The same items in another order are the same deal; another amount is not.
working.incoming[2][0] = { personDeal[2], personDeal[1] }
check("order does not change the id", board().incoming[1].id, firstId)
working.incoming[2][0] = {
	{ kind = "resources", from = 0, value = 12, amount = 1, duration = 30 },
	{ kind = "gold", from = 2, amount = 6, duration = 30 },
}
b = board()
check("an amount changes the id", b.incoming[1].id ~= firstId, true)
check("a changed deal is a new deal_incoming", count("deal_incoming"), 2)
check("and pending from this turn", b.incoming[1].pending_turns, 0)

-- A working deal nobody proposed (no pending deal) is not on the table.
pending[0] = nil
check("no pending, no incoming", #board().incoming, 0)

-- Empty incoming working deal: read their outgoing one (and say so).
pending[3] = { [2] = true }
working.incoming[2][3] = {}
working.outgoing = { [3] = { [2] = {
	{ kind = "agreements", from = 3, sub = "open_borders", duration = 30 },
	{ kind = "greatwork", from = 2, value = 900, sub = 61 },
} } }
b = board()
check("one incoming from seat 3", #b.incoming, 1)
check("from a CIVVIS seat", b.incoming[1].from_kind, "civvis")
check("read from their outgoing deal", b.incoming[1].source, "their_outgoing")
check("an agreement by name", b.incoming[1].get[1].name, "OPEN_BORDERS")
check("a great work by instance", b.incoming[1].give[1].instance, 900)
check("a great work by type", b.incoming[1].give[1].name, "GREATWORK_POE_1")
pending[3] = nil

-- Our own proposal to seat 1, answered on seat 1's turn.
pending[2] = { [1] = true }
working.outgoing[2] = { [1] = { { kind = "gold", from = 2, amount = 40 },
	{ kind = "resources", from = 1, value = 44, amount = 10 } } }
turn = 22
b = board()
check("our pending deal listed", #b.outgoing, 1)
check("to seat 1", b.outgoing[1].to, 1)
check("proposed this turn", b.outgoing[1].proposed_turn, 22)
local ourId = b.outgoing[1].id
localPlayer = 1
trade.seatHeard(1, 2, DealProposalAction.ACCEPTED)
trade.seatHeard(3, 1, DealProposalAction.REJECTED) -- seat 3 answering seat 1: not ours
check("a note for another seat is not kept", trade.seat.heard[3], nil)
localPlayer = 2
pending[2][1] = nil
turn = 23
b = board()
check("no longer pending", #b.outgoing, 0)
check("one outcome", #b.outcomes, 1)
check("accepted, as heard", b.outcomes[1].outcome, "accepted")
check("outcome carries the id", b.outcomes[1].id, ourId)
check("deal_outcome recorded", count("deal_outcome"), 1)

-- Our proposal to the person, gone with no answer heard: closed, and the
-- person is never offered that deal again.
pending[2] = { [0] = true }
working.outgoing[2][0] = { { kind = "favor", from = 2, amount = 10 }, { kind = "gold", from = 0, amount = 30 } }
board()
local personId = trade.seat.mine[0].id
pending[2][0] = nil
turn = 24
b = board()
check("closed when nothing was heard", b.outcomes[#b.outcomes].outcome, "closed")
check("the person's refusal is remembered", #b.refused_by_person, 1)
check("by id", b.refused_by_person[1].id, personId)

-- A counter heard from seat 3.
pending[2] = { [3] = true }
working.outgoing[2][3] = { { kind = "gold", from = 2, amount = 5 } }
board()
trade.seatHeard(3, 2, DealProposalAction.ADJUSTED)
pending[2][3] = nil
turn = 25
check("countered", board().outcomes[3].outcome, "countered")

-- ★ The empty pending deal our own answer leaves behind (live trade-live-3
-- and -4: `pending_from_us: true` after every ACCEPTED or REJECTED, no item):
-- no outgoing deal on the board, and no outcome when it goes.
local outcomesBefore = #board().outcomes
pending[2] = { [4] = true }
working.outgoing[2][4] = {}
check("an empty pending deal we never sent is not listed", #board().outgoing, 0)
check("nor kept as ours", trade.seat.mine[4], nil)
pending[2][4] = nil
turn = 26
check("and leaves no outcome", #board().outcomes, outcomesBefore)
-- One we did send, its working deal since emptied by another arm, stays ours.
trade.seat.mine[4] = { id = 4444, first = 26, give = {}, get = {}, round = 1 }
pending[2] = { [4] = true }
check("our own, emptied, is still listed", #board().outgoing, 1)
check("by its id", board().outgoing[1].id, 4444)
pending[2][4] = nil

-- Heard outside hotseat: nothing kept.
CivvisControlConfig.Hotseat = false
trade.seatHeard(1, 2, DealProposalAction.ACCEPTED)
CivvisControlConfig.Hotseat = true
check("nothing heard outside hotseat", trade.seat.heard[1], nil)

-- Reading never sends, clears, copies, opens or closes anything.
check("no deal action of any kind", sends, 0)

-- The wiring, read from the shipped source: the board key on the state
-- record, and the ungated listener only in hotseat.
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local rivalsAt = assert(source:find("\t\trivals = rivals,\n", 1, true), "rivals on the state record")
local dealsAt = source:find("deals = try(function() return CivvisTrade.seatBoard(player, pid, turn); end, nil),", rivalsAt, true)
check("deals beside rivals on the state record", dealsAt ~= nil and dealsAt - rivalsAt < 300, true)
check("the listener is registered in hotseat only", source:find(
	"if cfg.Hotseat == true and CivvisSeat.seat ~= nil then\n\t\tpcall(function() Events.DiplomacyIncomingDeal.Add(CivvisTrade.seatHeard); end);",
	1, true) ~= nil, true)

print = realPrint
if failures > 0 then
	print(string.format("%d hotseat deal check(s) failed", failures))
	os.exit(1)
end
print("all hotseat deal checks passed")
