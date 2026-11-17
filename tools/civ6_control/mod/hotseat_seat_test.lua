-- Hotseat: one agent context per CIVVIS seat, silent outside its own turn, and
-- the hand-over screen pressed on CIVVIS seats only (tools/civ6_hotseat.py).
-- Run: lua5.1 tools/civ6_control/mod/hotseat_seat_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."
local function read(name)
	local handle = assert(io.open(here .. "/" .. name, "r"))
	local text = handle:read("*a")
	handle:close()
	return text
end
local agent = read("CivvisControlAgent.lua")
local closer = read("CivvisControlAutoClose.lua")

-- 1. The seat gate. --------------------------------------------------------
local gateBlock = assert(agent:match("(CivvisSeat = { seat = .-\nfunction CivvisSeat%.gate.-\nend\n)"),
	"the agent defines CivvisSeat.mine and CivvisSeat.gate")
local localPlayer = 0
local paused = false
local function seatEnv(seat)
	local env = {
		cfg = { HotseatSeat = seat },
		tonumber = tonumber, type = type, pcall = pcall,
		Game = { GetLocalPlayer = function() return localPlayer end },
		PlayerConfigurations = setmetatable({}, { __index = function()
			return { GetWantsPause = function() return paused end }
		end }),
	}
	local chunk = assert(loadstring(gateBlock))
	setfenv(chunk, env)
	chunk()
	return env.CivvisSeat
end

-- Single player: no seat, so the gate is the identity and nothing changes.
local solo = seatEnv(nil)
local handler = function() return "ran" end
assert(solo.gate(handler) == handler, "single player keeps every handler as it is")
assert(solo.mine() == true)

-- A seat context acts only while its own seat is the local player.
local seat2 = seatEnv(2)
local calls = 0
local gated = seat2.gate(function(a, b) calls = calls + 1; return a + b end)
assert(gated ~= nil and gated ~= handler)
localPlayer = 0
assert(gated(1, 2) == nil and calls == 0, "the operator's turn reaches no handler")
localPlayer = 1
assert(gated(1, 2) == nil and calls == 0, "another jack seat's turn reaches no handler")
localPlayer = 2
assert(gated(1, 2) == 3 and calls == 1, "its own turn passes arguments and results through")
-- Not while its hand-over screen still holds the game paused.
paused = true
assert(gated(1, 2) == nil and calls == 1, "the hand-over screen is pressed first")
paused = false
assert(gated(1, 2) == 3 and calls == 2)
-- An unreadable local player is not this seat.
local broken = seatEnv(2)
localPlayer = nil
assert(broken.mine() == false)

-- 2. Every entry point of the agent goes through the gate. -----------------
local init = assert(agent:match("\nfunction Initialize%(%)(.-)\nend\n\nInitialize%(%);"),
	"Initialize is the agent's last definition")
local adds = 0
for line in init:gmatch("[^\n]+") do
	if line:find("%.Add%(") and not line:match("^%s*%-%-") then
		adds = adds + 1
		assert(line:find("CivvisSeat.gate(", 1, true),
			"an Initialize registration skips the seat gate: " .. line)
	end
end
assert(adds >= 6, "found " .. adds .. " registrations in Initialize")
assert(agent:find('LuaEvents.CivvisCongressBallot.Add(CivvisSeat.gate(function() castBallot("popup"); end));', 1, true),
	"the congress ballot hook is gated too")
-- A seat context does not replay the single-player load-screen dismissals.
local started = assert(agent:match("(\tCivvisK%.guard%(survey%);.-Events%.LoadScreenClose)"))
assert(started:find("if CivvisSeat.seat ~= nil then", 1, true)
	and started:find("CivvisK.guard(CivvisSeat.claim);", 1, true)
	and started:find("return;", 1, true),
	"a seat claims its name and returns before LoadScreenClose")
-- A second seat shares the first seat's ATTACH of the one orders database.
local attach = assert(agent:match("(local function attachOrders%(%).-\nend\n)"))
assert(attach:find("local shared = not ok and CivvisSeat.seat ~= nil;", 1, true))
assert(attach:find("ordersAttached = (ok or shared) and reads;", 1, true))

-- 3. The hand-over screen. --------------------------------------------------
local screenBlock = assert(closer:match("(CivvisHotseatScreen = { .-\n)if NAME == \"PlayerChange\" then"),
	"AutoClose defines CivvisHotseatScreen before the PlayerChange arm")
