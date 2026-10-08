local here = arg[0]:match("(.*)/[^/]*$") or "."
local f = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = f:read("*a"); f:close()
local block = assert(source:match('(CivvisTiles.lakeState = function.-)\n\tlocal known = CivvisTiles.known;'))
local env = setmetatable({CivvisTiles={},
    try=function(fn, fallback) local ok,v=pcall(fn); if ok then return v end; return fallback end}, {__index=_G})
local chunk=assert(loadstring(block));setfenv(chunk,env);chunk()
local lake = env.CivvisTiles.lakeState
assert(lake({IsLake=function() return true end})==true)
assert(lake({IsLake=function() return false end})==false)
assert(lake({})==nil, "an unavailable API must not invent a lake")
assert(lake({IsLake=function() error("not ready") end})==nil)
assert(source:find('lk = water and CivvisTiles.lakeState(plot) or nil',1,true), "wire native lake identity into revealed water payloads")
local signature=assert(source:match('(mark = %(owner %* 1024.-)\n\t\t\t\tend'))
assert(signature:find('tostring(CivvisTiles.lakeState(plot))',1,true), "lake identity must invalidate a delta")
print("native lake identity export checks passed")
