-- Offline test for the envoy spend order.
--
-- ⚠ It loads the SHIPPED `CivvisControlAgent.lua` and calls the function the
-- agent itself calls. A test that re-implemented the sort would have passed
-- while the agent kept its old behaviour -- the same trap that let a name fixed
-- in one emitter fail live in three others.
--
-- `CivvisControlAgent.lua` is a Civ 6 UI script: it indexes globals the game
-- provides (`Players`, `UI`, `Events`, ...) at load time. The stub metatable
-- below answers any global with a permissive dummy so the chunk can run to the
-- point where it defines its functions. Nothing here calls into the engine.
--
-- Run: lua tools/civ6_control/mod/envoy_spend_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

-- Any unknown global returns a table that is callable, indexable, and
-- comparable, so top-level code in the agent neither errors nor branches on it.
local dummy = {}
local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end
setmetatable(_G, { __index = function(_, k)
	if k == "CivvisEnvoySpendOrder" then return rawget(_G, k) end
	return stub()
end })

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))

-- ⚠⚠ REPORT THE PCALL RESULT. The first version of this test wrote
-- `pcall(chunk)` and threw the result away, so a chunk that DIED at load still
-- passed as long as the export had already happened — and that is exactly the
-- failure it should have caught: #1047 shipped an agent that raises at load and
-- this gate called it green. A check that swallows the error it exists to catch
-- is worse than no check.
local ran, runtime_err = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtime_err))

-- ⚠ `rawget`, not `_G.x`: the agent must never be able to pass this test by
-- doing something the game's sandbox forbids. See the bare-global note beside
-- the export itself.
local spendOrder = rawget(_G, "CivvisEnvoySpendOrder")
assert(type(spendOrder) == "function",
	"CivvisControlAgent.lua did not export CivvisEnvoySpendOrder")

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", name, tostring(got)))
	end
end

-- Walk the order the way `chooseEnvoy` does: spend `need` on each target while
-- tokens last, and spend the remainder on the next one when a flip is out of
-- reach. Mirrors the caller's clamp, which is the part the engine bounds.
local function simulate(seen, tokens)
	local flips, spent, first = 0, 0, nil
	for _, m in ipairs(spendOrder(seen)) do
		if tokens < 1 then break end
		local want = m.need
		if want < 1 or want > tokens then want = tokens end
		if want >= m.need and m.need >= 1 then flips = flips + 1 end
		tokens = tokens - want
		spent = spent + want
		first = first or m.id
	end
	return flips, spent, tokens, first
end

-- The live board: civvis-20260803T191900Z, turn 231. Four met city-states, all
-- held by rivals, 56 envoys in hand. Needs are (most_envoys + 1) - mine.
local live = {
	{ id = 6,  mine = 0, need = 14, ours = false, takes = true },  -- Bologna, Kongo
	{ id = 7,  mine = 1, need = 13, ours = false, takes = true },  -- Kandy, Phoenicia
	{ id = 8,  mine = 5, need = 7,  ours = false, takes = true },  -- Kumasi, Phoenicia
	{ id = 12, mine = 1, need = 7,  ours = false, takes = true },  -- Akkad, Norway
}

local flips, spent, leftover, first = simulate(live, 56)
check("live board: suzerainties bought", flips, 4)
check("live board: envoys spent", spent, 41)
check("live board: envoys left over", leftover, 15)
-- Cheapest first, and Kumasi (5 invested) outranks Akkad at the same need.
check("live board: first target is Kumasi", first, 8)

-- The delta this change exists for, measured rather than asserted. The old loop
-- picked one `best` and ran `for _ = 1, tokens` on it, so it bought one flip and
-- sank the rest into a minor it already led.
local function simulateOldLoop(seen, tokens)
	local best
	for _, m in ipairs(seen) do
		if m.takes and not m.ours then
			if best == nil or m.need < best.need
				or (m.need == best.need and m.mine > best.mine) then
				best = m
			end
		end
	end
	if best == nil then return 0, 0 end
	local flipped = tokens >= best.need and 1 or 0
	return flipped, tokens, tokens - best.need   -- flips, spent, wasted
end

local oldFlips, _, wasted = simulateOldLoop(live, 56)
check("old loop bought", oldFlips, 1)
check("old loop wasted", wasted, 49)
check("new loop buys more", flips - oldFlips, 3)

-- A purse that covers one flip and no more must still buy that one.
check("tight purse buys the cheapest flip", (simulate(live, 7)), 1)

-- A purse below every flip price buys nothing but must not hoard: the tokens
-- go into the cheapest partial claim rather than expiring with the game.
local f2, s2, l2 = simulate(live, 3)
check("short purse: flips", f2, 0)
check("short purse: still spends", s2, 3)
check("short purse: nothing hoarded", l2, 0)

-- Minors we already hold are not re-bought, and illegal targets are skipped.
local mixed = {
	{ id = 1, mine = 9, need = 0, ours = true,  takes = true },
	{ id = 2, mine = 0, need = 3, ours = false, takes = false },
	{ id = 3, mine = 2, need = 2, ours = false, takes = true },
}
local f3, s3 = simulate(mixed, 10)
check("held and illegal minors skipped: flips", f3, 1)
check("held and illegal minors skipped: spent", s3, 2)

-- Determinism: equal need and equal investment resolve by id, so the order does
-- not drift with survey order between turns.
local tied = {
	{ id = 9, mine = 2, need = 4, ours = false, takes = true },
	{ id = 4, mine = 2, need = 4, ours = false, takes = true },
}
check("ties resolve by id", spendOrder(tied)[1].id, 4)

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("\nall envoy spend-order checks passed")
