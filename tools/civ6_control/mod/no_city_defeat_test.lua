-- "An empire with no cities is defeated" (CivvisControlAgent.lua) is about
-- LOSING them: a seat that has not founded its first city is still playing.
-- Live 2026-11-27, a hotseat check game: a CIVVIS seat drew Kupe (starts at
-- sea), held its Settler and Warrior on turn 6 with no city, was written off as
-- defeated, and its turn then stood 45 s every round.
-- Run: lua5.1 tools/civ6_control/mod/no_city_defeat_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."
local handle = assert(io.open(here .. "/CivvisControlAgent.lua", "r"))
local source = handle:read("*a")
handle:close()
local block = assert(source:match("\n(\tif not defeatReported then\n\t\tlocal alive = 0;.-\n\tend\n)"),
	"the no-cities rule is where it was")
assert(block:find('reason = "no cities remain"', 1, true))

-- One seat's game: its cities, its units, and what the rule wrote.
local function seat(founded)
	local world = { cities = 0, units = {}, turn = 1, written = {} }
	local env = {
		defeatReported = false, finished = false, pid = 1, player = {},
		CivvisSeat = { founded = founded },
		GameInfo = { Units = { SETTLER = { FoundCity = true }, WARRIOR = { FoundCity = false },
		                       BUILDER = {} } },
		eachCity = function(_, fn) for i = 1, world.cities do fn({}) end end,
		eachUnit = function(_, fn)
			for _, kind in ipairs(world.units) do fn({ kind = kind }) end
		end,
		-- The agent's own lookup of a unit's type name (`unitTypeName`).
		unitTypeName = function(unit) return unit.kind end,
		emit = function(kind, fields) world.written[#world.written + 1] = { kind = kind, fields = fields } end,
		ipairs = ipairs,
	}
	local chunk = assert(loadstring(block))
	setfenv(chunk, env)
	local function play(turn)
		world.turn, env.turn = turn, turn
		chunk()
		return env
	end
	return world, play, env
end

-- Kupe at sea: no city on turn 6, 20 or 60, Settler in hand: still playing.
do
	local world, play = seat(nil)
	world.units = { "SETTLER", "WARRIOR" }
	for _, turn in ipairs({ 1, 5, 6, 7, 20, 60 }) do
		local env = play(turn)
		assert(#world.written == 0 and env.finished == false and env.defeatReported == false,
			"a seat that can still found is not defeated: turn " .. turn)
	end
	-- It lands and founds: remembered; then it loses that city with a Settler
	-- still in the field: that is a defeat, once.
	world.cities = 1
	local env = play(61)
	assert(env.CivvisSeat.founded == true and #world.written == 0)
	world.cities = 0
	env = play(62)
	assert(#world.written == 1 and world.written[1].kind == "defeat"
		and world.written[1].fields.reason == "no cities remain"
		and world.written[1].fields.turn == 62 and world.written[1].fields.ours == true
		and world.written[1].fields.local_player == 1, "losing the only city is a defeat")
	assert(env.finished == true and env.defeatReported == true)
	play(63)
	assert(#world.written == 1, "written once")
end
-- Its Settler is taken before it ever founds: nothing left to found with.
do
	local world, play = seat(nil)
	world.units = { "WARRIOR", "BUILDER" }
	assert(#world.written == 0 and play(5).finished == false, "not before turn 6, as before")
	local env = play(6)
	assert(#world.written == 1 and env.finished == true, "no city and no Settler: defeated")
end
-- A seat with cities is never touched, early or late; the plain case is as it was.
do
	local world, play = seat(nil)
	world.cities, world.units = 2, { "WARRIOR" }
	for _, turn in ipairs({ 1, 6, 80 }) do assert(play(turn).finished == false) end
	assert(#world.written == 0)
	world.cities, world.units = 0, { "SETTLER" }
	local env = play(81)
	assert(#world.written == 1 and env.finished == true,
		"a seat that had cities and lost them all is defeated, Settler or not")
end
-- A unit the game has no row for does not count as a founder, and breaks nothing.
do
	local world, play = seat(nil)
	world.units = { "UNKNOWN" }
	local env = play(9)
	assert(#world.written == 1 and env.finished == true)
end

print("no_city_defeat_test: ok")
