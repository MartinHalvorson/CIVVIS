-- The actual preview reader follows UnitPanel.lua:1748-1767: final strength
-- is base + modifier, clamped at zero. Observation only; one simulation and
-- no operation request. Missing native fields remain unknown.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local first = assert(source:find("CivvisLedger.preview = function(unit, verb, x, y)", 1, true))
local last = assert(source:find("\nend;", first, true))
local section = source:sub(first, last + #"\nend;" - 1)

local function fixture()
    local state = { calls = 0, requests = 0, busy = false,
        result = { att = { strength = 25, modifier = 0, damage = 0 },
                   def = { strength = 65, modifier = 16, damage = 4, walls = 0 } } }
    local env = {
        tonumber = tonumber, math = math,
        try = function(callback, fallback)
            local ok, value = pcall(callback)
            if ok then return value end
            return fallback
        end,
        UI = { IsGameCoreBusy = function() return state.busy end },
        CombatTypes = { RANGED = 1, BOMBARD = 2 },
        CombatResultParameters = { ATTACKER = "att", DEFENDER = "def", DAMAGE_TO = "damage",
            DEFENSE_DAMAGE_TO = "walls", COMBAT_STRENGTH = "strength", STRENGTH_MODIFIER = "modifier" },
        CivvisLedger = {},
        CombatManager = { SimulateAttackInto = function(component, combatType, x, y)
            state.calls = state.calls + 1
            assert(component == 42 and combatType == 1 and x == 7 and y == 9)
            return state.result
        end },
        UnitManager = { RequestOperation = function()
            state.requests = state.requests + 1
            error("a preview must not request an operation")
        end },
    }
    local chunk
    if setfenv then chunk = assert(loadstring(section)); setfenv(chunk, env)
    else chunk = assert(load(section, "native-preview-reader", "t", env)) end
    chunk()
    local unit = { GetComponentID = function() return 42 end,
        GetRangedCombat = function() return 25 end, GetBombardCombat = function() return 0 end }
    return state, env, function() return env.CivvisLedger.preview(unit, "RANGE_ATTACK", 7, 9) end
end

local s, env, preview = fixture()
local value = preview()
assert(value.attacker_strength == 25 and value.defender_strength == 65, "legacy fields remain base strength")
assert(value.attacker_strength_modifier == 0 and value.defender_strength_modifier == 16)
assert(value.attacker_total_strength == 25 and value.defender_total_strength == 81, "the UI displays the modified totals")
assert(value.damage_to_defender == 4 and value.damage_to_attacker == 0 and value.defender_wall_damage == 0)
assert(s.calls == 1 and s.requests == 0, "no extra simulation or operation")

s.result.att.modifier = -30
s.result.def.modifier = -65
value = preview()
assert(value.attacker_total_strength == 0 and value.defender_total_strength == 0, "totals clamp at zero")
s.result.att.modifier = nil
value = preview()
assert(value.attacker_strength == 25 and value.attacker_strength_modifier == nil and value.attacker_total_strength == nil,
    "missing modifier is unknown, never fabricated zero")
s.result.att.modifier = 3
s.result.att.strength = nil
value = preview()
assert(value.attacker_strength_modifier == 3 and value.attacker_total_strength == nil, "a modifier is not a base")
s.result.att.strength = 25
env.CombatResultParameters.STRENGTH_MODIFIER = nil
value = preview()
assert(value.attacker_strength == 25 and value.attacker_total_strength == nil and value.defender_total_strength == nil,
    "an older native enum leaves totals absent")
s.result = nil
assert(preview() == nil, "unavailable native preview stays unavailable")
local calls = s.calls
s.busy = true
assert(preview() == nil and s.calls == calls, "busy core does not simulate")
assert(s.requests == 0)
print("native preview base/modifier/total observations: PASS")
