-- Offline regression for `LuxuryQuoteProposals` (opt-in, off unless set): a
-- luxury ask the rival answers REJECTED with its own price on our side of the
-- deal is proposed back once, the way a Great Work or Open Borders quote is,
-- and the rival's verdict is then accepted by the existing closer. 58 live
-- Lv 8 Deity games from 2026-11-19 02:01Z walked away from 140 such answers
-- inside our own ceiling. The asks are made by the real buy arm, inside a
-- real `MAKE_DEAL` session, answered through the real statement handler.
--
-- It also pins what must never happen: an unpriced or over-budget answer, a
-- second item on either side, a pending engine deal, EQUALIZE_FAILED, a
-- strategic or cash-bid ask, and a second proposal for the same ask. Native
-- 131031Z and 135842Z stalled after re-proposing an unpriced resource quote
-- and closing under its late EQUALIZE_FAILED.
--
-- Run: lua5.1 tools/civ6_control/mod/deal_luxury_quote_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end
setmetatable(_G, { __index = function(_, key)
	if key == "CivvisApplyOrder" or key == "CivvisOnIncomingDeal" or key == "CivvisTrade"
			or key == "CivvisOnDiplomacyStatement" or key == "CivvisOnDealSessionClosed" then
		return rawget(_G, key)
	end
	return stub()
end })

CivvisControlConfig = { TradeRetryTurns = 3, TradeResponseTurns = 2, PeaceRetryTurns = 5,
	DealSessions = true, DealSessionHoldSeconds = 4, DealSessionStandDown = 3 }

