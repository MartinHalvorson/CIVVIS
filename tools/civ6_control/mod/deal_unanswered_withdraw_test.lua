-- Offline regression for `UnansweredProposalWithdraw` (opt-in, off unless
-- set). An ask we sent whose session closed unanswered holds the end turn
-- (`deal_turn_hold`, PR #3930); when that hold releases on its own turn, the
-- end turn's next check withdraws the ask before the turn may end: REJECTED
-- unless the engine reports a pending deal, any session still open with that
-- rival closed, the outgoing working deal cleared, then a short settle hold.
-- All three such asks on 2026-11-20 04:00-08:20Z hung that rival's turn
-- (civvis-20261120T053354Z t130, 063234Z t121, 081130Z t94).
--
-- Pinned: off changes nothing (the turn ends at the hold's expiry and nothing
-- goes out after the question); on, an `expired` and a `rival_spoke` release
-- withdraw, the latter from the end turn's check and never inside the
-- rival's statement; an answered ask and an `expired` abandon are never
-- touched; a pending engine deal gets no deal action; a hold whose turn is
-- over and a rival asked again since are skipped; and `ai_phase_stall`
-- carries each active rival's deal state whether or not the arm is set.
--
-- The asks are made by the real sell arm, in a real `MAKE_DEAL` session, and
-- closed through the real session-closed handler and end-turn request.
--
-- Run: lua5.1 tools/civ6_control/mod/deal_unanswered_withdraw_test.lua

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
			or key == "CivvisOnDiplomacyStatement" or key == "CivvisOnDealSessionClosed"
			or key == "CivvisQueue" then
		return rawget(_G, key)
	end
	return stub()
end })

CivvisControlConfig = { TradeRetryTurns = 3, TradeResponseTurns = 2, PeaceRetryTurns = 5,
	DealSessions = true, DealSessionHoldSeconds = 4, DealSessionStandDown = 99,
	DealAnswerHoldSeconds = 30 }

