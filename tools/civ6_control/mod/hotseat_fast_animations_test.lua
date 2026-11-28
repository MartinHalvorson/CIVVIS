-- Hotseat `--fast-animations` (`HotseatFastAnimations`): every mod clock that
-- counts real seconds still does at the console's `timescale N`, and every
-- way back to 1, and the one way up (`--fast-animations`).
--
--   A. The agent (`CivvisClock`, run from the shipped file): a context follows
--      a scale another context set without a jump; a person's turn begin, the
--      last CIVVIS seat's end and a load each command `timescale 1`; a refused
--      reset stands the feature down; nothing is registered without the key.
--   B. The cover: its wait (`waited`, the bar) and its stand-aside clocks in
--      real seconds; every lift asks for 1 first; a person's own lift waits
--      for it (a second at most); the game menu and a person's turn begin ask
--      for it; without the key nothing is asked.
--   C. The HUD guard: a person's frame at a scale but 1 asks for 1 and says
--      so once a turn; a UI clock running fast on a person's turn forces it
--      and stands the feature down; CIVVIS seats' frames and installs without
--      the key are left alone.
--   D. The hand-over: its 0.3 s and 2 s in real seconds; a person's Start Turn
--      waits for 1 (a second at most); a CIVVIS seat's does not.
--   E. The switch: a CIVVIS seat's own turn under the cover commands N; every
--      reason not to (no cover, the menu, an end screen, stood down, the
--      ladder's own timescale, no live harness, a person's seat) holds it at 1
--      and says why; a refused command stands it down; the rollover key.
--
-- Run: lua5.1 tools/civ6_control/mod/hotseat_fast_animations_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."
local function read(name)
	local handle = assert(io.open(here .. "/" .. name, "r"))
	local text = handle:read("*a")
	handle:close()
	return text
end
local failures = 0
local function check(name, ok)
	if ok then
		print("ok   " .. name)
	else
		failures = failures + 1
		print("FAIL " .. name)
	end
end
local function near(a, b, tolerance)
	return type(a) == "number" and math.abs(a - b) <= (tolerance or 1e-6)
