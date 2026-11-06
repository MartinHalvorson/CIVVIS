-- Execute the production read-only observer with host APIs; no game required.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local helper = assert(source:match("(CivvisLedger%.annotateStrikeOffer = function.-\nend;)"))
local activity = assert(source:match("(CivvisBoard%.activityName = function.-\nend;)"))

local function fixture()
    local state = { queries = 0, roster_reads = 0 }
    local function unit(id, kind, x, y, owner)
        return {
            GetID = function() return id end,
            GetUnitType = function() return kind end,
            GetOwner = function() return owner or 0 end,
            GetX = function() return x or 8 end, GetY = function() return y or 8 end,
            IsEmbarked = function() return false end,
        }
    end
    state.makeUnit = unit
    state.shooter = unit(77, "archer")
    state.roster = {state.shooter}
    local env = {
        CivvisLedger = {}, CivvisBoard = {},
        ActivityTypes = {ACTIVITY_AWAKE = 1797587246, ACTIVITY_OPERATION = 1225574625},
        UnitOperationResults = {PLOTS = "plots", MODIFIERS = "modifiers", MODIFIER_IS_TARGET = 7},
        UnitManager = {
            GetActivityType = function() return state.activity or 1797587246 end,
            GetOperationTargets = function()
                state.queries = state.queries + 1
                if state.queryError then error("native query failed") end
                if state.nilResults then return nil end
                return state.results or {plots = {}, modifiers = {}}
            end,
            RequestOperation = function() error("an observer must never issue an operation") end,
            RequestCommand = function() error("an observer must never issue a command") end,
        },
        Game = {GetLocalPlayer = function() return 0 end},
        Map = {GetPlot = function() return {GetIndex = function() return 20 end} end},
        GameInfo = {Units = {
            archer = {UnitType = "UNIT_ARCHER", Domain = "DOMAIN_LAND", Combat = 15, RangedCombat = 25},
            warrior = {UnitType = "UNIT_WARRIOR", Domain = "DOMAIN_LAND", Combat = 20, RangedCombat = 0},
            builder = {UnitType = "UNIT_BUILDER", Domain = "DOMAIN_LAND", Combat = 0, RangedCombat = 0},
            galley = {UnitType = "UNIT_GALLEY", Domain = "DOMAIN_SEA", Combat = 30, RangedCombat = 0},
            bomber = {UnitType = "UNIT_BOMBER", Domain = "DOMAIN_AIR", Combat = 0, Bombard = 110},
        }},
        try = function(fn, fallback)
            local ok, value = pcall(fn)
            if ok and value ~= nil then return value end
            return fallback
        end,
    }
    env.Players = setmetatable({[0] = {GetUnits = function()
        state.roster_reads = state.roster_reads + 1
        return {Members = function() return ipairs(state.roster) end}
    end}}, {__index = function() error("do not inspect a foreign player's roster") end})
    setmetatable(env, {__index = _G})
    local chunk = assert(loadstring(activity .. "\n" .. helper)); setfenv(chunk, env); chunk()
    local function observe(payload)
        return env.CivvisLedger.annotateStrikeOffer(state.shooter, 111, 10, 8, payload or {})
    end
    return state, env, observe
end

