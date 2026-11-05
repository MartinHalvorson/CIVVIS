-- Execute the production melee handler with the native request/path calls stubbed.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local helper = assert(source:match("(CivvisLedger%.annotateMeleePath = function.-\nend;)"))
local strike = assert(source:match("(CivvisLedger%.strike = function.-\nend;)"))
local start = assert(source:find('\t\tif verb == "MOVE_TO" or verb == "ATTACK" or verb == "CAPTURE" then', 1, true))
local finish = assert(source:find('\t\tif verb == "RANGE_ATTACK" then', start, true))
local section = source:sub(start, finish - 1)

local function fixture()
    local state = { queries = 0, requests = 0, previews = 0, emitted = {}, modifiers = 3,
        path = { plots = {10, 20, 30}, turns = {1, 1, 2} } }
    state.unit = {
        GetX = function() return 7 end, GetY = function() return 23 end,
        GetMovesRemaining = function() return 2 end,
        GetAttacksRemaining = function() return 1 end,
        GetUnitType = function() return "UNIT_WARRIOR" end,
        GetDamage = function() return 40 end,
    }
    local plots = { [10] = {7, 23}, [20] = {8, 24}, [30] = {8, 23} }
    local env = {
        cfg = { StrikePreview = false }, OP = { UNITOPERATION_MOVE_TO = 123 },
        UnitOperationTypes = { PARAM_X = "x", PARAM_Y = "y", PARAM_MODIFIERS = "modifiers" },
        GameInfo = { Units = { UNIT_WARRIOR = {UnitType = "UNIT_WARRIOR"} } },
        CivvisLedger = { pending = {},
            attackModifiers = function() return state.modifiers end,
            refuseWarStarter = function() return nil end,
            refuseLethalPreview = function() return nil end,
            preview = function() state.previews = state.previews + 1; return { damage_to_attacker = 29 } end,
        },
        CivvisFrames = { noteStrike = function() end },
        Map = {
            GetPlotIndex = function(x, y) assert(x == 8 and y == 23); return 30 end,
            GetPlotByIndex = function(id)
                local xy = plots[id]
                if not xy then return nil end
                return { GetX = function() return xy[1] end, GetY = function() return xy[2] end }
            end,
        },
        UnitManager = {},
        try = function(callback, fallback)
            local ok, value = pcall(callback)
            if ok and value ~= nil then return value end
            return fallback
        end,
        emit = function(kind, payload) state.emitted[#state.emitted + 1] = {kind, payload} end,
    }
    env.UnitManager.GetMoveToPathEx = function(...)
        assert(select("#", ...) == 2, "use the shipped two-argument movement path query")
        local unit, destination = ...
        assert(unit == state.unit and destination == 30, "query the actual unit and ordered destination")
        state.queries = state.queries + 1
        return state.path
    end
    env.UnitManager.RequestOperation = function(unit, operation, params)
        assert(unit == state.unit and operation == 123)
        state.requests = state.requests + 1
        state.params = params
        return true
    end
    env.operate = env.UnitManager.RequestOperation
    env.TEST_UNIT = state.unit
    setmetatable(env, {__index = _G})
    local define = assert(loadstring(helper .. "\n" .. strike)); setfenv(define, env); define()
    local chunk = assert(loadstring("return function()\n"
        .. "local verb, x, y, unit, subject, pid, turn, row = 'ATTACK', 8, 23, TEST_UNIT, 77, 0, 40, {}\n"
        .. section .. "\nend"))
    setfenv(chunk, env)
    return state, env, chunk()
end

local function apply(state, handler)
    local ok, verb = handler()
    assert(ok and verb == "ATTACK" and state.requests == 1, "keep the original single native request")
    assert(state.params.x == 8 and state.params.y == 23 and state.params.modifiers == state.modifiers,
        "observation must not change the target or movement modifiers")
    assert(state.previews == 0, "observation must not introduce a combat simulation")
    assert(#state.emitted == 1 and state.emitted[1][1] == "strike")
    local payload = state.emitted[1][2]
    assert(payload.turn == 40 and payload.unit == 77 and payload.hp == 60)
    assert(payload.x == 8 and payload.y == 23 and payload.verb == "ATTACK")
    return payload
end

local failures, cases = {}, 0
local function test(name, callback)
    cases = cases + 1
    local ok, err = pcall(callback)
    if ok then print("PASS " .. name)
    else failures[#failures + 1] = name; print("FAIL " .. name .. ": " .. tostring(err)) end
end

test("an approach path records this turn's endpoint and the later destination", function()
    local s, _, handler = fixture(); local p = apply(s, handler)
    assert(s.queries == 1 and p.native_move_path_known == true and p.native_move_path_length == 3)
    assert(p.unit_x == 7 and p.unit_y == 23 and p.moves == 2 and p.attacks == 1)
    assert(p.native_move_modifiers_known == true and p.native_move_modifiers == 3)
    assert(p.native_move_path_destination_matches_order == true)
    assert(p.native_move_path_destination.x == 8 and p.native_move_path_destination.y == 23
        and p.native_move_path_destination.turn == 2)
    assert(p.native_move_path_first_step.x == 8 and p.native_move_path_first_step.y == 24)
    assert(p.native_move_path_this_turn_end.x == 8 and p.native_move_path_this_turn_end.y == 24)
    assert(s.path.plots[3] == 30 and s.path.turns[3] == 2, "the native answer is read only")
end)
test("a same-turn path preserves the native one-turn destination", function()
    local s, _, handler = fixture(); s.path = {plots = {10, 30}, turns = {1, 1}}
    local p = apply(s, handler)
    assert(p.native_move_path_known == true and p.native_move_path_destination.turn == 1)
    assert(p.native_move_path_this_turn_end.x == 8 and p.native_move_path_this_turn_end.y == 23)
end)
test("a path ending elsewhere is recorded without claiming the order's destination", function()
    local s, _, handler = fixture(); s.path = {plots = {10, 20}, turns = {1, 1}}
    local p = apply(s, handler)
    assert(p.native_move_path_known == true and p.native_move_path_destination_matches_order == false)
    assert(p.native_move_path_destination.y == 24)
end)
test("a one-entry native path reports no fabricated first step", function()
    local s, _, handler = fixture(); s.path = {plots = {10}, turns = {1}}
    local p = apply(s, handler)
    assert(p.native_move_path_known == true and p.native_move_path_length == 1)
    assert(p.native_move_path_first_step == nil and p.native_move_path_destination_matches_order == false)
end)
test("an empty native answer reports only its observed length", function()
    local s, _, handler = fixture(); s.path = {plots = {}, turns = {}}
    local p = apply(s, handler)
    assert(p.native_move_path_known == true and p.native_move_path_length == 0)
    assert(p.native_move_path_destination == nil and p.native_move_path_this_turn_end == nil)
end)
for _, fault in ipairs({"no API", "throwing API", "no path", "no plots", "no turns", "extra turn",
        "sparse plots", "late bad plot", "nonfinite turn", "bad destination", "too long"}) do
    test(fault .. " stays unknown and retains the original request", function()
        local s, env, handler = fixture()
        if fault == "no API" then env.UnitManager.GetMoveToPathEx = nil
        elseif fault == "throwing API" then env.UnitManager.GetMoveToPathEx = function() error("unavailable") end
        elseif fault == "no path" then s.path = nil
        elseif fault == "no plots" then s.path.plots = nil
        elseif fault == "no turns" then s.path.turns = nil
        elseif fault == "extra turn" then s.path.turns[4] = 2
        elseif fault == "sparse plots" then s.path = {plots = {[1] = 10, [3] = 30}, turns = {[1] = 1, [3] = 2}}
        elseif fault == "late bad plot" then s.path.plots[3] = 99
        elseif fault == "nonfinite turn" then s.path.turns[3] = 0/0
        elseif fault == "bad destination" then env.Map.GetPlotIndex = function() return -1 end
        elseif fault == "too long" then for i = 1, 257 do s.path.plots[i] = 10; s.path.turns[i] = 1 end end
        local p = apply(s, handler)
        assert(p.native_move_path_known == false and p.native_move_path_length == nil)
        assert(p.native_move_path_destination == nil and p.native_move_path_first_step == nil,
            "a malformed later row must not publish a partial positive path")
    end)
end
test("missing modifiers stay unknown without inventing an attack flag", function()
    local s, _, handler = fixture(); s.modifiers = nil
    local p = apply(s, handler)
    assert(p.native_move_modifiers_known == false and p.native_move_modifiers == nil)
end)
test("unreadable or nonfinite unit getters do not add invalid JSON numbers", function()
    local s, _, handler = fixture()
    s.unit.GetX = function() error("unknown") end
    s.unit.GetMovesRemaining = function() return 0/0 end
    local p = apply(s, handler)
    assert(p.unit_x == nil and p.moves == nil and p.native_move_path_known == true)
end)
test("ranged strikes do not query movement paths", function()
    local s, env = fixture()
    env.CivvisLedger.strike(s.unit, 77, "RANGE_ATTACK", 8, 23, 40)
    assert(s.queries == 0 and s.requests == 0 and s.previews == 0)
    assert(s.emitted[1][2].native_move_path_known == nil)
end)
test("the existing combat preview still runs once when enabled", function()
    local s, env, handler = fixture(); env.cfg.StrikePreview = true
    assert(handler())
    assert(s.previews == 1 and s.requests == 1 and s.queries == 1)
    assert(s.emitted[1][2].preview.damage_to_attacker == 29)
end)
assert(#failures == 0, table.concat(failures, ", "))
print("PASS melee path observation " .. cases .. " cases")
