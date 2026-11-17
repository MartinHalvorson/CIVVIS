-- Offline regression: the Domination seat's strategic cash bid (a PROPOSED
-- buy, `StrategicCashCeilingBid`) goes through both opt-in deal fixes, the
-- same way a peace or alliance proposal does (`CivvisTrade.proposes`):
--   * `DealAskOnTick`: the bid is not put from the statement that opened the
--     session but on the next tick (`ask_deferred` -> `asked`); a session the
--     end turn overtook is closed unasked, and a bid closed before its
--     question leaves no pending offer and no retry wait behind; a bid whose
--     Gold was spent meanwhile is not asked;
--   * `DealCounterRefuse`: a REJECTED bid whose counter hold runs out with no
--     refusal dialogue and the deal screen up is declined with REJECTED
--     (`refusing`, then `closed why=rejected_working_deal`), the echo of our
--     refusal is not its counter, and a later counter is refused as before.
-- Live civvis-20261116T234151Z t118 and -cont1 t152 (both switches on) hung
-- on exactly this bid: asked inline, REJECTED, `counter_wait hold=4`, deal
-- view up, bare close `counter_not_sent`. The default, the dialogue path,
-- an EQUALIZE buy and a sale are pinned unchanged. Fixture copied from
-- deal_session_test.lua, listener capture from deal_counter_refuse_test.lua.
--
-- Run: lua5.1 tools/civ6_control/mod/deal_cash_bid_fixes_test.lua

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
		return rawget(_G, key);
	end
	return stub()
end })

CivvisControlConfig = { TradeRetryTurns = 3, TradeResponseTurns = 2, PeaceRetryTurns = 5,
	DealSessions = true, DealSessionHoldSeconds = 4, DealSessionStandDown = 3,
	StrategicCashCeilingBid = true, VictoryLane = "domination" }
local cfg = CivvisControlConfig

local events = {}
local realPrint = print
print = function(line)
	if type(line) == "string" and (line:find('"kind":"deal_', 1, true)
			or line:find('"kind":"peace_', 1, true)
			or line:find('"kind":"counter_', 1, true)) then
		events[#events + 1] = line
	end
end

-- LuaEvents: record the holds the agent sets, and the listeners it adds.
local holds, listeners = {}, {}
LuaEvents = setmetatable({}, { __index = function(t, name)
	local event = setmetatable({
		Add = function(fn) listeners[name] = fn end,
		Remove = function() end,
	}, { __call = function(_, a, b, c)
		if name == "CivvisDealSession" then
			holds[#holds + 1] = { subject = a, open = b, seconds = c }
		end
	end })
	rawset(t, name, event)
	return event
end })

DealDirection = { OUTGOING = "outgoing", INCOMING = "incoming" }
DealItemTypes = { RESOURCES = "resources", FAVOR = "favor", GOLD = "gold", AGREEMENTS = "agreements", GREATWORK = "greatwork" }
DealAgreementTypes = { MAKE_PEACE = "make_peace", OPEN_BORDERS = "open_borders" }
DealProposalAction = {
	PENDING = "pending", PROPOSED = "proposed", ACCEPTED = "accepted", REJECTED = "rejected",
	ADJUSTED = "adjusted", EQUALIZE = "equalize", EQUALIZE_FAILED = "equalize_failed",
}
local resourceRows = {
	RESOURCE_IRON = { ResourceType = "RESOURCE_IRON", Index = 43,
		ResourceClassType = "RESOURCECLASS_STRATEGIC" },
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
local TURN = 118
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
	IsSessionIDOpen = function() return true end,
}

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtimeErr = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtimeErr))

local applyOrder = rawget(_G, "CivvisApplyOrder")
local onStatement = rawget(_G, "CivvisOnDiplomacyStatement")
local onClosed = rawget(_G, "CivvisOnDealSessionClosed")
local trade = rawget(_G, "CivvisTrade")
assert(type(applyOrder) == "function" and type(onStatement) == "function"
	and type(onClosed) == "function" and type(trade) == "table",
	"CivvisControlAgent.lua did not export the session lane")

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
local function count(phase)
	local n = 0
	for _, line in ipairs(events) do
		if line:find('"phase":"' .. phase .. '"', 1, true) then n = n + 1 end
	end
	return n
