-- A stranded settler must not hold the expansion gate shut.
--
-- ⚠ Loads the SHIPPED `CivvisControlAgent.lua` and calls the predicate the agent
-- itself calls. A test that re-implemented the threshold would pass while the
-- agent kept its old behaviour.
--
-- Run: lua5.1 tools/civ6_control/mod/stranded_settler_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end
setmetatable(_G, { __index = function(_, k)
	if k == "CivvisSettlerIsStranded" then return rawget(_G, k) end
	return stub()
end })

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
-- ⚠ REPORT THE PCALL RESULT. A chunk that dies at load must fail this test, not
-- pass because the export happened first — that hole let an agent that raises at
-- load ship green once already.
local ran, runtime_err = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtime_err))

local stranded = rawget(_G, "CivvisSettlerIsStranded")
assert(type(stranded) == "function",
	"CivvisControlAgent.lua did not export CivvisSettlerIsStranded")

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", name, tostring(got)))
	end
end

-- A settler that has never been seen idle, or has just moved, is in flight.
check("nil streak is not stranded", stranded(nil), false)
check("zero streak is not stranded", stranded(0), false)
check("one turn is not stranded", stranded(1), false)

-- ⚠ The threshold must sit BELOW the observed streaks or it never fires. The
-- fleet census reports 38% of runs parking a settler for >=15 consecutive turns
-- at full movement, median streak 37; the live run this was written for sat
-- still from t153 to the end.
check("11 turns is still in flight", stranded(11), false)
check("12 turns is stranded", stranded(12), true)
check("15 turns (the census floor) is stranded", stranded(15), true)
check("37 turns (the census median) is stranded", stranded(37), true)

-- The gate arithmetic the agent performs: `(settlers - stranded) < inFlight`.
local function gateOpen(settlers, strandedCount, inFlight)
	return (settlers - strandedCount) < inFlight
end
check("no settlers -> gate open", gateOpen(0, 0, 1), true)
check("one walking settler -> gate SHUT", gateOpen(1, 0, 1), false)
check("one stranded settler -> gate OPEN", gateOpen(1, 1, 1), true)
check("one walking + one stranded -> gate SHUT", gateOpen(2, 1, 1), false)
-- The cap the old comment defends still holds: seventeen settlers, two cities.
check("two walking settlers -> gate SHUT", gateOpen(2, 0, 1), false)

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("\nall stranded-settler checks passed")
