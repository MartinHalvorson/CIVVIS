-- The tactical ledger: the mod writes who fought whom, and what it cost.
--
-- ⚠ Loads the SHIPPED `CivvisControlAgent.lua` and drives its own
-- `CivvisLedger` handlers against a fake host, so the record the ledger tool
-- reads is the record the agent writes.
--
-- What is checked:
--   1. a strike order emits `strike` with the host's preview (or an honest
--      nil when the preview API is absent);
--   2. CombatVisBegin/End produce one `combat` event with damage both ways,
--      read back from hit points, and the strike's preview joined on;
--   3. a defender that no longer resolves at End is `defender_killed`;
--   4. UnitDamageChanged deltas inside an open combat are recorded;
--   5. our unit leaving the map emits `unit_lost` with its last known kind
--      and the treasury; another player's does not;
--   5a. a Great Person consumed by activation keeps that witness but is marked
--       with its non-loss cause;
--   5b. our unit TAKEN emits `unit_captured` naming the captor and whether it
--      is the barbarians (`UnitCaptured.lua:8`); a unit we capture does not;
--   6. a city changing hands emits `city_occupation`.
--
-- Run: lua5.1 tools/civ6_control/mod/combat_ledger_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end

local EXPORTS = { CivvisLedger = true }
local LOG = {}
Automation = { Log = function(line) LOG[#LOG + 1] = line end }
ComponentType = { UNIT = 1, CITY = 2, DISTRICT = 3 }
CombatVisType = { ATTACKER = "attacker", DEFENDER = "defender", INTERCEPTOR = "interceptor", ANTI_AIR = "anti_air" }
CombatTypes = { RANGED = 10, BOMBARD = 11, MELEE = 12 }
CombatResultParameters = { ATTACKER = "att", DEFENDER = "def", DAMAGE_TO = "dmg", DEFENSE_DAMAGE_TO = "wall",
	COMBAT_STRENGTH = "str", ID = "id" }
DefenseTypes = { DISTRICT_GARRISON = 1, DISTRICT_OUTER = 2 }
GameInfo = setmetatable({}, { __index = function(_, k)
	if k == "Units" then
		return setmetatable({}, { __index = function(_, name) return { UnitType = name, Combat = 20, RangedCombat = 0 } end })
	end
	return stub()
end })
setmetatable(_G, { __index = function(_, k)
	if EXPORTS[k] then return rawget(_G, k) end
	return stub()
end })

-- ---------------------------------------------------------------- fake host
local host = { units = {}, preview = nil, busy = false, gold = 120 }
local PID = 0
local function unitObject(u)
	return {
		GetID = function() return u.id end,
		GetX = function() return u.x end,
		GetY = function() return u.y end,
		GetDamage = function() return u.damage or 0 end,
		GetUnitType = function() return u.kind end,
		GetRangedCombat = function() return u.ranged or 0 end,
		GetBombardCombat = function() return u.bombard or 0 end,
		GetComponentID = function() return { player = u.player, id = u.id, type = ComponentType.UNIT } end,
	}
end
UnitManager = {
	GetUnit = function(pid, id)
		local u = host.units[pid .. ":" .. id]
		if u == nil or u.gone then return nil end
		return unitObject(u)
	end,
}
UI = { IsGameCoreBusy = function() return host.busy end }
CombatManager = {
	SimulateAttackInto = function(component, combatType, x, y)
		host.lastSim = { component = component, combatType = combatType, x = x, y = y }
		return host.preview
	end,
}
Players = setmetatable({}, { __index = function(_, pid)
	return {
		GetTreasury = function() return { GetGoldBalance = function() return host.gold end } end,
		IsBarbarian = function() return pid == 63 end,
	}
end })
Game = { GetLocalPlayer = function() return PID end, GetCurrentGameTurn = function() return 41 end }
CityManager = {
	GetCity = function(player, id)
		return { GetName = function() return "Ostia" end, GetOriginalOwner = function() return 3 end }
	end,
	GetDistrict = function() return nil end,
}

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtime_err = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtime_err))

local ledger = rawget(_G, "CivvisLedger")
assert(type(ledger) == "table", "CivvisLedger is not exported")

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", name, tostring(got)))
	end
end
local function lastEvent(kind)
	for i = #LOG, 1, -1 do
		if LOG[i]:find('"kind":"' .. kind .. '"', 1, true) then return LOG[i] end
	end
	return nil
end
local function has(line, needle) return line ~= nil and line:find(needle, 1, true) ~= nil end
local function id(player, comp) return { playerID = player, componentID = comp, componentType = ComponentType.UNIT } end

-- Board: our archer 7 at (2,2), their warrior 900 at (4,2).
host.units["0:7"] = { player = 0, id = 7, kind = "UNIT_ARCHER", x = 2, y = 2, damage = 10, ranged = 25 }
host.units["63:900"] = { player = 63, id = 900, kind = "UNIT_WARRIOR", x = 4, y = 2, damage = 0 }
ledger.kinds["7"] = "UNIT_ARCHER"

