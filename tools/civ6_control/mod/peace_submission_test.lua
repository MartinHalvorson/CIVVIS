-- Offline regression for major-civilization peace submission.
--
-- The live bridge used to open a MAKE_DEAL session after building the working
-- deal.  CivvisControlAutoClose must refuse that screen to keep unattended
-- games moving, so a request that did not throw was logged as an offer even
-- though it could never reach the rival.  This test loads the shipped agent
-- and exercises the exact `applyOrder` path that dispatches the proposal.
--
-- Run: lua5.1 tools/civ6_control/mod/peace_submission_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

-- CivvisControlAgent.lua is a Civ 6 UI script.  Its top-level setup reads a
-- large engine surface, so answer unrelated globals with a permissive dummy
-- while preserving the bare global exported for this test.
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

-- The agent snapshots this installer-provided table into a file-local `cfg`
-- while loading.  Give the exercised retry guard its real numeric default;
-- otherwise the permissive dummy would be truthy and mask the cooldown path.
CivvisControlConfig = { PeaceRetryTurns = 5 }

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

	local peaceItem = {}
	function peaceItem:SetSubType(value)
		state.peaceSubtype = value
		call("peace_subtype")
	end
	function peaceItem:SetLocked(value)
		state.peaceLocked = value
		call("peace_locked")
	end

	local goldItem = {}
	function goldItem:SetDuration(value)
		state.goldDuration = value
		call("gold_duration")
	end
	function goldItem:GetMaxAmount()
		return opts.goldMax or 9999
	end
	function goldItem:SetAmount(value)
		state.goldAmount = value
		call("gold_amount")
	end
	function goldItem:IsValid()
		return opts.goldValid ~= false
	end
	function goldItem:GetID()
		return 88
	end

	local deal = {}
	function deal:AddItemOfType(kind, owner)
		if kind == DealItemTypes.AGREEMENTS then
			state.peaceOwner = owner
			call("peace_item")
			if opts.noPeaceItem then return nil end
			return peaceItem
		end
		if kind == DealItemTypes.GOLD then
			state.goldOwner = owner
			call("gold_item")
			if opts.noGoldItem then return nil end
			return goldItem
		end
		return nil
	end
	function deal:Validate()
		state.validated = true
		state.validationCount = (state.validationCount or 0) + 1
		call("validate")
	end
	function deal:IsValid()
		state.validityChecked = (state.validityChecked or 0) + 1
		call("valid")
		return opts.dealValid ~= false
	end
	function deal:RemoveItemByID(id)
		state.removed = id
		call("remove")
	end

	DealDirection = { OUTGOING = "outgoing" }
	DealItemTypes = { AGREEMENTS = "agreements", GOLD = "gold" }
	DealAgreementTypes = { MAKE_PEACE = "make_peace" }
	DealProposalAction = { PROPOSED = "proposed" }
	Players = setmetatable({}, {
		__index = function()
			return { IsMajor = function() return opts.major ~= false end }
		end,
	})
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
	-- Any accidental return to the UI-session path becomes a test failure.
	DiplomacyManager = {
		RequestSession = function()
			state.openedSession = true
		end,
	}

	local player = {
		GetDiplomacy = function()
			return {
				IsAtWarWith = function(_, subject)
					state.warSubject = subject
					return opts.atWar ~= false
				end,
			}
		end,
		GetTreasury = function()
			return { GetGoldBalance = function() return opts.gold or 0 end }
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

local function peaceOrder(pid, subject, player, turn, cap)
	return applyOrder(player, pid, {
		kind = "peace", subject = tostring(subject), x = cap,
	}, turn)
end

-- A white peace offer must construct both locked peace sides, validate them,
-- then submit a normal PROPOSED working deal without opening the deal view.
local white, whitePlayer = fixture()
local submitted, reason = peaceOrder(7, 11, whitePlayer, 10)
check("white peace submits", submitted, true)
check("white peace says submitted", reason, "peace_submitted")
check("white peace clears outgoing deal", white.clearArgs[1], "outgoing")
check("white peace keeps players", white.clearArgs[2] .. ":" .. white.clearArgs[3], "7:11")
check("peace item belongs to sender", white.peaceOwner, 7)
check("peace agreement is MAKE_PEACE", white.peaceSubtype, "make_peace")
check("peace agreement is locked", white.peaceLocked, true)
check("working deal validates before send", callAt(white, "validate") < callAt(white, "send"), true)
check("working deal checks final validity", white.validityChecked, 2)
check("working deal uses PROPOSED", white.sendArgs[1], "proposed")
check("proposal keeps players", white.sendArgs[2] .. ":" .. white.sendArgs[3], "7:11")
check("proposal does not open a UI session", white.openedSession, nil)

-- A retry without a tribute cap from the planner is white peace again: the
-- order carries no `x`, so no Gold item is added at all.  Until 2026-08-24
-- every retry put three quarters of the treasury on the table.
local whiteRetryFirst, whiteRetryFirstPlayer = fixture()
peaceOrder(3, 5, whiteRetryFirstPlayer, 40)
local whiteRetry, whiteRetryPlayer = fixture({ gold = 200 })
submitted, reason = peaceOrder(3, 5, whiteRetryPlayer, 46)
check("uncapped retry submits", submitted, true)
check("uncapped retry says submitted", reason, "peace_submitted")
check("uncapped retry adds no gold", whiteRetry.goldAmount, nil)
check("uncapped retry still sends direct proposal", whiteRetry.sendArgs[1], "proposed")
local zeroRetryFirst, zeroRetryFirstPlayer = fixture()
peaceOrder(3, 8, zeroRetryFirstPlayer, 40)
local zeroRetry, zeroRetryPlayer = fixture({ gold = 200 })
submitted = peaceOrder(3, 8, zeroRetryPlayer, 46, 0)
check("a zero cap adds no gold", submitted and zeroRetry.goldAmount, nil)

-- A routed retry carries the planner's cap, still bounded by three quarters
-- of the treasury.  The locked peace agreement remains and the same direct
-- proposal, not a session, is sent.
local retryFirst, retryFirstPlayer = fixture()
peaceOrder(3, 9, retryFirstPlayer, 40)
local retry, retryPlayer = fixture({ gold = 200 })
submitted, reason = peaceOrder(3, 9, retryPlayer, 46, 120)
check("capped retry submits", submitted, true)
check("capped retry says submitted", reason, "peace_submitted")
check("capped retry records the cap", retry.goldAmount, 120)
check("capped retry gold has no duration", retry.goldDuration, 0)
check("capped retry still sends direct proposal", retry.sendArgs[1], "proposed")
check("capped retry does not open a UI session", retry.openedSession, nil)
local richFirst, richFirstPlayer = fixture()
peaceOrder(3, 10, richFirstPlayer, 40)
local rich, richPlayer = fixture({ gold = 100 })
submitted = peaceOrder(3, 10, richPlayer, 46, 500)
check("a cap above three quarters of the treasury pays three quarters", submitted and rich.goldAmount, 75)
local firstAsk, firstAskPlayer = fixture({ gold = 1000 })
submitted = peaceOrder(3, 12, firstAskPlayer, 10, 250)
check("the first ask is white even with a cap", submitted and firstAsk.goldAmount, nil)

local cappedFirst, cappedFirstPlayer = fixture()
peaceOrder(3, 6, cappedFirstPlayer, 60)
local capped, cappedPlayer = fixture({ gold = 1000, goldMax = 125 })
submitted = peaceOrder(3, 6, cappedPlayer, 66, 300)
check("retry respects Firaxis gold maximum", submitted and capped.goldAmount, 125)
check("capped retry sends direct proposal", capped.sendArgs[1], "proposed")

-- An existing pending deal is already in the engine.  Do not overwrite it or
-- call the proposal surface again; the caller will retain its normal cooldown.
local pending, pendingPlayer = fixture({ pending = true })
submitted, reason = peaceOrder(3, 21, pendingPlayer, 10)
check("pending deal is not resubmitted", submitted, false)
check("pending deal reports why", reason, "pending")
check("pending deal does not clear work", pending.clearArgs, nil)
check("pending deal does not send", pending.sendArgs, nil)

-- A missing peace item is not a valid offer.  Previously this still opened a
-- session and recorded success solely because the call did not throw.
local missing, missingPlayer = fixture({ noPeaceItem = true })
submitted, reason = peaceOrder(3, 22, missingPlayer, 10)
check("missing peace item does not submit", submitted, false)
check("missing peace item is named", reason, "no_peace_item")
check("missing peace item does not validate", missing.validated, nil)
check("missing peace item does not send", missing.sendArgs, nil)

-- The UI only enables its normal proposal button after `Validate`/`IsValid`.
-- The direct bridge must preserve that guard rather than handing an invalid
-- package to the engine and recording a submission.
local invalidDeal, invalidDealPlayer = fixture({ dealValid = false })
submitted, reason = peaceOrder(3, 23, invalidDealPlayer, 10)
check("invalid package does not submit", submitted, false)
check("invalid package is named", reason, "invalid_deal")
check("invalid package does not send", invalidDeal.sendArgs, nil)

-- A rejected optional Gold item must not convert a free peace proposal into a
-- false payment, but it must still submit the valid locked peace agreement.
local invalidGoldFirst, invalidGoldFirstPlayer = fixture()
peaceOrder(3, 24, invalidGoldFirstPlayer, 40)
local invalidGold, invalidGoldPlayer = fixture({ gold = 200, goldValid = false })
submitted, reason = peaceOrder(3, 24, invalidGoldPlayer, 46, 120)
check("invalid tribute keeps peace proposal", submitted, true)
check("invalid tribute is removed", invalidGold.removed, 88)
check("invalid tribute says submitted", reason, "peace_submitted")
check("invalid tribute still uses direct proposal", invalidGold.sendArgs[1], "proposed")

-- Ensure the order arm actually routes through the tested helper and cannot
-- regress to the known auto-refused session path behind its back.
local src = assert(io.open(here .. "/CivvisControlAgent.lua")):read("*a")
local peaceAt = assert(src:find('if kind == "peace" then', 1, true))
local delegationAt = assert(src:find('if kind == "delegation" then', peaceAt, true))
local peaceArm = src:sub(peaceAt, delegationAt - 1)
check("peace arm calls tested submitter", peaceArm:find(
	"pcall(submitMajorPeaceDeal, subject, asked, x)", 1, true) ~= nil, true)
check("peace arm does not reopen a deal session",
	peaceArm:find("DiplomacyManager.RequestSession", 1, true) == nil, true)
check("submitter uses Firaxis normal proposal",
	src:find("DealManager.SendWorkingDeal(DealProposalAction.PROPOSED, pid, subject);", 1, true) ~= nil,
	true)
check("test reaches the actual order handler",
	src:find("CivvisApplyOrder = applyOrder;", 1, true) ~= nil, true)

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("\nall direct peace-submission checks passed")
