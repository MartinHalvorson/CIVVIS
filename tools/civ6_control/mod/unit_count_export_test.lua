-- The standings' UNIT column reads `public_stats.unit_count` and
-- `public_stats.military_unit_count`. This runs the shipped
-- `publicEmpireStats` against a stub roster: every unit is counted, only the
-- land, naval and air combat formations are an army, and neither a unit that
-- will not answer nor a roster that cannot be read takes the export down.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local f = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = f:read("*a"); f:close()
local block = assert(source:match(
    "\tlocal function (publicEmpireStats%(subject, suzerainCounts%).-\n\tend)\n"),
    "publicEmpireStats is no longer where the exporter keeps it")
local env = setmetatable({
    try = function(fn, fallback)
        local ok, value = pcall(fn)
        if ok then return value end
        return fallback
    end,
    wonderBuildingIndices = {},
    YieldTypes = {FOOD = 0, PRODUCTION = 1},
    GameInfo = {
        Units = {
            [1] = {FormationClass = "FORMATION_CLASS_LAND_COMBAT"},
            [2] = {FormationClass = "FORMATION_CLASS_NAVAL"},
            [3] = {FormationClass = "FORMATION_CLASS_AIR"},
            [4] = {FormationClass = "FORMATION_CLASS_CIVILIAN"},
            [5] = {FormationClass = "FORMATION_CLASS_SUPPORT"},
        },
        WMDs = {},
    },
}, {__index = _G})
local chunk = assert(loadstring("local function " .. block .. "\nreturn publicEmpireStats"))
setfenv(chunk, env)
local publicEmpireStats = chunk()

local function unitOf(kind)
    return {GetUnitType = function()
        if kind == nil then error("not a valid instance") end
        return kind
    end}
end
local function subjectWith(kinds)
    local units = {}
    for index = 1, #kinds do units[index] = unitOf(kinds[index] or nil) end
    return {
        GetID = function() return 3 end,
        GetCities = function() return {Members = function() return ipairs({}) end} end,
        GetWMDs = function() return nil end,
        GetUnits = function() return {Members = function() return ipairs(units) end} end,
    }
end

-- A Warrior, a Galley, a Biplane, a Settler, a Builder and a Battering Ram.
local stats = publicEmpireStats(subjectWith({1, 2, 3, 4, 4, 5}), nil)
assert(stats.unit_count == 6, "every unit on the roster is a unit")
assert(stats.military_unit_count == 3,
    "land, naval and air combat formations are the army; civilians and support are not")

-- An empire with no units reports a real nought, not the unread sentinel.
stats = publicEmpireStats(subjectWith({}), nil)
assert(stats.unit_count == 0 and stats.military_unit_count == 0)

-- A unit that throws on its type is still a unit; only its class is lost.
stats = publicEmpireStats(subjectWith({1, false, 2}), nil)
assert(stats.unit_count == 3, "a unit that will not answer still counts")
assert(stats.military_unit_count == 2)

-- A roster that cannot be read leaves the sentinel the mirror reads as
-- "no figure", and the rest of the aggregate still crosses.
local unreadable = subjectWith({})
unreadable.GetUnits = function() error("no roster") end
stats = publicEmpireStats(unreadable, {[3] = 2})
assert(stats.unit_count == -1 and stats.military_unit_count == -1)
assert(stats.suzerain_count == 2 and stats.city_count == 0)

print("unit count export checks passed")
