-- The mirror keeps up (CivvisBoard.mirrorHold): a played turn does not end
-- until the CIVVIS page has painted it, so Civilization VI never runs more
-- than one turn ahead of the page. The painted turn arrives as the `mirror`
-- row, read on the pause's quarter-second poll; no row is no wait, and a wait
-- is capped so a page that cannot paint never stalls the lane.
-- Run: lua5.1 tools/civ6_control/mod/mirror_sync_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."
local handle = assert(io.open(here .. "/CivvisControlAgent.lua", "r"))
local source = handle:read("*a")
handle:close()

local holdBlock = assert(source:match("(CivvisBoard%.hold = .-\nend\n)"),
	"the operator hold is defined on CivvisBoard")
local mirrorBlock = assert(source:match(
	"(CivvisBoard%.mirror = .-\nfunction CivvisBoard%.mirrorHold%(turn%).-\nend\n)"),
	"the mirror hold and its peek are defined on CivvisBoard")

local clock, attached, sent, shown = 0, true, false, nil
local peeks = 0
local events = {}
local env = {
	string = string, math = math, type = type, tonumber = tonumber,
	ipairs = ipairs, pcall = pcall,
	CivvisBoard = {},
	CivvisClock = { now = function() return clock end },
	cfg = { RunTag = "civvis-test", CivvisDecides = true },
	awaiting = { turn = 7, done = true, frame = 0 },
	attachOrders = function() return attached end,
	sqlSafe = function(s) return s end,
	try = function(f, default)
		local ok, value = pcall(f)
		if ok then return value end
		return default
	end,
	UI = { HasSentTurnComplete = function() return sent end },
	ExposedMembers = {},
	DB = { Query = function(sql)
		assert(sql:find("run = 'civvis-test'", 1, true), sql)
		if sql:find("kind = 'mirror'", 1, true) then
			peeks = peeks + 1
		else
			assert(sql:find("kind IN ('pause', 'pace', 'mirror')", 1, true), sql)
		end
		if shown == nil then return {} end
		return { { kind = "mirror", x = shown } }
	end },
	emit = function(kind, payload) events[#events + 1] = { kind = kind, payload = payload } end,
}
for _, block in ipairs({ holdBlock, mirrorBlock }) do
	local chunk = assert(loadstring(block))
	setfenv(chunk, env)
	chunk()
end
local board = env.CivvisBoard
local function poll(turn) assert(board.operatorHold(turn) == false) end
local function kinds()
	local out = {}
	for _, e in ipairs(events) do out[#out + 1] = e.kind end
	return table.concat(out, ",")
end

-- 1. No row: the turn ends at once, and nothing is said.
poll(7)
assert(board.hold.mirrorTurn == nil)
assert(board.mirrorHold(7) == false and board.hold.mirrorWaiting == false)
assert(#events == 0)

-- 2. A page already showing this turn does not hold it.
shown = 7
clock = 0.3
poll(7)
assert(board.hold.mirrorTurn == 7, "the painted turn rides the pause's poll")
assert(board.mirrorHold(7) == false and #events == 0)

-- 3. A page a turn behind holds the played turn, says so once, and the peek
-- pulse is told to keep ticking.
clock = 1
env.awaiting.turn = 8
poll(8)
assert(board.mirrorHold(8) == true and board.hold.mirrorWaiting == true)
assert(kinds() == "mirror_wait")
assert(events[1].payload.turn == 8 and events[1].payload.shown == 7)
clock = 1.2
assert(board.mirrorHold(8) == true and #events == 1, "said once per turn")

-- 4. The page paints the turn: the next poll releases it, with the wait.
shown = 8
clock = 1.3
poll(8)
assert(board.mirrorHold(8) == false and board.hold.mirrorWaiting == false)
assert(kinds() == "mirror_wait,mirror_caught_up")
assert(events[2].payload.waited_ms == 300 and events[2].payload.shown == 8)
assert(board.mirrorHold(8) == false and #events == 2, "released once")

-- 5. A page that cannot paint: the turn ends at the cap, once, and the rest of
-- that turn does not wait again.
clock = 2
env.awaiting.turn = 9
poll(9)
assert(board.mirrorHold(9) == true)
clock = 11.9
assert(board.mirrorHold(9) == true)
clock = 12
assert(board.mirrorHold(9) == false and board.hold.mirrorWaiting == false)
assert(kinds() == "mirror_wait,mirror_caught_up,mirror_wait,mirror_wait_capped")
assert(events[4].payload.waited_ms == 10000 and events[4].payload.turn == 9)
clock = 12.5
assert(board.mirrorHold(9) == false and #events == 4)
-- The next turn waits afresh.
clock = 13
env.awaiting.turn = 10
assert(board.mirrorHold(10) == true and events[5].kind == "mirror_wait")

-- 6. A turn whose end is already sent ends whatever the page says.
sent = true
assert(board.mirrorHold(10) == false and board.hold.mirrorWaiting == false)
sent = false

-- 7. The row going away (the harness gave up on the page) lifts the wait.
shown = nil
clock = 13.5
poll(10)
assert(board.hold.mirrorTurn == nil)
assert(board.mirrorHold(10) == false)
assert(events[#events].kind == "mirror_caught_up" and events[#events].payload.shown == nil)

-- 8. A channel that cannot be read is a game that plays.
shown, attached = 3, false
clock = 14
env.awaiting.turn = 11
poll(11)
assert(board.hold.mirrorTurn == nil and board.mirrorHold(11) == false)
attached = true

-- 9. A waiting turn peeks at the row itself, at most every 50 ms: the page
-- painting is seen without waiting for the pause's quarter-second poll.
shown = 11
clock = 20
env.awaiting.turn = 12
poll(12)
assert(board.mirrorHold(12) == true)
local before = peeks
clock = 20.02
assert(board.mirrorHold(12) == true)
assert(peeks == before + 1, "the first waiting tick peeks")
clock = 20.04
assert(board.mirrorHold(12) == true and peeks == before + 1, "not again inside 50 ms")
shown = 12
clock = 20.08
assert(board.mirrorHold(12) == false, "seen on the next peek, between pause polls")
assert(events[#events].kind == "mirror_caught_up" and events[#events].payload.waited_ms == 80)
clock = 20.2
assert(board.mirrorHold(12) == false and peeks == before + 2, "a released turn stops peeking")
-- A failed peek keeps what the pause poll last read.
shown = 12
clock = 21
env.awaiting.turn = 13
assert(board.mirrorHold(13) == true)
local query = env.DB.Query
env.DB.Query = function() error("locked") end
clock = 21.1
assert(board.mirrorHold(13) == true and board.hold.mirrorTurn == 12)
env.DB.Query = query

-- The wiring: the wait sits after the turn is settled (the moves play while
-- the page catches up) and before any end-turn request; the peek pulse ticks a
-- waiting turn, once per wait, ahead of the landed-orders check.
local tick = assert(source:match("\nlocal function tick%(%)\n(.-)\nend\n"))
local settleAt = tick:find("not settleTurn(player, pid, turn, playTurn)", 1, true)
local repairAt = tick:find("CivvisFrames.repairProduction(player, pid, turn)", 1, true)
local mirrorAt = tick:find("if CivvisBoard.mirrorHold(turn) then return; end", 1, true)
local endAt = tick:find("CivvisQueue.requestEndTurn(", 1, true)
local blockerAt = tick:find("local blocker = currentBlocker(pid);", 1, true)
assert(settleAt and repairAt and mirrorAt and endAt and blockerAt)
assert(settleAt < mirrorAt and repairAt < mirrorAt, "the turn is played first")
assert(mirrorAt < blockerAt and mirrorAt < endAt,
	"no blocker is answered or end requested while the page catches up")
local peek = assert(source:match("CivvisQueue%.onPeekPulse = function%(%)(.-)\nend;"))
local heldAt = peek:find("if CivvisBoard.hold.held then return; end", 1, true)
local waitAt = peek:find("if CivvisBoard.hold.mirrorWaiting then", 1, true)
local clearAt = peek:find("CivvisBoard.hold.mirrorWaiting = false;", 1, true)
local landedAt = peek:find("if not CivvisQueue.ordersLanded() then return; end", 1, true)
assert(heldAt and waitAt and clearAt and landedAt)
assert(heldAt < waitAt and waitAt < clearAt and clearAt < landedAt)

print("mirror_sync_test: ok")
