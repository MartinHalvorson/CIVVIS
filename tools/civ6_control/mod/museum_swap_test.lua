local here = arg[0]:match('(.*)/[^/]*$') or '.'
local function stub() return setmetatable({}, {__index=function() return stub() end,__call=function() return stub() end,__newindex=function() end}) end
setmetatable(_G,{__index=function() return stub() end})
CivvisControlConfig={};Automation={Log=function() end};
local art={};local data={};local index=0
local function museum(id, names, factor)
 local slots={};local created={}
 for i,name in ipairs(names) do index=index+1;slots[i]=index;created[index]=0;art[index]={GreatWorkType=name..'_'..i,GreatWorkObjectType='GREATWORKOBJECT_LANDSCAPE',Tourism=2};data[index]={CreatorName=name} end
 local b={HasBuilding=function() return true end,GetNumGreatWorkSlots=function() return 3 end,
 GetGreatWorkInSlot=function(_,_,s) return slots[s+1] or -1 end,
 GetGreatWorkTypeFromIndex=function(_,i) return i end,GetTurnFromIndex=function(_,i) return created[i] end,
 GetBuildingTourismFromGreatWorks=function(_,religious) if religious then return 0 end;return (factor or 1)*CivvisMuseumSwaps.score((function() local w={};for i,v in pairs(slots) do w[i]={creator=data[v].CreatorName,object=art[v].GreatWorkObjectType,tourism=art[v].Tourism} end;return w end)()) end}
 return {GetID=function() return id end,GetBuildings=function() return b end},slots,created
