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

-- Offline regression for a wonder whose plot the host refuses while offering
-- another: `CivvisWonderOfferedPlot` picks the offered plot nearest the one
-- CIVVIS asked for, and `buildParams` builds the wonder there instead of
-- refusing it. Districts keep the refusal.
--
-- Run: lua5.1 tools/civ6_control/mod/wonder_offered_plot_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end

setmetatable(_G, { __index = function(_, key)
	if key == "CivvisWonderOfferedPlot" then return rawget(_G, key) end
	return stub()
end })
local logs = {}
Automation = { Log = function(line) logs[#logs + 1] = line end }
UI = { DataError = function() end }
CivvisControlConfig = { Play = true }
local config = CivvisControlConfig

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtimeErr = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtimeErr))

local choose = rawget(_G, "CivvisWonderOfferedPlot")
assert(type(choose) == "function", "CivvisControlAgent.lua did not export CivvisWonderOfferedPlot")

local failures = 0
local function check(label, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", label, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", label, tostring(got)))
	end
end
local function at(plot) return plot and (plot.x .. "," .. plot.y) or "nil" end

-- A plain distance for the pure checks: the larger offset difference.
local function span(x1, y1, x2, y2) return math.max(math.abs(x1 - x2), math.abs(y1 - y2)) end

-- 1. The live refusals of 2026-11-12/13.
check("Statue of Liberty, 195128Z t130",
	at(choose({ { x = 24, y = 34 } }, 23, 35, {}, span)), "24,34")
check("Statue of Liberty, 204218Z t138",
	at(choose({ { x = 61, y = 39 } }, 60, 38, {}, span)), "61,39")
check("Orszaghaz: the neighbour of three",
	at(choose({ { x = 61, y = 35 }, { x = 59, y = 37 }, { x = 60, y = 38 } }, 59, 36, {}, span)), "59,37")
check("Taj Mahal: the neighbour of four",
	at(choose({ { x = 13, y = 16 }, { x = 13, y = 15 }, { x = 13, y = 17 }, { x = 12, y = 17 } },
		11, 17, {}, span)), "12,17")
-- 2. Its edges.
check("nothing offered is still a refusal", at(choose({}, 23, 35, {}, span)), "nil")
check("no list is still a refusal", at(choose(nil, 23, 35, {}, span)), "nil")
check("no plot asked: nothing to be near", at(choose({ { x = 24, y = 34 } }, nil, nil, {}, span)), "nil")
check("the switch off is still a refusal",
	at(choose({ { x = 24, y = 34 } }, 23, 35, { WonderTakesOfferedPlot = false }, span)), "nil")
check("a tie keeps the host's first", at(choose({ { x = 24, y = 35 }, { x = 22, y = 35 } }, 23, 35, {}, span)), "24,35")
check("a malformed entry is passed over",
	at(choose({ { x = "?" }, { x = 30, y = 40 } }, 23, 35, {}, span)), "30,40")
check("a host that will not measure falls back to the offsets",
	at(choose({ { x = 40, y = 40 }, { x = 24, y = 34 } }, 23, 35, {}, function() return nil end)), "24,34")
check("the host's own distance decides",
	at(choose({ { x = 24, y = 34 }, { x = 40, y = 40 } }, 23, 35, {},
		function(_, _, px) return px == 40 and 1 or 9 end)), "40,40")

