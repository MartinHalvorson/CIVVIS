-- The operator's Pause (tools/civ6_control/live_pause.py): the seat holds on
-- the first tick of our turn that sees a `pause` row in the orders channel —
-- mid-turn included — polls that row on the UI clock rather than per tick,
-- tells the mod's other contexts through ExposedMembers, and resumes when the
-- row is gone. Run: lua5.1 tools/civ6_control/mod/operator_pause_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."
local handle = assert(io.open(here .. "/CivvisControlAgent.lua", "r"))
local source = handle:read("*a")
handle:close()

local block = assert(source:match("(CivvisBoard%.hold = .-\nend\n)"),
	"the operator hold is defined on CivvisBoard")

local clock, rows, queries, attached, sent, pace = 0, 0, 0, true, false, 0
local events = {}
local env = {
	string = string, math = math, type = type, tonumber = tonumber,
	ipairs = ipairs, pcall = pcall,
	CivvisBoard = {},
	CivvisClock = { now = function() return clock end },
	cfg = { RunTag = "civvis-test", CivvisDecides = true },
	awaiting = { turn = 7, done = false, frame = 0 },
	attachOrders = function() return attached end,
	sqlSafe = function(s) return s end,
	try = function(f, default)
		local ok, value = pcall(f)
		if ok then return value end
		return default
	end,
	UI = { HasSentTurnComplete = function() return sent end },
	ExposedMembers = { CivvisOperatorHold = true },
	DB = { Query = function(sql)
		queries = queries + 1
		assert(sql:find("kind IN ('pause', 'pace')", 1, true), sql)
		assert(sql:find("run = 'civvis-test'", 1, true), sql)
		local out = {}
		if rows > 0 then out[#out + 1] = { kind = "pause" } end
		if pace > 0 then out[#out + 1] = { kind = "pace", x = pace } end
		return out
	end },
	emit = function(kind, payload) events[#events + 1] = { kind = kind, payload = payload } end,
}
local chunk = assert(loadstring(block))
setfenv(chunk, env)
chunk()
local hold = env.CivvisBoard.operatorHold
local shared = env.ExposedMembers
local function kinds()
	local out = {}
	for _, e in ipairs(events) do out[#out + 1] = e.kind end
	return table.concat(out, ",")
end
assert(shared.CivvisOperatorHold == false, "a fresh game starts unheld")

-- 1. No request: play on, and say nothing.
assert(hold(7) == false)
assert(#events == 0 and queries == 1)

-- 2. The request is polled at most four times a second, never per tick.
rows = 1
for _ = 1, 50 do assert(hold(7) == false) end
assert(queries == 1, "polled " .. queries .. " times inside a quarter second")

-- 3. A press mid-turn holds on the next poll, wherever the turn stands: here a
-- replan frame whose orders are in and draining.
clock = 0.3
env.awaiting.done, env.awaiting.frame = true, 2
assert(hold(7) == true)
-- The held line right behind it is what gets the paused one written out.
assert(kinds() == "operator_paused,operator_pause_held")
assert(events[1].payload.turn == 7 and events[1].payload.frame == 2
	and events[1].payload.settled == true)
assert(events[2].payload.held_s == 0)
assert(shared.CivvisOperatorHold == true, "the camera and screen closer are told")
for _ = 1, 20 do assert(hold(7) == true) end
assert(#events == 2, "a held game repeats nothing inside 30 s")

-- 4. Held, it stays held and reports every 30 s.
clock = 30.3
assert(hold(7) == true)
assert(kinds() == "operator_paused,operator_pause_held,operator_pause_held")
assert(events[3].payload.held_s == 30)

-- 5. Lifting the request resumes on the next poll, with the time held.
rows = 0
clock = 30.4
assert(hold(7) == true, "not re-polled within a quarter second")
clock = 30.6
assert(hold(7) == false)
assert(kinds() == "operator_paused,operator_pause_held,operator_pause_held,operator_resumed")
assert(events[4].payload.held_s == 30 and events[4].payload.turn == 7)
assert(shared.CivvisOperatorHold == false, "the camera and screen closer get the game back")
assert(hold(7) == false and #events == 4)

-- 6. A turn whose end is already sent ends whatever this says; the next
-- opening holds, once its board is out.
rows, sent = 1, true
clock = 31
assert(hold(7) == false and #events == 4)
sent = false
env.awaiting.turn, env.awaiting.done, env.awaiting.frame = 8, false, 0
assert(hold(7) == false, "the board for this turn is not out yet")
assert(hold(8) == true)
assert(events[5].kind == "operator_paused" and events[5].payload.turn == 8
	and events[5].payload.frame == 0 and events[5].payload.settled == false)
rows = 0
clock = 32
assert(hold(8) == false and events[7].kind == "operator_resumed")

-- 7. A channel that cannot be read is a game that plays.
rows, attached = 1, false
clock = 40
env.awaiting.turn, env.awaiting.done, env.awaiting.frame = 9, false, 0
assert(hold(9) == false and #events == 7)
attached = true

-- 8. A seat the Lua plays itself has no orders to wait for: any tick holds.
env.cfg.CivvisDecides = false
env.awaiting.done = true
clock = 41
assert(hold(9) == true)
assert(events[8].kind == "operator_paused")
env.cfg.CivvisDecides = true

-- 9. The watch pace rides the same poll.
env.cfg.CivvisDecides = true
rows, pace = 0, 1500
clock = 50
assert(hold(9) == false)
assert(env.CivvisBoard.hold.paceMs == 1500, "the pace row is read with the pause")
pace = 0
clock = 51
hold(9)
assert(env.CivvisBoard.hold.paceMs == 0, "no row is Lightning")

-- 10. Each turn's opening waits until the previous opening is the pace ago,
-- says so once, and lets the moves run at speed once they begin.
local paceBlock = assert(source:match("(function CivvisBoard%.paceHold%(turn%).-\nend\n)"),
	"the watch pace is defined on CivvisBoard")
local paceChunk = assert(loadstring(paceBlock))
setfenv(paceChunk, env)
paceChunk()
local paceHold = env.CivvisBoard.paceHold
local board = env.CivvisBoard.hold
board.paceMs = 2000
clock = 100
env.awaiting.turn, env.awaiting.done, env.awaiting.frame = 20, false, 0
assert(paceHold(20) == false, "the first opening has nothing to wait on")
env.awaiting.done = true
clock = 101.2
env.awaiting.turn, env.awaiting.done = 21, false
local before = #events
assert(paceHold(21) == true, "1.2 s after the last opening, a 2 s pace waits")
assert(board.pacing == true)
assert(events[#events].kind == "pace_hold" and events[#events].payload.wait_ms == 800
	and events[#events].payload.pace_ms == 2000)
clock = 101.9
assert(paceHold(21) == true and #events == before + 1, "said once per turn")
clock = 102.05
assert(paceHold(21) == false and board.pacing == false, "released at the budget")
clock = 106
env.awaiting.turn = 22
assert(paceHold(22) == false, "a turn slower than the pace does not wait")
clock = 106.5
env.awaiting.turn, env.awaiting.done = 23, true
assert(paceHold(23) == false, "moves already begun are not held back")
board.paceMs = 0
clock = 106.6
env.awaiting.turn, env.awaiting.done = 24, false
assert(paceHold(24) == false, "Lightning never waits")

-- The wiring: the tick holds after the board goes out and before any of it is
-- applied; the held game's UI pulse ticks without calling itself a wake, and
-- landed orders do not wake it either.
local tick = assert(source:match("\nlocal function tick%(%)\n(.-)\nend\n"))
local opening = tick:find("beginTurn(player, pid, turn);", 1, true)
local holdAt = tick:find("if CivvisBoard.operatorHold(turn) then return; end", 1, true)
local settleAt = tick:find("not settleTurn(player, pid, turn, playTurn)", 1, true)
assert(opening and holdAt and settleAt and opening < holdAt and holdAt < settleAt,
	"the hold sits between the board going out and the turn being settled")
local paceAt = tick:find("if CivvisBoard.paceHold(turn) then return; end", 1, true)
assert(paceAt and holdAt < paceAt and paceAt < settleAt,
	"the pace waits after the pause and before the turn is settled")
local pulse = assert(source:match("CivvisQueue%.onUiPulse = function%(source%)(.-)\nend;"))
local heldAt = pulse:find("if CivvisBoard.hold.held then", 1, true)
local wakeAt = pulse:find('emit("controller_wake"', 1, true)
assert(heldAt and wakeAt and heldAt < wakeAt)
local peek = assert(source:match("CivvisQueue%.onPeekPulse = function%(%)(.-)\nend;"))
local peekHeld = peek:find("if CivvisBoard.hold.held then return; end", 1, true)
local peekWake = peek:find('emit("orders_peek_wake"', 1, true)
assert(peekHeld and peekWake and peekHeld < peekWake)
local peekPacing = peek:find("if CivvisBoard.hold.pacing then tick(); return; end", 1, true)
assert(peekPacing and peekPacing < peekWake, "a paced turn ticks without a wake per peek")

-- A game retired while paused is over: its screens go back to the closer.
local retireAt = tick:find("CivvisBoard.retireAsked = true;", 1, true)
local releaseAt = tick:find("ExposedMembers.CivvisOperatorHold = false;", retireAt or 1, true)
local requestAt = tick:find("UI.RequestAction(action);", retireAt or 1, true)
assert(retireAt and releaseAt and requestAt and releaseAt < requestAt,
	"a retire releases the operator's hold on the screens")

print("operator pause: all checks passed")
