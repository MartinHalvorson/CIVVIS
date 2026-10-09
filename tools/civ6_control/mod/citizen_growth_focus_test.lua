local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local block = assert(source:match("(CivvisCitizenGrowth =.-)\nlocal function exportState"))
local favored, disfavored, can, throws, requests = false, false, true, false, {}
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
local city={GetID=function() return 65536 end,GetOwner=function() return 0 end,
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
print("native citizen Food focus request/readback checks passed")
