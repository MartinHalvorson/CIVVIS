local here = arg[0]:match("(.*)/[^/]*$") or "."
local f = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = f:read("*a"); f:close()
local block = assert(source:match('(CivvisTiles.owningCity = function.-)\n\tlocal known = CivvisTiles.known;'))
local queries = 0
local city = {GetOwner=function() return 0 end, GetID=function() return 20 end}
local plot = {GetOwner=function() return 0 end}
local env = setmetatable({CivvisTiles={}, Cities={GetPlotPurchaseCity=function() queries=queries+1; return city end},
    try=function(fn, fallback) local ok,v=pcall(fn); if ok then return v end; return fallback end}, {__index=_G})
local chunk=assert(loadstring(block));setfenv(chunk,env);chunk()
local read=env.CivvisTiles.owningCity
assert(read(plot,0)==20)
city.GetID=function() return 0 end
assert(read(plot,0)==0, "city zero must survive")
local before=queries
assert(read(plot,1)==nil and queries==before, "foreign plots must not query hidden cities")
city.GetOwner=function() return 1 end
assert(read(plot,0)==nil, "owner mismatch must not leak a stale city")
city=nil
assert(read(plot,0)==nil)
env.Cities.GetPlotPurchaseCity=function() error("not ready") end
assert(read(plot,0)==nil, "an unavailable host API leaves an unknown observation")
assert(source:find('oc = CivvisTiles.owningCity(plot, pid)',1,true), "wire the observation into the tile payload")
local signature=assert(source:match('(mark = %(owner %* 1024.-)\n\t\t\t\tend'))
assert(signature:find('tostring(CivvisTiles.owningCity(plot, pid))',1,true), "same-player swaps must invalidate the delta signature")
print("native plot city ownership checks passed")
