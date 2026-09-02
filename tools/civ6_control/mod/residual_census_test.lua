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
	CivvisChooseSpyEscapeRoute = true,
}

setmetatable(_G, { __index = function(_, k)
	if EXPORTS[k] then return rawget(_G, k); end
	return stub();
end })

local request = nil
PlayerOperations = {
	PARAM_DISTRICT_TYPE = "district_type",
	SET_ESCAPE_ROUTE = "set_escape_route",
}
GameInfo = {
	Districts = {
		DISTRICT_CITY_CENTER = { Index = 17 },
	},
}
UI = {
	RequestPlayerOperation = function(pid, operation, parameters)
		request = { pid = pid, operation = operation, parameters = parameters }
	end,
}

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

-- 0. The unattended escape uses the same native operation as the shipped
-- `EspionageEscape.lua:29-33` fourth button, with the city-center district
-- index rather than a UI click that may never open the popup.
local escape = rawget(_G, "CivvisChooseSpyEscapeRoute")
assert(type(escape) == "function", "CivvisChooseSpyEscapeRoute is not exported")
assert(escape(7) == true, "spy escape operation was not accepted by the host")
assert(request ~= nil and request.pid == 7,
	"spy escape request did not target the local player")
assert(request.operation == "set_escape_route",
	"spy escape used the wrong player operation")
assert(request.parameters.district_type == 17,
	"spy escape did not select the shipped city-center route")

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
assert(agentSrc:find('answered%s*=%s*answered%s*%.%.%s*"%+parked:"%s*%.%.%s*parked'),
	"a parked answer must be distinguishable from a bare civvis_complete")
local unitsStart = agentSrc:find('answered = "civvis_complete";', 1, true)
local policyStart = agentSrc:find(
	"-- ⚠⚠⚠ THE SAME CLAIM-NOT-CHECK DEFECT, ON THE POLICY SLOT.",
	unitsStart or 1, true)
assert(unitsStart and policyStart and policyStart > unitsStart,
	"the first CIVVIS units answer arm was not found")
local unitsAnswer = agentSrc:sub(unitsStart, policyStart)
local parkedAt = unitsAnswer:find("parkReadyUnits(player)", 1, true)
local dismissedAt = unitsAnswer:find("dismissBlocker(pid, blocker)", 1, true)
local forcedAt = unitsAnswer:find('REASON = "UserForced"', 1, true)
assert(parkedAt and dismissedAt and forcedAt
	and parkedAt < dismissedAt and dismissedAt < forcedAt,
	"the first CIVVIS units answer must park, dismiss, and force in one pass")
assert(unitsAnswer:find("same_pass_forced = true", 1, true),
	"a same-pass forced units answer must skip the ordinary end-turn request")
-- Parking must never be a MOVE: that is what the branch's own comment forbids,
-- after the legacy AI walked a Settler into a barbarian capture zone.
local idle = agentSrc:match("local function orderIdle%(unit%)(.-)\nend")
assert(idle, "orderIdle not found")
assert(not idle:find("MOVE_TO"), "orderIdle must not move a unit")
for _, op in ipairs({ "SKIP_TURN", "FORTIFY", "ALERT", "SLEEP" }) do
	assert(idle:find(op, 1, true), "orderIdle must offer " .. op)
end
print("units blocker: parks ready units before claiming completion, holding orders only")

--- ⚠⚠⚠ THE SAME CLAIM-NOT-CHECK DEFECT, MEASURED AGAIN ON THE POLICY SLOT.
--- Run civvis-20260901T230916Z wedged at turns 184--208 after the full deck
--- request returned pcall_ok=true while one Economic slot stayed empty:
---     policy_deck_deferred  why=same_turn_transaction_in_flight
---     blocked               FILL_CIVIC_SLOT  civvis_complete
--- The hard blocker stopped the next board publication, so its old
--- second-sighting forfeit could not run. The same pass must force a fresh
--- turn after CIVVIS has answered, without racing a second policy request.
local productionStart = agentSrc:find(
	"-- ⚠⚠⚠ AND THE SAME THING AGAIN ON PRODUCTION, WHICH PARKS THE",
	policyStart or 1, true)
assert(productionStart and productionStart > (policyStart or 0),
	"the policy blocker arm boundary was not found")
local policyArm = agentSrc:sub(policyStart, productionStart)
local policyCompleteStart, policyCompleteEnd = policyArm:find(
	'if answered == "civvis_complete" then%s+local dropped = dismissBlocker%(pid, blocker%)')
assert(policyCompleteStart and policyCompleteEnd,
	"a CIVVIS-complete policy blocker must dismiss in the same pass")
local policyDismiss = policyArm:find("dismissBlocker(pid, blocker)", 1, true)
local policyForce = policyArm:find('REASON = "UserForced"', 1, true)
local policySamePass = policyArm:find("same_pass_forced = true", 1, true)
assert(policyDismiss and policyForce and policyDismiss < policyForce,
	"a CIVVIS-complete policy blocker must force the end turn")
assert(policySamePass and policyForce and policySamePass < policyForce,
	"a forced policy answer must mark the same pass")
local policyElseStart = policyArm:find('else%s+local filled = fillPolicies%(player%)', policyCompleteEnd or 1)
assert(policyElseStart,
	"the policy filler must remain on the non-racing path")
