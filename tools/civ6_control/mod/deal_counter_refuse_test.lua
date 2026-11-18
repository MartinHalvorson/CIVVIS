-- Offline regression for `DealCounterRefuse` (opt-in): when a refused (REJECTED)
-- peace or alliance ask's counter hold runs out with the refusal left on the
-- deal screen, the agent declines it with REJECTED and lets the game close the
-- session, instead of the closer's bare close. Live 2026-11-15..16: of 68 such
-- asks closed `counter_not_sent`, the 59 that did not hang all showed the
-- refusal as conversation (`MAKE_DEAL, AI_REFUSE_DEAL`, answered CHOICE_EXIT),
-- and all 9 that hung had no dialogue and the deal screen up. This pins:
--   * the default and the 59 dialogue-path cases are untouched (nothing is
--     sent, the hold is not re-armed, the close is `counter_not_sent`);
--   * ADJUSTED answers, trade sessions and a conversation panel that is
--     showing are untouched;
--   * the target case sends REJECTED once, re-arms the hold before the send,
--     and closes `rejected_working_deal` with `closer` game or ladder;
--   * a counter processed after our REJECTED is refused (`after_refusal`),
--     and the echo of our own refusal is not taken for it;
--   * the closer (CivvisControlAutoClose.lua) hands an expired hold to the
--     agent only under the switch, and keeps holding when the agent re-arms.
-- Agent harness copied from deal_ask_on_tick_test.lua.
--
-- Run: lua5.1 tools/civ6_control/mod/deal_counter_refuse_test.lua

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
	VictoryLane = "diplomatic" }
local cfg = CivvisControlConfig

