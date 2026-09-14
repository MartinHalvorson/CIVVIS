-- Exercise the shipped autoclose loop with filtered native leader options.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local update, sessionListener
local selections, sent = {}, {}
local fading, fail, followup, filterNegative = false, false, false, false
local closes, blind = 0, 0
Game = { GetLocalPlayer = function() return 0 end }
GetStatementMood = function(_, mood) return mood end
CivvisControlConfig = { DialogueSeconds = 2, Play = false }
ContextPtr = {
    GetID = function() return "DiplomacyActionView" end,
    IsHidden = function() return false end,
    SetUpdate = function(_, fn) update = fn end,
}
Controls = { BlackFadeAnim = { IsStopped = function() return not fading end } }
LuaEvents = { CivvisDealSession = { Add = function(fn) sessionListener = fn end } }
Automation = { Log = function() end }
include = function() end
CloseFocusedState = function() closes = closes + 1 end
ApplyStatement = function() end
DefaultHandlers = {
    ApplyStatement = ApplyStatement,
    ExtractStatement = function() return { Selections = selections } end,
    RemoveInvalidSelections = function(parsed)
        for _, option in ipairs(parsed.Selections) do
            if filterNegative and option.Key == "CHOICE_NEGATIVE" then option.IsDisabled = true end
        end
    end,
}
StatementHandlers = { DEFAULT = DefaultHandlers }
local function statement(options)
    selections = options
    DefaultHandlers.ApplyStatement(DefaultHandlers, "EMBASSY", "NONE", 0,
        {FromPlayer=1, FromPlayerMood=0, Initiator=1})
end
DefaultHandlers.OnSelectionButtonClicked = function(key)
    if fail then error("native callback unavailable") end
    sent[#sent+1] = key
    if followup then
        followup = false
        statement({{Key="CHOICE_EXIT"}})
    end
end
OnSelectConversationDiplomacyStatement = function() blind=blind+1; error("blind response must never run") end
assert(loadfile(here .. "/CivvisControlAutoClose.lua"))()
assert(update and sessionListener)
statement({{Key="CHOICE_EXIT"},{Key="CHOICE_NEGATIVE"}})
fading=true; update(.01); assert(#sent==0)
fading=false; update(.01)
assert(#sent==1 and sent[1]=="CHOICE_NEGATIVE", "answer available request on first ready frame")
for _=1,20 do update(2) end
assert(#sent==1, "native close retries must not repeat the response")
assert(closes>0, "answered dialogue must still reach native close logic")
-- Native filtering wins over the raw option list, and a new request bypasses backoff.
filterNegative=true
statement({{Key="CHOICE_NEGATIVE"},{Key="CHOICE_IGNORE"},{Key="CHOICE_EXIT"}})
update(.01)
assert(#sent==2 and sent[2]=="CHOICE_IGNORE")
filterNegative=false
-- A controller-owned deal cannot be dismissed by the UI helper.
statement({{Key="CHOICE_NEGATIVE"}})
sessionListener(1,true,1); update(.01); assert(#sent==2)
sessionListener(1,false,0); update(.01); assert(#sent==3)
-- Failed dispatch retries; a synchronous follow-up keeps its own answer.
statement({{Key="CHOICE_NEGATIVE"}})
fail=true; update(.01); assert(#sent==3)
fail=false; followup=true; update(.01); assert(#sent==4)
update(.01); assert(#sent==5 and sent[5]=="CHOICE_EXIT")
-- Unknown/disabled choices do not authorize invented answers.
statement({{Key="CHOICE_POSITIVE"},{Key="CHOICE_NEGATIVE",IsDisabled=true}})
for _=1,20 do update(2) end
assert(#sent==5 and blind==0, "unsupported fallback answers must not be attempted")
print("leader responses: ready-frame dispatch, native filtering, no duplicates, hold, retry and follow-up passed")