assert(not policyArm:sub(policyCompleteEnd + 1, policyElseStart - 1):find("fillPolicies(player)", 1, true),
	"the CIVVIS-complete policy path must not race a second same-turn request")
print("policy slot: forces a CIVVIS-complete hard blocker and preserves non-racing fill")

--- ⚠⚠⚠ AND THE SAME SHAPE ON PRODUCTION, WHICH COSTS THE WHOLE GAME.
--- A city with nothing queued is something end-turn genuinely requires, so
--- `civvis_complete` is a claim the engine does not accept — and unlike the
--- policy slot it is not merely re-raised. The Game Core stops publishing while
--- it waits, and the agent, driven only by `GameCoreEventPublishComplete`,
--- never ticks again. Nothing recovers that: an external forced end turn was
--- measured and ignored twice.
--- Run civvis-20260830T074021Z parked at t87 on `ENDTURN_BLOCKING_PRODUCTION`
--- answered `civvis_complete` at attempts=1, with `Ravenna producing nil`.
assert(agentSrc:find('if name == "ENDTURN_BLOCKING_PRODUCTION" then%s+local set = driveProduction%(player, turn, true%)'),
	"the production blocker must actually set production before claiming completion")
assert(agentSrc:find('answered%s*=%s*answered%s*%.%.%s*"%+produced:"%s*%.%.%s*set'),
	"a produced answer must be distinguishable from a bare civvis_complete")
print("production: sets an empty city before claiming completion")

--- ⚠⚠⚠ THE FORFEIT'S FORCED END TURN MUST NOT BE GATED ON THE UNIT BLOCKERS.
--- `ACTION_ENDTURN` with no reason is refused while a blocker stands, and
--- dismissing does not stick for anything end-turn genuinely requires, so a
--- forfeited blocker that is not a unit blocker left the turn unable to end.
--- Dismissals across the 2026-08-28/29 ladder runs: 26 forced (all UNITS), and
--- 39 world-congress, 24 policy-slot, 12 envoy-token, 6 great-person NOT forced.
--- Run civvis-20260829T032446Z shows both halves: t88 dismissed UNITS with
--- `parked=0`, forced, and advanced; t94 dismissed GIVE_INFLUENCE_TOKEN unforced
--- and never played another turn.
local forfeitArm = agentSrc:match("forced = not holdForVote %}%);(.-)elseif not residual_taken")
assert(forfeitArm, "the forfeit dismissal arm was not found")
assert(forfeitArm:find('REASON = "UserForced"', 1, true),
	"a forfeited blocker must force the end turn")
--- Spelled in two halves on purpose. The ladder-type gate in `tests.yml`
--- strips comments and then reads any bare `"UNIT_…"` string literal as a
--- Civilization VI unit type; this one names a Lua table, so shipping it whole
--- in #2730 turned `main` red on `control-mod`.
local unitBlockerTable = "UNIT" .. "_BLOCKERS"
assert(not forfeitArm:find(unitBlockerTable, 1, true),
	"the forced end turn must not be gated on the unit-blocker table")
print("forfeit: every dismissed blocker forces the end turn")

--- ⚠⚠⚠ EXCEPT THE CONGRESS SESSION, WHICH DEFERS ITS BALLOT ONE CYCLE.
--- Forfeit 1 waits for the stage-1/popup ballot; only forfeit 2 falls back to
--- vote-and-submit. Forcing the turn at forfeit 1 ends it before either can
--- happen, so the session is dismissed unvoted every time -- and this seat plays
--- for a DIPLOMATIC victory, where those votes are the win condition. The hold
--- lifts as soon as the ballot is cast for the turn.
assert(forfeitArm:find("holdForVote", 1, true),
	"the forced end turn must be held while a congress ballot is still owed")
local hold = agentSrc:match("local holdForVote = (.-);\n")
assert(hold, "holdForVote is not defined")
assert(hold:find("ENDTURN_BLOCKING_WORLD_CONGRESS_SESSION", 1, true),
	"the hold must name the congress session")
assert(hold:find("voted_turn", 1, true),
	"the hold must lift once the ballot is cast for this turn")
print("forfeit: the congress session is not forced before its ballot")

-- A live era prompt is owned by CIVVIS, but it can only be owned when the
-- mirrored board carries Firaxis's actual remaining choice count.  Older
-- exports left the model at zero, so it emitted no dedication order while the
-- ownership guard returned `civvis_complete` forever. Keep both halves: the
-- export makes the normal CIVVIS order possible and the residual bridge clears
-- a prompt that still survives a completed reply.
local dedicationCount = agentSrc:find("dedication_choices = try(function()", 1, true)
local dedicationAllowance = dedicationCount
	and agentSrc:find("GetPlayerNumAllowedCommemorations(pid)", dedicationCount, true)
assert(dedicationCount and dedicationAllowance and dedicationCount < dedicationAllowance,
	"the state export must carry the native dedication allowance")
assert(agentSrc:find('or name == "ENDTURN_BLOCKING_COMMEMORATION_AVAILABLE" then', 1, true),
	"a standing owned commemoration must re-enter the native residual ladder")
print("dedication: exports the allowance and bridges a standing completed prompt")

print("blocker ownership: " ..
	#(function() local n = {} for _ in pairs(answers) do n[#n + 1] = 1 end return n end)() ..
	" answered prompts owned, no soft overlap, every order kind real")
