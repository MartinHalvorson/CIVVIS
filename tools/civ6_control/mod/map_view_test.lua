local here = arg[0]:match("(.*)/[^/]*$") or "."
local function host(cfg, initial)
    local h = { zoom = initial, writes = 0, events = {}, options = {LookAtPlayerTurnCombat=2, LookAtPlayerOffTurnCombat=0}, logs={} }
    local env = setmetatable({ CivvisControlConfig=cfg }, {__index=_G})
    env.Options = {
        GetUserOption=function(_, key) if h.optionThrow then error("options unavailable") end; return h.options[key] end,
        SetUserOption=function(_, key, value) h.options[key]=value end,
    }
    env.UI = {
        GetMapZoom=function() if h.throw then error("loading") end; return h.zoom end,
        SetMapZoom=function(value, x, y)
            assert(x==0 and y==0, "must not steer focus to a unit")
            h.writes=h.writes+1
            if not h.refuse then h.zoom=value end
            if h.events.Camera_Updated then h.events.Camera_Updated() end
        end,
        SetRestoreMapZoom=function(value) h.restore=value end,
    }
    env.Events=setmetatable({}, {__index=function(_, name)
        return {Add=function(callback) h.events[name]=callback end}
    end})
    env.Automation={Log=function(line) h.logs[#h.logs+1]=line end}
    local chunk=assert(loadfile(here.."/CivvisControlMapView.lua"));setfenv(chunk,env);chunk()
    h.enforce=env.CivvisMapView.Enforce
    h.pulse=env.CivvisMapView.Pulse
    h.loadHud=function()
        env.include=function() env.LateInitialize=function() end end
        env.ContextPtr={SetUpdate=function(_, callback) h.hudTick=callback end}
        env.LuaEvents={CivvisControlPulse=function() end}
        local hud=assert(loadfile(here.."/CivvisControlHeartbeat.lua"));setfenv(hud,env);hud()
    end
    return h
end
local rounded=host({CivvisDecides=true}, .8 - 0.00000005)
for i=1,100 do rounded.events.Camera_Updated() end
assert(rounded.writes==0, "native floating-point rounding must not create a correction loop")
assert(#rounded.logs==1 and rounded.logs[1]:find('"verified":true',1,true), "rounded wide zoom is a verified stable view")
local h=host({CivvisDecides=true}, .1)
assert(h.zoom==.8 and h.restore==.8 and h.writes==1, "startup must restore a broad map without recursion")
assert(h.options.LookAtPlayerTurnCombat==1 and h.options.LookAtPlayerOffTurnCombat==0)
for _, name in ipairs({"Camera_Updated","LoadGameViewStateDone","LocalPlayerTurnBegin","CombatVisEnd"}) do
    h.zoom=.2;h.events[name]();assert(h.zoom==.8, name.." must correct a close-up")
end
h.zoom=.95;local writes=h.writes;h.enforce();assert(h.zoom==.95 and h.writes==writes, "already wide view must not be moved")
h.throw=true;h.enforce();h.throw=false;h.zoom=.1;h.enforce();assert(h.zoom==.8, "temporary host error must not disable protection")
h.optionThrow=true;h.zoom=.1;h.enforce();assert(h.zoom==.8, "an options error must not prevent zoom correction");h.optionThrow=false
h.refuse=true;h.zoom=.1;h.enforce();assert(h.logs[#h.logs]:find('"verified":false',1,true), "native refusal must not be reported as fixed")
local refusedWrites=h.writes;local refusedLogs=#h.logs
for i=1,100 do h.events.Camera_Updated() end
assert(h.writes==refusedWrites and #h.logs==refusedLogs, "failed readback must not flood writes or logs between pulses")
h.loadHud();h.hudTick(.5);assert(h.writes==refusedWrites)
h.hudTick(.5);assert(h.writes==refusedWrites+1, "actual HUD heartbeat must retry a refused correction")
h.zoom=.8-0.00000005;h.events.Camera_Updated()
assert(h.logs[#h.logs]:find('"verified":true',1,true), "asynchronous successful readback must be reported")
h.zoom=.1
h.refuse=false;h.pulse();assert(h.zoom==.8)
h.zoom=.798;h.events.Camera_Updated();assert(h.zoom==.8, "genuinely closer view must still be corrected")
for _, cfg in ipairs({{}, {Play=false,CivvisDecides=true},{CivvisDecides=false}}) do
    local off=host(cfg,.1);off.enforce();assert(off.writes==0 and next(off.events)==nil)
end
print("map view guard checks passed")
