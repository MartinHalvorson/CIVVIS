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

-- RECORD-ONLY ui_gap: two or more wall seconds between consecutive frames is
-- logged with the UI clock's jump and the frame's own (capped) delta; an
-- ordinary second of frames is not, and a host without the clock logs nothing.
local gap = abHost({ CivvisDecides = true, RunTag = "gap-test" })
gap.wall, gap.ui = 100, 50
local genv = getfenv(gap.update)
genv.Automation.GetTime = function() return gap.wall end
genv.UI = { GetElapsedTime = function() return gap.ui end }
gap.update(0.01)
for _ = 1, 10 do gap.ui = gap.ui + 0.1; gap.update(0.1) end
gap.wall = 101; gap.ui = gap.ui + 0.1; gap.update(0.1)
assert(lastLog(gap, "ui_gap") == nil, "a one-second clock tick is not a gap")
gap.wall = 105; gap.ui = gap.ui + 0.5; gap.turn = 20; gap.update(0.5)
local row = lastLog(gap, "ui_gap")
assert(row and row:find('"wall":4,"ui":0.50,"dt":0.500,"turn":20', 1, true), "a stopped loop is logged: " .. tostring(row))
assert(row:find('"run":"gap-test"', 1, true), "the gap carries the run tag")
gap.wall = 106; gap.update(0.1)
local n = 0
for _, line in ipairs(gap.logs) do if line:find('"kind":"ui_gap"', 1, true) then n = n + 1 end end
assert(n == 1, "one gap, one row")
genv.Automation.GetTime = nil
gap.update(0.1); gap.update(0.1)
print("all ui-gap checks passed")
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

