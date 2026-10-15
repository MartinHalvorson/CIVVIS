local here = arg[0]:match("(.*)/[^/]*$") or "."
local f = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = f:read("*a"); f:close()
local block = assert(source:match('(CivvisTiles.improvementOwner = function.-\nend;)'))
local shown, owner, calls = true, 0, 0
local env = setmetatable({
    CivvisTiles = {improvementOwners = {}},
    PlayersVisibility = setmetatable({}, {__index = function()
        return {IsVisible = function() return shown end}
    end}),
    try = function(fn, fallback)
        local ok, value = pcall(fn)
        if ok then return value end
        return fallback
    end,
}, {__index = _G})
local chunk = assert(loadstring(block)); setfenv(chunk, env); chunk()
local plot = {
    GetImprovementType = function() return 4 end,
    GetImprovementOwner = function() calls = calls + 1; return owner end,
}
local read = env.CivvisTiles.improvementOwner
assert(read(plot, 0, 24, 21) == 0, "player zero is an owner, not missing")
shown, owner = false, 7
assert(read(plot, 0, 24, 21) == 0 and calls == 1,
    "fog must retain the observed owner without querying the new one")
assert(read(plot, 1, 24, 21) == nil, "one viewer cannot borrow another's sight")
shown = true
assert(read(plot, 0, 24, 21) == 7)
owner = -1
assert(read(plot, 0, 24, 21) == nil, "an unavailable owner clears a visible old reading")
owner = 1.5
assert(read(plot, 0, 24, 21) == nil)
plot.GetImprovementOwner = function() error("unavailable") end
assert(read(plot, 0, 24, 21) == nil)
shown = false
assert(read(plot, 0, 24, 21) == nil)
plot.GetImprovementType = function() return -1 end
assert(read(plot, 0, 24, 21) == nil, "bare ground has no improvement owner")
assert(source:find('io = CivvisTiles.improvementOwner(plot, pid, x, y)', 1, true))
local signature = assert(source:match('(mark = %(owner %* 1024.-)\n\t\t\t\tend'))
assert(signature:find('tostring(CivvisTiles.improvementOwner(plot, pid, x, y))', 1, true),
    "an owner-only change must invalidate the tile delta")
print("native improvement owner export checks passed")
