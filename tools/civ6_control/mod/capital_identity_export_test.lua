-- Test every city export's production expressions, including false vs unknown.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local f = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = f:read("*a"); f:close()
local blocks = {}
for current, original, founder in source:gmatch('(capital = try%(function%(%) return city:IsCapital%(%)%; end, false%),.-original_capital = )(try%(function%(%) return city:IsOriginalCapital%(%)%; end, nil%))(.-original_owner = try%(function%(%) return city:GetOriginalOwner%(%)%; end, nil%))') do
 blocks[#blocks+1] = {original = original, founder = assert(founder:match('original_owner = (try%(function%(%) return city:GetOriginalOwner%(%)%; end, nil%))'))}
end
assert(#blocks == 3, "own, rival, and minor exports must all carry both identity facts")
local city = {}
local env = setmetatable({ city=city, try=function(fn,fallback) local ok,v=pcall(fn); if ok then return v end; return fallback end }, {__index=_G})
for _,block in ipairs(blocks) do
 local original=assert(loadstring("return "..block.original)); setfenv(original,env)
 local founder=assert(loadstring("return "..block.founder)); setfenv(founder,env)
 city.IsOriginalCapital=function() return true end
 city.GetOriginalOwner=function() return 2 end
 assert(original()==true and founder()==2, "captured original identity survives an owner change")
 city.IsOriginalCapital=function() return false end
 assert(original()==false, "a replacement capital is explicitly not original")
 city.IsOriginalCapital=nil
 city.GetOriginalOwner=nil
 assert(original()==nil and founder()==nil, "unsupported observations must not invent history")
end
print("capital identity export checks passed")
