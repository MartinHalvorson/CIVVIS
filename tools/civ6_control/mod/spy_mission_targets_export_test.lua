-- `CivvisSpyMenu.read`: an idle Spy's host menu with each mission's target plot
-- and host odds, read the way EspionageChooser/EspionagePopup read them.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local f = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = f:read("*a"); f:close()
local block = assert(source:match("(CivvisSpyMenu = {.-\n};\n%-%- END CivvisSpyMenu%.read)"),
	"the helper and its end marker")
assert(source:find("spy_missions_available = spyMissions and spyMissions.names or nil", 1, true),
	"the export keeps the names list's shape")
assert(source:find("spy_mission_targets = spyMissions and spyMissions.targets or nil", 1, true),
	"the export carries the targets")

local function plot(x, y, index)
	return { GetX = function() return x end, GetY = function() return y end, index = index }
end
local plots = { [101] = plot(10, 20, 101), [102] = plot(11, 20, 102) }
local cityPlot = plots[101]
local campusIndex = 102
local owner = 2
local city = {
	GetX = function() return 10 end, GetY = function() return 20 end,
	GetOwner = function() return owner end,
}
local operations = {
	{ OperationType = "UNITOPERATION_SPY_STEAL_TECH_BOOST", CategoryInUI = "OFFENSIVESPY", Hash = 1, Index = 11 },
	{ OperationType = "UNITOPERATION_SPY_LISTENING_POST", CategoryInUI = "OFFENSIVESPY", Hash = 2, Index = 12 },
	{ OperationType = "UNITOPERATION_SPY_DISRUPT_ROCKETRY", CategoryInUI = "OFFENSIVESPY", Hash = 3, Index = 13 },
	{ OperationType = "UNITOPERATION_SPY_COUNTERSPY", CategoryInUI = "DEFENSIVESPY", Hash = 4, Index = 14 },
}
local allowed = { [1] = true, [2] = true, [3] = false, [4] = true }
local odds = true
local env = setmetatable({
	try = function(fn, fallback) local ok, v = pcall(fn); if ok then return v end; return fallback end,
	Map = {
		GetPlot = function(x, y) return x == 10 and y == 20 and cityPlot or plot(x, y, -1) end,
		GetPlotByIndex = function(i) return plots[i] end,
	},
	Cities = { GetPlotPurchaseCity = function() return city end },
	GameInfo = { UnitOperations = function()
		local i = 0
		return function() i = i + 1; return operations[i] end
	end },
	UnitOperationResults = { PLOTS = "PLOTS" },
	UnitManager = {
		CanStartOperation = function(_, hash, target)
			assert(target == cityPlot, "the menu is asked from the city plot")
			if not allowed[hash] then return false, nil end
			if hash == 1 then return true, { PLOTS = { campusIndex } } end
			return true, {}
		end,
		GetResultProbability = function(index, _, target)
			if not odds then error("no such call on this build") end
			if index == 11 then
				assert(target == plots[102], "the steal is priced at its Campus")
				return { ESPIONAGE_SUCCESS_UNDETECTED = 0.5, ESPIONAGE_SUCCESS_MUST_ESCAPE = 0.1,
					ESPIONAGE_FAIL_UNDETECTED = 0.15, ESPIONAGE_FAIL_MUST_ESCAPE = 0.05 }
			end
			return { ESPIONAGE_SUCCESS_UNDETECTED = 1 }
		end,
	},
}, { __index = _G })
local chunk = assert(loadstring(block)); setfenv(chunk, env); chunk()
local read = env.CivvisSpyMenu.read
local unit = { GetX = function() return 10 end, GetY = function() return 20 end }

local menu = assert(read(unit, 0), "a rival city offers missions")
assert(#menu.names == 2 and menu.names[1] == "UNITOPERATION_SPY_LISTENING_POST"
	and menu.names[2] == "UNITOPERATION_SPY_STEAL_TECH_BOOST", "sorted names, refused ones dropped")
assert(#menu.targets == 2, "one target row per offered mission")
local steal = menu.targets[2]
assert(steal.op == "UNITOPERATION_SPY_STEAL_TECH_BOOST" and steal.x == 11 and steal.y == 20,
	"the steal aims at the host's Campus plot")
assert(math.abs(steal.success - 0.6) < 1e-9 and math.abs(steal.fail - 0.2) < 1e-9, "success and failure odds")
local post = menu.targets[1]
assert(post.x == 10 and post.y == 20 and post.success == 1, "a district-less mission aims at the city")

odds = false
local blind = assert(read(unit, 0))
assert(blind.targets[2].success == nil and blind.targets[2].fail == nil, "no odds call, no odds")

owner = 0
local home = assert(read(unit, 0))
assert(#home.names == 1 and home.names[1] == "UNITOPERATION_SPY_COUNTERSPY", "our own city: counterspy only")

allowed = {}
assert(read(unit, 0) == nil, "nothing offered is nil, not an empty table")
print("spy mission targets export checks passed")