local events = {}
local realPrint = print
print = function(line)
	if type(line) == "string" and (line:find('"kind":"deal_', 1, true)
			or line:find('"kind":"counter_', 1, true)) then
		events[#events + 1] = line
	end
end

local holds = {}
LuaEvents = setmetatable({
	CivvisDealSession = function(subject, open, seconds)
		holds[#holds + 1] = { subject = subject, open = open, seconds = seconds }
	end,
}, { __index = function() return stub() end })

DealDirection = { OUTGOING = "outgoing", INCOMING = "incoming" }
DealItemTypes = { RESOURCES = "resources", FAVOR = "favor", GOLD = "gold",
	AGREEMENTS = "agreements", GREATWORK = "greatwork" }
DealAgreementTypes = { MAKE_PEACE = "make_peace", OPEN_BORDERS = "open_borders" }
DealProposalAction = {
	PENDING = "pending", PROPOSED = "proposed", ACCEPTED = "accepted", REJECTED = "rejected",
	ADJUSTED = "adjusted", EQUALIZE = "equalize", EQUALIZE_FAILED = "equalize_failed",
}
-- Their Jade (18) is the luxury we lack; our Truffles (31) the spare we swap.
local resourceRows = {
	RESOURCE_JADE = { ResourceType = "RESOURCE_JADE", Index = 18, ResourceClassType = "RESOURCECLASS_LUXURY" },
	RESOURCE_TRUFFLES = { ResourceType = "RESOURCE_TRUFFLES", Index = 31, ResourceClassType = "RESOURCECLASS_LUXURY" },
	RESOURCE_IRON = { ResourceType = "RESOURCE_IRON", Index = 43, ResourceClassType = "RESOURCECLASS_STRATEGIC" },
}
GameInfo = {
	Resources = setmetatable({}, { __index = function(_, key)
		if type(key) == "number" then
			for _, row in pairs(resourceRows) do if row.Index == key then return row end end
			return nil
		end
		return resourceRows[key]
	end }),
	Resource_Consumption = { RESOURCE_IRON = { Accumulate = true } },
	GreatWorks = {},
}
local TURN = 120
Game = {
	GetLocalPlayer = function() return 7 end,
	GetCurrentGameTurn = function() return TURN end,
}

local sessions = { requested = {}, closed = {} }
DiplomacyResponseTypes = { INITIAL = 0, ACKNOWLEDGE = 1 }
DiplomacyManager = {
	GetKeyName = function(key) return key end,
	FindOpenSessionID = function() return nil end,
	HasQueuedSession = function() return true end,
	RequestSession = function(pid, subject, kind)
		sessions.requested[#sessions.requested + 1] = { pid = pid, subject = subject, kind = kind }
	end,
	CloseSession = function(id) sessions.closed[#sessions.closed + 1] = id end,
}

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtimeErr = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtimeErr))

local applyOrder = rawget(_G, "CivvisApplyOrder")
local onStatement = rawget(_G, "CivvisOnDiplomacyStatement")
local trade = rawget(_G, "CivvisTrade")
assert(type(applyOrder) == "function" and type(onStatement) == "function"
	and type(trade) == "table", "CivvisControlAgent.lua did not export the session lane")

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		realPrint(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		realPrint(string.format("ok   %s = %s", name, tostring(got)))
	end
end
local function lastEvent(kind)
	for i = #events, 1, -1 do
		if events[i]:find('"kind":"' .. kind .. '"', 1, true) then return events[i] end
	end
	return nil
end
local function eventField(line, key)
	if line == nil then return nil end
	local quoted = line:match('"' .. key .. '":"([^"]*)"')
	if quoted ~= nil then return quoted end
	return line:match('"' .. key .. '":([^,}]*)')
end

-- The engine's part: one rival (3), our seat (7), `opts.incoming` the rival's
-- working deal when it answers.
local function fixture(opts)
	local state = { sends = {}, items = {}, pending = false,
		dealsEqual = opts.dealsEqual ~= false, gold = opts.gold or 400 }
	local nextId = 100
	local function newItem(kind)
		local item = { kind = kind, id = nextId }
		nextId = nextId + 1
		function item:SetValueType(v) self.valueType = v end
		function item:SetSubType(v) self.subType = v end
		function item:GetSubType() return self.subType end
		function item:SetDuration(v) self.duration = v end
		function item:SetAmount(v) self.amount = v end
		function item:GetAmount() return self.amount end
		function item:GetMaxAmount() return 999 end
		function item:IsValid() return true end
		function item:GetID() return self.id end
		return item
	end
	local outgoing = {}
	function outgoing:AddItemOfType(kind, owner)
		local item = newItem(kind); item.owner = owner
		state.items[#state.items + 1] = item
		return item
	end
	function outgoing:Validate() end
	function outgoing:IsValid() return true end
	function outgoing:RemoveItemByID() end
	local incoming = {}
	function incoming:Items()
		local list = opts.incoming or {}
		local i = 0
		return function()
			i = i + 1
			local spec = list[i]
			if spec == nil then return nil end
			return {
				GetType = function() return spec.kind end,
				GetFromPlayerID = function() return spec.from end,
				GetDuration = function() return spec.duration or 0 end,
				GetAmount = function() return spec.amount or 0 end,
				GetValueType = function() return spec.valueType end,
				GetSubType = function() return spec.subType end,
			}
		end
	end
	DealManager = {
		HasPendingDeal = function() return state.pending end,
		ClearWorkingDeal = function(direction) state.cleared = direction end,
		GetWorkingDeal = function(direction)
			if direction == DealDirection.INCOMING then return incoming end
			return outgoing
		end,
		GetPossibleDealItems = function(owner, other, kind)
			if kind ~= DealItemTypes.RESOURCES then return {} end
			if owner == 3 then
				return opts.theirTable or { { ForType = 18, MaxAmount = 2, IsValid = true } }
			end
			return { { ForType = 31, MaxAmount = 3, IsValid = true } }
		end,
		CopyIncomingToOutgoingWorkingDeal = function() state.copied = true end,
		AreWorkingDealsEqual = function() return state.dealsEqual end,
		SendWorkingDeal = function(action, pid, subject)
			state.sends[#state.sends + 1] = { action, pid, subject, holds = #holds }
			if opts.onSend ~= nil then opts.onSend(action) end
		end,
	}
	local owned = { [18] = 0, [31] = 3, [43] = 0 }
	local player = {
		GetDiplomacy = function()
			return {
				HasMet = function() return true end,
				IsAtWarWith = function() return false end,
				HasOpenBordersFrom = function() return false end,
			}
		end,
		GetTreasury = function() return { GetGoldBalance = function() return state.gold end } end,
		GetResources = function()
			return { GetResourceAmount = function(_, forType) return owned[forType] end }
		end,
	}
	Players = setmetatable({ [7] = player }, {
		__index = function() return { IsMajor = function() return true end } end,
	})
	return state, player
end

local function reset()
	trade.pending, trade.asked, trade.sessions = {}, {}, {}
	trade.unanswered, trade.disabled, trade.turnHold = 0, false, nil
	sessions.requested, sessions.closed = {}, {}
	events, holds = {}, {}
end
local function open()
	onStatement(7, 3, { StatementType = "MAKE_DEAL", SessionID = 901 })
end
local function answer(action)
	onStatement(3, 7, { StatementType = "MAKE_DEAL", SessionID = 901, DealAction = action })
end

-- The live shape: their copy against our spare, and their price on our side
-- (`ours` "RESOURCES:22=1x30,GOLD=4x30" in the 2026-11-19 games). 4 GPT is
-- 100 Gold-equivalent against a 205 ceiling.
local function quote(extra, price)
	local items = {
		{ kind = DealItemTypes.RESOURCES, from = 3, valueType = 18, amount = 1, duration = 30 },
		{ kind = DealItemTypes.RESOURCES, from = 7, valueType = 31, amount = 1, duration = 30 },
	}
	if price ~= 0 then
		items[#items + 1] = { kind = DealItemTypes.GOLD, from = 7, amount = price or 4, duration = 30 }
	end
	for _, item in ipairs(extra or {}) do items[#items + 1] = item end
	return items
end

-- A luxury asked by the real buy arm, its session opened and the question put.
local function luxurySession(opts)
	reset()
	opts = opts or {}
	if opts.incoming == nil then opts.incoming = quote() end
	local state, player = fixture(opts)
	state.ordered = applyOrder(player, 7, { kind = "buy", subject = "3",
		verb = opts.verb or "LUXURY_ANY;RESOURCE_TRUFFLES=1", x = opts.ceiling or 205, y = 0 }, TURN)
	open()
	return state
end

local function onFlag(on) CivvisControlConfig.LuxuryQuoteProposals = on end

-- ─── off (the default): today's behaviour, walked away from ────────────────
onFlag(nil)
local off = luxurySession()
check("the real buy arm asks the luxury", off.ordered, true)
check("the ask is a luxury want", trade.pending[3] and trade.pending[3].want, "RESOURCES:18")
check("the ask is no strategic block", trade.pending[3] and trade.pending[3].strategic, nil)
check("the ask barters our spare", trade.pending[3] and trade.pending[3].gave
	and trade.pending[3].gave["RESOURCES:31"], 1)
check("the question is EQUALIZE", off.sends[1] and off.sends[1][1], "equalize")
answer(DealProposalAction.REJECTED)
check("off: a priced luxury refusal is not proposed", #off.sends, 1)
check("off: nothing is re-proposed", lastEvent("deal_reproposed"), nil)
check("off: it is declined", eventField(lastEvent("deal_declined"), "want"), "RESOURCES:18")
check("off: the ask settles", trade.pending[3], nil)
check("off: the session closes once", #sessions.closed, 1)
check("off: the response reads it as in budget", eventField(lastEvent("deal_response"), "within_budget"), "true")
onFlag(false)
local offFalse = luxurySession()
answer(DealProposalAction.REJECTED)
check("an explicit false is off too", #offFalse.sends, 1)

-- ─── on: proposed once, then the rival's verdict is accepted ───────────────
onFlag(true)
local lux = luxurySession()
answer(DealProposalAction.REJECTED)
check("on: a priced in-budget luxury refusal is PROPOSED", lux.sends[2] and lux.sends[2][1], "proposed")
check("on: the rival's quote is copied, never rebuilt", lux.copied, true)
check("on: no acceptance before the rival's verdict", #lux.sends, 2)
check("on: the proposal ledger names the luxury", eventField(lastEvent("deal_reproposed"), "want"), "RESOURCES:18")
check("on: the proposal ledger carries the price", eventField(lastEvent("deal_reproposed"), "pay"), "100")
check("on: no decline is filed", lastEvent("deal_declined"), nil)
check("on: the ask stays pending", trade.pending[3] ~= nil, true)
check("on: the ask is marked proposed", trade.pending[3] and trade.pending[3].purchase_quote_proposed, true)
check("on: the session stays open", #sessions.closed, 0)
check("on: the answer hold is renewed", holds[#holds] and holds[#holds].seconds, 30)
check("on: the hold precedes the proposal", lux.sends[2] and lux.sends[2].holds, #holds)
lux.pending = true
answer(DealProposalAction.REJECTED)
check("on: the old refusal while the engine holds our proposal sends nothing", #lux.sends, 2)
check("on: …and waits", eventField(lastEvent("deal_proposal_wait"), "reason"), "engine_pending")
check("on: …with the session open", #sessions.closed, 0)
lux.pending = false
answer(DealProposalAction.ADJUSTED)
check("on: the rival's ADJUSTED is accepted", lux.sends[3] and lux.sends[3][1], "accepted")
check("on: the closed luxury is on the ledger", eventField(lastEvent("deal_closed"), "want"), "RESOURCES:18")
check("on: the closed luxury paid the quoted price", eventField(lastEvent("deal_closed"), "pay"), "100")
check("on: acceptance settles the ask", trade.pending[3], nil)
check("on: acceptance closes the session once", #sessions.closed, 1)
answer(DealProposalAction.ADJUSTED)
check("on: a late echo after the close sends nothing", #lux.sends, 3)

local sync
sync = luxurySession({ onSend = function(action)
	if action == DealProposalAction.PROPOSED then answer(DealProposalAction.ACCEPTED) end
end })
answer(DealProposalAction.REJECTED)
check("on: a verdict inside the send is accepted once", sync.sends[3] and sync.sends[3][1], "accepted")
check("on: …and closes once", #sessions.closed, 1)
check("on: …leaving nothing pending", trade.pending[3], nil)

-- A Gold-only ask (no spare to swap): the luxury against Gold alone.
local goldOnly = luxurySession({ verb = "LUXURY_ANY", incoming = {
	{ kind = DealItemTypes.RESOURCES, from = 3, valueType = 18, amount = 1, duration = 30 },
	{ kind = DealItemTypes.GOLD, from = 7, amount = 3, duration = 30 },
} })
check("a Gold-only luxury ask barters nothing", trade.pending[3] and trade.pending[3].gave, nil)
answer(DealProposalAction.REJECTED)
check("on: a priced Gold-only luxury refusal is PROPOSED", goldOnly.sends[2] and goldOnly.sends[2][1], "proposed")

-- ─── never a second proposal ───────────────────────────────────────────────
local twice = luxurySession()
answer(DealProposalAction.REJECTED)
answer(DealProposalAction.REJECTED)
check("a second refusal sends no second proposal", #twice.sends, 2)
check("a second refusal is declined", eventField(lastEvent("deal_declined"), "action"), "rejected")
check("a second refusal settles the ask", trade.pending[3], nil)
check("a second refusal closes the session once", #sessions.closed, 1)
answer(DealProposalAction.REJECTED)
answer(DealProposalAction.ADJUSTED)
check("nothing after the close revives the ask", #twice.sends, 2)

-- ─── EQUALIZE_FAILED ───────────────────────────────────────────────────────
local failed = luxurySession()
answer(DealProposalAction.EQUALIZE_FAILED)
check("EQUALIZE_FAILED is never proposed", #failed.sends, 1)
check("EQUALIZE_FAILED is declined", eventField(lastEvent("deal_declined"), "action"), "equalize_failed")
check("EQUALIZE_FAILED closes the session once", #sessions.closed, 1)

-- The 135842Z order: our proposal out, then the original EQUALIZE's late
-- failure. While the engine holds our proposal, the session is not closed
-- under it; the rival's verdict still closes the deal.
local late = luxurySession()
answer(DealProposalAction.REJECTED)
late.pending = true
answer(DealProposalAction.EQUALIZE_FAILED)
check("a late EQUALIZE_FAILED sends nothing", #late.sends, 2)
check("a late EQUALIZE_FAILED over our pending proposal waits",
	eventField(lastEvent("deal_proposal_wait"), "reason"), "late_equalize_failed")
check("…keeping the session open", #sessions.closed, 0)
check("…and the ask", trade.pending[3] ~= nil, true)
late.pending = false
answer(DealProposalAction.ADJUSTED)
check("the verdict after a late failure is accepted", late.sends[3] and late.sends[3][1], "accepted")
check("…and closes once", #sessions.closed, 1)
local lateIdle = luxurySession()
answer(DealProposalAction.REJECTED)
answer(DealProposalAction.EQUALIZE_FAILED)
check("a late EQUALIZE_FAILED with no engine deal sends nothing", #lateIdle.sends, 2)
check("…and is declined", eventField(lastEvent("deal_declined"), "action"), "equalize_failed")
check("…closing the session once", #sessions.closed, 1)

-- ─── the guards: each walked away from, as before ──────────────────────────
for _, case in ipairs({
	{ "over the ceiling", { incoming = quote(nil, 9) } },
	{ "unpriced (the 131031Z barter shape)", { incoming = quote(nil, 0) } },
	{ "their Gold beside our price", { incoming = quote({
		{ kind = DealItemTypes.GOLD, from = 3, amount = 2, duration = 30 } }) } },
	{ "their Gold on a Gold-only ask (foreign)", { verb = "LUXURY_ANY", incoming = {
		{ kind = DealItemTypes.RESOURCES, from = 3, valueType = 18, amount = 1, duration = 30 },
		{ kind = DealItemTypes.GOLD, from = 3, amount = 2, duration = 30 },
		{ kind = DealItemTypes.GOLD, from = 7, amount = 3, duration = 30 },
	} } },
	{ "a second item on their side", { incoming = quote({
		{ kind = DealItemTypes.RESOURCES, from = 3, valueType = 43, amount = 1 } }) } },
	{ "a second copy of their luxury", { incoming = {
		{ kind = DealItemTypes.RESOURCES, from = 3, valueType = 18, amount = 2, duration = 30 },
		{ kind = DealItemTypes.RESOURCES, from = 7, valueType = 31, amount = 1, duration = 30 },
		{ kind = DealItemTypes.GOLD, from = 7, amount = 4, duration = 30 },
	} } },
	{ "an item of ours we did not offer", { incoming = quote({
		{ kind = DealItemTypes.RESOURCES, from = 7, valueType = 43, amount = 1 } }) } },
	{ "more of our spare than offered", { incoming = {
		{ kind = DealItemTypes.RESOURCES, from = 3, valueType = 18, amount = 1, duration = 30 },
		{ kind = DealItemTypes.RESOURCES, from = 7, valueType = 31, amount = 2, duration = 30 },
		{ kind = DealItemTypes.GOLD, from = 7, amount = 4, duration = 30 },
	} } },
}) do
	local guarded = luxurySession(case[2])
	answer(DealProposalAction.REJECTED)
	check(case[1] .. ": not proposed", #guarded.sends, 1)
	check(case[1] .. ": nothing re-proposed", lastEvent("deal_reproposed"), nil)
	check(case[1] .. ": declined", eventField(lastEvent("deal_declined"), "want"), "RESOURCES:18")
	check(case[1] .. ": closes once", #sessions.closed, 1)
end

local busy = luxurySession()
busy.pending = true
answer(DealProposalAction.REJECTED)
check("an engine already holding a deal: not proposed", #busy.sends, 1)
check("…and declined", eventField(lastEvent("deal_declined"), "want"), "RESOURCES:18")
local unread = luxurySession()
DealManager.HasPendingDeal = function() return nil end
answer(DealProposalAction.REJECTED)
check("unreadable pending state: not proposed", #unread.sends, 1)
local unequal = luxurySession({ dealsEqual = false })
answer(DealProposalAction.REJECTED)
check("unequal copied working deals: not proposed", #unequal.sends, 1)
check("…and declined", eventField(lastEvent("deal_declined"), "want"), "RESOURCES:18")
check("…closing once", #sessions.closed, 1)

-- ─── strategic asks keep their own protocol ────────────────────────────────
local ironQuote = {
	{ kind = DealItemTypes.RESOURCES, from = 3, valueType = 43, amount = 2 },
	{ kind = DealItemTypes.GOLD, from = 7, amount = 20 },
}
local iron = luxurySession({ verb = "RESOURCE_IRON=2", ceiling = 34, incoming = ironQuote,
	theirTable = { { ForType = 43, MaxAmount = 10, IsValid = true } } })
check("a strategic block is asked", iron.ordered, true)
check("…as a strategic want", trade.pending[3] and trade.pending[3].strategic, true)
answer(DealProposalAction.REJECTED)
check("a priced strategic refusal is not proposed", #iron.sends, 1)
check("…and declined", eventField(lastEvent("deal_declined"), "want"), "RESOURCES:43")

CivvisControlConfig.StrategicCashCeilingBid = true
CivvisControlConfig.VictoryLane = "domination"
local cash = luxurySession({ verb = "RESOURCE_IRON=2", ceiling = 34, incoming = ironQuote,
	theirTable = { { ForType = 43, MaxAmount = 10, IsValid = true } } })
check("the strategic cash bid is asked", cash.ordered, true)
check("…as its one PROPOSED question", cash.sends[1] and cash.sends[1][1], "proposed")
check("…marked as a cash bid", trade.pending[3] and trade.pending[3].cash_bid_requested, true)
answer(DealProposalAction.REJECTED)
check("a refused cash bid is never proposed again", #cash.sends, 1)
check("…nothing re-proposed", lastEvent("deal_reproposed"), nil)
check("…it keeps its counter wait", eventField(lastEvent("deal_session"), "phase"), "counter_wait")
answer(DealProposalAction.EQUALIZE_FAILED)
check("…and a late failure sends nothing", #cash.sends, 1)
CivvisControlConfig.StrategicCashCeilingBid = nil
CivvisControlConfig.VictoryLane = nil

-- ─── Great Work and Open Borders quotes are unchanged by the switch ───────
local work = luxurySession({ incoming = {
	{ kind = DealItemTypes.AGREEMENTS, from = 3, subType = DealAgreementTypes.OPEN_BORDERS },
	{ kind = DealItemTypes.GOLD, from = 7, amount = 14 },
} })
trade.pending[3].want, trade.pending[3].gave, trade.pending[3].ceiling = "OPEN_BORDERS", nil, 28
answer(DealProposalAction.REJECTED)
check("an Open Borders quote is still proposed", work.sends[2] and work.sends[2][1], "proposed")
work.pending = true
answer(DealProposalAction.EQUALIZE_FAILED)
check("its late EQUALIZE_FAILED keeps the old lifecycle (declined)",
	eventField(lastEvent("deal_declined"), "action"), "equalize_failed")
check("…and closes as before", #sessions.closed, 1)

onFlag(nil)
if failures > 0 then
	realPrint(string.format("\n%d luxury-quote check(s) FAILED", failures))
	os.exit(1)
end
realPrint("\nall luxury-quote checks passed")