-- 1. Strike preview.
host.preview = { att = { dmg = 0, str = 25 }, def = { dmg = 31, str = 20, wall = 0 } }
ledger.strike(UnitManager.GetUnit(0, 7), 7, "RANGE_ATTACK", 4, 2, 41)
local strike = lastEvent("strike")
check("strike event emitted", strike ~= nil, true)
check("strike carries the host preview", has(strike, '"damage_to_defender":31'), true)
check("ranged strike simulated as RANGED", host.lastSim.combatType, CombatTypes.RANGED)
check("preview aimed at the target plot", host.lastSim.x == 4 and host.lastSim.y == 2, true)

-- 2. Combat: the archer's shot lands for 34; the warrior does not die.
ledger.onCombatVisBegin({ attacker = id(0, 7), defender = id(63, 900) })
host.units["63:900"].damage = 34
ledger.onUnitDamageChanged(63, 900, 34)  -- first sighting: no previous, no delta recorded
ledger.onCombatVisEnd({ attacker = id(0, 7), defender = id(63, 900) })
local combat = lastEvent("combat")
check("combat event emitted", combat ~= nil, true)
check("damage to defender read back", has(combat, '"damage_to_defender":34'), true)
check("damage to attacker read back", has(combat, '"damage_to_attacker":0'), true)
check("defender not killed", has(combat, '"defender_killed":false'), true)
check("the strike's preview rides on the combat", has(combat, '"preview":{') and has(combat, '"damage_to_defender":31'), true)
check("ours flagged", has(combat, '"ours":true'), true)

-- 3 + 4. Their warrior hits back and dies to our counter next combat; deltas recorded.
ledger.onCombatVisBegin({ attacker = id(63, 900), defender = id(0, 7) })
host.units["0:7"].damage = 45
ledger.onUnitDamageChanged(0, 7, 45)          -- previous unknown -> no delta
ledger.onUnitDamageChanged(0, 7, 52)          -- delta +7 while the combat is open
host.units["63:900"].gone = true
ledger.onCombatVisEnd({ attacker = id(63, 900), defender = id(0, 7) })
combat = lastEvent("combat")
check("defender killed detected when it no longer resolves (attacker here)", has(combat, '"attacker_killed":true'), true)
check("damage events recorded", has(combat, '"delta":7'), true)
check("against_us flagged", has(combat, '"against_us":true'), true)

-- 5. Our unit leaves the map: named with kind and treasury; theirs ignored.
local before = #LOG
ledger.onUnitRemoved(63, 900)
check("another player's unit is not our loss", #LOG, before)
ledger.onUnitRemoved(0, 7)
local lost = lastEvent("unit_lost")
check("unit_lost names the kind", has(lost, '"unit_kind":"UNIT_ARCHER"'), true)
check("unit_lost carries the treasury", has(lost, '"gold":120'), true)

-- 5a. Great Person activation consumes the physical unit, but is not a loss.
ledger.kinds["9"] = "UNIT_GREAT_SCIENTIST"
ledger.expected_gp_activation["9"] = 41
ledger.onUnitRemoved(0, 9)
local activated = lastEvent("unit_lost")
check("activated Great Person keeps the removal witness", activated ~= nil, true)
check("activated Great Person names its cause",
	has(activated, '"cause":"great_person_activation"'), true)

-- 5b. Our settler is TAKEN by the barbarians: the game's own word, ours only.
ledger.kinds["8"] = "UNIT_SETTLER"
before = #LOG
ledger.onUnitCaptured(63, 900, 63, 0)   -- we took theirs: not our loss
check("a unit we capture is not our loss", #LOG, before)
ledger.onUnitCaptured(0, 8, 0, 63)
local captured = lastEvent("unit_captured")
check("unit_captured names the kind", has(captured, '"unit_kind":"UNIT_SETTLER"'), true)
check("unit_captured names the captor", has(captured, '"captor":63'), true)
check("the captor is the barbarians", has(captured, '"captor_is_barbarian":true'), true)
ledger.onUnitCaptured(0, 8, 0, 3)
captured = lastEvent("unit_captured")
check("a major's capture is not barbarian", has(captured, '"captor_is_barbarian":false'), true)

-- 6. A city changes hands.
ledger.onCityOccupationChanged(0, 5)
local occ = lastEvent("city_occupation")
check("city_occupation emitted", has(occ, '"name":"Ostia"') and has(occ, '"ours_now":true'), true)

-- 7. No preview API: an honest nil, not a zero.
host.preview = nil
ledger.strike(UnitManager.GetUnit(0, 7) or unitObject(host.units["0:7"]), 7, "ATTACK", 4, 2, 41)
strike = lastEvent("strike")
check("absent preview is absent", has(strike, '"preview":'), false)

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("\nall combat-ledger checks passed")
