-- Offline regression for the war-starter guard on strikes.
--
-- The mod requests ATTACK / CAPTURE / RANGE_ATTACK and the city and
-- encampment strikes directly, which skips the "Declare Surprise War?"
-- question the shipped WorldInput asks through
-- `CombatManager.IsAttackChangeWarState`. Measured on 46 King runs: 28 wars
-- the seat never chose, 21 of them Suzerain Wars from a strike on a
-- city-state's unit. The guard asks the same question and refuses.
--
-- What is checked:
--   1. a strike the host would answer with a war is refused as
--      `would_declare_war:<players>`, nothing is requested, and a
--      `war_refused` event names the unit, verb and plot;
--   2. a strike that changes no war state goes through unchanged, with the
--      attack modifier the melee path depends on;
--   3. CAPTURE, RANGE_ATTACK, the city strike and the air sortie (AIR_ATTACK)
--      are guarded the same way; REBASE and PATROL move an aircraft between
--      friendly plots and never ask the question;
--   4. an absent API answers nil and the strike proceeds (the historical
--      behaviour), never a blanket refusal.
--
-- Run: lua5.1 tools/civ6_control/mod/accidental_war_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end
setmetatable(_G, { __index = function(_, key)
	if key == "CivvisApplyOrder" or key == "CivvisResolveActions" then return rawget(_G, key); end
	return stub()
end })

local LOG = {}
Automation = { Log = function(line) LOG[#LOG + 1] = line end }
CivvisControlConfig = { StrikePreview = false, CapMovesToReach = false }
UnitOperationTypes = { PARAM_X = "x", PARAM_Y = "y", PARAM_MODIFIERS = "mods", MOVE_TO = "MOVE_TO" }
UnitOperationMoveModifiers = { ATTACK = 1, MOVE_IGNORE_UNEXPLORED_DESTINATION = 2 }
CityCommandTypes = { RANGE_ATTACK = "CITY_RANGE" }
Map = {
	GetPlotDistance = function(x1, y1, x2, y2)
		return math.max(math.abs(x1 - x2), math.abs(y1 - y2))
	end,
	GetPlot = function() return { GetOwner = function() return 9 end } end,
}
GameInfo = setmetatable({}, { __index = function(_, k)
	if k == "UnitOperations" or k == "UnitCommands" then
		return setmetatable({}, { __index = function(_, name) return { Hash = name } end })
	end
	if k == "Units" then
		return setmetatable({}, { __index = function(_, name)
			if name == "UNIT_SETTLER" then return { UnitType = name, Combat = 0, RangedCombat = 0 } end
			if name == "UNIT_ARCHER" then return { UnitType = name, Combat = 15, RangedCombat = 25 } end
			return { UnitType = name, Combat = 20, RangedCombat = 0 }
		end })
	end
	return stub()
end })
UI = { IsGameCoreBusy = function() return false end }
Game = { GetCurrentGameTurn = function() return 40 end }

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtimeErr = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtimeErr))

-- The operation hashes are resolved on game load, not at chunk load.
local resolveActions = rawget(_G, "CivvisResolveActions")
assert(type(resolveActions) == "function",
	"CivvisControlAgent.lua did not export CivvisResolveActions")
resolveActions()

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

local function logged(kind)
	local n = 0
	for _, line in ipairs(LOG) do
		if line:find('"kind":"' .. kind .. '"', 1, true) or line:find('"kind": "' .. kind .. '"', 1, true) then
			n = n + 1
		end
	end
	return n
end

