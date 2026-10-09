local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local block = assert(source:match("(CivvisCitizenGrowth =.-)\nlocal function exportState"))
local favored, disfavored, can, throws, requests = false, false, true, false, {}
local surplus, housing, population, owner = 2, 5, 4, 0
local env = setmetatable({
    YieldTypes={FOOD=0},
    CityCommandTypes={SET_FOCUS=12,PARAM_FLAGS=1,PARAM_YIELD_TYPE=2,PARAM_DATA0=3},
    try=function(fn,fallback) local ok,v=pcall(fn); if ok then return v end; return fallback end,
    CityManager={
        CanStartCommand=function(_, command, params)
            assert(command==12 and params[1]==0 and params[2]==0)
            return can
        end,
        RequestCommand=function(_, command, params)
            if throws then error("request rejected") end
            assert(command==12)
            requests[#requests+1]=params[3]
        end
    }
}, {__index=_G})
local chunk=assert(loadstring(block)); setfenv(chunk,env); chunk()
local growth=env.CivvisCitizenGrowth
local city={GetID=function() return 65536 end,GetOwner=function() return owner end,
    GetPopulation=function() return population end,
    GetGrowth=function() return {GetFoodSurplus=function() return surplus end,GetHousing=function() return housing end} end,
    GetCitizens=function() return {
        IsFavoredYield=function(_,yield) assert(yield==0); return favored end,
        IsDisfavoredYield=function(_,yield) assert(yield==0); return disfavored end
    } end}
assert(growth.read(city).favored==false and growth.read(city).managed==false)
can=false
assert(growth.request(city,true)==false and #requests==0)
can=true; throws=true
assert(growth.request(city,true)==false and growth.read(city).managed==false)
throws=false
assert(growth.request(city,true)==true and requests[1]==1)
assert(growth.read(city).favored==false, "a non-throwing request is not native readback")
favored=true
assert(growth.read(city).managed==true)
assert(growth.request(city,true)==true and #requests==1, "idempotent once read back")
assert(growth.request(city,false)==true and requests[2]==0)
assert(growth.read(city).managed==true, "release ownership persists until native readback")
favored=false
assert(growth.read(city).managed==true, "export alone does not commit a safe release")
assert(not growth.pending(true))
assert(growth.read(city).managed==false)
assert(growth.request(city,false)==false and #requests==2, "do not release an unowned preference")
favored=true
assert(growth.request(city,true)==false, "do not claim another source's Food preference")
favored=false; disfavored=true
assert(growth.request(city,true)==false, "preserve explicitly ignored Food")
assert(growth.read({})==nil, "a failed native accessor stays unknown")
for _,field in ipairs({"food_favored","food_disfavored","food_focus_managed"}) do
    assert(source:find(field .. " = try(function() return CivvisCitizenGrowth.read(city).",1,true))
end
assert(source:find('return CivvisCitizenGrowth.request(city, verb == "FAVOR_FOOD")',1,true))
-- Native Panama104: release with almost no storedFood reallocates7Food
-- for4 citizens. Exercise the actual central end-turn submission function,
-- not merely the request helper: neither ordinary nor forced submission can
-- proceed until the compensating Food preference is read back.
chunk(); growth=env.CivvisCitizenGrowth
favored=false; disfavored=false; surplus=2; requests={}
assert(growth.request(city,true)); favored=true
assert(growth.read(city).managed)
assert(growth.request(city,false)); assert(#requests==2)
local submissions=0
local ending=assert(source:match("(CivvisQueue.requestEndTurn = function.-)\nlocal function tick"))
env.CivvisQueue={noteEndTurnRequest=function() end}
env.CivvisTrade={answerOrphanSessions=function() end,holdsEndTurn=function() return false end}
env.CivvisClock={now=function() return 5 end}
env.UI={HasSentTurnComplete=function() return false end,RequestAction=function() submissions=submissions+1 end}
env.ActionTypes={ACTION_ENDTURN=99}
env.emit=function() end
local endChunk=assert(loadstring(ending));setfenv(endChunk,env);endChunk()
assert(env.CivvisQueue.requestEndTurn(104)==false and submissions==0,
    "release submission is not native allocation readback")
favored=false; surplus=-1
assert(env.CivvisQueue.requestEndTurn(104,{forced=true})==false and submissions==0)
assert(#requests==3 and requests[3]==1,"negative native surplus restores Food")
assert(growth.read(city).managed and not growth.read(city).favored)
assert(growth.pending() and #requests==3,"wait for restoration without duplicate requests")
favored=true; surplus=2
assert(not growth.pending())
env.CivvisQueue.requestEndTurn(104)
assert(submissions==1 and growth.read(city).managed,
    "turn proceeds only after actual restored Food preference")
assert(growth.request(city,false)==false and #requests==3,
    "same allocation facts cannot repeatedly toggle an unsafe release")
housing=6; growth.read(city)
assert(growth.request(city,false)==true and #requests==4,
    "changed native housing permits a new release trial")
favored=false; surplus=0
assert(not growth.pending() and growth.read(city).managed,
    "safe allocation remains owned through later actions")
assert(not growth.pending(true) and not growth.read(city).managed,
    "final non-starving allocation completes release")

-- Unknown native Food cannot certify a safe allocation; restore the owned
-- preference. A thrown restoration request remains pending and can retry.
chunk(); growth=env.CivvisCitizenGrowth
favored=false; surplus=2; requests={};housing=5
assert(growth.request(city,true));favored=true
assert(growth.request(city,false));favored=false;surplus=nil;throws=true
assert(growth.pending() and #requests==2)
throws=false
assert(growth.pending() and requests[3]==1)
favored=true;surplus=2
assert(not growth.pending() and growth.read(city).managed)

-- A safe export can precede a later own action changing Food. Ownership
-- must remain until final submission, which checks the allocation again.
chunk();growth=env.CivvisCitizenGrowth
favored=false;surplus=2;requests={}
assert(growth.request(city,true));favored=true
assert(growth.request(city,false));favored=false;surplus=0
assert(not growth.pending() and growth.read(city).managed)
env.CivvisQueue.endTurnSubmittedAt=nil
surplus=-1
assert(env.CivvisQueue.requestEndTurn(105)==false and submissions==1)
assert(env.CivvisQueue.endTurnSubmittedAt==nil,"held submission does not record a sent turn")
assert(requests[3]==1 and growth.pending())
favored=true;surplus=nil
assert(not growth.pending())
surplus=2;growth.read(city)
assert(growth.request(city,false)==false)
housing=7;growth.read(city)
assert(growth.request(city,false)==true,"known allocation after an unknown quote can change")
favored=false;surplus=0
assert(not growth.pending(true))

-- Losing the city does not hold another owner's turn or alter its focus.
chunk();growth=env.CivvisCitizenGrowth
favored=false;surplus=2;requests={}
assert(growth.request(city,true));favored=true
assert(growth.request(city,false));owner=1
assert(not growth.pending() and #requests==2)
owner=0
print("native citizen Food focus request/readback and starvation guard checks passed")
