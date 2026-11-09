-- Run the actual clearer: a synchronous follow-up must not relabel its predecessor.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local update, followup
local selections, logs, sent = {}, {}, {}
Game = { GetLocalPlayer = function() return 0 end }
GetStatementMood = function(_, mood) return mood end
CivvisControlConfig = { DialogueSeconds = 2, Play = false }
ContextPtr = {
    GetID = function() return "DiplomacyActionView" end,
    IsHidden = function() return false end,
    SetUpdate = function(_, fn) update = fn end,
}
Controls = { BlackFadeAnim = { IsStopped = function() return true end } }
LuaEvents = { CivvisDealSession = { Add = function() end } }
Automation = { Log = function(line)
    if line:find('"kind":"diplomacy_choice"', 1, true) then logs[#logs+1] = line end
end }
include = function() end
CloseFocusedState = function() end
ApplyStatement = function() end
DefaultHandlers = {
    ApplyStatement = ApplyStatement,
    ExtractStatement = function() return { Selections = selections } end,
    RemoveInvalidSelections = function() end,
}
StatementHandlers = { DEFAULT = DefaultHandlers }
local function statement(kind, subtype, from, to, initiator)
    selections = {{Key="CHOICE_EXIT"}}
    DefaultHandlers.ApplyStatement(DefaultHandlers, kind, subtype, to,
        {FromPlayer=from, FromPlayerMood=0, Initiator=initiator})
end
DefaultHandlers.OnSelectionButtonClicked = function(key)
    sent[#sent+1] = key
    if followup then
        followup = false
        statement("RESIDENT_EMBASSY", "AI_ACCEPT_DEAL", 2, 0, 2)
    end
end
assert(loadfile(here .. "/CivvisControlAutoClose.lua"))()
assert(update)

-- Both shipped delegation responses have only EXIT. Its subtype identifies the result.
statement("DIPLOMATIC_DELEGATION", "AI_REFUSE_DEAL", 5, 0, 0)
followup = true
update(.01)
assert(#sent == 1 and sent[1] == "CHOICE_EXIT" and #logs == 1)
for _, field in ipairs({
    '"statement":"DIPLOMATIC_DELEGATION"', '"subtype":"AI_REFUSE_DEAL"',
    '"from_player":5', '"to_player":0', '"initiator":0', '"sent":true',
}) do
    assert(logs[1]:find(field, 1, true), "original response missing " .. field)
end
assert(not logs[1]:find("RESIDENT_EMBASSY", 1, true), "follow-up relabelled original response")
update(.01)
assert(#sent == 2 and sent[2] == "CHOICE_EXIT" and #logs == 2)
for _, field in ipairs({
    '"statement":"RESIDENT_EMBASSY"', '"subtype":"AI_ACCEPT_DEAL"',
    '"from_player":2', '"to_player":0', '"initiator":2',
}) do
    assert(logs[2]:find(field, 1, true), "follow-up response missing " .. field)
end
statement("DIPLOMATIC_DELEGATION", "AI_ACCEPT_DEAL", 5, 0, 0)
update(.01)
assert(#sent == 3 and sent[3] == "CHOICE_EXIT" and #logs == 3)
assert(logs[3]:find('"subtype":"AI_ACCEPT_DEAL"', 1, true), "acceptance must differ from refusal")

-- Missing optional response metadata stays absent as JSON null, not an invented value.
statement("DIPLOMATIC_DELEGATION", nil, nil, nil, nil)
update(.01)
for _, field in ipairs({'"subtype":null', '"from_player":null', '"to_player":null', '"initiator":null'}) do
    assert(logs[4]:find(field, 1, true), "missing metadata must be null: " .. field)
end
print("diplomacy response context: ok")
