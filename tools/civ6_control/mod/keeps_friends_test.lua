-- Offline regression for gene `score-seat-keeps-friends`.
--
-- Live Deity Rome (Score target), civvis-20261113T041706Z: Gilgamesh
-- (player 5) offered a declared friendship; the NEGATIVE-first ladder
-- answered CHOICE_NEGATIVE and the follow-up read HUMAN_REFUSE_DEAL. The
-- same night refused 52 such offers.
--
-- What is checked:
--   1. no lease: the offer is refused, as live (off = unchanged);
--   2. a lease from the previous turn accepts it with CHOICE_POSITIVE and
--      reports "keeps_friends":"accept";
--   3. the player the batch names as spared keeps the refusal ("spared");
--   4. our own friendship request (we are the initiator) is untouched;
--   5. another statement under the lease keeps the decline ladder;
--   6. the lease lapses two turns after the last batch that renewed it.
--
-- Run: lua5.1 tools/civ6_control/mod/keeps_friends_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function load(withLease)
    local env = { update = nil, keeps = nil, sent = {}, log = {}, turn = 60 }
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
        CivvisDialogueKeepsFriends = withLease and { Add = function(fn) env.keeps = fn end } or nil,
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
    OnSelectConversationDiplomacyStatement = function() end
    assert(loadfile(here .. "/CivvisControlAutoClose.lua"))()
    assert(env.update, "the shim installs its update loop")
    env.statement = function(statementType, options, from, initiator)
        env.selections = options
        DefaultHandlers.ApplyStatement(DefaultHandlers, statementType, "NONE", 0,
            { FromPlayer = from, FromPlayerMood = 0, Initiator = initiator })
    end
    return env
end

-- DiplomacyStatements_DeclareFriendship.xml:82-106 as Civ VI ships it.
local function friendOffer()
    return {
        { Key = "CHOICE_POSITIVE", Text = "LOC_DIPLO_DECLARE_FRIEND_CHOICE_POSITIVE" },
        { Key = "CHOICE_NEGATIVE", Text = "LOC_DIPLO_DECLARE_FRIEND_CHOICE_NEGATIVE" },
        { Key = "CHOICE_STOP_ASKING", Text = "LOC_DIPLO_DECLARE_FRIEND_CHOICE_STOP_ASKING" },
    }
end

local function lastChoiceLine(env)
    for i = #env.log, 1, -1 do
        if string.find(env.log[i], '"kind":"diplomacy_choice"', 1, true) then return env.log[i] end
    end
end

-- 1. Off: the live refusal.
local off = load(false)
off.statement("DECLARE_FRIEND", friendOffer(), 5, 5)
off.update(.01)
assert(#off.sent == 1 and off.sent[1] == "CHOICE_NEGATIVE",
    "off: the ladder still refuses, got " .. tostring(off.sent[1]))
assert(not string.find(lastChoiceLine(off), "keeps_friends", 1, true))

-- 2. On: the previous turn's batch leased the policy; the offer is accepted.
local on = load(true)
assert(on.keeps, "the shim listens for the lease in DiplomacyActionView")
on.keeps(59, nil)
on.statement("DECLARE_FRIEND", friendOffer(), 5, 5)
on.update(.01)
assert(#on.sent == 1 and on.sent[1] == "CHOICE_POSITIVE",
    "on: the offer is accepted, got " .. tostring(on.sent[1]))
local line = lastChoiceLine(on)
assert(string.find(line, '"statement":"DECLARE_FRIEND"', 1, true), line)
assert(string.find(line, '"keeps_friends":"accept"', 1, true), line)

-- 3. The campaign's target keeps the refusal.
local spared = load(true)
spared.keeps(60, 5)
spared.statement("DECLARE_FRIEND", friendOffer(), 5, 5)
spared.update(.01)
assert(#spared.sent == 1 and spared.sent[1] == "CHOICE_NEGATIVE", "the spared rival is refused")
assert(string.find(lastChoiceLine(spared), '"keeps_friends":"spared"', 1, true))
local listed = load(true)
listed.keeps(60, "3,5")
listed.statement("DECLARE_FRIEND", friendOffer(), 3, 3)
listed.update(.01)
assert(#listed.sent == 1 and listed.sent[1] == "CHOICE_NEGATIVE", "every listed rival is refused")
local other = load(true)
other.keeps(60, 5)
other.statement("DECLARE_FRIEND", friendOffer(), 3, 3)
other.update(.01)
assert(#other.sent == 1 and other.sent[1] == "CHOICE_POSITIVE", "another rival is accepted")

-- 4. Our own request is not an offer to us.
local ours = load(true)
ours.keeps(60, nil)
ours.statement("DECLARE_FRIEND", { { Key = "CHOICE_EXIT" } }, 0, 0)
ours.update(.01)
assert(#ours.sent == 1 and ours.sent[1] == "CHOICE_EXIT", "our own session exits as before")
assert(not string.find(lastChoiceLine(ours), "keeps_friends", 1, true))

-- 5. Another statement keeps the decline ladder.
local plain = load(true)
plain.keeps(60, nil)
plain.statement("EMBASSY_FROM_AI", { { Key = "CHOICE_POSITIVE" }, { Key = "CHOICE_NEGATIVE" } }, 5, 5)
plain.update(.01)
assert(#plain.sent == 1 and plain.sent[1] == "CHOICE_NEGATIVE", "other statements are untouched")

-- 6. The lease lapses when the brain stops renewing it.
local lapsed = load(true)
lapsed.keeps(57, nil)
lapsed.statement("DECLARE_FRIEND", friendOffer(), 5, 5)
lapsed.update(.01)
assert(#lapsed.sent == 1 and lapsed.sent[1] == "CHOICE_NEGATIVE", "a stale lease accepts nothing")

print("score-seat-keeps-friends: off refuses as live; the lease accepts a major's offer, spares the campaign's target, leaves our own sessions and other statements, and lapses")
