-- The victory event names the host's victory type next to its integer index.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local f = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = f:read("*a"); f:close()
local expr = assert(source:match("victory_type = (try%(function%(%).-end, nil%)),"),
	"the victory event must carry victory_type")
local env = setmetatable({
	try = function(fn, fallback) local ok, v = pcall(fn); if ok then return v end; return fallback end,
	GameInfo = { Victories = {} },
	victoryType = 3,
}, {__index = _G})
local chunk = assert(loadstring("return " .. expr)); setfenv(chunk, env)
env.GameInfo.Victories[3] = { VictoryType = "VICTORY_CULTURE" }
assert(chunk() == "VICTORY_CULTURE", "index 3 reads as its host row's type")
env.victoryType = 4
assert(chunk() == nil, "an index with no row stays unknown")
env.GameInfo = nil
assert(chunk() == nil, "a missing table must not break the event")
print("victory type export checks passed")
