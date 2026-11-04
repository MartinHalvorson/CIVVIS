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

-- Run: lua5.1 tools/civ6_control/mod/heritage_culture_seat_test.lua
-- `CivvisHeritageCultureSeat`: a Culture seat votes Heritage Organization on
-- the type that widens its lead over the strongest culture rival -- A where
-- it draws more tourism than the rival, B where the rival draws more.
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

local SCULPTURE = "LOC_GREAT_WORK_OBJECT_SCULPTURE_NAME"
local WRITING = "LOC_GREAT_WORK_OBJECT_WRITING_NAME"
local RELIC = "LOC_GREAT_WORK_OBJECT_RELIC_NAME"
local costs = { MaxVotes = 20 }
for k = 0, 20 do costs[k] = 2 * (k + 1) * k end
local targets = { SCULPTURE, WRITING, RELIC }
local cands = { { id = 1, culture = 3 }, { id = 2, culture = 14 } }
local culture = { VictoryLane = "culture" }
local held = function(table_)
    return function(player) return table_[player] or {} end
end

-- Rome (2) holds three relics; we (0) hold six writings: B on Relic.
local rome = held({
    [0] = { [WRITING] = 12, [SCULPTURE] = 2 },
    [2] = { [RELIC] = 24, [WRITING] = 4 },
})
local option, selection, votes, favor, spent = CivvisHeritageCultureSeat("WC_RES_HERITAGE_ORG",
    targets, cands, 0, 300, costs, { WC_RES_HERITAGE_ORG = 3 }, culture, 1, 1, 1, 0, rome)
check("the rival's lead is the widest gap: option B", option, 2)
check("on the type the rival draws the most from", targets[selection], RELIC)
check("votes are last A plus one", votes, 4)
check("favor is spent", favor, 300 - costs[3])
check("and counted", spent, costs[3])

-- We out-draw the strongest rival on Writing by more than any gap it holds.
local ahead = held({
    [0] = { [WRITING] = 30 },
    [2] = { [RELIC] = 8, [WRITING] = 2 },
})
option, selection = CivvisHeritageCultureSeat("WC_RES_HERITAGE_ORG", targets, cands, 0, 300,
    costs, nil, culture, 1, 1, 1, 0, ahead)
check("our lead is the widest gap: option A", option, 1)
check("on the type we draw the most from", targets[selection], WRITING)

-- Not a Culture seat, the kill switches, another resolution, or no rival:
-- the values passed in come back unchanged.
local function unchanged(name, rtype, cands_, config)
    local o, s, v, f, sp = CivvisHeritageCultureSeat(rtype, targets, cands_, 0, 300, costs, nil,
        config, 7, 8, 9, 10, rome)
    check(name, table.concat({ o, s, v, f, sp }, ","), "7,8,9,300,10")
end
unchanged("a Diplomatic seat is left to the denial", "WC_RES_HERITAGE_ORG", cands,
    { VictoryLane = "diplomatic" })
unchanged("the Heritage kill switch", "WC_RES_HERITAGE_ORG", cands,
    { VictoryLane = "culture", HeritageDenial = false })
unchanged("its own kill switch", "WC_RES_HERITAGE_ORG", cands,
    { VictoryLane = "culture", HeritageCultureSeat = false })
unchanged("another resolution", "WC_RES_TRADE_TREATY", cands, culture)
unchanged("no rival", "WC_RES_HERITAGE_ORG", { { id = 0, culture = 50 } }, culture)

-- Nobody draws tourism from the ballot's types: no vote is changed.
local o = CivvisHeritageCultureSeat("WC_RES_HERITAGE_ORG", targets, cands, 0, 300, costs, nil,
    culture, 7, 8, 9, 10, held({}))
check("no gap on the ballot leaves the option", o, 7)

-- An empty bank still chooses, with the free vote.
option, selection, votes, favor = CivvisHeritageCultureSeat("WC_RES_HERITAGE_ORG", targets,
    cands, 0, 0, costs, { WC_RES_HERITAGE_ORG = 3 }, culture, 1, 1, 1, 0, rome)
check("an empty bank names the type", targets[selection], RELIC)
check("with the free vote", votes, 1)
check("costing nothing", favor, 0)

if failures > 0 then
    print(failures .. " failure(s)")
    os.exit(1)
end
print("all heritage culture seat checks passed")
