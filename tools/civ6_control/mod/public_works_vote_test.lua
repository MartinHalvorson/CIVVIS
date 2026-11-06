-- ⚠ lupa's Lua 5.1.5 (the local runner) barriers the CLOSURE in
-- lua_setupvalue, not the shared UpVal; a full collection first leaves
-- nothing marked (see congress_claim_denial_test.lua). Harmless under the
-- real lua5.1 CI runs.
do
	local setupvalue = debug.setupvalue
	debug.setupvalue = function(...)
		collectgarbage("collect")
		return setupvalue(...)
	end
end

-- Run: lua5.1 tools/civ6_control/mod/public_works_vote_test.lua
-- `CivvisPublicWorksVote`: option B on the leading racer's next project, or
-- option A on our own next project when we lead the race.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local function stub()
    return setmetatable({}, {
        __index = function() return stub() end,
        __call = function() return stub() end,
        __newindex = function() end,
    })
end
setmetatable(_G, { __index = function() return stub() end })
assert(pcall(assert(loadfile(here .. "/CivvisControlAgent.lua"))))

local failures = 0
local function check(name, got, want)
    if got ~= want then
        failures = failures + 1
        print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
    else
        print("ok " .. name)
    end
end

local vote = CivvisPublicWorksVote
local SATELLITE = "LOC_PROJECT_LAUNCH_EARTH_SATELLITE_NAME"
local MOON = "LOC_PROJECT_LAUNCH_MOON_LANDING_NAME"
local MARS = "LOC_PROJECT_LAUNCH_MARS_BASE_NAME"
local EXOPLANET = "LOC_PROJECT_LAUNCH_EXOPLANET_EXPEDITION_NAME"
local ORBITAL = "LOC_PROJECT_ORBITAL_LASER_NAME"
local TERRESTRIAL = "LOC_PROJECT_TERRESTRIAL_LASER_NAME"
local MANHATTAN = "LOC_PROJECT_MANHATTAN_PROJECT_NAME"
local targets = { SATELLITE, MANHATTAN, MOON, MARS, EXOPLANET, ORBITAL, TERRESTRIAL }
-- `costs[k]`: cumulative Favor for k + 1 votes on the Online table.
local costs = { 4, 12, 24, 40, 60, 84, 112, 144, 180, 220, MaxVotes = 12 }
local rivals = { { id = 1 }, { id = 2 } }
local function stages(t)
    return function(id) return t[id] or 0 end
end
local RTYPE = "WC_RES_PUBLIC_WORKS"

-- A rival on the Moon, us nowhere: B on its Mars Base, the next project.
local idx, option, votes, price = vote(RTYPE, targets, rivals, 0, 400, costs, {},
    stages({ [1] = 2 }))
check("racer's next project", targets[idx], MARS)
check("racer gets option B", option, 2)
check("half the bank buys the cap", votes, 9)
check("nine votes price", price, 144)

-- The bank sizes the ballot: half of 150 buys six votes, 60 Favor.
idx, option, votes, price = vote(RTYPE, targets, rivals, 0, 150, costs, {}, stages({ [1] = 2 }))
check("half of 150 buys six votes", votes, 6)
check("six votes price", price, 60)

-- No Favor at all still casts the free vote against the racer.
idx, option, votes, price = vote(RTYPE, targets, rivals, 0, 0, costs, {}, stages({ [1] = 2 }))
check("free vote stands", votes, 1)
check("free vote costs nothing", price, 0)
check("free vote is still B", option, 2)

-- The strongest racer sets the target, whichever seat it holds.
idx = vote(RTYPE, targets, rivals, 0, 400, costs, {}, stages({ [1] = 1, [2] = 3 }))
check("the leading racer is read", targets[idx], EXOPLANET)

-- We lead: A on our own next project, which no rival can build yet.
idx, option = vote(RTYPE, targets, rivals, 0, 400, costs, {}, stages({ [0] = 3, [1] = 1 }))
check("our next project", targets[idx], EXOPLANET)
check("our project gets option A", option, 1)

