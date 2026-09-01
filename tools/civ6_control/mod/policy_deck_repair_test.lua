-- Regression for a partial asynchronous policy-deck transaction.
--
-- The host can return successfully from RequestPolicyChanges while retaining
-- one old card and dropping one newly requested card. The next turn must make
-- one targeted repair, and same-turn re-plans must not submit a second racing
-- transaction.
--
-- Run: lua5.1 tools/civ6_control/mod/policy_deck_repair_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."
local realPrint = print

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end

local EXPORTS = { CivvisApplyOrder = true }
local LOG = {}
Automation = { Log = function(line) LOG[#LOG + 1] = line end }
CivvisControlConfig = { RunTag = "policy-deck-repair-test" }

-- The policy arm only needs these real database rows. Keep both name and Index
-- lookup because the shipped agent resolves the order by name and reads the
-- host's active card by numeric policy index.
local policyRows = {
	POLICY_INTEGRATED_SPACE_CELL = {
		PolicyType = "POLICY_INTEGRATED_SPACE_CELL", Index = 1, Hash = 101,
		GovernmentSlotType = "SLOT_MILITARY",
	},
	POLICY_LEVEE_EN_MASSE = {
		PolicyType = "POLICY_LEVEE_EN_MASSE", Index = 2, Hash = 102,
		GovernmentSlotType = "SLOT_MILITARY",
	},
	POLICY_LIGHTNING_WARFARE = {
		PolicyType = "POLICY_LIGHTNING_WARFARE", Index = 3, Hash = 103,
		GovernmentSlotType = "SLOT_MILITARY",
	},
	POLICY_FIVE_YEAR_PLAN = {
		PolicyType = "POLICY_FIVE_YEAR_PLAN", Index = 4, Hash = 104,
		GovernmentSlotType = "SLOT_ECONOMIC",
	},
	POLICY_NEW_DEAL = {
		PolicyType = "POLICY_NEW_DEAL", Index = 5, Hash = 105,
		GovernmentSlotType = "SLOT_ECONOMIC",
	},
	POLICY_PUBLIC_WORKS = {
		PolicyType = "POLICY_PUBLIC_WORKS", Index = 6, Hash = 106,
		GovernmentSlotType = "SLOT_ECONOMIC",
	},
	POLICY_LIBERALISM = {
		PolicyType = "POLICY_LIBERALISM", Index = 7, Hash = 107,
		GovernmentSlotType = "SLOT_ECONOMIC",
	},
	POLICY_CRYPTOGRAPHY = {
		PolicyType = "POLICY_CRYPTOGRAPHY", Index = 8, Hash = 108,
		GovernmentSlotType = "SLOT_DIPLOMATIC",
	},
	POLICY_RATIONALISM = {
		PolicyType = "POLICY_RATIONALISM", Index = 9, Hash = 109,
		GovernmentSlotType = "SLOT_ECONOMIC",
	},
}
for _, name in ipairs({
	"POLICY_INTEGRATED_SPACE_CELL", "POLICY_LEVEE_EN_MASSE",
	"POLICY_LIGHTNING_WARFARE", "POLICY_FIVE_YEAR_PLAN", "POLICY_NEW_DEAL",
	"POLICY_PUBLIC_WORKS", "POLICY_LIBERALISM", "POLICY_CRYPTOGRAPHY",
	"POLICY_RATIONALISM",
}) do
	local row = policyRows[name]
	policyRows[row.Index] = row
end
setmetatable(policyRows, {
	__call = function()
		local rows, i = {}, 0
		for _, row in pairs(policyRows) do
			if type(row) == "table" then rows[#rows + 1] = row end
		end
		table.sort(rows, function(a, b) return a.Index < b.Index end)
		return function()
			i = i + 1
			return rows[i]
		end
	end,
})

local slotRows = {
	[1] = { GovernmentSlotType = "SLOT_MILITARY" },
	[2] = { GovernmentSlotType = "SLOT_MILITARY" },
	[3] = { GovernmentSlotType = "SLOT_MILITARY" },
	[4] = { GovernmentSlotType = "SLOT_ECONOMIC" },
	[5] = { GovernmentSlotType = "SLOT_ECONOMIC" },
	[6] = { GovernmentSlotType = "SLOT_ECONOMIC" },
	[7] = { GovernmentSlotType = "SLOT_DIPLOMATIC" },
	[8] = { GovernmentSlotType = "SLOT_WILDCARD" },
}
GameInfo = setmetatable({ Policies = policyRows, GovernmentSlots = slotRows }, {
	__index = function(_, key)
		if key == "UnitOperations" or key == "UnitCommands" then
			return setmetatable({}, { __index = function(_, name) return { Hash = name } end })
		end
		return stub()
	end,
})
setmetatable(_G, { __index = function(_, key)
	if EXPORTS[key] then return rawget(_G, key) end
	return stub()
end })

local host = {
	-- Old deck: Liberalism occupies the slot where the requested New Deal should
	-- land. The simulated host will reproduce the observed partial apply.
	slots = { 1, 2, 3, 4, 7, 6, 8, 9 },
	calls = {},
}

local function copy(list)
	local out = {}
	for i, value in pairs(list) do out[i] = value end
	return out
end

local culture = {}
function culture:GetNumPolicySlots() return 8 end
function culture:GetSlotType(i) return i + 1 end
function culture:GetSlotPolicy(i) return host.slots[i + 1] or -1 end
function culture:IsPolicyUnlocked() return true end
function culture:IsPolicyObsolete() return false end
function culture:RequestPolicyChanges(clearList, addList)
	host.calls[#host.calls + 1] = { clear = copy(clearList), add = copy(addList) }
	for _, slot in ipairs(clearList) do host.slots[slot + 1] = -1 end
	for slot, hash in pairs(addList) do
		-- This is the host behavior under test: the call succeeds, but the newly
		-- unlocked New Deal is not applied and the old card remains in slot 4.
		if hash ~= policyRows.POLICY_NEW_DEAL.Hash then
			for _, row in pairs(policyRows) do
				if type(row) == "table" and row.Hash == hash then
					host.slots[slot + 1] = row.Index
				end
			end
		elseif #clearList == 1 then
			-- A targeted retry is the repaired transaction; this time apply it.
			host.slots[slot + 1] = policyRows.POLICY_NEW_DEAL.Index
		else
			host.slots[slot + 1] = policyRows.POLICY_LIBERALISM.Index
		end
	end
end

local player = { GetCulture = function() return culture end }
local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtimeErr = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtimeErr))

local applyOrder = rawget(_G, "CivvisApplyOrder")
assert(type(applyOrder) == "function", "CivvisApplyOrder was not exported")

local desired = table.concat({
	"POLICY_INTEGRATED_SPACE_CELL", "POLICY_LEVEE_EN_MASSE",
	"POLICY_LIGHTNING_WARFARE", "POLICY_FIVE_YEAR_PLAN", "POLICY_NEW_DEAL",
	"POLICY_PUBLIC_WORKS", "POLICY_CRYPTOGRAPHY", "POLICY_RATIONALISM",
}, ",")
local order = { kind = "policy_deck", verb = desired }

local ok, reason = applyOrder(player, 0, order, 100)
assert(ok and reason == "policy_deck", "initial deck request failed: " .. tostring(reason))
assert(#host.calls == 1, "initial request count: " .. tostring(#host.calls))
assert(host.calls[1].clear[1] == 0 and host.calls[1].clear[8] == 7,
	"full request did not clear every zero-based slot")
assert(host.slots[5] == policyRows.POLICY_LIBERALISM.Index,
	"fixture did not reproduce the partial host apply")

ok, reason = applyOrder(player, 0, order, 100)
assert(not ok and reason == "policy_deck_same_turn",
	"same-turn duplicate was not deferred: " .. tostring(reason))
assert(#host.calls == 1, "same-turn replan raced a second host transaction")

ok, reason = applyOrder(player, 0, order, 101)
assert(ok and reason == "policy_deck_repair",
	"targeted repair failed: " .. tostring(reason))
assert(#host.calls == 2, "repair request count: " .. tostring(#host.calls))
assert(#host.calls[2].clear == 1 and host.calls[2].clear[1] == 4,
	"repair did not target the unwanted zero-based slot 4")
assert(host.calls[2].add[4] == policyRows.POLICY_NEW_DEAL.Hash,
	"repair did not add New Deal to slot 4")
assert(host.slots[5] == policyRows.POLICY_NEW_DEAL.Index,
	"New Deal was not present after targeted repair")

local function lastEvent(kind)
	for i = #LOG, 1, -1 do
		if LOG[i]:find('"kind":"' .. kind .. '"', 1, true) then return LOG[i] end
	end
	return nil
end
assert(lastEvent("policy_deck_deferred") ~= nil, "missing same-turn deferral telemetry")
local request = lastEvent("policy_deck_request")
assert(request ~= nil and request:find('"mode":"repair"', 1, true),
	"missing repair request telemetry")
assert(request:find('"repaired":"POLICY_NEW_DEAL"', 1, true),
	"repair telemetry did not name the missing card")
assert(request:find('"slot":4', 1, true) ~= nil
	and request:find('"current":"POLICY_LIBERALISM"', 1, true) ~= nil,
	"repair telemetry did not include the per-slot readback context")

realPrint("ok   policy deck partial apply is deferred, diagnosed, and repaired")
