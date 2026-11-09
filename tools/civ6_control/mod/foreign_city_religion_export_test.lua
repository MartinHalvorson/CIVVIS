-- Exercise the actual visibility and majority reader used by BOTH foreign lists.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local helper = assert(source:match("%-%- BEGIN visible foreign city religion(.-)%-%- END visible foreign city religion"))
local _, calls = source:gsub("CivvisReligionState%.visibleCity%(pid, city%)", "")
assert(calls == 3, "one definition and both major/minor city exporter calls")
local _, flags = source:gsub("religion_observed = religionObserved", "")
assert(flags == 2, "both foreign-city lists carry the observation flag")

local seen, majority, reads = true, 1, 0
local religion = {GetMajorityReligion = function() reads = reads + 1; return majority end}
local city = {
 GetX = function() return 24 end,
 GetY = function() return 31 end,
 GetReligion = function() return religion end,
}
local visibility = {IsVisible = function(_, x, y) assert(x == 24 and y == 31); return seen end}
local env = setmetatable({
 CivvisReligionState = {}, PlayersVisibility = {[0] = visibility},
 GameInfo = {Religions = {[1] = {ReligionType = "RELIGION_BUDDHISM"},
                         [2] = {ReligionType = "RELIGION_ISLAM"}}},
 try = function(fn, fallback) local ok, result = pcall(fn); if ok then return result end; return fallback end,
}, {__index = _G})
local chunk = assert(loadstring(helper)); setfenv(chunk, env); chunk()
local function observe() return env.CivvisReligionState.visibleCity(0, city) end
local observed, faith = observe()
assert(observed == true and faith == "RELIGION_BUDDHISM" and reads == 1)
majority = 2
observed, faith = observe()
assert(observed == true and faith == "RELIGION_ISLAM")

-- No majority is an observed fact, not an unknown read.
majority = -1
observed, faith = observe()
assert(observed == true and faith == nil)

-- Revelation alone is insufficient: neither fog nor visibility errors may
-- inspect the foreign city's current religion.
for _, hidden in ipairs({false, "visibility failure"}) do
 local before = reads
 visibility.IsVisible = function() if hidden then error(hidden) end; return false end
 observed, faith = observe()
 assert(observed == false and faith == nil and reads == before)
end
visibility.IsVisible = function() return true end
for _, bad in ipairs({"missing", "unsupported", "throws"}) do
 religion.GetMajorityReligion = function()
  if bad == "throws" then error("native read failed") end
  if bad == "unsupported" then return 99 end
  return nil
 end
 observed, faith = observe()
 assert(observed == false and faith == nil, "unknown must not clear an observed majority")
end
print("foreign city religion export: visible majority, no majority, fog and failed reads passed")
