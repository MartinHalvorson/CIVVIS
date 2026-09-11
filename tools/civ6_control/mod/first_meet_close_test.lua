-- Exercise the production first-contact branch with asynchronous native closure.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local f = assert(io.open(here .. "/CivvisControlAutoClose.lua"))
local source = f:read("*a"); f:close()
local closer = assert(source:match('(local function endScreen%(attempt%).-)\nend\n')) .. '\nend\n'
local responses, closes, hidden = 0, 0, false
local pendingChoice = "CHOICE_EXIT"
local nextStatement = nil
local failResponse, failClose = false, false
OnSelectConversationDiplomacyStatement = function(key)
    if failResponse then error("response unavailable") end
    responses = responses + 1
    assert(key == pendingChoice)
    if nextStatement then nextStatement() end
end
CloseFocusedState = function()
    if failClose then error("close unavailable") end
    closes = closes + 1
    hidden = true
end
local close, reset = assert(loadstring(
    'local NAME="DiplomacyActionView"; local firstMeetChoice="CHOICE_EXIT"; '
    .. 'local firstMeetAnswered=false; local report=function() end; '
    .. 'local closeStaleDiplomacyContext=function() return false end;\n'
    .. closer .. '\nreturn endScreen, function(choice) firstMeetChoice=choice; firstMeetAnswered=false end'))()
assert(close(1))
assert(responses == 1 and closes == 0 and not hidden)
assert(close(2))
assert(hidden and closes == 1, "answered first contact must reach native closure")
assert(responses == 1, "closing must not send a second response")
-- A synchronous follow-up keeps its own answer pending, without closing it.
reset("CHOICE_POSITIVE"); pendingChoice="CHOICE_POSITIVE"; hidden=false
nextStatement=function() reset("CHOICE_EXIT") end
close(1)
assert(not hidden and responses == 2)
nextStatement=nil; pendingChoice="CHOICE_EXIT"
close(2)
assert(not hidden and responses == 3)
close(3)
assert(hidden and responses == 3)
-- Failed responses remain retryable; failed closes are reported honestly.
reset("CHOICE_EXIT"); hidden=false; failResponse=true
close(1)
assert(responses == 3 and not hidden)
failResponse=false; close(2)
assert(responses == 4 and not hidden)
failClose=true
assert(not close(3), "a throwing native close cannot count as handled")
failClose=false; close(4)
assert(hidden and responses == 4)
print("first-contact close: closure, no duplicate response, follow-up, and retry passed")