end

-- One rival's deal manager. `opts.incoming` is its answer; `state.gold` our
-- treasury.
local function fixture(opts)
	opts = opts or {}
	local state = { sends = {}, items = {}, gold = opts.gold or 129 }
	local nextId = 100
	local function newItem(kind)
		local item = { kind = kind, id = nextId }
		nextId = nextId + 1
		function item:SetValueType(v) self.valueType = v end
		function item:SetSubType(v) self.subType = v end
		function item:GetSubType() return self.subType end
		function item:SetDuration(v) self.duration = v end
		function item:SetAmount(v) self.amount = v end
		function item:SetLocked(v) self.locked = v end
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
		HasPendingDeal = function() return false end,
		ClearWorkingDeal = function(direction) state.cleared = direction end,
		GetWorkingDeal = function(direction)
			if direction == DealDirection.INCOMING then return incoming end
			return outgoing
		end,
		GetPossibleDealItems = function(pid, subject, kind)
			if kind == DealItemTypes.RESOURCES then
				return opts.possibleResources or { { ForType = 43, MaxAmount = 10, IsValid = true } }
			end
			return {}
		end,
		CopyIncomingToOutgoingWorkingDeal = function() state.copied = true end,
		AreWorkingDealsEqual = function() return true end,
		SendWorkingDeal = function(action, pid, subject)
			state.sends[#state.sends + 1] = { action = action, subject = subject, holds = #holds }
			if opts.onSend ~= nil then opts.onSend(action) end
		end,
	}
	local player = {
		GetDiplomacy = function()
			return {
				HasMet = function() return true end,
				IsAtWarWith = function() return false end,
				HasOpenBordersFrom = function() return false end,
			}
		end,
		GetTreasury = function() return { GetGoldBalance = function() return state.gold end } end,
	}
	Players = setmetatable({ [7] = player }, {
		__index = function() return { IsMajor = function() return true end } end,
	})
	state.player = player
	return state
end

-- The rival's side of the answer: the Iron asked for, our Gold as the price.
local function quote(subject)
	return { { kind = DealItemTypes.RESOURCES, from = subject, valueType = 43, amount = 2 },
		{ kind = DealItemTypes.GOLD, from = 7, amount = 34 } }
end
-- A strategic cash bid to `subject` through the real buy arm. (The buy
-- cooldown is `TradeRetryTurns`, so each case moves the turn on by three.)
local function bid(subject, opts)
	opts = opts or {}
	opts.incoming = opts.incoming or quote(subject)
	local state = fixture(opts)
	state.ordered = applyOrder(state.player, 7,
		{ kind = "buy", subject = tostring(subject), verb = "RESOURCE_IRON=2", x = 34 }, TURN)
	return state
end
local function open(subject, id)
	onStatement(7, subject, { StatementType = "MAKE_DEAL", SessionID = id })
end
local function ack(subject, id)
	onStatement(subject, 7, { StatementType = "MAKE_DEAL", SessionID = id, StatementSubType = "NONE",
		ResponseType = DiplomacyResponseTypes.ACKNOWLEDGE })
end
local function verdict(subject, id, action)
	onStatement(subject, 7, { StatementType = "MAKE_DEAL", SessionID = id,
		StatementSubType = "AI_REFUSE_DEAL", DealAction = action or DealProposalAction.REJECTED })
end
local function expired(screen, subject, dealUp)
	return listeners.CivvisDealHoldExpired(screen, subject, dealUp)
end

