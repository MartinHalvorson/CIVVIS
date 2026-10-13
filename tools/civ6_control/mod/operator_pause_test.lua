-- The operator's Pause (tools/civ6_control/live_pause.py): the seat holds at
-- the opening of our turn while a `pause` row stands in the orders channel,
-- polls that row on the UI clock rather than per tick, and resumes when it is
-- gone. Run: lua5.1 tools/civ6_control/mod/operator_pause_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."
local handle = assert(io.open(here .. "/CivvisControlAgent.lua", "r"))
local source = handle:read("*a")
handle:close()

local block = assert(source:match("(CivvisBoard%.hold = .-\nend\n)"),
	"the operator hold is defined on CivvisBoard")

local clock, rows, queries, attached = 0, 0, 0, true
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
	DB = { Query = function(sql)
		queries = queries + 1
		assert(sql:find("kind = 'pause'", 1, true), sql)
		assert(sql:find("run = 'civvis-test'", 1, true), sql)
		return { { n = rows } }
	end },
	emit = function(kind, payload) events[#events + 1] = { kind = kind, payload = payload } end,
}
local chunk = assert(loadstring(block))
setfenv(chunk, env)
chunk()
local hold = env.CivvisBoard.operatorHold
local function kinds()
	local out = {}
	for _, e in ipairs(events) do out[#out + 1] = e.kind end
	return table.concat(out, ",")
end

-- 1. No request: play on, and say nothing.
assert(hold(7) == false)
assert(#events == 0 and queries == 1)

-- 2. The request is polled at most once a second, never per tick.
rows = 1
for _ = 1, 50 do assert(hold(7) == false) end
assert(queries == 1, "polled " .. queries .. " times inside one second")

-- 3. Asked while the turn is already being played: finish it first.
clock = 1.5
env.awaiting.done = true
assert(hold(7) == false and #events == 0)
env.awaiting.done = false
env.awaiting.frame = 2
assert(hold(7) == false and #events == 0, "a combat frame is not the opening")

-- 4. The next opening holds, and says so once.
env.awaiting.turn, env.awaiting.frame = 8, 0
assert(hold(7) == false, "the board for this turn is not out yet")
assert(hold(8) == true)
assert(kinds() == "operator_paused" and events[1].payload.turn == 8)
for _ = 1, 20 do assert(hold(8) == true) end
assert(#events == 1, "a held game repeats nothing inside 30 s")

-- 5. Held, it stays held at any point of the turn and reports every 30 s.
env.awaiting.done = true
clock = 31.6
assert(hold(8) == true)
assert(kinds() == "operator_paused,operator_pause_held")
assert(events[2].payload.held_s == 30)

-- 6. Lifting the request resumes on the next poll, with the time held.
rows = 0
clock = 32.0
assert(hold(8) == true, "not re-polled within a second")
clock = 33.0
assert(hold(8) == false)
assert(kinds() == "operator_paused,operator_pause_held,operator_resumed")
assert(events[3].payload.held_s == 32 and events[3].payload.turn == 8)
assert(hold(8) == false and #events == 3)

-- 7. A channel that cannot be read is a game that plays.
rows, attached = 1, false
clock = 40
env.awaiting.turn, env.awaiting.done, env.awaiting.frame = 9, false, 0
assert(hold(9) == false and #events == 3)
attached = true

-- 8. A seat the Lua plays itself has no orders to wait for: any tick holds.
env.cfg.CivvisDecides = false
env.awaiting.done = true
clock = 41
assert(hold(9) == true)
assert(events[4].kind == "operator_paused")
env.cfg.CivvisDecides = true

-- The wiring: the tick holds after the board goes out and before any of it is
-- applied; the held game's UI pulse ticks without calling itself a wake, and
-- landed orders do not wake it either.
local tick = assert(source:match("\nlocal function tick%(%)\n(.-)\nend\n"))
local opening = tick:find("beginTurn(player, pid, turn);", 1, true)
local holdAt = tick:find("if CivvisBoard.operatorHold(turn) then return; end", 1, true)
local settleAt = tick:find("not settleTurn(player, pid, turn, playTurn)", 1, true)
assert(opening and holdAt and settleAt and opening < holdAt and holdAt < settleAt,
	"the hold sits between the board going out and the turn being settled")
local pulse = assert(source:match("CivvisQueue%.onUiPulse = function%(source%)(.-)\nend;"))
local heldAt = pulse:find("if CivvisBoard.hold.held then", 1, true)
local wakeAt = pulse:find('emit("controller_wake"', 1, true)
assert(heldAt and wakeAt and heldAt < wakeAt)
local peek = assert(source:match("CivvisQueue%.onPeekPulse = function%(%)(.-)\nend;"))
local peekHeld = peek:find("if CivvisBoard.hold.held then return; end", 1, true)
local peekWake = peek:find('emit("orders_peek_wake"', 1, true)
assert(peekHeld and peekWake and peekHeld < peekWake)

print("operator pause: all checks passed")
