-- Offline regression for the planner's ALLIANCE (`CivvisAllianceStep`): on a
-- Diplomatic seat a friendship is asked first, then the ALLIANCE agreement in
-- a MAKE_DEAL session, accepted back only when the rival accepts. Harness
-- copied from deal_session_test.lua.
--
-- (deal_session_test.lua's header follows.)
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
-- Run: lua5.1 tools/civ6_control/mod/alliance_ask_test.lua

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
			or line:find('"kind":"alliance_', 1, true)) then
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

-- ── Not yet declared friends: the friendship goes first ──
TURN = 100
local friendly, friendlyPlayer = fixture({ stateIndex = 0 })
local ok, why = alliance(friendlyPlayer, 3)
check("a friendly rival is asked the friendship first", why, "alliance_friendship_asked")
check("the friendship ask succeeds", ok, true)
check("the friendship is the host's DECLARE_FRIEND session",
	sessions.requested[#sessions.requested] and sessions.requested[#sessions.requested].kind, "DECLARE_FRIEND")
check("no working deal goes out with the friendship", #friendly.sends, 0)
check("the friendship step is in the ledger", eventField(lastEvent("alliance_request"), "step"), "friendship")
local requested = #sessions.requested
ok, why = alliance(friendlyPlayer, 3)
check("a second ask inside the cooldown waits", why, "alliance_cooldown")
check("the cooldown opens no session", #sessions.requested, requested)

-- ── Declared friends: the ALLIANCE agreement in a MAKE_DEAL session ──
TURN = 110
local friends, friendsPlayer = fixture({ stateIndex = 1 })
ok, why = alliance(friendsPlayer, 3)
check("declared friends are asked the alliance", why, "alliance_asked")
check("the alliance ask succeeds", ok, true)
local item = friends.items[1]
check("the item is an agreement", item and item.kind, "agreements")
check("the agreement is an alliance", item and item.subType, "alliance")
check("the alliance type is the planner's", item and item.valueType, "ALLIANCE_RESEARCH")
check("the item is locked, as the shipped screen locks it", item and item.locked, true)
check("the item is ours", item and item.owner, 7)
check("the session is a MAKE_DEAL", sessions.requested[#sessions.requested].kind, "MAKE_DEAL")
check("the session is registered as an alliance", trade.sessions[3] and trade.sessions[3].kind, "alliance")
check("the host validated the specific alliance action",
	friends.validity and friends.validity[1], "DIPLOACTION_ALLIANCE_RESEARCH")
check("nothing is sent before the session opens", #friends.sends, 0)
-- Our opening statement puts the question as PROPOSED.
onStatement(7, 3, { StatementType = "MAKE_DEAL", SessionID = 950 })
check("the question is PROPOSED", friends.sends[1] and friends.sends[1][1], "proposed")
-- The rival accepts: accepted back, session closed.
onStatement(3, 7, { StatementType = "MAKE_DEAL", SessionID = 950, DealAction = DealProposalAction.ACCEPTED })
check("an accepted alliance is accepted back", friends.sends[2] and friends.sends[2][1], "accepted")
check("the answer is in the ledger", eventField(lastEvent("alliance_response"), "accepted"), "true")
check("the alliance session is closed by us", sessions.closed[#sessions.closed], 950)
check("the alliance session is forgotten", trade.sessions[3], nil)

-- ── A refusal is walked away from ──
TURN = 130
local refused, refusedPlayer = fixture({ stateIndex = 1 })
ok, why = alliance(refusedPlayer, 3)
check("the refused ask still goes out", why, "alliance_asked")
onStatement(7, 3, { StatementType = "MAKE_DEAL", SessionID = 951 })
onStatement(3, 7, { StatementType = "MAKE_DEAL", SessionID = 951, DealAction = DealProposalAction.REJECTED })
check("a refusal sends nothing back", #refused.sends, 1)
check("a refusal is in the ledger", eventField(lastEvent("alliance_response"), "accepted"), "false")
check("a refused session is closed", sessions.closed[#sessions.closed], 951)
-- Accepted but unequal: nothing is accepted back.
TURN = 140
local unequal, unequalPlayer = fixture({ stateIndex = 1, dealsEqual = false })
alliance(unequalPlayer, 3)
onStatement(7, 3, { StatementType = "MAKE_DEAL", SessionID = 952 })
onStatement(3, 7, { StatementType = "MAKE_DEAL", SessionID = 952, DealAction = DealProposalAction.ACCEPTED })
check("an unequal acceptance is not accepted back", #unequal.sends, 1)

-- ── The guards ──
TURN = 200
local guard, guardPlayer = fixture({ stateIndex = 1 })
guard.allianceType = 2
check("an existing alliance is not asked again", select(2, alliance(guardPlayer, 3)), "alliance_already_allied")
guard.allianceType = -1
guard.atWar = true
check("no alliance with an enemy", select(2, alliance(guardPlayer, 3)), "alliance_at_war")
guard.atWar = false
check("an unknown alliance type is refused", select(2, alliance(guardPlayer, 3, "ALLIANCE_IMPERIAL")), "alliance_unknown_type")
local _, coldPlayer = fixture({ stateIndex = 2 })
check("an unfriendly rival is not asked", select(2, alliance(coldPlayer, 4)), "alliance_friendship_not_friendly")
local _, closedPlayer = fixture({ stateIndex = 1, invalid = { DIPLOACTION_ALLIANCE_RESEARCH = true } })
check("an alliance the host does not offer is not asked", select(2, alliance(closedPlayer, 5)), "alliance_not_offered")
-- A culture racer at half the bar is neither befriended nor allied: the
-- counter's war on its tourism sources must stay open.
PlayerManager = { GetAliveMajorIDs = function() return { 7, 3, 4, 5 } end }
TURN = 220
local _, racerPlayer = fixture({ stateIndex = 1, tourists = { [3] = 60 }, domestic = { [7] = 100, [4] = 40 } })
check("an allied culture racer is not asked", select(2, alliance(racerPlayer, 3)), "alliance_culture_racer")
local _, racerFriend = fixture({ stateIndex = 0, tourists = { [3] = 60 }, domestic = { [7] = 100 } })
check("nor befriended", select(2, alliance(racerFriend, 3)), "alliance_culture_racer")
check("the share is the racer's tourists over the highest other domestic",
	CivvisAllianceCultureRacer(3, {}), true)
local _, calm = fixture({ stateIndex = 1, tourists = { [3] = 30 }, domestic = { [7] = 100 } })
check("below half the bar the alliance is asked", select(2, alliance(calm, 3)), "alliance_asked")
check("the share is configurable", CivvisAllianceCultureRacer(3, { AllianceCultureRacerShare = 0.25 }), true)

CivvisControlConfig.VictoryLane = "domination"
local _, offPlayer = fixture({ stateIndex = 1 })
check("another lane asks nothing", select(2, alliance(offPlayer, 6)), "alliance_off_lane")
CivvisControlConfig.VictoryLane = "diplomatic"
CivvisControlConfig.DiploAllies = false
check("the switch turns it off", select(2, alliance(offPlayer, 6)), "alliance_off_lane")
CivvisControlConfig.DiploAllies = nil

-- `culture-seat-allies`: a Culture seat asks only under `CultureAllies`,
-- and no other lane does.
CivvisControlConfig.VictoryLane = "culture"
local _, culturePlayer = fixture({ stateIndex = 1 })
check("a culture seat asks nothing by default", select(2, alliance(culturePlayer, 6)), "alliance_off_lane")
CivvisControlConfig.CultureAllies = true
local _, cultureAsks = fixture({ stateIndex = 1 })
check("a culture seat asks under CultureAllies", select(2, alliance(cultureAsks, 6)), "alliance_asked")
CivvisControlConfig.VictoryLane = "domination"
local _, dominationAsks = fixture({ stateIndex = 1 })
check("CultureAllies opens only the culture lane", select(2, alliance(dominationAsks, 6)), "alliance_off_lane")
-- `science-seat-allies`: a Science seat asks only under `ScienceAllies`.
CivvisControlConfig.CultureAllies = nil
CivvisControlConfig.VictoryLane = "science"
local _, sciencePlayer = fixture({ stateIndex = 1 })
check("a science seat asks nothing by default", select(2, alliance(sciencePlayer, 6)), "alliance_off_lane")
CivvisControlConfig.CultureAllies = true
local _, scienceCulture = fixture({ stateIndex = 1 })
check("CultureAllies does not open the science lane", select(2, alliance(scienceCulture, 6)), "alliance_off_lane")
CivvisControlConfig.CultureAllies = nil
CivvisControlConfig.ScienceAllies = true
local _, scienceAsks = fixture({ stateIndex = 1 })
check("a science seat asks under ScienceAllies", select(2, alliance(scienceAsks, 5)), "alliance_asked")
CivvisControlConfig.VictoryLane = "diplomatic"
CivvisControlConfig.ScienceAllies = nil

if failures > 0 then
	realPrint(failures .. " failure(s)")
	os.exit(1)
end
realPrint("alliance ask: all ok")
