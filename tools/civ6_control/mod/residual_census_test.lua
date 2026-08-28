-- Offline test for the blocker-ownership tables and the residual census.
--
-- ⚠ THE OWNED LIST WAS MAINTAINED BY NOTICING, AND NOTICING TOOK MONTHS.
-- Every name `CIVVIS_OWNED_BLOCKERS` has gained arrived the same way: a prompt
-- CIVVIS issues orders for was answered by the hand-written ladder for a long
-- time, somebody eventually read a log, and the name was added. The pantheon
-- and the civic slot in #1465; the government change in this change, whose
-- predecessor comment had written down the measurement that would justify it
-- and then waited for a human to come back and take it.
--
-- `CIVVIS_ANSWERS_PROMPT` writes the join down — prompt to the CIVVIS order
-- kind that answers it — and this test enforces it, so the next order kind that
-- answers a prompt fails the gate until the prompt is claimed.
--
-- Run: lua5.1 tools/civ6_control/mod/residual_census_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end

local EXPORTS = {
	CivvisOwnedBlockers = true,
	CivvisAnswersPrompt = true,
	CivvisSoftBlockers = true,
	CivvisResidualBucket = true,
}

setmetatable(_G, { __index = function(_, k)
	if EXPORTS[k] then return rawget(_G, k); end
	return stub();
end })

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))

-- ⚠⚠ REPORT THE PCALL RESULT. A chunk that DIES at load must fail this test,
-- not pass it because the export happened first. See envoy_spend_test.lua.
local ran, runtime_err = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtime_err))

local owned = rawget(_G, "CivvisOwnedBlockers")
local answers = rawget(_G, "CivvisAnswersPrompt")
local soft = rawget(_G, "CivvisSoftBlockers")
assert(type(owned) == "table", "CivvisOwnedBlockers is not exported")
assert(type(answers) == "table", "CivvisAnswersPrompt is not exported")
assert(type(soft) == "table", "CivvisSoftBlockers is not exported")

-- 1. Every prompt CIVVIS answers is a prompt CIVVIS owns.
--
-- This is the whole point. A prompt with a CIVVIS order behind it that is not
-- owned gets raced by the hand-written ladder on first sight, and the race is
-- silent: the turn ends, `orders_source` still reads `civvis`, and only a
-- residual census read months later says a second AI decided.
for prompt, kind in pairs(answers) do
	assert(owned[prompt],
		"CIVVIS emits `" .. kind .. "` orders that answer " .. prompt ..
		", but the prompt is not in CIVVIS_OWNED_BLOCKERS — the hand-written " ..
		"ladder will answer it first and win the race. Add it there.")
end

-- 2. Owned and soft are different mechanisms and must not overlap.
--
-- A soft blocker short-circuits in the caller with `civvis_complete` BEFORE
-- `answerBlocker` runs; an owned one is short-circuited inside it. A name in
-- both is a name whose second mechanism is dead code, and dead code in this
-- table reads as protection that is not there.
for prompt in pairs(owned) do
	assert(not soft[prompt],
		prompt .. " is in both CIVVIS_OWNED_BLOCKERS and SOFT_BLOCKERS. They " ..
		"are two different ways to keep the heuristics off a decision and the " ..
		"soft arm wins, so the owned entry never runs.")
end

-- 3. Prompt names are real. A blocker name the game has no type for is
--    invisible: the prompt is never matched, the ladder answers it anyway, and
--    nothing anywhere says the entry can never fire. Same class as the
--    `BUILDING_ANCIENT_WALLS` trap the tests workflow guards for build types.
for _, table_under_test in ipairs({ owned, answers, soft }) do
	for prompt in pairs(table_under_test) do
		assert(prompt:match("^ENDTURN_BLOCKING_[A-Z0-9_]+$"),
			"not an end-turn blocker name: " .. tostring(prompt))
	end
end

