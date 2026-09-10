-- Exercise the exact shipped wrapper without pretending to emulate Firaxis.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local wrapper = assert(source:match("(CivvisTransitions = { sequence = 0, apply = applyOrder };.-CivvisApplyOrder = applyOrder;)"))
local cfg, events, calls = { ActionTransitions = false }, {}, 0
local result, reason, throwing = true, "queued", false
local env = setmetatable({ cfg = cfg, CivvisFrames = { current = 2 },
    applyOrder = function()
        calls = calls + 1
        if throwing then error("host error") end
        return result, reason
    end,
    emit = function(kind, payload) events[#events + 1] = { kind, payload } end,
    exportState = function(_, _, _, _, kind)
        assert(kind ~= "state", "partial batches must never wake the brain")
        events[#events + 1] = { kind, {} }
    end,
}, { __index = _G })
local chunk = assert(loadstring(wrapper)); setfenv(chunk, env); chunk()
local order = { kind = "unit", subject = 7, verb = "MOVE_TO", x = 2, y = 3 }
local ok, why = env.CivvisApplyOrder({}, 0, order, 10)
assert(ok == true and why == "queued" and calls == 1 and #events == 0)
cfg.ActionTransitions = true
ok, why = env.CivvisApplyOrder({}, 0, order, 10)
assert(ok == true and why == "queued" and calls == 2 and #events == 4)
assert(events[1][1] == "action_transition_begin" and events[1][2].order.subject == 7)
assert(events[2][1] == "action_transition_before" and events[3][1] == "action_transition_after")
assert(events[4][2].phase == "request_boundary" and events[4][2].accepted == true)
assert(events[4][2].sequence == events[1][2].sequence)
result, reason, events = false, "cannot_move", {}
ok, why = env.CivvisApplyOrder({}, 0, order, 10)
assert(ok == false and why == "cannot_move" and events[4][2].accepted == false)
throwing, events = true, {}
assert(not pcall(function() env.CivvisApplyOrder({}, 0, order, 10) end))
assert(events[4][2].threw == true and events[4][2].accepted == false)
print("action transition wrapper: passed")
