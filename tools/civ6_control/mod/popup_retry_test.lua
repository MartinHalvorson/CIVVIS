local here = arg[0]:match("(.*)/[^/]*$") or "."
local function host(name, fading, closeAt)
    local h={hidden=false, closes=0, logs={}}
    local env=setmetatable({CivvisControlConfig={AnnouncementSeconds=.25,DialogueSeconds=.25}}, {__index=_G})
    env.include=function() end
    env.OnClose=function() h.closes=h.closes+1;if closeAt and h.closes>=closeAt then h.hidden=true end end
    env.OnShow=function() end
    env.Controls={BlackFadeAnim={IsStopped=function() return not fading end}}
    env.ContextPtr={
        GetID=function() return name end, IsHidden=function() return h.hidden end,
        SetUpdate=function(_,f) h.tick=f end, SetShowHandler=function(_,f) h.show=f end,
    }
    env.Automation={Log=function(line) h.logs[#h.logs+1]=line end}
    env.LuaEvents={}
    local chunk=assert(loadfile(here.."/CivvisControlAutoClose.lua"));setfenv(chunk,env);chunk()
    return h
end
local stuck=host("NaturalWonderPopup",false,25)
for i=1,48 do if not stuck.hidden then stuck.tick(.25) end end
assert(stuck.hidden, "popup recovering after twenty failed closes must close within twelve seconds")
local fade=host("DiplomacyActionView",true)
for i=1,7 do fade.tick(.25) end
assert(fade.closes==0, "normal opening fade needs its grace period")
for i=1,8 do fade.tick(.25) end
assert(fade.closes>0, "a permanently running fade must not prevent native close attempts")
local before=fade.closes;fade.show()
for i=1,7 do fade.tick(.25) end
assert(fade.closes==before, "fresh popup must get a fresh fade deadline")
for i=1,120 do fade.tick(.25) end
assert(fade.closes>before+20, "persistent popup must keep retrying complete close cycles")
print("bounded popup recovery checks passed")