local events = {}
local realPrint = print
print = function(line)
	if type(line) == "string" and (line:find('"kind":"deal_', 1, true)
			or line:find('"kind":"unanswered_withdraw"', 1, true)
			or line:find('"kind":"ai_phase_stall"', 1, true)) then
		events[#events + 1] = line
	end
end

LuaEvents = setmetatable({ CivvisDealSession = function() end },
	{ __index = function() return stub() end })

DealDirection = { OUTGOING = "outgoing", INCOMING = "incoming" }
DealItemTypes = { RESOURCES = "resources", FAVOR = "favor", GOLD = "gold",
	AGREEMENTS = "agreements", GREATWORK = "greatwork" }
DealAgreementTypes = { MAKE_PEACE = "make_peace", OPEN_BORDERS = "open_borders" }
DealProposalAction = {
	PENDING = "pending", PROPOSED = "proposed", ACCEPTED = "accepted", REJECTED = "rejected",
	ADJUSTED = "adjusted", EQUALIZE = "equalize", EQUALIZE_FAILED = "equalize_failed",
	CLOSED = "closed",
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
local TURN = 100
Game = {
	GetLocalPlayer = function() return 7 end,
	GetCurrentGameTurn = function() return TURN end,
}

-- The diplomacy manager's part, with what it still holds toward rival 3.
local engine, sessions
local function resetEngine()
	engine = { open = nil, initiator = nil, askedOpen = false, active = {} }
	sessions = { requested = {}, closed = {} }
end
resetEngine()
DiplomacyResponseTypes = { INITIAL = 0, ACKNOWLEDGE = 1 }
DiplomacyManager = {
	GetKeyName = function(key) return key end,
	FindOpenSessionID = function() return engine.open end,
	GetSessionInfo = function() return { FromPlayer = engine.initiator, ToPlayer = 7 } end,
	IsSessionIDOpen = function() return engine.askedOpen end,
	HasQueuedSession = function() return false end,
	RequestSession = function(pid, subject, kind)
		sessions.requested[#sessions.requested + 1] = { pid = pid, subject = subject, kind = kind }
	end,
	CloseSession = function(id) sessions.closed[#sessions.closed + 1] = id end,
}

-- The clock and the end turn: every `UI.RequestAction` is one turn ended.
local clock, endTurns = 1000.0, 0
UI = setmetatable({ GetElapsedTime = function() return clock end,
	HasSentTurnComplete = function() return false end,
	IsGameCoreBusy = function() return false end,
	IsProcessingMessages = function() return false end,
	RequestAction = function() endTurns = endTurns + 1 end },
	{ __index = function() return stub() end })
ActionTypes = { ACTION_ENDTURN = "end_turn" }

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtimeErr = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtimeErr))

local applyOrder = rawget(_G, "CivvisApplyOrder")
local onStatement = rawget(_G, "CivvisOnDiplomacyStatement")
local onClosed = rawget(_G, "CivvisOnDealSessionClosed")
local trade = rawget(_G, "CivvisTrade")
local queue = rawget(_G, "CivvisQueue")
assert(type(applyOrder) == "function" and type(onStatement) == "function"
	and type(onClosed) == "function" and type(trade) == "table" and type(queue) == "table",
	"CivvisControlAgent.lua did not export the session lane")
assert(type(trade.withdraw) == "function" and type(trade.dealState) == "function",
	"CivvisControlAgent.lua did not export the withdrawal")

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
-- A field of a nested object (`before`, `after`): `"before":{...}`.
local function nestedField(line, outer, key)
	if line == nil then return nil end
	local body = line:match('"' .. outer .. '":(%b{})')
	return eventField(body, key)
end
local function count(kind)
	local n = 0
	for _, line in ipairs(events) do
		if line:find('"kind":"' .. kind .. '"', 1, true) then n = n + 1 end
	end
	return n
end

-- The deal manager's part: one rival (3), our seat (7).
local function fixture(opts)
	opts = opts or {}
	local state = { sends = {}, pending = opts.pending == true, cleared = 0 }
	local outgoing = {}
	function outgoing:AddItemOfType(kind, owner)
		local item = { kind = kind, owner = owner }
		function item:SetValueType(v) self.valueType = v end
		function item:SetSubType(v) self.subType = v end
		function item:SetDuration(v) self.duration = v end
		function item:SetAmount(v) self.amount = v end
		function item:GetAmount() return self.amount end
		function item:GetMaxAmount() return 999 end
		function item:IsValid() return true end
		function item:GetID() return 1 end
		return item
	end
	function outgoing:Validate() end
	function outgoing:IsValid() return true end
	function outgoing:RemoveItemByID() end
	function outgoing:GetItemCount() return state.cleared > 0 and 0 or 1 end
	local incoming = {}
	function incoming:Items() return function() return nil end end
	function incoming:GetItemCount() return opts.incoming or 0 end
	DealManager = {
		HasPendingDeal = function() return state.pending end,
		ClearWorkingDeal = function(direction)
			state.cleared = state.cleared + 1
			state.clearedDirection = direction
		end,
		GetWorkingDeal = function(direction)
			if direction == DealDirection.INCOMING then return incoming end
			return outgoing
		end,
		GetPossibleDealItems = function(_, _, kind)
			if kind == DealItemTypes.RESOURCES then
				return { { ForType = 12, MaxAmount = 3, IsValid = true } }
			end
			return {}
		end,
		AreWorkingDealsEqual = function() return true end,
		SendWorkingDeal = function(action, pid, subject)
			state.sends[#state.sends + 1] = { action, pid, subject }
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
		GetTreasury = function() return { GetGoldBalance = function() return 400 end } end,
	}
	Players = setmetatable({ [7] = player }, {
		__index = function(_, id)
			return { IsMajor = function() return true end,
				IsTurnActive = function() return engine.active[id] == true end }
		end,
	})
	return state, player
end

local function onFlag(on) CivvisControlConfig.UnansweredProposalWithdraw = on end
local function reset()
	trade.pending, trade.asked, trade.sessions = {}, {}, {}
	trade.unanswered, trade.disabled, trade.turnHold = 0, false, nil
	trade.withdrawDue, trade.withdrawSettle = nil, nil
	queue.endTurnWait = nil
	resetEngine()
	events = {}
	TURN = TURN + 10
	clock = clock + 100
end
-- A sale asked by the real arm, its session opened and the question put;
-- `closed` then has the engine close that session with no answer.
local function askedSale(opts)
	reset()
	local state, player = fixture(opts)
	state.ordered = applyOrder(player, 7,
		{ kind = "sell", subject = "3", verb = "RESOURCE_DYES=1", x = 60 }, TURN)
	onStatement(7, 3, { StatementType = "MAKE_DEAL", SessionID = 960 })
	-- The arm clears the working deal before it builds the ask; count from here.
	state.cleared = 0
	return state
end
local function closedUnanswered(opts)
	local state = askedSale(opts)
	onClosed(960)
	return state
end
local function endTurn() return queue.requestEndTurn(TURN) end

-- ─── off (the default): the turn ends at the hold's expiry, nothing sent ───
onFlag(nil)
local off = closedUnanswered()
check("the real sell arm asks", off.ordered, true)
check("the question is the only send", #off.sends, 1)
check("off: the unanswered ask holds the turn",
	eventField(lastEvent("deal_turn_hold"), "phase"), "held")
local endsBefore = endTurns
endTurn()
check("off: inside the window the turn is held", endTurns, endsBefore)
clock = clock + 31
endTurn()
check("off: the hold expires", eventField(lastEvent("deal_turn_hold"), "why"), "expired")
check("off: the turn ends at the expiry, as before", endTurns, endsBefore + 1)
check("off: nothing is sent after the question", #off.sends, 1)
check("off: no session is closed", #sessions.closed, 0)
check("off: the working deal is not cleared", off.cleared, 0)
check("off: no withdrawal is recorded", count("unanswered_withdraw"), 0)
check("off: nothing is queued", trade.withdrawDue, nil)

onFlag(nil)
local offSpoke = closedUnanswered()
endsBefore = endTurns
onStatement(3, 7, { StatementType = "MAKE_DEAL", SessionID = 961 })
check("off: the rival speaking releases the hold",
	eventField(lastEvent("deal_turn_hold"), "why"), "rival_spoke")
clock = clock + 1
endTurn()
check("off: …and the turn ends at once", endTurns, endsBefore + 1)
check("off: …with nothing sent", #offSpoke.sends, 1)
check("off: …and no withdrawal", count("unanswered_withdraw"), 0)

onFlag(false)
local offFalse = closedUnanswered()
endsBefore = endTurns
clock = clock + 31
endTurn()
check("an explicit false is off too", endTurns, endsBefore + 1)
check("…sending nothing", #offFalse.sends, 1)

-- ─── on: the `expired` release withdraws before the turn ends ──────────────
onFlag(true)
local expired = closedUnanswered()
endsBefore = endTurns
endTurn()
check("on: inside the window the turn is still held", endTurns, endsBefore)
check("on: nothing is withdrawn while it holds", #expired.sends, 1)
clock = clock + 31
endTurn()
check("on: the hold expires", eventField(lastEvent("deal_turn_hold"), "why"), "expired")
check("on: the withdrawal goes out first: REJECTED", expired.sends[2] and expired.sends[2][1], "rejected")
check("on: …from our seat to the rival",
	expired.sends[2] and (expired.sends[2][2] .. ":" .. expired.sends[2][3]), "7:3")
check("on: …once", #expired.sends, 2)
check("on: the outgoing working deal is cleared", expired.clearedDirection, "outgoing")
check("on: no session was open, none is closed", #sessions.closed, 0)
local row = lastEvent("unanswered_withdraw")
check("on: one withdrawal row", count("unanswered_withdraw"), 1)
check("on: …withdrawn", eventField(row, "phase"), "withdrawn")
check("on: …released by the expiry", eventField(row, "released"), "expired")
check("on: …naming the rival", eventField(row, "target"), "3")
check("on: …the ask's kind", eventField(row, "ask"), "sell")
check("on: …the ask's action", eventField(row, "action"), "EQUALIZE")
check("on: …the session it went out in", eventField(row, "session"), "960")
check("on: …the calls made", eventField(row, "calls"), "rejected,cleared")
check("on: …what was pending before", nestedField(row, "before", "pending"), "false")
check("on: …our working deal before", nestedField(row, "before", "outgoing"), "1")
check("on: …and after", nestedField(row, "after", "outgoing"), "0")
check("on: …the settle window", eventField(row, "settle"), "3")
check("on: the turn does not end in the withdrawal's own check", endTurns, endsBefore)
clock = clock + 1
endTurn()
check("on: the settle window holds the turn", endTurns, endsBefore)
clock = clock + 2.5
endTurn()
check("on: after the settle window the turn ends", endTurns, endsBefore + 1)
check("on: nothing more is sent", #expired.sends, 2)
check("on: the withdrawal is not repeated", count("unanswered_withdraw"), 1)

-- A session still open with the rival (its own, as in a verdict session) is
-- refused and closed as the shipped forced close does; the session the ask
-- went out in is closed too if the engine still has it open.
onFlag(true)
local openOne = closedUnanswered()
engine.open, engine.initiator, engine.askedOpen = 970, 3, true
clock = clock + 31
endTurn()
row = lastEvent("unanswered_withdraw")
check("on: an open rival session is REJECTED", openOne.sends[2] and openOne.sends[2][1], "rejected")
check("on: …and closed", sessions.closed[1], 970)
check("on: the asked session still open is closed too", sessions.closed[2], 960)
check("on: …the calls", eventField(row, "calls"), "rejected,closed_session,closed_asked,cleared")
check("on: …the open session's opener", nestedField(row, "before", "initiator"), "3")
check("on: …the asked session was open", nestedField(row, "before", "asked_open"), "true")

-- ─── on: `rival_spoke` withdraws from the end turn, not the statement ──────
onFlag(true)
local spoke = closedUnanswered()
endsBefore = endTurns
onStatement(3, 7, { StatementType = "MAKE_DEAL", SessionID = 961 })
check("on: the rival speaking releases the hold",
	eventField(lastEvent("deal_turn_hold"), "why"), "rival_spoke")
check("on: nothing is sent inside the rival's statement", #spoke.sends, 1)
check("on: …no withdrawal yet", count("unanswered_withdraw"), 0)
clock = clock + 1
endTurn()
check("on: the end turn's check withdraws", spoke.sends[2] and spoke.sends[2][1], "rejected")
check("on: …released by the rival", eventField(lastEvent("unanswered_withdraw"), "released"),
	"rival_spoke")
check("on: …and the settle window holds that check", endTurns, endsBefore)
clock = clock + 3.5
endTurn()
check("on: then the turn ends", endTurns, endsBefore + 1)

-- ─── never: an answered ask ───────────────────────────────────────────────
onFlag(true)
local answered = askedSale()
endsBefore = endTurns
onStatement(3, 7, { StatementType = "MAKE_DEAL", SessionID = 960,
	DealAction = DealProposalAction.REJECTED })
check("an answered ask is closed by us", sessions.closed[1], 960)
check("…as answered", eventField(lastEvent("deal_session"), "why"), "answered")
onClosed(960)
check("…its late close holds no turn", trade.turnHold, nil)
clock = clock + 1
endTurn()
check("…the turn ends at once", endTurns, endsBefore + 1)
check("…nothing but the question was sent", #answered.sends, 1)
check("…no withdrawal", count("unanswered_withdraw"), 0)

-- ─── never: the `expired` abandon (nothing held, nothing withdrawn) ───────
onFlag(true)
local stale = askedSale()
endsBefore = endTurns
trade.abandon(3, "expired")
check("an expired abandon is unanswered", eventField(lastEvent("deal_session"), "why"), "expired")
check("…holds no turn", trade.turnHold, nil)
check("…queues nothing", trade.withdrawDue, nil)
clock = clock + 1
endTurn()
check("…the turn ends at once", endTurns, endsBefore + 1)
check("…nothing is sent after the question", #stale.sends, 1)
check("…no withdrawal", count("unanswered_withdraw"), 0)
-- And through the real arm: an ask never answered, dropped at the next sale
-- two turns on (`TradeResponseTurns`), the live `expired` shape (sent = false).
onFlag(true)
reset()
local unsent, unsentPlayer = fixture()
applyOrder(unsentPlayer, 7, { kind = "sell", subject = "3", verb = "RESOURCE_DYES=1", x = 60 }, TURN)
TURN = TURN + 2
applyOrder(unsentPlayer, 7, { kind = "sell", subject = "3", verb = "RESOURCE_DYES=1", x = 60 }, TURN)
check("the arm's own expiry is unanswered `expired`",
	eventField(lastEvent("deal_expired"), "target"), "3")
check("…with no question sent", eventField(lastEvent("deal_session"), "sent"), "false")
check("…why expired", eventField(lastEvent("deal_session"), "why"), "expired")
check("…holds no turn", trade.turnHold, nil)
check("…queues no withdrawal", trade.withdrawDue, nil)
check("…sends nothing", #unsent.sends, 0)

-- ─── on, but the engine still holds a pending deal: no deal action ────────
onFlag(true)
local pending = closedUnanswered()
pending.pending = true
clock = clock + 31
endTurn()
row = lastEvent("unanswered_withdraw")
check("a pending engine deal gets no deal action", #pending.sends, 1)
check("…and no working-deal clear", pending.cleared, 0)
check("…the record says so", eventField(row, "calls"), "pending_left")
check("…with what was pending", nestedField(row, "before", "pending"), "true")

-- ─── skipped: the hold's turn is over, or the rival was asked again ───────
onFlag(true)
local over = closedUnanswered()
TURN = TURN + 1
clock = clock + 1
endTurn()
check("a hold from an ended turn is released as over",
	eventField(lastEvent("deal_turn_hold"), "why"), "turn_over")
check("…and skipped", eventField(lastEvent("unanswered_withdraw"), "skipped"), "turn_over")
check("…sending nothing", #over.sends, 1)

onFlag(true)
local again = closedUnanswered()
trade.sessions[3] = { kind = "sell", action = "EQUALIZE", turn = TURN, sent = false }
trade.withdrawLater(trade.turnHold, "expired")
trade.turnHold = nil
check("a rival asked again since is skipped", trade.withdrawUnanswered(TURN), false)
check("…naming why", eventField(lastEvent("unanswered_withdraw"), "skipped"), "new_session")
check("…sending nothing", #again.sends, 1)

-- A second rival's unanswered ask replaces the first's hold: the first is
-- still withdrawn, with the second, once the second's hold releases.
onFlag(true)
local first = closedUnanswered()
trade.sessions[4] = { kind = "peace", action = "PROPOSED", turn = TURN, sent = true, sessionID = 980 }
trade.abandon(4, "session_closed")
check("a second rival's hold replaces the first", trade.turnHold and trade.turnHold.target, 4)
check("…the first is queued", trade.withdrawDue and trade.withdrawDue[1]
	and trade.withdrawDue[1].released, "replaced")
endsBefore = endTurns
endTurn()
check("…nothing is withdrawn while the second holds", #first.sends, 1)
check("…nor does the turn end", endTurns, endsBefore)
clock = clock + 31
endTurn()
check("…at its expiry the first is withdrawn", first.sends[2] and first.sends[2][3], 3)
check("…and the second", first.sends[3] and first.sends[3][3], 4)
check("…two rows", count("unanswered_withdraw"), 2)
check("…the second released by its expiry",
	eventField(lastEvent("unanswered_withdraw"), "released"), "expired")
check("…the turn still waits out the settle window", endTurns, endsBefore)

-- ─── the stall record carries each active rival's deal state ──────────────
for _, on in ipairs({ false, true }) do
	onFlag(on)
	reset()
	local stalled = fixture({ pending = on, incoming = 2 })
	engine.active[3] = true
	engine.open, engine.initiator = 990, 3
	PlayerManager = { GetAliveIDs = function() return { 3, 7 } end,
		GetAliveMajorIDs = function() return { 3, 7 } end }
	queue.endTurnWait = { turn = TURN, emitted = true, ended_at = clock - 40 }
	check("a stall is reported (arm " .. tostring(on) .. ")", queue.checkAiPhaseStall(), true)
	local stall = lastEvent("ai_phase_stall")
	local state = stall and stall:match('"deal_state":%[(%b{})')
	check("…naming the active rival", eventField(state, "with"), "3")
	check("…its pending deal", eventField(state, "pending"), tostring(on))
	check("…the session open with it", eventField(state, "session"), "990")
	check("…who opened it", eventField(state, "initiator"), "3")
	check("…the rival's working deal", eventField(state, "incoming"), "2")
	check("…ours", eventField(state, "outgoing"), "1")
	check("…and sends nothing", #stalled.sends, 0)
end

realPrint(string.format("%d failure(s)", failures))
if failures > 0 then os.exit(1) end
