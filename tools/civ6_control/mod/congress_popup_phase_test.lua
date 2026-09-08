-- Exercise the real popup timer and both native Congress close paths.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local function host(closeName, segment)
    local h = { segment = segment, ballots = 0, closes = 0 }
    local env = setmetatable({}, { __index = _G })
    env.CivvisControlConfig = {AnnouncementSeconds = 0, CivvisDecides = true}
    env.include = function() end
    env[closeName] = function() h.closes = h.closes + 1 end
    env.ContextPtr = {
        GetID = function() return "WorldCongressPopup" end,
        IsHidden = function() return false end,
        SetUpdate = function(_, f) h.update = f end,
    }
    env.Automation = { Log = function() end }
    env.Game = { GetCurrentTurnSegment = function()
        if h.badRead then error("unavailable phase") end
        return h.segment
    end }
    env.DB = { MakeHash = function(name) return name end }
    env.LuaEvents = { CivvisCongressBallot = function()
        h.ballots = h.ballots + 1
        if h.badListener then error("agent unavailable") end
    end }
    local chunk = assert(loadfile(here .. "/CivvisControlAutoClose.lua"))
    setfenv(chunk, env); chunk()
    return h
end
for _, closer in ipairs({"OnPass", "OnAccept"}) do
    for _, segment in ipairs({"TURNSEG_WORLDCONGRESS_1", "TURNSEG_WORLDCONGRESS_2"}) do
        local h = host(closer, segment)
        h.update(.1)
        assert(h.ballots == 1 and h.closes == 1, "active voting must precede native close")
        -- Reopen/retain the same popup for review on a later ordinary turn.
        h.segment = "TURNSEG_GAMEPLAY"
        h.update(.1)
        assert(h.ballots == 1 and h.closes == 2, "review must close without recasting")
    end
    for _, segment in ipairs({"TURNSEG_GAMEPLAY", "TURNSEG_WORLDCONGRESS_RESOLUTION", false}) do
        local h = host(closer, segment or nil)
        h.update(.1)
        assert(h.ballots == 0 and h.closes == 1, "non-voting popup must not cast")
    end
    local unreadable = host(closer, "TURNSEG_WORLDCONGRESS_1")
    unreadable.badRead = true; unreadable.update(.1)
    assert(unreadable.ballots == 0 and unreadable.closes == 1, "unreadable phase must still close")
    local broken = host(closer, "TURNSEG_WORLDCONGRESS_1")
    broken.badListener = true; broken.update(.1)
    assert(broken.ballots == 1 and broken.closes == 1, "listener failure must not prevent close")
end
print("all Congress popup phase checks passed")