local reports, presses, saves = {}, 0, {}
local dialogHidden, turn = false, 10
local env = {
	cfg = { Hotseat = true, HotseatHumanSeats = { 0 }, HotseatSeats = { 1, 2, 3 },
	        HotseatSaveEvery = 5, HotseatSaveName = "hotseat-test" },
	RUN = "hotseat-test",
	tonumber = tonumber, tostring = tostring, type = type, pcall = pcall,
	ipairs = ipairs, string = string,
	report = function(kind, fields) reports[#reports + 1] = { kind = kind, fields = fields or "" } end,
	Game = { GetLocalPlayer = function() return localPlayer end,
	         GetCurrentGameTurn = function() return turn end },
	Controls = { PopupAlphaIn = { IsHidden = function() return dialogHidden end } },
	OnOk = function() presses = presses + 1 end,
	SaveLocations = { LOCAL_STORAGE = 7 },
	Network = { GetGameConfigurationSaveType = function() return 3 end,
	            SaveGame = function(file) saves[#saves + 1] = file end },
	Events = { LocalPlayerTurnBegin = { Add = function() end },
	           LocalPlayerTurnEnd = { Add = function() end } },
	ContextPtr = { SetUpdate = function() end },
}
local chunk = assert(loadstring(screenBlock))
setfenv(chunk, env)
chunk()
local screen = env.CivvisHotseatScreen
local function kinds()
	local out = {}
	for _, r in ipairs(reports) do out[#out + 1] = r.kind end
	return table.concat(out, ",")
end

-- The operator's turn: the screen and every other closer leave it alone.
localPlayer = 0
assert(screen.humanTurn() == true)
for _ = 1, 20 do screen.frame(0.1) end
assert(presses == 0 and #reports == 0, "nothing presses Start Turn for the operator")

-- A jack seat's turn: pressed once its slide-in has had a moment, saved first
-- when the turn is a save turn and this is the first jack seat to play it.
localPlayer = 1
assert(screen.humanTurn() == false)
screen.frame(0.1)
assert(presses == 0, "not on the first frame")
screen.frame(0.25)
assert(presses == 1 and #saves == 1, kinds())
assert(saves[1].Name == "hotseat-test-T010" and saves[1].Location == 7 and saves[1].Type == 3
	and saves[1].IsAutosave == false)
assert(kinds() == "hotseat_saved,hotseat_start_turn")
-- Still up: pressed again only after a two-second wait, and never saved twice.
screen.frame(0.5)
assert(presses == 1)
screen.frame(1.6)
assert(presses == 2 and #saves == 1)
assert(reports[#reports].fields:find('"again":true', 1, true))
-- The next jack seat in the same turn presses but does not save.
localPlayer = 2
screen.frame(0.4)
assert(presses == 3 and #saves == 1)
-- "Please wait" mode (the dialog hidden) is never pressed.
localPlayer = 3
dialogHidden = true
for _ = 1, 10 do screen.frame(0.5) end
assert(presses == 3)
dialogHidden = false
-- A turn off the save cadence presses without saving.
turn = 11
localPlayer = 1
screen.frame(0.4)
assert(presses == 4 and #saves == 1)

-- The turn announcements say whose turn and whether it is the operator's.
reports = {}
localPlayer = 0
screen.announce("begin")
assert(reports[1].kind == "hotseat_turn"
	and reports[1].fields == ',"phase":"begin","player":0,"turn":11,"human":true,"civvis":false',
	reports[1].fields)
localPlayer = 3
screen.announce("end")
assert(reports[2].fields == ',"phase":"end","player":3,"turn":11,"human":false,"civvis":true',
	reports[2].fields)

-- The `--auto-human` test stub sits in the operator's seat and is pressed too.
env.cfg.HotseatHumanSeats = {}
env.cfg.HotseatStubSeats = { 0 }
localPlayer = 0
screen.frame(0.4)
assert(presses == 5, "the test stub's hand-over is pressed")
-- A seat that is neither listed human nor ours is never pressed.
env.cfg.HotseatStubSeats = {}
localPlayer = 5
screen.frame(0.4)
assert(presses == 5)

-- A single-player install never has a human seat list: nothing is held back.
env.cfg = { HotseatHumanSeats = { 0 } }
localPlayer = 0
assert(screen.humanTurn() == false, "the gate needs cfg.Hotseat")

-- The generic closer and the behind-deal closer both stand down on the
-- operator's turn.
assert(closer:find("if operatorHolds() or CivvisHotseatScreen.humanTurn() then", 1, true))
assert(closer:find("or CivvisHotseatScreen.humanTurn() then return; end", 1, true))
-- And the camera never runs in a hotseat game.
assert(read("CivvisControlMapView.lua"):find(
	"return cfg.Play ~= false and cfg.CivvisDecides and cfg.Hotseat ~= true;", 1, true))

print("hotseat_seat_test: ok")
