-- Complete the native artifact choice even when its notification never opens a popup.
-- Run: lua5.1 tools/civ6_control/mod/artifact_choice_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."
local function stub()
    return setmetatable({}, { __index = function() return stub() end,
        __call = function() return stub() end, __newindex = function() end })
end
setmetatable(_G, { __index = function() return stub() end })
CivvisControlConfig = {}
Automation = { Log = function() end }
local pending, artifact, requests = true, { ActingPlayerID = 7, TargetPlayerID = 3 }, {}
local unit = { GetArchaeology = function()
    return { GetArtifactIndex = function() return 42 end }
end }
Players = { [0] = { GetUnits = function()
    return { GetNextExtractingArchaeologist = function() return pending and unit or nil end }
end } }
Game = { GetArtifactByIndex = function(index) assert(index == 42); return artifact end }
PlayerOperations = { PARAM_PLAYER_ONE = 'player_one', CHOOSE_ARTIFACT_PLAYER = 91 }
UI = { RequestPlayerOperation = function(pid, op, params)
    requests[#requests + 1] = { pid = pid, op = op, chosen = params.player_one }
end }
assert(loadfile(here .. '/CivvisControlAgent.lua'))()
local choose = rawget(_G, 'CivvisChooseArtifactPlayer')
assert(type(choose) == 'function', 'artifact prompt needs a controller answer')
assert(choose(0))
assert(#requests == 1 and requests[1].pid == 0 and requests[1].op == 91)
assert(requests[1].chosen == 7, 'use the shipped first-choice civilization, not the unit owner')
pending = false
assert(not choose(0) and #requests == 1, 'no pending extraction must not submit')
pending = true; artifact = nil
assert(not choose(0) and #requests == 1, 'an unavailable artifact must not invent a choice')
artifact = { ActingPlayerID = 0, TargetPlayerID = 7 }
assert(choose(0) and requests[2].chosen == 0, 'player zero is a valid civilization')
UI.RequestPlayerOperation = function() error('native rejection') end
assert(not choose(0), 'a raised request must not report success')
local file = assert(io.open(here .. '/CivvisControlAgent.lua'))
local source = file:read('*a'); file:close()
assert(source:find('elseif name == "ENDTURN_BLOCKING_ARTIFACT" then', 1, true))
assert(source:find('answered = CivvisChooseArtifactPlayer(pid)', 1, true))
print('Artifact choice: pending, absent, unavailable, player zero and failed request passed')
