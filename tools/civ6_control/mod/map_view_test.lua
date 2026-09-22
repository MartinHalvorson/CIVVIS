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
    return h
end
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
h.refuse=false;h.enforce();assert(h.zoom==.8)
for _, cfg in ipairs({{}, {Play=false,CivvisDecides=true},{CivvisDecides=false}}) do
    local off=host(cfg,.1);off.enforce();assert(off.writes==0 and next(off.events)==nil)
end
print("map view guard checks passed")
