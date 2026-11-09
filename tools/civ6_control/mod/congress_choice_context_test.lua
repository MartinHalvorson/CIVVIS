-- Pre-request context from the production ballot. No native game is started.
-- transport_only is for an explicit old/new operation-stream comparison;
-- ordinary test discovery always runs all assertions.
do
    local setupvalue = debug.setupvalue
    debug.setupvalue = function(...)
        collectgarbage("collect")
        return setupvalue(...)
    end
end
local here = arg[0]:match("(.*)/[^/]*$") or "."
local transportOnly = arg[1] == "transport_only"
local function stub()
    return setmetatable({}, { __index = function() return stub() end,
        __call = function() return stub() end, __newindex = function() end })
end
setmetatable(_G, { __index = function() return stub() end })
assert(pcall(assert(loadfile(here .. "/CivvisControlAgent.lua"))))
local function upvalue(fn, key, replacement)
    for i = 1, 100 do
        local name, value = debug.getupvalue(fn, i)
        if name == nil then break end
        if name == key then
            if replacement ~= nil then debug.setupvalue(fn, i, replacement) end
            return value
        end
    end
    error("missing upvalue " .. key)
end
local checks = 0
local function check(name, condition)
    assert(condition, name)
    checks = checks + 1
    print("ok " .. name)
