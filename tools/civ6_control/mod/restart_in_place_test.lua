-- New game by restart (tools/civ6_control/restart_handoff.py): a retire row
-- with `x = 1` parks the game instead of retiring it, and the parked agent
-- answers the NEXT run's `restart` row from the HUD pulse with a fresh game.
-- Run: lua5.1 tools/civ6_control/mod/restart_in_place_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."
local handle = assert(io.open(here .. "/CivvisControlAgent.lua", "r"))
local source = handle:read("*a")
handle:close()

local block = assert(source:match(
	"(CivvisBoard%.restart = {.-function CivvisBoard%.restartPoll%(%).-\nend\n)"),
	"the restart in place is defined on CivvisBoard")

local rows, attached, regenerates = {}, true, true
local seeds = { map = 11, game = 22 }
local calls, events, queries = {}, {}, {}
local env = {
	string = string, tonumber = tonumber, ipairs = ipairs, pcall = pcall,
	CivvisBoard = {},
	CivvisQueue = { resetTimescale = function(why) calls[#calls + 1] = "timescale:" .. why end },
	cfg = { RunTag = "civvis-old" },
	finished = false,
	attachOrders = function() return attached end,
	sqlSafe = function(s) return s end,
	try = function(f, default)
		local ok, value = pcall(f)
		if ok then return value end
		return default
	end,
	DB = { Query = function(sql)
		queries[#queries + 1] = sql
		assert(sql:find("kind = 'restart'", 1, true), sql)
		assert(sql:find("run = 'civvis-old'", 1, true), sql)
		return rows
	end },
	MapConfiguration = { GetValue = function(key)
		assert(key == "RANDOM_SEED", key)
		return seeds.map
	end },
	GameConfiguration = {
		GetValue = function(key)
			assert(key == "GAME_SYNC_RANDOM_SEED", key)
			return seeds.game
		end,
		RegenerateSeeds = function()
			if not regenerates then error("not in this context") end
			seeds.map, seeds.game = seeds.map + 100, seeds.game + 100
			calls[#calls + 1] = "regenerate"
		end,
		SetValue = function(key, value) calls[#calls + 1] = "set:" .. key .. "=" .. tostring(value) end,
	},
	Network = {
		RestartGame = function() calls[#calls + 1] = "RestartGame" end,
		HostGame = function(kind) calls[#calls + 1] = "HostGame:" .. tostring(kind) end,
	},
	ServerType = { SERVER_TYPE_NONE = 0 },
	emit = function(kind, payload) events[#events + 1] = { kind = kind, payload = payload } end,
}
local chunk = assert(loadstring(block))
setfenv(chunk, env)
chunk()
local board = env.CivvisBoard
local function kinds()
	local out = {}
	for _, e in ipairs(events) do out[#out + 1] = e.kind end
	return table.concat(out, ",")
end

-- 1. A fresh game is neither parked nor restarting.
assert(board.restart.parked == false and board.restart.issued == false)

-- 2. Parking stops play and files the run as an operator retirement, with a
-- record right behind it so the retirement reaches the log at once.
board.parkForRestart(42)
assert(env.finished == true, "a parked game plays no more")
assert(kinds() == "retired,restart_parked", kinds())
assert(events[1].payload.why == "restart" and events[1].payload.parked == true
	and events[1].payload.turn == 42)
assert(calls[1] == "timescale:restart")
board.parkForRestart(42)
assert(#events == 2, "parking is answered once")

-- 3. Parked with no order yet: poll quietly.
board.restartPoll()
assert(#events == 2 and #queries == 1 and #calls == 1)

-- 4. A channel that cannot be read asks nothing.
attached = false
board.restartPoll()
assert(#queries == 1)
attached = true

-- 5. The next run's order: new seeds, then the stock Restart, once.
rows = { { x = 1 } }
board.restartPoll()
assert(table.concat(calls, ",") == "timescale:restart,regenerate,RestartGame",
	table.concat(calls, ","))
assert(kinds() == "retired,restart_parked,restart_issued,restart_issued_flush," ..
	"restart_called,restart_called_flush", kinds())
local issued = events[3].payload
assert(issued.mode == 1 and issued.regenerated == true)
assert(issued.map_seed_before == 11 and issued.map_seed == 111)
assert(issued.game_seed_before == 22 and issued.game_seed == 122)
assert(events[5].payload.called == true)
board.restartPoll()
assert(#queries == 2 and #events == 6, "a restart is issued once")

-- 6. Mode 2 hosts the current configuration again, marked, never after a leave.
board.restart.issued = false
calls, events = {}, {}
rows = { { x = 2 } }
board.restartPoll()
assert(table.concat(calls, ",") == "regenerate,set:CIVVIS_SETUP=restart,HostGame:0",
	table.concat(calls, ","))

-- 7. Mode 3 is the stock Restart, seeds untouched; a refused regeneration is
-- reported, not fatal.
board.restart.issued = false
calls, events = {}, {}
rows = { { x = 3 } }
board.restartPoll()
assert(table.concat(calls, ",") == "RestartGame", table.concat(calls, ","))
assert(events[1].payload.regenerated == false
	and events[1].payload.map_seed == events[1].payload.map_seed_before)
board.restart.issued = false
calls, events = {}, {}
regenerates = false
rows = { { x = 1 } }
board.restartPoll()
assert(table.concat(calls, ",") == "RestartGame", table.concat(calls, ","))
assert(events[1].payload.regenerated == false)

-- 8. The wiring: the retire row's `x = 1` parks instead of retiring, and the
-- HUD pulse polls a parked game before anything else.
assert(source:find('"SELECT x FROM civvis.orders WHERE run = \'%s\' " ..\n' ..
	'\t\t\t\t\t"AND kind = \'retire\'"', 1, true), "the retire poll reads x")
assert(source:find("restart = restart or tonumber(row.x) == 1;", 1, true))
assert(source:find("if restart then\n\t\t\t\t\tCivvisBoard.parkForRestart(pollTurn);\n" ..
	"\t\t\t\t\treturn;\n\t\t\t\tend", 1, true), "a restart retire parks")
assert(source:find("CivvisQueue.onUiPulse = function(source)\n" ..
	"\t-- A game parked for a restart in place listens for the next run's order.\n" ..
	"\tif CivvisBoard.restart.parked then pcall(CivvisBoard.restartPoll); return; end", 1, true),
	"the HUD pulse polls a parked game")
assert(source:find('map_seed = try(function() return MapConfiguration.GetValue("RANDOM_SEED"); end),',
	1, true), "the seat reports the map seed")

print("restart in place: all checks passed")