local proposes = trade.proposes
check("the asks a counter follows are named", type(proposes), "function")
check("...a peace proposal", proposes({ kind = "peace", action = "PROPOSED" }), true)
check("...an alliance proposal", proposes({ kind = "alliance", action = "PROPOSED" }), true)
check("...the strategic cash bid", proposes({ kind = "buy", action = "PROPOSED" }), true)
check("...not an EQUALIZE buy", proposes({ kind = "buy", action = "EQUALIZE" }), false)
check("...not a sale", proposes({ kind = "sell", action = "EQUALIZE" }), false)
check("the closer's hold expiry reaches the agent", type(listeners.CivvisDealHoldExpired), "function")

-- ── Both switches off: the live hang path, unchanged ──
local plain = bid(3)
check("off: the bid is submitted", plain.ordered, true)
check("off: ...as a cash offer", eventField(lastEvent("deal_offer"), "action"), "PROPOSED")
open(3, 900)
check("off: the opening statement puts the question", plain.sends[1] and plain.sends[1].action, "proposed")
verdict(3, 900)
check("off: the refusal waits for a counter", eventField(lastEvent("deal_session"), "phase"), "counter_wait")
check("off: ...for four seconds", holds[#holds].seconds, 4)
expired("DiplomacyDealView", 3, true)
check("off: an expired hold sends nothing", #plain.sends, 1)
onClosed(900)
check("off: the close is counter_not_sent", eventField(lastEvent("deal_session"), "why"), "counter_not_sent")
check("off: the session is forgotten", trade.sessions[3], nil)

-- ── DealAskOnTick: the bid is put from the tick ──
cfg.DealAskOnTick = true
TURN = TURN + 3
local ticked = bid(3)
check("tick: the bid is submitted", ticked.ordered, true)
local deferredBefore = count("ask_deferred")
open(3, 901)
check("tick: nothing is sent from the opening dispatch", #ticked.sends, 0)
check("tick: the session is not marked sent", trade.sessions[3] and trade.sessions[3].sent, false)
check("tick: the wait is in the ledger", eventField(lastEvent("deal_session"), "phase"), "ask_deferred")
check("tick: ...as a PROPOSED ask", eventField(lastEvent("deal_session"), "action"), "PROPOSED")
ack(3, 901)
check("tick: the acknowledgement sends nothing either", #ticked.sends, 0)
check("tick: ...and records one wait, not two", count("ask_deferred"), deferredBefore + 1)
trade.askDeferred()
check("tick: the tick puts the bid", ticked.sends[1] and ticked.sends[1].action, "proposed")
check("tick: ...once", #ticked.sends, 1)
check("tick: the question is in the ledger", eventField(lastEvent("deal_session"), "phase"), "asked")
check("tick: ...for the session the statement named", eventField(lastEvent("deal_session"), "session"), "901")
check("tick: the closer holds for the answer", holds[#holds].seconds, 30)
trade.askDeferred()
check("tick: a second tick sends nothing more", #ticked.sends, 1)
verdict(3, 901)
check("tick: the refusal waits for its counter", eventField(lastEvent("deal_session"), "phase"), "counter_wait")
check("tick: the refusal settles the pending offer", trade.pending[3], nil)
onStatement(3, 7, { StatementType = "MAKE_DEAL", SessionID = 901, StatementSubType = "POSITIVE",
	DealAction = DealProposalAction.PROPOSED })
check("tick: the counter is refused", ticked.sends[2] and ticked.sends[2].action, "rejected")
check("tick: ...and the session closed by us", sessions.closed[#sessions.closed], 901)
check("tick: ...as counter_answered", eventField(lastEvent("deal_session"), "why"), "counter_answered")

-- An EQUALIZE buy and a sale are still asked from the opening statement.
cfg.StrategicCashCeilingBid = nil
TURN = TURN + 3
local equalize = bid(4)
open(4, 902)
check("tick: an EQUALIZE buy is still asked inline", equalize.sends[1] and equalize.sends[1].action, "equalize")
trade.close(7, 4, "test_equalize")
cfg.StrategicCashCeilingBid = true

-- -cont1 t152: the session opened after its opening hold ran out.
TURN = TURN + 3
local late = bid(5)
open(5, 903)
trade.sessions[5].end_turn_hold_expired = true
local closedBefore = #sessions.closed
trade.askDeferred()
check("overtaken: the bid is not put", #late.sends, 0)
check("overtaken: ...nor closed under an acknowledgement still to come", #sessions.closed, closedBefore)
check("overtaken: it is marked unasked", eventField(lastEvent("deal_session"), "phase"), "unasked")
check("overtaken: its pending offer goes", trade.pending[5], nil)
check("overtaken: its retry wait is put back", trade.asked[5], nil)
ack(5, 903)
check("overtaken: the acknowledgement closes it", sessions.closed[#sessions.closed], 903)
check("overtaken: ...as opened_after_end_turn", eventField(lastEvent("deal_session"), "why"), "opened_after_end_turn")
check("overtaken: nothing was sent", #late.sends, 0)
local again = bid(5)
check("overtaken: the next bid is not held back", again.ordered, true)
trade.close(7, 5, "test_again")

-- Closed under us before the tick asked: nothing owed, nothing held.
TURN = TURN + 3
local shut = bid(6)
open(6, 904)
local unansweredBefore = trade.unanswered
onClosed(904)
check("closed first: the close says closed_before_asked", eventField(lastEvent("deal_session"), "why"), "closed_before_asked")
check("closed first: no unanswered ask", trade.unanswered, unansweredBefore)
check("closed first: no turn held", trade.turnHold, nil)
check("closed first: no pending offer", trade.pending[6], nil)
check("closed first: no retry wait", trade.asked[6], nil)
trade.askDeferred()
check("closed first: the tick then sends nothing", #shut.sends, 0)

-- The Gold went elsewhere before the tick: the bid is not put.
TURN = TURN + 3
local spent = bid(8)
open(8, 905)
spent.gold = 120
trade.askDeferred()
check("spent: the bid is not put", #spent.sends, 0)
check("spent: the session is closed", sessions.closed[#sessions.closed], 905)
check("spent: ...as cash_budget_changed", eventField(lastEvent("deal_session"), "why"), "cash_budget_changed")
check("spent: the pending offer goes", trade.pending[8], nil)
cfg.DealAskOnTick = nil

-- ── DealCounterRefuse: the refusal left on the deal screen is declined ──
cfg.DealCounterRefuse = true
local function refused(subject, id, opts)
	TURN = TURN + 3
	local state = bid(subject, opts)
	open(subject, id)
	check("refuse " .. subject .. ": the bid is put", state.sends[1] and state.sends[1].action, "proposed")
	verdict(subject, id)
	check("refuse " .. subject .. ": the refusal waits for its counter",
		eventField(lastEvent("deal_session"), "phase"), "counter_wait")
	return state
end

-- t118: REJECTED, no refusal dialogue, the deal view up.
local left = refused(3, 910)
local holdsBefore = #holds
expired("DiplomacyDealView", 3, true)
check("left: REJECTED is sent", left.sends[2] and left.sends[2].action, "rejected")
check("left: ...to that rival", left.sends[2] and left.sends[2].subject, 3)
check("left: ...once", #left.sends, 2)
check("left: the hold is re-armed before the send", left.sends[2] and left.sends[2].holds, holdsBefore + 1)
check("left: ...for the grace", holds[holdsBefore + 1].seconds, 2)
check("left: the ledger says refusing", eventField(lastEvent("deal_session"), "phase"), "refusing")
check("left: the session stays registered for its counter", trade.sessions[3] ~= nil, true)
closedBefore = #sessions.closed
local unansweredBeforeLeft = trade.unanswered
onClosed(910)
check("left: the game's close is rejected_working_deal",
	eventField(lastEvent("deal_session"), "why"), "rejected_working_deal")
check("left: ...closed by the game", eventField(lastEvent("deal_session"), "closer"), "game")
check("left: no CloseSession of ours", #sessions.closed, closedBefore)
check("left: the session is forgotten", trade.sessions[3], nil)
check("left: it is not an unanswered ask", trade.unanswered, unansweredBeforeLeft)

-- The echo of our REJECTED (which carries REJECTED) is not the counter; the
-- counter after it is refused.
local echoed = refused(4, 911)
expired("DiplomacyActionView", 4, true)
check("echo: REJECTED is sent from the action view too", echoed.sends[2] and echoed.sends[2].action, "rejected")
onStatement(4, 7, { StatementType = "MAKE_DEAL", SessionID = 911,
	StatementSubType = "HUMAN_REFUSE_DEAL", DealAction = DealProposalAction.REJECTED })
check("echo: our own refusal echoed is not the counter", #echoed.sends, 2)
check("echo: ...and is in the ledger as the echo", eventField(lastEvent("counter_wait"), "reason"), "refusal_echo")
check("echo: the session still waits", trade.sessions[4] ~= nil, true)
onStatement(4, 7, { StatementType = "MAKE_DEAL", SessionID = 911, StatementSubType = "POSITIVE",
	DealAction = DealProposalAction.PROPOSED })
check("late counter: refused", echoed.sends[3] and echoed.sends[3].action, "rejected")
check("late counter: after the refusal", eventField(lastEvent("counter_answered"), "after_refusal"), "true")
check("late counter: closed as counter_answered", eventField(lastEvent("deal_session"), "why"), "counter_answered")

-- The grace runs out: the ordinary closer takes it.
local slow = refused(5, 912)
expired("DiplomacyDealView", 5, true)
holdsBefore = #holds
expired("DiplomacyDealView", 5, true)
check("grace out: nothing more is sent", #slow.sends, 2)
check("grace out: the hold is not re-armed again", #holds, holdsBefore)
onClosed(912)
check("grace out: ...closed by the closer", eventField(lastEvent("deal_session"), "closer"), "ladder")

-- The 27 that did not hang: the refusal shown as conversation.
local talked = refused(6, 913)
listeners.CivvisDealDialogue(6, "MAKE_DEAL", "AI_REFUSE_DEAL")
expired("DiplomacyActionView", 6, true)
check("dialogue: nothing is sent", #talked.sends, 1)
onClosed(913)
check("dialogue: the close is counter_not_sent", eventField(lastEvent("deal_session"), "why"), "counter_not_sent")
-- The conversation panel showing.
local showing = refused(8, 914)
expired("DiplomacyActionView", 8, false)
check("conversation showing: nothing is sent", #showing.sends, 1)
onClosed(914)
check("conversation showing: counter_not_sent", eventField(lastEvent("deal_session"), "why"), "counter_not_sent")
-- A counter already being refused.
local answering = refused(9, 915)
trade.sessions[9].counter_wait.answering = true
expired("DiplomacyDealView", 9, true)
check("answering: nothing more is sent", #answering.sends, 1)
trade.close(7, 9, "test_answering", true)

-- ── Both on: t118 end to end ──
cfg.DealAskOnTick = true
TURN = TURN + 3
local both = bid(3)
open(3, 920)
ack(3, 920)
check("both: nothing from the opening dispatch", #both.sends, 0)
trade.askDeferred()
check("both: the tick puts the bid", both.sends[1] and both.sends[1].action, "proposed")
verdict(3, 920)
expired("DiplomacyDealView", 3, true)
check("both: the refusal is declined", both.sends[2] and both.sends[2].action, "rejected")
onClosed(920)
check("both: rejected_working_deal", eventField(lastEvent("deal_session"), "why"), "rejected_working_deal")
check("both: nothing left registered", trade.sessions[3], nil)
check("both: no pending offer", trade.pending[3], nil)
cfg.DealAskOnTick = nil
cfg.DealCounterRefuse = nil

if failures > 0 then
	realPrint(failures .. " failure(s)")
	os.exit(1)
end
realPrint("deal cash bid fixes: all ok")
