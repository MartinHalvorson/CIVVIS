-- Complete the native artifact choice even when its notification never opens a popup.
-- Run: lua5.1 tools/civ6_control/mod/artifact_choice_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."
local function stub()
    return setmetatable({}, { __index = function() return stub() end,
        __call = function() return stub() end, __newindex = function() end })
end
setmetatable(_G, { __index = function() return stub() end })
CivvisControlConfig = {}
local logs = {}
Automation = { Log = function(line) logs[#logs + 1] = line end }
local pending, artifact, requests = true, { ActingPlayerID = 7, TargetPlayerID = 3 }, {}
local unit = { GetID = function() return 92 end, GetArchaeology = function()
    return { GetArtifactIndex = function() return 42 end }
end }
Players = { [0] = { GetUnits = function()
    return { GetNextExtractingArchaeologist = function() return pending and unit or nil end }
end } }
Game = { GetCurrentGameTurn = function() return 156 end,
    GetArtifactByIndex = function(index) assert(index == 42); return artifact end }
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
local function witnesses()
    local found = {}
    for _, line in ipairs(logs) do
        if line:find('"kind":"artifact_extraction_pending"', 1, true) then found[#found + 1] = line end
    end
    return found
end
assert(#witnesses() == 1)
local proof = witnesses()[1]
for _, field in ipairs({ '"unit":92', '"artifact":42', '"turn":156', '"player":0' }) do
    assert(proof:find(field, 1, true), 'extraction witness missing ' .. field)
end
pending = false
assert(not choose(0) and #requests == 1, 'no pending extraction must not submit')
assert(#witnesses() == 1, 'no extracting unit must not invent a witness')
pending = true; artifact = nil
assert(not choose(0) and #requests == 1, 'an unavailable artifact must not invent a choice')
assert(#witnesses() == 1, 'missing native artifact must not invent a witness')
artifact = { ActingPlayerID = 0, TargetPlayerID = 7 }
assert(choose(0) and requests[2].chosen == 0, 'player zero is a valid civilization')
unit.GetID = function() error('identity unavailable') end
assert(choose(0) and #requests == 3, 'unavailable telemetry must not prevent the artifact choice')
assert(#witnesses() == 2, 'an unknown identity must not invent a witness')
unit.GetID = function() return 92 end
UI.RequestPlayerOperation = function() error('native rejection') end
assert(not choose(0), 'a raised request must not report success')
assert(#witnesses() == 3, 'extraction is witnessed independently of a failed civilization choice')
local file = assert(io.open(here .. '/CivvisControlAgent.lua'))
local source = file:read('*a'); file:close()
assert(source:find('elseif name == "ENDTURN_BLOCKING_ARTIFACT" then', 1, true))
assert(source:find('answered = CivvisChooseArtifactPlayer(pid)', 1, true))
print('Artifact choice: pending, absent, unavailable, player zero and failed request passed')