-- The seat watch (`CivvisSeatWatch`): a CIVVIS hotseat seat whose turn is
-- active while its controller's stamp does not move. Tournament 6, 2026-11-21:
-- 12 minutes at civvis-t6-20261121T131243Z t61 and 7 at
-- civvis-t6-20261121T135836Z t180, the map on NEXT TURN and no record at all.
local function seatHost(cfg, before)
    local h = { clock = 1000, logs = {}, requests = {}, broadcasts = 0, stockTicks = 0,
                local_player = 0, turn = 61, active = true, paused = false, sent = false,
                handover_hidden = true, dialog_hidden = true, end_game_hidden = true,
                blocker = 0 }
    local env = setmetatable({ CivvisControlConfig = cfg }, { __index = _G })
    env.include = function(name)
        if name ~= "TopPanel" then error("unavailable expansion") end
        env.LateInitialize = function() h.registered = env.OnRefreshTimeTick end
        env.OnRefreshTimeTick = function() h.stockTicks = h.stockTicks + 1 end
    end
    env.ContextPtr = {
        SetUpdate = function(_, callback) h.update = callback end,
        LookUpControl = function(_, path)
            local hidden
            if path == "/InGame/PlayerChange" then hidden = h.handover_hidden
            elseif path == "/InGame/PlayerChange/PopupAlphaIn" then hidden = h.dialog_hidden
            elseif path == "/InGame/EndGameMenu" then hidden = h.end_game_hidden
            else error("unknown control " .. path) end
            return { IsHidden = function() return hidden end }
        end,
    }
    env.LuaEvents = { CivvisControlPulse = function() end, CivvisControlPeek = function() end }
    env.Automation = {
        GetTime = function() return h.clock end,
        Log = function(line) h.logs[#h.logs + 1] = line end,
    }
    env.Game = { GetLocalPlayer = function() return h.local_player end,
                 GetCurrentGameTurn = function() return h.turn end }
    env.Players = setmetatable({}, { __index = function() return {
        IsTurnActive = function() return h.active end,
        CanUnreadyTurn = function() return h.can_unready == true end,
        GetUnits = function() return { GetFirstReadyUnit = function()
            if h.ready == nil then return nil end
            return { GetUnitType = function() return h.ready end }
        end } end } end })
    env.GameInfo = { Units = setmetatable({}, { __index = function(_, index)
        return { UnitType = index } end }) }
    env.PlayerConfigurations = setmetatable({}, { __index = function() return {
        GetWantsPause = function() return h.paused end,
        SetWantsPause = function(_, value) h.paused = value; h.pause_set = value end } end })
    env.Network = { BroadcastPlayerInfo = function() h.broadcasts = h.broadcasts + 1 end }
    env.UI = {
        HasSentTurnComplete = function() return h.sent end,
        CanEndTurn = function() return true end,
        IsGameCoreBusy = function() return false end,
        IsProcessingMessages = function() return h.processing == true end,
        RequestAction = function(action, parameters)
            h.requests[#h.requests + 1] = { action = action, reason = parameters and parameters.REASON }
        end,
    }
    env.ActionTypes = { ACTION_ENDTURN = "ACTION_ENDTURN", ACTION_UNREADYTURN = "ACTION_UNREADYTURN" }
    env.GameConfiguration = { IsPaused = function() return h.game_paused == true end }
    env.EndTurnBlockingTypes = { NO_ENDTURN_BLOCKING = 0, ENDTURN_BLOCKING_UNITS = 7 }
    env.NotificationManager = { GetFirstEndTurnBlocking = function() return h.blocker end }
    env.ExposedMembers = { CivvisSeatBeat = {} }
    -- Every listener an event is given, called in the order added.
    h.listeners = {}
    env.Events = setmetatable({}, { __index = function(events, name)
        local event = { Add = function(listener)
            h.listeners[name] = h.listeners[name] or {}
            table.insert(h.listeners[name], listener)
        end }
        rawset(events, name, event)
        return event
    end })
    h.fire = function(name, ...)
        for _, listener in ipairs(h.listeners[name] or {}) do listener(...) end
    end
    h.victory = function(...) h.fire("TeamVictory", ...) end
    if before ~= nil then before(env, h) end
    local chunk = assert(loadfile(here .. "/CivvisControlHeartbeat.lua"))
    setfenv(chunk, env)
    chunk()
    env.LateInitialize()
    h.env = env
    -- One second passes: the frame clock's pulse, then the shipped clock's timer.
    h.second = function(playing)
        h.clock = h.clock + 1
        if playing then env.ExposedMembers.CivvisSeatBeat[h.local_player] = h.clock end
        h.update(1)
        h.registered()
    end
    h.kinds = function(kind)
        local found = {}
        for _, line in ipairs(h.logs) do
            if line:find('"kind":"' .. kind .. '"', 1, true) then found[#found + 1] = line end
        end
        return found
    end
    return h
end
local HOTSEAT = { CivvisDecides = true, Hotseat = true, HotseatSeats = { 0, 1 },
                  HotseatHumanSeats = {}, RunTag = "table" }
do
    -- Armed on the shipped clock's timer, which keeps repainting the time.
    local h = seatHost(HOTSEAT)
    assert(#h.kinds("seat_watch_armed") == 1, "the watch says it is armed")
    assert(h.logs[1]:find('"clock":true', 1, true), "armed on the shipped clock timer")
    h.registered()
    assert(h.stockTicks == 1, "the shipped clock tick still runs")
    -- A seat being played is left alone (until its turn has stood
    -- `SeatLongTurnSeconds`, 180 s: see the long-turn checks below).
    for _ = 1, 100 do h.second(true) end
    assert(#h.requests == 0 and #h.kinds("seat_quiet") == 0, "a playing seat is never ended")
    -- The controller stops: 44 quiet seconds are nothing, the 45th is a stall.
    for _ = 1, 44 do h.second(false) end
    assert(#h.requests == 0 and #h.kinds("seat_quiet") == 0, "44 quiet seconds are not a stall")
    h.second(false)
    assert(#h.kinds("seat_quiet") == 1, "45 quiet seconds are reported")
    assert(#h.requests == 1 and h.requests[1].action == "ACTION_ENDTURN"
        and h.requests[1].reason == "UserForced", "the turn is ended with the shipped forced request")
    local quiet = h.kinds("seat_quiet")[1]
    for _, piece in ipairs({ '"ctx":"heartbeat"', '"run":"table"', '"turn":61', '"player":0',
            '"paused":false', '"sent":false', '"blocker":null', '"handover_hidden":true',
            '"controller_ran":true', '"quiet":45' }) do
        assert(quiet:find(piece, 1, true), "seat_quiet lacks " .. piece .. ": " .. quiet)
    end
    assert(h.logs[#h.logs]:find('"kind":"seat_watch_flush"', 1, true),
        "a record behind each report delivers it (the game writes one record late)")
    -- Retried every 20 s while it stays quiet, six times a turn, reported once a minute.
    for _ = 1, 19 do h.second(false) end
    assert(#h.requests == 1, "not asked again inside 20 s")
    h.second(false)
    assert(#h.requests == 2, "asked again after 20 s")
    for _ = 1, 400 do h.second(false) end
    assert(#h.requests == 6, "six forced requests a turn at most: " .. #h.requests)
    assert(#h.kinds("seat_quiet") == 8, "reported once a minute: " .. #h.kinds("seat_quiet"))
    -- The seat's next activation starts clean, in the same game turn too
    -- (the World Congress phase hands a seat a second turn inside one).
    h.active = false; h.second(false); h.active = true
    local asked = #h.requests
    for _ = 1, 44 do h.second(false) end
    assert(#h.requests == asked, "a new activation gets its own 45 s")
    for _ = 1, 3 do h.second(false) end
    assert(#h.requests == asked + 1, "and is ended when it too stays quiet")
    -- An unknown blocker is named by its number, a known one by its name.
    h.active = false; h.second(false); h.active = true
    h.blocker = 7
    for _ = 1, 47 do h.second(false) end
    assert(h.kinds("seat_quiet")[#h.kinds("seat_quiet")]:find('"blocker":"ENDTURN_BLOCKING_UNITS"', 1, true),
        "the standing blocker is named")
end
do
    -- A turn already sent complete, and a seat no controller ever ran on,
    -- are reported and never ended from here.
    local h = seatHost(HOTSEAT)
    h.sent = true
    h.second(true)
    for _ = 1, 120 do h.second(false) end
    assert(#h.kinds("seat_quiet") == 2 and #h.requests == 0, "a sent turn is the engine's to finish")
    h = seatHost(HOTSEAT)
    for _ = 1, 120 do h.second(false) end
    assert(#h.requests == 0, "no controller ever ran: nothing of ours to end")
    assert(h.kinds("seat_quiet")[1]:find('"controller_ran":false', 1, true), "and the report says so")
end
do
    -- The hand-over's pause. With its Start Turn dialog up it is the screen
    -- closer's to press, for as long as it takes.
    local h = seatHost(HOTSEAT)
    h.second(true)
    h.paused, h.handover_hidden, h.dialog_hidden = true, false, false
    for _ = 1, 300 do h.second(false) end
    assert(h.pause_set == nil and #h.requests == 0 and #h.kinds("seat_quiet") == 0,
        "a hand-over on screen is left to its own press")
    -- Left standing with no dialog (hidden, or the screen in Please Wait), it
    -- is released after six seconds the way Start Turn releases it.
    h.dialog_hidden = true
    for _ = 1, 5 do h.second(false) end
    assert(h.pause_set == nil, "five seconds of a bare pause are not yet a stuck one")
    h.second(false)
    assert(h.pause_set == false and h.broadcasts == 1, "a pause with no dialog is released and broadcast")
    local released = h.kinds("hotseat_pause_released")
    assert(#released == 1 and released[1]:find('"still_paused":false', 1, true)
        and released[1]:find('"waited":6', 1, true), "and the release is read back: " .. tostring(released[1]))
    -- The seat then has its 45 s again before anything is ended.
    for _ = 1, 44 do h.second(false) end
    assert(#h.requests == 0, "a released seat gets its own 45 s")
    h.second(false)
    assert(#h.requests == 1, "and is ended if its controller still does not move")
    -- A hand-over that cannot be read is taken as up: nothing is released blind.
    h = seatHost(HOTSEAT)
    h.second(true)
    h.paused = true
    h.env.ContextPtr.LookUpControl = function() error("no such control") end
    for _ = 1, 120 do h.second(false) end
    assert(h.pause_set == nil and #h.kinds("hotseat_pause_released") == 0 and #h.requests == 0,
        "an unreadable hand-over is never unpaused or ended")
    -- A release the engine refuses is tried again, not looped on every frame.
    h = seatHost(HOTSEAT)
    h.second(true)
    h.paused = true
    h.env.PlayerConfigurations = setmetatable({}, { __index = function() return {
        GetWantsPause = function() return true end,
        SetWantsPause = function() h.refused = (h.refused or 0) + 1 end } end })
    for _ = 1, 30 do h.second(false) end
    assert(h.refused == 5, "one release attempt per six seconds: " .. tostring(h.refused))
    assert(h.kinds("hotseat_pause_released")[1]:find('"still_paused":true', 1, true),
        "a refused release says the pause still stands")
end
do
    -- Nobody else's turn is ever touched.
    for _, change in ipairs({
        function(h) h.local_player = 3 end,                        -- a Firaxis AI's turn
        function(h) h.active = false end,                          -- between turns
        function(h) h.end_game_hidden = false end,                 -- the end screen is up
        function(h) h.env.ExposedMembers.CivvisOperatorHold = true end,  -- the operator's pause
        function(h) h.victory() end,                               -- the game is over
    }) do
        local h = seatHost(HOTSEAT)
        h.second(true)
        change(h)
        for _ = 1, 200 do h.second(false) end
        assert(#h.requests == 0 and h.pause_set == nil and #h.kinds("seat_quiet") == 0,
            "the watch acted where it must not")
    end
    -- A person's seat: their turn is theirs for as long as they like.
    local human = { CivvisDecides = true, Hotseat = true, HotseatSeats = { 1, 2, 3 },
                    HotseatHumanSeats = { 0 }, RunTag = "table" }
    local h = seatHost(human)
    h.paused = true
    for _ = 1, 600 do h.second(false) end
    assert(#h.requests == 0 and h.pause_set == nil, "the operator's seat is never ended or unpaused")
    -- ...and a stub elsewhere at the table changes nothing about theirs.
    h = seatHost({ CivvisDecides = true, Hotseat = true, HotseatSeats = { 1, 2 },
                   HotseatHumanSeats = { 0 }, HotseatStubSeats = { 3 }, RunTag = "table" })
    h.second(true)
    for _ = 1, 600 do h.second(false) end
    assert(#h.requests == 0, "a person's seat with no stub in it is never ended")
    -- The `--auto-human` test stubs are watched like a CIVVIS seat: `gated`
    -- lists seat 0 as the person's AND the stub's (run cover-live-6 went quiet
    -- on the stub's turn 1 and nothing ended it), `stub` lists it as a stub only.
    -- A stub's turn takes half a second, so its whole turn standing
    -- `StubTurnSeconds` (30) is ended, before the 45 quiet seconds would be.
    for _, stub in ipairs({
        { CivvisDecides = true, Hotseat = true, HotseatSeats = { 1 }, HotseatHumanSeats = { 0 },
          HotseatStubSeats = { 0 }, RunTag = "table" },
        { CivvisDecides = true, Hotseat = true, HotseatSeats = { 1 }, HotseatHumanSeats = {},
          HotseatStubSeats = { 0 }, RunTag = "table" },
    }) do
        h = seatHost(stub)
        h.second(true)
        for _ = 1, 29 do h.second(false) end
        assert(#h.requests == 0, "29 s of a stub's turn are nothing")
        h.second(false)
        local stuck = h.kinds("seat_stuck")[1]
        assert(#h.requests == 1 and stuck and stuck:find('"player":0', 1, true)
            and stuck:find('"why":"stub_turn"', 1, true) and stuck:find('"age":30', 1, true),
            "a stub's turn standing 30 s is ended: " .. tostring(stuck))
        assert(h.kinds("seat_stuck_end_turn")[1], "and the request is reported")
    end
    -- A HUD hidden for a minute (a leader screen stops both clocks) is not a quiet minute.
    h = seatHost(HOTSEAT)
    h.second(true)
    for _ = 1, 30 do h.second(false) end
    h.clock = h.clock + 600
    h.second(false)
    assert(#h.requests == 0, "a gap the watch slept through is not counted")
    for _ = 1, 11 do h.second(false) end
    assert(#h.requests == 0, "33 watched quiet seconds are not 45")
    for _ = 1, 3 do h.second(false) end
    assert(#h.requests == 1, "45 watched quiet seconds are")
end
do
    -- Not a hotseat game: no watch, and the shipped clock tick is untouched.
    for _, cfg in ipairs({ { CivvisDecides = true }, { CivvisDecides = true, Hotseat = false },
                           { CivvisDecides = true, Hotseat = true, Play = false },
                           { Hotseat = true } }) do
        local h = { stockTicks = 0, logs = {} }
        local env = setmetatable({ CivvisControlConfig = cfg }, { __index = _G })
        local stock = function() h.stockTicks = h.stockTicks + 1 end
        env.include = function(name)
            if name ~= "TopPanel" then error("unavailable expansion") end
            env.LateInitialize = function() end
            env.OnRefreshTimeTick = stock
        end
        env.ContextPtr = { SetUpdate = function(_, callback) h.update = callback end }
        env.LuaEvents = { CivvisControlPulse = function() end }
        env.Automation = { Log = function(line) h.logs[#h.logs + 1] = line end }
        local chunk = assert(loadfile(here .. "/CivvisControlHeartbeat.lua"))
        setfenv(chunk, env)
        chunk()
        assert(env.OnRefreshTimeTick == stock, "outside hotseat the shipped clock tick is not wrapped")
        assert(#h.logs == 0, "outside hotseat the watch is not armed")
    end
end
-- A turn that stands with its controller still beating. The test stub's
-- stand-in, and a CIVVIS seat past its forfeit ladder, keep ticking on the
-- HUD's pulse, so the quiet rule never fires (smoke-6p-20261128T0244Zb: the
-- gated stub's turn 2 stood ten minutes and the watch wrote nothing).
local GATED = { CivvisDecides = true, Hotseat = true, HotseatSeats = { 1, 2, 3, 4, 5 },
                HotseatHumanSeats = { 0 }, HotseatStubSeats = { 0 }, RunTag = "table" }
local function count(h, kind) return #h.kinds(kind) end
do
    -- smoke-6p t2 as it was: the stub beat once (its one pass) and went quiet,
    -- with no blocker standing. Ended at 30 s, not 45.
    local h = seatHost(GATED)
    h.turn = 2
    h.second(true)
    for _ = 1, 30 do h.second(false) end
    assert(#h.requests == 1 and h.requests[1].reason == "UserForced", "the stub's turn 2 is ended at 30 s")
    local stuck = h.kinds("seat_stuck")[1]
    for _, piece in ipairs({ '"turn":2', '"player":0', '"why":"stub_turn"', '"blocker":null',
            '"sent":false', '"processing":false', '"controller_ran":true', '"quiet":30', '"age":30' }) do
        assert(stuck:find(piece, 1, true), "seat_stuck lacks " .. piece .. ": " .. stuck)
    end
    assert(count(h, "seat_quiet") == 0, "named for what it is, not as a quiet seat")
    -- The stand-in beating every second: still ended at 30 s, and the record
    -- names the unit the engine waits on and the event locks a screen holds.
    h = seatHost(GATED)
    h.turn, h.blocker, h.ready = 5, 7, "UNIT_SCOUT"
    h.env.ExposedMembers.CivvisEventLocks = { count = 1, held = {} }
    for _ = 1, 30 do h.second(true) end
    assert(#h.requests == 0, "29 beating seconds of a stub's turn are nothing")
    h.second(true)
    assert(#h.requests == 1, "a beating stub standing 30 s is ended")
    stuck = h.kinds("seat_stuck")[1]
    for _, piece in ipairs({ '"blocker":"ENDTURN_BLOCKING_UNITS"', '"ready":"UNIT_SCOUT"',
            '"locks":1', '"quiet":0', '"age":30' }) do
        assert(stuck:find(piece, 1, true), "seat_stuck lacks " .. piece .. ": " .. stuck)
    end
    -- Same budget as a quiet seat: every 20 s, six a turn, reported once a minute.
    for _ = 1, 19 do h.second(true) end
    assert(#h.requests == 1, "not asked again inside 20 s")
    h.second(true)
    assert(#h.requests == 2, "asked again after 20 s")
    for _ = 1, 400 do h.second(true) end
    assert(#h.requests == 6, "six forced requests a turn at most: " .. #h.requests)
    assert(count(h, "seat_stuck") == 8, "reported once a minute: " .. count(h, "seat_stuck"))
    assert(count(h, "seat_stuck_end_turn") == 6)
    -- A new turn gets its own 30 s.
    h.turn = 6
    for _ = 1, 30 do h.second(true) end
    assert(#h.requests == 6, "turn 6 starts its own clock")
    h.second(true)
    assert(#h.requests == 7)
    -- A stub turn that reads as sent and still stands was dropped: the engine
    -- ignores a turn complete sent while the game is paused
    -- (live-6p-20261128T0405Z t3, `PlayerTurnComplete ignored due to game
    -- paused.`) and the UI keeps it as sent. It is asked again at 30 s:
    -- unreadied where the game allows, else sent with SHIFT+ENTER's request.
    h = seatHost(GATED)
    h.sent = true
    for _ = 1, 30 do h.second(true) end
    assert(#h.requests == 0, "29 s of a sent stub turn are nothing")
    h.second(true)
    assert(#h.requests == 1 and h.requests[1].action == "ACTION_ENDTURN"
        and h.requests[1].reason == "UserForced", "a dropped turn complete is sent again")
    assert(h.kinds("seat_stuck_end_turn")[1]:find('"how":"end_turn"', 1, true))
    assert(h.kinds("seat_stuck")[1]:find('"sent":true', 1, true)
        and h.kinds("seat_stuck")[1]:find('"game_paused":false', 1, true))
    h = seatHost(GATED)
    h.sent, h.can_unready = true, true
    for _ = 1, 31 do h.second(true) end
    assert(#h.requests == 1 and h.requests[1].action == "ACTION_UNREADYTURN",
        "where the game allows it, the sent turn is unreadied first")
    assert(h.kinds("seat_stuck_end_turn")[1]:find('"how":"unready"', 1, true))
    h.sent = false                         -- the unready landed
    for _ = 1, 20 do h.second(true) end
    assert(#h.requests == 2 and h.requests[2].action == "ACTION_ENDTURN", "and the next request ends it")
    -- The game's own pause is named in every report.
    h = seatHost(GATED)
    h.game_paused = true
    for _ = 1, 31 do h.second(true) end
    assert(h.kinds("seat_stuck")[1]:find('"game_paused":true', 1, true), "the game's pause is reported")
    -- Its hand-over standing paused, with its dialog on screen, is not its turn yet.
    h = seatHost(GATED)
    h.paused, h.handover_hidden, h.dialog_hidden = true, false, false
    for _ = 1, 100 do h.second(false) end
    h.paused = false
    for _ = 1, 29 do h.second(true) end
    assert(#h.requests == 0, "the hand-over's pause is not counted")
    h.second(true)
    assert(#h.requests == 1)
    -- A gap the watch slept through counts 3 s, not 600.
    h = seatHost(GATED)
    h.second(true)
    for _ = 1, 10 do h.second(true) end
    h.clock = h.clock + 600
    for _ = 1, 16 do h.second(true) end
    assert(#h.requests == 0, "a gap the watch slept through is not a stuck turn")
    for _ = 1, 2 do h.second(true) end
    assert(#h.requests == 1, "30 watched seconds are")
end
do
    -- A CIVVIS seat's turn: a beating controller is left alone for 179 s and
    -- ended at 180 (`SeatLongTurnSeconds`).
    local h = seatHost(GATED)
    h.local_player = 3
    for _ = 1, 180 do h.second(true) end
    assert(#h.requests == 0 and count(h, "seat_stuck") == 0, "179 s of a CIVVIS turn are nothing")
    h.second(true)
    local stuck = h.kinds("seat_stuck")[1]
    assert(#h.requests == 1 and stuck and stuck:find('"why":"long_turn"', 1, true)
        and stuck:find('"player":3', 1, true) and stuck:find('"age":180', 1, true),
        "a CIVVIS turn standing 180 s is ended: " .. tostring(stuck))
    -- A CIVVIS turn that reads as sent: quiet, the engine's to finish; past
    -- its 180 s it was dropped, and is sent again.
    h = seatHost(GATED)
    h.local_player, h.sent = 4, true
    h.second(true)
    for _ = 1, 179 do h.second(false) end
    assert(#h.requests == 0 and count(h, "seat_quiet") >= 1, "a sent quiet turn is reported, not ended")
    h.second(false)
    assert(#h.requests == 1 and h.kinds("seat_quiet_end_turn")[1]:find('"why":"quiet"', 1, true),
        "a sent turn standing 180 s is sent again")
    -- Configurable, and the stub's 30 s is the stub's alone.
    h = seatHost({ CivvisDecides = true, Hotseat = true, HotseatSeats = { 1, 2 },
                   HotseatHumanSeats = { 0 }, HotseatStubSeats = { 0 }, SeatLongTurnSeconds = 60,
                   RunTag = "table" })
    h.local_player = 2
    for _ = 1, 60 do h.second(true) end
    assert(#h.requests == 0, "a CIVVIS seat is not a stub")
    h.second(true)
    assert(#h.requests == 1, "SeatLongTurnSeconds is read")
    -- The quiet rule still comes first and keeps its name.
    h = seatHost(GATED)
    h.local_player = 1
    h.second(true)
    for _ = 1, 45 do h.second(false) end
    assert(#h.requests == 1 and count(h, "seat_quiet") == 1 and count(h, "seat_stuck") == 0)
    assert(h.kinds("seat_quiet")[1]:find('"why":"quiet"', 1, true))
    -- A person's own turn, beating or not, is never ended or reported.
    h = seatHost({ CivvisDecides = true, Hotseat = true, HotseatSeats = { 1, 2 },
                   HotseatHumanSeats = { 0 }, RunTag = "table" })
    for _ = 1, 600 do h.second(true) end
    h.active = false
    for _ = 1, 600 do h.second(false) end
    assert(#h.requests == 0 and #h.logs == 1, "the person's seat: only the armed record")
end
do
    -- The local player a watched seat whose turn is not active: between its
    -- end of turn and the next seat's start. Reported after 60 s, once a
    -- minute, never acted on.
    local h = seatHost(GATED)
    h.second(true)
    h.active, h.sent, h.processing = false, true, true
    h.env.ExposedMembers.CivvisEventLocks = { count = 2, held = {} }
    for _ = 1, 60 do h.second(false) end
    assert(count(h, "seat_between") == 0, "59 s between turns are nothing")
    h.second(false)
    local between = h.kinds("seat_between")[1]
    for _, piece in ipairs({ '"active":false', '"sent":true', '"processing":true', '"locks":2',
            '"between":60', '"player":0' }) do
        assert(between and between:find(piece, 1, true), "seat_between lacks " .. piece .. ": " .. tostring(between))
    end
    for _ = 1, 59 do h.second(false) end
    assert(count(h, "seat_between") == 1, "once a minute")
    h.second(false)
    assert(count(h, "seat_between") == 2)
    assert(#h.requests == 0 and h.pause_set == nil, "nothing is asked of a turn that is not active")
    -- The next seat's turn (or the person's) clears it.
    h.local_player = 0
    h.active = true
    h.second(true)
    h.active = false
    for _ = 1, 60 do h.second(false) end
    assert(count(h, "seat_between") == 2, "a new wait starts its own 60 s")
    h.second(false)
    assert(count(h, "seat_between") == 3)
    -- Another seat's turn in between (a Firaxis AI's, unwatched) clears it too.
    h.local_player = 7
    h.second(false)
    h.local_player = 0
    for _ = 1, 59 do h.second(false) end
    assert(count(h, "seat_between") == 3, "an unwatched seat's turn clears the wait")
    -- Not for the end screen or an operator's hold.
    for _, change in ipairs({
        function(x) x.end_game_hidden = false end,
        function(x) x.env.ExposedMembers.CivvisOperatorHold = true end,
    }) do
        local q = seatHost(GATED)
        q.active = false
        change(q)
        for _ = 1, 200 do q.second(false) end
        assert(count(q, "seat_between") == 0, "a finished or held game is not between turns")
    end
end
print("all seat watch checks passed")

-- The game's ending, written by the table (`CivvisEnding`). Each seat's agent
-- writes `victory`/`defeat` only while its own seat is local (CivvisSeat.gate),
-- so an ending on the person's own turn reached nobody's log. The HUD writes
-- each ending once under the table's tag, whoever is local.
local PERSON_TABLE = { CivvisDecides = true, Hotseat = true, HotseatSeats = { 1, 2, 3, 4, 5 },
                       HotseatHumanSeats = { 0 }, RunTag = "hs" }
local function endingHost(cfg, before)
    local h = seatHost(cfg, before)
    h.alive, h.cities = {}, {}
    for seat = 0, 5 do h.alive[seat], h.cities[seat] = true, 2 end
    h.env.Players = setmetatable({}, { __index = function(_, id) return {
        IsTurnActive = function() return h.active end,
        IsAlive = function()
            if h.alive[id] == nil then error("no such player") end
            return h.alive[id]
        end,
        GetCities = function() return { GetCount = function() return h.cities[id] end } end,
        GetUnits = function() return { GetFirstReadyUnit = function() return nil end } end,
    } end })
    h.env.Teams = { [0] = { 0 }, [1] = { 1 }, [2] = { 2 }, [3] = { 3 }, [4] = { 4 }, [5] = { 5 } }
    h.env.GameInfo.Victories = { [0] = { VictoryType = "VICTORY_SCORE" }, [3] = { VictoryType = "VICTORY_CULTURE" },
                                 [5] = { VictoryType = "VICTORY_TECHNOLOGY" } }
    h.env.GameInfo.Defeats = { [0] = { DefeatType = "DEFEAT_DEFAULT" }, [1] = { DefeatType = "DEFEAT_TIME" } }
    h.has = function(line, pieces)
        for _, piece in ipairs(pieces) do
            assert(line and line:find(piece, 1, true), "the record lacks " .. piece .. ": " .. tostring(line))
        end
    end
    return h
end
do
    -- A culture win on the person's own turn: local player 0, no CIVVIS
    -- context open. Written once, with the winning team's players.
    local h = endingHost(PERSON_TABLE)
    h.local_player, h.turn = 0, 201
    h.second(false)
    h.victory(0, 3, 77)
    local won = h.kinds("victory")
    assert(#won == 1, "the person's victory is written by the table")
    h.has(won[1], { '"ctx":"heartbeat"', '"kind":"victory"', '"run":"hs"', '"t":1001', '"turn":201',
        '"team":0', '"victory":3', '"victory_type":"VICTORY_CULTURE"', '"members":[0]', '"source":"table"',
        '"local_player":0' })
    assert(h.logs[#h.logs]:find('"kind":"ending_flush"', 1, true), "and delivered by the record behind it")
    -- However often it is raised, and whoever is local then: once.
    h.local_player = 3
    h.victory(0, 3, 78)
    h.victory(2, 5, 79)
    assert(#h.kinds("victory") == 1, "one ending per game")
    -- A raise with no team is not a victory.
    local q = endingHost(PERSON_TABLE)
    q.victory(nil, 3)
    assert(#q.kinds("victory") == 0, "no team, no victory")
    q.victory(4, 5)
    q.has(q.kinds("victory")[1], { '"team":4', '"members":[4]', '"victory_type":"VICTORY_TECHNOLOGY"' })
    -- A team the UI cannot list keeps its number; an unnamed type is null.
    q = endingHost(PERSON_TABLE)
    q.env.Teams = {}
    q.victory(2, 9)
    q.has(q.kinds("victory")[1], { '"team":2', '"members":[]', '"victory":9', '"victory_type":null' })
end
do
    -- The person eliminated: one city went to seat 2, the last to seat 3.
    local h = endingHost(PERSON_TABLE)
    h.local_player, h.turn = 3, 88
    h.second(true)                          -- the city counts' baseline: 2 each
    h.cities[0] = 1; h.cities[2] = 3
    h.fire("CityTransfered", 2, 65536)
    h.cities[0] = 0; h.cities[3] = 3
    h.fire("CityOccupationChanged", 3, 131072)
    h.fire("PlayerDefeat", 0, 0, 12)
    local gone = h.kinds("defeat")
    assert(#gone == 1, "the person's defeat is written")
    h.has(gone[1], { '"ctx":"heartbeat"', '"run":"hs"', '"turn":88', '"player":0', '"defeat":0',
        '"defeat_type":"DEFEAT_DEFAULT"', '"by":3', '"how":"event"', '"source":"table"', '"local_player":3' })
    -- Raised again, or gone on the next readings too: still one.
    h.fire("PlayerDefeat", 0, 0, 13)
    h.alive[0] = false
    for _ = 1, 5 do h.second(true) end
    assert(#h.kinds("defeat") == 1, "one record per seat")
    -- A CIVVIS seat whose last city the person took (a transfer, no occupation event).
    h = endingHost(PERSON_TABLE)
    h.second(true)
    h.cities[1] = 0; h.cities[0] = 3
    h.fire("CityTransfered", 0, 9)
    h.fire("PlayerDefeat", 1, 0)
    h.has(h.kinds("defeat")[1], { '"player":1', '"by":0' })
    -- A city a seat took back from no one, or a count with no baseline, names nobody.
    h = endingHost(PERSON_TABLE)
    h.fire("CityTransfered", 2, 1)          -- no reading yet: nothing to compare
    h.cities[2] = 1
    h.fire("CityTransfered", 2, 2)          -- seat 2's own city event
    h.fire("PlayerDefeat", 2, 0)
    h.has(h.kinds("defeat")[1], { '"player":2', '"by":null' })
    -- The turn limit with the Score victory off: every seat's DEFEAT_TIME.
    h = endingHost(PERSON_TABLE)
    h.turn = 251
    for seat = 0, 5 do h.fire("PlayerDefeat", seat, 1) end
    assert(#h.kinds("defeat") == 6, "each seat's time defeat")
    h.has(h.kinds("defeat")[1], { '"player":0', '"defeat_type":"DEFEAT_TIME"', '"turn":251' })
end
do
    -- Gone with no PlayerDefeat (civvis-20260730T170738Z): a seat seen alive,
    -- then not alive on two readings in a row.
    local h = endingHost(PERSON_TABLE)
    h.second(true)
    h.alive[4] = false
    h.second(true)
    assert(#h.kinds("defeat") == 0, "one reading is not an elimination")
    h.alive[4] = true
    h.second(true)
    h.alive[4] = false
    h.second(true)
    assert(#h.kinds("defeat") == 0, "a reading between resets it")
    h.second(true)
    local gone = h.kinds("defeat")
    assert(#gone == 1, "two readings in a row are")
    h.has(gone[1], { '"player":4', '"defeat":null', '"defeat_type":null', '"by":null', '"how":"not_alive"' })
    h.fire("PlayerDefeat", 4, 0)
    assert(#h.kinds("defeat") == 1, "and the event after it adds nothing")
    -- A seat never seen alive (closed, or not yet in the game) is not defeated.
    h = endingHost(PERSON_TABLE)
    h.alive[5] = false
    for _ = 1, 10 do h.second(true) end
    assert(#h.kinds("defeat") == 0, "never alive, never defeated")
    -- A player the UI cannot read is left alone.
    h.alive[5] = nil
    for _ = 1, 10 do h.second(true) end
    assert(#h.kinds("defeat") == 0)
end
do
    -- Not a hotseat game (the ladder's single seat): its agent writes its own
    -- ending, and the HUD neither listens nor writes.
    for _, cfg in ipairs({ { CivvisDecides = true, RunTag = "ladder" },
                           { CivvisDecides = true, Hotseat = false, RunTag = "ladder" },
                           { CivvisDecides = true, Hotseat = true, Play = false, RunTag = "ladder" } }) do
        local h = endingHost(cfg)
        h.victory(0, 3)
        h.fire("PlayerDefeat", 0, 0)
        h.alive[0] = false
        if h.update ~= nil then for _ = 1, 5 do h.second(true) end end
        assert(#h.kinds("victory") == 0 and #h.kinds("defeat") == 0, "no ending of ours outside hotseat")
        assert(h.listeners.PlayerDefeat == nil and h.listeners.CityTransfered == nil, "and no listener")
    end
    -- The person-facing install with its harness gone writes nothing.
    local beat = false
    local function harnessed(env)
        env.GameConfiguration.IsHotseat = function() return true end
        env.DB = { Query = function(sql)
            if sql:find("civvis.harness", 1, true) and beat then return { { age = 1 } } end
            return {}
        end }
    end
    local REQUIRE = { CivvisDecides = true, Hotseat = true, HotseatSeats = { 1 }, HotseatHumanSeats = { 0 },
                      RunTag = "hs", RequireHarness = true, HarnessRun = "hs", OrdersDb = "/r/o.sqlite" }
    local h = endingHost(REQUIRE, harnessed)
    h.victory(0, 3)
    h.fire("PlayerDefeat", 1, 0)
    assert(#h.kinds("victory") == 0 and #h.kinds("defeat") == 0, "no harness, no line")
    beat = true
    h = endingHost(REQUIRE, harnessed)
    h.victory(0, 3)
    assert(#h.kinds("victory") == 1, "and with it live, the ending is written")
    -- An unattended table (every seat CIVVIS's) is written the same way.
    h = endingHost({ CivvisDecides = true, Hotseat = true, HotseatSeats = { 0, 1 }, HotseatHumanSeats = {},
                     RunTag = "sp" })
    h.local_player = 0
    h.victory(1, 3)
    h.has(h.kinds("victory")[1], { '"run":"sp"', '"team":1', '"members":[1]', '"local_player":0' })
end
print("all table ending checks passed")
