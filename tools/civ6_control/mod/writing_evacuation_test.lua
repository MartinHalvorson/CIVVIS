local here = arg[0]:match('(.*)/[^/]*$') or '.'
local function stub() return setmetatable({}, {__index=function() return stub() end,__call=function() return stub() end,__newindex=function() end}) end
setmetatable(_G,{__index=function() return stub() end})
CivvisControlConfig={};Automation={Log=function() end};
local damage,loyalty,slots,pillaged,war,fail={ [10]=50,[20]=0 },100,{[10]={[0]=1,[1]=2},[20]={}},false,true,false
local art={[1]={GreatWorkType='W1',GreatWorkObjectType='GREATWORKOBJECT_WRITING'},[2]={GreatWorkType='W2',GreatWorkObjectType='GREATWORKOBJECT_WRITING'}}
local function city(id)
 local b={HasBuilding=function() return true end,IsPillaged=function() return pillaged end,GetNumGreatWorkSlots=function() return 2 end,GetGreatWorkInSlot=function(_,_,slot) return slots[id][slot] or -1 end,GetGreatWorkTypeFromIndex=function(_,i) return i end}
 return {GetID=function() return id end,GetX=function() return id end,GetY=function() return 0 end,GetBuildings=function() return b end,GetCulturalIdentity=function() return {GetLoyalty=function() return loyalty end} end}
end
local cities={city(10),city(20)}
local player={GetCities=function() return {Members=function() local i=0;return function() i=i+1;if cities[i] then return i,cities[i] end end end} end,GetDiplomacy=function() return {IsAtWarWith=function() return war end} end}
PlayerManager={GetAliveMajorIDs=function() return {0,1} end}
Map={GetPlot=function(x,y) return x end};CityManager={GetDistrictAt=function(id) return {GetDefenseStrength=function() return 50 end,GetDamage=function() return damage[id] end,GetMaxDamage=function() return 200 end} end};DefenseTypes={DISTRICT_GARRISON=0,DISTRICT_OUTER=1}
GameInfo=setmetatable({Buildings={BUILDING_AMPHITHEATER={Index=5}},GreatWorks=art,UnitOperations={},UnitCommands={}}, {__index=function() return stub() end})
Game={GetLocalPlayer=function() return 0 end,GetCurrentGameTurn=function() return 20 end}
PlayerOperations={PARAM_PLAYER_ONE='player',PARAM_CITY_SRC='src',PARAM_CITY_DEST='dst',PARAM_BUILDING_SRC='bs',PARAM_BUILDING_DEST='bd',PARAM_GREAT_WORK_INDEX='work',PARAM_SLOT='slot',MOVE_GREAT_WORK=88}
local requests,got=0,nil
UI={RequestPlayerOperation=function(pid,op,p) assert(pid==0 and op==88);requests=requests+1;got=p;return not fail end}
assert(loadfile(here..'/CivvisControlAgent.lua'))()
local row={kind='writing_evacuation',subject=10,verb='W1',x=20,y=0}
local function reject(expected)
 local n=requests;local ok,why=CivvisApplyOrder(player,0,row,20);assert(not ok and why==expected,tostring(why));assert(requests==n)
end
local ok,why=CivvisApplyOrder(player,0,row,20);assert(ok and requests==1,tostring(why));assert(got.src==10 and got.dst==20 and got.bs==5 and got.bd==5 and got.work==1 and got.slot==0)
reject('museum_swap_turn_used');CivvisMuseumSwaps.lastTurn=-1
war=false;reject('writing_evacuation_no_war');war=true
damage[10]=0;reject('writing_evacuation_danger_changed');damage[10]=50
damage[20]=1;reject('writing_evacuation_danger_changed');damage[20]=0
loyalty=50;reject('writing_evacuation_danger_changed');loyalty=100
slots[20][0]=2;reject('writing_evacuation_destination_changed');slots[20][0]=nil
row.verb='STALE';reject('writing_evacuation_work_changed');row.verb='W1'
art[1].GreatWorkObjectType='GREATWORKOBJECT_ARTIFACT';reject('writing_evacuation_work_changed');art[1].GreatWorkObjectType='GREATWORKOBJECT_WRITING'
pillaged=true;reject('writing_evacuation_building_changed');pillaged=false
row.x=99;reject('writing_evacuation_not_ours');row.x=20
fail=true;ok,why=CivvisApplyOrder(player,0,row,20);assert(not ok and why=='writing_evacuation_refused' and CivvisMuseumSwaps.lastTurn==-1)
print('writing_evacuation_test: passed')
