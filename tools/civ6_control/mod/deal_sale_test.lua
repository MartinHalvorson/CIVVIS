-- Offline regression for the surplus-sale lane: the `sell` order arm and the
-- `Events.DiplomacyIncomingDeal` handler that closes what the rival offers.
--
-- The lane asks the rival's own valuation for the gold (EQUALIZE) and accepts
-- only an answer that returns exactly the offered items against gold at or
-- above the floor the order carried. This test loads the shipped agent and
-- exercises the real `applyOrder` arm and the real handler against a deal
-- manager fixture, so a regression to a blind PROPOSED, a session, or an
-- accept of a foreign counter-offer fails here rather than in a live game.
--
-- Run: lua5.1 tools/civ6_control/mod/deal_sale_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end
setmetatable(_G, { __index = function(_, key)
	if key == "CivvisApplyOrder" or key == "CivvisOnIncomingDeal" or key == "CivvisTrade" then
		return rawget(_G, key);
	end
	return stub()
end })

CivvisControlConfig = { TradeRetryTurns = 3, TradeResponseTurns = 2 }

-- Capture the agent's ledger lines so the events the lane writes can be
-- asserted on, not just its return values.
local events = {}
local realPrint = print
print = function(line)
	if type(line) == "string" and line:find('"kind":"deal_', 1, true) then
		events[#events + 1] = line
	end
end

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtimeErr = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtimeErr))

local applyOrder = rawget(_G, "CivvisApplyOrder")
assert(type(applyOrder) == "function", "CivvisControlAgent.lua did not export CivvisApplyOrder")
local onIncoming = rawget(_G, "CivvisOnIncomingDeal")
assert(type(onIncoming) == "function", "CivvisControlAgent.lua did not export CivvisOnIncomingDeal")
local trade = rawget(_G, "CivvisTrade")
assert(type(trade) == "table", "CivvisControlAgent.lua did not export CivvisTrade")

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

DealDirection = { OUTGOING = "outgoing", INCOMING = "incoming" }
DealItemTypes = { RESOURCES = "resources", FAVOR = "favor", GOLD = "gold", AGREEMENTS = "agreements" }
DealProposalAction = {
	PENDING = "pending", PROPOSED = "proposed", ACCEPTED = "accepted", REJECTED = "rejected",
	ADJUSTED = "adjusted", EQUALIZE = "equalize", EQUALIZE_FAILED = "equalize_failed",
}
local resourceRows = {
	RESOURCE_DYES = { ResourceType = "RESOURCE_DYES", Index = 12 },
	RESOURCE_IRON = { ResourceType = "RESOURCE_IRON", Index = 44 },
	RESOURCE_SILK = { ResourceType = "RESOURCE_SILK", Index = 30 },
}
GameInfo = {
	Resources = setmetatable({}, { __index = function(_, key)
		if type(key) == "number" then
			for _, row in pairs(resourceRows) do
				if row.Index == key then return row end
			end
			return nil
		end
		return resourceRows[key]
	end }),
	Resource_Consumption = { RESOURCE_IRON = { Accumulate = true } },
}
Game = {
	GetLocalPlayer = function() return 7 end,
	GetCurrentGameTurn = function() return 50 end,
}
DiplomacyManager = {
	FindOpenSessionID = function() return nil end,
	RequestSession = function() SESSION_OPENED = true end,
}

