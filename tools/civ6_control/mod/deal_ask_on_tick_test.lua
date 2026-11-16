-- Offline regression for `DealAskOnTick` (opt-in): a peace or alliance
-- question is put on the next tick (`CivvisTrade.askDeferred`), not inside
-- the statement that says the session is live. Every lost counter after a
-- refused proposal (7 of 116 refusals, live 2026-11-12..16, each followed by
-- that rival's hung turn) was queued while that statement was still being
-- dispatched; Firaxis's screen proposes from a screen already up. This pins
-- the lane's side: nothing is sent from the opening dispatch, the tick sends
-- it exactly once, the verdict and counter are handled as before, trade asks
-- and the default are unchanged, and a session our end turn overtook is
-- closed unasked. Harness copied from alliance_ask_test.lua.
--
-- Run: lua5.1 tools/civ6_control/mod/deal_ask_on_tick_test.lua

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

local events = {}
local realPrint = print
print = function(line)
	if type(line) == "string" and (line:find('"kind":"deal_', 1, true)
			or line:find('"kind":"peace_', 1, true)
			or line:find('"kind":"orphan_', 1, true)
			or line:find('"kind":"alliance_', 1, true)
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
DealAgreementTypes = { MAKE_PEACE = "make_peace", OPEN_BORDERS = "open_borders", ALLIANCE = "alliance" }
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
		dealsEqual = opts.dealsEqual ~= false }
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
	function outgoing:Validate() state.validated = (state.validated or 0) + 1 end
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
				return { { ForType = 12, MaxAmount = 3, IsValid = true } }
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
				GetAllianceType = function() return state.allianceType or -1 end,
				IsDiplomaticActionValid = function(_, action)
					state.validity = state.validity or {}
					state.validity[#state.validity + 1] = action
					return not (opts.invalid and opts.invalid[action])
				end,
			}
		end,
		GetTreasury = function() return { GetGoldBalance = function() return 400 end } end,
		GetCulture = function() return {
			GetTouristsTo = function() return 0 end,
			GetStaycationers = function() return (opts.domestic or {})[7] or 0 end,
		} end,
	}
	Players = setmetatable({ [7] = player }, {
		__index = function(_, id) return {
			IsMajor = function() return true end,
			GetDiplomaticAI = function() return {
				GetDiplomaticStateIndex = function() return opts.stateIndex or 0 end,
			} end,
			GetCulture = function() return {
				GetTouristsTo = function() return (opts.tourists or {})[id] or 0 end,
				GetStaycationers = function() return (opts.domestic or {})[id] or 0 end,
			} end,
		} end,
	})
	return state, player
end

local function alliance(p, subject, verb)
	return applyOrder(p, 7, { kind = "alliance", subject = tostring(subject), verb = verb or "ALLIANCE_RESEARCH" }, TURN)
end
local askDeferred = trade.askDeferred
check("the tick pump is exported", type(askDeferred), "function")

-- ── Off by default: the question goes out on the opening statement ──
TURN = 100
local plain, plainPlayer = fixture({ stateIndex = 1 })
check("the default asks", select(2, alliance(plainPlayer, 3)), "alliance_asked")
onStatement(7, 3, { StatementType = "MAKE_DEAL", SessionID = 950 })
check("by default the opening statement puts the question", plain.sends[1] and plain.sends[1][1], "proposed")
trade.close(7, 3, "test_default")

-- ── On: the opening statement defers, the tick asks ──
CivvisControlConfig.DealAskOnTick = true
TURN = 110
local tick, tickPlayer = fixture({ stateIndex = 1 })
check("the deferred lane still asks", select(2, alliance(tickPlayer, 3)), "alliance_asked")
local function deferrals()
	local n = 0
	for _, line in ipairs(events) do
		if line:find('"phase":"ask_deferred"', 1, true) then n = n + 1 end
	end
	return n
