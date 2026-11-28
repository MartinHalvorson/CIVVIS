-- "Just One More Turn" in a person's hotseat game: the seat a victory stopped
-- plays on (CivvisControlAgent.lua `CivvisSeat.oneMoreTurn`), a defeated seat
-- does not, and the ladder's game stays over at its victory.
-- The end screen raises it: Base/Assets/UI/EndGame/EndGameMenu.lua:349-351
-- `function OnBack() Close(); LuaEvents.EndGameMenu_OneMoreTurn(); end`.
-- Run: lua5.1 tools/civ6_control/mod/one_more_turn_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."
local handle = assert(io.open(here .. "/CivvisControlAgent.lua", "r"))
local source = handle:read("*a")
handle:close()
local block = assert(source:match(
	"\n(local function onTeamVictory%(.-\nfunction CivvisSeat%.oneMoreTurn%(%)\n.-\nend\n)"),
	"the victory, the defeat and the one-more-turn handlers are where they were")

-- One seat's context: its config, its `finished`, and what it wrote.
local function context(cfg)
	local rows = {}
	local env = setmetatable({
		cfg = cfg, finished = false, turnsPlayed = 9, CivvisSeat = { seat = 1 },
		try = function(fn, fallback) local ok, v = pcall(fn); if ok then return v; end return fallback; end,
		Game = { GetLocalPlayer = function() return 1; end, GetCurrentGameTurn = function() return 140; end },
		Players = { [1] = { GetTeam = function() return 1; end, GetScore = function() return 300; end } },
		GameInfo = { Victories = { [2] = { VictoryType = "VICTORY_TECHNOLOGY" } } },
		CivvisQueue = { resetTimescale = function() end },
		exportState = function() end,
		emit = function(kind, row) rows[#rows + 1] = { kind = kind, row = row }; end,
	}, { __index = _G })
	local chunk = assert(loadstring(block .. "\nreturn onTeamVictory, onPlayerDefeat", "one_more_turn"))
	setfenv(chunk, env)
	local victory, defeat = chunk()
	env.victory, env.defeat, env.rows = victory, defeat, rows
	env.kinds = function(kind)
		local n = 0
		for _, r in ipairs(rows) do if r.kind == kind then n = n + 1 end end
		return n
	end
	return env
end
local HOTSEAT = { Hotseat = true, HotseatSeat = 1 }

-- The seat the victory stopped plays on after one more turn, and says so.
do
	local c = context(HOTSEAT)
	c.victory(0, 2, 7)
	assert(c.finished == true, "a victory stops the controller while the end screen is up")
	c.CivvisSeat.oneMoreTurn()
	assert(c.finished == false, "one more turn: this seat plays again")
	assert(c.kinds("one_more_turn") == 1)
	local said = c.rows[#c.rows].row
	assert(said.turn == 140 and said.local_player == 1, "the record says when and whose")
	-- Once: a second press (another victory's screen) without a new victory does nothing.
	c.finished = true
	c.CivvisSeat.oneMoreTurn()
	assert(c.finished == true and c.kinds("one_more_turn") == 1)
end
-- A seat defeated stays stopped, whichever event came first.
do
	local c = context(HOTSEAT)
	c.victory(0, 2, 7)
	c.defeat(1, 3, 8)
	c.CivvisSeat.oneMoreTurn()
	assert(c.finished == true and c.kinds("one_more_turn") == 0, "victory, then our defeat: stopped")
	c = context(HOTSEAT)
	c.defeat(1, 3, 8)
	c.victory(0, 2, 7)
	c.CivvisSeat.oneMoreTurn()
	assert(c.finished == true and c.kinds("one_more_turn") == 0, "our defeat, then a victory: stopped")
	-- Another player's defeat changes nothing.
	c = context(HOTSEAT)
	c.victory(0, 2, 7)
	c.defeat(4, 3, 8)
	c.CivvisSeat.oneMoreTurn()
	assert(c.finished == false, "a rival's defeat does not keep this seat stopped")
end
-- Stopped for another reason ("no cities remain" sets `finished` too): not resumed.
do
	local c = context(HOTSEAT)
	c.finished = true
	c.victory(0, 2, 7)
	c.CivvisSeat.oneMoreTurn()
	assert(c.finished == true, "a seat already stopped is not started by a victory's screen")
end
-- The ladder (no Hotseat key): over at the victory, as before.
do
	for _, cfg in ipairs({ {}, { Hotseat = false } }) do
		local c = context(cfg)
		c.victory(0, 2, 7)
		c.CivvisSeat.oneMoreTurn()
		assert(c.finished == true and c.kinds("one_more_turn") == 0, "the ladder's game stays over")
		assert(c.CivvisSeat.victoryStop == nil, "nothing of the hotseat's is kept on the ladder")
	end
end
-- Wired for hotseat only, and not through the seat gate: the context the
-- victory stopped answers whoever is local when the button is pressed.
assert(source:find("\tif cfg.Hotseat == true then\n\t\tpcall(function() LuaEvents.EndGameMenu_OneMoreTurn.Add("
	.. "CivvisSeat.oneMoreTurn); end);\n\tend\n", 1, true), "Initialize listens for one more turn in hotseat")

print("one_more_turn_test: ok")