end
-- An event or LuaEvent: every listener kept; calling it raises it (and says so).
local function bus(raised)
	return setmetatable({}, { __index = function(t, name)
		local listeners = {}
		local event = setmetatable({ listeners = listeners,
			Add = function(f) listeners[#listeners + 1] = f end }, {
			__call = function(_, ...)
				raised[#raised + 1] = { name = name, args = { ... } }
				for _, f in ipairs(listeners) do f(...) end
			end })
		rawset(t, name, event)
		return event
	end })
end

-- ===================================================================== A
local agent = read("CivvisControlAgent.lua")
local clockBlock = assert(agent:match("\n(CivvisClock = { scale = 1, offset = 0 };\n.-\n)CivvisExportClock = "),
	"the agent's CivvisClock block")
local initBlock = assert(agent:match(
	"\n(\t%-%- Hotseat `%-%-fast%-animations` %(`CivvisClock%.fastWant`%).-\n\tend\n)"),
	"the agent's fast-animations registrations")

local function agentHost(cfg)
	local h = { clock = 100, local_ = 5, records = {}, commands = {}, raised = {}, dead = {} }
	h.events, h.lua = bus({}), bus(h.raised)
	h.exposed = {}
	h.autoprofiler = { RunCommand = function(command)
		if h.refuse then error("console refused") end
		h.commands[#h.commands + 1] = command
		return true
	end }
	local env
	env = {
		cfg = cfg, ipairs = ipairs, pairs = pairs, tonumber = tonumber, tostring = tostring,
		type = type, pcall = pcall, table = table, math = math, string = string,
		try = function(fn, fallback)
			local ok, value = pcall(fn)
			if ok and value ~= nil then return value end
			return fallback
		end,
		emit = function(kind, payload) h.records[#h.records + 1] = { kind = kind, p = payload } end,
		UI = { GetElapsedTime = function() return h.clock end },
		ExposedMembers = h.exposed,
		Events = h.events, LuaEvents = h.lua,
		Game = { GetLocalPlayer = function() return h.local_ end,
		         GetCurrentGameTurn = function() return 12 end },
		Players = setmetatable({}, { __index = function(_, pid)
			return { IsAlive = function() return not h.dead[pid] end }
		end }),
		CivvisSeat = { seat = tonumber(cfg.HotseatSeat) },
		-- The popup queue (`h.popups[path]`), the person-facing harness, the
		-- ladder's own timescale: what `fastBlocked` reads.
		ContextPtr = { LookUpControl = function(_, path) return path end },
		UIManager = { IsInPopupQueue = function(_, context) return h.popups[context] == true end },
		CivvisHarness = { live = function() return h.harness ~= false end },
		CivvisQueue = {},
	}
	h.popups = {}
	env.AutoProfiler = h.autoprofiler
	local chunk = assert(loadstring(clockBlock .. "\nif true then\n" .. initBlock .. "end\n"))
	setfenv(chunk, env)
	chunk()
	h.env, h.c = env, env.CivvisClock
	function h.last(kind)
		for i = #h.records, 1, -1 do
			if h.records[i].kind == kind then return h.records[i].p end
		end
	end
	function h.fire(name, ...)
		for _, f in ipairs(h.events[name].listeners) do f(...) end
	end
	return h
end
local function tableCfg(extra)
	local cfg = { Hotseat = true, HotseatCover = true, HotseatSeat = 5, HotseatSeats = { 1, 2, 3, 4, 5 },
		HotseatHumanSeats = { 0 }, HotseatStubSeats = {}, HotseatFastAnimations = 3 }
	for k, v in pairs(extra or {}) do cfg[k] = v end
	return cfg
end

-- A0. Without the key, or without the cover: nothing is registered.
for label, extra in pairs({ ["no key"] = { HotseatFastAnimations = false },
		["no cover"] = { HotseatCover = false }, ["not hotseat"] = { Hotseat = false },
		["key above 4"] = { HotseatFastAnimations = 6 } }) do
	local h = agentHost(tableCfg(extra))
	check("A0. " .. label .. ": no listener", #h.events.LocalPlayerTurnBegin.listeners == 0
		and #h.events.LoadScreenClose.listeners == 0 and #h.lua.CivvisFastAnimations.listeners == 0)
	h.fire("LoadScreenClose")
	check("A0. " .. label .. ": nothing commanded", #h.commands == 0)
end

local h = agentHost(tableCfg())
check("A1. with the key: the turn events, the load and the ask are heard",
	#h.events.LocalPlayerTurnBegin.listeners == 1 and #h.events.LocalPlayerTurnEnd.listeners == 1
	and #h.events.LoadScreenClose.listeners == 1 and #h.lua.CivvisFastAnimations.listeners == 1)

-- A2. A load: `timescale 1` once, whatever the table says.
h.fire("LoadScreenClose")
check("A2. a load commands timescale 1", h.commands[1] == "timescale 1" and #h.commands == 1)
check("…says so", h.last("fast_animations") ~= nil and h.last("fast_animations").phase == "loaded")
h.fire("LoadScreenClose")
check("…once per context", #h.commands == 1)

-- A3. Another context set the scale to 3: this one follows, without a jump,
-- and counts real seconds after.
h.clock = 200
local before = h.c.now()
h.exposed.CivvisTimeScale = 3
h.exposed.CivvisFastAnim = { on = true }
for _, f in ipairs(h.lua.CivvisFastAnimations.listeners) do f("follow", "applied") end
check("A3. follows the shared scale", h.c.scale == 3)
check("…without a jump", near(h.c.now(), before))
h.clock = h.clock + 3
check("…and three UI seconds at 3 are one real second", near(h.c.now() - before, 1))

-- A4. A CIVVIS seat's turn begins (not this one's): no command, the clock follows.
h.exposed.CivvisTimeScale = 1
h.c.scale = 3
h.local_ = 2
h.exposed.CivvisTimeScale = 3
h.fire("LocalPlayerTurnBegin")
check("A4. a CIVVIS seat's begin commands nothing", #h.commands == 1)
check("…and the clock is on the table's scale", h.c.scale == 3)

-- A5. The person's turn begins: timescale 1, the clock follows, quietly (every
-- agent context hears the same event).
h.local_ = 0
local raised = #h.raised
local t0 = h.c.now()
h.fire("LocalPlayerTurnBegin")
check("A5. a person's begin commands timescale 1", h.commands[#h.commands] == "timescale 1" and #h.commands == 2)
check("…the table reads 1", h.exposed.CivvisTimeScale == 1 and h.exposed.CivvisFastAnim.on == false)
check("…this clock too", h.c.scale == 1 and near(h.c.now(), t0))
h.clock = h.clock + 2
check("…counting a real second a second", near(h.c.now() - t0, 2))
check("…without telling the others (they hear the turn event)", #h.raised == raised)
local rec = h.last("fast_animations")
check("…recorded with why, the scale and the UI clock",
	rec.phase == "off" and rec.why == "person_turn_begin" and rec.scale == 1 and rec.ui == h.clock - 2)
-- At 1 already: nothing more.
h.fire("LocalPlayerTurnBegin")
check("A5. at 1 already, nothing is commanded", #h.commands == 2)
-- The test stub stands in for the person.
local stub = agentHost(tableCfg({ HotseatHumanSeats = {}, HotseatStubSeats = { 0 } }))
stub.exposed.CivvisTimeScale, stub.local_ = 3, 0
stub.fire("LocalPlayerTurnBegin")
check("A5. the stub's turn begins at 1 as well", stub.commands[1] == "timescale 1")

-- A6. The last CIVVIS seat before a person ends its turn: back to 1, and the
-- other contexts are told to follow.
h.exposed.CivvisTimeScale, h.exposed.CivvisFastAnim.on = 3, true
h.c.scale = 3
h.local_ = 5
raised = #h.raised
h.fire("LocalPlayerTurnEnd")
check("A6. the last seat's end commands timescale 1", h.commands[#h.commands] == "timescale 1" and #h.commands == 3)
check("…and tells the others to follow", #h.raised == raised + 1
	and h.raised[#h.raised].name == "CivvisFastAnimations" and h.raised[#h.raised].args[1] == "follow")
check("…recorded as last_seat_end", h.last("fast_animations").why == "last_seat_end")
-- A middle seat: nothing. Seat 3 with 4 alive plays on at N.
h.env.CivvisSeat.seat, h.local_ = 3, 3
h.exposed.CivvisTimeScale, h.exposed.CivvisFastAnim.on = 3, true
h.fire("LocalPlayerTurnEnd")
check("A6. a middle seat's end leaves the scale", #h.commands == 3 and h.exposed.CivvisTimeScale == 3)
-- ...but with seats 4 and 5 gone, seat 3 is the last before the person.
h.dead[4], h.dead[5] = true, true
h.fire("LocalPlayerTurnEnd")
check("A6. the last ALIVE seat before a person puts it back", #h.commands == 4)
h.dead[4], h.dead[5] = nil, nil
-- Another seat's end (not this context's): nothing.
h.local_ = 4
h.exposed.CivvisTimeScale, h.exposed.CivvisFastAnim.on = 3, true
h.fire("LocalPlayerTurnEnd")
check("A6. another seat's end is that seat's to answer", #h.commands == 4)
check("A6. who plays next", h.c.nextIsPerson(5) == true and h.c.nextIsPerson(0) == false
	and h.c.nextIsPerson(4) == false and h.c.nextIsPerson(nil) == true)

-- A7. Asked by another context (the cover, the hand-over, the HUD).
h.c.scale = 3
for _, f in ipairs(h.lua.CivvisFastAnimations.listeners) do f("reset", "menu") end
check("A7. an ask from the cover commands timescale 1", #h.commands == 5 and h.c.scale == 1)
check("…recorded with its reason", h.last("fast_animations").why == "menu")
-- `force`: commanded although the table already reads 1.
for _, f in ipairs(h.lua.CivvisFastAnimations.listeners) do f("force", "guard_clock") end
check("A7. force commands it anyway", #h.commands == 6)

-- A8. The console refuses: the feature stands down for the game, and this
-- clock stays on the scale the engine still runs at.
h.exposed.CivvisTimeScale, h.exposed.CivvisFastAnim.on = 3, true
h.c.scale, h.refuse = 3, true
h.local_ = 0
h.fire("LocalPlayerTurnBegin")
check("A8. a refused reset stands the feature down", h.exposed.CivvisFastAnim.off == "reset_failed")
check("…says so", h.last("fast_animations").phase == "reset_failed" and h.last("fast_animations").scale == 3)
check("…and keeps counting real seconds at 3", h.c.scale == 3 and h.exposed.CivvisTimeScale == 3)
h.refuse = false

-- E. The one way up: this CIVVIS seat's own turn begins under the cover.
local function seatHost(extra)
	local e = agentHost(tableCfg(extra))
	e.exposed.CivvisCoverUp = true
	e.local_ = 5
	return e
end
local function phase(e) return e.last("fast_animations") and e.last("fast_animations").phase end
local e = seatHost()
e.fire("LocalPlayerTurnBegin")
check("E1. its own turn under the cover commands timescale 3", e.commands[1] == "timescale 3" and #e.commands == 1)
check("…the table reads 3, this clock too", e.exposed.CivvisTimeScale == 3 and e.c.scale == 3
	and e.exposed.CivvisFastAnim.on == true)
check("…recorded with the scale and the UI clock", phase(e) == "on" and e.last("fast_animations").scale == 3
	and e.last("fast_animations").ui == e.clock)
check("…and the other contexts are told to follow", (e.raised[#e.raised] or {}).name == "CivvisFastAnimations"
	and e.raised[#e.raised].args[1] == "follow")
local t1 = e.c.now()
e.clock = e.clock + 3
check("E1. a real second later this clock says one", near(e.c.now() - t1, 1))
-- The next CIVVIS seat finds it at 3: nothing commanded, its clock follows.
local nxt = seatHost({ HotseatSeat = 4 })
nxt.exposed.CivvisTimeScale, nxt.exposed.CivvisFastAnim = 3, { on = true }
nxt.local_ = 4
nxt.fire("LocalPlayerTurnBegin")
check("E2. a seat that finds 3 commands nothing and follows", #nxt.commands == 0 and nxt.c.scale == 3)
-- Every reason not to, each leaving it at 1 and saying which.
for label, setup in pairs({
	not_covered = function(x) x.exposed.CivvisCoverUp = false end,
	menu = function(x) x.popups["/InGame/TopOptionsMenu"] = true end,
	end_screen = function(x) x.popups["/InGame/EndGameMenu"] = true end,
	stood_down = function(x) x.exposed.CivvisFastAnim = { off = "reset_failed" } end,
	ladder_timescale = function(x) x.env.CivvisQueue.timescale = { want = 3 } end,
}) do
	local x = seatHost()
	setup(x)
	x.fire("LocalPlayerTurnBegin")
	check("E3. " .. label .. ": nothing commanded", #x.commands == 0)
	check("…held, saying " .. label, phase(x) == "held" and x.last("fast_animations").why == label)
end
local nh = seatHost({ RequireHarness = true })
nh.harness = false
nh.fire("LocalPlayerTurnBegin")
check("E3. a person-facing install with no live harness: nothing", #nh.commands == 0
	and nh.last("fast_animations").why == "no_harness")
-- Held while the table still reads 3 (the cover stood aside, say): back to 1.
local back = seatHost()
back.exposed.CivvisCoverUp = false
back.exposed.CivvisTimeScale, back.exposed.CivvisFastAnim = 3, { on = true }
back.fire("LocalPlayerTurnBegin")
check("E3. held at 3: put back to 1", back.commands[1] == "timescale 1" and back.exposed.CivvisTimeScale == 1)
-- A seat that is a person's never raises it: the stand-in's context, or a
-- second person at the table.
for label, extra in pairs({ ["the stand-in"] = { HotseatSeat = 0, HotseatHumanSeats = { 0 }, HotseatStubSeats = { 0 } },
		["a second person"] = { HotseatSeat = 3, HotseatHumanSeats = { 0, 3 } } }) do
	local x = seatHost(extra)
	x.local_ = tonumber(extra.HotseatSeat)
	x.fire("LocalPlayerTurnBegin")
	check("E4. " .. label .. "'s own turn: nothing raised", #x.commands == 0 and x.exposed.CivvisTimeScale ~= 3)
	-- The second defence, behind the turn event's own person branch.
	check("E4. …and asked directly, " .. label .. "'s seat is held", x.c.fastApply(x.local_) == false
		and #x.commands == 0 and x.last("fast_animations").why == "person_seat")
end
-- A seat the table does not list as CIVVIS's is not one.
local unlisted = seatHost({ HotseatSeats = { 1, 2, 3, 4 } })
check("E4. a seat not listed as CIVVIS's is held", unlisted.c.fastApply(5) == false
	and #unlisted.commands == 0 and unlisted.last("fast_animations").why == "person_seat")
-- The console refuses the command: stood down, the table still at 1.
local refused = seatHost()
refused.refuse = true
refused.fire("LocalPlayerTurnBegin")
check("E5. a refused timescale 3 stands the feature down", phase(refused) == "apply_failed"
	and refused.exposed.CivvisFastAnim.off == "apply_failed" and refused.exposed.CivvisTimeScale ~= 3
	and refused.c.scale == 1)
-- `HotseatFastRollover`: the last seat's end leaves it for the rollover; the
-- person's turn begin is the reset.
local roll = seatHost({ HotseatFastRollover = true })
roll.fire("LocalPlayerTurnBegin")
roll.fire("LocalPlayerTurnEnd")
check("E6. rollover: the last seat's end leaves it at 3", #roll.commands == 1 and roll.exposed.CivvisTimeScale == 3)
roll.local_ = 0
roll.fire("LocalPlayerTurnBegin")
check("E6. …and the person's turn begin puts it back", roll.commands[2] == "timescale 1"
	and roll.exposed.CivvisTimeScale == 1)
local plain = seatHost()
plain.fire("LocalPlayerTurnBegin")
plain.fire("LocalPlayerTurnEnd")
check("E6. without it, the last seat's end puts it back", plain.commands[2] == "timescale 1")

-- ===================================================================== B
local coverSource = read("CivvisHotseatCover.lua")
local FRAME = 1 / 60

-- The cover at a table where seat 0 is the person and 1-2 are CIVVIS's.
-- `world.scale` is the engine's: each frame is FRAME real seconds, `scale`
-- times that on the UI clock and in the frame's own delta.
local function coverHost(extra, agents)
	local world = { pid = 0, turn = 3, active = { [0] = true }, paused = {}, clock = 0, menu = false,
		reports = {}, asks = {}, scale = 1 }
	local raised = {}
	local exposed = { CivvisSeatBeat = {} }
	world.exposed = exposed
	local lua, events = bus(raised), bus({})
	-- The agents: an ask puts the table back to 1, unless they refuse.
	lua.CivvisFastAnimations.Add(function(op, why)
		world.asks[#world.asks + 1] = { op = op, why = why }
		if op ~= "follow" and agents ~= "refuse" then exposed.CivvisTimeScale, world.scale = 1, 1 end
	end)
	local cfg = { Hotseat = true, RunTag = "fast", HotseatSeats = { 1, 2 }, HotseatHumanSeats = { 0 },
		HotseatCover = true, HotseatAutoStart = true, HotseatCoverTimer = true, HotseatFastAnimations = 3 }
	for k, v in pairs(extra or {}) do cfg[k] = v end
	local control = function() return {} end
	local env = {
		CivvisControlConfig = cfg, include = function() end,
		tonumber = tonumber, tostring = tostring, type = type, pcall = pcall, ipairs = ipairs,
		pairs = pairs, string = string, table = table, math = math, setmetatable = setmetatable,
		Automation = { Log = function(line) world.reports[#world.reports + 1] = line end },
		PopupPriority = { Utmost = 100, HotSeatBackground = 101, PausePanel = 102 },
		UIManager = { QueuePopup = function() end, DequeuePopup = function() end,
			IsInPopupQueue = function(_, context) return context == "menu" and world.menu end },
		Controls = setmetatable({}, { __index = function()
			return { SetHide = function() end, SetText = function() end, SetSizeX = function() end,
				SetOffsetX = function() end, CalculateSize = function() end, SetTexture = function() end,
				RegisterCallback = function() end }
		end }),
		ExposedMembers = exposed,
		Game = { GetLocalPlayer = function() return world.pid end,
		         GetCurrentGameTurn = function() return world.turn end },
		Players = setmetatable({}, { __index = function(_, pid)
			return { IsTurnActive = function() return world.active[pid] == true end }
		end }),
		PlayerConfigurations = setmetatable({}, { __index = function(_, pid)
			return { GetWantsPause = function() return world.paused[pid] == true end,
			         GetHotseatPassword = function() return "" end }
		end }),
		UI = { GetElapsedTime = function() return world.clock end },
		Events = events, LuaEvents = lua,
		Mouse = { eLClick = 1 },
		ContextPtr = {
			LookUpControl = function(_, path)
				if path == "/InGame/TopOptionsMenu" then return "menu" end
				return nil
			end,
			SetShutdown = function() end, SetInputHandler = function() end,
			SetUpdate = function(_, f) world.update = f end,
		},
	}
	if agents == "none" then
		-- No agent answers; this context's own console does.
		env.AutoProfiler = { RunCommand = function(command) world.own = command end }
	end
	local chunk = assert(loadstring(coverSource))
	setfenv(chunk, env)
	chunk()
	world.cover, world.raised, world.events, world.lua = env.CivvisHotseatCover, raised, events, lua
	function world.frames(n)
		for _ = 1, n do
			world.clock = world.clock + FRAME * world.scale
			world.update(FRAME * world.scale)
		end
	end
	function world.fire(name, ...)
		for _, f in ipairs(events[name].listeners) do f(...) end
	end
	function world.last(kind)
		for i = #world.reports, 1, -1 do
			if world.reports[i]:find('"kind":"' .. kind .. '"', 1, true) then return world.reports[i] end
		end
	end
	-- The engine at `scale`, as an applied switch leaves it.
	function world.at(scale) world.scale, exposed.CivvisTimeScale = scale, scale end
	return world
end
-- The person ends their turn; seat 1 plays at 3.
local function seatPlays(w, seconds)
	w.pid, w.active[0] = 0, false
	w.fire("LocalPlayerTurnEnd")
	w.pid, w.active[1] = 1, true
	w.fire("LocalPlayerTurnBegin")
	w.at(3)
	w.frames(math.floor(seconds / FRAME + 0.5))
end

-- B1. The wait counts real seconds at 3.
local w = coverHost()
seatPlays(w, 2)
check("B1. two real seconds at 3 are two seconds of the wait", near(w.cover.wait.elapsed, 2, 0.02))
-- The person's turn: their begin asks for 1, their lift is real seconds too.
w.active[1], w.pid, w.active[0], w.paused[0] = false, 0, true, true
local asks = #w.asks
w.fire("LocalPlayerTurnBegin")
check("B1. a person's turn begin asks for 1", #w.asks == asks + 1 and (w.asks[#w.asks] or {}).why == "person_turn_begin")
check("…and the table reads 1", w.exposed.CivvisTimeScale == 1)
w.paused[0] = false
w.frames(1)
local lifted = w.last("hotseat_cover")
check("B1. the lift's waited is real seconds", lifted ~= nil and lifted:find('"up":false', 1, true) ~= nil
	and near(tonumber(lifted:match('"waited":([%d.]+)')), 2 + FRAME, 0.03))

-- B2. A stand-aside clock in real seconds: a quiet CIVVIS seat at 3 is not
-- left before 90 real seconds.
w = coverHost()
seatPlays(w, 0)
w.frames(math.floor(89 / FRAME))
check("B2. 89 real seconds of a quiet seat at 3: still covered", w.cover.up == true)
w.frames(math.floor(2 / FRAME))
check("B2. past 90: the cover stands aside", w.cover.up == false
	and w.last("hotseat_cover"):find('"reason":"yield_quiet"', 1, true) ~= nil)
check("…and asked for 1 first", (w.asks[#w.asks] or {}).why == "lift_yield_quiet")

-- B3. The person's lift waits for 1: the agents refuse, the lift holds a
-- second, asking every frame, then lifts and says so.
w = coverHost(nil, "refuse")
seatPlays(w, 1)
w.active[1], w.pid, w.active[0], w.paused[0] = false, 0, true, true
w.fire("LocalPlayerTurnBegin")
w.paused[0] = false
for _, f in ipairs(w.lua.PlayerChange_Close.listeners) do f(0) end
check("B3. a refused reset holds the person's lift", w.cover.up == true)
check("…and says so", w.last("hotseat_cover_scale") ~= nil
	and w.last("hotseat_cover_scale"):find('"held":true', 1, true) ~= nil)
asks = #w.asks
w.frames(math.floor(0.9 / FRAME))
check("B3. still held at 0.9 s, asking every frame", w.cover.up == true and #w.asks >= asks + 50)
w.frames(math.floor(0.2 / FRAME))
check("B3. lifted after a second", w.cover.up == false)
check("…saying it was not confirmed", w.last("hotseat_cover_scale"):find('"confirmed":false', 1, true) ~= nil)
-- `--keep-handover`: the person presses their own, so the cover lifts as
-- their turn begins; held, it is asked again every frame while they have not.
w = coverHost({ HotseatAutoStart = false }, "refuse")
seatPlays(w, 0.5)
w.active[1], w.pid, w.active[0], w.paused[0] = false, 0, true, true
w.fire("LocalPlayerTurnBegin")
check("B3. keep-handover: the lift at their turn begin is held", w.cover.up == true)
w.frames(math.floor(1.2 / FRAME))
check("B3. …and lifted a second on although they have not pressed", w.cover.up == false
	and w.last("hotseat_cover"):find('"reason":"person_turn"', 1, true) ~= nil)
-- A stand-aside is not held: the person must see the stall at once.
w = coverHost(nil, "refuse")
seatPlays(w, 0)
w.cover.lift("yield_long", true)
check("B3. a stand-aside lifts at once even unconfirmed", w.cover.up == false)
-- Confirmed: the lift is not held at all.
w = coverHost()
seatPlays(w, 1)
w.active[1], w.pid, w.active[0] = false, 0, true
w.at(3)
for _, f in ipairs(w.lua.PlayerChange_Close.listeners) do f(0) end
check("B3. a reset that takes lifts at once", w.cover.up == false and w.last("hotseat_cover_scale") == nil)

-- B4. The game menu over the cover: asked for 1 on its first frame, and on
-- opening it from anywhere.
w = coverHost()
seatPlays(w, 0.5)
w.menu = true
w.frames(1)
check("B4. the menu over the cover asks for 1", (w.asks[#w.asks] or {}).why == "menu" and w.exposed.CivvisTimeScale == 1)
w.menu = false
w.at(3)
for _, f in ipairs(w.lua.PlayerChange_OpenInGameOptionsMenu.listeners) do f() end
check("B4. opening the menu asks for 1 at once", (w.asks[#w.asks] or {}).why == "menu" and w.exposed.CivvisTimeScale == 1)

-- B5. No agent answers: the cover's own console puts it back.
w = coverHost(nil, "none")
seatPlays(w, 0.2)
w.lua.CivvisFastAnimations.listeners[1] = function() end
w.cover.lift("quit_request")
check("B5. with no agent, the cover's own console commands it", w.own == "timescale 1"
	and w.exposed.CivvisTimeScale == 1)

-- B6. Without the key: nothing asked, at any scale (the clocks still divide).
w = coverHost({ HotseatFastAnimations = false })
check("B6. without the key: no menu listener", #w.lua.PlayerChange_OpenInGameOptionsMenu.listeners == 0)
seatPlays(w, 1)
w.cover.lift("quit_request")
check("B6. without the key a lift asks nothing", #w.asks == 0 and w.cover.up == false)
w = coverHost({ HotseatFastAnimations = false })
seatPlays(w, 0)
w.at(1)
w.frames(60)
check("B6. at scale 1 the wait is the UI clock, as before", near(w.cover.wait.elapsed, 1, 0.001))

-- B7. The cover says whether it is up, for the seat whose turn begins under
-- it: up as it is queued, down on every lift; never written without the key.
w = coverHost()
check("B7. down until it is queued", w.exposed.CivvisCoverUp == false)
seatPlays(w, 0.1)
check("B7. up once queued", w.exposed.CivvisCoverUp == true)
w.cover.lift("yield_quiet", true)
check("B7. down when it stands aside", w.exposed.CivvisCoverUp == false)
w = coverHost({ HotseatFastAnimations = false })
seatPlays(w, 0.1)
check("B7. without the key it is never written", w.exposed.CivvisCoverUp == nil)

-- ===================================================================== C
local function hudHost(extra, agents)
	local h = { pid = 1, turn = 7, ui = 0, wall = 1000, logs = {}, asks = {}, pulses = 0 }
	local exposed = {}
	h.exposed = exposed
	local cfg = { Hotseat = true, HotseatCover = true, HotseatFastAnimations = 3, CivvisDecides = true,
		HotseatHumanSeats = { 0 }, HotseatSeats = { 1, 2 }, RunTag = "fast-hud" }
	for k, v in pairs(extra or {}) do cfg[k] = v end
	local env = setmetatable({ CivvisControlConfig = cfg }, { __index = _G })
	env.include = function(name)
		if name ~= "TopPanel" then error("unavailable") end
		env.LateInitialize = function() end
	end
	env.ContextPtr = { SetUpdate = function(_, f) h.update = f end }
	env.ExposedMembers = exposed
	env.LuaEvents = setmetatable({
		CivvisControlPulse = function() h.pulses = h.pulses + 1 end,
		CivvisControlPeek = function() end,
		CivvisFastAnimations = function(op, why)
			h.asks[#h.asks + 1] = { op = op, why = why }
			if op ~= "follow" and agents ~= "refuse" then exposed.CivvisTimeScale = 1 end
		end,
	}, { __index = function() return { Add = function() end } end })
	env.Events = setmetatable({}, { __index = function() return { Add = function() end } end })
	env.Game = { GetLocalPlayer = function() return h.pid end, GetCurrentGameTurn = function() return h.turn end }
	env.UI = { GetElapsedTime = function() return h.ui end }
	env.Automation = { Log = function(line) h.logs[#h.logs + 1] = line end,
	                   GetTime = function() return math.floor(h.wall) end }
	env.Players = setmetatable({}, { __index = function() return { IsTurnActive = function() return false end } end })
	local chunk = assert(loadfile(here .. "/CivvisControlHeartbeat.lua"))
	setfenv(chunk, env)
	chunk()
	-- One frame: `real` seconds of wall clock, `rate` times that on the UI clock.
	function h.frame(real, rate)
		h.wall = h.wall + real
		h.ui = h.ui + real * (rate or 1)
		h.update(real * (rate or 1))
	end
	function h.count(kind)
		local n = 0
		for _, line in ipairs(h.logs) do
			if line:find('"kind":"' .. kind .. '"', 1, true) then n = n + 1 end
		end
		return n
	end
	return h
end
local function guardLog(h)
	for i = #h.logs, 1, -1 do
		if h.logs[i]:find('"kind":"fast_animations_guard"', 1, true) then return h.logs[i] end
	end
end

-- C1. A CIVVIS seat's frames at 3: left alone.
local hud = hudHost()
hud.exposed.CivvisTimeScale = 3
for _ = 1, 30 do hud.frame(FRAME, 3) end
check("C1. a CIVVIS seat's frames at 3 ask nothing", #hud.asks == 0 and hud.count("fast_animations_guard") == 0)
-- C2. The person's frame at 3: asked for 1 at once, said once.
hud.pid = 0
hud.frame(FRAME, 3)
check("C2. a person's frame at 3 asks for 1", #hud.asks == 1 and hud.asks[1].op == "reset"
	and hud.asks[1].why == "guard" and hud.exposed.CivvisTimeScale == 1)
local said = guardLog(hud)
check("…and says so", said ~= nil and said:find('"why":"scale"', 1, true) ~= nil
	and said:find('"reset":true', 1, true) ~= nil and said:find('"by":"agents"', 1, true) ~= nil
	and said:find('"scale":3', 1, true) ~= nil)
for _ = 1, 30 do hud.frame(FRAME) end
check("C2. at 1 after: no more asks", #hud.asks == 1 and hud.count("fast_animations_guard") == 1)
-- The real-seconds pulse kept its second at 3 (CivvisFrameClock).
local p0 = hud.pulses
hud.pid = 1
hud.exposed.CivvisTimeScale = 3
for _ = 1, 30 do hud.frame(FRAME, 3) end
check("C2. at 3 the HUD pulse still waits a real second", hud.pulses == p0)
for _ = 1, 31 do hud.frame(FRAME, 3) end
check("…and comes after it", hud.pulses == p0 + 1)

-- C3. Refused, with no console of its own: asked every frame, said once a
-- turn, and the feature stands down.
hud = hudHost(nil, "refuse")
hud.pid = 0
hud.exposed.CivvisTimeScale = 3
for _ = 1, 20 do hud.frame(FRAME, 3) end
check("C3. refused: asked on every frame", #hud.asks == 20)
check("…said once this turn", hud.count("fast_animations_guard") == 1
	and guardLog(hud):find('"reset":false', 1, true) ~= nil)
check("…and the feature stands down", hud.exposed.CivvisFastAnim ~= nil
	and hud.exposed.CivvisFastAnim.off == "guard_reset_failed")
hud.turn = 8
hud.frame(FRAME, 3)
check("C3. said again on the next turn", hud.count("fast_animations_guard") == 2)

-- C4. The UI clock runs twice the wall clock on a person's turn although the
-- table says 1: forced, said, stood down.
hud = hudHost()
hud.pid = 0
for _ = 1, 9 * 60 do hud.frame(FRAME, 1) end
check("C4. a person's turn at 1: no reading against it", #hud.asks == 0)
for _ = 1, 12 * 60 do hud.frame(FRAME, 1) end
check("C4. a real second a second, over ten: still nothing", #hud.asks == 0)
for _ = 1, 12 * 60 do hud.frame(FRAME, 2) end
check("C4. twice the wall clock: forced", #hud.asks >= 1 and hud.asks[#hud.asks].op == "force"
	and hud.asks[#hud.asks].why == "guard_clock")
said = guardLog(hud)
check("…said with the ratio", said ~= nil and said:find('"why":"clock"', 1, true) ~= nil
	and tonumber(said:match('"ratio":([%d.]+)')) >= 1.8)
check("…and stood down", hud.exposed.CivvisFastAnim.off == "clock_fast")

-- C5. Without the key: nothing, at any scale.
hud = hudHost({ HotseatFastAnimations = false })
hud.pid = 0
hud.exposed.CivvisTimeScale = 3
for _ = 1, 20 do hud.frame(FRAME, 3) end
check("C5. without the key the guard is silent", #hud.asks == 0 and hud.count("fast_animations_guard") == 0)
hud = hudHost({ Hotseat = false })
hud.pid = 0
hud.exposed.CivvisTimeScale = 3
for _ = 1, 20 do hud.frame(FRAME, 3) end
check("C5. outside hotseat (the ladder at its timescale) too", #hud.asks == 0)

-- ===================================================================== D
local closer = read("CivvisControlAutoClose.lua")
local screenBlock = assert(closer:match("(CivvisHotseatScreen = { .-\n)if NAME == \"PlayerChange\" then"))
local function screenHost(extra, agents)
	local s = { presses = 0, reports = {}, player = 1, hidden = false, paused = true, asks = {} }
	local exposed = {}
	s.exposed = exposed
	local cfg = { Hotseat = true, HotseatCover = true, HotseatHumanSeats = { 0 }, HotseatSeats = { 1, 2 },
		HotseatAutoStart = true, HotseatFastAnimations = 3 }
	for k, v in pairs(extra or {}) do cfg[k] = v end
	local env = {
		cfg = cfg, RUN = "fast", tonumber = tonumber, tostring = tostring, type = type, pcall = pcall,
		ipairs = ipairs, string = string,
		report = function(kind, fields) s.reports[#s.reports + 1] = { kind = kind, fields = fields or "" } end,
		Game = { GetLocalPlayer = function() return s.player end, GetCurrentGameTurn = function() return 4 end },
		Controls = {
			PopupAlphaIn = { IsHidden = function() return s.hidden end, SetHide = function(_, x) s.hidden = x end },
			PlayerChangingText = { IsHidden = function() return true end, SetHide = function() end },
		},
		PlayerConfigurations = setmetatable({}, { __index = function()
			return { GetWantsPause = function() return s.paused end, GetHotseatPassword = function() return "" end }
		end }),
		OnOk = function() s.presses = s.presses + 1 end,
		SaveLocations = { LOCAL_STORAGE = 7 },
		Network = { GetGameConfigurationSaveType = function() return 3 end, SaveGame = function() end },
		Events = { LocalPlayerTurnBegin = { Add = function() end }, LocalPlayerTurnEnd = { Add = function() end } },
		ContextPtr = { SetUpdate = function() end },
		ExposedMembers = exposed,
		LuaEvents = { CivvisFastAnimations = function(op, why)
			s.asks[#s.asks + 1] = { op = op, why = why }
			if agents ~= "refuse" then exposed.CivvisTimeScale = 1 end
		end },
	}
	local chunk = assert(loadstring(screenBlock))
	setfenv(chunk, env)
	chunk()
	s.screen = env.CivvisHotseatScreen
	function s.show(player) s.player, s.hidden, s.paused = player, false, true end
	function s.kind(i) return s.reports[i or #s.reports] and s.reports[i or #s.reports].kind end
	return s
end

-- D1. A CIVVIS seat's hand-over at 3 (no `--fast-turns`): 0.3 real seconds.
local sc = screenHost()
sc.exposed.CivvisTimeScale = 3
sc.show(1)
for _ = 1, 15 do sc.screen.frame(FRAME * 3) end
check("D1. at 3, not pressed after 0.25 real seconds", sc.presses == 0)
for _ = 1, 4 do sc.screen.frame(FRAME * 3) end
check("D1. pressed at 0.3 real seconds", sc.presses == 1)
check("…without asking for 1 (a CIVVIS seat's turn)", #sc.asks == 0)
for _ = 1, 110 do sc.screen.frame(FRAME * 3) end
check("D1. its retry not before two real seconds", sc.presses == 1)
for _ = 1, 12 do sc.screen.frame(FRAME * 3) end
check("…and at two", sc.presses == 2)

-- D2. A person's hand-over at 3: asked for 1, pressed once it reads 1.
sc = screenHost({ HotseatFastTurns = true })
sc.exposed.CivvisTimeScale = 3
sc.show(0)
sc.screen.frame(FRAME * 3)
check("D2. the person's press asks for 1 first", #sc.asks == 1 and sc.asks[1].why == "person_handover")
check("…and presses once it reads 1", sc.presses == 1 and sc.exposed.CivvisTimeScale == 1)

-- D3. Refused: the person's press waits a real second, then goes and says so.
sc = screenHost({ HotseatFastTurns = true }, "refuse")
sc.exposed.CivvisTimeScale = 3
sc.show(0)
sc.screen.frame(FRAME * 3)
check("D3. refused: the person's press waits", sc.presses == 0 and sc.kind() == "hotseat_scale_hold")
for _ = 1, 50 do sc.screen.frame(FRAME * 3) end
check("D3. still waiting under a real second", sc.presses == 0)
for _ = 1, 12 do sc.screen.frame(FRAME * 3) end
check("D3. pressed after a second", sc.presses == 1)
local unconfirmed = false
for _, r in ipairs(sc.reports) do
	if r.kind == "hotseat_scale_unconfirmed" then unconfirmed = true end
end
check("…saying so", unconfirmed)

-- D4. Without the key, nothing is asked of anyone.
sc = screenHost({ HotseatFastTurns = true, HotseatFastAnimations = false })
sc.exposed.CivvisTimeScale = 3
sc.show(0)
sc.screen.frame(FRAME)
check("D4. without the key the person's press asks nothing", #sc.asks == 0 and sc.presses == 1)

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("\nhotseat_fast_animations_test: ok")
