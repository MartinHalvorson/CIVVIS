-- Offline regression for major-civilization peace submission.
--
-- The live bridge used to open a MAKE_DEAL session after building the working
-- deal.  CivvisControlAutoClose must refuse that screen to keep unattended
-- games moving, so a request that did not throw was logged as an offer even
-- though it could never reach the rival.  This test loads the shipped agent
-- and exercises the exact helper used by `applyOrder`.
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
	if key == "CivvisSubmitMajorPeaceDeal" then return rawget(_G, key); end
	return stub()
end })

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtimeErr = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtimeErr))

local submit = rawget(_G, "CivvisSubmitMajorPeaceDeal")
assert(type(submit) == "function",
	"CivvisControlAgent.lua did not export CivvisSubmitMajorPeaceDeal")

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

-- A white peace offer must construct both locked peace sides, validate them,
-- then submit a normal PROPOSED working deal without opening the deal view.
local white, whitePlayer = fixture()
local submitted, concession, reason = submit(7, 11, whitePlayer, nil)
check("white peace submits", submitted, true)
check("white peace has no concession", concession, 0)
check("white peace says submitted", reason, "submitted")
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

-- A retry may offer at most three quarters of the treasury.  The locked peace
-- agreement remains and the same direct proposal, not a session, is sent.
local retry, retryPlayer = fixture({ gold = 200 })
submitted, concession, reason = submit(3, 5, retryPlayer, 40)
check("retry submits", submitted, true)
check("retry says submitted", reason, "submitted")
check("retry offers three quarters", concession, 150)
check("retry records gold item", retry.goldAmount, 150)
check("retry gold has no duration", retry.goldDuration, 0)
check("retry still sends direct proposal", retry.sendArgs[1], "proposed")
check("retry does not open a UI session", retry.openedSession, nil)

local capped, cappedPlayer = fixture({ gold = 1000, goldMax = 125 })
submitted, concession = submit(3, 5, cappedPlayer, 40)
check("retry respects Firaxis gold maximum", submitted and concession, 125)
check("capped retry sends direct proposal", capped.sendArgs[1], "proposed")

-- An existing pending deal is already in the engine.  Do not overwrite it or
-- call the proposal surface again; the caller will retain its normal cooldown.
local pending, pendingPlayer = fixture({ pending = true })
submitted, concession, reason = submit(3, 5, pendingPlayer, nil)
check("pending deal is not resubmitted", submitted, false)
check("pending deal has no concession", concession, 0)
check("pending deal reports why", reason, "pending")
check("pending deal does not clear work", pending.clearArgs, nil)
check("pending deal does not send", pending.sendArgs, nil)

-- A missing peace item is not a valid offer.  Previously this still opened a
-- session and recorded success solely because the call did not throw.
local missing, missingPlayer = fixture({ noPeaceItem = true })
submitted, concession, reason = submit(3, 5, missingPlayer, nil)
check("missing peace item does not submit", submitted, false)
check("missing peace item has no concession", concession, 0)
check("missing peace item is named", reason, "no_peace_item")
check("missing peace item does not validate", missing.validated, nil)
check("missing peace item does not send", missing.sendArgs, nil)

-- The UI only enables its normal proposal button after `Validate`/`IsValid`.
-- The direct bridge must preserve that guard rather than handing an invalid
-- package to the engine and recording a submission.
local invalidDeal, invalidDealPlayer = fixture({ dealValid = false })
submitted, concession, reason = submit(3, 5, invalidDealPlayer, nil)
check("invalid package does not submit", submitted, false)
check("invalid package has no concession", concession, 0)
check("invalid package is named", reason, "invalid_deal")
check("invalid package does not send", invalidDeal.sendArgs, nil)

-- A rejected optional Gold item must not convert a free peace proposal into a
-- false payment, but it must still submit the valid locked peace agreement.
local invalidGold, invalidGoldPlayer = fixture({ gold = 200, goldValid = false })
submitted, concession, reason = submit(3, 5, invalidGoldPlayer, 40)
check("invalid tribute keeps peace proposal", submitted, true)
check("invalid tribute is removed", invalidGold.removed, 88)
check("invalid tribute reports no concession", concession, 0)
check("invalid tribute still uses direct proposal", invalidGold.sendArgs[1], "proposed")

-- Ensure the order arm actually routes through the tested helper and cannot
-- regress to the known auto-refused session path behind its back.
local src = assert(io.open(here .. "/CivvisControlAgent.lua")):read("*a")
local peaceAt = assert(src:find('if kind == "peace" then', 1, true))
local delegationAt = assert(src:find('if kind == "delegation" then', peaceAt, true))
local peaceArm = src:sub(peaceAt, delegationAt - 1)
check("peace arm calls tested submitter",
	peaceArm:find("submitMajorPeaceDeal, pid, subject, player, asked", 1, true) ~= nil, true)
check("peace arm does not reopen a deal session",
	peaceArm:find("DiplomacyManager.RequestSession", 1, true) == nil, true)
check("submitter uses Firaxis normal proposal",
	src:find("DealManager.SendWorkingDeal(DealProposalAction.PROPOSED, pid, subject);", 1, true) ~= nil,
	true)

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("\nall direct peace-submission checks passed")
