-- Exercise the shipped HUD wrapper under Lua 5.1, independent of Game Core.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local function host(stock, cfg)
    local env = setmetatable({ CivvisControlConfig = cfg }, { __index = _G })
    local result = { pulses = 0, includes = {} }
    env.include = function(name)
        result.includes[#result.includes + 1] = name
        if name ~= stock then error("unavailable expansion") end
        env.LateInitialize = function() end
        result.stockLoaded = true
    end
    env.ContextPtr = { SetUpdate = function(_, callback) result.update = callback end }
    env.LuaEvents = { CivvisControlPulse = function(source)
        assert(source == "TopPanel", "HUD pulse must identify its native context")
        result.pulses = result.pulses + 1
        if result.throw then error("listener temporarily unavailable") end
    end }
    local chunk = assert(loadfile(here .. "/CivvisControlHeartbeat.lua"))
    setfenv(chunk, env)
    chunk()
    return result
end
for _, stock in ipairs({"TopPanel_Expansion2", "TopPanel_Expansion1", "TopPanel"}) do
    local h = host(stock, { CivvisDecides = true })
    assert(h.stockLoaded, "stock HUD must load before the clock")
    assert(h.includes[#h.includes] == stock, "wrong expansion loaded")
    assert(type(h.update) == "function", "HUD clock not armed")
    h.update(nil); h.update(-1); h.update("invalid")
    assert(h.pulses == 0, "invalid deltas must not create a pulse")
    h.update(.5)
    assert(h.pulses == 0, "pulse too early")
    h.update(.5)
    assert(h.pulses == 1, "one elapsed second must pulse")
    h.update(100)
    assert(h.pulses == 2, "long frame must not burst")
    h.throw = true; h.update(1)
    h.throw = false; h.update(1)
    assert(h.pulses == 4, "a listener failure must not kill the clock")
end
for _, cfg in ipairs({{Play = false, CivvisDecides = true}, {CivvisDecides = false}, {}}) do
    local h = host("TopPanel", cfg)
    assert(h.stockLoaded, "disabled automation must preserve the stock HUD")
    assert(h.update == nil, "disabled automation must not arm a clock")
end
print("all HUD heartbeat checks passed")

-- The VSync A/B: off unless `VSyncABTurns` is set, and then even blocks run
-- with VSync on, odd blocks off, one switch per block, with the frame rate
-- each turn ran at.
local function abHost(cfg, throws)
    local h = { turn = 0, sets = {}, applies = 0, logs = {} }
    local env = setmetatable({ CivvisControlConfig = cfg }, { __index = _G })
    env.include = function(name)
        if name ~= "TopPanel" then error("unavailable expansion") end
        env.LateInitialize = function() end
    end
    env.ContextPtr = { SetUpdate = function(_, callback) h.update = callback end }
    env.LuaEvents = { CivvisControlPulse = function() end }
    env.Game = { GetCurrentGameTurn = function() return h.turn end }
    env.Options = {
        SetGraphicsOption = function(section, key, value)
            if throws then error("options unavailable") end
            assert(section == "Video" and key == "VSync", "only VSync may change")
            h.sets[#h.sets + 1] = value; h.current = value
        end,
        ApplyGraphicsOptions = function() h.applies = h.applies + 1; return true end,
        GetGraphicsOption = function() return h.current end,
    }
    env.Automation = { Log = function(line) h.logs[#h.logs + 1] = line end }
    local chunk = assert(loadfile(here .. "/CivvisControlHeartbeat.lua"))
    setfenv(chunk, env); chunk()
    return h
end
local function lastLog(h, kind)
    for i = #h.logs, 1, -1 do
        if h.logs[i]:find('"kind":"' .. kind .. '"', 1, true) then return h.logs[i] end
    end
end
local off = abHost({ CivvisDecides = true })
for _ = 1, 5 do off.update(1) end
assert(#off.sets == 0 and #off.logs == 0, "without VSyncABTurns the HUD must not touch VSync")
local ab = abHost({ CivvisDecides = true, VSyncABTurns = 2, RunTag = "ab-test" })
ab.update(1)
assert(ab.sets[1] == 1 and ab.applies == 1, "block 0 runs with VSync on")
assert(lastLog(ab, "vsync_ab"):find('"vsync":1', 1, true), "the switch is logged")
for _ = 1, 4 do ab.update(0.25) end
ab.turn = 1; ab.update(1)
assert(#ab.sets == 1, "no switch inside a block")
assert(lastLog(ab, "frame_rate"):find('"turn":0,"vsync":1,"fps":', 1, true),
    "a finished turn reports the frame rate it ran at")
ab.turn = 2; ab.update(1)
assert(ab.sets[2] == 0 and ab.applies == 2, "block 1 runs with VSync off")
assert(lastLog(ab, "vsync_ab"):find('"turn":2,"vsync":0,"applied":true,"read_back":0', 1, true),
    "the switch reads its value back")
assert(lastLog(ab, "frame_rate"):find('"run":"ab-test"', 1, true), "events carry the run tag")
ab.turn = 3; ab.update(1); ab.turn = 4; ab.update(1)
assert(ab.sets[3] == 1 and #ab.sets == 3, "block 2 switches VSync back on")
local broken = abHost({ CivvisDecides = true, VSyncABTurns = 2 }, true)
for _ = 1, 3 do broken.update(1) end
assert(#broken.sets == 0 and broken.applies == 0, "a throwing options API never applies")
assert(lastLog(broken, "vsync_ab"):find('"applied":false', 1, true), "the failure is logged")
local tries = 0
for _, line in ipairs(broken.logs) do
    if line:find('"kind":"vsync_ab"', 1, true) then tries = tries + 1 end
end
assert(tries == 1, "a failing switch is tried once per block, not once a second")
print("all VSync A/B checks passed")

-- The HUD receives no frames while a visible leader overlay covers it.
-- Exercise the real popup wrapper with a stock close callback that stays up.
local function popupHost(cfg)
    local h = { pulses = 0, hidden = false, closes = 0 }
    local env = setmetatable({ CivvisControlConfig = cfg }, { __index = _G })
    env.include = function() end
    env.OnClose = function() h.closes = h.closes + 1 end
    env.OnShow = function() h.stockShows = (h.stockShows or 0) + 1 end
    env.ContextPtr = {
        GetID = function() return "NaturalWonderPopup" end,
        IsHidden = function() return h.hidden end,
        SetUpdate = function(_, f) h.update = f end,
        SetShowHandler = function(_, f) h.show = f end,
    }
    env.Automation = { Log = function() end }
    env.LuaEvents = { CivvisControlPulse = function(source)
        assert(source == "NaturalWonderPopup", "popup pulse must identify its native context")
        h.pulses = h.pulses + 1
        if h.hideOnPulse then h.hidden = true end
        if h.throw then error("listener unavailable") end
    end }
    local chunk = assert(loadfile(here .. "/CivvisControlAutoClose.lua"))
    setfenv(chunk, env); chunk()
    return h
end
local p = popupHost({CivvisDecides = true, AnnouncementSeconds = 1000})
assert(type(p.update) == "function")
p.update(.5); assert(p.pulses == 0)
p.update(.5); assert(p.pulses == 1, "visible popup must wake agent without HUD frames")
p.update(100); assert(p.pulses == 2, "popup long frame must not burst")
p.throw = true; p.update(1); p.throw = false; p.update(1)
assert(p.pulses == 4, "failed pulse must not stop popup clock")
p.update(.5); p.hidden = true; p.update(10)
assert(p.pulses == 4, "hidden popup must not pulse")
p.hidden = false; p.update(.5)
assert(p.pulses == 4, "hidden reset must discard elapsed pulse time")
p.show(); p.update(.5)
assert(p.stockShows == 1 and p.pulses == 4, "show must preserve stock callback and reset clock")
p.update(.5); assert(p.pulses == 5)
for _, cfg in ipairs({{Play = false, CivvisDecides = true}, {CivvisDecides = false}, {}}) do
    local h = popupHost(cfg)
    h.update(2)
    assert(h.pulses == 0, "disabled controller must not be pulsed by a popup")
    assert(h.closes > 0, "popup's stock close handling must remain intact")
end
print("all popup heartbeat checks passed")

-- A remembered diplomacy ID is not proof of a live session. Exercise the
-- actual popup update callback: stale contexts should stop ticking, while an
-- open session or failed native query must never authorize a forced hide.
local function diplomacyHost(mode)
    local h = { hidden = false, nativeCloses = 0, queries = {}, open = false }
    local env = setmetatable({
        CivvisControlConfig = { AnnouncementSeconds = 0.05, DialogueSeconds = 0.05 },
        ms_ActiveSessionID = 17,
    }, { __index = _G })
    env.include = function() end
    env.OnClose = function() end
    env.Close = function()
        h.nativeCloses = h.nativeCloses + 1
        if mode == "replacement" then
            env.ms_ActiveSessionID, h.open = 18, true
        end
    end
    env.DiplomacyManager = {}
    if mode ~= "missing" then
        env.DiplomacyManager.IsSessionIDOpen = function(id)
            h.queries[#h.queries + 1] = id
            if mode == "throw" then error("host read failed") end
            if mode == "unknown" then return nil end
            return mode == "open" or h.open
        end
    end
    env.m_PopupDialog = { IsOpen = function() return mode == "popup" end }
    env.ContextPtr = {
        GetID = function() return "DiplomacyActionView" end,
        IsHidden = function() return h.hidden end,
        SetHide = function(_, hidden) h.hidden = hidden end,
        SetUpdate = function(_, f) h.update = f end,
    }
    env.Automation = { Log = function() end }
    env.LuaEvents = {}
    local chunk = assert(loadfile(here .. "/CivvisControlAutoClose.lua"))
    setfenv(chunk, env); chunk()
    h.update(1)
    return h
end
local stale = diplomacyHost("closed")
assert(stale.hidden and stale.nativeCloses == 1,
       "closed remembered session must use native Close before hiding its stale context")
assert(#stale.queries == 2 and stale.queries[1] == 17 and stale.queries[2] == 17,
       "native session state must be checked again after Close")
for _, mode in ipairs({"open", "throw", "unknown", "missing", "popup"}) do
    local h = diplomacyHost(mode)
    assert(not h.hidden, mode .. " must not authorize a stale-context hide")
end
local replacement = diplomacyHost("replacement")
assert(not replacement.hidden and replacement.queries[2] == 18,
       "native Close may open a replacement session; leave its context visible")
print("all native diplomacy session checks passed")

local closedByAgent = popupHost({CivvisDecides = true, AnnouncementSeconds = 0})
closedByAgent.hideOnPulse = true
closedByAgent.update(1)
assert(closedByAgent.pulses == 1 and closedByAgent.closes == 0,
       "a pulse that closes the popup must not run its native close again")

-- The landed-orders peek rides the same per-frame clock: one
-- `CivvisControlPeek` every `OrdersPeekSeconds` (floored at 20 ms), never a
-- burst after a long frame, and a failing listener cannot stop the clock.
-- The 1 s pulse is unchanged beside it.
local function peekHost(cfg)
    local h = { peeks = 0, pulses = 0 }
    local env = setmetatable({ CivvisControlConfig = cfg }, { __index = _G })
    env.include = function(name)
        if name ~= "TopPanel" then error("unavailable expansion") end
        env.LateInitialize = function() end
    end
    env.ContextPtr = { SetUpdate = function(_, callback) h.update = callback end }
    env.LuaEvents = {
        CivvisControlPulse = function() h.pulses = h.pulses + 1 end,
        CivvisControlPeek = function()
            h.peeks = h.peeks + 1
            if h.throw then error("listener temporarily unavailable") end
        end,
    }
    local chunk = assert(loadfile(here .. "/CivvisControlHeartbeat.lua"))
    setfenv(chunk, env); chunk()
    return h
end
local pk = peekHost({ CivvisDecides = true })
pk.update(0.03); assert(pk.peeks == 0, "peek too early")
pk.update(0.03); assert(pk.peeks == 1, "50 ms of frames offer one peek")
for _ = 1, 10 do pk.update(0.05) end
assert(pk.peeks == 11, "one peek per 50 ms")
pk.update(5); assert(pk.peeks == 12, "a long frame offers one peek, not a burst")
pk.throw = true; pk.update(0.05); pk.throw = false; pk.update(0.05)
assert(pk.peeks == 14, "a failing peek listener must not stop the clock")
assert(pk.pulses == 1, "the 1 s pulse still fires on its own cadence")
local slow = peekHost({ CivvisDecides = true, OrdersPeekSeconds = 0.2 })
for _ = 1, 3 do slow.update(0.05) end
assert(slow.peeks == 0, "a configured interval is honoured")
slow.update(0.06); assert(slow.peeks == 1)
local floor = peekHost({ CivvisDecides = true, OrdersPeekSeconds = 0 })
floor.update(0.01); assert(floor.peeks == 0, "the interval is floored at 20 ms")
floor.update(0.011); assert(floor.peeks == 1)
for _, cfg in ipairs({{Play = false, CivvisDecides = true}, {CivvisDecides = false}}) do
    local h = peekHost(cfg)
    assert(h.update == nil, "disabled automation must not arm the peek clock")
end
print("all landed-orders peek clock checks passed")

-- At a 2x debug timescale the frame deltas run twice as fast, and the agent's
-- pulse (which every pulse-counted timer inherits) still comes once per REAL
-- second: each delta is divided by the scale the agent shares. The Heartbeat
-- acknowledges the scale it read, and reports how its frame deltas compare
-- with the UI clock, so the agent can revert a timescale they disagree with.
do
    local env = setmetatable({ CivvisControlConfig = { CivvisDecides = true } }, { __index = _G })
    local h = { pulses = 0, ui = 100 }
    env.include = function(name)
        if name ~= "TopPanel" then error("unavailable expansion") end
        env.LateInitialize = function() end
    end
    env.ContextPtr = { SetUpdate = function(_, callback) h.update = callback end }
    env.LuaEvents = { CivvisControlPulse = function() h.pulses = h.pulses + 1 end,
                      CivvisControlPeek = function() end }
    env.UI = { GetElapsedTime = function() return h.ui end }
    env.ExposedMembers = { CivvisTimeScale = 2, CivvisClockAck = { scale = {}, at = {} } }
    local chunk = assert(loadfile(here .. "/CivvisControlHeartbeat.lua"))
    setfenv(chunk, env)
    chunk()
    local function frame(dt) h.ui = h.ui + dt; h.update(dt) end
    frame(1.5)
    assert(h.pulses == 0, "at 2x, 1.5 frame seconds (0.75 real) must not pulse")
    frame(0.6)
    assert(h.pulses == 1, "at 2x, 2.1 frame seconds (1.05 real) pulse once")
    assert(env.ExposedMembers.CivvisClockAck.scale.Heartbeat == 2, "the Heartbeat acknowledges the scale")
    assert(env.ExposedMembers.CivvisClockAck.at.Heartbeat == h.ui, "…with the UI time it read it")
    for _ = 1, 120 do frame(0.1) end
    local fc = env.ExposedMembers.CivvisFrameClock
    assert(fc ~= nil and fc.scale == 2 and fc.ratio == 1, "frame deltas that keep pace with the UI clock read 1")
    -- Frame deltas that do not scale while the UI clock does read 0.5.
    for _ = 1, 120 do h.ui = h.ui + 0.2; h.update(0.1) end
    assert(env.ExposedMembers.CivvisFrameClock.ratio == 0.5, "unscaled frame deltas read 0.5")
    -- Only consecutive frames count. A hidden HUD (a leader screen stops this
    -- SetUpdate while the UI clock runs on: G98 read 0.48) and a long hitch
    -- whose delta the engine caps are gaps, not missing frame time.
    env.ExposedMembers.CivvisFrameClock = nil
    env.CivvisFrameClock.raw, env.CivvisFrameClock.ui = 0, 0  -- a fresh window
    for _ = 1, 60 do frame(0.1) end
    h.ui = h.ui + 5.0                       -- 5 UI-s with no frames at all
    for _ = 1, 30 do frame(0.1) end
    h.ui = h.ui + 0.8; h.update(0.2)        -- one long frame, its delta capped
    for _ = 1, 30 do frame(0.1) end
    fc = env.ExposedMembers.CivvisFrameClock
    assert(fc ~= nil and fc.ratio == 1, "a hidden stretch and a capped hitch read 1, not 0.48: " .. tostring(fc and fc.ratio))
    -- Unscaled deltas still read 0.5 in every ordinary frame.
    env.ExposedMembers.CivvisFrameClock = nil
    env.CivvisFrameClock.raw, env.CivvisFrameClock.ui = 0, 0
    for _ = 1, 120 do h.ui = h.ui + 0.2; h.update(0.1) end
    assert(env.ExposedMembers.CivvisFrameClock.ratio == 0.5, "unscaled deltas across consecutive frames read 0.5")
    -- A missing or nonsense scale reads as 1, and is acknowledged as 1.
    env.ExposedMembers.CivvisTimeScale = "fast"
    frame(0.1)
    assert(env.ExposedMembers.CivvisClockAck.scale.Heartbeat == 1, "a nonsense scale reads as 1")
    local before = h.pulses
    frame(1.0)
    assert(h.pulses == before + 1, "at scale 1 one frame second pulses")
end
print("all real-seconds heartbeat checks passed")