-- Level with the racer: B, which costs the larger builder more.
idx, option = vote(RTYPE, targets, rivals, 0, 400, costs, {}, stages({ [0] = 2, [1] = 2 }))
check("a tie slows the shared project", targets[idx], MARS)
check("a tie takes option B", option, 2)

-- Past the Exoplanet the racer's next project is a laser.
idx, option = vote(RTYPE, targets, rivals, 0, 400, costs, {}, stages({ [1] = 4 }))
check("laser after the Exoplanet", targets[idx], ORBITAL)
check("laser takes option B", option, 2)

-- A project the ballot does not offer falls through to the next one.
idx = vote(RTYPE, { SATELLITE, MANHATTAN, TERRESTRIAL }, rivals, 0, 400, costs, {},
    stages({ [1] = 4 }))
check("second laser when the first is absent", idx, 3)
idx = vote(RTYPE, { MANHATTAN }, rivals, 0, 400, costs, {}, stages({ [1] = 2 }))
check("no space target leaves the ballot alone", idx, nil)

-- Nothing else is touched, and the switches hold.
check("another resolution", vote("WC_RES_HERITAGE_ORG", targets, rivals, 0, 400, costs, {},
    stages({ [1] = 2 })), nil)
check("kill switch", vote(RTYPE, targets, rivals, 0, 400, costs, { PublicWorksDenial = false },
    stages({ [1] = 2 })), nil)
check("a Diplomatic racer keeps its vote", vote(RTYPE, targets, rivals, 0, 400, costs,
    { VictoryLane = "diplomatic" }, stages({ [1] = 2 })), nil)
idx, option, votes = vote(RTYPE, targets, rivals, 0, 400, costs, { PublicWorksMaxVotes = 3 },
    stages({ [1] = 2 }))
check("the vote cap reads its setting", votes, 3)

-- The stage reader counts landed chain projects and nothing else.
local realPlayers, realInfo = Players, GameInfo
Players = { [1] = { GetStats = function()
    return { GetNumProjectsAdvanced = function(_, index) return index <= 2 and 1 or 0 end }
end } }
GameInfo = { Projects = {
    PROJECT_LAUNCH_EARTH_SATELLITE = { Index = 1 },
    PROJECT_LAUNCH_MOON_LANDING = { Index = 2 },
    PROJECT_LAUNCH_MARS_BASE = { Index = 3 },
    PROJECT_LAUNCH_EXOPLANET_EXPEDITION = { Index = 4 },
} }
check("stage counts landed projects", CivvisSpaceStage(1), 2)
Players, GameInfo = realPlayers, realInfo

-- The ballot itself: a Science seat, Public Works on the ballot, a rival on
-- the Moon and on 14 Diplomatic Victory points.
local function upvalue(fn, key)
    for i = 1, 100 do
        local name, value = debug.getupvalue(fn, i)
        if name == nil then break end
        if name == key then return value end
    end
    error("missing upvalue " .. key)
end
local tick = upvalue(CivvisQueue.onUiPulse, "tick")
local config = { Play = true, CivvisDecides = false, VictoryLane = "science" }
for i = 1, 100 do
    local name = debug.getupvalue(tick, i)
    if name == nil then break end
    if name == "cfg" then debug.setupvalue(tick, i, config) end
end
local hooks = {}
Events = setmetatable({ WorldCongressStage1 = { Add = function(fn) hooks.stage = fn end } },
    { __index = function() return stub() end })
LuaEvents = setmetatable({ CivvisCongressBallot = { Add = function(fn) hooks.popup = fn end } },
    { __index = function() return stub() end })
