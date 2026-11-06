-- The receiver reports effective timing settings used by CivvisQueue.drain.
-- Drive the shipped survey function, not a second copy of its settings table.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local function stub()
 return setmetatable({}, { __index = function() return stub() end,
                          __call = function() return stub() end })
end
setmetatable(_G, {__index=function() return stub() end})
local lines = {}
Automation = {Log=function(line) lines[#lines+1]=line end}
GameInfo = setmetatable({}, {__index=function() return {} end})
Game = {GetLocalPlayer=function() return -1 end, GetCurrentGameTurn=function() return 0 end,
        IsVictoryEnabled=function() return true end}
GameConfiguration = setmetatable({}, {__index=function() return function() return nil end end})
MapConfiguration = setmetatable({}, {__index=function() return function() return nil end end})
PlayerManager = {GetAliveIDs=function() return {} end, GetAliveMajorIDs=function() return {} end}
Players = {}
local function survey(config)
 config.Survey = true
 config.SurveyEnums = false
 CivvisControlConfig = config
 assert(pcall(assert(loadfile(here.."/CivvisControlAgent.lua"))))
 lines = {}
 local actual = rawget(_G,"CivvisSurvey")
 assert(type(actual)=="function", "the shipped survey was not exported")
 actual()
 for _,line in ipairs(lines) do
  if line:find('"kind":"seat"',1,true) then return line end
 end
 error("the real survey did not publish a seat event")
end
local function expect(line,idle,noop,grace)
 assert(line:find('"order_queue_idle_probe_ticks":'..idle,1,true),line)
 assert(line:find('"order_queue_noop_probe_ticks":'..noop,1,true),line)
 assert(line:find('"order_queue_grace_ticks":'..grace,1,true),line)
end
expect(survey({}),2,8,30)
expect(survey({OrderQueueIdleProbeTicks=8,OrderQueueNoopProbeTicks=12,OrderQueueGraceTicks=40}),8,12,40)
expect(survey({OrderQueueIdleProbeTicks="9",OrderQueueNoopProbeTicks="bad",OrderQueueGraceTicks="31"}),9,8,31)
print("queue timing seat readback: existing defaults, explicit overrides and consumer numeric fallback pass")
