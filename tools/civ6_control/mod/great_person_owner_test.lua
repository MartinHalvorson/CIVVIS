-- Offline regression for Great Person activation-target ownership.
--
-- The host's `GetActivationHighlightPlots` list can contain a qualifying
-- district owned by another civilization.  Such a plot is visible to the
-- host UI but is not legal for our Great Person, especially when the border
-- is closed.  The state exporter must discard it before CIVVIS chooses a
-- destination; an owner read that fails must be discarded too.
--
-- Run: lua5.1 tools/civ6_control/mod/great_person_owner_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end

Automation = { Log = function() end }
UnitOperationTypes = { PARAM_X = "x", PARAM_Y = "y" }
UnitCommandTypes = {}
CivvisControlConfig = { GreatPeopleUse = true, ExportState = false }

local plots = {}
local function plot(x, y, owner, raises)
	return {
		GetX = function() return x end,
		GetY = function() return y end,
		GetOwner = function()
			if raises then error("owner unavailable") end
			return owner
		end,
	}
end

plots[11] = plot(1, 1, 0) -- our compatible district
plots[22] = plot(2, 2, 2) -- nearest, but behind a foreign border
plots[33] = plot(3, 3, nil, true) -- incomplete host read
plots[44] = plot(4, 4, 0) -- ours, but no compatible slot
Map = {
	GetPlotByIndex = function(index) return plots[index] end,
	GetPlotDistance = function(x1, y1, x2, y2)
		return math.max(math.abs(x1 - x2), math.abs(y1 - y2))
	end,
}

GameInfo = setmetatable({}, { __index = function(_, key)
	if key == "UnitOperations" or key == "UnitCommands" then
		return setmetatable({}, {
			__index = function(_, name) return { Hash = name } end,
		})
	end
	return stub()
end })

local hidden = {}
setmetatable(_G, { __index = function(_, key)
	if hidden[key] then return nil end
	return stub()
end })

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtime_err = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtime_err))

local activationPlots = rawget(_G, "CivvisGreatPersonActivationPlots")
assert(type(activationPlots) == "function",
	"CivvisControlAgent.lua did not export CivvisGreatPersonActivationPlots")

local unit = { GetX = function() return 0 end, GetY = function() return 0 end }
local gp = {
	GetActivationHighlightPlots = function()
		return { 22, 11, 33, 44 }
	end,
}
local survey = { district_plots = { [11] = true, [22] = true, [33] = true, [44] = true } }
local openPlots = { [11] = true, [22] = true, [33] = true, [44] = false }
local targets = activationPlots(unit, gp, 0, survey, openPlots)

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", name, tostring(got)))
	end
end
local function find(x, y)
	for _, target in ipairs(targets) do
		if target.x == x and target.y == y then return target end
	end
	return nil
end

check("only owned highlights cross", #targets, 2)
check("owned compatible district crosses", find(1, 1) ~= nil, true)
check("owned compatible district keeps slot truth", find(1, 1).slot_open, true)
check("owned target keeps host distance", find(1, 1).distance, 1)
check("foreign highlight is discarded", find(2, 2), nil)
check("owner-read failure is discarded", find(3, 3), nil)
check("owned slotless district crosses", find(4, 4) ~= nil, true)
check("owned slotless district is marked closed", find(4, 4).slot_open, false)

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("\nall Great Person ownership-export checks passed")