-- 4. The order kinds named here are kinds the order channel actually emits.
--    Checked against `src/bin/civvis_orders.rs`, which is the only writer.
local orders_rs = here .. "/../../../src/bin/civvis_orders.rs"
local handle = io.open(orders_rs, "r")
assert(handle, "could not read " .. orders_rs)
local source = handle:read("*a")
handle:close()
local emitted = {}
for kind in source:gmatch('kind:%s*"([a-z_]+)"') do emitted[kind] = true; end
assert(next(emitted) ~= nil, "found no order kinds in civvis_orders.rs")
for prompt, kind in pairs(answers) do
	assert(emitted[kind],
		"CIVVIS_ANSWERS_PROMPT maps " .. prompt .. " to order kind `" .. kind ..
		"`, which civvis_orders.rs never emits. A mapping to a kind that is " ..
		"never sent claims a prompt CIVVIS cannot actually answer.")
end

-- 5. The census tells the three outcomes apart.
--
-- One flat number over all three reads as the leak. On 2026-08-17 a review of
-- 14 runs read 1,577 residuals as "1,577 decisions taken by the Lua fallback
-- instead of CIVVIS" and had to be withdrawn: 937 were the bounded escape after
-- CIVVIS had answered, ~350 were declines that decided nothing, and the real
-- leak was 3. These three cases are what keeps that mistake from being
-- available to the next reader.
local bucket = rawget(_G, "CivvisResidualBucket")
assert(type(bucket) == "function", "CivvisResidualBucket is not exported")
assert(bucket(nil, false) == "declined", "no answer must not count as a decision")
assert(bucket(nil, true) == "declined", "no answer is a decline even on the escape")
assert(bucket("research", true) == "after_civvis",
	"an answer AFTER CIVVIS answered is the designed escape, not a leak")
assert(bucket("research", false) == "unasked",
	"an answer on a prompt CIVVIS was never asked about IS the leak")

-- ⚠⚠⚠ `civvis_complete` WAS A CLAIM, NOT A CHECK.
--
-- Under `CivvisDecides` a soft blocker is answered `civvis_complete` on the
-- premise that CIVVIS already made its COMPLETE unit-order pass. That premise
-- holds for units CIVVIS mentioned. A unit can go ready again inside the same
-- turn — it finishes the walk the opening board gave it — and a REPLAN FRAME
-- deliberately does not re-run the unassigned pass, so nothing dispositions it.
--
-- Measured 2026-08-28, run civvis-20260828T190111Z at turn 113, 7 cities and
-- 33 units, the furthest game of the day:
--     frame 0  expl=20 civskip=3    (everything dispositioned)
--     frame 1  6 movers replanned, expl=0
--     blocked ENDTURN_BLOCKING_UNITS answered="civvis_complete"
-- and the game never advanced again.
--
-- So the three unit-family blockers must PARK the still-ready units before
-- claiming completion. `UNIT_BLOCKERS` already existed for the forfeit path and
-- its own comment calls these "the ones whose forfeit needs the parking pass";
-- the pass simply was not reached until the forfeit.
local agentSrc = io.open(here .. "/CivvisControlAgent.lua"):read("*a")
assert(agentSrc:find("if UNIT_BLOCKERS%[name%] then%s+local parked = parkReadyUnits%(player%)"),
	"the units blocker must park ready units before answering civvis_complete")
assert(agentSrc:find('"civvis_complete%+parked:"'),
	"a parked answer must be distinguishable from a bare civvis_complete")
-- Parking must never be a MOVE: that is what the branch's own comment forbids,
-- after the legacy AI walked a Settler into a barbarian capture zone.
local idle = agentSrc:match("local function orderIdle%(unit%)(.-)\nend")
assert(idle, "orderIdle not found")
assert(not idle:find("MOVE_TO"), "orderIdle must not move a unit")
for _, op in ipairs({ "SKIP_TURN", "FORTIFY", "ALERT", "SLEEP" }) do
	assert(idle:find(op, 1, true), "orderIdle must offer " .. op)
end
print("units blocker: parks ready units before claiming completion, holding orders only")

print("blocker ownership: " ..
	#(function() local n = {} for _ in pairs(answers) do n[#n + 1] = 1 end return n end)() ..
	" answered prompts owned, no soft overlap, every order kind real")
