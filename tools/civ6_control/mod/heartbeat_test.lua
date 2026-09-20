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
    env.LuaEvents = { CivvisControlPulse = function()
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
    env.LuaEvents = { CivvisControlPulse = function()
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
