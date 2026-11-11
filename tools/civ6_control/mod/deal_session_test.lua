-- Offline regression for the deal SESSION lane: a sale, a passage purchase
-- and a peace offer are asked inside a `MAKE_DEAL` diplomacy session, the way
-- the shipped screens do, because a session-less working deal is never
-- evaluated — 636 EQUALIZE asks and 253 peace proposals went out over 42 live
-- runs and not one answer ever came back. This test loads the shipped agent
-- and drives the real order arms, the real statement handler and the real
-- closer against a fixture that plays the engine's part: the session opens,
-- our opening statement puts the question, the rival's statement carries the
-- verdict, the answer is accepted or walked away from, and the session is
-- closed by us. It also pins the stand-down: three unanswered sessions and
-- the lane stops opening screens for the run.
--
-- Run: lua5.1 tools/civ6_control/mod/deal_session_test.lua

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
	DealSessions = true, DealSessionHoldSeconds = 4, DealSessionStandDown = 3 }

local events = {}
local realPrint = print
print = function(line)
	if type(line) == "string" and (line:find('"kind":"deal_', 1, true)
			or line:find('"kind":"peace_', 1, true)
			or line:find('"kind":"orphan_', 1, true)
			or line:find('"kind":"counter_', 1, true)) then
		events[#events + 1] = line
	end
end

-- The one channel the contexts share: record what the agent tells the closer.
local holds = {}
LuaEvents = setmetatable({
	CivvisDealSession = function(subject, open, seconds)
		holds[#holds + 1] = { subject = subject, open = open, seconds = seconds }
	end,
}, { __index = function() return stub() end })

DealDirection = { OUTGOING = "outgoing", INCOMING = "incoming" }
DealItemTypes = { RESOURCES = "resources", FAVOR = "favor", GOLD = "gold", AGREEMENTS = "agreements", GREATWORK = "greatwork" }
DealAgreementTypes = { MAKE_PEACE = "make_peace", OPEN_BORDERS = "open_borders" }
DealProposalAction = {
	PENDING = "pending", PROPOSED = "proposed", ACCEPTED = "accepted", REJECTED = "rejected",
	ADJUSTED = "adjusted", EQUALIZE = "equalize", EQUALIZE_FAILED = "equalize_failed",
}
local resourceRows = { RESOURCE_DYES = { ResourceType = "RESOURCE_DYES", Index = 12 } }
GameInfo = {
	Resources = setmetatable({}, { __index = function(_, key)
		if type(key) == "number" then
			for _, row in pairs(resourceRows) do if row.Index == key then return row end end
			return nil
		end
		return resourceRows[key]
	end }),
	Resource_Consumption = {},
	GreatWorks = {},
}
local TURN = 50
Game = {
	GetLocalPlayer = function() return 7 end,
	GetCurrentGameTurn = function() return TURN end,
}

local sessions = { requested = {}, closed = {}, nextId = 900 }
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

-- The engine's part: one rival's deal manager, with `opts.incoming` as the
-- rival's answer.
local function fixture(opts)
	opts = opts or {}
	local state = { sends = {}, items = {}, atWar = opts.atWar or false,
		dealsEqual = opts.dealsEqual ~= false, pending = opts.pending == true, gold = opts.gold or 400 }
	local nextId = 100
	local function newItem(kind)
		local item = { kind = kind, id = nextId }
		nextId = nextId + 1
		function item:SetValueType(v) self.valueType = v end
		function item:SetSubType(v) self.subType = v end
		function item:GetSubType() return self.subType end
		function item:SetDuration(v) self.duration = v end
		function item:SetAmount(v)
			self.amount = (kind == DealItemTypes.GOLD and opts.goldReadbackMismatch) and v + 1 or v
		end
		function item:SetLocked(v) self.locked = v end
		function item:GetAmount() return self.amount end
		function item:GetMaxAmount()
			if kind == DealItemTypes.GOLD then
				assert(self.duration == 0, "one-time Gold maximum requires duration first")
				if opts.goldMaxMissing then return nil end
				return opts.goldMax or 999
			end
			return 999
		end
		function item:IsValid() return not (kind == DealItemTypes.GOLD and opts.goldInvalid) end
		function item:GetID() return self.id end
		return item
	end
	local outgoing = {}
	function outgoing:AddItemOfType(kind, owner)
		if kind == DealItemTypes.GOLD and opts.noGoldItem then return nil end
		local item = newItem(kind); item.owner = owner
		state.items[#state.items + 1] = item
		return item
	end
	function outgoing:Validate() state.validated = (state.validated or 0) + 1 end
	function outgoing:IsValid() return opts.dealInvalid ~= true end
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
		GetPossibleDealItems = function(pid, subject, kind)
			if kind == DealItemTypes.RESOURCES then
				return opts.possibleResources or { { ForType = 12, MaxAmount = 3, IsValid = true } }
			end
			return {}
		end,
		CopyIncomingToOutgoingWorkingDeal = function() state.copied = true end,
		AreWorkingDealsEqual = function() return state.dealsEqual end,
		SendWorkingDeal = function(action, pid, subject)
			state.sends[#state.sends + 1] = { action, pid, subject, holds = #holds }
			if opts.onSend ~= nil then opts.onSend(action) end
		end,
	}
	local player = {
		GetDiplomacy = function()
			return {
				HasMet = function() return true end,
				IsAtWarWith = function() return state.atWar end,
				HasOpenBordersFrom = function() return false end,
			}
		end,
		GetTreasury = function() return { GetGoldBalance = function()
			if opts.goldMissing then return nil end
			return state.gold
		end } end,
	}
	Players = setmetatable({ [7] = player }, {
		__index = function() return { IsMajor = function() return true end } end,
	})
	return state, player
end

-- A rejected equalized Great Work quote is proposed once, then waits for
-- the engine's actual verdict. This drives the real session lifecycle.
;(function()
    local function resetWork()
        trade.pending, trade.asked, trade.sessions = {}, {}, {};
        trade.unanswered, trade.disabled, trade.turnHold = 0, false, nil;
        sessions.requested, sessions.closed = {}, {};
        events, holds = {}, {};
    end
    local function quote(work, gold, gpt, extra)
        local items = {
            { kind = DealItemTypes.GREATWORK, from = 3, valueType = work or 13 },
            { kind = DealItemTypes.GOLD, from = 7, amount = gold or 50 },
            { kind = DealItemTypes.GOLD, from = 7, amount = gpt or 5, duration = 30 },
        };
        if extra then items[#items + 1] = extra; end
        return items;
    end
    local function answer(action)
        onStatement(3, 7, { StatementType = "MAKE_DEAL", SessionID = 901,
            DealAction = action });
    end
    local function workSession(opts)
        resetWork();
        opts = opts or {};
        opts.incoming = opts.incoming or quote();
        local state = fixture(opts);
        trade.pending[3] = { direction = "buy", want = "GREATWORK:13",
            ceiling = opts.ceiling or 184, gold_cap = opts.goldCap, gpt_cap = opts.gptCap, turn = TURN };
        trade.ask(7, 3, "EQUALIZE", "buy", TURN);
        onStatement(7, 3, { StatementType = "MAKE_DEAL", SessionID = 901 });
        return state;
    end

    local fair = workSession();
    answer(DealProposalAction.REJECTED);
    check("a rejected exact affordable work quote is PROPOSED", fair.sends[2] and fair.sends[2][1], "proposed");
    check("a quote is not accepted without the rival's verdict", #fair.sends, 2);
    check("the proposal ledger names PROPOSED", eventField(lastEvent("deal_reproposed"), "action"), "PROPOSED");
    check("the proposal preserves the pending purchase", trade.pending[3] ~= nil, true);
    check("the proposal keeps the MAKE_DEAL session open", #sessions.closed, 0);
    check("the proposal renews the answer hold", holds[#holds] and holds[#holds].seconds, 30);
    check("the hold precedes the proposal", fair.sends[2] and fair.sends[2].holds, #holds);
    fair.pending = true;
    answer(DealProposalAction.REJECTED);
    check("a duplicate old verdict sends no second proposal", #fair.sends, 2);
    check("the engine's pending proposal keeps its session", #sessions.closed, 0);
    fair.pending = false;
    answer(DealProposalAction.ACCEPTED);
    check("an accepted proposal reaches the existing closer", fair.sends[3] and fair.sends[3][1], "accepted");
    check("acceptance settles the pending request", trade.pending[3], nil);
    check("acceptance closes the session once", #sessions.closed, 1);

    local financed = workSession({ ceiling = 500, goldCap = 227, gptCap = 20,
        incoming = quote(13, 105, 12) });
    answer(DealProposalAction.REJECTED);
    check("an affordable financed quote is re-proposed", financed.sends[2] and financed.sends[2][1], "proposed");
    answer(DealProposalAction.ACCEPTED);
    check("its confirmed response can be accepted", financed.sends[3] and financed.sends[3][1], "accepted");

    for name, spec in pairs({
        cash = { cash = 228, gpt = 0, limit = 20 },
        income = { cash = 0, gpt = 11, limit = 10 },
        total = { cash = 200, gpt = 13, limit = 20 },
    }) do
        local excess = workSession({ ceiling = 500, goldCap = 227, gptCap = spec.limit,
            incoming = quote(13, spec.cash, spec.gpt) });
        answer(DealProposalAction.REJECTED);
        check(name .. " limit also blocks re-proposal", #excess.sends, 1);
    end
    -- Native 082733Z t99: proposing a changed 8 Gold + 9 GPT package
    -- returned the original 58 Gold + 7 GPT quote, then repeatedly stalled.
    -- Until that native protocol is understood, do not mutate the quote.
    local constrained = workSession({ ceiling = 500, goldCap = 22, gptCap = 20,
        incoming = quote(13, 58, 7) });
    answer(DealProposalAction.REJECTED);
    check("a component-constrained work quote sends no changed proposal", #constrained.sends, 1);
    check("a component-constrained work quote is never copied for mutation", constrained.copied, nil);
    check("the constrained work ask releases its pending request", trade.pending[3], nil);
    check("the constrained work session closes once", #sessions.closed, 1);
    answer(DealProposalAction.ADJUSTED);
    check("a late counterquote cannot revive the declined work ask", #constrained.sends, 1);
    check("a declined constrained quote cannot record a purchase", lastEvent("deal_closed"), nil);

    local laterIncome = quote();
    local adjustedIncome = workSession({ ceiling = 500, goldCap = 227, gptCap = 10, incoming = laterIncome });
    answer(DealProposalAction.REJECTED);
    laterIncome[3].amount = 11;
    answer(DealProposalAction.ADJUSTED);
    check("a later adjustment cannot exceed the GPT cap", #adjustedIncome.sends, 2);

    local refused = workSession();
    answer(DealProposalAction.REJECTED);
    answer(DealProposalAction.REJECTED);
    check("a second rejection cannot loop proposals", #refused.sends, 2);
    check("a second rejection clears the request", trade.pending[3], nil);
    check("a second rejection closes the session", #sessions.closed, 1);

    local pendingQuote = quote();
    local waiting = workSession({ incoming = pendingQuote });
    answer(DealProposalAction.REJECTED);
    pendingQuote[2].amount, pendingQuote[3].amount = 0, 0;
    waiting.pending = true;
    answer(DealProposalAction.REJECTED);
    check("a stale zero-price work callback still waits for the engine", trade.pending[3] ~= nil, true);
    check("a stale zero-price work callback leaves the session open", #sessions.closed, 0);
    check("a stale zero-price work callback sends no duplicate", #waiting.sends, 2);

    for name, incoming in pairs({
        over_budget = quote(13, 60, 5),
        substituted_work = quote(14, 50, 0),
        collateral = quote(13, 50, 0, { kind = DealItemTypes.FAVOR, from = 7, amount = 20 }),
        foreign_item = quote(13, 50, 0, { kind = DealItemTypes.RESOURCES, from = 3, amount = 1, valueType = 12 }),
        no_price = quote(13, 0, 0),
    }) do
        local guarded = workSession({ incoming = incoming });
        answer(DealProposalAction.REJECTED);
        check(name .. " is not re-proposed", #guarded.sends, 1);
        check(name .. " closes the session", #sessions.closed, 1);
    end
    local changedQuote = quote();
    local changedPrice = workSession({ incoming = changedQuote });
    answer(DealProposalAction.REJECTED);
    changedQuote[2].amount, changedQuote[3].amount = 185, 0;
    answer(DealProposalAction.ADJUSTED);
    check("a later adjustment still respects the original ceiling", #changedPrice.sends, 2);
    check("an over-budget adjustment clears the request", trade.pending[3], nil);

    local changedItem = quote();
    local substituted = workSession({ incoming = changedItem });
    answer(DealProposalAction.REJECTED);
    changedItem[1].valueType = 14;
    answer(DealProposalAction.ACCEPTED);
    check("an accepted response cannot substitute the purchased work", #substituted.sends, 2);

    local passage = workSession({ incoming = {
        { kind = DealItemTypes.AGREEMENTS, from = 3, subType = DealAgreementTypes.OPEN_BORDERS },
        { kind = DealItemTypes.GOLD, from = 7, amount = 50 },
    } });
    trade.pending[3].want = "OPEN_BORDERS";
    answer(DealProposalAction.REJECTED);
    check("other purchase protocols retain their existing behavior", #passage.sends, 1);

    local unequal = workSession({ dealsEqual = false });
    answer(DealProposalAction.REJECTED);
    check("unequal copied working deals are not proposed", #unequal.sends, 1);
    check("unequal working deals settle the request", trade.pending[3], nil);

    local unavailable = workSession();
    DealManager.HasPendingDeal = function() return nil; end;
    answer(DealProposalAction.REJECTED);
    check("unreadable pending state cannot propose", #unavailable.sends, 1);

    local sync = workSession({ onSend = function(action)
        if action == DealProposalAction.PROPOSED then answer(DealProposalAction.ACCEPTED); end
    end });
    answer(DealProposalAction.REJECTED);
    check("a synchronous answer can accept once", sync.sends[3] and sync.sends[3][1], "accepted");
    check("a synchronous answer closes once", #sessions.closed, 1);
    check("a synchronous answer leaves no pending ask", trade.pending[3], nil);

    local thrown = workSession({ onSend = function(action)
        if action == DealProposalAction.PROPOSED then error("fixture send failure"); end
    end });
    answer(DealProposalAction.REJECTED);
    check("a throwing proposal clears its request", trade.pending[3], nil);
    check("a throwing proposal closes its session", #sessions.closed, 1);
    check("a throwing proposal is never accepted", #thrown.sends, 2);

    workSession();
    answer(DealProposalAction.REJECTED);
    onClosed(901);
    check("a timed-out proposal releases the pending request", trade.pending[3], nil);
    check("a timed-out proposal releases the session", trade.sessions[3], nil);
    resetWork();
end)()


-- Native 135842Z t146: re-entering EQUALIZE with a paid PROPOSED led to
-- a late EQUALIZE_FAILED and a core-idle stall. Build one funded offer BEFORE
-- opening the session, wait for the verdict, and own any following counter.
;(function()
    local function resetResource()
        trade.pending, trade.asked, trade.sessions = {}, {}, {};
        trade.unanswered, trade.disabled, trade.turnHold = 0, false, nil;
        sessions.requested, sessions.closed = {}, {};
        events, holds = {}, {};
    end
    local function quote(amount, payment, resource)
        local items = { { kind = DealItemTypes.RESOURCES, from = 3,
            valueType = resource or 43, amount = amount or 2 } };
        if payment then items[#items + 1] = { kind = DealItemTypes.GOLD, from = 7, amount = payment }; end
        return items;
    end
    local function answer(action)
        onStatement(3, 7, { StatementType = "MAKE_DEAL", SessionID = 901, DealAction = action });
    end
    local function open()
        onStatement(7, 3, { StatementType = "MAKE_DEAL", SessionID = 901 });
    end
    local function resourceSession(opts)
        resetResource(); opts = opts or {};
        opts.incoming = opts.incoming or quote();
        local state = fixture(opts);
        trade.pending[3] = { direction = "buy", want = "RESOURCES:43", want_amount = 2,
            strategic = true, verb = "RESOURCE_IRON=2", ceiling = 45, turn = TURN };
        trade.ask(7, 3, "EQUALIZE", "buy", TURN);
        open();
        return state;
    end
    local bare = resourceSession();
    answer(DealProposalAction.REJECTED);
    check("legacy resource refusal never re-enters the core with PROPOSED", #bare.sends, 1);
    check("legacy refusal settles its pending want", trade.pending[3], nil);
    check("legacy refusal closes once", #sessions.closed, 1);
    answer(DealProposalAction.EQUALIZE_FAILED);
    check("late failed equalization cannot revive a resource quote", #bare.sends, 1);
    check("late failed equalization cannot close twice", #sessions.closed, 1);
    for _, verdict in ipairs({ DealProposalAction.ACCEPTED, DealProposalAction.ADJUSTED }) do
        local legacy = resourceSession({ incoming = quote(2, 40) });
        answer(verdict);
        check("legacy confirmed resource quote still closes " .. verdict,
            legacy.sends[2] and legacy.sends[2][1], "accepted");
    end

    CivvisControlConfig.StrategicCashCeilingBid = true;
    CivvisControlConfig.VictoryLane = "domination";
    resourceRows.RESOURCE_IRON = { ResourceType = "RESOURCE_IRON", Index = 43,
        ResourceClassType = "RESOURCECLASS_STRATEGIC" };
    GameInfo.Resource_Consumption.RESOURCE_IRON = { Accumulate = true };
    local function cashSession(opts, deferOpen)
        resetResource(); opts = opts or {};
        opts.incoming = opts.incoming or quote(2, 34);
        if opts.gold == nil then opts.gold = 129 end
        opts.possibleResources = { { ForType = 43, MaxAmount = 10, IsValid = true } };
        local state, player = fixture(opts);
        state.ordered = applyOrder(player, 7,
            { kind = "buy", subject = "3", verb = "RESOURCE_IRON=2", x = 34 }, TURN);
        if not deferOpen then open(); end
        return state;
    end
    local paid = cashSession(nil, true);
    check("the real buy arm prepares the cash offer", paid.ordered, true);
    check("the offer waits for a session", #paid.sends, 0);
    check("the request opens MAKE_DEAL", sessions.requested[1] and sessions.requested[1].kind, "MAKE_DEAL");
    check("the pending offer protects ask-time cash", trade.pending[3] and trade.pending[3].cash_reserve, 95);
    check("the pending offer records its bid", trade.pending[3] and trade.pending[3].cash_offer_bid, 34);
    check("the initial resource is requested from the rival", paid.items[1] and paid.items[1].owner, 3);
    check("the initial resource quantity is exact", paid.items[1] and paid.items[1].amount, 2);
    check("payment exists before the opening statement", paid.items[2] and paid.items[2].amount, 34);
    check("payment belongs to us", paid.items[2] and paid.items[2].owner, 7);
    check("payment uses no future income", paid.items[2] and paid.items[2].duration, 0);
    open();
    check("the first and only question is PROPOSED", paid.sends[1] and paid.sends[1][1], "proposed");
    check("there is no initial EQUALIZE", #paid.sends, 1);
    check("the ledger identifies a funded initial offer", eventField(lastEvent("deal_offer"), "cash_bid"), "34");
    answer(DealProposalAction.ACCEPTED);
    check("a real engine acceptance can be accepted", paid.sends[2] and paid.sends[2][1], "accepted");
    check("acceptance settles the request", trade.pending[3], nil);
    check("acceptance closes once", #sessions.closed, 1);

    local adjusted = cashSession({ incoming = quote(2, 30) });
    answer(DealProposalAction.ADJUSTED);
    check("a cheaper valid host adjustment can close", adjusted.sends[2] and adjusted.sends[2][1], "accepted");
    local sync = cashSession({ onSend = function(action)
        if action == DealProposalAction.PROPOSED then answer(DealProposalAction.ACCEPTED); end
    end });
    check("a synchronous verdict accepts once", sync.sends[2] and sync.sends[2][1], "accepted");
    check("a synchronous verdict closes once", #sessions.closed, 1);
    check("a synchronous verdict leaves no pending offer", trade.pending[3], nil);
    local limited = cashSession({ goldMax = 20, incoming = quote(2, 20) });
    check("the native maximum clips the initial bid", limited.items[2] and limited.items[2].amount, 20);
    check("native clipping preserves the original reserve", trade.pending[3] and trade.pending[3].cash_reserve, 95);
    answer(DealProposalAction.ACCEPTED);
    check("the clipped valid bid can close", limited.sends[2] and limited.sends[2][1], "accepted");

    for name, opts in pairs({
        insufficient_cash = { gold = 33 }, missing_balance = { goldMissing = true },
        nonfinite_balance = { gold = math.huge }, nan_balance = { gold = 0/0 },
        unknown_maximum = { goldMaxMissing = true }, invalid_gold = { goldInvalid = true },
        nonfinite_maximum = { goldMax = 0/0 }, infinite_maximum = { goldMax = math.huge },
        no_native_cash = { goldMax = 0 }, invalid_deal = { dealInvalid = true },
        missing_gold_item = { noGoldItem = true }, wrong_readback = { goldReadbackMismatch = true },
    }) do
        local blocked = cashSession(opts, true);
        check(name .. " cannot prepare a cash offer", blocked.ordered, false);
        check(name .. " cannot open a session", #sessions.requested, 0);
        check(name .. " cannot send a deal", #blocked.sends, 0);
        check(name .. " leaves no pending ask", trade.pending[3], nil);
    end
    local preSpent = cashSession(nil, true); preSpent.gold = 120;
    open();
    check("spending before session acknowledgement prevents submission", #preSpent.sends, 0);
    check("a stale cash budget clears its pending ask", trade.pending[3], nil);
    check("a stale cash budget closes its session", #sessions.closed, 1);
    local thrown = cashSession({ onSend = function(action)
        if action == DealProposalAction.PROPOSED then error("fixture send failure"); end
    end });
    check("a throwing initial send is not retried", #thrown.sends, 1);
    check("a throwing initial send clears its pending ask", trade.pending[3], nil);
    check("a throwing initial send closes once", #sessions.closed, 1);

    for name, items in pairs({
        over_budget = quote(2, 35), short_quantity = quote(1, 34), wrong_resource = quote(2, 34, 42),
        future_income = { { kind = DealItemTypes.RESOURCES, from = 3, valueType = 43, amount = 2 },
            { kind = DealItemTypes.GOLD, from = 7, amount = 1, duration = 30 } },
        collateral = { { kind = DealItemTypes.RESOURCES, from = 3, valueType = 43, amount = 2 },
            { kind = DealItemTypes.FAVOR, from = 7, amount = 10 } },
    }) do
        local blocked = cashSession({ incoming = items });
        answer(DealProposalAction.ADJUSTED);
        check(name .. " cannot be accepted", #blocked.sends, 1);
        check(name .. " settles without buying", trade.pending[3], nil);
    end
    local unequal = cashSession({ dealsEqual = false });
    answer(DealProposalAction.ACCEPTED);
    check("unequal working deals cannot be accepted", #unequal.sends, 1);
    local delayed = cashSession(); delayed.gold = 120;
    answer(DealProposalAction.ACCEPTED);
    check("cash spent awaiting a verdict cannot consume the reserve", #delayed.sends, 1);
    check("cash spent awaiting a verdict settles without buying", trade.pending[3], nil);

    local refused = cashSession();
    answer(DealProposalAction.REJECTED);
    check("a refusal cannot submit another resource offer", #refused.sends, 1);
    check("a refusal settles the pending want", trade.pending[3], nil);
    check("a refusal keeps ownership for the following counter", #sessions.closed, 0);
    check("the following counter has a bounded hold", holds[#holds] and holds[#holds].seconds, 4);
    answer(DealProposalAction.REJECTED);
    answer(DealProposalAction.EQUALIZE_FAILED);
    check("duplicate refusal callbacks do not submit or close", #refused.sends + #sessions.closed, 1);
    answer(DealProposalAction.PROPOSED);
    check("a following counter is refused using the shipped action", refused.sends[2] and refused.sends[2][1], "rejected");
    check("a following counter closes once", #sessions.closed, 1);
    local syncCounter = cashSession({ onSend = function(action)
        if action == DealProposalAction.REJECTED then answer(DealProposalAction.PROPOSED); end
    end });
    answer(DealProposalAction.REJECTED);
    answer(DealProposalAction.PROPOSED);
    check("a synchronous counter callback cannot re-enter its refusal", #syncCounter.sends, 2);
    check("a synchronous counter callback closes once", #sessions.closed, 1);
    local timed = cashSession(); answer(DealProposalAction.REJECTED);
    onClosed(901);
    check("the bounded closer releases a missing counter", trade.sessions[3], nil);
    check("a refused offer is not an unanswered session", trade.unanswered, 0);
    check("a missing counter submits no further offer", #timed.sends, 1);

    CivvisControlConfig.StrategicCashCeilingBid = nil;
    local off = cashSession();
    check("default cash-off behavior keeps EQUALIZE", off.sends[1] and off.sends[1][1], "equalize");
    check("cash-off behavior adds no payment", #off.items, 1);
    CivvisControlConfig.StrategicCashCeilingBid = true;
    CivvisControlConfig.VictoryLane = "science";
    local offLane = cashSession();
    check("non-Domination behavior keeps EQUALIZE", offLane.sends[1] and offLane.sends[1][1], "equalize");
    check("non-Domination behavior adds no payment", #offLane.items, 1);
    CivvisControlConfig.VictoryLane = "domination";

    -- Native131031Z t143 / cont1+cont2 t140: preserve the barter regression.
    local barter = resourceSession({ incoming = {
        { kind = DealItemTypes.RESOURCES, from = 3, valueType = 40, amount = 6 },
        { kind = DealItemTypes.RESOURCES, from = 7, valueType = 19, amount = 1, duration = 30 },
        { kind = DealItemTypes.RESOURCES, from = 7, valueType = 41, amount = 1 },
    } });
    trade.pending[3].want, trade.pending[3].want_amount = "RESOURCES:40", 6;
    trade.pending[3].verb = "RESOURCE_ALUMINUM=6;RESOURCE_IVORY=1,RESOURCE_COAL=1";
    trade.pending[3].gave = { ["RESOURCES:19"] = 1, ["RESOURCES:41"] = 1 };
    trade.pending[3].ceiling = 21;
    answer(DealProposalAction.REJECTED);
    check("the captured Aluminum barter matches its authorized items", eventField(lastEvent("deal_response"), "matches"), "true");
    check("the captured Aluminum barter is within budget", eventField(lastEvent("deal_response"), "within_budget"), "true");
    check("a rejected barter sends no new proposal", #barter.sends, 1);
    check("a rejected barter settles its pending request", trade.pending[3], nil);
    check("a rejected barter closes its session once", #sessions.closed, 1);
    answer(DealProposalAction.EQUALIZE_FAILED);
    check("late failed equalization cannot revive barter", #barter.sends, 1);
    CivvisControlConfig.StrategicCashCeilingBid = nil;
    CivvisControlConfig.VictoryLane = nil;
    resourceRows.RESOURCE_IRON = nil;
    GameInfo.Resource_Consumption.RESOURCE_IRON = nil;
    resetResource();
end)()

-- ── A sale: session first, question on our statement, close on theirs ──
local sale, player = fixture({ incoming = {
	{ kind = "gold", from = 3, duration = 0, amount = 90 },
	{ kind = "resources", from = 7, duration = 30, amount = 1, valueType = 12 },
} })
local ok, why = applyOrder(player, 7, { kind = "sell", subject = "3", verb = "RESOURCE_DYES=1", x = 60 }, TURN)
check("the sale is asked", ok, true)
check("the sale says asked", why, "sell_asked")
check("no working deal is sent before the session", #sale.sends, 0)
check("a MAKE_DEAL session is requested", sessions.requested[1] and sessions.requested[1].kind, "MAKE_DEAL")
check("the session names the rival", sessions.requested[1] and sessions.requested[1].subject, 3)
check("the closer is told to hold", holds[1] and holds[1].open, true)
check("the hold carries the configured seconds", holds[1] and holds[1].seconds, 4)
check("the session is registered", trade.sessions[3] ~= nil, true)
check("the opening is in the ledger", eventField(lastEvent("deal_session"), "phase"), "opening")

-- Unrelated diplomacy must not claim the pending outgoing deal's session.
onStatement(3, 7, { StatementType = "GREETING", SessionID = 899 })
check("a greeting does not send the pending offer", #sale.sends, 0)
check("a greeting does not bind its session", trade.sessions[3] and trade.sessions[3].sessionID, nil)
onStatement(7, 3, { StatementType = "MAKE_DEAL" })
check("an opening without a session id does not send", #sale.sends, 0)

-- Our own opening statement: the session is live, the question goes out.
onStatement(7, 3, { StatementType = "MAKE_DEAL", SessionID = 901 })
check("the question is EQUALIZE", sale.sends[1] and sale.sends[1][1], "equalize")
check("the question names both players", sale.sends[1] and (sale.sends[1][2] .. ":" .. sale.sends[1][3]), "7:3")
check("the session id is kept", trade.sessions[3] and trade.sessions[3].sessionID, 901)
check("the ask is in the ledger", eventField(lastEvent("deal_session"), "phase"), "asked")
-- The rival answers from the game core, behind the turn's queued commands
-- (DiplomacyActionView.lua:2582): live answers took up to 14.7 s, and a
-- session the 4-second opening hold closed first froze the game four times.
check("the ask re-arms the closer's hold", holds[#holds] and holds[#holds].open, true)
check("the answer window outlasts the opening hold", holds[#holds] and holds[#holds].seconds, 30)
check("the hold is re-armed before the question goes out", sale.sends[1] and sale.sends[1].holds, #holds)
check("the ask records its answer window", eventField(lastEvent("deal_session"), "hold"), "30")

trade.unanswered = 2
-- Firaxis distinguishes the opening acknowledgement from deal evaluation
-- (DiplomacyActionView.lua:2547-2559). Neither it nor another session's
-- response may accept the fixture's already-populated incoming deal.
onStatement(3, 7, { StatementType = "MAKE_DEAL", SessionID = 901,
	StatementSubType = "NONE", ResponseType = DiplomacyResponseTypes.ACKNOWLEDGE,
	DealAction = DealProposalAction.ADJUSTED })
onStatement(3, 7, { StatementType = "MAKE_DEAL", SessionID = 901 })
onStatement(3, 7, { StatementType = "MAKE_DEAL", SessionID = 999,
	DealAction = DealProposalAction.ADJUSTED })
onStatement(3, 7, { StatementType = "GREETING", SessionID = 901,
	DealAction = DealProposalAction.ADJUSTED })
check("nonanswers do not accept an incoming deal", #sale.sends, 1)
check("nonanswers keep the deal session open", trade.sessions[3] ~= nil, true)
check("nonanswers do not close native sessions", #sessions.closed, 0)
check("nonanswers do not reset the stand-down count", trade.unanswered, 2)

-- The rival's statement carries its verdict: a fair answer is accepted and
-- the session is closed by us.
onStatement(3, 7, { StatementType = "MAKE_DEAL", SessionID = 901, DealAction = DealProposalAction.ADJUSTED })
check("a fair answer is accepted", sale.sends[2] and sale.sends[2][1], "accepted")
check("the close is in the ledger", eventField(lastEvent("deal_closed"), "gold"), "90")
check("the session is closed by us", sessions.closed[1], 901)
check("the session is forgotten", trade.sessions[3], nil)
check("the closer is released", holds[#holds] and holds[#holds].open, false)
check("the answer resets the unanswered count", trade.unanswered, 0)

-- ── A lowball is walked away from, and the session still closes ──
TURN = 60
local low, lowPlayer = fixture({ incoming = {
	{ kind = "gold", from = 3, duration = 0, amount = 20 },
	{ kind = "resources", from = 7, duration = 30, amount = 1, valueType = 12 },
} })
applyOrder(lowPlayer, 7, { kind = "sell", subject = "3", verb = "RESOURCE_DYES=1", x = 60 }, TURN)
onStatement(7, 3, { StatementType = "MAKE_DEAL", SessionID = 902 })
onStatement(3, 7, { StatementType = "MAKE_DEAL", SessionID = 902, DealAction = DealProposalAction.ADJUSTED })
check("a lowball is not accepted", #low.sends, 1)
check("a lowball is in the ledger", eventField(lastEvent("deal_declined"), "worth"), "20")
check("a declined session is closed too", sessions.closed[2], 902)

-- ── An answer delivered from inside the send still releases the closer ──
-- The answer window is armed before the send, so the close an inline answer
-- triggers is the last thing the closer hears; armed after, it would hold a
-- closed session's screen for the whole window.
TURN = 65
local inline, inlinePlayer
inline, inlinePlayer = fixture({
	incoming = {
		{ kind = "gold", from = 3, duration = 0, amount = 20 },
		{ kind = "resources", from = 7, duration = 30, amount = 1, valueType = 12 },
	},
	onSend = function(action)
		if action == "equalize" then
			onStatement(3, 7, { StatementType = "MAKE_DEAL", SessionID = 907,
				DealAction = DealProposalAction.ADJUSTED })
		end
	end,
})
applyOrder(inlinePlayer, 7, { kind = "sell", subject = "3", verb = "RESOURCE_DYES=1", x = 60 }, TURN)
onStatement(7, 3, { StatementType = "MAKE_DEAL", SessionID = 907 })
check("an inline answer is read as the answer", eventField(lastEvent("deal_declined"), "worth"), "20")
check("an inline answer closes the session", trade.sessions[3], nil)
check("the closer ends released after an inline answer", holds[#holds] and holds[#holds].open, false)

-- ── Peace rides the same lane: PROPOSED on our statement, then waits for the
-- host frame that proves ACCEPTED actually ended the war. ──
TURN = 70
local peace, peacePlayer = fixture({ atWar = true })
ok, why = applyOrder(peacePlayer, 7, { kind = "peace", subject = "5" }, TURN)
check("peace is submitted through the lane", ok, true)
check("peace says submitted", why, "peace_submitted")
check("peace waits for its session", #peace.sends, 0)
check("peace requests a MAKE_DEAL session", sessions.requested[#sessions.requested].kind, "MAKE_DEAL")
onStatement(7, 5, { StatementType = "MAKE_DEAL", SessionID = 903 })
check("the peace offer is PROPOSED once the session is live", peace.sends[1] and peace.sends[1][1], "proposed")
onStatement(5, 7, { StatementType = "MAKE_DEAL", SessionID = 903,
	StatementSubType = "NONE", ResponseType = DiplomacyResponseTypes.INITIAL,
	DealAction = DealProposalAction.ADJUSTED })
onStatement(5, 7, { StatementType = "MAKE_DEAL", SessionID = 903 })
check("a peace greeting does not consume the offer", trade.sessions[5] ~= nil, true)
check("a peace greeting does not submit another deal", #peace.sends, 1)

onStatement(5, 7, { StatementType = "MAKE_DEAL", SessionID = 903, DealAction = DealProposalAction.ACCEPTED })
check("an accepted peace is enacted by our ACCEPTED", peace.sends[2] and peace.sends[2][1], "accepted")
check("the peace answer is in the ledger", eventField(lastEvent("peace_response"), "accepted"), "true")
check("the answer is not called enacted before a host frame",
	eventField(lastEvent("peace_response"), "enacted"), "false")
check("the accepted session waits for the host frame", trade.sessions[5] ~= nil, true)
peace.atWar = false
trade.pollPeace()
check("the host frame proves peace", eventField(lastEvent("peace_result"), "enacted"), "true")
check("the peace session is closed", sessions.closed[#sessions.closed], 903)

-- Match the shipped `OnProposeOrAcceptDeal` path when the returned working
-- deal is not equal: send ADJUSTED and keep the session alive instead of
-- claiming that an ACCEPTED call enacted an invalid package.
TURN = 75
local adjusted, adjustedPlayer = fixture({ atWar = true, dealsEqual = false })
applyOrder(adjustedPlayer, 7, { kind = "peace", subject = "12" }, TURN)
onStatement(7, 12, { StatementType = "MAKE_DEAL", SessionID = 905 })
onStatement(12, 7, { StatementType = "MAKE_DEAL", SessionID = 905, DealAction = DealProposalAction.ACCEPTED })
check("an unequal accepted deal is adjusted", adjusted.sends[2] and adjusted.sends[2][1], "adjusted")
check("an unequal accepted deal is not enacted", eventField(lastEvent("peace_response"), "enacted"), "false")
check("an adjusted peace session stays open", trade.sessions[12] ~= nil, true)
adjusted.dealsEqual = true
onStatement(12, 7, { StatementType = "MAKE_DEAL", SessionID = 905, DealAction = DealProposalAction.ACCEPTED })
check("a reconciled deal is accepted", adjusted.sends[3] and adjusted.sends[3][1], "accepted")
adjusted.atWar = false
trade.pollPeace()
check("a reconciled deal proves peace", eventField(lastEvent("peace_result"), "enacted"), "true")

-- An adjusted answer is not an accepted treaty. If the closer later ends the
-- still-open session, it must be closed without manufacturing a peace result.
TURN = 78
local adjustedClosed, adjustedClosedPlayer = fixture({ atWar = true, dealsEqual = false })
applyOrder(adjustedClosedPlayer, 7, { kind = "peace", subject = "13" }, TURN)
onStatement(7, 13, { StatementType = "MAKE_DEAL", SessionID = 906 })
onStatement(13, 7, { StatementType = "MAKE_DEAL", SessionID = 906, DealAction = DealProposalAction.ACCEPTED })
onClosed(906)
check("an adjusted session closes", trade.sessions[13], nil)
check("an adjusted close has no synthetic peace result",
	eventField(lastEvent("peace_result"), "target"), "12")

-- Native 20261108T022611Z t146: after our equal ACCEPTED, the engine
-- repeated ACCEPTED with unequal working deals, then PROPOSED and ACCEPTED.
-- None may turn the outstanding acceptance into an adjusted/rejected counter.
-- Also deliver that sequence inside SendWorkingDeal: its callback can re-enter.
;(function()
    TURN = 79
    local reentered = false
    local pending, pendingPlayer
    pending, pendingPlayer = fixture({ atWar = true, onSend = function(action)
        if action ~= DealProposalAction.ACCEPTED or reentered then return end
        reentered = true
        pending.dealsEqual = false
        for _, followup in ipairs({ DealProposalAction.ACCEPTED,
                DealProposalAction.PROPOSED, DealProposalAction.ACCEPTED }) do
            onStatement(20, 7, { StatementType = "MAKE_DEAL", SessionID = 920,
                DealAction = followup })
        end
    end })
    applyOrder(pendingPlayer, 7, { kind = "peace", subject = "20" }, TURN)
    onStatement(7, 20, { StatementType = "MAKE_DEAL", SessionID = 920 })
    onStatement(20, 7, { StatementType = "MAKE_DEAL", SessionID = 920,
        DealAction = DealProposalAction.ACCEPTED })
    check("reentrant peace followups do not adjust or reject", #pending.sends, 2)
    check("reentrant peace stays owned until host confirmation", trade.sessions[20] ~= nil, true)
    onStatement(20, 7, { StatementType = "MAKE_DEAL", SessionID = 920,
        DealAction = DealProposalAction.PROPOSED })
    check("later peace followups do not restart negotiation", #pending.sends, 2)
    pending.atWar = false
    trade.pollPeace()
    check("pending peace is proven by the host", eventField(lastEvent("peace_result"), "target"), "20")
    check("pending peace host result is enacted", eventField(lastEvent("peace_result"), "enacted"), "true")
    check("confirmed pending peace releases its session", trade.sessions[20], nil)

    local inline, inlinePlayer
    inline, inlinePlayer = fixture({ atWar = true, onSend = function(action)
        if action ~= DealProposalAction.ACCEPTED then return end
        inline.atWar = false
        onClosed(921)
    end })
    applyOrder(inlinePlayer, 7, { kind = "peace", subject = "21" }, TURN)
    onStatement(7, 21, { StatementType = "MAKE_DEAL", SessionID = 921 })
    onStatement(21, 7, { StatementType = "MAKE_DEAL", SessionID = 921,
        DealAction = DealProposalAction.ACCEPTED })
    check("inline host confirmation releases peace ownership", trade.sessions[21], nil)
    check("inline host confirmation is recorded", eventField(lastEvent("peace_result"), "target"), "21")
    check("inline peace confirmation does not rearm the hold", holds[#holds].open, false)

    local failed, failedPlayer = fixture({ atWar = true, onSend = function(action)
        if action == DealProposalAction.ACCEPTED then error("send failed") end
    end })
    applyOrder(failedPlayer, 7, { kind = "peace", subject = "22" }, TURN)
    onStatement(7, 22, { StatementType = "MAKE_DEAL", SessionID = 922 })
    onStatement(22, 7, { StatementType = "MAKE_DEAL", SessionID = 922,
        DealAction = DealProposalAction.ACCEPTED })
    check("failed peace submission releases ownership", trade.sessions[22], nil)
    check("failed peace submission releases the hold", holds[#holds].open, false)
    check("failed peace submission does not invent a host result", eventField(lastEvent("peace_result"), "target"), "21")
end)()

TURN = 80
local refused, refusedPlayer = fixture({ atWar = true })
applyOrder(refusedPlayer, 7, { kind = "peace", subject = "6" }, TURN)
onStatement(7, 6, { StatementType = "MAKE_DEAL", SessionID = 904 })
local closedBeforePeaceRefusal = #sessions.closed
onStatement(6, 7, { StatementType = "MAKE_DEAL", SessionID = 904, DealAction = DealProposalAction.REJECTED })
check("a refused peace sends nothing more", #refused.sends, 1)
check("the refusal is in the ledger", eventField(lastEvent("peace_response"), "accepted"), "false")

-- A refusal may be followed by the rival's counter in this same session.
-- Closing at the refusal loses ownership before that next statement arrives.
check("a refused peace keeps its session for the counter", trade.sessions[6] ~= nil, true)
check("a peace refusal does not close under its counter", #sessions.closed, closedBeforePeaceRefusal)
check("the peace counter wait is recorded", eventField(lastEvent("deal_session"), "phase"), "counter_wait")
check("the peace counter hold is bounded", holds[#holds] and holds[#holds].seconds, 4)
onStatement(6, 7, { StatementType = "MAKE_DEAL", SessionID = 9999, StatementSubType = "POSITIVE" })
onStatement(6, 7, { StatementType = "GREETING", SessionID = 904, StatementSubType = "POSITIVE" })
onStatement(7, 6, { StatementType = "MAKE_DEAL", SessionID = 904, StatementSubType = "POSITIVE" })
check("unrelated statements do not consume a peace counter", #refused.sends, 1)
onStatement(6, 7, { StatementType = "MAKE_DEAL", SessionID = 904, StatementSubType = "POSITIVE" })
check("the peace counter is refused", refused.sends[2] and refused.sends[2][1], "rejected")
check("the peace counter is recorded", eventField(lastEvent("counter_answered"), "statement"), "POSITIVE")
check("peace closes after the counter is answered", sessions.closed[#sessions.closed], 904)
check("the answered peace counter is forgotten", trade.sessions[6], nil)
check("a refused counter does not enact peace", refused.atWar, true)

TURN = 84
local noCounter, noCounterPlayer = fixture({ atWar = true })
applyOrder(noCounterPlayer, 7, { kind = "peace", subject = "18" }, TURN)
onStatement(7, 18, { StatementType = "MAKE_DEAL", SessionID = 918 })
onStatement(18, 7, { StatementType = "MAKE_DEAL", SessionID = 918, DealAction = DealProposalAction.REJECTED })
local unansweredBeforePeaceCounter = trade.unanswered
onClosed(918)
check("no peace counter closes with its own reason", eventField(lastEvent("deal_session"), "why"), "counter_not_sent")
check("a refused peace is not an unanswered ask", trade.unanswered, unansweredBeforePeaceCounter)
check("no peace counter leaves no held turn", trade.turnHold, nil)
check("no peace counter leaves no session", trade.sessions[18], nil)

CivvisControlConfig.PeaceCounterHoldSeconds = 0
TURN = 85
local immediate, immediatePlayer = fixture({ atWar = true })
applyOrder(immediatePlayer, 7, { kind = "peace", subject = "19" }, TURN)
onStatement(7, 19, { StatementType = "MAKE_DEAL", SessionID = 919 })
onStatement(19, 7, { StatementType = "MAKE_DEAL", SessionID = 919, DealAction = DealProposalAction.REJECTED })
check("zero peace counter hold restores immediate close", trade.sessions[19], nil)
check("zero hold closes the actual session", sessions.closed[#sessions.closed], 919)
CivvisControlConfig.PeaceCounterHoldSeconds = nil

CivvisControlConfig.DealSessions = "trade"
TURN = 86
local requestedBeforeTradeOnly = #sessions.requested
local directPeace, directPeacePlayer = fixture({ atWar = true })
applyOrder(directPeacePlayer, 7, { kind = "peace", subject = "20" }, TURN)
check("trade-only peace retains its direct proposal", directPeace.sends[1] and directPeace.sends[1][1], "proposed")
check("trade-only peace opens no session", #sessions.requested, requestedBeforeTradeOnly)
check("trade-only peace registers no counter wait", trade.sessions[20], nil)

-- ── Peace-only mode keeps trade asks direct but still opens a peace session ──
CivvisControlConfig.DealSessions = "peace"
TURN = 90
local requestedBeforePeaceOnly = #sessions.requested
local peaceOnlySale, peaceOnlySeller = fixture()
applyOrder(peaceOnlySeller, 7,
	{ kind = "sell", subject = "3", verb = "RESOURCE_DYES=1", x = 60 }, TURN)
check("peace-only sends sales directly", peaceOnlySale.sends[1] and peaceOnlySale.sends[1][1], "equalize")
check("peace-only sale opens no session", #sessions.requested, requestedBeforePeaceOnly)
check("peace-only sale does not spend the unanswered budget", trade.unanswered, 0)
local peaceOnly, peaceOnlyPlayer = fixture({ atWar = true })
applyOrder(peaceOnlyPlayer, 7, { kind = "peace", subject = "14" }, TURN)
check("peace-only peace waits for a session", #peaceOnly.sends, 0)
check("peace-only peace opens a session", #sessions.requested, requestedBeforePeaceOnly + 1)
onStatement(7, 14, { StatementType = "MAKE_DEAL", SessionID = 907 })
check("peace-only peace sends in the session", peaceOnly.sends[1] and peaceOnly.sends[1][1], "proposed")
onStatement(14, 7, { StatementType = "MAKE_DEAL", SessionID = 907,
	DealAction = DealProposalAction.REJECTED })
CivvisControlConfig.DealSessions = true

-- ── Unanswered sessions are counted, and the third stands the lane down ──
for i = 1, 3 do
	TURN = 100 + i * 10
	local quiet, quietPlayer = fixture()
	local asked = applyOrder(quietPlayer, 7, { kind = "sell", subject = "3", verb = "RESOURCE_DYES=1", x = 60 }, TURN)
	check("unanswered ask " .. i .. " goes out", asked, true)
	onStatement(7, 3, { StatementType = "MAKE_DEAL", SessionID = 910 + i })
	-- The closer's ladder shuts the screen; the core reports the session closed.
	onClosed(910 + i)
	check("unanswered session " .. i .. " is counted", trade.unanswered, i)
	check("unanswered session " .. i .. " drops the ask", trade.pending[3], nil)
	check("unanswered session " .. i .. " says the question went out",
		eventField(lastEvent("deal_session"), "sent"), "true")
	check("unanswered session " .. i .. " records a queued rival session",
		eventField(lastEvent("deal_session"), "queued"), "true")
end
check("the lane stands down after three", trade.disabled, true)
check("the stand-down is in the ledger", lastEvent("deal_sessions_stood_down") ~= nil, true)
local requestedBefore = #sessions.requested
TURN = 150
local direct, directPlayer = fixture()
applyOrder(directPlayer, 7, { kind = "sell", subject = "3", verb = "RESOURCE_DYES=1", x = 60 }, TURN)
check("a stood-down lane asks directly", direct.sends[1] and direct.sends[1][1], "equalize")
check("a stood-down lane opens no session", #sessions.requested, requestedBefore)

-- ── The auto-closer honours the hold, and only for the diplomacy views ──
-- ── A sent ask closed unanswered holds the end turn (G72 t126 wedge) ──
-- The rival's verdict still arrives, as its own session to us; ending the
-- turn under it is the wedge. Hold until the rival speaks or the window ends.
local queue = rawget(_G, "CivvisQueue")
local clock, endTurns = 1000.0, 0
UI = setmetatable({ GetElapsedTime = function() return clock end,
       HasSentTurnComplete = function() return false end,
       RequestAction = function() endTurns = endTurns + 1 end },
       { __index = function() return stub() end })
ActionTypes = { ACTION_ENDTURN = "end_turn" }
CivvisControlConfig.DealAnswerHoldSeconds = 30
-- A fresh lane: the earlier sections' unanswered asks left holds and sessions.
trade.disabled, trade.unanswered, trade.turnHold = false, 0, nil
trade.sessions, trade.pending, trade.asked = {}, {}, {}
local function holdPhase() return eventField(lastEvent("deal_turn_hold"), "phase") end

TURN = 200
local held, heldPlayer = fixture()
check("a held-turn ask goes out",
	applyOrder(heldPlayer, 7, { kind = "sell", subject = "3", verb = "RESOURCE_DYES=1", x = 60 }, TURN), true)
onStatement(7, 3, { StatementType = "MAKE_DEAL", SessionID = 960 })
onClosed(960)
check("a sent ask closed unanswered holds the turn", holdPhase(), "held")
queue.requestEndTurn(TURN)
check("…the end turn is not requested", endTurns, 0)
check("…and the hold says so once", holdPhase(), "holding")
clock = clock + 10
queue.requestEndTurn(TURN, { REASON = "UserForced" })
check("…a forced end turn is held too", endTurns, 0)
onStatement(3, 7, { StatementType = "MAKE_DEAL", SessionID = 961 })
check("the rival speaking releases the hold", holdPhase(), "released")
check("…naming why", eventField(lastEvent("deal_turn_hold"), "why"), "rival_spoke")
clock = clock + 1
queue.requestEndTurn(TURN)
check("…and the turn ends", endTurns, 1)

TURN = 210
local quiet2, quiet2Player = fixture()
applyOrder(quiet2Player, 7, { kind = "sell", subject = "3", verb = "RESOURCE_DYES=1", x = 60 }, TURN)
onStatement(7, 3, { StatementType = "MAKE_DEAL", SessionID = 962 })
onClosed(962)
clock = clock + 29
queue.requestEndTurn(TURN)
check("inside the window the turn is held", endTurns, 1)
clock = clock + 2
queue.requestEndTurn(TURN)
check("after DealAnswerHoldSeconds the turn ends", endTurns, 2)
check("…the hold expired", eventField(lastEvent("deal_turn_hold"), "why"), "expired")

TURN = 220
local stale, stalePlayer = fixture()
applyOrder(stalePlayer, 7, { kind = "sell", subject = "3", verb = "RESOURCE_DYES=1", x = 60 }, TURN)
onStatement(7, 3, { StatementType = "MAKE_DEAL", SessionID = 964 })
onClosed(964)
TURN = 221
clock = clock + 1
queue.requestEndTurn(TURN)
check("a hold from an earlier turn does not hold this one", endTurns, 3)
check("…it is dropped as over", eventField(lastEvent("deal_turn_hold"), "why"), "turn_over")

-- An ask that never went out (no opening statement) leaves no hold.
TURN = 230
local unsent, unsentPlayer = fixture()
applyOrder(unsentPlayer, 7, { kind = "sell", subject = "3", verb = "RESOURCE_DYES=1", x = 60 }, TURN)
local before = #events
for subject, session in pairs(trade.sessions) do
	if subject == 3 then session.sent = false end
end
trade.abandon(3, "session_closed")
check("an unsent ask closed unanswered holds nothing", trade.turnHold, nil)
clock = clock + 1
queue.requestEndTurn(TURN)
check("…the turn ends at once", endTurns, 4)

-- A normal session can still be opening or awaiting its answer when the
-- order batch reaches EndTurn. Native 20261108T084123Z t147 forced EndTurn
-- while one counter waited and another rival's request queued behind it.
do
	local saved = { sessions = trade.sessions, pending = trade.pending, asked = trade.asked,
		hold = trade.turnHold, disabled = trade.disabled, unanswered = trade.unanswered,
		now = CivvisClock.now, turn = TURN, ends = endTurns }
	local activeClock = 3000
	CivvisClock.now = function() return activeClock end
	trade.sessions, trade.pending, trade.asked = {}, {}, {}
	trade.turnHold, trade.disabled, trade.unanswered = nil, false, 0
	TURN = 600
	fixture()
	trade.ask(7, 3, "PROPOSED", "alliance", TURN)
	queue.requestEndTurn(TURN)
	check("an opening session holds the normal end turn", endTurns, saved.ends)
	activeClock = activeClock + 0.3
	queue.requestEndTurn(TURN, { REASON = "UserForced" })
	check("an opening session holds the forced end turn", endTurns, saved.ends)
	onStatement(7, 3, { StatementType = "MAKE_DEAL", SessionID = 1400 })
	check("the first active question was actually sent", trade.sessions[3].sent, true)
	activeClock = activeClock + 0.3
	queue.requestEndTurn(TURN)
	check("a sent session awaiting its verdict holds the turn", endTurns, saved.ends)
	onStatement(3, 7, { StatementType = "MAKE_DEAL", SessionID = 1400,
		DealAction = DealProposalAction.REJECTED })
	check("the refused alliance is waiting for its counter", trade.sessions[3].counter_wait ~= nil, true)
	trade.ask(7, 4, "EQUALIZE", "trade", TURN)
	activeClock = activeClock + 0.3
	queue.requestEndTurn(TURN, { REASON = "UserForced" })
	check("a counter and a queued second opening hold the turn", endTurns, saved.ends)
	trade.close(7, 3, "counter_answered")
	activeClock = activeClock + 0.3
	queue.requestEndTurn(TURN)
	check("closing one rival does not release the other opening", endTurns, saved.ends)
	onStatement(7, 4, { StatementType = "MAKE_DEAL", SessionID = 1401 })
	activeClock = activeClock + 0.3
	queue.requestEndTurn(TURN)
	check("the second rival's pending verdict still holds", endTurns, saved.ends)
	activeClock = activeClock + 29.4
	check("the sent question gets its full reply window after opening",
		trade.holdsEndTurn(TURN), true)
	trade.close(7, 4, "answered")
	activeClock = activeClock + 0.3
	queue.requestEndTurn(TURN)
	check("all active sessions closed permits the end turn", endTurns, saved.ends + 1)

	TURN = 601
	trade.ask(7, 3, "EQUALIZE", "trade", TURN)
	activeClock = activeClock + 31
	queue.requestEndTurn(TURN, { REASON = "UserForced" })
	check("an active session cannot hold beyond its deadline", endTurns, saved.ends + 2)
	trade.close(7, 3, "test_expired")
	-- An opening queued behind another session waits for the end turn itself.
	TURN = 606
	trade.ask(7, 3, "EQUALIZE", "trade", TURN)
	activeClock = activeClock + 7.9
	check("an opening is held just inside its own bound", trade.holdsEndTurn(TURN), true)
	activeClock = activeClock + 0.2
	check("an opening nobody asked through stops holding", trade.holdsEndTurn(TURN), false)
	check("…naming why", eventField(lastEvent("deal_active_hold"), "why"), "opening_unasked")
	trade.close(7, 3, "test_opening")
	TURN = 607
	trade.ask(7, 3, "EQUALIZE", "trade", TURN)
	activeClock = activeClock + 7.5
	onStatement(7, 3, { StatementType = "MAKE_DEAL", SessionID = 1402 })
	activeClock = activeClock + 29.5
	check("a question asked at the bound still gets its full reply window",
		trade.holdsEndTurn(TURN), true)
	trade.close(7, 3, "test_late_ask")
	TURN = 602
	trade.ask(7, 3, "EQUALIZE", "trade", TURN)
	TURN = 603
	check("an active session from another turn holds nothing", trade.holdsEndTurn(TURN), false)
	trade.close(7, 3, "test_old_turn")
	TURN = 604
	trade.ask(7, 3, "EQUALIZE", "trade", TURN)
	activeClock = activeClock - 1
	check("a backwards clock cannot leave an active hold", trade.holdsEndTurn(TURN), false)
	trade.close(7, 3, "test_clock")
	CivvisClock.now = function() return nil end
	TURN = 605
	trade.ask(7, 3, "EQUALIZE", "trade", TURN)
	check("an unreadable clock cannot leave an active hold", trade.holdsEndTurn(TURN), false)
	trade.close(7, 3, "test_no_clock")
	trade.sessions, trade.pending, trade.asked = saved.sessions, saved.pending, saved.asked
	trade.turnHold, trade.disabled, trade.unanswered = saved.hold, saved.disabled, saved.unanswered
	CivvisClock.now, TURN, endTurns = saved.now, saved.turn, saved.ends
end

-- ── A rival's session queued behind a dismissed screen is answered (G75 t133) ──
-- Our peace asks made 2 and 3 open their own sessions to us; 2's was refused,
-- 3's queued and opened behind a view the closer had already dismissed, so
-- nothing answered it and Civ un-readied every end turn for 3.5 minutes.
local realDiplomacy = DiplomacyManager
local open, answers = {}, {}
local stuck = false
local function closeIt(id)
	answers[#answers + 1] = "close:" .. id
	if not stuck then open[id] = nil end
end
DiplomacyManager = setmetatable({
	FindOpenSessionID = function(pid, other)
		for id, s in pairs(open) do
			if s.with == other and pid == 7 then return id end
		end
		return nil
	end,
	GetSessionInfo = function(id)
		local s = open[id]
		return s and { FromPlayer = s.from, ToPlayer = 7 } or nil
	end,
	IsSessionIDOpen = function(id) return open[id] ~= nil end,
	CloseSession = closeIt,
	AddResponse = function(id, pid, answer)
		answers[#answers + 1] = "respond:" .. id .. ":" .. answer
	end,
	HasQueuedSession = function() return false end,
}, { __index = realDiplomacy })
PlayerManager = { GetAliveMajorIDs = function() return { 7, 2, 3 } end }
local rejected = {}
local realDeals = DealManager
DealManager = setmetatable({
	SendWorkingDeal = function(action, pid, other) rejected[#rejected + 1] = action .. ":" .. other end,
}, { __index = realDeals })
CivvisControlConfig.OrphanSessionSeconds = 10
local function orphan() return lastEvent("orphan_session_answered") end

TURN = 240
trade.sessions, trade.turnHold = {}, nil
open[971] = { with = 3, from = 3 }
queue.requestEndTurn(TURN)
check("the first end turn goes out", endTurns, 5)
clock = clock + 5
queue.requestEndTurn(TURN)
check("a session open 5 s into the end turn is left to the closer", #answers, 0)
clock = clock + 5.5
queue.requestEndTurn(TURN)
check("after OrphanSessionSeconds the rival's session is refused", rejected[1], "rejected:3")
check("…then closed, as OnRefuseDeal(true) does", answers[1], "close:971")
check("…and the session is gone", open[971], nil)
check("the event names the rival", eventField(orphan(), "target"), "3")
check("…how it was answered", eventField(orphan(), "how"), "refused")
check("…that we did not own it", eventField(orphan(), "owned"), "false")
check("…and that it closed", eventField(orphan(), "still_open"), "false")
clock = clock + 1
queue.requestEndTurn(TURN)
check("with nothing open, the end turn goes out again", #answers, 1)

-- A session that will not close escalates once per second, then gives up.
TURN = 250
stuck = true
open[972] = { with = 2, from = 2 }
queue.requestEndTurn(TURN)
clock = clock + 10.5
queue.requestEndTurn(TURN)
clock = clock + 0.5
queue.requestEndTurn(TURN)
check("the net checks at most once a second", #answers, 2)
clock = clock + 0.6
queue.requestEndTurn(TURN)
check("a session still open is answered NEGATIVE", answers[3], "respond:972:NEGATIVE")
clock = clock + 1.1
queue.requestEndTurn(TURN)
check("…then closed bare", answers[4], "close:972")
clock = clock + 1.1
queue.requestEndTurn(TURN)
check("…then given up on", #answers, 4)
check("…and said so", eventField(lastEvent("orphan_session_stuck"), "session"), "972")
clock = clock + 1.1
queue.requestEndTurn(TURN)
check("…once", #answers, 4)
stuck, open = false, {}

-- At a 2x debug timescale the UI clock runs twice as fast, and both windows
-- still wait REAL seconds (CivvisClock divides the scale out): the 30 s
-- answer hold and the 10 s orphan net each take twice as many UI seconds.
local realClock = rawget(_G, "CivvisClock")
realClock.setScale(2)
trade.disabled, trade.unanswered, trade.turnHold = false, 0, nil
trade.sessions, trade.pending, trade.asked = {}, {}, {}
TURN = 260
local slow, slowPlayer = fixture()
applyOrder(slowPlayer, 7, { kind = "sell", subject = "3", verb = "RESOURCE_DYES=1", x = 60 }, TURN)
onStatement(7, 3, { StatementType = "MAKE_DEAL", SessionID = 966 })
onClosed(966)
local endsBefore = endTurns
clock = clock + 50
queue.requestEndTurn(TURN)
check("at 2x, 50 UI seconds (25 real) still hold the turn", endTurns, endsBefore)
clock = clock + 12
queue.requestEndTurn(TURN)
check("…31 real seconds release it", endTurns, endsBefore + 1)
check("…as expired", eventField(lastEvent("deal_turn_hold"), "why"), "expired")
TURN = 262
open[973] = { with = 3, from = 3 }
local answersBefore = #answers
queue.requestEndTurn(TURN)
clock = clock + 15
queue.requestEndTurn(TURN)
check("at 2x, 15 UI seconds (7.5 real) leave the session to the closer", #answers, answersBefore)
clock = clock + 6
queue.requestEndTurn(TURN)
check("…10.5 real seconds answer it", answers[#answers], "close:973")
open = {}
-- Back to the plain UI clock the sections below stamp holds with.
realClock.scale, realClock.offset = 1, 0

-- A session we opened is closed (not refused) and our ledger is closed with it.
TURN = 260
trade.sessions = { [3] = { kind = "sell", action = "EQUALIZE", turn = TURN, sent = true } }
open[973] = { with = 3, from = 7 }
local rejectedBefore = #rejected
queue.requestEndTurn(TURN)
clock = clock + 10.5
queue.requestEndTurn(TURN)
check("our own session is not refused", #rejected, rejectedBefore)
check("…it is closed", answers[#answers], "close:973")
check("…the event says we owned it", eventField(orphan(), "owned"), "true")
check("…and the ledger closes it as orphaned", eventField(lastEvent("deal_session"), "why"), "orphaned")
check("…leaving no session behind", trade.sessions[3], nil)

-- The deliberate unanswered-deal hold is not second-guessed.
TURN = 270
open[974] = { with = 3, from = 3 }
trade.turnHold = { turn = TURN, target = 3, at = clock }
local answersBefore = #answers
queue.requestEndTurn(TURN)
clock = clock + 12
CivvisTrade.answerOrphanSessions(TURN)
check("inside a deal hold the session is left alone", #answers, answersBefore)
trade.turnHold, open = nil, {}

DiplomacyManager, DealManager, PlayerManager = realDiplomacy, realDeals, nil
UI = nil  -- back to the stub for the sections below

local closerSrc = assert(io.open(here .. "/CivvisControlAutoClose.lua")):read("*a")
check("the closer listens for the session hold",
	closerSrc:find("LuaEvents.CivvisDealSession.Add(", 1, true) ~= nil, true)
check("the closer holds only the diplomacy views",
	closerSrc:find("local civvisDealView = NAME == \"DiplomacyActionView\" or NAME == \"DiplomacyDealView\";", 1, true) ~= nil, true)
check("the hold is bounded by the seconds it was given",
	closerSrc:find("dealHold = dealHold - dt;", 1, true) ~= nil, true)
check("an expired owned hold enters the force-close path",
	closerSrc:find("dealForceClose = true;", 1, true) ~= nil, true)
check("the owned hold can bypass an unfinished fade",
	closerSrc:find("if civvisDealView and not dealForceClose", 1, true) ~= nil, true)
check("a host-closed owned session keeps force-close latched",
	closerSrc:find("else\n\t\t\t\t\tdealHold = 0;\n\t\t\t\t\t-- CivvisTrade emits this event", 1, true) ~= nil, true)

-- ── A host close before the hold expires must still close the UI ──
-- The live t40 wedge happened in this ordering: the host closed the
-- Civvis-owned session before its four-second hold elapsed, the listener
-- cleared the hold, and `BlackFadeAnim:IsStopped()` stayed false forever.
-- Exercise the actual listener and update callback so a source-only check
-- cannot regress to the old `dealForceClose = false` behavior.
local oldGlobalMetatable = getmetatable(_G)
setmetatable(_G, nil)
local autoCloseConfig = CivvisControlConfig
autoCloseConfig.AnnouncementSeconds = 0.05
autoCloseConfig.DialogueSeconds = 0.05
autoCloseConfig.RunTag = "deal-session-autoclose-test"

local hidden = false
local update = nil
local sessionListener = nil
local closeCalls = 0
ContextPtr = {
	GetID = function() return "DiplomacyActionView" end,
	IsHidden = function() return hidden end,
	SetHide = function(_, value) hidden = value end,
	SetUpdate = function(_, callback) update = callback end,
}
Controls = {
	BlackFadeAnim = { IsStopped = function() return false end },
}
CloseFocusedState = function()
	closeCalls = closeCalls + 1
	hidden = true
end
LuaEvents = {
	CivvisDealSession = {
		Add = function(callback) sessionListener = callback end,
	},
}
Automation = { Log = function() end }
include = function() end

local autoCloseChunk, autoCloseErr = loadfile(here .. "/CivvisControlAutoClose.lua")
assert(autoCloseChunk, "could not load the autoclose shim: " .. tostring(autoCloseErr))
local autoCloseRan, autoCloseRuntimeErr = pcall(autoCloseChunk)
assert(autoCloseRan, "CivvisControlAutoClose.lua raised in deal-session test: " .. tostring(autoCloseRuntimeErr))
assert(type(sessionListener) == "function", "the autoclose shim did not register the deal listener")
assert(type(update) == "function", "the autoclose shim did not register an update callback")

-- The opening hold is still honored while the session is live.
sessionListener(4, true, 4)
update(0.05)
check("a live deal session does not close during its hold", closeCalls, 0)

-- The ask re-arms the hold with the answer window: a slow answer is not
-- closed out from under it when the opening hold's four seconds run out.
sessionListener(4, true, 30)
update(4.5)
check("an asked deal outlives the opening hold", closeCalls, 0)

-- The host closes it early. With the old listener this next update waits on
-- the never-stopping fade and leaves `closeCalls` at zero.
sessionListener(4, false, 0)
update(0.05)
check("an early host close enters the native close ladder", closeCalls, 1)
check("the early host close hides the action view", hidden, true)

-- At a 2x debug timescale the frame deltas run twice as fast, and the answer
-- hold still waits 30 REAL seconds: the closer divides each delta by the
-- scale the agent shares, and says which scale it read.
update(0.05)  -- a hidden frame resets the closer for the next show
ExposedMembers = { CivvisTimeScale = 2, CivvisClockAck = { scale = {}, at = {} } }
hidden = false
local closesAt2x = closeCalls
sessionListener(4, true, 30)
update(25); update(25)
check("at 2x, 50 frame seconds (25 real) keep the asked deal open", closeCalls, closesAt2x)
check("…and the closer acknowledges the scale it read",
	ExposedMembers.CivvisClockAck.scale.DiplomacyActionView, 2)
update(11)
update(0.05)
check("…31 real seconds end the hold", closeCalls, closesAt2x + 1)
-- A missing or nonsense scale reads as 1, and is acknowledged as 1, so the
-- agent sees the disagreement and reverts the timescale.
ExposedMembers = { CivvisTimeScale = "fast", CivvisClockAck = { scale = {}, at = {} } }
update(0.05)
check("a nonsense scale reads as 1", ExposedMembers.CivvisClockAck.scale.DiplomacyActionView, 1)
ExposedMembers = nil
update(0.05)
check("a missing shared table still ticks", type(update), "function")

-- A direct deal can also leave a live session on an otherwise blank action
-- view.  The ordinary close rung calls CloseSession, but Firaxis does not hide
-- the view as part of that API.  Exercise all ten native rungs: the first nine
-- are deliberately inert, while the session rung must prove it closed and
-- finish the same context without closing a replacement session.
local orphanHidden = false
local orphanUpdate = nil
local orphanCloseFocusedCalls = 0
local orphanCloseCalls = 0
local orphanSessionCloses = 0
local orphanSessionOpen = true
ContextPtr = {
	GetID = function() return "DiplomacyActionView" end,
	IsHidden = function() return orphanHidden end,
	SetHide = function(_, value) orphanHidden = value end,
	SetUpdate = function(_, callback) orphanUpdate = callback end,
}
Controls = {
	BlackFadeAnim = { IsStopped = function() return true end },
}
ms_ActiveSessionID = 77
ms_currentViewMode = -1
CloseFocusedState = function()
	orphanCloseFocusedCalls = orphanCloseFocusedCalls + 1
end
Close = function()
	orphanCloseCalls = orphanCloseCalls + 1
	orphanHidden = true
end
DiplomacyManager = {
	CloseSession = function(sessionID)
		check("the closer shuts only the captured session", sessionID, 77)
		orphanSessionCloses = orphanSessionCloses + 1
		orphanSessionOpen = false
	end,
	IsSessionIDOpen = function(sessionID)
		check("the closer verifies the captured session", sessionID, 77)
		return orphanSessionOpen
	end,
}
LuaEvents = {
	CivvisDealSession = {
		Add = function() end,
	},
}
local orphanChunk, orphanErr = loadfile(here .. "/CivvisControlAutoClose.lua")
assert(orphanChunk, "could not reload the autoclose shim: " .. tostring(orphanErr))
local orphanRan, orphanRuntimeErr = pcall(orphanChunk)
assert(orphanRan, "CivvisControlAutoClose.lua raised in orphan-session test: " .. tostring(orphanRuntimeErr))
assert(type(orphanUpdate) == "function", "the orphan-session closer has no update callback")
for rung = 1, 10 do
	orphanUpdate(rung == 1 and 0.05 or 30.01)
end
check("the first nine rungs leave the blank session visible", orphanCloseFocusedCalls, 6)
check("the verified session is closed once", orphanSessionCloses, 1)
check("a closed blank session finishes its action view", orphanCloseCalls, 1)
check("the session finalizer hides the action view", orphanHidden, true)

setmetatable(_G, oldGlobalMetatable)

if failures > 0 then
	realPrint(string.format("%d failure(s)", failures))
	os.exit(1)
end
realPrint("all deal session checks passed")