end
local deferredBefore = deferrals()
onStatement(7, 3, { StatementType = "MAKE_DEAL", SessionID = 960 })
check("nothing is sent from the opening dispatch", #tick.sends, 0)
check("the session is not marked sent", trade.sessions[3] and trade.sessions[3].sent, false)
check("the wait is in the ledger", eventField(lastEvent("deal_session"), "phase"), "ask_deferred")
check("…with its session", eventField(lastEvent("deal_session"), "session"), "960")
onStatement(3, 7, { StatementType = "MAKE_DEAL", SessionID = 960, StatementSubType = "NONE",
	ResponseType = DiplomacyResponseTypes.ACKNOWLEDGE })
check("the rival's acknowledgement sends nothing either", #tick.sends, 0)
check("…and records one wait, not two", deferrals(), deferredBefore + 1)
askDeferred()
check("the tick puts the question", tick.sends[1] and tick.sends[1][1], "proposed")
check("the question is in the ledger", eventField(lastEvent("deal_session"), "phase"), "asked")
check("…for the session the statement named", eventField(lastEvent("deal_session"), "session"), "960")
check("the closer holds for the answer", holds[#holds] and holds[#holds].seconds, 30)
check("the session is marked sent", trade.sessions[3].sent, true)
askDeferred()
check("a second tick sends nothing more", #tick.sends, 1)
-- The verdict and the counter are handled exactly as before.
onStatement(3, 7, { StatementType = "MAKE_DEAL", SessionID = 960, DealAction = DealProposalAction.REJECTED })
check("the refusal waits for the counter", eventField(lastEvent("deal_session"), "phase"), "counter_wait")
onStatement(3, 7, { StatementType = "MAKE_DEAL", SessionID = 960, StatementSubType = "POSITIVE" })
check("the counter is refused", tick.sends[2] and tick.sends[2][1], "rejected")
check("the session is closed after its counter", sessions.closed[#sessions.closed], 960)
check("the session is forgotten", trade.sessions[3], nil)

-- ── Peace is deferred the same way ──
local peace, peacePlayer = fixture({ atWar = true })
applyOrder(peacePlayer, 7, { kind = "peace", subject = "4" }, TURN)
onStatement(7, 4, { StatementType = "MAKE_DEAL", SessionID = 961 })
check("a peace offer is not sent from the opening dispatch", #peace.sends, 0)
askDeferred()
check("the tick proposes the peace", peace.sends[1] and peace.sends[1][1], "proposed")
trade.close(7, 4, "test_peace")

-- ── A trade ask is not deferred ──
local sale, seller = fixture()
applyOrder(seller, 7, { kind = "sell", subject = "5", verb = "RESOURCE_DYES=1", x = 60 }, TURN)
onStatement(7, 5, { StatementType = "MAKE_DEAL", SessionID = 962 })
check("a sale is still asked on the opening statement", sale.sends[1] and sale.sends[1][1], "equalize")
trade.close(7, 5, "test_sale")

-- ── A session the end turn overtook is closed unasked ──
local late, latePlayer = fixture({ stateIndex = 1 })
check("a late ask goes out", select(2, alliance(latePlayer, 6)), "alliance_asked")
onStatement(7, 6, { StatementType = "MAKE_DEAL", SessionID = 963 })
onStatement(6, 7, { StatementType = "MAKE_DEAL", SessionID = 963, StatementSubType = "NONE",
	ResponseType = DiplomacyResponseTypes.ACKNOWLEDGE })
TURN = 111
askDeferred()
check("a session from an earlier turn is not asked", #late.sends, 0)
check("…it is closed, its acknowledgement already in", sessions.closed[#sessions.closed], 963)
check("…as opened after the end turn", eventField(lastEvent("deal_session"), "why"), "opened_after_end_turn")
check("…and forgotten", trade.sessions[6], nil)
check("…with its retry marker put back", CivvisK.allianceAsked and CivvisK.allianceAsked[6], nil)
local expired, expiredPlayer = fixture({ stateIndex = 1 })
alliance(expiredPlayer, 8)
onStatement(7, 8, { StatementType = "MAKE_DEAL", SessionID = 964 })
trade.sessions[8].end_turn_hold_expired = true
local closedBeforeAck = #sessions.closed
askDeferred()
check("a session whose opening hold ran out is not asked", #expired.sends, 0)
check("…and is not closed under an acknowledgement still to come", #sessions.closed, closedBeforeAck)
check("…it is marked unasked", trade.sessions[8] and trade.sessions[8].unasked ~= nil, true)
onStatement(8, 7, { StatementType = "MAKE_DEAL", SessionID = 964, StatementSubType = "NONE",
	ResponseType = DiplomacyResponseTypes.ACKNOWLEDGE })
check("…the acknowledgement closes it", sessions.closed[#sessions.closed], 964)
check("…and nothing was sent", #expired.sends, 0)

-- ── Turned off again, a pending wait is left to the ordinary closer ──
local parked, parkedPlayer = fixture({ stateIndex = 1 })
alliance(parkedPlayer, 9)
onStatement(7, 9, { StatementType = "MAKE_DEAL", SessionID = 965 })
CivvisControlConfig.DealAskOnTick = nil
askDeferred()
check("the pump does nothing while the switch is off", #parked.sends, 0)
trade.close(7, 9, "test_off")

-- ── Wiring: the game-core tick and the leader screen's pulse both pump ──
local src = assert(io.open(here .. "/CivvisControlAgent.lua")):read("*a")
local tickAt = assert(src:find("local function onGameCoreTick()", 1, true))
local tickEnd = assert(src:find("\nend\n", tickAt, true))
check("the game-core tick pumps the deferred questions",
	src:sub(tickAt, tickEnd):find("CivvisK.guard(CivvisTrade.askDeferred);", 1, true) ~= nil, true)
local pulseAt = assert(src:find("CivvisQueue.onUiPulse = function(source)", 1, true))
local pulseEnd = assert(src:find("\nend;\n", pulseAt, true))
check("the UI pulse pumps them too",
	src:sub(pulseAt, pulseEnd):find("CivvisK.guard(CivvisTrade.askDeferred);", 1, true) ~= nil, true)

if failures > 0 then
	realPrint(failures .. " failure(s)")
	os.exit(1)
end
realPrint("deal ask on tick: all ok")