-- 3. `buildParams` itself. It is a local of the chunk; find it among the
-- upvalues the agent's own functions hold.
local function findUpvalue(name)
	local seen, queue = {}, {}
	for _, value in pairs(_G) do
		if type(value) == "function" then queue[#queue + 1] = value end
		if type(value) == "table" then
			for _, inner in pairs(value) do
				if type(inner) == "function" then queue[#queue + 1] = inner end
			end
		end
	end
	local head = 1
	while head <= #queue do
		local fn = queue[head]
		head = head + 1
		if not seen[fn] then
			seen[fn] = true
			for i = 1, 250 do
				local key, value = debug.getupvalue(fn, i)
				if key == nil then break end
				if key == name and type(value) == "function" then return value end
				if type(value) == "function" and not seen[value] then queue[#queue + 1] = value end
			end
		end
	end
	return nil
end
local buildParams = findUpvalue("buildParams")
assert(type(buildParams) == "function", "buildParams is not reachable from the agent's globals")

CityOperationTypes = {
	PARAM_INSERT_MODE = "insert", VALUE_REPLACE_AT = "replace_at",
	PARAM_QUEUE_DESTINATION_LOCATION = "destination",
	PARAM_UNIT_TYPE = "unit", PARAM_BUILDING_TYPE = "building", PARAM_DISTRICT_TYPE = "district",
	PARAM_X = "x", PARAM_Y = "y", BUILD = "build",
}
CityOperationResults = { PLOTS = "plots" }
local hostPlots = {}
Map = {
	GetPlotByIndex = function(index)
		local plot = hostPlots[index]
		if plot == nil then return nil end
		return { GetX = function() return plot.x end, GetY = function() return plot.y end }
	end,
	GetPlotDistance = span,
}
CityManager = { GetOperationTargets = function()
	local indices = {}
	for index in pairs(hostPlots) do indices[#indices + 1] = index end
	table.sort(indices)
	return { plots = indices }
end }
GameInfo = { Buildings = {
	BUILDING_STATUE_LIBERTY = { IsWonder = true },
}, Districts = {} }
Game = { GetCurrentGameTurn = function() return 138 end }
local city = {
	GetID = function() return 196610 end,
	GetBuildQueue = function() return {
		HasBeenPlaced = function() return false end,
		CanProduce = function() return true, {} end,
	} end,
}
local statue = { Kind = "KIND_BUILDING", Type = "BUILDING_STATUE_LIBERTY", Hash = 4242 }
local campus = { Kind = "KIND_DISTRICT", Type = "DISTRICT_CAMPUS", Hash = 77 }
local function lastRow(kind)
	for i = #logs, 1, -1 do
		if logs[i]:find('"kind":"' .. kind .. '"', 1, true) then return logs[i] end
	end
	return nil
end

-- The host offers (61,39) and CIVVIS asks (60,38): the wonder is built on
-- the host's plot and the row says where it went.
do
	logs = {}
	hostPlots = { [9001] = { x = 61, y = 39 } }
	config.WonderTakesOfferedPlot = nil
	local params = buildParams(statue, city, 60, 38)
	check("the wonder is built", type(params), "table")
	check("on the host's x", params and params.x, 61)
	check("on the host's y", params and params.y, 39)
	check("as that wonder", params and params.building, 4242)
	local row = lastRow("build_plot_substituted")
	check("the substitution is reported", row ~= nil, true)
	check("with the plot asked", row ~= nil and row:find('"x":60', 1, true) ~= nil
		and row:find('"y":38', 1, true) ~= nil, true)
	check("and the plot used", row ~= nil and row:find('"to_x":61', 1, true) ~= nil
		and row:find('"to_y":39', 1, true) ~= nil, true)
	check("no refusal row", lastRow("build_no_plot"), nil)
end

-- The plot asked is offered: used as it is, nothing reported.
do
	logs = {}
	hostPlots = { [9001] = { x = 61, y = 39 }, [9002] = { x = 60, y = 38 } }
	local params = buildParams(statue, city, 60, 38)
	check("an offered plot is used as asked", params and (params.x .. "," .. params.y), "60,38")
	check("with no substitution row", lastRow("build_plot_substituted"), nil)
end

-- Nothing offered: the refusal it always was.
do
	logs = {}
	hostPlots = {}
	check("no ground: refused", buildParams(statue, city, 60, 38), nil)
	local row = lastRow("build_no_plot")
	check("no ground: the refusal row", row ~= nil and row:find('"offered":0', 1, true) ~= nil, true)
end

-- Switched off: the refusal, with the host's offer in the row as before.
do
	logs = {}
	hostPlots = { [9001] = { x = 61, y = 39 } }
	config.WonderTakesOfferedPlot = false
	check("switch off: refused", buildParams(statue, city, 60, 38), nil)
	local row = lastRow("build_no_plot")
	check("switch off: the row carries the offer",
		row ~= nil and row:find('"offered":1', 1, true) ~= nil, true)
	config.WonderTakesOfferedPlot = nil
end

-- A district keeps the refusal: its plot is the decision.
do
	logs = {}
	hostPlots = { [9001] = { x = 61, y = 39 } }
	check("a district is still refused", buildParams(campus, city, 60, 38), nil)
	check("a district is not substituted", lastRow("build_plot_substituted"), nil)
	check("a district's refusal row", lastRow("build_no_plot") ~= nil, true)
end

if failures > 0 then
	print(string.format("%d FAILED", failures))
	os.exit(1)
end
print("all wonder offered-plot checks passed")
