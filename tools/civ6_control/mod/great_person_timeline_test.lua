-- Execute the shipped eligibility helper, export expressions, and recruitment
-- branch against a native timeline. No UI/game fixture is needed for these
-- isolated expressions; source extraction fails if their boundaries change.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local function try(fn, fallback)
	local ok, value = pcall(fn)
	if ok then return value end
	return fallback
end
local timeline, eligible, requests = {}, {}, {}
local greatPeople = {
	GetTimeline = function() return timeline end,
	CanRecruitPerson = function(_, pid, id)
		assert(pid == 0)
		if eligible[id] == "throw" then error("unavailable") end
		return eligible[id]
	end,
}
local env = setmetatable({
	try = try, CivvisLedger = {},
	Game = { GetGreatPeople = function() return greatPeople end },
	GameInfo = {
		GreatPersonIndividuals = {
			[1] = { GreatPersonClassType = "SCIENTIST", GreatPersonIndividualType = "HYPATIA" },
		},
	},
	PlayerOperations = { PARAM_GREAT_PERSON_INDIVIDUAL_TYPE = "individual", RECRUIT_GREAT_PERSON = "recruit" },
	UI = { RequestPlayerOperation = function(pid, op, params)
		requests[#requests + 1] = { pid = pid, op = op, individual = params.individual }
	end },
}, { __index = _G })
-- GameInfo iterators return the row as their first result.
env.GameInfo.GreatPersonClasses = function()
	local done = false
	return function()
		if done then return nil end
		done = true; return { GreatPersonClassType = "SCIENTIST" }
	end
end
local function compile(text)
	local fn = assert(loadstring(text)); setfenv(fn, env); return fn()
end
compile(assert(source:match("(CivvisLedger%.greatPersonAvailable = function.-\nend;)")))
local exports = {}
for _, name in ipairs({ "great_person_exhausted", "great_person_costs", "great_person_offers" }) do
	local expression = assert(source:match(name .. " = (try%(function%(%).-\n\t\tend, nil%)),"))
	exports[name] = compile("return function(pid) return " .. expression .. " end")
end
local branch = assert(source:match('(if kind == "gp_recruit".-return false, "gp_class_not_offered";\n\tend)'))
local recruit = compile("return function(pid, kind, verb) " .. branch .. " end")
local function check(label, got, expected)
	assert(got == expected, label .. ": got " .. tostring(got) .. ", expected " .. tostring(expected))
end
local function scenario(label, claimant, canRecruit, available)
	timeline = { { Individual = 1, Claimant = claimant, Cost = 30 } }
	eligible, requests = { [1] = canRecruit }, {}
	local exhausted = exports.great_person_exhausted(0)
	local costs = exports.great_person_costs(0)
	local offers = exports.great_person_offers(0)
	check(label .. " exhaustion", #exhausted == 0, available)
	check(label .. " cost", costs and costs.SCIENTIST, available and 30 or nil)
	check(label .. " offer", offers.SCIENTIST and offers.SCIENTIST.individual,
		available and "HYPATIA" or nil)
	local ok = recruit(0, "gp_recruit", "SCIENTIST")
	check(label .. " request", ok, canRecruit == true)
	check(label .. " request count", #requests, canRecruit == true and 1 or 0)
	if #requests > 0 then check(label .. " individual", requests[1].individual, 1) end
end
scenario("unclaimed future offer", nil, false, true)
scenario("unclaimed ready offer", nil, true, true)
scenario("reserved ready offer", 0, true, true)
scenario("already recruited", 0, false, false)
scenario("other player's claim", 1, false, false)
scenario("eligibility unavailable", 0, "throw", false)
scenario("eligibility unknown", 0, nil, false)
print("all Great Person timeline checks passed")