Automation = { Log = function() end }
local table20 = { MaxVotes = 20 }
for k = 0, 20 do table20[k] = 2 * (k + 1) * k end
local wc = {
    GetVotesandFavorCost = function() return table20 end,
    GetResolutions = function()
        return { Stage = 2147483647, { Type = "WC_RES_PUBLIC_WORKS",
            TargetType = "ProjectType", PossibleTargets = targets } }
    end,
}
Game = {
    GetLocalPlayer = function() return 0 end,
    GetCurrentGameTurn = function() return 152 end,
    GetCurrentTurnSegment = function() return "TURNSEG_WORLDCONGRESS_1" end,
    GetWorldCongress = function() return wc end,
}
DB = { MakeHash = function(name) return name end }
GameInfo = {
    Projects = {
        PROJECT_LAUNCH_EARTH_SATELLITE = { Index = 1 },
        PROJECT_LAUNCH_MOON_LANDING = { Index = 2 },
        PROJECT_LAUNCH_MARS_BASE = { Index = 3 },
        PROJECT_LAUNCH_EXOPLANET_EXPEDITION = { Index = 4 },
    },
    Resolutions = { WC_RES_PUBLIC_WORKS = { ResolutionType = "WC_RES_PUBLIC_WORKS", Hash = 1 } },
}
local bank = 300
Players = {}
for id = 0, 2 do
    local landed = id == 1 and 2 or 0
    Players[id] = {
        IsTurnActive = function() return false end,
        GetFavor = function() return bank end,
        GetFavorEnteringCongress = function() return bank end,
        GetScore = function() return 800 + id end,
        GetStats = function()
            return {
                GetDiplomaticVictoryPoints = function() return id == 1 and 14 or 5 end,
                GetNumProjectsAdvanced = function(_, index) return index <= landed and 1 or 0 end,
            }
        end,
    }
end
PlayerManager = { GetAliveMajorIDs = function() return { 0, 1, 2 } end }
PlayerOperations = {
    PARAM_RESOLUTION_TYPE = "type", PARAM_WORLD_CONGRESS_VOTES = "votes",
    PARAM_RESOLUTION_OPTION = "option", PARAM_RESOLUTION_SELECTION = "selection",
    WORLD_CONGRESS_RESOLUTION_VOTE = "vote", WORLD_CONGRESS_SUBMIT_TURN = "submit",
}
local requests = {}
UI = { RequestPlayerOperation = function(pid, operation, params)
    assert(pid == 0)
    if operation == "vote" then requests[#requests + 1] = params end
end }
tick()
local cast = upvalue(hooks.popup, "castBallot")
local ballot = upvalue(cast, "voteWorldCongress")
local tally = upvalue(ballot, "envoyTally")
-- The leader's last Public Works vote was B, so the free-vote denial would
-- flip the ballot to A on the racer's own project without the guard.
tally.wc_last_option = { ["WC_RES_PUBLIC_WORKS:1"] = 2 }

ballot(0)
check("one Public Works request", #requests, 1)
check("the ballot takes option B", requests[1].option, 2)
check("on the racer's Mars Base", targets[requests[1].selection + 1], MARS)
check("half the bank buys the cap", requests[1].votes, 9)

bank = 0
requests = {}
ballot(0)
check("a broke ballot still casts its free vote", requests[1].votes, 1)
check("the denial does not flip the free vote", requests[1].option, 2)
check("the free vote stays on Mars", targets[requests[1].selection + 1], MARS)

config.PublicWorksDenial = false
bank = 300
requests = {}
ballot(0)
check("with the switch off, the old first target", requests[1].selection, 0)

-- `science-seat-races-the-congress`: the Congress racer on a Science seat
-- keeps slowing a space racer; only the Diplomatic lane stands this vote down.
idx, option, votes, price = vote(RTYPE, targets, rivals, 0, 400, costs,
    { VictoryLane = "science", ScienceCongressRacer = true }, stages({ [1] = 2 }))
check("science racer still slows the space racer", targets[idx], MARS)
check("science racer keeps option B", option, 2)
idx = vote(RTYPE, targets, rivals, 0, 400, costs,
    { VictoryLane = "diplomatic" }, stages({ [1] = 2 }))
check("the Diplomatic lane still stands down", idx, nil)

if failures > 0 then
    error(failures .. " failure(s)")
end
print("public_works_vote_test OK")
