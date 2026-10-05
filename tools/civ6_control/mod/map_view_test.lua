local here = arg[0]:match("(.*)/[^/]*$") or "."
local function host(cfg, initial)
    local h = { zoom = initial, writes = 0, events = {}, looks = {}, logs = {},
                turn = 10, active = false, capital = {10, 10},
                options = {LookAtPlayerTurnCombat=2, LookAtPlayerOffTurnCombat=1, AutoUnitCycle=1} }
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
        LookAtPlot=function(x, y) h.looks[#h.looks+1]={x, y} end,
    }
    env.Game = { GetLocalPlayer=function() return 0 end, GetCurrentGameTurn=function() return h.turn end }
    env.Map = { GetPlotDistance=function(ax, ay, bx, by) return math.max(math.abs(ax-bx), math.abs(ay-by)) end }
    local capital = { GetX=function() return h.capital[1] end, GetY=function() return h.capital[2] end }
    env.Players = { [0] = {
        IsTurnActive=function() return h.active end,
        GetCities=function() return { GetCapitalCity=function() return h.capital and capital or nil end,
                                      Members=function() return function() end end } end,
        GetUnits=function() return { Members=function() return function() end end } end,
    } }
    env.GameInfo = { Units = { [1] = {FormationClass="FORMATION_CLASS_LAND_COMBAT"},
                               [2] = {FormationClass="FORMATION_CLASS_CIVILIAN"} } }
    env.UnitManager = { GetUnit=function(_, id) return { GetType=function() return id >= 100 and 2 or 1 end } end }
    env.CityManager = { GetCity=function(_, id) return { GetX=function() return id end, GetY=function() return id end } end }
    env.Events=setmetatable({}, {__index=function(_, name)
        return {Add=function(callback) h.events[name]=callback end}
    end})
    env.Automation={Log=function(line) h.logs[#h.logs+1]=line end}
    local chunk=assert(loadfile(here.."/CivvisControlMapView.lua"));setfenv(chunk,env);chunk()
    h.enforce=env.CivvisMapView.Enforce
    h.pulse=env.CivvisMapView.Pulse
    h.pulses=function(n) for _=1,n do h.pulse() end end
    h.last=function() return h.looks[#h.looks] end
    h.march=function(player, unit, x, y, steps)
        for _=1,(steps or 1) do h.events.UnitMoved(player, unit, x, y) end
    end
    h.loadHud=function()
        env.include=function() env.LateInitialize=function() end end
        env.ContextPtr={SetUpdate=function(_, callback) h.hudTick=callback end}
        env.LuaEvents={CivvisControlPulse=function() end}
        local hud=assert(loadfile(here.."/CivvisControlHeartbeat.lua"));setfenv(hud,env);hud()
    end
    return h
end
local function near(look, x, y, r)
    return look ~= nil and math.max(math.abs(look[1]-x), math.abs(look[2]-y)) <= (r or 0)
end
local function aims(h)
    local n=0; for _, line in ipairs(h.logs) do if line:find('"aim":',1,true) then n=n+1 end end; return n
end

-- Zoom: wide, held in a band, never a correction loop.
local rounded=host({CivvisDecides=true}, .8 - 0.00000005)
for i=1,100 do rounded.events.Camera_Updated() end
assert(rounded.writes==0, "native floating-point rounding must not create a correction loop")
assert(#rounded.logs==1 and rounded.logs[1]:find('"verified":true',1,true), "rounded wide zoom is a verified stable view")
local h=host({CivvisDecides=true}, .4)
assert(h.zoom==.8 and h.restore==.8 and h.writes==1, "startup must take a close map out wide without recursion")
assert(h.options.LookAtPlayerTurnCombat==0 and h.options.LookAtPlayerOffTurnCombat==0,
    "native combat panning must not pull the camera off our area")
assert(h.options.AutoUnitCycle==0, "unit cycling must not pan the camera to every ready unit")
for _, name in ipairs({"Camera_Updated","LoadGameViewStateDone","LocalPlayerTurnBegin","CombatVisEnd"}) do
    h.zoom=.4;h.events[name]();assert(h.zoom==.8, name.." must correct a close view")
    h.zoom=.05;h.events[name]();assert(h.zoom==.8, name.." must correct an extreme close-up")
end
h.zoom=.83;local writes=h.writes;h.enforce();assert(h.zoom==.83 and h.writes==writes, "a view inside the band must not be moved")
h.throw=true;h.enforce();h.throw=false;h.zoom=.4;h.enforce();assert(h.zoom==.8, "temporary host error must not disable protection")
h.optionThrow=true;h.zoom=.4;h.pulse();assert(h.zoom==.8, "an options error must not prevent zoom correction");h.optionThrow=false
h.options.AutoUnitCycle=1;for i=1,50 do h.events.Camera_Updated() end
assert(h.options.AutoUnitCycle==1, "per-frame camera events must not touch user options")
h.events.LocalPlayerTurnBegin();assert(h.options.AutoUnitCycle==0, "turn start re-quiets the native camera")
h.refuse=true;h.zoom=.4;h.enforce();assert(h.logs[#h.logs]:find('"verified":false',1,true), "native refusal must not be reported as fixed")
local refusedWrites=h.writes;local refusedLogs=#h.logs
for i=1,100 do h.events.Camera_Updated() end
assert(h.writes==refusedWrites and #h.logs==refusedLogs, "failed readback must not flood writes or logs between pulses")
h.loadHud();h.hudTick(.5);assert(h.writes==refusedWrites)
h.hudTick(.5);assert(h.writes==refusedWrites+1, "actual HUD heartbeat must retry a refused correction")
h.zoom=.8-0.00000005;h.events.Camera_Updated()
assert(h.logs[#h.logs]:find('"verified":true',1,true), "asynchronous successful readback must be reported")
h.refuse=false

-- Aim: home first, then the hottest area of OUR activity.
local f=host({CivvisDecides=true}, .8)
assert(#f.looks==0, "loading the module must not pan")
f.pulse();assert(#f.looks==1 and f.last()[1]==10 and f.last()[2]==10, "no activity yet: look at the capital")
f.march(1, 7, 30, 30, 20);f.pulses(4)
assert(f.last()[1]==10, "an enemy army's moves are not our active area")
f.march(0, 7, 30, 30, 4);f.march(0, 8, 31, 30, 4);f.events.CombatVisBegin({{playerID=0},{playerID=1},x=32,y=31})
f.pulse();assert(near(f.last(), 32, 31), "our front outweighs an idle capital, centred on the fight")
local aimed=aims(f)
f.march(0, 9, 31, 31, 3);f.pulses(6)
assert(near(f.last(), 32, 31) and aims(f)==aimed, "activity in the same place must not re-aim")
f.march(0, 120, 5, 40, 2);f.pulses(6)
assert(near(f.last(), 32, 31) and aims(f)==aimed, "a builder elsewhere must not steal the camera from a front")
f.events.CityAddedToMap(1, 5, 45, 5);f.events.ImprovementAddedToMap(45, 5, 3, 1);f.pulses(6)
assert(aims(f)==aimed, "a rival's city and improvements are not our activity")

-- A second front only wins when clearly hotter, and never during our turn.
f.active=true
f.events.CombatVisBegin({{playerID=2},{playerID=0},x=50,y=10})
for _=1,4 do f.events.CombatVisBegin({{playerID=0},{playerID=2},x=50,y=11}) end
f.events.CityAddedToMap(0, 9, 51, 10)
f.pulses(10);assert(near(f.last(), 32, 31) and aims(f)==aimed, "no pan while our order queue drains")
f.active=false;f.pulse()
assert(near(f.last(), 50, 10, 1), "the hotter front takes the camera once our turn ends")
aimed=aims(f)
local looks=#f.looks;f.pulses(9);assert(#f.looks==looks, "a settled aim must not re-pan every pulse")
f.pulse();assert(#f.looks==looks+1 and aims(f)==aimed, "re-centre on the aim if something else panned")

-- The turn start re-aims before the queue drains, on what the rivals did.
f.turn=11;f.pulses(4);f.active=true
f.events.CombatVisBegin({{playerID=3},{playerID=0},x=20,y=20});f.events.CityOccupationChanged(0, 20)
f.events.CombatVisBegin({{playerID=3},{playerID=0},x=20,y=21})
f.events.CombatVisBegin({{playerID=3},{playerID=0},x=21,y=21})
f.turn=12;f.events.LocalPlayerTurnBegin()
assert(near(f.last(), 20, 20, 1), "turn start takes in the rivals' turn before our moves")

-- Activity fades with turns and is forgotten.
f.active=false;f.turn=16;f.march(0, 7, 2, 2, 1);f.pulses(5)
assert(f.last()[1]==2 and f.last()[2]==2, "old fronts fade; today's activity wins")

-- A load starts afresh and waits for the camera to exist.
f.events.LoadGameViewStateDone();f.capital={8, 9};f.turn=15;f.pulse()
assert(f.last()[1]==8 and f.last()[2]==9, "a reload forgets remembered activity and looks home")

-- Hysteresis: a slightly hotter place, or a hot plot inside the view, keeps the aim.
local g=host({CivvisDecides=true}, .8)
for _=1,2 do g.events.CombatVisBegin({{playerID=0},{playerID=1},x=10,y=30}) end
g.pulses(5);assert(near(g.last(), 10, 30), "first front aimed")
local gAims=aims(g)
g.events.CityAddedToMap(0, 3, 40, 30);g.events.DistrictAddedToMap(0, 1, 3, 40, 31, 4)
g.events.DistrictAddedToMap(0, 2, 3, 39, 30, 5);g.events.ImprovementAddedToMap(40, 29, 1, 0)
g.events.ImprovementAddedToMap(41, 29, 1, 0);g.pulses(5)
assert(near(g.last(), 10, 30) and aims(g)==gAims, "a place only slightly hotter must not take the camera")
g.events.CombatVisBegin({{playerID=2},{playerID=0},x=41,y=31});g.pulses(5)
assert(near(g.last(), 40, 30, 1) and aims(g)==gAims+1, "a place clearly hotter takes the camera")
for _=1,3 do g.events.CombatVisBegin({{playerID=0},{playerID=1},x=43,y=30}) end
for _=1,3 do g.events.CombatVisBegin({{playerID=0},{playerID=1},x=45,y=30}) end
g.pulses(5);assert(aims(g)==gAims+1, "a hot plot three tiles away is still the same view")

-- Re-aims are spaced: a new front waits out the hold after the last aim.
local k=host({CivvisDecides=true}, .8)
k.pulse();assert(near(k.last(), 10, 10), "home first")
k.events.CombatVisBegin({{playerID=0},{playerID=1},x=40,y=40});k.pulses(2)
assert(near(k.last(), 10, 10), "no re-aim inside the hold")
k.pulses(2);assert(near(k.last(), 40, 40), "re-aim once the hold has passed")

-- Memory is bounded.
local busy=host({CivvisDecides=true}, .8)
for i=1,300 do busy.march(0, 7, i % 60, math.floor(i / 60), 1) end
busy.pulse();assert(#busy.looks==1, "a flood of activity still yields one aim")

for _, cfg in ipairs({{}, {Play=false,CivvisDecides=true},{CivvisDecides=false}}) do
    local off=host(cfg,.9);off.enforce();off.pulse()
    assert(off.writes==0 and #off.looks==0 and next(off.events)==nil)
end
print("map view guard checks passed")