-- One fixture = one rival's deal manager. `opts.possible` lists what the
-- engine says we can trade (by resource name -> MaxAmount); `opts.incoming`
-- is the rival's answer, a list of {kind, from, duration, amount, valueType}.
local function fixture(opts)
	opts = opts or {}
	local state = { calls = {}, items = {} }
	local function call(name) state.calls[#state.calls + 1] = name end

	local nextId = 100
	local function newItem(kind)
		local item = { kind = kind, id = nextId }
		nextId = nextId + 1
		function item:SetValueType(v) self.valueType = v; call("value_type") end
		function item:SetDuration(v) self.duration = v; call("duration") end
		function item:SetAmount(v) self.amount = v; call("amount") end
		function item:GetMaxAmount()
			if kind == DealItemTypes.FAVOR then return opts.favorMax or 999 end
			return opts.resourceMax or 999
		end
		function item:IsValid() return opts.itemValid ~= false end
		function item:GetID() return self.id end
		return item
	end

	local outgoing = {}
	function outgoing:AddItemOfType(kind, owner)
		call("add_" .. tostring(kind))
		local item = newItem(kind)
		item.owner = owner
		state.items[#state.items + 1] = item
		return item
	end
	function outgoing:Validate() state.validated = true; call("validate") end
	function outgoing:IsValid() call("valid"); return opts.dealValid ~= false end
	function outgoing:RemoveItemByID(id)
		state.removed = state.removed or {}
		state.removed[#state.removed + 1] = id
		call("remove")
	end

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
			}
		end
	end

	DealManager = {
		HasPendingDeal = function(pid, subject)
			state.pendingArgs = { pid, subject }
			call("pending")
			return opts.hostPending or false
		end,
		ClearWorkingDeal = function(direction, pid, subject)
			state.clearArgs = { direction, pid, subject }
			state.clears = (state.clears or 0) + 1
			call("clear")
		end,
		GetWorkingDeal = function(direction, pid, subject)
			state.dealArgs = { direction, pid, subject }
			call("working_" .. tostring(direction))
			if opts.noWorkingDeal then return nil end
			if direction == DealDirection.INCOMING then return incoming end
			return outgoing
		end,
		GetPossibleDealItems = function(pid, subject, kind, deal)
			call("possible")
			local out = {}
			for name, max in pairs(opts.possible or {}) do
				out[#out + 1] = { ForType = resourceRows[name].Index, MaxAmount = max, IsValid = true }
			end
			return out
		end,
		SendWorkingDeal = function(action, pid, subject)
			state.sends = state.sends or {}
			state.sends[#state.sends + 1] = { action, pid, subject }
			call("send_" .. tostring(action))
			-- An engine that answers from inside the ask, before it returns.
			if action == DealProposalAction.EQUALIZE and opts.answerInline ~= nil then
				opts.incoming = opts.answerInline
				onIncoming(subject, pid, DealProposalAction.ADJUSTED)
			end
		end,
		CopyIncomingToOutgoingWorkingDeal = function(pid, subject)
			state.copied = { pid, subject }
			call("copy")
		end,
		AreWorkingDealsEqual = function(pid, subject)
			call("equal")
			return opts.dealsEqual ~= false
		end,
	}
	Players = setmetatable({}, {
		__index = function()
			return { IsMajor = function() return opts.major ~= false end }
		end,
	})
	local player = {
		GetDiplomacy = function()
			return {
				HasMet = function(_, subject) return opts.met ~= false end,
				IsAtWarWith = function(_, subject) return opts.atWar == true end,
			}
		end,
	}
	return state, player
end

local function callAt(state, wanted)
	for index, name in ipairs(state.calls) do
		if name == wanted then return index end
	end
	return nil
end

local function itemOfKind(state, kind)
	for _, item in ipairs(state.items) do
		if item.kind == kind then return item end
	end
	return nil
end

local function sellOrder(pid, subject, player, turn, verb, floor)
	return applyOrder(player, pid, {
		kind = "sell", subject = tostring(subject), verb = verb, x = floor, y = 0,
	}, turn)
end

local function reset()
	for k in pairs(trade.pending) do trade.pending[k] = nil end
	for k in pairs(trade.asked) do trade.asked[k] = nil end
	SESSION_OPENED = nil
end

-- ─── the ask ────────────────────────────────────────────────────────────────
-- A luxury copy and a favor block go into the outgoing deal the way the
-- shipped screen puts them there, the package validates, and the rival is
-- asked to price it with EQUALIZE — never PROPOSED, never a session.
reset()
local ask, askPlayer = fixture({ possible = { RESOURCE_DYES = 2 } })
local ok, why = sellOrder(7, 3, askPlayer, 50, "RESOURCE_DYES=1,FAVOR=10", 60)
check("ask is submitted", ok, true)
check("ask says asked", why, "sell_asked")
check("ask clears the outgoing deal first", ask.clearArgs[1], "outgoing")
check("ask names both players", ask.clearArgs[2] .. ":" .. ask.clearArgs[3], "7:3")
local dyes = itemOfKind(ask, DealItemTypes.RESOURCES)
check("luxury item is ours", dyes and dyes.owner, 7)
check("luxury item carries the engine's type", dyes and dyes.valueType, 12)
check("luxury item runs thirty turns", dyes and dyes.duration, 30)
check("luxury item amount", dyes and dyes.amount, 1)
local favor = itemOfKind(ask, DealItemTypes.FAVOR)
check("favor item is ours", favor and favor.owner, 7)
check("favor is a lump", favor and favor.duration, 0)
check("favor amount", favor and favor.amount, 10)
check("package validates before it is sent", callAt(ask, "validate") < callAt(ask, "send_equalize"), true)
check("the ask is EQUALIZE", ask.sends[1][1], "equalize")
check("the ask names both players", ask.sends[1][2] .. ":" .. ask.sends[1][3], "7:3")
check("no PROPOSED goes out on the ask", callAt(ask, "send_proposed"), nil)
check("no session opens", rawget(_G, "SESSION_OPENED"), nil)
check("the ask is pending", trade.pending[3] ~= nil, true)
check("the pending ask keeps the floor", trade.pending[3].floor, 60)
check("the pending ask keeps what was offered", trade.pending[3].gave["RESOURCES:12"], 1)
check("the pending ask keeps the favor", trade.pending[3].gave["FAVOR"], 10)
check("the offer is in the ledger", eventField(lastEvent("deal_offer"), "submitted"), "true")
check("the offer says what it gave", eventField(lastEvent("deal_offer"), "gave"), "RESOURCE_DYES=1x30,FAVOR=10")

-- A second ask to the same rival while the first is unanswered is refused
-- and does not touch the working deal.
local again, againPlayer = fixture({ possible = { RESOURCE_DYES = 2 } })
ok, why = sellOrder(7, 3, againPlayer, 51, "RESOURCE_DYES=1", 30)
check("a pending ask is not repeated", ok, false)
check("a pending ask says why", why, "sell_pending")
check("a pending ask sends nothing", again.sends, nil)

-- ─── the answer ─────────────────────────────────────────────────────────────
-- The rival returns exactly our items and puts 90 gold against them: the
-- shipped click — copy incoming over outgoing, then ACCEPTED.
local answer = fixture({ incoming = {
	{ kind = DealItemTypes.RESOURCES, from = 7, duration = 30, amount = 1, valueType = 12 },
	{ kind = DealItemTypes.FAVOR, from = 7, duration = 0, amount = 10 },
	{ kind = DealItemTypes.GOLD, from = 3, duration = 0, amount = 90 },
} })
onIncoming(3, 7, DealProposalAction.ADJUSTED)
check("a fair answer is copied over the outgoing deal", answer.copied and answer.copied[2], 3)
check("a fair answer is accepted", answer.sends and answer.sends[1][1], "accepted")
check("the accept names both players", answer.sends[1][2] .. ":" .. answer.sends[1][3], "7:3")
check("the answer settles the ask", trade.pending[3], nil)
check("the close is in the ledger", eventField(lastEvent("deal_closed"), "gold"), "90")
check("the close records the send", eventField(lastEvent("deal_closed"), "sent"), "true")
check("the response is in the ledger", eventField(lastEvent("deal_response"), "closable"), "true")

-- After a close the same rival is on cooldown, then free again.
local cool, coolPlayer = fixture({ possible = { RESOURCE_DYES = 2 } })
ok, why = sellOrder(7, 3, coolPlayer, 51, "RESOURCE_DYES=1", 30)
check("a just-closed rival is on cooldown", why, "sell_cooldown")
check("cooldown sends nothing", cool.sends, nil)
local free, freePlayer = fixture({ possible = { RESOURCE_DYES = 2 } })
ok, why = sellOrder(7, 3, freePlayer, 53, "RESOURCE_DYES=1", 30)
check("the cooldown ends", ok, true)
check("a fresh ask goes out", free.sends[1][1], "equalize")

-- ─── declined answers ───────────────────────────────────────────────────────
-- Below the floor: nothing sent, the outgoing deal cleared, the ask settled.
local low = fixture({ incoming = {
	{ kind = DealItemTypes.RESOURCES, from = 7, duration = 30, amount = 1, valueType = 12 },
	{ kind = DealItemTypes.GOLD, from = 3, duration = 0, amount = 20 },
} })
onIncoming(3, 7, DealProposalAction.ADJUSTED)
check("a lowball is not accepted", low.sends, nil)
check("a lowball clears the outgoing deal", low.clearArgs and low.clearArgs[1], "outgoing")
check("a lowball settles the ask", trade.pending[3], nil)
check("the decline is in the ledger", eventField(lastEvent("deal_declined"), "worth"), "20")

-- Per-turn gold counts at 25× — 3 a turn clears a floor of 60.
reset()
local gptAsk, gptAskPlayer = fixture({ possible = { RESOURCE_DYES = 2 } })
sellOrder(7, 3, gptAskPlayer, 60, "RESOURCE_DYES=1", 60)
local gpt = fixture({ incoming = {
	{ kind = DealItemTypes.RESOURCES, from = 7, duration = 30, amount = 1, valueType = 12 },
	{ kind = DealItemTypes.GOLD, from = 3, duration = 30, amount = 3 },
} })
onIncoming(3, 7, DealProposalAction.ACCEPTED)
check("gold per turn is priced at 25 a point", gpt.sends and gpt.sends[1][1], "accepted")
check("the per-turn close records the rate", eventField(lastEvent("deal_closed"), "gold_per_turn"), "3")

-- A foreign item on their side (an agreement) is not a sale, whatever the gold.
reset()
local foreignAsk, foreignAskPlayer = fixture({ possible = { RESOURCE_DYES = 2 } })
sellOrder(7, 3, foreignAskPlayer, 60, "RESOURCE_DYES=1", 30)
local foreign = fixture({ incoming = {
	{ kind = DealItemTypes.RESOURCES, from = 7, duration = 30, amount = 1, valueType = 12 },
	{ kind = DealItemTypes.AGREEMENTS, from = 3, duration = 30, amount = 0 },
	{ kind = DealItemTypes.GOLD, from = 3, duration = 0, amount = 300 },
} })
onIncoming(3, 7, DealProposalAction.ADJUSTED)
check("a counter with an agreement is declined", foreign.sends, nil)
check("the foreign count is in the ledger", eventField(lastEvent("deal_declined"), "foreign"), "1")

-- Our side altered (fifty favor where ten was offered) is declined.
reset()
local moreAsk, moreAskPlayer = fixture({ possible = {} })
sellOrder(7, 3, moreAskPlayer, 60, "FAVOR=10", 10)
local more = fixture({ incoming = {
	{ kind = DealItemTypes.FAVOR, from = 7, duration = 0, amount = 50 },
	{ kind = DealItemTypes.GOLD, from = 3, duration = 0, amount = 300 },
} })
onIncoming(3, 7, DealProposalAction.ADJUSTED)
check("a counter that takes more of ours is declined", more.sends, nil)
check("the mismatch is in the ledger", eventField(lastEvent("deal_declined"), "matches"), "false")

-- An extra item on our side (a luxury we never offered) is declined too.
reset()
local extraAsk, extraAskPlayer = fixture({ possible = {} })
sellOrder(7, 3, extraAskPlayer, 60, "FAVOR=10", 10)
local extra = fixture({ incoming = {
	{ kind = DealItemTypes.FAVOR, from = 7, duration = 0, amount = 10 },
	{ kind = DealItemTypes.RESOURCES, from = 7, duration = 30, amount = 1, valueType = 30 },
	{ kind = DealItemTypes.GOLD, from = 3, duration = 0, amount = 300 },
} })
onIncoming(3, 7, DealProposalAction.ADJUSTED)
check("a counter that adds to ours is declined", extra.sends, nil)

-- EQUALIZE_FAILED is a no, however the deal reads.
reset()
local failAsk, failAskPlayer = fixture({ possible = { RESOURCE_DYES = 2 } })
sellOrder(7, 3, failAskPlayer, 60, "RESOURCE_DYES=1", 30)
local failed = fixture({ incoming = {
	{ kind = DealItemTypes.RESOURCES, from = 7, duration = 30, amount = 1, valueType = 12 },
	{ kind = DealItemTypes.GOLD, from = 3, duration = 0, amount = 300 },
} })
onIncoming(3, 7, DealProposalAction.EQUALIZE_FAILED)
check("an equalize failure is declined", failed.sends, nil)
check("an equalize failure settles the ask", trade.pending[3], nil)

-- ─── answers nobody asked for ───────────────────────────────────────────────
-- A rival's own proposal is logged and left alone: no accept, no clear.
reset()
local theirs = fixture({ incoming = {
	{ kind = DealItemTypes.GOLD, from = 5, duration = 0, amount = 500 },
	{ kind = DealItemTypes.RESOURCES, from = 7, duration = 30, amount = 1, valueType = 12 },
} })
onIncoming(5, 7, DealProposalAction.PROPOSED)
check("a rival's proposal is not accepted", theirs.sends, nil)
check("a rival's proposal is not cleared", theirs.clearArgs, nil)
check("a rival's proposal is still in the ledger", eventField(lastEvent("deal_response"), "asked"), "false")

-- An answer addressed to someone else is ignored outright.
local other = fixture({ incoming = {} })
onIncoming(3, 9, DealProposalAction.ACCEPTED)
check("another seat's deal is ignored", other.dealArgs, nil)

-- ─── an ask the engine never answers ────────────────────────────────────────
-- Dropped after `TradeResponseTurns` so it cannot hold the working deal
-- against the peace arm; the per-rival cooldown still runs from the ask.
reset()
local stale, stalePlayer = fixture({ possible = { RESOURCE_DYES = 2 } })
sellOrder(7, 3, stalePlayer, 60, "RESOURCE_DYES=1", 30)
local early, earlyPlayer = fixture({ possible = { RESOURCE_DYES = 2 } })
ok, why = sellOrder(7, 3, earlyPlayer, 61, "RESOURCE_DYES=1", 30)
check("an ask waits for its answer", why, "sell_pending")
local drop, dropPlayer = fixture({ possible = { RESOURCE_DYES = 2 } })
ok, why = sellOrder(7, 3, dropPlayer, 63, "RESOURCE_DYES=1", 30)
check("an unanswered ask expires", ok, true)
check("the expiry is in the ledger", eventField(lastEvent("deal_expired"), "asked_turn"), "60")
check("the new ask goes out", drop.sends[1][1], "equalize")

-- ─── an engine that answers inside the ask ──────────────────────────────────
-- Whether the answer comes back on a later event batch or synchronously from
-- inside `SendWorkingDeal`, the ask is already registered and the close lands.
reset()
local inline, inlinePlayer = fixture({ possible = { RESOURCE_DYES = 2 }, answerInline = {
	{ kind = DealItemTypes.RESOURCES, from = 7, duration = 30, amount = 1, valueType = 12 },
	{ kind = DealItemTypes.GOLD, from = 3, duration = 0, amount = 120 },
} })
ok, why = sellOrder(7, 3, inlinePlayer, 80, "RESOURCE_DYES=1", 60)
check("an inline answer still reports the ask", why, "sell_asked")
check("an inline answer is accepted", inline.sends[2] and inline.sends[2][1], "accepted")
check("an inline answer settles the ask", trade.pending[3], nil)
check("an inline close is in the ledger", eventField(lastEvent("deal_closed"), "gold"), "120")

-- ─── item shapes ────────────────────────────────────────────────────────────
-- A stockpiled strategic is a lump, clipped to what the engine allows.
reset()
local iron, ironPlayer = fixture({ possible = { RESOURCE_IRON = 12 }, resourceMax = 12 })
ok, why = sellOrder(7, 3, ironPlayer, 70, "RESOURCE_IRON=20", 20)
check("a strategic block is asked", ok, true)
local ironItem = itemOfKind(iron, DealItemTypes.RESOURCES)
check("a stockpiled strategic is a lump", ironItem and ironItem.duration, 0)
check("a strategic block is clipped to the engine's maximum", ironItem and ironItem.amount, 12)
check("the pending ask keeps the clipped amount", trade.pending[3].gave["RESOURCES:44"], 12)

-- Favor is clipped to the bank the engine reports.
reset()
local bank, bankPlayer = fixture({ favorMax = 35 })
sellOrder(7, 3, bankPlayer, 70, "FAVOR=150", 150)
local bankItem = itemOfKind(bank, DealItemTypes.FAVOR)
check("favor is clipped to the bank", bankItem and bankItem.amount, 35)

-- A resource the engine will not let us trade is left out; nothing to sell
-- is a named refusal that starts the cooldown rather than re-asking.
reset()
local none, nonePlayer = fixture({ possible = {} })
ok, why = sellOrder(7, 3, nonePlayer, 70, "RESOURCE_DYES=1", 30)
check("nothing tradeable is refused", ok, false)
check("nothing tradeable is named", why, "nothing_tradeable")
check("nothing tradeable sends nothing", none.sends, nil)
check("nothing tradeable starts the cooldown", trade.asked[3], 70)
check("nothing tradeable leaves no pending ask", trade.pending[3], nil)

-- An unknown resource type is a named refusal.
reset()
local unknown, unknownPlayer = fixture({ possible = {} })
ok, why = sellOrder(7, 3, unknownPlayer, 70, "RESOURCE_UNOBTAINIUM=1", 30)
check("an unknown resource is refused", ok, false)
check("an unknown resource is named", why, "unknown_resource:RESOURCE_UNOBTAINIUM")

-- An invalid package is not sent.
reset()
local invalid, invalidPlayer = fixture({ possible = { RESOURCE_DYES = 2 }, dealValid = false })
ok, why = sellOrder(7, 3, invalidPlayer, 70, "RESOURCE_DYES=1", 30)
check("an invalid package is refused", ok, false)
check("an invalid package is named", why, "invalid_deal")
check("an invalid package sends nothing", invalid.sends, nil)

-- ─── guards ─────────────────────────────────────────────────────────────────
reset()
local unmet, unmetPlayer = fixture({ met = false })
ok, why = sellOrder(7, 3, unmetPlayer, 70, "FAVOR=10", 10)
check("an unmet rival is refused", why, "sell_not_met")
local war, warPlayer = fixture({ atWar = true })
ok, why = sellOrder(7, 3, warPlayer, 70, "FAVOR=10", 10)
check("a rival at war is refused", why, "sell_at_war")
local minor, minorPlayer = fixture({ major = false })
ok, why = sellOrder(7, 3, minorPlayer, 70, "FAVOR=10", 10)
check("a city-state is refused", why, "sell_not_major")
local unmapped, unmappedPlayer = fixture({})
ok, why = applyOrder(unmappedPlayer, 7, { kind = "sell", verb = "FAVOR=10", x = 10 }, 70)
check("an unmapped seat is refused", why, "sell_target_unmapped")
local hostPending, hostPendingPlayer = fixture({ hostPending = true })
ok, why = sellOrder(7, 3, hostPendingPlayer, 70, "FAVOR=10", 10)
check("an engine-pending deal is refused", why, "sell_host_pending")
check("no guard opened a session", rawget(_G, "SESSION_OPENED"), nil)

-- ─── wiring ─────────────────────────────────────────────────────────────────
local src = assert(io.open(here .. "/CivvisControlAgent.lua")):read("*a")
local sellAt = assert(src:find('if kind == "sell" then', 1, true))
local envoyAt = assert(src:find('if kind == "envoy" then', sellAt, true))
local sellArm = src:sub(sellAt, envoyAt - 1)
check("sell arm asks with EQUALIZE",
	sellArm:find("DealManager.SendWorkingDeal(DealProposalAction.EQUALIZE, pid, subject);", 1, true) ~= nil, true)
check("sell arm never proposes blind",
	sellArm:find("DealProposalAction.PROPOSED", 1, true) == nil, true)
check("sell arm never opens a session",
	sellArm:find("DiplomacyManager.RequestSession", 1, true) == nil, true)
check("the handler is wired to the engine event",
	src:find("DiplomacyIncomingDeal = CivvisOnIncomingDeal,", 1, true) ~= nil, true)
check("the handler accepts with ACCEPTED",
	src:find("DealManager.SendWorkingDeal(DealProposalAction.ACCEPTED, pid, fromPlayer);", 1, true) ~= nil, true)

if failures > 0 then
	realPrint(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
realPrint("\nall surplus-sale checks passed")