-- A fixture: one unit, one city, a host that answers the war question with
-- `answer` (a list of player ids, {} for "no war", or `false` for "no API").
local function fixture(answer)
	local state = { requests = {}, asked = nil }
	local unit = setmetatable({
		GetComponentID = function() return { player = 0, id = 7 } end,
		GetID = function() return 7 end,
		GetX = function() return 9 end,
		GetY = function() return 9 end,
		GetOwner = function() return 0 end,
		GetDamage = function() return 0 end,
		GetUnitType = function() return "UNIT_WARRIOR" end,
		GetRangedCombat = function() return 0 end,
		GetBombardCombat = function() return 0 end,
	}, { __index = function() return function() return stub() end end })
	local city = setmetatable({
		GetComponentID = function() return { player = 0, id = 3 } end,
		GetID = function() return 3 end,
	}, { __index = function() return function() return stub() end end })
	UnitManager = {
		GetUnit = function() return unit end,
		CanStartOperation = function() return true end,
		RequestOperation = function(_, hash, params)
			state.requests[#state.requests + 1] = { hash = hash, params = params }
		end,
	}
	CityManager = {
		GetCity = function() return city end,
		CanStartCommand = function() return true end,
		RequestCommand = function(_, command, params)
			state.requests[#state.requests + 1] = { hash = command, params = params }
		end,
	}
	if answer == false then
		CombatManager = {}
	else
		CombatManager = {
			IsAttackChangeWarState = function(component, x, y)
				state.asked = { component = component, x = x, y = y }
				return answer
			end,
			SimulateAttackInto = function() return nil end,
		}
	end
	return state, unit, city
end

local player = setmetatable({}, { __index = function() return function() return stub() end end })

-- 1. a melee strike that would start a war is refused and named
do
	local state = fixture({ 3 })
	local before = #LOG
	local ok, why = applyOrder(player, 0, { kind = "unit", subject = 7, verb = "ATTACK", x = 10, y = 12 }, 40)
	check("war_attack_refused", ok, false)
	check("war_attack_reason", why, "would_declare_war:3")
	check("war_attack_not_requested", #state.requests, 0)
	check("war_attack_asked_x", state.asked and state.asked.x, 10)
	check("war_attack_asked_y", state.asked and state.asked.y, 12)
	check("war_attack_asked_component", state.asked and state.asked.component.id, 7)
	check("war_attack_event", logged("war_refused") >= 1, true)
	local tail = table.concat(LOG, "\n", before + 1)
	check("war_attack_event_names_player", tail:find('"players":[3]', 1, true) ~= nil, true)
	check("war_attack_event_names_verb", tail:find('"verb":"ATTACK"', 1, true) ~= nil, true)
	check("war_attack_event_names_owner", tail:find('"target_owner":9', 1, true) ~= nil, true)
end

-- 2. a strike that changes no war state goes through with the attack modifier
do
	local state = fixture({})
	local ok, why = applyOrder(player, 0, { kind = "unit", subject = 7, verb = "ATTACK", x = 10, y = 12 }, 40)
	check("peace_attack_sent", ok, true)
	check("peace_attack_verb", why, "ATTACK")
	check("peace_attack_requested", #state.requests, 1)
	check("peace_attack_modifiers", state.requests[1] and state.requests[1].params.mods, 3)
end

-- 3. CAPTURE, RANGE_ATTACK and the city strike are guarded the same way
do
	local state = fixture({ 2, 4 })
	local ok, why = applyOrder(player, 0, { kind = "unit", subject = 7, verb = "CAPTURE", x = 10, y = 12 }, 40)
	check("war_capture_refused", ok, false)
	check("war_capture_reason", why, "would_declare_war:2,4")
	check("war_capture_not_requested", #state.requests, 0)
end
do
	local state = fixture({ 5 })
	local ok, why = applyOrder(player, 0, { kind = "unit", subject = 7, verb = "RANGE_ATTACK", x = 10, y = 12 }, 40)
	check("war_range_refused", ok, false)
	check("war_range_reason", why, "would_declare_war:5")
	check("war_range_not_requested", #state.requests, 0)
end
do
	local state = fixture({})
	local ok, why = applyOrder(player, 0, { kind = "unit", subject = 7, verb = "RANGE_ATTACK", x = 10, y = 12 }, 40)
	check("peace_range_sent", ok, true)
	check("peace_range_requested", #state.requests, 1)
end

-- 3c. the air sortie is a strike and is guarded; REBASE and PATROL move an
--     aircraft between friendly plots and never ask the war question
do
	local state = fixture({ 6 })
	local ok, why = applyOrder(player, 0, { kind = "unit", subject = 7, verb = "AIR_ATTACK", x = 10, y = 12 }, 40)
	check("war_air_refused", ok, false)
	check("war_air_reason", why, "would_declare_war:6")
	check("war_air_not_requested", #state.requests, 0)
end
do
	local state = fixture({})
	local ok, why = applyOrder(player, 0, { kind = "unit", subject = 7, verb = "AIR_ATTACK", x = 10, y = 12 }, 40)
	check("peace_air_sent", ok, true)
	check("peace_air_verb", why, "AIR_ATTACK")
	check("peace_air_requested", #state.requests, 1)
	check("peace_air_operation", state.requests[1] and state.requests[1].hash, "UNITOPERATION_AIR_ATTACK")
	check("peace_air_target_x", state.requests[1] and state.requests[1].params.x, 10)
	check("peace_air_target_y", state.requests[1] and state.requests[1].params.y, 12)
end
do
	local state = fixture({ 6 })
	local ok, why = applyOrder(player, 0, { kind = "unit", subject = 7, verb = "REBASE", x = 10, y = 12 }, 40)
	check("rebase_sent_whatever_the_war_answer", ok, true)
	check("rebase_verb", why, "REBASE")
	check("rebase_never_asked", state.asked, nil)
	check("rebase_operation", state.requests[1] and state.requests[1].hash, "UNITOPERATION_REBASE")
	local sent, verb = applyOrder(player, 0, { kind = "unit", subject = 7, verb = "PATROL", x = 10, y = 12 }, 40)
	check("patrol_sent", sent, true)
	check("patrol_verb", verb, "PATROL")
	check("patrol_never_asked", state.asked, nil)
	check("patrol_operation_is_the_shipped_deploy", state.requests[2] and state.requests[2].hash, "UNITOPERATION_DEPLOY")
end
do
	local state = fixture({ 4 })
	local ok, why = applyOrder(player, 0, { kind = "city_strike", subject = 3, verb = "CITY_STRIKE", x = 10, y = 12 }, 40)
	check("war_city_refused", ok, false)
	check("war_city_reason", why, "would_declare_war:4")
	check("war_city_not_requested", #state.requests, 0)
	check("war_city_asked_component", state.asked and state.asked.component.id, 3)
end
do
	local state = fixture({})
	local ok, why = applyOrder(player, 0, { kind = "city_strike", subject = 3, verb = "CITY_STRIKE", x = 10, y = 12 }, 40)
	check("peace_city_sent", ok, true)
	check("peace_city_verb", why, "CITY_STRIKE")
	check("peace_city_requested", #state.requests, 1)
end

-- 3b. the plain move is guarded too: a MOVE_TO onto a peaceful civilian's
--     plot is a capture the engine declares a war to make
do
	local state = fixture({ 4 })
	local ok, why = applyOrder(player, 0, { kind = "unit", subject = 7, verb = "MOVE_TO", x = 10, y = 12 }, 40)
	check("war_move_refused", ok, false)
	check("war_move_reason", why, "would_declare_war:4")
	check("war_move_not_requested", #state.requests, 0)
	check("war_move_asked_x", state.asked and state.asked.x, 10)
end
do
	local state = fixture({})
	local ok, why = applyOrder(player, 0, { kind = "unit", subject = 7, verb = "MOVE_TO", x = 10, y = 12 }, 40)
	check("peace_move_sent", ok, true)
	check("peace_move_requested", #state.requests, 1)
	check("peace_move_no_attack_modifier", state.requests[1] and state.requests[1].params.mods, nil)
end

-- 4. an absent API never refuses
do
	local state = fixture(false)
	local ok, why = applyOrder(player, 0, { kind = "unit", subject = 7, verb = "ATTACK", x = 10, y = 12 }, 40)
	check("no_api_attack_sent", ok, true)
	check("no_api_attack_requested", #state.requests, 1)
end

-- The guard is wired into the shipped file, not only into this harness.
do
	local src = assert(io.open(here .. "/CivvisControlAgent.lua")):read("*a")
	check("source_asks_the_host", src:find("CombatManager.IsAttackChangeWarState(actor:GetComponentID(), x, y)", 1, true) ~= nil, true)
	local guards = 0
	for _ in src:gmatch("CivvisLedger%.refuseWarStarter%(") do guards = guards + 1 end
	-- ATTACK/MOVE_TO/CAPTURE (one call), RANGE_ATTACK, AIR_ATTACK, the city
	-- strike and the encampment strike.
	check("source_guards_four_arms_the_assault_and_the_sortie", guards, 6)
end

if failures > 0 then
	print(string.format("%d failure(s)", failures))
	os.exit(1)
end
print("accidental_war_test: all checks passed")
