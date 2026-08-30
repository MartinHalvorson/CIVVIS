-- Offline regression for the unit-consolidation and religious-operation verbs.
--
-- Three order shapes that had no test and one that had no verb at all:
--
--   1. FORM_CORPS / FORM_ARMY -- `Action::CombineUnits`, decided 10,015 times
--      across the live archive and sent zero times. Two distinct Firaxis
--      commands, each requested with the PARTNER's owner and id, exactly as the
--      shipped `WorldInput.lua` builds them (`FormCorps` 2879-2882, `FormArmy`
--      2949-2952). A guessed single "combine" verb could never have fired.
--   2. A refused merge is named PER TIER, so `cannot_form_army` against a unit
--      CIVVIS believes is already a Corps is a readable signal that the
--      mirror's formation tier and the host's have diverged.
--   3. REMOVE_HERESY -- and every other parameterless operation on the generic
--      tail -- used to return its own verb for BOTH outcomes, so a refusal was
--      indistinguishable from success in the queue's refusal ledger.
--   4. A theological attack is an ordinary ATTACK verb, and must reach the host
--      as MOVE_TO carrying PARAM_MODIFIERS. There is no theological-combat
--      operation in the shipped `UnitOperations.xml`; the Civilopedia
--      (`Civilopedia_Concepts_Text.xml:636`) says to attack the religious unit
--      like any other, and without the modifier that is a walk, not a strike.
--
-- Run: lua5.1 tools/civ6_control/mod/formation_command_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end

local EXPORTS = { CivvisApplyOrder = true, CivvisResolveActions = true, CivvisLedger = true }
setmetatable(_G, { __index = function(_, key)
	if EXPORTS[key] then return rawget(_G, key); end
	return stub()
end })