end
if not transportOnly then
    local capture = assert(rawget(_G, "CivvisCongressChoiceContext"))
    local row = { Type = "WC_RES_LUXURY", TargetType = "ResourceType",
        PossibleTargets = { "LOC_RESOURCE_COCOA_NAME", "LOC_RESOURCE_MARBLE_NAME" },
        FavoredPlayerIDs = { 2, 3 }, DisfavoredPlayerIDs = { 1 } }
    local rivals = { { id = 2, points = 13, score = 1054, culture = 20, progress = 65 } }
    local request = { turn = 171, option = 2, selection = 1, votes = 4,
        favor_at_ballot = 238, favor_after_allocations = 18 }
    local c = capture("WC_RES_LUXURY", row, { VictoryLane = "diplomatic" }, rivals, request)
    check("schema and native identity", c.schema == 1 and c.turn == 171
        and c.native_resolution_type == row.Type and c.target_type == "ResourceType")
    check("raw target order retains native zero-based selection", c.possible_targets[1].selection == 0
        and c.possible_targets[1].target == "LOC_RESOURCE_COCOA_NAME"
        and c.possible_targets[2].selection == 1)
    check("request target comes from the actual menu", c.requested.target == "LOC_RESOURCE_MARBLE_NAME"
        and c.requested.selection == 1 and c.requested.option == 2 and c.requested.votes == 4)
    check("complete offered menu is explicit", c.targets_complete and c.requested_target_known)
    check("public favored and disfavored groups are captured", c.favored_players_known
        and c.disfavored_players_known and c.favored_players[2].player == 3
        and c.disfavored_players[1].player == 1)
    check("lane gates are the effective production gates", c.lane_known and c.diplomatic_racer
        and c.congress_racer)
    check("observed rival pressure inputs are retained", c.rivals[1].points == 13
        and c.rivals[1].culture == 20 and c.rivals[1].progress == 65)
    row.PossibleTargets[2] = "LOC_RESOURCE_COCOA_NAME"
    row.FavoredPlayerIDs[1] = 5
    rivals[1].points = 19
    request.votes = 1
    check("capture survives later menu mutation", c.possible_targets[2].target == "LOC_RESOURCE_MARBLE_NAME"
        and c.requested.target == "LOC_RESOURCE_MARBLE_NAME")
    check("capture survives preference, rival and request mutation", c.favored_players[1].player == 2
        and c.rivals[1].points == 13 and c.requested.votes == 4)
    local unknown = capture("WC_RES_LUXURY", {}, {}, nil, {})
    check("absent menus and preferences remain unknown", not unknown.targets_complete
        and not unknown.requested_target_known and not unknown.favored_players_known
        and not unknown.disfavored_players_known and not unknown.lane_known)
    local empty = capture("WC_RES_LUXURY", { PossibleTargets = {}, FavoredPlayerIDs = {},
        DisfavoredPlayerIDs = {} }, { VictoryLane = "culture" }, {}, { selection = 0 })
    check("known empty arrays remain distinguishable", empty.targets_complete and empty.favored_players_known
        and empty.disfavored_players_known and not empty.requested_target_known)
    check("recording another lane does not enable the racer", not empty.diplomatic_racer and not empty.congress_racer)
    local malformed = capture("WC_RES_LUXURY", { PossibleTargets = {
        [1] = "LOC_RESOURCE_MARBLE_NAME", [2] = function() end, [math.huge] = "LOC_RESOURCE_COCOA_NAME" } },
        {}, {}, { selection = 0 })
    check("unsupported entries do not claim complete capture", not malformed.targets_complete
        and #malformed.possible_targets == 1 and malformed.requested_target_known)
    check("context makes no registration or prediction claim", c.registered == nil and c.prediction == nil)
end

local tick = upvalue(CivvisQueue.onUiPulse, "tick")
local config = { Play = true, CivvisDecides = false, VictoryLane = arg[2] or "diplomatic",
    ScienceCongressRacer = arg[2] == "science" }
upvalue(tick, "cfg", config)
local hooks = {}
Events = setmetatable({ WorldCongressStage1 = { Add = function(fn) hooks.stage = fn end } },
    { __index = function() return stub() end })
LuaEvents = setmetatable({ CivvisCongressBallot = { Add = function(fn) hooks.popup = fn end } },
    { __index = function() return stub() end })
Automation = { Log = function() end }
local costs = { MaxVotes = 20 }
for k = 0, 20 do costs[k] = 2 * (k + 1) * k end
local rows = {
    { Type = "WC_RES_DIPLOVICTORY", TargetType = "PlayerType", PossibleTargets = { 0, 1, 2, 3 } },
    { Type = "WC_RES_LUXURY", TargetType = "ResourceType",
        PossibleTargets = { "LOC_RESOURCE_COCOA_NAME", "LOC_RESOURCE_MARBLE_NAME" },
        FavoredPlayerIDs = { 1, 3 }, DisfavoredPlayerIDs = { 2 } },
    { Type = "WC_RES_MIGRATION_TREATY", TargetType = "PlayerType", PossibleTargets = { 0, 1, 2, 3 } },
}
rows.Stage = 2147483647
if arg[6] == "unmet" then rows[1].PossibleTargets = { 0, 1, 3 } end
Game = { GetLocalPlayer = function() return 0 end, GetCurrentGameTurn = function() return 171 end,
    GetCurrentTurnSegment = function() return "TURNSEG_WORLDCONGRESS_1" end,
    GetWorldCongress = function() return { GetVotesandFavorCost = function() return costs end,
        GetResolutions = function() return rows end } end }
DB = { MakeHash = function(name) return name end }
Players = {}
for id = 0, 3 do
    local who = id
    Players[id] = { IsTurnActive = function() return false end,
        GetFavor = function() return tonumber(arg[3]) or 238 end,
        GetFavorEnteringCongress = function() return tonumber(arg[3]) or 238 end,
        GetStats = function() return { GetDiplomaticVictoryPoints = function()
            return who == 0 and (tonumber(arg[4]) or 11)
                or (who == 2 and (tonumber(arg[5]) or 13) or 7) end } end,
        GetScore = function() return who == 2 and 1054 or 800 end,
        GetCulture = function() return { GetStaycationers = function() return 100 end,
            GetTouristsTo = function() return 20 end } end }
end
PlayerManager = { GetAliveMajorIDs = function() return { 0, 1, 2, 3 } end }
GameInfo = { Resolutions = {} }
for _, row in ipairs(rows) do GameInfo.Resolutions[row.Type] = { ResolutionType = row.Type, Hash = row.Type } end
PlayerOperations = { PARAM_RESOLUTION_TYPE = "type", PARAM_WORLD_CONGRESS_VOTES = "votes",
    PARAM_RESOLUTION_OPTION = "option", PARAM_RESOLUTION_SELECTION = "selection",
    WORLD_CONGRESS_RESOLUTION_VOTE = "vote", WORLD_CONGRESS_SUBMIT_TURN = "submit" }
local requests, contexts, order, contextFailures = {}, {}, {}, 0
local submissions = 0
local expectContext = not transportOnly
UI = { RequestPlayerOperation = function(pid, operation, params)
    assert(pid == 0)
    if operation == "vote" then
        order[#order + 1] = "request:" .. params.type
        requests[#requests + 1] = params
        if expectContext then
            local c = contexts[#contexts]
            check("context precedes its native request " .. params.type,
                order[#order - 1] == "context:" .. params.type and c.resolution == params.type)
            check("captured parameters equal actual request " .. params.type,
                c.requested.resolution_hash == params.type and c.requested.option == params.option
                and c.requested.selection == params.selection and c.requested.votes == params.votes)
        end
    elseif operation == "submit" then
        submissions = submissions + 1
    else
        error("unexpected operation " .. tostring(operation))
    end
end }
tick()
local cast = upvalue(hooks.popup, "castBallot")
local vote = upvalue(cast, "voteWorldCongress")
if not transportOnly then upvalue(vote, "emit", function(kind, payload)
    if kind == "wc_ballot_context" then
        contexts[#contexts + 1] = payload
        order[#order + 1] = "context:" .. payload.resolution
    elseif kind == "wc_ballot_context_failed" then
        contextFailures = contextFailures + 1
    end
end) end
hooks.popup()
check("production sends one vote per resolution", #requests == 3)
check("production submits the ballot once", submissions == 1)
if not transportOnly then
    check("production captures every requested resolution", #contexts == #requests)
    check("caller supplies its already observed turn", contexts[1].turn == 171)
    check("no historical or world outcome is invented", contexts[2].winner == nil
        and contexts[2].registered == nil)
    check("public menu and preference lists are captured from the live row",
        contexts[2].favored_players[2].player == 3
        and contexts[2].possible_targets[2].target == "LOC_RESOURCE_MARBLE_NAME")
end
for _, req in ipairs(requests) do
    print(string.format("TRANSPORT %s %s %s %s", tostring(req.type), tostring(req.option),
        tostring(req.selection), tostring(req.votes)))
end
print("TRANSPORT_SUBMIT " .. tostring(submissions))
if not transportOnly then
    local original = CivvisCongressChoiceContext
    CivvisCongressChoiceContext = function() error("injected context failure") end
    expectContext = false
    Game.GetCurrentGameTurn = function() return 172 end
    requests = {}
    hooks.popup()
    check("failed diagnostics do not suppress any vote", #requests == 3)
    check("failed diagnostics do not suppress submission", submissions == 2)
    check("each failed context is observable", contextFailures == 3)
    check("failed diagnostics preserve all parameters", requests[1].option == 1
        and requests[1].selection == 0 and requests[1].votes == 11
        and requests[2].option == 1 and requests[2].selection == 0 and requests[2].votes == 1
        and requests[3].option == 2 and requests[3].selection == 2 and requests[3].votes == 1)
    CivvisCongressChoiceContext = original
end
print(string.format("all %d Congress choice context checks passed", checks))
