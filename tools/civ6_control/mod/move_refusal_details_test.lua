-- Exercise the emitted stalled-move payload, including real refusal-probe forms.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local f = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = f:read("*a"); f:close()
local function run(mode, why)
    local state = { calls = 0, emitted = {}, fallbacks = 0 }
    local unit = {}
    local env = {
        cfg = { MoveFallback = true },
        UI = { IsGameCoreBusy = function() return false end },
        ActivityTypes = {},
        Map = { GetPlotIndex = function(x, y) assert(x == 8 and y == 9); return 98 end },
        UnitOperationTypes = { PARAM_X = "x", PARAM_Y = "y" },
        UnitOperationResults = { FAILURE_REASONS = "reasons" },
        OperationResultsTypes = { ALL = 999 },
        OP = { UNITOPERATION_MOVE_TO = 321 },
        Locale = { Lookup = function(key) return "localized:" .. key end },
        unitTypeName = function() return "UNIT_ARCHER" end,
        emit = function(kind, payload) state.emitted[#state.emitted + 1] = {kind, payload} end,
        try = function(fn, fallback)
            local ok, value = pcall(fn)
            if ok and value ~= nil then return value end
            return fallback
        end,
    }
    env.CivvisBoard = {
        moveAttempts = {}, stats = { move_noop = 0 },
        classifyNoop = function() return why or "cannot_start" end,
        activityName = function() return "awake" end,
        fallbackStep = function() state.fallbacks = state.fallbacks + 1; return nil end,
    }
    env.UnitManager = {
        GetActivityType = function() return 17 end,
        GetMoveToPathEx = function() return { plots = {} } end,
        CanStartOperation = function(actual, op, plots, params, results)
            state.calls = state.calls + 1
            assert(actual == unit and op == 321 and plots == nil and results == 999,
                "probe must preserve the exact refused unit, MOVE_TO and result form")
            if mode == "throw" then error("native probe unavailable") end
            if type(params) == "table" then
                assert(params.x == 8 and params.y == 9, "probe must preserve the exact target")
                if mode == "fallback" then return false, {} end
                if mode == "empty" then return false, { reasons = {} } end
                return false, { reasons = { "LOC_BLOCKED_BY_UNIT", "LOC_NO_OPEN_BORDERS" } }
            end
            assert(params == false, "only the existing alternate form may be attempted")
            if mode == "fallback" then return false, { reasons = { "LOC_ALTERNATE_REASON" } } end
            return false, {}
        end,
    }
    setmetatable(env, { __index = _G })
    local helper = assert(source:match("(local function refusalReason.-\n\tend)"))
    helper = helper:gsub("^local function refusalReason", "refusalReason = function", 1)
    local define = assert(loadstring(helper)); setfenv(define, env); define()
    -- The real order handler wires this same helper to the shared board.
    local wiring = source:match("\n\t(CivvisBoard%.operationRefusalReason = refusalReason;)")
    if wiring then
        local wire = assert(loadstring(wiring)); setfenv(wire, env); wire()
    end
    for _, name in ipairs({ "noopEvidence", "moveNoop" }) do
        local body = assert(source:match("(CivvisBoard%." .. name .. " = function.-\nend;)"))
        local chunk = assert(loadstring(body)); setfenv(chunk, env); chunk()
    end
    local board = env.CivvisBoard
    assert(board.moveNoop({}, 0, 7, unit, { expect = { x = 8, y = 9 }, wait = 2 },
        40, 6, 9, 3) == false, "diagnostics must not alter the movement outcome")
    assert(#state.emitted == 1 and state.emitted[1][1] == "move_noop")
    assert(state.emitted[1][2].why == (why or "cannot_start"), "keep the existing classifier")
    assert(state.fallbacks == 1, "keep the existing fallback decision")
    return state, state.emitted[1][2].native
end
local state, native = run("normal")
assert(native.operation_failure == "localized:LOC_BLOCKED_BY_UNIT | localized:LOC_NO_OPEN_BORDERS [p4r]",
    "cannot_start must retain the actual native reasons and answered call form")
assert(state.calls == 1, "stop after the first answered form with reasons")
state, native = run("fallback")
assert(native.operation_failure == "localized:LOC_ALTERNATE_REASON [t5r]" and state.calls == 2,
    "retain provenance when only the existing alternate form answers")
state, native = run("empty")
assert(native.operation_failure == "can_start=false,no_reasons [p4r]" and state.calls == 2,
    "an empty result stays explicitly unknown")
state, native = run("throw")
assert(native.operation_failure:find("probe_threw[p4r]:", 1, true) == 1 and state.calls == 2,
    "unavailable native overloads retain their failure, not an invented reason")
state, native = run("normal", "occupied")
assert(native.operation_failure == nil and state.calls == 0,
    "other classifier outcomes incur no extra native query")
print("native move refusal detail checks passed")
