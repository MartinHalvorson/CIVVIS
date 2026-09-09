-- Executed by the existing *_test.lua CI discovery gate.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAutoClose.lua"))
local source = file:read("*a"); file:close()
local hook = assert(source:match('(local firstMeetChoice = nil;.-)\n%-%- A stock diplomacy context'))
local closer = assert(source:match('(local function endScreen%(attempt%).-)\nend\n')) .. '\nend\n'
local selections, sent, stockCalls = {}, {}, 0
Game = { GetLocalPlayer = function() return 0 end }
GetStatementMood = function(_, mood) return mood end
ApplyStatement = function() stockCalls = stockCalls + 1 end
DefaultHandlers = {
    ApplyStatement = ApplyStatement,
    ExtractStatement = function() return { Selections = selections } end,
    RemoveInvalidSelections = function(_, player, other)
        assert(player == 0 and other == 1)
    end,
}
StatementHandlers = { DEFAULT = DefaultHandlers }
OnSelectConversationDiplomacyStatement = function(key) table.insert(sent, key) end
local endScreen, pending = assert(loadstring(
    'local NAME = "DiplomacyActionView"; local report = function() end;\n'
    .. hook .. '\n' .. closer .. '\nreturn endScreen, function() return firstMeetChoice end'))()

local function statement(kind, text, disabled)
    selections = {
        {Key = "CHOICE_POSITIVE", Text = text, IsDisabled = disabled},
        {Key = "CHOICE_EXIT", Text = "LOC_DIPLO_CHOICE_EXIT"},
    }
    DefaultHandlers.ApplyStatement(DefaultHandlers, kind, "NONE", 0,
        {FromPlayer = 1, FromPlayerMood = 0, Initiator = 1})
end
local function check(kind, text, expected, disabled)
    local before = #sent
    statement(kind, text, disabled)
    assert(endScreen(1))
    assert(sent[#sent] == expected, kind .. " chose " .. tostring(sent[#sent]))
    endScreen(2)
    assert(#sent == before + 1, "same statement must not be answered twice")
end
check("FIRST_MEET_VISIT_RECIPIENT", "LOC_DIPLO_CHOICE_FIRST_MEET_VISIT", "CHOICE_POSITIVE")
check("FIRST_MEET_NEAR_RECIPIENT", "LOC_DIPLO_CHOICE_FIRST_MEET_NEAR_RECIPIENT_POSITIVE", "CHOICE_POSITIVE")
check("FIRST_MEET_NEAR_INITIATOR", "LOC_DIPLO_CHOICE_FIRST_MEET_NEAR_INITIATOR_POSITIVE", "CHOICE_EXIT")
check("FIRST_MEET_NO_MANS_INFO_EXCHANGE", "LOC_DIPLO_CHOICE_FIRST_MEET_EXCHANGE_INFO", "CHOICE_EXIT")
check("FIRST_MEET_NO_MANS", "LOC_DIPLO_CHOICE_NO_MANS_POSITIVE", "CHOICE_EXIT")
check("FIRST_MEET_VISIT_RECIPIENT", "LOC_DIPLO_CHOICE_FIRST_MEET_VISIT", "CHOICE_EXIT", true)
check("FIRST_MEET_UNKNOWN", "UNKNOWN", "CHOICE_EXIT")
statement("EMBASSY", "LOC_DIPLO_CHOICE_FIRST_MEET_VISIT")
assert(pending() == nil, "another conversation must clear the first-meet decision")
-- Responses can synchronously apply the next statement; do not mark it answered.
statement("FIRST_MEET_NEAR_RECIPIENT", "LOC_DIPLO_CHOICE_FIRST_MEET_NEAR_RECIPIENT_POSITIVE")
OnSelectConversationDiplomacyStatement = function(key)
    table.insert(sent, key)
    statement("FIRST_MEET_VISIT_RECIPIENT", "LOC_DIPLO_CHOICE_FIRST_MEET_VISIT")
end
endScreen(1)
local before = #sent
OnSelectConversationDiplomacyStatement = function(key) table.insert(sent, key) end
endScreen(2)
assert(#sent == before + 1 and sent[#sent] == "CHOICE_POSITIVE")
assert(stockCalls == 10, "stock rendering must still run for every statement")
print("first-contact privacy: 10 statement cases passed")

-- Exercise the real update loop: decisions bypass the reading/retry timer,
-- but cannot run through the native fade or a controller-owned deal hold.
local update, sessionListener
local fading = false
local exits = 0
sent = {}
CivvisControlConfig = { DialogueSeconds = 2, Play = false }
ContextPtr = {
    GetID = function() return "DiplomacyActionView" end,
    IsHidden = function() return false end,
    SetUpdate = function(_, callback) update = callback end,
}
Controls = { BlackFadeAnim = { IsStopped = function() return not fading end } }
LuaEvents = { CivvisDealSession = { Add = function(callback) sessionListener = callback end } }
Automation = { Log = function() end }
include = function() end
CloseFocusedState = function() exits = exits + 1 end
ApplyStatement = function() end
DefaultHandlers.ApplyStatement = ApplyStatement
assert(loadfile(here .. "/CivvisControlAutoClose.lua"))()
assert(update and sessionListener)
statement("FIRST_MEET_NEAR_RECIPIENT", "LOC_DIPLO_CHOICE_FIRST_MEET_NEAR_RECIPIENT_POSITIVE")
fading = true
update(0.01)
assert(#sent == 0, "never answer during the native fade")
fading = false
update(0.01)
assert(#sent == 1 and sent[1] == "CHOICE_POSITIVE", "answer on the first ready frame")
update(0.01)
assert(#sent == 1, "do not resubmit while waiting for the next statement")
statement("FIRST_MEET_VISIT_RECIPIENT", "LOC_DIPLO_CHOICE_FIRST_MEET_VISIT")
update(0.01)
assert(#sent == 2 and sent[2] == "CHOICE_POSITIVE", "follow-up hospitality bypasses the old timer")
statement("FIRST_MEET_NEAR_INITIATOR", "LOC_DIPLO_CHOICE_FIRST_MEET_NEAR_INITIATOR_POSITIVE")
sessionListener(1, true, 1)
update(0.01)
assert(#sent == 2, "owned deal holds still take priority")
sessionListener(1, false, 0)
update(0.01)
assert(#sent == 3 and sent[3] == "CHOICE_EXIT", "privacy exit is equally prompt")
statement("EMBASSY", "UNKNOWN")
update(0.01)
assert(exits == 0 and #sent == 3, "other dialogues retain their normal timer")
-- A new statement must not inherit a stuck screen's thirty-second back-off.
for _ = 1, 20 do update(2) end
local beforeBackoff = #sent
statement("FIRST_MEET_VISIT_RECIPIENT", "LOC_DIPLO_CHOICE_FIRST_MEET_VISIT")
update(0.01)
assert(#sent == beforeBackoff + 1 and sent[#sent] == "CHOICE_POSITIVE",
    "new hospitality bypasses a previous screen's retry back-off")
print("first-contact timing: fade, follow-up, duplicate, hold, privacy, ordinary-dialogue and back-off checks passed")