local events = {}
local realPrint = print
print = function(line)
	if type(line) == "string" and (line:find('"kind":"deal_', 1, true)
			or line:find('"kind":"alliance_', 1, true)
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
DealAgreementTypes = { MAKE_PEACE = "make_peace", OPEN_BORDERS = "open_borders", ALLIANCE = "alliance" }
DealProposalAction = {
	PENDING = "pending", PROPOSED = "proposed", ACCEPTED = "accepted", REJECTED = "rejected",
	ADJUSTED = "adjusted", EQUALIZE = "equalize", EQUALIZE_FAILED = "equalize_failed",
}
GameInfo = {
	Resources = setmetatable({}, { __index = function() return nil end }),
	Resource_Consumption = {},
	GreatWorks = {},
	DiplomaticStates = {
		[0] = { StateType = "DIPLO_STATE_FRIENDLY" },
		[1] = { StateType = "DIPLO_STATE_DECLARED_FRIEND" },
		[2] = { StateType = "DIPLO_STATE_UNFRIENDLY" },
	},
}
DB = { MakeHash = function(name) return name end }
local TURN = 50
Game = {
	GetLocalPlayer = function() return 7 end,
	GetCurrentGameTurn = function() return TURN end,
}

local sessions = { closed = {}, open = {} }
DiplomacyResponseTypes = { INITIAL = 0, ACKNOWLEDGE = 1 }
DiplomacyManager = {
	GetKeyName = function(key) return key end,
	FindOpenSessionID = function() return nil end,
	HasQueuedSession = function() return true end,
	RequestSession = function() end,
	CloseSession = function(id) sessions.closed[#sessions.closed + 1] = id end,
	IsSessionIDOpen = function(id) return sessions.open[id] ~= false end,
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
local function phases(phase)
	local n = 0
	for _, line in ipairs(events) do
		if line:find('"phase":"' .. phase .. '"', 1, true) then n = n + 1 end
	end
	return n
end

-- One rival's deal manager (`opts.onSend` sees every SendWorkingDeal).
local function fixture(opts)
	opts = opts or {}
	local state = { sends = {}, atWar = opts.atWar or false }
	local outgoing = {}
	function outgoing:AddItemOfType(kind, owner)
		local item = { kind = kind, owner = owner }
		function item:SetValueType(v) self.valueType = v end
		function item:SetSubType(v) self.subType = v end
		function item:GetSubType() return self.subType end
		function item:SetDuration(v) self.duration = v end
		function item:SetAmount(v) self.amount = v end
		function item:SetLocked(v) self.locked = v end
		function item:GetMaxAmount() return 999 end
		function item:IsValid() return true end
		function item:GetID() return 1 end
		return item
	end
	function outgoing:Validate() end
	function outgoing:IsValid() return true end
	function outgoing:RemoveItemByID() end
	DealManager = {
		HasPendingDeal = function() return false end,
		ClearWorkingDeal = function() end,
		GetWorkingDeal = function() return outgoing end,
		GetPossibleDealItems = function() return {} end,
		CopyIncomingToOutgoingWorkingDeal = function() end,
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
				IsAtWarWith = function() return state.atWar end,
				HasOpenBordersFrom = function() return false end,
				GetAllianceType = function() return -1 end,
				IsDiplomaticActionValid = function() return true end,
			}
		end,
		GetTreasury = function() return { GetGoldBalance = function() return 400 end } end,
		GetCulture = function() return {
			GetTouristsTo = function() return 0 end,
			GetStaycationers = function() return 0 end,
		} end,
	}
	Players = setmetatable({ [7] = player }, {
		__index = function() return {
			IsMajor = function() return true end,
			GetDiplomaticAI = function() return {
				GetDiplomaticStateIndex = function() return 1 end,
			} end,
			GetCulture = function() return {
				GetTouristsTo = function() return 0 end,
				GetStaycationers = function() return 0 end,
			} end,
		} end,
	})
	return state, player
end

-- An alliance (or, with `peace`, a peace) ask to `subject` answered `verdict`.
local function refused(subject, sessionID, verdict, peace, opts)
	TURN = TURN + 1
	local state, player = fixture(opts or { atWar = peace })
	if peace then
		applyOrder(player, 7, { kind = "peace", subject = tostring(subject) }, TURN)
	else
		applyOrder(player, 7, { kind = "alliance", subject = tostring(subject),
			verb = "ALLIANCE_RESEARCH" }, TURN)
	end
	onStatement(7, subject, { StatementType = "MAKE_DEAL", SessionID = sessionID })
	check("ask " .. subject .. " is put", state.sends[1] and state.sends[1].action, "proposed")
	onStatement(subject, 7, { StatementType = "MAKE_DEAL", SessionID = sessionID,
		StatementSubType = "AI_REFUSE_DEAL", DealAction = verdict or DealProposalAction.REJECTED })
	check("ask " .. subject .. " waits for its counter",
		eventField(lastEvent("deal_session"), "phase"), "counter_wait")
	return state
end
local expired = function(screen, subject, dealUp)
	return listeners.CivvisDealHoldExpired(screen, subject, dealUp)
end

check("the closer's hold expiry reaches the agent", type(listeners.CivvisDealHoldExpired), "function")
check("the closer's dialogue reaches the agent", type(listeners.CivvisDealDialogue), "function")

-- ── Off (the default): an expired hold changes nothing ──
local off = refused(3, 900)
local holdsBefore = #holds
expired("DiplomacyDealView", 3, true)
expired("DiplomacyActionView", 3, true)
check("off: nothing more is sent", #off.sends, 1)
check("off: the hold is not re-armed", #holds, holdsBefore)
trade.settleRefused()
check("off: the backstop does nothing", trade.sessions[3] ~= nil, true)
onClosed(900)
check("off: the close is counter_not_sent", eventField(lastEvent("deal_session"), "why"), "counter_not_sent")

cfg.DealCounterRefuse = true

-- ── On, the dialogue path (the 59 that did not hang): untouched ──
local talk = refused(4, 901)
listeners.CivvisDealDialogue(4, "MAKE_DEAL", "AI_REFUSE_DEAL")
holdsBefore = #holds
expired("DiplomacyActionView", 4, false)
expired("DiplomacyDealView", 4, true)
check("dialogue: nothing more is sent", #talk.sends, 1)
check("dialogue: the hold is not re-armed", #holds, holdsBefore)
check("dialogue: no refusal in the ledger", phases("refusing"), 0)
onClosed(901)
check("dialogue: the close is counter_not_sent", eventField(lastEvent("deal_session"), "why"), "counter_not_sent")
-- The dialogue can land before our handler reads the verdict.
TURN = TURN + 1
local early, earlyPlayer = fixture()
applyOrder(earlyPlayer, 7, { kind = "alliance", subject = "10", verb = "ALLIANCE_RESEARCH" }, TURN)
onStatement(7, 10, { StatementType = "MAKE_DEAL", SessionID = 902 })
listeners.CivvisDealDialogue(10, "MAKE_DEAL", "AI_REFUSE_DEAL")
onStatement(10, 7, { StatementType = "MAKE_DEAL", SessionID = 902,
	StatementSubType = "AI_REFUSE_DEAL", DealAction = DealProposalAction.REJECTED })
expired("DiplomacyDealView", 10, true)
check("dialogue first: nothing more is sent", #early.sends, 1)
onClosed(902)
check("dialogue first: counter_not_sent", eventField(lastEvent("deal_session"), "why"), "counter_not_sent")
-- The opening's own conversation statement is not a refusal dialogue.
local opening = refused(11, 903)
listeners.CivvisDealDialogue(11, "MAKE_DEAL", "NONE")
holdsBefore = #holds
-- ...but a conversation panel that is showing is not the deal screen.
expired("DiplomacyActionView", 11, false)
check("conversation showing: nothing more is sent", #opening.sends, 1)
check("conversation showing: the hold is not re-armed", #holds, holdsBefore)
onClosed(903)
check("conversation showing: counter_not_sent", eventField(lastEvent("deal_session"), "why"), "counter_not_sent")

-- ── On, an ADJUSTED answer: untouched ──
local adjusted = refused(12, 904, DealProposalAction.ADJUSTED)
expired("DiplomacyDealView", 12, true)
check("adjusted: nothing more is sent", #adjusted.sends, 1)
onClosed(904)
check("adjusted: counter_not_sent", eventField(lastEvent("deal_session"), "why"), "counter_not_sent")

-- ── On, a trade session in a counter wait: untouched ──
trade.sessions[13] = { kind = "sell", sent = true, sessionID = 905,
	counter_wait = { turn = TURN, refusal = DealProposalAction.REJECTED } }
local sendsBefore = 0
DealManager.SendWorkingDeal = function() sendsBefore = sendsBefore + 1 end
expired("DiplomacyDealView", 13, true)
check("trade: nothing is sent", sendsBefore, 0)
trade.close(7, 13, "test_trade", true)

-- ── On, the hang shape: REJECTED, no dialogue, deal screen up ──
local left = refused(5, 906)
holdsBefore = #holds
expired("DiplomacyDealView", 5, true)
check("left: REJECTED is sent", left.sends[2] and left.sends[2].action, "rejected")
check("left: ...to that rival", left.sends[2] and left.sends[2].subject, 5)
check("left: ...once", #left.sends, 2)
check("left: the hold is re-armed before the send", left.sends[2] and left.sends[2].holds, holdsBefore + 1)
check("left: ...for the grace", holds[holdsBefore + 1].seconds, 2)
check("left: ...as open", holds[holdsBefore + 1].open, true)
check("left: the ledger says refusing", eventField(lastEvent("deal_session"), "phase"), "refusing")
check("left: ...from the deal view", eventField(lastEvent("deal_session"), "screen"), "DiplomacyDealView")
check("left: the session stays registered for its counter", trade.sessions[5] ~= nil, true)
local closedBefore = #sessions.closed
onClosed(906)
check("left: the game's close is rejected_working_deal",
	eventField(lastEvent("deal_session"), "why"), "rejected_working_deal")
check("left: ...closed by the game", eventField(lastEvent("deal_session"), "closer"), "game")
check("left: no second CloseSession of ours", #sessions.closed, closedBefore)
check("left: the hold is released", holds[#holds].open, false)
check("left: the session is forgotten", trade.sessions[5], nil)
check("left: it is not an unanswered ask", trade.turnHold, nil)

-- The action view's expiry acts the same when its conversation panel is hidden
-- or unreadable, and peace is handled like an alliance.
local peace = refused(14, 907, nil, true)
expired("DiplomacyActionView", 14, nil)
check("peace, unreadable panel: REJECTED is sent", peace.sends[2] and peace.sends[2].action, "rejected")
onClosed(907)
check("peace: rejected_working_deal", eventField(lastEvent("deal_session"), "why"), "rejected_working_deal")

-- ── A counter processed after our REJECTED is refused like any other ──
local late = refused(6, 908)
expired("DiplomacyDealView", 6, true)
onStatement(6, 7, { StatementType = "MAKE_DEAL", SessionID = 908,
	StatementSubType = "HUMAN_REFUSE_DEAL" })
check("echo: our own refusal echoed is not the counter", #late.sends, 2)
check("echo: ...and is in the ledger", eventField(lastEvent("counter_wait"), "reason"), "refusal_echo")
check("echo: the session still waits", trade.sessions[6] ~= nil, true)
onStatement(6, 7, { StatementType = "MAKE_DEAL", SessionID = 908, StatementSubType = "POSITIVE" })
check("late counter: refused", late.sends[3] and late.sends[3].action, "rejected")
check("late counter: counter_answered after the refusal",
	eventField(lastEvent("counter_answered"), "after_refusal"), "true")
check("late counter: closed by us", sessions.closed[#sessions.closed], 908)
check("late counter: the close says counter_answered",
	eventField(lastEvent("deal_session"), "why"), "counter_answered")

-- ── The grace runs out: the ordinary closer takes it ──
local slow = refused(8, 909)
expired("DiplomacyDealView", 8, true)
holdsBefore = #holds
expired("DiplomacyDealView", 8, true)
expired("DiplomacyActionView", 8, true)
check("grace out: nothing more is sent", #slow.sends, 2)
check("grace out: the hold is not re-armed again", #holds, holdsBefore)
onClosed(909)
check("grace out: rejected_working_deal", eventField(lastEvent("deal_session"), "why"), "rejected_working_deal")
check("grace out: ...closed by the closer", eventField(lastEvent("deal_session"), "closer"), "ladder")

-- ── The send throws: the screen goes straight back to the closer ──
local broken = refused(9, 910, nil, false, { onSend = function(action)
	if action == DealProposalAction.REJECTED then error("no deal manager") end
end })
expired("DiplomacyDealView", 9, true)
check("throw: the ledger says refuse_failed", eventField(lastEvent("deal_session"), "phase"), "refuse_failed")
check("throw: the hold is released to the closer", holds[#holds].seconds, 0)
holdsBefore = #holds
expired("DiplomacyDealView", 9, true)
check("throw: it is not tried again", #holds, holdsBefore)
onClosed(910)
check("throw: the close is counter_not_sent", eventField(lastEvent("deal_session"), "why"), "counter_not_sent")

-- ── A close that never reaches us is settled from the tick ──
refused(15, 911)
expired("DiplomacyDealView", 15, true)
trade.settleRefused()
check("backstop: an open session is left alone", trade.sessions[15] ~= nil, true)
sessions.open[911] = false
trade.settleRefused()
check("backstop: a closed one is settled", trade.sessions[15], nil)
check("backstop: rejected_working_deal", eventField(lastEvent("deal_session"), "why"), "rejected_working_deal")

-- ── Wiring: both agent pumps run the backstop ──
local src = assert(io.open(here .. "/CivvisControlAgent.lua")):read("*a")
local tickAt = assert(src:find("local function onGameCoreTick()", 1, true))
local tickEnd = assert(src:find("\nend\n", tickAt, true))
check("the game-core tick runs the backstop",
	src:sub(tickAt, tickEnd):find("CivvisK.guard(CivvisTrade.settleRefused);", 1, true) ~= nil, true)
local pulseAt = assert(src:find("CivvisQueue.onUiPulse = function(source)", 1, true))
local pulseEnd = assert(src:find("\nend;\n", pulseAt, true))
check("the UI pulse runs it too",
	src:sub(pulseAt, pulseEnd):find("CivvisK.guard(CivvisTrade.settleRefused);", 1, true) ~= nil, true)

-- ── The closer: an expired hold goes to the agent only under the switch ──
-- Load the real CivvisControlAutoClose.lua as one leader-screen context, with
-- LuaEvents dispatched synchronously the way the game does.
local function closerContext(name, refuse, agentReply)
	local bus, calls = {}, { refuseDeal = 0, expired = {}, dialogue = {} }
	local env
	local Events = setmetatable({}, { __index = function(t, event)
		local entry = setmetatable({ Add = function(fn)
			bus[event] = bus[event] or {}
			table.insert(bus[event], fn)
		end, Remove = function() end }, { __call = function(_, ...)
			if event == "CivvisDealHoldExpired" then
				calls.expired[#calls.expired + 1] = { ... }
				if agentReply then agentReply(env, ...) end
			elseif event == "CivvisDealDialogue" then
				calls.dialogue[#calls.dialogue + 1] = { ... }
			end
			for _, fn in ipairs(bus[event] or {}) do fn(...) end
		end })
		rawset(t, event, entry)
		return entry
	end })
	local hidden, conversationHidden = false, true
	local tick
	env = {
		CivvisControlConfig = { DealCounterRefuse = refuse, CivvisDecides = false },
		ContextPtr = {
			GetID = function() return name end,
			IsHidden = function() return hidden end,
			SetHide = function(_, value) hidden = value end,
			SetUpdate = function(_, fn) tick = fn end,
			SetShowHandler = function() end,
		},
		Controls = { ConversationContainer = { IsHidden = function() return conversationHidden end } },
		LuaEvents = Events,
		Events = setmetatable({}, { __index = function() return { Add = function() end } end }),
		ExposedMembers = {},
		Automation = { Log = function() end },
		Game = Game,
		include = function() end,
		OnRefuseDeal = function() calls.refuseDeal = calls.refuseDeal + 1; hidden = true end,
		pcall = pcall, type = type, tonumber = tonumber, tostring = tostring, string = string,
		math = math, table = table, pairs = pairs, ipairs = ipairs, setmetatable = setmetatable,
		select = select, unpack = unpack, error = error, next = next, os = os,
	}
	local closerChunk = assert(loadfile(here .. "/CivvisControlAutoClose.lua"))
	setfenv(closerChunk, env)
	closerChunk()
	assert(type(tick) == "function", name .. ": the closer did not arm")
	return {
		calls = calls, tick = function(dt) tick(dt) end,
		hold = function(subject, seconds) Events.CivvisDealSession(subject, true, seconds) end,
		setConversation = function(v) conversationHidden = v end,
		up = function() return not hidden end,
	}
end
local function runFor(ctx, seconds)
	for _ = 1, math.floor(seconds / 0.1 + 0.5) do ctx.tick(0.1) end
end

local plain = closerContext("DiplomacyDealView", nil, nil)
plain.hold(5, 3)
runFor(plain, 3.5)
check("closer off: the expiry is not handed over", #plain.calls.expired, 0)
check("closer off: the deal view is refused as before", plain.calls.refuseDeal, 1)

-- The agent re-arms: the closer keeps holding, then closes when it declines.
local reArmed = 0
local held = closerContext("DiplomacyDealView", true, function(env, screen, subject, dealUp)
	if reArmed == 0 then
		reArmed = 1
		env.LuaEvents.CivvisDealSession(subject, true, 2)
	end
end)
held.hold(5, 3)
runFor(held, 3.05)
check("closer on: the expiry is handed to the agent", #held.calls.expired, 1)
check("closer on: ...naming the screen", held.calls.expired[1][1], "DiplomacyDealView")
check("closer on: ...the session's rival", held.calls.expired[1][2], 5)
check("closer on: ...and the deal screen up", held.calls.expired[1][3], true)
check("closer on: a re-armed hold is not closed", held.calls.refuseDeal, 0)
runFor(held, 1.5)
check("closer on: still held inside the grace", held.calls.refuseDeal, 0)
runFor(held, 1)
check("closer on: the grace's expiry is handed over too", #held.calls.expired, 2)
check("closer on: then the ordinary close runs", held.calls.refuseDeal, 1)

-- Not re-armed (the dialogue path): the close runs on the same expiry.
local declined = closerContext("DiplomacyDealView", true, function() end)
declined.hold(4, 3)
runFor(declined, 3.5)
check("closer on, agent declines: closed at the expiry", declined.calls.refuseDeal, 1)

-- The action view reports its conversation panel as the deal screen's state.
local action = closerContext("DiplomacyActionView", true, function() end)
action.hold(4, 3)
action.setConversation(false)
runFor(action, 3.05)
check("action view: the expiry is handed over", #action.calls.expired, 1)
check("action view: a showing conversation reads as no deal screen", action.calls.expired[1][3], false)
local actionDeal = closerContext("DiplomacyActionView", true, function() end)
actionDeal.hold(5, 3)
runFor(actionDeal, 3.05)
check("action view: a hidden conversation reads as the deal screen", actionDeal.calls.expired[1][3], true)

-- ── The closer's dialogue report: only under the switch ──
local src2 = assert(io.open(here .. "/CivvisControlAutoClose.lua")):read("*a")
local wrapAt = assert(src2:find("ApplyStatement = function(handler, statementType, subType, toPlayer, statement)", 1, true))
local observedAt = assert(src2:find("dialogueObserved = true;", wrapAt, true))
local rankAt = assert(src2:find("local rank = {", observedAt, true))
local block = src2:sub(observedAt, rankAt)
check("the dialogue report is gated on the switch",
	block:find("if cfg.DealCounterRefuse == true then", 1, true) ~= nil, true)
check("the dialogue report names the speaker, statement and subtype",
	block:find("LuaEvents.CivvisDealDialogue(statement.FromPlayer, statementType, subType);", 1, true) ~= nil, true)

if failures > 0 then
	realPrint(failures .. " failure(s)")
	os.exit(1)
end
realPrint("deal counter refuse: all ok")
