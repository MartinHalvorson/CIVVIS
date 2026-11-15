-- Execute the shipped `CivvisLedger.momentCensus` helper against a native
-- history manager. Source extraction fails if its boundaries change.
CivvisK = CivvisK or { guard = function(fn) pcall(fn) end, attempt = function(fn) return (pcall(fn)) end }
local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local function try(fn, fallback)
	local ok, value = pcall(fn)
	if ok then return value end
	return fallback
end
local moments, asked = {}, {}
local history = {
	GetAllMomentsData = function(_, pid, interest)
		asked[#asked + 1] = { pid = pid, interest = interest }
		if moments == "throw" then error("unavailable") end
		return moments
	end,
}
local env = setmetatable({
	try = try, CivvisLedger = {},
	Game = { GetHistoryManager = function() return history end },
	GameInfo = { Moments = {
		[7] = { MomentType = "MOMENT_BARBARIAN_CAMP_DESTROYED" },
		[9] = { MomentType = "MOMENT_CITY_BUILT_NEAR_VOLCANO" },
	} },
}, { __index = _G })
local function compile(text)
	local fn = assert(loadstring(text)); setfenv(fn, env); return fn()
end
compile(assert(source:match("(CivvisLedger%.momentCensus = function.-\nend;)")))
local census = env.CivvisLedger.momentCensus
local function check(label, got, expected)
	assert(got == expected, label .. ": got " .. tostring(got) .. ", expected " .. tostring(expected))
end

-- The export site calls the helper for the seat itself.
assert(source:find("historic_moments = CivvisLedger.momentCensus(pid),", 1, true), "export site")

-- No moment yet: nil, never an empty table.
moments = {}
check("no moments", census(0), nil)
check("asked for the seat", asked[#asked].pid, 0)
check("asked at the timeline's interest level", asked[#asked].interest, 1)

-- Counted and summed by type; a moment with no Era Score still counts; a
-- moment whose type the database does not know is left out.
moments = {
	{ Type = 7, EraScore = 2 },
	{ Type = 9, EraScore = 1 },
	{ Type = 7, EraScore = 3 },
	{ Type = 7 },
	{ Type = 42, EraScore = 5 },
}
local out = census(0)
check("camp count", out.MOMENT_BARBARIAN_CAMP_DESTROYED[1], 3)
check("camp era score", out.MOMENT_BARBARIAN_CAMP_DESTROYED[2], 5)
check("volcano count", out.MOMENT_CITY_BUILT_NEAR_VOLCANO[1], 1)
check("volcano era score", out.MOMENT_CITY_BUILT_NEAR_VOLCANO[2], 1)
local types = 0
for _ in pairs(out) do types = types + 1 end
check("unknown type left out", types, 2)

-- Only unknown types: still nil.
moments = { { Type = 42, EraScore = 5 } }
check("only unknown types", census(0), nil)

-- A host that throws, answers a non-table, or has no history manager.
moments = "throw"
check("throwing host", census(0), nil)
moments = 12
check("non-table answer", census(0), nil)
history = nil
check("no history manager", census(0), nil)
print("historic_moments_export_test: ok")
