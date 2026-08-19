-- Offline regression for the Aid Request direct-Gold finisher.
--
-- Firaxis scores ordinary Gold gifts to an Aid Request's target.  The bridge
-- must send exactly the bounded amount CIVVIS chose, through a normal PROPOSED
-- working deal, without opening a UI session, accepting a counteroffer, or
-- silently sending a partial gift that cannot take first place.
--
-- Run: lua5.1 tools/civ6_control/mod/aid_gift_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end
setmetatable(_G, { __index = function(_, key)
	if key == "CivvisApplyOrder" then return rawget(_G, key); end
	return stub()
end })

CivvisControlConfig = { AidGiftRetryTurns = 2 }

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtimeErr = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtimeErr))

local applyOrder = rawget(_G, "CivvisApplyOrder")
assert(type(applyOrder) == "function",
	"CivvisControlAgent.lua did not export CivvisApplyOrder")

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", name, tostring(got)))
	end
end

local function fixture(opts)
	opts = opts or {}
	local state = { calls = {} }
	local function call(name)
		state.calls[#state.calls + 1] = name
	end

	local gift = {}
	function gift:SetDuration(value)
		state.duration = value
		call("duration")
	end
	function gift:GetMaxAmount()
		return opts.goldMax or 9999
	end
	function gift:SetAmount(value)
		state.amount = value
		call("amount")
	end
	function gift:IsValid()
		return opts.goldValid ~= false
	end
	function gift:GetID()
		return 91
	end

	local deal = {}
	function deal:AddItemOfType(kind, owner)
		state.itemKind, state.itemOwner = kind, owner
		call("item")
		if opts.noGoldItem then return nil end
		return gift
	end
	function deal:Validate()
		state.validationCount = (state.validationCount or 0) + 1
		call("validate")
	end
	function deal:IsValid()
		call("valid")
		return opts.dealValid ~= false
	end
	function deal:RemoveItemByID(id)
		state.removed = id
		call("remove")
	end

	DealDirection = { OUTGOING = "outgoing" }
	DealItemTypes = { GOLD = "gold" }
	DealProposalAction = { PROPOSED = "proposed", EQUALIZE = "equalize" }
	Players = setmetatable({}, {
		__index = function()
			return { IsMajor = function() return opts.major ~= false end }
		end,
	})
	CivvisTrade = { pending = opts.tradePending and { [11] = { turn = 9 } } or {}, asked = {} }
	DealManager = {
		HasPendingDeal = function(pid, subject)
			state.pendingArgs = { pid, subject }
			call("pending")
			return opts.pending or false
		end,
		ClearWorkingDeal = function(direction, pid, subject)
			state.clearArgs = { direction, pid, subject }
			call("clear")
		end,
		GetWorkingDeal = function(direction, pid, subject)
			state.dealArgs = { direction, pid, subject }
			call("working")
			if opts.noWorkingDeal then return nil end
			return deal
		end,
		SendWorkingDeal = function(action, pid, subject)
			state.sendArgs = { action, pid, subject }
			call("send")
		end,
	}
	DiplomacyManager = {
		RequestSession = function()
			state.openedSession = true
		end,
	}

	local player = {
		GetDiplomacy = function()
			return {
				HasMet = function(_, subject)
					state.metSubject = subject
					return opts.met ~= false
				end,
				IsAtWarWith = function(_, subject)
					state.warSubject = subject
					return opts.atWar or false
				end,
			}
		end,
		GetTreasury = function()
			return { GetGoldBalance = function() return opts.gold or 500 end }
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

local function aidOrder(pid, subject, player, turn, amount, emergency)
	return applyOrder(player, pid, {
		kind = "aid_gift",
		subject = tostring(subject),
		verb = emergency or "EMERGENCY_SEND_AID",
		x = tostring(amount),
	}, turn)
end

-- A valid request is one exact, sender-owned, zero-duration Gold item and a
-- direct normal offer. It never opens a deal UI, so the unattended closer
-- cannot automatically refuse it before the recipient sees it.
local happy, happyPlayer = fixture()
local submitted, reason = aidOrder(7, 11, happyPlayer, 10, 151)
check("gift submits", submitted, true)
check("gift says submitted", reason, "aid_gift_submitted")
check("gift checks pending first", happy.pendingArgs[1] .. ":" .. happy.pendingArgs[2], "7:11")
check("gift clears outgoing work", happy.clearArgs[1], "outgoing")
check("gift keeps players", happy.clearArgs[2] .. ":" .. happy.clearArgs[3], "7:11")
check("gift item is Gold", happy.itemKind, "gold")
check("gift belongs to sender", happy.itemOwner, 7)
check("gift is lump sum", happy.duration, 0)
check("gift keeps exact score amount", happy.amount, 151)
check("gift validates before send", callAt(happy, "validate") < callAt(happy, "send"), true)
check("gift checks final validity", callAt(happy, "valid") ~= nil, true)
check("gift uses direct proposal", happy.sendArgs[1], "proposed")
check("gift proposal keeps players", happy.sendArgs[2] .. ":" .. happy.sendArgs[3], "7:11")
check("gift opens no UI session", happy.openedSession, nil)

-- Exact means exact: if the host's item cap or treasury cannot make the lead,
-- remove the half-built item and leave score at zero rather than sending a
-- smaller, strategically useless gift.
local capped, cappedPlayer = fixture({ goldMax = 150 })
submitted, reason = aidOrder(7, 12, cappedPlayer, 10, 151)
check("capped gift does not submit", submitted, false)
check("capped gift is named", reason, "gold_limit")
check("capped gift is removed", capped.removed, 91)
check("capped gift does not set partial amount", capped.amount, nil)
check("capped gift does not send", capped.sendArgs, nil)

local poor, poorPlayer = fixture({ gold = 150 })
submitted, reason = aidOrder(7, 13, poorPlayer, 10, 151)
check("poor gift does not submit", submitted, false)
check("poor gift is named", reason, "unaffordable")
check("poor gift does not clear a deal", poor.clearArgs, nil)
check("poor gift does not send", poor.sendArgs, nil)

-- Existing host offers, open EQUALIZE work, or an unsafe recipient keep the
-- host state untouched. These checks complement Rust's mirror-side target
-- filter, because state may change between export and actuation.
local pending, pendingPlayer = fixture({ pending = true })
submitted, reason = aidOrder(7, 14, pendingPlayer, 10, 151)
check("host pending gift is not rebuilt", submitted, false)
check("host pending reason", reason, "pending")
check("host pending does not clear", pending.clearArgs, nil)

local trading, tradingPlayer = fixture({ tradePending = true })
submitted, reason = aidOrder(7, 11, tradingPlayer, 20, 151)
check("equalize work blocks gift", submitted, false)
check("equalize work reason", reason, "aid_gift_trade_pending")
check("equalize work does not clear", trading.clearArgs, nil)

for _, unsafe in ipairs({
	{ name = "unmet", opts = { met = false }, reason = "aid_gift_not_met" },
	{ name = "war", opts = { atWar = true }, reason = "aid_gift_at_war" },
	{ name = "minor", opts = { major = false }, reason = "aid_gift_not_major" },
}) do
	local guarded, guardedPlayer = fixture(unsafe.opts)
	submitted, reason = aidOrder(7, 20, guardedPlayer, 10, 151)
	check(unsafe.name .. " gift does not submit", submitted, false)
	check(unsafe.name .. " gift is named", reason, unsafe.reason)
	check(unsafe.name .. " gift does not clear", guarded.clearArgs, nil)
end

-- A successful normal offer is in the engine and may not be recreated on the
-- immediately following frame. A different recipient keeps the regression
-- independent from the previous cases' local cooldown keys.
local first, firstPlayer = fixture()
aidOrder(7, 31, firstPlayer, 40, 151, "EMERGENCY_SEND_MILITARY_AID")
local retry, retryPlayer = fixture()
submitted, reason = aidOrder(7, 31, retryPlayer, 41, 151, "EMERGENCY_SEND_MILITARY_AID")
check("gift cooldown does not resubmit", submitted, false)
check("gift cooldown is named", reason, "aid_gift_cooldown")
check("gift cooldown does not clear", retry.clearArgs, nil)

-- Keep the tested arm wired to the real order handler and prevent a future
-- regression to a price negotiation or UI path that would cease to be a gift.
local src = assert(io.open(here .. "/CivvisControlAgent.lua")):read("*a")
local aidAt = assert(src:find('if kind == "aid_gift" then', 1, true))
local sellAt = assert(src:find('if kind == "sell" then', aidAt, true))
local aidArm = src:sub(aidAt, sellAt - 1)
check("aid arm calls exact submitter", aidArm:find(
	"pcall(submitAidGift, subject, amount)", 1, true) ~= nil, true)
check("aid arm opens no UI session",
	aidArm:find("DiplomacyManager.RequestSession", 1, true) == nil, true)
check("aid arm does not negotiate a price",
	aidArm:find("DealProposalAction.EQUALIZE", 1, true) == nil, true)
check("gift helper uses normal proposal",
	src:find("local function submitAidGift(subject, asked)", 1, true) ~= nil
		and src:find("DealManager.SendWorkingDeal(DealProposalAction.PROPOSED, pid, subject);", 1, true) ~= nil,
	true)
check("test reaches actual order handler",
	src:find("CivvisApplyOrder = applyOrder;", 1, true) ~= nil, true)

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("\nall direct Aid Request gift checks passed")
