-- Offline regression for gene `dialogue-never-declares-war`.
--
-- Live King civvis-20261005T045443Z (game 101), t94: Korea sent
-- WARNING_TOO_MANY_TROOPS_NEAR_ME_FROM_AI while our siege stood on Gwangju's
-- ring. The NEGATIVE-first ladder answered CHOICE_NEGATIVE, which Civ VI binds
-- to DIPLOACTION_DECLARE_SURPRISE_WAR (DiplomacyStatements_Warning.xml), and
-- the frame-1 export read Korea at war with 150 grievances against us. The
-- board was holding off and no `war` order was ever sent.
--
-- What is checked:
--   1. no lease: the t94 statement is answered NEGATIVE, as live (off = unchanged);
--   2. a lease from the previous turn answers the same statement IGNORE and
--      reports the statement and `war_withheld`;
--   3. under the lease a NEGATIVE that declares nothing is still the answer;
--   4. the lease lapses two turns after the last batch that renewed it;
--   5. an unread statement's blind rung answers IGNORE under the lease and
--      NEGATIVE without it.
--
-- Run: lua5.1 tools/civ6_control/mod/dialogue_no_war_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function load(withLease)
    local env = { update = nil, noWar = nil, sent = {}, blind = {}, log = {}, turn = 94 }
    Game = {
        GetLocalPlayer = function() return 0 end,
        GetCurrentGameTurn = function() return env.turn end,
    }
    GetStatementMood = function(_, mood) return mood end
    CivvisControlConfig = { DialogueSeconds = 2, Play = false }
    ContextPtr = {
        GetID = function() return "DiplomacyActionView" end,
        IsHidden = function() return false end,
        SetUpdate = function(_, fn) env.update = fn end,
    }
    Controls = { BlackFadeAnim = { IsStopped = function() return true end } }
    LuaEvents = {
        CivvisDealSession = { Add = function() end },
        CivvisDialogueNoWar = withLease and { Add = function(fn) env.noWar = fn end } or nil,
    }
    Automation = { Log = function(line) env.log[#env.log + 1] = line end }
    include = function() end
    CloseFocusedState = function() end
    ApplyStatement = function() end
    ms_ActiveSessionID = nil
    DefaultHandlers = {
        ApplyStatement = ApplyStatement,
        ExtractStatement = function() return { Selections = env.selections } end,
        RemoveInvalidSelections = function() end,
        OnSelectionButtonClicked = function(key) env.sent[#env.sent + 1] = key end,
    }
    StatementHandlers = { DEFAULT = DefaultHandlers }
    OnSelectConversationDiplomacyStatement = function(key) env.blind[#env.blind + 1] = key end
    assert(loadfile(here .. "/CivvisControlAutoClose.lua"))()
    assert(env.update, "the shim installs its update loop")
    env.statement = function(statementType, options)
        env.selections = options
        DefaultHandlers.ApplyStatement(DefaultHandlers, statementType, "NONE", 0,
            { FromPlayer = 2, FromPlayerMood = 0, Initiator = 2 })
    end
    return env
end

-- The t94 statement as Civ VI ships it.
local function troopsWarning()
    return {
        { Key = "CHOICE_POSITIVE" },
        { Key = "CHOICE_NEGATIVE", DiplomaticActionType = "DIPLOACTION_DECLARE_SURPRISE_WAR" },
        { Key = "CHOICE_IGNORE" },
    }
end

local function lastChoiceLine(env)
    for i = #env.log, 1, -1 do
        if string.find(env.log[i], '"kind":"diplomacy_choice"', 1, true) then return env.log[i] end
    end
end

-- 1. Off: the live answer, a surprise war.
local off = load(false)
off.statement("WARNING_TOO_MANY_TROOPS_NEAR_ME_FROM_AI", troopsWarning())
off.update(.01)
assert(#off.sent == 1 and off.sent[1] == "CHOICE_NEGATIVE",
    "off: the ladder still answers NEGATIVE, got " .. tostring(off.sent[1]))
assert(not string.find(lastChoiceLine(off), "war_withheld", 1, true))

-- 2. On: the previous turn's batch leased the policy; the warning is ignored.
local on = load(true)
assert(on.noWar, "the shim listens for the lease in DiplomacyActionView")
on.noWar(93)
on.statement("WARNING_TOO_MANY_TROOPS_NEAR_ME_FROM_AI", troopsWarning())
on.update(.01)
assert(#on.sent == 1 and on.sent[1] == "CHOICE_IGNORE",
    "on: the war-declaring NEGATIVE is withheld, got " .. tostring(on.sent[1]))
local line = lastChoiceLine(on)
assert(string.find(line, '"statement":"WARNING_TOO_MANY_TROOPS_NEAR_ME_FROM_AI"', 1, true), line)
assert(string.find(line, '"war_withheld":true', 1, true), line)

-- 3. On: a NEGATIVE that declares nothing is still the conservative decline.
local plain = load(true)
plain.noWar(94)
plain.statement("EMBASSY_FROM_AI", { { Key = "CHOICE_POSITIVE" }, { Key = "CHOICE_NEGATIVE" }, { Key = "CHOICE_EXIT" } })
plain.update(.01)
assert(#plain.sent == 1 and plain.sent[1] == "CHOICE_NEGATIVE", "a peaceful NEGATIVE is untouched")
assert(not string.find(lastChoiceLine(plain), "war_withheld", 1, true))

-- 4. The lease lapses when the brain stops renewing it.
local lapsed = load(true)
lapsed.noWar(93)
lapsed.turn = 96
lapsed.statement("WARNING_TOO_MANY_TROOPS_NEAR_ME_FROM_AI", troopsWarning())
lapsed.update(.01)
assert(#lapsed.sent == 1 and lapsed.sent[1] == "CHOICE_NEGATIVE", "a stale lease withholds nothing")

-- 5. An unread statement: the blind rung asks IGNORE first under the lease.
local function blindFirst(withLease)
    local env = load(withLease)
    if withLease then env.noWar(94) end
    ms_ActiveSessionID = 7
    for _ = 1, 4 do env.update(2) end
    return env.blind[1]
end
assert(blindFirst(true) == "CHOICE_IGNORE", "on: the blind rung answers IGNORE")
assert(blindFirst(false) == "CHOICE_NEGATIVE", "off: the blind rung is unchanged")

print("dialogue-never-declares-war: off answers NEGATIVE as live; the lease withholds only war choices, lapses, and covers the blind rung")
