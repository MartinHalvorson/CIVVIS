local here = arg[0]:match("(.*)/[^/]*$") or "."
local f = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = f:read("*a"); f:close()
local helper = assert(source:match('(local function cityRangedStrength%(.-\nend)'))
local _, calls = source:gsub('ranged_strength = cityRangedStrength%(pid, city%)', '')
assert(calls == 3, "own, rival and minor city exports must carry ranged strength")
local owner, visible, value, reads = 1, true, 60, 0
local district = {GetAttackStrength=function() reads=reads+1;return value end}
local city = {GetOwner=function() return owner end,GetX=function() return 13 end,GetY=function() return 8 end}
local env = setmetatable({
 try=function(fn) local ok,v=pcall(fn);if ok then return v end end,
 PlayersVisibility={[0]={IsVisible=function(_,x,y) assert(x==13 and y==8);return visible end}},
 Map={GetPlot=function(x,y) assert(x==13 and y==8);return {} end},
 CityManager={GetDistrictAt=function() return district end},
}, {__index=_G})
local chunk=assert(loadstring(helper..'\nreturn cityRangedStrength'));setfenv(chunk,env)
local read=chunk()
assert(read(0,city)==60 and reads==1, "visible rival uses native attack strength")
visible=false
assert(read(0,city)==nil and reads==1, "fog must not query attack strength")
env.PlayersVisibility=nil
assert(read(0,city)==nil and reads==1, "unknown visibility fails closed")
owner=0
assert(read(0,city)==60, "our own city remains observable without a visibility API")
for _,v in ipairs({0,3,75}) do value=v;assert(read(0,city)==v) end
for _,v in ipairs({-1,math.huge,0/0,"60"}) do value=v;assert(read(0,city)==nil) end
district.GetAttackStrength=nil
assert(read(0,city)==nil, "missing API must stay unknown, not defense or zero")
district=nil
assert(read(0,city)==nil, "missing district remains unknown")
print('city ranged strength export checks passed')