local cases, failures = 0, {}
local function test(name, fn)
    cases = cases + 1
    local ok, err = pcall(fn)
    if ok then print("PASS " .. name)
    else failures[#failures + 1] = name; print("FAIL " .. name .. ": " .. tostring(err)) end
end
local function unknownTile(payload, reason)
    local c = payload.refusal_context
    assert(c.tile_known == false and c.units == nil and c.x == nil and c.y == nil,
        "a failed read must not publish a partial census or an empty-tile claim")
    assert(c.tile_reason == reason, tostring(c.tile_reason) .. " ~= " .. reason)
end

test("single shooter is a known live tile, rather than an empty census", function()
    local s, _, observe = fixture(); local p = observe(); local c = p.refusal_context
    assert(c.tile_known and c.tile_reason == nil and c.x == 8 and c.y == 8)
    assert(#c.units == 1 and c.units[1].id == 77 and c.units[1].kind == "UNIT_ARCHER")
    assert(c.units[1].domain == "DOMAIN_LAND" and c.units[1].ranged == 25)
    assert(c.units[1].embarked == false and s.roster_reads == 1 and s.queries == 1)
end)
test("live own occupants preserve land civilian sea and air distinctions", function()
    local s, _, observe = fixture()
    s.roster = {s.makeUnit(99, "bomber"), s.makeUnit(81, "galley"), s.shooter,
        s.makeUnit(78, "warrior"), s.makeUnit(80, "builder"), s.makeUnit(1, "unreadable", 9, 8)}
    local c = observe().refusal_context
    assert(c.tile_known and #c.units == 5)
    assert(c.units[1].id == 77 and c.units[2].kind == "UNIT_WARRIOR")
    assert(c.units[3].kind == "UNIT_BUILDER" and c.units[3].combat == 0)
    assert(c.units[4].domain == "DOMAIN_SEA" and c.units[5].domain == "DOMAIN_AIR")
    assert(c.same_layer == nil, "the observer must not turn domain overlap into a causal verdict")
end)
test("activity name is resolved against runtime enums", function()
    local s, _, observe = fixture()
    assert(observe().refusal_context.activity_name == "awake")
    s.activity = 1225574625
    assert(observe().refusal_context.activity_name == "operation")
end)
test("an unrecognized activity retains its raw value", function()
    local s, _, observe = fixture(); s.activity = 1234
    local c = observe().refusal_context
    assert(c.activity == 1234 and c.activity_name == "1234")
end)
test("unavailable activity does not discard a valid tile census", function()
    local _, e, observe = fixture(); e.UnitManager.GetActivityType = nil
    local c = observe().refusal_context
    assert(c.tile_known and c.activity == nil and c.activity_name == nil)
end)
test("missing activity enums do not invent a name", function()
    local _, e, observe = fixture(); e.ActivityTypes = nil
    local c = observe().refusal_context
    assert(c.tile_known and c.activity_name == nil)
end)
test("a foreign shooter cannot expose a foreign roster", function()
    local s, _, observe = fixture(); s.shooter = s.makeUnit(77, "archer", 8, 8, 1)
    unknownTile(observe(), "not_local_unit"); assert(s.roster_reads == 0)
end)
test("a missing player is unknown", function()
    local _, e, observe = fixture(); e.Players = {}
    unknownTile(observe(), "missing_player")
end)
test("a missing unit collection is unknown", function()
    local _, e, observe = fixture(); e.Players[0].GetUnits = function() return nil end
    unknownTile(observe(), "missing_unit_collection")
end)
test("an iterator failure discards partial occupants", function()
    local s, _, observe = fixture()
    s.roster[2] = {GetX = function() error("roster member unavailable") end}
    unknownTile(observe(), "read_error")
end)
test("a shooter missing from the roster is unknown", function()
    local s, _, observe = fixture(); s.roster = {}
    unknownTile(observe(), "shooter_not_in_roster")
end)
test("duplicate occupant identifiers invalidate the census", function()
    local s, _, observe = fixture(); s.roster[2] = s.makeUnit(77, "warrior")
    unknownTile(observe(), "invalid_occupant")
end)
test("a foreign member in our roster invalidates the census", function()
    local s, _, observe = fixture(); s.roster[2] = s.makeUnit(78, "warrior", 8, 8, 1)
    unknownTile(observe(), "invalid_occupant")
end)
test("an unavailable unit definition discards partial occupants", function()
    local s, _, observe = fixture(); s.roster[2] = s.makeUnit(78, "unknown")
    unknownTile(observe(), "missing_unit_definition")
end)
test("an unavailable domain is unknown rather than assumed land", function()
    local _, e, observe = fixture(); e.GameInfo.Units.archer.Domain = nil
    unknownTile(observe(), "missing_unit_definition")
end)
test("a moved shooter cannot publish the earlier tile", function()
    local s, _, observe = fixture(); local reads = 0
    s.shooter.GetX = function() reads = reads + 1; return reads <= 2 and 8 or 9 end
    unknownTile(observe(), "shooter_moved")
end)
test("unplaced shooter coordinates remain unknown", function()
    local s, _, observe = fixture(); s.shooter.GetX = function() return -1 end
    unknownTile(observe(), "invalid_shooter")
end)
test("an optional embark read may be unknown without becoming false", function()
    local s, _, observe = fixture(); s.shooter.IsEmbarked = nil
    local c = observe().refusal_context
    assert(c.tile_known and c.units[1].embarked == nil)
end)
test("an embarked unit preserves its native embark state", function()
    local s, _, observe = fixture(); s.shooter.IsEmbarked = function() return true end
    assert(observe().refusal_context.units[1].embarked == true)
end)
test("target query failure does not discard known occupants", function()
    local s, _, observe = fixture(); s.queryError = true
    local p = observe()
    assert(p.refusal_context.tile_known and not p.native_target_known and p.native_target_reason == "query_error")
end)
test("unavailable occupants do not discard a known offered target", function()
    local s, _, observe = fixture(); s.roster = {}
    s.results = {plots = {20}, modifiers = {7}}
    local p = observe(); unknownTile(p, "shooter_not_in_roster")
    assert(p.native_target_known and p.native_target_offered and p.native_target_reason == nil)
end)
test("nil native results have a named unknown reason", function()
    local s, _, observe = fixture(); s.nilResults = true
    local p = observe()
    assert(not p.native_target_known and p.native_target_reason == "non_table_results")
end)
test("reused payload cannot retain stale positive target fields", function()
    local s, _, observe = fixture(); s.nilResults = true
    local p = observe({native_target_known = true, native_target_offered = true, native_target_rows = 4})
    assert(not p.native_target_known and p.native_target_offered == nil and p.native_target_rows == nil)
end)

print("RESULT " .. cases .. " cases; " .. #failures .. " failed")
assert(#failures == 0, "refused shot context failures: " .. table.concat(failures, ", "))
