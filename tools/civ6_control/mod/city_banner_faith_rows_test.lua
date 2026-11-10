-- Exercise the banner faith-row reader used by the own, major and minor city exporters.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local helper = assert(source:match("%-%- BEGIN city banner faith rows(.-)%-%- END city banner faith rows"))
local _, calls = source:gsub("CivvisReligionState%.cityFaiths%(city%)", "")
assert(calls == 4, "one definition and the own, major and minor city exporter calls")
local _, gated = source:gsub("faiths = religionObserved and CivvisReligionState%.cityFaiths%(city%) or nil", "")
assert(gated == 2, "a foreign city's rows ship only when its majority was readable")

local entries = {
 {Religion = 2, Followers = 7, Pressure = 1840.5},
 {Religion = 0, Followers = 1, Pressure = 55.25},
 {Religion = -1, Followers = 0, Pressure = 4},
}
local asked = {}
local religion = {
 GetReligionsInCity = function() return entries end,
 GetTotalPressureOnCity = function(_, id) asked[#asked + 1] = id; return id * 6 end,
}
local city = {GetReligion = function() return religion end}
local env = setmetatable({
 CivvisReligionState = {},
 GameInfo = {Religions = {[0] = {ReligionType = "RELIGION_PANTHEON"},
                         [2] = {ReligionType = "RELIGION_BUDDHISM"}}},
}, {__index = _G})
local chunk = assert(loadstring(helper)); setfenv(chunk, env); chunk()

local rows = env.CivvisReligionState.cityFaiths(city)
assert(#rows == 3)
assert(rows[1].religion == "RELIGION_BUDDHISM" and rows[1].followers == 7)
assert(rows[1].pressure == 1840.5 and rows[1].per_turn == 12)
assert(rows[2].religion == "RELIGION_PANTHEON" and rows[2].per_turn == 0)
-- A row for no religion keeps its figures and names nothing.
assert(rows[3].religion == nil and rows[3].pressure == 4 and rows[3].per_turn == nil)
assert(#asked == 2, "per-turn pressure is asked only for a real religion index")

-- An unknown religion index is a row without a name, not an error.
entries = {{Religion = 99, Followers = 2, Pressure = 10}}
rows = env.CivvisReligionState.cityFaiths(city)
assert(#rows == 1 and rows[1].religion == nil and rows[1].per_turn == 594)

-- No religion present at all is an observed empty list.
entries = {}
rows = env.CivvisReligionState.cityFaiths(city)
assert(type(rows) == "table" and #rows == 0)

-- A failed native read is an unknown, never an empty list.
religion.GetReligionsInCity = function() error("native read failed") end
assert(env.CivvisReligionState.cityFaiths(city) == nil)
religion.GetReligionsInCity = function() return {{Religion = 2, Followers = 1, Pressure = 1}} end
religion.GetTotalPressureOnCity = function() error("native read failed") end
assert(env.CivvisReligionState.cityFaiths(city) == nil)
print("city banner faith rows: names, pantheon, unknown index, empty and failed reads passed")
