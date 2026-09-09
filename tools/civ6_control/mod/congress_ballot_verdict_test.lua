-- Match the complete native Congress selection, not merely its vote count.
-- Run under Lua 5.1; loads the actual agent helper and checks its live wiring.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end
setmetatable(_G, { __index = function(_, key)
	if key == "CivvisCongressBallotVerdict" then return rawget(_G, key) end
	return stub()
end })
assert(loadfile(here .. "/CivvisControlAgent.lua"))()
local verdict = assert(rawget(_G, "CivvisCongressBallotVerdict"))
local function ask(votes, option, target)
	return { votes = votes, option = option, target = target }
end
local function check(name, condition)
	assert(condition, name)
	print("ok " .. name)
end
local exact = verdict(ask(3, 2, 4), ask(3, 2, "4"))
check("matching native vote accepts numeric/string player IDs", exact.registered)
check("all three dimensions are independently reported",
	exact.count_matches and exact.option_matches and exact.target_matches)

-- The live t161 Mercenary Companies row: one vote was requested for B,
-- one was recorded for A. The old count-only verifier reported success.
local opposite = verdict(ask(1, 2, "PROMOTION_CLASS_MELEE"),
	ask(1, 1, "PROMOTION_CLASS_MELEE"))
check("opposite option is not registered", not opposite.registered)
check("opposite option retains the successful count evidence",
	opposite.count_matches and not opposite.option_matches and opposite.target_matches)
local wrongTarget = verdict(ask(3, 2, 4), ask(3, 2, 0))
check("right option against the wrong player fails", not wrongTarget.registered)
check("wrong target is distinguishable", wrongTarget.option_matches and not wrongTarget.target_matches)
check("named non-player targets compare without localization",
	verdict(ask(1, 1, "RESOURCE_IRON"), ask(1, 1, "RESOURCE_IRON")).registered)
check("under-count fails", not verdict(ask(3, 2, 4), ask(1, 2, 4)).registered)
check("over-count is not an exact actuation match", not verdict(ask(3, 2, 4), ask(4, 2, 4)).registered)
check("missing observed target cannot prove success",
	not verdict(ask(1, 1, 0), ask(1, 1, nil)).registered)
check("missing requested target cannot prove success",
	not verdict(ask(1, 1, nil), ask(1, 1, nil)).registered)
check("missing option cannot prove success", not verdict(ask(1, nil, 0), ask(1, nil, 0)).registered)
check("invalid option cannot prove success", not verdict(ask(1, -1, 0), ask(1, -1, 0)).registered)
check("zero votes cannot prove a submitted ballot", not verdict(ask(0, 1, 0), ask(0, 1, 0)).registered)
check("missing readback cannot prove success", not verdict(ask(1, 1, 0), nil).registered)
check("missing request cannot prove success", not verdict(nil, ask(1, 1, 0)).registered)

-- The helper must verify the native per-voter record, not the winning target.
local src = assert(io.open(here .. "/CivvisControlAgent.lua")):read("*a")
check("native voter target is read", src:find("local target = sel.ResolutionTarget;", 1, true))
check("requested selection retains its actual target",
	src:find("selection = selection - 1, target = targets[selection]", 1, true))
check("live review calls tested verifier",
	src:find("local verdict = CivvisCongressBallotVerdict(ask, r.ours);", 1, true))
check("live event uses complete verdict", src:find("registered = verdict.registered", 1, true))
check("operation calls have an accurately named field", src:find("request_calls =", 1, true))
print("all Congress ballot verdict checks passed")
