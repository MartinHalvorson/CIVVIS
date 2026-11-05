-- Read the real preview function and check the ten categories used by the
-- shipped UnitPanel.lua:1965-1974. No extra simulation or operation.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local f = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = f:read("*a"); f:close()
local first = assert(source:find("CivvisLedger.preview = function(unit, verb, x, y)", 1, true))
local last = assert(source:find("\nend;", first, true))
local section = source:sub(first, last + #"\nend;" - 1)
local keys = {
    "PREVIEW_TEXT_INTERCEPTOR", "PREVIEW_TEXT_ANTI_AIR", "PREVIEW_TEXT_HEALTH",
    "PREVIEW_TEXT_TERRAIN", "PREVIEW_TEXT_OPPONENT", "PREVIEW_TEXT_MODIFIER",
    "PREVIEW_TEXT_ASSIST", "PREVIEW_TEXT_PROMOTION", "PREVIEW_TEXT_DEFENSES",
    "PREVIEW_TEXT_RESOURCES",
}
local E = { ATTACKER = 1, DEFENDER = 2, DAMAGE_TO = 3, COMBAT_STRENGTH = 4,
    STRENGTH_MODIFIER = 5, DEFENSE_DAMAGE_TO = 6 }
for index, key in ipairs(keys) do E[key] = index + 10 end
local s = { calls = 0, operations = 0, busy = false,
    result = { [1] = { [3] = 0, [4] = 25, [5] = 0 },
               [2] = { [3] = 46, [4] = 20, [5] = -6, [6] = 0 } } }
for _, key in ipairs(keys) do
    s.result[1][E[key]] = { "attacker " .. key }
    s.result[2][E[key]] = { "defender " .. key, "quoted \"text\"\n[ICON_Strength] +4" }
end
local env = {
    tonumber = tonumber, math = math, type = type, ipairs = ipairs,
    try = function(callback, fallback)
        local ok, value = pcall(callback)
        if ok then return value end
        return fallback
    end,
    UI = { IsGameCoreBusy = function() return s.busy end },
    CombatTypes = { RANGED = 1, BOMBARD = 2 }, CombatResultParameters = E,
    CivvisLedger = {},
    CombatManager = { SimulateAttackInto = function(component, combatType, x, y)
        s.calls = s.calls + 1
        assert(component == 55 and combatType == 1 and x == 17 and y == 8)
        return s.result
    end },
    UnitManager = { RequestOperation = function()
        s.operations = s.operations + 1; error("preview observation must not operate")
    end },
}
local chunk
if setfenv then chunk = assert(loadstring(section)); setfenv(chunk, env)
else chunk = assert(load(section, "native-preview-breakdown", "t", env)) end
chunk()
local unit = { GetComponentID = function() return 55 end,
    GetRangedCombat = function() return 25 end, GetBombardCombat = function() return 0 end }
local function preview() return env.CivvisLedger.preview(unit, "RANGE_ATTACK", 17, 8) end
local value = preview()
assert(s.calls == 1 and s.operations == 0)
assert(value.attacker_strength == 25 and value.attacker_total_strength == 25)
assert(value.defender_strength == 20 and value.defender_total_strength == 14)
assert(value.damage_to_defender == 46 and value.damage_to_attacker == 0 and value.defender_wall_damage == 0)
for _, key in ipairs(keys) do
    assert(value.attacker_modifier_text[key][1] == "attacker " .. key)
    assert(value.defender_modifier_text[key][1] == "defender " .. key)
    assert(value.defender_modifier_text[key][2] == "quoted \"text\"\n[ICON_Strength] +4")
    assert(value.defender_modifier_text[key] ~= s.result[2][E[key]], "copy the native list")
end
local health = E.PREVIEW_TEXT_HEALTH
s.result[2][health][1] = "changed native value"
assert(value.defender_modifier_text.PREVIEW_TEXT_HEALTH[1] == "defender PREVIEW_TEXT_HEALTH")
value.defender_modifier_text.PREVIEW_TEXT_HEALTH[2] = "changed exported value"
assert(s.result[2][health][2] == "quoted \"text\"\n[ICON_Strength] +4")

-- Optional/malformed groups never fabricate a bonus or discard other groups.
s.result[1][health] = { "known", 4, { nested = true }, false, "later known" }
s.result[2][health] = "not a list"
value = preview()
assert(#value.attacker_modifier_text.PREVIEW_TEXT_HEALTH == 2)
assert(value.attacker_modifier_text.PREVIEW_TEXT_HEALTH[2] == "later known")
assert(value.defender_modifier_text.PREVIEW_TEXT_HEALTH == nil)
assert(value.defender_modifier_text.PREVIEW_TEXT_TERRAIN[1] == "defender PREVIEW_TEXT_TERRAIN")
for _, key in ipairs(keys) do s.result[1][E[key]] = {}; s.result[2][E[key]] = nil end
value = preview()
assert(value.attacker_modifier_text == nil and value.defender_modifier_text == nil)
for _, key in ipairs(keys) do E[key] = nil end
value = preview()
assert(value.attacker_modifier_text == nil and value.defender_modifier_text == nil)
assert(value.damage_to_defender == 46 and value.defender_total_strength == 14)
s.result[1][3] = nil; s.result[2][3] = nil
assert(preview() == nil, "text cannot substitute for an unavailable combat result")
s.result = nil
assert(preview() == nil)
local calls = s.calls; s.busy = true
assert(preview() == nil and s.calls == calls)
assert(s.operations == 0)
print("native preview modifier text observations: PASS")