end
GameInfo=setmetatable({Buildings={BUILDING_MUSEUM_ART={Index=7}},GreatWorks=art,UnitOperations={},UnitCommands={}}, {__index=function() return stub() end})
Game={GetLocalPlayer=function() return 0 end,GetCurrentGameTurn=function() return 20 end,GetGreatWorkDataFromIndex=function(i) return data[i] end};GlobalParameters={GREATWORK_ART_LOCK_TIME=10};
PlayerOperations={PARAM_PLAYER_ONE='player',PARAM_CITY_SRC='src',PARAM_CITY_DEST='dst',PARAM_BUILDING_SRC='bs',PARAM_BUILDING_DEST='bd',PARAM_GREAT_WORK_INDEX='work',PARAM_SLOT='slot',MOVE_GREAT_WORK=88}
local requests,got,throw=0,nil,false
UI={RequestPlayerOperation=function(pid,op,p) assert(pid==0 and op==88);requests=requests+1;got=p;if throw then error('refused') end end}
assert(loadfile(here..'/CivvisControlAgent.lua'))()
local a,as,ac= museum(10,{'A','A','A'},1);local b,bs,bc=museum(20,{'B','B','B'},1)
local cities={a,b};local player={GetCities=function() return {Members=function() local i=0;return function() i=i+1;if cities[i] then return i,cities[i] end end end} end}
local row={kind='museum_swap',subject=10,verb='A_1',x=20,y=0}
local ok,why=CivvisApplyOrder(player,0,row,20);assert(ok and requests==1,tostring(why));assert(got.src==10 and got.dst==20 and got.bs==7 and got.bd==7 and got.slot==0 and got.work==as[1])
ok,why=CivvisApplyOrder(player,0,row,20);assert(not ok and why=='museum_swap_turn_used' and requests==1)
CivvisMuseumSwaps.lastTurn=-1;ac[as[1]]=15
ok,why=CivvisApplyOrder(player,0,row,20);assert(not ok and why=='museum_art_locked' and requests==1)
ac[as[1]]=0;bc[bs[1]]=15
ok,why=CivvisApplyOrder(player,0,row,20);assert(not ok and why=='museum_art_locked' and requests==1)
bc[bs[1]]=0;row.x=99
ok,why=CivvisApplyOrder(player,0,row,20);assert(not ok and why=='museum_not_ours' and requests==1)
row.x=20;row.verb='STALE_WORK'
ok,why=CivvisApplyOrder(player,0,row,20);assert(not ok and why=='museum_work_changed' and requests==1)
row.verb='A_1';throw=true
ok,why=CivvisApplyOrder(player,0,row,20);assert(not ok and why=='museum_swap_refused' and requests==2 and CivvisMuseumSwaps.lastTurn==-1)
throw=false
-- A base-score improvement can lose native Tourism when the donor has Curator.
local c,cs= museum(30,{'A','B','C'},5);local d,ds=museum(40,{'A','A','A'},1);cities={c,d}
row={kind='museum_swap',subject=30,verb='B_2',x=40,y=0}
ok,why=CivvisApplyOrder(player,0,row,20);assert(not ok and why=='museum_no_native_gain' and requests==2,tostring(why))
-- Unknown native readback and religious-art channels do not invent a multiplier.
data[cs[1]]=nil
ok,why=CivvisApplyOrder(player,0,row,20);assert(not ok and why=='museum_unknown_inventory:source_creator' and requests==2,tostring(why))
-- The refusal names the museum and the read that stopped it: it was one word
-- for nine gates, and every swap of two live Culture games was refused by it.
data[cs[1]]={CreatorName='A'}
local e,es=museum(50,{},1);cities={c,e};row={kind='museum_swap',subject=30,verb='B_2',x=50,y=0}
ok,why=CivvisApplyOrder(player,0,row,20);assert(not ok and why=='museum_unknown_inventory:dest_empty' and requests==2,tostring(why))
local f,fs=museum(60,{'F'},1);art[fs[1]].GreatWorkObjectType='GREATWORKOBJECT_RELIGIOUS';cities={c,f};row.x=60
ok,why=CivvisApplyOrder(player,0,row,20);assert(not ok and why=='museum_unknown_inventory:dest_not_art' and requests==2,tostring(why))
art[fs[1]].GreatWorkObjectType='GREATWORKOBJECT_LANDSCAPE';art[cs[2]].Tourism=0
ok,why=CivvisApplyOrder(player,0,row,20);assert(not ok and why=='museum_unknown_inventory:source_work_tourism' and requests==2,tostring(why))
art[cs[2]].Tourism=2
-- A work of the Babylon pack's artists carries Tourism 4, not 2
-- (DLC/Babylon/Data/Babylon_GreatWorks.xml). A museum holding one was unknown
-- inventory: every swap of civvis-20261115T102552Z (three Kandinskys in the
-- destination) and 110627Z (three Tohakus in the source) was refused. It is
-- read, weighs twice a base work, and the swap is judged on its gain.
local g,gs=museum(70,{'G','G','G'},1);for _,i in ipairs(gs) do art[i].Tourism=4 end
local h,hs=museum(80,{'H','H','H'},1);cities={g,h}
assert(CivvisMuseumSwaps.read(g,7)~=nil and CivvisMuseumSwaps.read(g,7).tourism==7,'three 4s by one artist score 6 + 1')
row={kind='museum_swap',subject=70,verb='G_1',x=80,y=0}
ok,why=CivvisApplyOrder(player,0,row,20);assert(ok and requests==3 and got.src==70 and got.dst==80 and got.work==gs[1],tostring(why))
CivvisMuseumSwaps.lastTurn=-1
-- ...and the other way round, a base work into the museum of 4s.
row={kind='museum_swap',subject=80,verb='H_1',x=70,y=2}
ok,why=CivvisApplyOrder(player,0,row,20);assert(ok and requests==4 and got.src==80 and got.dst==70 and got.slot==2,tostring(why))
CivvisMuseumSwaps.lastTurn=-1
-- Base works alone score as they always did: three by three artists of one
-- kind are (3 + 3) * 2, three by one artist 3 + 1.
assert(CivvisMuseumSwaps.score({{creator='A',object='L'},{creator='B',object='L'},{creator='C',object='L'}})==12)
assert(CivvisMuseumSwaps.score({{creator='A',object='L'},{creator='A',object='L'},{creator='A',object='L'}})==4)
print('museum_swap_test: passed')
