-- Hotseat deals v1, the acting half: the `deal` order (`CivvisTrade.seatOrder`
-- through the real `applyOrder`) and its send inside the MAKE_DEAL session
-- (`CivvisTrade.seatSend`, reached through the real
-- `CivvisOnDiplomacyStatement`). The contract is
-- ~/civvis-civ6-runs/hotseat/findings/trade-deals-design.md.
--
-- Pinned here: each verb sends exactly the shipped deal screen's action and
-- nothing else, from this seat only, inside a session it opened and then
-- closed; a deal that changed since the board was read is not accepted; our
-- terms are built item by item as the screen builds them, and a refused item
-- leaves no half-built deal; the person is never sent terms they refused; a
-- sale or purchase to a human seat in hotseat is refused, not left to expire;
-- and nothing of it exists outside hotseat.
--
-- Run: lua5.1 tools/civ6_control/mod/hotseat_deal_orders_test.lua

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

-- Seat 2 is this context's; 1, 2 and 3 are CIVVIS seats, 0 the person, 4 a
-- Firaxis AI, 5 a seat we have not met.
CivvisControlConfig = { Hotseat = true, HotseatSeat = 2, HotseatSeats = { 1, 2, 3 },
	HotseatHumanSeats = { 0 }, TradeRetryTurns = 3, TradeResponseTurns = 2 }

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
	GetLocalPlayer = function() return 2 end,
	GetCurrentGameTurn = function() return turn end,
}
local humans = { [0] = true, [1] = true, [2] = true, [3] = true, [4] = false, [5] = true }
Players = setmetatable({}, { __index = function(_, id)
	return { IsHuman = function() return humans[id] end, IsMajor = function() return true end }
end })
PlayerManager = { GetAliveMajorIDs = function() return { 0, 1, 2, 3, 4, 5 } end }
local diplomacy = {
	HasMet = function(_, other) return other ~= 5 end,
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
local function order(kind, subject, verb, x)
	local ok, why = applyOrder(player, 2, { kind = kind, subject = subject, verb = verb, x = x }, turn)
	return why, ok
end
local function reset()
	for i = #log, 1, -1 do log[i] = nil end
end
local function logged(prefix)
	local n = 0
	for _, entry in ipairs(log) do if entry:sub(1, #prefix) == prefix then n = n + 1 end end
	return n
end
local function last(kind)
	for i = #events, 1, -1 do
		if events[i]:find('"kind":"' .. kind .. '"', 1, true) then return events[i] end
	end
end
local function field(line, key)
	if line == nil then return nil end
	local quoted = line:match('"' .. key .. '":"([^"]*)"')
	if quoted ~= nil then return quoted end
	return line:match('"' .. key .. '":([^,}]*)')
end
local function idOf(dir, a, b, pid)
	local _, _, id = trade.seatSides(DealManager.GetWorkingDeal(dir, a, b), pid)
	return id
end

check("deal orders go out after the frame's unit orders", trade.SESSION_ORDER_KINDS.deal, true)

-- Refusals that touch nothing.
reset()
CivvisControlConfig.Hotseat = false
check("not outside hotseat", order("deal", 0, "ACCEPT"), "deal_not_hotseat")
CivvisControlConfig.Hotseat = true
check("a verb the contract lacks", order("deal", 0, "HAGGLE"), "deal_verb_unknown")
check("an item the grammar lacks", order("deal", 1, "PROPOSE;give=GOLD=x"), "deal_item_unknown:GOLD=x")
check("a Firaxis AI is the sell/buy lane's", order("deal", 4, "PROPOSE;give=GOLD=5"), "deal_not_human")
check("an unmet seat", order("deal", 5, "PROPOSE;give=GOLD=5"), "deal_not_met")
check("not ourselves", order("deal", 2, "PROPOSE;give=GOLD=5"), "deal_target_unmapped")
check("nothing incoming to accept", order("deal", 0, "ACCEPT"), "deal_none_incoming")
check("nothing pending to withdraw", order("deal", 1, "WITHDRAW"), "deal_not_pending")
check("no session for a refusal", logged("session:"), 0)
check("a refusal is written down for the verdict", last("deal_refused") ~= nil, true)
check("with the seat", field(last("deal_refused"), "to"), "1")
check("the verb", field(last("deal_refused"), "verb"), "WITHDRAW")
check("and the mod's reason", field(last("deal_refused"), "why"), "deal_not_pending")
check("no send for a refusal", logged("send:"), 0)

-- The person's offer: Dyes for 30 turns against 5 Gold a turn.
pending[0] = { [2] = true }
working.incoming = { [2] = { [0] = {
	{ kind = "resources", from = 0, value = 12, amount = 1, duration = 30 },
	{ kind = "gold", from = 2, amount = 5, duration = 30 },
} } }
local offer = idOf("incoming", 2, 0, 2)
check("a changed deal is not accepted", order("deal", 0, "ACCEPT", offer + 1), "deal_changed")
check("and opens no session", logged("session:"), 0)
local why, ok = order("deal", 0, "ACCEPT", offer)
check("accepted", why, "deal_accepted")
check("accept order ok", ok, true)
check("one MAKE_DEAL session to the person", logged("session:2>0:MAKE_DEAL"), 1)
check("their deal copied to ours", logged("copy:2>0"), 1)
check("ACCEPTED sent once, by this seat", logged("send:ACCEPTED:2>0"), 1)
check("nothing else sent", logged("send:"), 1)
check("the session closed", logged("close:55"), 1)
check("deal_answer sent", field(last("deal_answer"), "sent"), "true")
check("deal_answer verb", field(last("deal_answer"), "verb"), "ACCEPT")
check("their deal no longer pending after", field(last("deal_answer"), "pending_from_them"), "false")
check("the session is released", trade.sessions[0], nil)

-- Accept, but the copy does not match: nothing is sent.
reset()
pending[0] = { [2] = true }
opts.unequal = true
check("accept asked", order("deal", 0, "ACCEPT", offer), "deal_accepted")
opts.unequal = false
check("unequal deals: no ACCEPTED", logged("send:"), 0)
check("says why", field(last("deal_answer"), "why"), "deals_not_equal")
check("and still closes", logged("close:55"), 1)

-- Reject.
reset()
check("rejected", order("deal", 0, "REJECT"), "deal_rejected")
check("REJECTED sent", logged("send:REJECTED:2>0"), 1)

-- A session of ours already open with that seat.
pending[0] = { [2] = true }
trade.sessions[0] = { kind = "sell" }
check("one session at a time", order("deal", 0, "REJECT"), "deal_session_busy")
trade.sessions[0] = nil

-- Propose to seat 1: 50 Gold (clipped to what we hold) for 10 Iron.
reset()
possible[1] = { resources = { { ForType = 44, IsValid = true } } }
opts.max = { gold = 30 }
check("proposed", order("deal", 1, "PROPOSE;give=GOLD=50;get=RESOURCE_IRON=10"), "deal_proposed")
opts.max = nil
local built = items("outgoing", 2, 1)
check("two items built", #built, 2)
check("gold from us", built[1].from, 2)
check("gold clipped to the maximum", built[1].amount, 30)
check("a lump", built[1].duration, 0)
check("iron from them", built[2].from, 1)
check("iron by index", built[2].value, 44)
check("ten of it", built[2].amount, 10)
check("PROPOSED sent", logged("send:PROPOSED:2>1"), 1)
check("our proposal is tracked", trade.seat.mine[1] ~= nil, true)
check("by the id of what was sent", trade.seat.mine[1].id, idOf("outgoing", 2, 1, 2))
check("a second proposal while one is pending", order("deal", 1, "PROPOSE;give=GOLD=5"), "deal_pending_exists")
pending[2][1] = nil
turn = 31
check("and too soon after the last", order("deal", 1, "PROPOSE;give=GOLD=5"), "deal_cooldown")

-- An item the other side cannot give leaves nothing behind.
reset()
turn = 40
check("an item not on their table", order("deal", 1, "PROPOSE;give=GOLD=5;get=RESOURCE_SILK=1@30"),
	"deal_item_refused:RESOURCE_SILK")
check("the half-built deal is cleared", #items("outgoing", 2, 1), 0)
check("no session", logged("session:"), 0)
opts.dealValid = false
check("an invalid deal", order("deal", 1, "PROPOSE;give=GOLD=5"), "deal_invalid")
opts.dealValid = nil
check("is not sent", logged("send:"), 0)

-- Great Work, Open Borders and a luxury, built as the screen builds them.
reset()
possible[2] = {
	greatwork = { { ForType = 900, ForTypeDescriptionID = 61, IsValid = true } },
	resources = { { ForType = 12, IsValid = true } },
}
possible[3] = { agreements = { { SubType = "open_borders", Duration = 30, IsValid = true } } }
check("a mixed proposal", order("deal", 3, "PROPOSE;give=GREATWORK#900,RESOURCE_DYES=1@30;get=OPEN_BORDERS@30"),
	"deal_proposed")
built = items("outgoing", 2, 3)
check("the work's description", built[1].sub, 61)
check("the work's instance", built[1].value, 900)
check("a luxury for 30 turns", built[2].duration, 30)
check("open borders from them", built[3].from, 3)
check("by subtype", built[3].sub, "open_borders")
check("for the entry's duration", built[3].duration, 30)

-- Counter: our terms in place of theirs, ADJUSTED.
reset()
pending[1] = { [2] = true }
working.incoming[2][1] = { { kind = "gold", from = 1, amount = 10 } }
trade.seat.seen[1] = { id = idOf("incoming", 2, 1, 2), first = 39, round = 1 }
check("countered", order("deal", 1, "COUNTER;get=GOLD=20", idOf("incoming", 2, 1, 2)), "deal_countered")
check("ADJUSTED sent", logged("send:ADJUSTED:2>1"), 1)
check("our counter carries round 2", trade.seat.mine[1].round, 2)

-- Withdraw.
reset()
check("withdrawn", order("deal", 3, "WITHDRAW"), "deal_withdrawn")
check("CLOSED sent", logged("send:CLOSED:2>3"), 1)

-- The person is never sent terms they refused.
reset()
turn = 60
pending[0] = nil
possible[0] = { resources = {} }
check("an offer to the person", order("deal", 0, "PROPOSE;give=GOLD=40"), "deal_proposed")
local toPerson = trade.seat.mine[0].id
pending[2][0] = nil
turn = 61
trade.seatBoard(player, 2, turn)
check("their silence is a refusal", trade.seat.refused[toPerson] ~= nil, true)
check("the board keeps the refused terms",
	trade.seatBoard(player, 2, turn).refused_by_person[1].give[1].amount, 40)
turn = 80
check("the same terms are not sent again", order("deal", 0, "PROPOSE;give=GOLD=40"), "deal_refused_before")
check("other terms may be", order("deal", 0, "PROPOSE;give=GOLD=41"), "deal_proposed")

-- The board: a counter to our proposal is named as one; items carry class
-- and object.
pending = { [2] = { [1] = true } }
working.outgoing[2][1] = { { kind = "gold", from = 2, amount = 7 } }
trade.seat.mine[1] = nil
turn = 81
trade.seatBoard(player, 2, turn)
local ours = trade.seat.mine[1].id
pending = { [1] = { [2] = true } }
working.incoming[2][1] = {
	{ kind = "resources", from = 1, value = 12, amount = 1, duration = 30 },
	{ kind = "greatwork", from = 2, value = 900, sub = 61 },
}
turn = 82
local b = trade.seatBoard(player, 2, turn)
check("our proposal resolved as countered", b.outcomes[#b.outcomes].outcome, "countered")
check("the outcome carries what we offered", b.outcomes[#b.outcomes].give[1].amount, 7)
check("the incoming deal names the proposal it counters", b.incoming[1].counter_of, ours)
check("round 2", b.incoming[1].round, 2)
check("a luxury says so", b.incoming[1].get[1].class, "luxury")
check("a strategic says so", trade.seatItem({ GetType = function() return "resources" end,
	GetValueType = function() return 44 end, GetAmount = function() return 10 end,
	GetDuration = function() return 0 end, GetSubType = function() return nil end }).class, "strategic")
check("a Great Work says its object", b.incoming[1].give[1].object, "GREATWORKOBJECT_WRITING")

-- A statement in a session already sent sends nothing more.
reset()
trade.sessions[1] = { kind = "hotseat", sent = true, verb = "REJECT", action = "REJECTED" }
CivvisOnDiplomacyStatement(1, 2, { SessionID = 55, StatementType = "MAKE_DEAL" })
check("no second send", logged("send:"), 0)
trade.sessions[1] = nil

-- In hotseat a sale or purchase to a human seat is refused at once.
check("no sale to a human seat", order("sell", 1, "RESOURCE_DYES=1", 10), "deal_human_seat")
check("no purchase from one", order("buy", 0, "OPEN_BORDERS", 28), "deal_human_seat")
CivvisControlConfig.Hotseat = false
check("outside hotseat the sale lane is unchanged", order("sell", 1, "RESOURCE_DYES=1", 10) ~= "deal_human_seat", true)
CivvisControlConfig.Hotseat = true

print = realPrint
if failures > 0 then
	print(string.format("%d hotseat deal order check(s) failed", failures))
	os.exit(1)
end
print("all hotseat deal order checks passed")
