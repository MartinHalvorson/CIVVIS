-- Offline regression for `deal_action_names`: the agent names the engine's
-- DealProposalAction numbers once per load, so a `deal_response` whose
-- `action` is a bare number can be decoded from the ledger alone. The test
-- loads the shipped agent (its chunk calls Initialize) against an engine
-- fixture whose enum carries numbers, the way the live host's does, and reads
-- the event back off the print channel the agent writes its ledger to.
--
-- Run: lua5.1 tools/civ6_control/mod/deal_action_names_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end
setmetatable(_G, { __index = function() return stub() end })

CivvisControlConfig = {}
DealProposalAction = { PROPOSED = 0, ACCEPTED = 1, REJECTED = 2, ADJUSTED = 3, EQUALIZE = 4 }

local lines = {}
print = function(line)
	if type(line) == "string" and line:find('"kind":"deal_action_names"', 1, true) then
		lines[#lines + 1] = line
	end
end

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtimeErr = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtimeErr))

assert(#lines == 1, "one deal_action_names event per load, got " .. #lines)
local line = lines[1]
for number, name in pairs({ ["0"] = "PROPOSED", ["1"] = "ACCEPTED", ["2"] = "REJECTED",
		["3"] = "ADJUSTED", ["4"] = "EQUALIZE" }) do
	assert(line:find('"' .. number .. '":"' .. name .. '"', 1, true),
		"names " .. number .. " as " .. name .. ": " .. line)
end

io.write("deal_action_names_test: ok\n")
