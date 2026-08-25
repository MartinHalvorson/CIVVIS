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
			or line:find('"kind":"peace_', 1, true)) then
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
DiplomacyManager = {
	FindOpenSessionID = function() return nil end,
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
	local state = { sends = {}, items = {} }
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
		AreWorkingDealsEqual = function() return true end,
		SendWorkingDeal = function(action, pid, subject)
			state.sends[#state.sends + 1] = { action, pid, subject }
		end,
	}
	local player = {
		GetDiplomacy = function()
			return {
				HasMet = function() return true end,
				IsAtWarWith = function() return opts.atWar or false end,
				HasOpenBordersFrom = function() return false end,
			}
		end,
		GetTreasury = function() return { GetGoldBalance = function() return 400 end } end,
	}
	Players = setmetatable({}, { __index = function() return { IsMajor = function() return true end } end })
	return state, player
end

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

-- Our own opening statement: the session is live, the question goes out.
onStatement(7, 3, { SessionID = 901 })
check("the question is EQUALIZE", sale.sends[1] and sale.sends[1][1], "equalize")
check("the question names both players", sale.sends[1] and (sale.sends[1][2] .. ":" .. sale.sends[1][3]), "7:3")
check("the session id is kept", trade.sessions[3] and trade.sessions[3].sessionID, 901)
check("the ask is in the ledger", eventField(lastEvent("deal_session"), "phase"), "asked")

-- The rival's statement carries its verdict: a fair answer is accepted and
-- the session is closed by us.
onStatement(3, 7, { SessionID = 901, DealAction = DealProposalAction.ADJUSTED })
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
onStatement(7, 3, { SessionID = 902 })
onStatement(3, 7, { SessionID = 902, DealAction = DealProposalAction.ADJUSTED })
check("a lowball is not accepted", #low.sends, 1)
check("a lowball is in the ledger", eventField(lastEvent("deal_declined"), "worth"), "20")
check("a declined session is closed too", sessions.closed[2], 902)

-- ── Peace rides the same lane: PROPOSED on our statement, ACCEPTED enacts ──
TURN = 70
local peace, peacePlayer = fixture({ atWar = true })
ok, why = applyOrder(peacePlayer, 7, { kind = "peace", subject = "5" }, TURN)
check("peace is submitted through the lane", ok, true)
check("peace says submitted", why, "peace_submitted")
check("peace waits for its session", #peace.sends, 0)
check("peace requests a MAKE_DEAL session", sessions.requested[#sessions.requested].kind, "MAKE_DEAL")
onStatement(7, 5, { SessionID = 903 })
check("the peace offer is PROPOSED once the session is live", peace.sends[1] and peace.sends[1][1], "proposed")
onStatement(5, 7, { SessionID = 903, DealAction = DealProposalAction.ACCEPTED })
check("an accepted peace is enacted by our ACCEPTED", peace.sends[2] and peace.sends[2][1], "accepted")
check("the peace answer is in the ledger", eventField(lastEvent("peace_response"), "accepted"), "true")
check("the peace session is closed", sessions.closed[#sessions.closed], 903)

TURN = 80
local refused, refusedPlayer = fixture({ atWar = true })
applyOrder(refusedPlayer, 7, { kind = "peace", subject = "6" }, TURN)
onStatement(7, 6, { SessionID = 904 })
onStatement(6, 7, { SessionID = 904, DealAction = DealProposalAction.REJECTED })
check("a refused peace sends nothing more", #refused.sends, 1)
check("the refusal is in the ledger", eventField(lastEvent("peace_response"), "accepted"), "false")

-- ── Unanswered sessions are counted, and the third stands the lane down ──
for i = 1, 3 do
	TURN = 100 + i * 10
	local quiet, quietPlayer = fixture()
	local asked = applyOrder(quietPlayer, 7, { kind = "sell", subject = "3", verb = "RESOURCE_DYES=1", x = 60 }, TURN)
	check("unanswered ask " .. i .. " goes out", asked, true)
	onStatement(7, 3, { SessionID = 910 + i })
	-- The closer's ladder shuts the screen; the core reports the session closed.
	onClosed(910 + i)
	check("unanswered session " .. i .. " is counted", trade.unanswered, i)
	check("unanswered session " .. i .. " drops the ask", trade.pending[3], nil)
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
local closerSrc = assert(io.open(here .. "/CivvisControlAutoClose.lua")):read("*a")
check("the closer listens for the session hold",
	closerSrc:find("LuaEvents.CivvisDealSession.Add(", 1, true) ~= nil, true)
check("the closer holds only the diplomacy views",
	closerSrc:find('dealHold > 0 and (NAME == "DiplomacyActionView" or NAME == "DiplomacyDealView")', 1, true) ~= nil, true)
check("the hold is bounded by the seconds it was given",
	closerSrc:find("dealHold = dealHold - dt;", 1, true) ~= nil, true)

if failures > 0 then
	realPrint(string.format("%d failure(s)", failures))
	os.exit(1)
end
realPrint("all deal session checks passed")