local LOG = {}
Automation = { Log = function(line) LOG[#LOG + 1] = line end }

-- `StrikePreview` off keeps the host's combat simulation out of the ATTACK
-- case; the strike is still recorded, which is all this test reads.
CivvisControlConfig = { StrikePreview = false, CapMovesToReach = false }

UnitCommandTypes = { PARAM_UNIT_PLAYER = "owner", PARAM_UNIT_ID = "unit_id" }
UnitOperationTypes = { PARAM_X = "x", PARAM_Y = "y", PARAM_MODIFIERS = "modifiers" }
UnitOperationMoveModifiers = { ATTACK = 4, MOVE_IGNORE_UNEXPLORED_DESTINATION = 8 }

-- Every name the agent resolves gets a distinct, checkable hash.
local HASHES = {}
local function hashFor(name)
	if HASHES[name] == nil then
		HASHES[name] = "hash:" .. name
	end
	return HASHES[name]
end
local function hashTable()
	return setmetatable({}, { __index = function(_, name)
		if type(name) ~= "string" then return nil; end
		return { Type = name, Hash = hashFor(name) }
	end })
end
GameInfo = setmetatable({}, { __index = function(_, key)
	if key == "UnitCommands" or key == "UnitOperations" then return hashTable(); end
	if key == "Units" then
		return setmetatable({}, { __index = function(_, name)
			return { UnitType = tostring(name), Combat = 0, RangedCombat = 0 }
		end })
	end
	return stub()
end })

-- ---------------------------------------------------------------- fake host
local host = { units = {}, allow = {}, calls = {} }
local PID = 0

local function unitObject(u)
	return {
		GetID = function() return u.id end,
		GetX = function() return u.x end,
		GetY = function() return u.y end,
		GetType = function() return u.kind end,
		GetUnitType = function() return u.kind end,
		GetDamage = function() return 0 end,
		GetMovesRemaining = function() return 2 end,
	}
end

UnitManager = {
	GetUnit = function(pid, id)
		local u = host.units[tostring(pid) .. ":" .. tostring(id)]
		if u == nil then return nil; end
		return unitObject(u)
	end,
	CanStartCommand = function(unit, hash, a, b)
		host.calls[#host.calls + 1] = { what = "CanStartCommand", hash = hash, params = a }
		return host.allow[hash] == true
	end,
	RequestCommand = function(unit, hash, params)
		host.calls[#host.calls + 1] = { what = "RequestCommand", hash = hash, params = params }
	end,
	CanStartOperation = function(unit, hash, _, params)
		host.calls[#host.calls + 1] = { what = "CanStartOperation", hash = hash, params = params }
		return host.allow[hash] == true
	end,
	RequestOperation = function(...)
		local argc = select("#", ...)
		local unit, hash, params = ...
		host.calls[#host.calls + 1] = {
			what = "RequestOperation", hash = hash, params = params, argc = argc
		}
	end,
}
Game = { GetLocalPlayer = function() return PID end, GetCurrentGameTurn = function() return 118 end }

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtimeErr = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtimeErr))

local applyOrder = rawget(_G, "CivvisApplyOrder")
assert(type(applyOrder) == "function", "CivvisControlAgent.lua did not export CivvisApplyOrder")
rawget(_G, "CivvisResolveActions")()

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", name, tostring(got)))
	end
end

local function lastCall(what, hash)
	for i = #host.calls, 1, -1 do
		local c = host.calls[i]
		if c.what == what and (hash == nil or c.hash == hash) then return c; end
	end
	return nil
end

local function reset()
	host.calls = {}
	host.allow = {}
end

-- Board: two Swordsmen of ours, 41 at (4,4) and 42 at (5,4), and an Apostle 91.
host.units["0:41"] = { player = 0, id = 41, x = 4, y = 4, kind = "UNIT_SWORDSMAN" }
host.units["0:42"] = { player = 0, id = 42, x = 5, y = 4, kind = "UNIT_SWORDSMAN" }
host.units["0:91"] = { player = 0, id = 91, x = 6, y = 6, kind = "UNIT_APOSTLE" }

local player = stub()

-- 1. A Corps reaches UNITCOMMAND_FORM_CORPS with the partner's owner and id.
reset()
host.allow[hashFor("UNITCOMMAND_FORM_CORPS")] = true
local ok, why = applyOrder(player, PID,
	{ kind = "unit", subject = 41, verb = "FORM_CORPS", x = 0, y = 42 }, 118)
check("corps accepted", ok, true)
check("corps label", why, "FORM_CORPS")
local corps = lastCall("RequestCommand", hashFor("UNITCOMMAND_FORM_CORPS"))
check("corps requested", corps ~= nil, true)
check("corps partner owner", corps and corps.params[UnitCommandTypes.PARAM_UNIT_PLAYER], 0)
check("corps partner id", corps and corps.params[UnitCommandTypes.PARAM_UNIT_ID], 42)

-- 2. An Army is a DIFFERENT command, not the same one with a flag.
reset()
host.allow[hashFor("UNITCOMMAND_FORM_ARMY")] = true
ok, why = applyOrder(player, PID,
	{ kind = "unit", subject = 41, verb = "FORM_ARMY", x = 0, y = 42 }, 118)
check("army accepted", ok, true)
check("army label", why, "FORM_ARMY")
check("army requested", lastCall("RequestCommand", hashFor("UNITCOMMAND_FORM_ARMY")) ~= nil, true)
check("army did not use the corps command",
	lastCall("RequestCommand", hashFor("UNITCOMMAND_FORM_CORPS")), nil)

-- 3. A host refusal is named PER TIER, never anonymous and never silent.
reset()
ok, why = applyOrder(player, PID,
	{ kind = "unit", subject = 41, verb = "FORM_ARMY", x = 0, y = 42 }, 118)
check("refused army is not applied", ok, false)
check("refused army is named", why, "cannot_form_army")
check("refused army sent nothing", lastCall("RequestCommand"), nil)

reset()
ok, why = applyOrder(player, PID,
	{ kind = "unit", subject = 41, verb = "FORM_CORPS", x = 0, y = 42 }, 118)
check("refused corps is named", why, "cannot_form_corps")

-- 4. Half an order is not an order: no partner, and a partner that has died.
reset()
host.allow[hashFor("UNITCOMMAND_FORM_CORPS")] = true
ok, why = applyOrder(player, PID,
	{ kind = "unit", subject = 41, verb = "FORM_CORPS" }, 118)
check("no partner refused", ok, false)
check("no partner named", why, "no_formation_target")

reset()
host.allow[hashFor("UNITCOMMAND_FORM_CORPS")] = true
ok, why = applyOrder(player, PID,
	{ kind = "unit", subject = 41, verb = "FORM_CORPS", x = 0, y = 777 }, 118)
check("dead partner refused", ok, false)
check("dead partner named", why, "formation_target_gone")
check("dead partner sent nothing", lastCall("RequestCommand"), nil)

-- 5. REMOVE_HERESY: a parameterless operation, and a refusal that says so.
reset()
host.allow[hashFor("UNITOPERATION_REMOVE_HERESY")] = true
ok, why = applyOrder(player, PID,
	{ kind = "unit", subject = 91, verb = "REMOVE_HERESY" }, 118)
check("remove heresy accepted", ok, true)
check("remove heresy label", why, "REMOVE_HERESY")
local heresy = lastCall("RequestOperation", hashFor("UNITOPERATION_REMOVE_HERESY"))
check("remove heresy requested", heresy ~= nil, true)
check("remove heresy uses the native parameterless signature",
	heresy and heresy.argc, 2)
check("remove heresy carries no parameter table", heresy and heresy.params, nil)

reset()
ok, why = applyOrder(player, PID,
	{ kind = "unit", subject = 91, verb = "REMOVE_HERESY" }, 118)
check("refused heresy is not applied", ok, false)
check("refused heresy is distinguishable from a successful one",
	why, "cannot_REMOVE_HERESY")

-- 6. Theological combat rides the ordinary ATTACK verb, and MUST carry the
--    move modifier: MOVE_TO without it is a walk that stops beside the target.
reset()
host.allow[hashFor("UNITOPERATION_MOVE_TO")] = true
ok, why = applyOrder(player, PID,
	{ kind = "unit", subject = 91, verb = "ATTACK", x = 6, y = 5 }, 118)
check("theological attack accepted", ok, true)
check("theological attack label", why, "ATTACK")
local strike = lastCall("RequestOperation", hashFor("UNITOPERATION_MOVE_TO"))
check("theological attack is a MOVE_TO", strike ~= nil, true)
check("theological attack aims at the defender",
	strike and strike.params[UnitOperationTypes.PARAM_X], 6)
check("theological attack carries the ATTACK modifier",
	strike and strike.params[UnitOperationTypes.PARAM_MODIFIERS],
	UnitOperationMoveModifiers.ATTACK
		+ UnitOperationMoveModifiers.MOVE_IGNORE_UNEXPLORED_DESTINATION)

-- 7. Both command names must be resolvable from the shipped tables, or the
--    verbs above are dead letters. `resolveActions` reports them.
local resolvedLine = nil
for i = #LOG, 1, -1 do
	if LOG[i]:find('"kind":"actions"', 1, true) then resolvedLine = LOG[i]; break; end
end
check("resolveActions reported", resolvedLine ~= nil, true)
for _, name in ipairs({ "UNITCOMMAND_FORM_CORPS", "UNITCOMMAND_FORM_ARMY",
                        "UNITOPERATION_REMOVE_HERESY", "UNITCOMMAND_CONDEMN_HERETIC" }) do
	check("resolved " .. name,
		resolvedLine ~= nil and resolvedLine:find(name, 1, true) ~= nil, true)
end

if failures > 0 then
	print(string.format("%d check(s) failed", failures))
	os.exit(1)
end
print("all checks passed")
