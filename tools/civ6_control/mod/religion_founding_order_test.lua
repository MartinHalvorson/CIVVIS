-- Exercise the actual religion order and its asynchronous confirmation helper.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local f = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = f:read("*a"); f:close()
local block = assert(source:match('(if kind == "religion" then.-)\n\tif kind == "research"'))
local calls, earned, created, turn, present = {}, 0, -1, 10, true
local follower = {Index=1, Hash=101, BeliefType="BELIEF_WORK_ETHIC"}
local founder = {Index=2, Hash=102, BeliefType="BELIEF_TITHE"}
local religion = {Index=3, Hash=103, ReligionType="RELIGION_BUDDHISM"}
local api = {
 GetReligionTypeCreated=function() return created end,
 HasReligiousFoundingUnit=function() return present end,
 GetNumBeliefsEarned=function() return earned end,
}
local prophet = {GetType=function() return 7 end, GetID=function() return 42 end}
local taken = false
local gameReligion = {IsInSomeReligion=function() return taken end, GetReligions=function() return {} end}
local player = {GetReligion=function() return api end, GetUnits=function() return {
 Members=function() local done=false; return function() if not done and present then done=true; return 42, prophet end end end
} end}
local env = setmetatable({
 player=player, pid=0, kind="religion", verb="BELIEF_WORK_ETHIC,BELIEF_TITHE",
 try=function(fn, fallback) local ok, value=pcall(fn); if ok then return value end; return fallback end,
 Game={GetReligion=function() return gameReligion end, GetCurrentGameTurn=function() return turn end},
 GameInfo={Beliefs={}, Units={[7]={UnitType="UNIT_GREAT_PROPHET"}},
  UnitOperations={UNITOPERATION_FOUND_RELIGION={Hash=104}},
  Religions=function() local done=false; return function() if not done then done=true; return religion end end end},
 resolveType=function(_, name) if name=="BELIEF_WORK_ETHIC" then return follower,name end; return founder,name end,
 OperationResultsTypes={NO_TARGETS=0},
 PlayerOperations={PARAM_INSERT_MODE="insert", VALUE_EXCLUSIVE=1, PARAM_RELIGION_TYPE="religion", PARAM_BELIEF_TYPE="belief", FOUND_RELIGION="found", ADD_BELIEF="belief"},
 UnitManager={CanStartOperation=function() return true end, RequestOperation=function() calls[#calls+1]="activate" end},
 UI={RequestPlayerOperation=function(_, op, params) calls[#calls+1]={op=op,params=params} end},
}, {__index=_G})
local function compile(text) local fn=assert(loadstring(text)); setfenv(fn,env); return fn end
local helper=source:match("%-%- BEGIN staged religion founding(.-)%-%- END staged religion founding")
if helper then compile(helper)() end
local order=compile(block)
assert(order())
assert(#calls==1 and calls[1]=="activate", "religion confirmation must wait for native activation")
assert(env.pendingReligionChoice and env.pendingReligionChoice.mode=="found")
-- Repeated decisions must not consume the Prophet or submit the choice twice.
order(); assert(#calls==1)
local confirm=assert(env.CivvisReligionFounding).confirm
assert(not confirm(player,0,env.pendingReligionChoice)); assert(#calls==1)
-- Native activation consumes the unit and grants earned beliefs later.
earned=2; present=false
assert(confirm(player,0,env.pendingReligionChoice)); assert(#calls==4)
assert(calls[2].op=="found" and calls[2].params.religion==103)
assert(calls[3].params.belief==101 and calls[4].params.belief==102)
confirm(player,0,env.pendingReligionChoice); assert(#calls==4)
-- A saved pending native choice can be completed without a surviving Prophet.
env.pendingReligionChoice=nil; calls={}; assert(order()); assert(#calls==3)
-- Unavailable beliefs at confirmation must not partially create a religion.
env.pendingReligionChoice=nil; calls={}; earned=0; present=true; assert(order())
earned=2; present=false; taken=true
assert(not confirm(player,0,env.pendingReligionChoice)); assert(#calls==1)
assert(env.pendingReligionChoice.failure=="belief_taken")
-- Missing readiness APIs must never be treated as earned beliefs.
taken=false; env.pendingReligionChoice=nil; calls={}; present=true; api.GetNumBeliefsEarned=nil
assert(order()); assert(#calls==1)
assert(not confirm(player,0,env.pendingReligionChoice)); assert(#calls==1)
-- The actual blocker entry completes the stored choice, once, after activation.
api.GetNumBeliefsEarned=function() return earned end
env.pendingReligionChoice=nil; calls={}; earned=0; present=true; assert(order())
earned=2; present=false; env.blockerName=function() return "ENDTURN_BLOCKING_BELIEF" end
local blocker=assert(source:match("(local function answerBlocker.-)%s+%-%- Firaxis's UnitPanel starts the Apostle operation"))
local answer=compile(blocker .. "\nend\nreturn answerBlocker")()
assert(answer(player,0,0,turn,false)=="civvis_complete" and #calls==4)
answer(player,0,0,turn,false); assert(#calls==4)
-- The exported gate survives loading an already activated, consumed Prophet.
local pending=assert(source:match("local prophet_pending = (.-);\n\t%-%-"))
env.playerReligion=api; env.religionCreated=-1
assert(compile("return " .. pending)()==true)
env.religionCreated=3; assert(compile("return " .. pending)()==false)
-- A refused or throwing activation must not leave a pending-choice lock.
env.pendingReligionChoice=nil; calls={}; earned=0; present=true
env.UnitManager.CanStartOperation=function() return false end
assert(not order() and env.pendingReligionChoice==nil and #calls==0)
env.UnitManager.CanStartOperation=function() return true end
env.UnitManager.RequestOperation=function() error("native request failed") end
assert(not order() and env.pendingReligionChoice==nil and #calls==0)

print("staged native religion founding checks passed")
