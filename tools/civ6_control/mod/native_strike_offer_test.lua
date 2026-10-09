-- Observe the shipped refusal handlers. Native WorldInput.lua:2004-2015 and
-- :2091-2097 offer only PLOTS rows paired with MODIFIER_IS_TARGET.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local applyStart = assert(source:find("local function applyOrder(", 1, true))
local first = assert(source:find('\t\tif verb == "RANGE_ATTACK" then', applyStart, true))
local last = assert(source:find('\t\tif verb == "IMPROVE"', first, true))
local section = source:sub(first, last - 1)

local function fixture(verb)
    local state = { queries = 0, requests = 0, emitted = {}, allowed = false,
        target = 20, results = { plots = {10, 20, 30}, modifiers = {0, 7, 0} } }
    state.unit = {
        GetX = function() return 8 end, GetY = function() return 8 end,
        GetMovesRemaining = function() return 2 end,
        GetAttacksRemaining = function() return 1 end,
    }
    local env = {
        UnitOperationTypes = { PARAM_X = "x", PARAM_Y = "y" },
        UnitOperationResults = { PLOTS = "plots", MODIFIERS = "modifiers", MODIFIER_IS_TARGET = 7 },
        OP = { UNITOPERATION_RANGE_ATTACK = 111, UNITOPERATION_AIR_ATTACK = 222 },
        Map = { GetPlot = function(x, y)
            assert(x == 10 and y == 8, "the native target plot must match the refused order")
            if state.missingPlot then return nil end
            return { GetIndex = function()
                if state.badIndex then error("plot index unavailable") end
                return state.target
            end }
        end },
        CivvisLedger = {
            refuseWarStarter = function() return state.war end,
            refuseLethalPreview = function() return nil end,
            strike = function() end,
        },
        CivvisFrames = { startStrike = function() return 1 end, finishStrike = function() end },
        unitTypeName = function() return verb == "AIR_ATTACK" and "UNIT_BOMBER" or "UNIT_ARCHER" end,
        refusalReason = function() return "can_start=false,no_reasons [p4r]" end,
        try = function(callback, fallback)
            local ok, value = pcall(callback)
            if ok and value ~= nil then return value end
            return fallback
        end,
        emit = function(kind, payload) state.emitted[#state.emitted + 1] = {kind, payload} end,
        canOperate = function() return state.allowed end,
        operate = function(unit, operation, params)
            assert(unit == state.unit and params.x == 10 and params.y == 8)
            assert(operation == (verb == "AIR_ATTACK" and 222 or 111))
            if state.allowed then state.requests = state.requests + 1 end
            return state.allowed
        end,
    }
    env.UnitManager = {
        GetActivityType = function() return "awake" end,
        GetOperationTargets = function(...)
            assert(select("#", ...) == 2, "use the shipped two-argument target-enumeration form")
            local unit, operation = ...
            assert(unit == state.unit, "query the actual refused unit")
            assert(operation == (verb == "AIR_ATTACK" and 222 or 111), "query the actual operation")
            state.queries = state.queries + 1
            if state.throw then error("native target query unavailable") end
            return state.results
        end,
    }
    setmetatable(env, { __index = _G })
    -- Optional on the unchanged source: the old handlers still execute and
    -- report their old fields, so every new diagnostic case gets an outcome.
    local helper = source:match("(CivvisLedger%.annotateStrikeOffer = function.-\nend;)")
    if helper then
        local define = assert(loadstring(helper)); setfenv(define, env); define()
    end
    local chunk = assert(loadstring("return function(verb, x, y)\n"
        .. "local unit, subject, pid, turn, row = TEST_UNIT, 77, 0, 180, {}\n"
        .. section .. "\nreturn false, 'unknown_order'\nend"))
    env.TEST_UNIT = state.unit
    setfenv(chunk, env)
    return state, env, chunk()
end

local function refused(state, apply, verb)
    local ok = apply(verb, 10, 8)
    assert(not ok and state.requests == 0, "a refused strike must submit no new operation")
    assert(#state.emitted == 1 and state.emitted[1][1] == "range_attack_refused")
    local payload = state.emitted[1][2]
    assert(payload.unit == 77 and payload.turn == 180 and payload.x == 10 and payload.y == 8)
    assert(payload.unit_x == 8 and payload.unit_y == 8 and payload.moves == 2 and payload.attacks == 1)
    assert(payload.why == "can_start=false,no_reasons [p4r]", "retain the original refusal reason")
    return payload
end

local function unknown(payload)
    assert(payload.native_target_known == false, "failed reads must be explicitly unknown")
    assert(payload.native_target_offered == nil and payload.native_target_rows == nil,
        "an unknown read must not claim a negative offer or zero targets")
end

local failures, cases = {}, 0
local function test(name, callback)
    cases = cases + 1
    local ok, err = pcall(callback)
    if ok then print("PASS " .. name)
    else failures[#failures + 1] = name; print("FAIL " .. name .. ": " .. tostring(err)) end
end

for _, verb in ipairs({"RANGE_ATTACK", "AIR_ATTACK"}) do
    test(verb .. " refusal records the native offered target", function()
        local s, _, apply = fixture(verb)
        local p = refused(s, apply, verb)
        assert(p.native_target_known == true and p.native_target_offered == true)
        assert(p.native_target_rows == 1 and s.queries == 1)
        assert(p.native_targets == nil and p.native_target_plots == nil, "do not export a target list")
    end)
    test(verb .. " accepted shot performs no diagnostic target query", function()
        local s, _, apply = fixture(verb); s.allowed = true
        assert(apply(verb, 10, 8))
        assert(s.requests == 1 and s.queries == 0 and #s.emitted == 0)
    end)
end

test("raw PLOTS membership with a non-target modifier is not an offered attack", function()
    local s, _, apply = fixture("RANGE_ATTACK"); s.target = 30
    local p = refused(s, apply, "RANGE_ATTACK")
    assert(p.native_target_known == true and p.native_target_offered == false)
    assert(p.native_target_rows == 1 and s.queries == 1)
end)
test("a missing target in valid native rows is a known absent offer", function()
    local s, _, apply = fixture("RANGE_ATTACK"); s.target = 40
    local p = refused(s, apply, "RANGE_ATTACK")
    assert(p.native_target_known == true and p.native_target_offered == false and p.native_target_rows == 1)
end)
test("valid empty native rows contain no offered attack", function()
    local s, _, apply = fixture("RANGE_ATTACK"); s.results = {plots = {}, modifiers = {}}
    local p = refused(s, apply, "RANGE_ATTACK")
    assert(p.native_target_known == true and p.native_target_offered == false and p.native_target_rows == 0)
end)
test("a duplicate plot with one target modifier remains offered", function()
    local s, _, apply = fixture("RANGE_ATTACK")
    s.results = {plots = {20, 20, 30}, modifiers = {0, 7, 7}}
    local p = refused(s, apply, "RANGE_ATTACK")
    assert(p.native_target_known == true and p.native_target_offered == true and p.native_target_rows == 2)
end)

test("missing native query API stays unknown", function()
    local s, env, apply = fixture("RANGE_ATTACK"); env.UnitManager.GetOperationTargets = nil
    unknown(refused(s, apply, "RANGE_ATTACK")); assert(s.queries == 0)
end)
test("a throwing native query stays unknown", function()
    local s, _, apply = fixture("RANGE_ATTACK"); s.throw = true
    unknown(refused(s, apply, "RANGE_ATTACK")); assert(s.queries == 1)
end)
for _, fault in ipairs({"all enums", "target modifier", "plot key", "modifier key"}) do
    test("missing " .. fault .. " stays unknown without a target query", function()
        local s, env, apply = fixture("RANGE_ATTACK")
        if fault == "all enums" then env.UnitOperationResults = nil
        elseif fault == "target modifier" then env.UnitOperationResults.MODIFIER_IS_TARGET = nil
        elseif fault == "plot key" then env.UnitOperationResults.PLOTS = nil
        else env.UnitOperationResults.MODIFIERS = nil end
        unknown(refused(s, apply, "RANGE_ATTACK")); assert(s.queries == 0)
    end)
end
for _, fault in ipairs({"missing plots", "missing modifiers", "misaligned rows", "unreadable results"}) do
    test(fault .. " stays unknown", function()
        local s, _, apply = fixture("RANGE_ATTACK")
        if fault == "missing plots" then s.results.plots = nil
        elseif fault == "missing modifiers" then s.results.modifiers = nil
        elseif fault == "misaligned rows" then s.results.modifiers = {0}
        else s.results = false end
        unknown(refused(s, apply, "RANGE_ATTACK")); assert(s.queries == 1)
    end)
end
for _, fault in ipairs({"missing plot", "unreadable index"}) do
    test(fault .. " stays unknown without a native target query", function()
        local s, _, apply = fixture("RANGE_ATTACK")
        if fault == "missing plot" then s.missingPlot = true else s.badIndex = true end
        unknown(refused(s, apply, "RANGE_ATTACK")); assert(s.queries == 0)
    end)
end
test("a diplomatic refusal performs no diagnostic target query", function()
    local s, _, apply = fixture("RANGE_ATTACK"); s.war = "would_declare_war:2"
    local ok, why = apply("RANGE_ATTACK", 10, 8)
    assert(not ok and why == s.war and s.queries == 0 and s.requests == 0 and #s.emitted == 0)
end)

print("RESULT " .. cases .. " cases; " .. #failures .. " failed")
assert(#failures == 0, "native strike offer failures: " .. table.concat(failures, ", "))
