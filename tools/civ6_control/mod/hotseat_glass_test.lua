-- The see-through probe (`tools/civ6_hotseat.py --probe`): CivvisHotseatObserver.lua,
-- CivvisHotseatGlass.lua, CivvisHotseatProbe.lua. A diagnostic, so what is
-- pinned is that it does nothing unless asked for, that the glass consumes
-- everything and moves the map only in its `own` mode and only by the calls
-- WorldInput.lua makes, and that every probe step leaves the screen as it was.
-- Run: lua5.1 tools/civ6_control/mod/hotseat_glass_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."
local function read(name)
	local handle = assert(io.open(here .. "/" .. name, "r"))
	local text = handle:read("*a")
	handle:close()
	return text
end

local function base(config, did)
	local handlers = {}
	local function event(name)
		return setmetatable({ Add = function(f) handlers[name] = f end },
			{ __call = function(_, ...) if handlers[name] then return handlers[name](...) end end })
	end
	local env = {
		CivvisControlConfig = config, include = function() end,
		tonumber = tonumber, tostring = tostring, type = type, pcall = pcall, ipairs = ipairs,
		pairs = pairs, string = string, table = table, math = math, setmetatable = setmetatable,
		print = function() end,
		Automation = { Log = function(line) did.logs[#did.logs + 1] = line end },
		PopupPriority = { Utmost = 100, PausePanel = 102 },
		MouseEvents = { LButtonDown = 1, LButtonUp = 2, MouseMove = 3, MouseWheel = 4, RButtonUp = 5 },
		KeyEvents = { KeyDown = 10, KeyUp = 11 },
		Keys = { VK_LEFT = 37, VK_UP = 38, VK_RIGHT = 39, VK_DOWN = 40, A = 65, D = 68, S = 83, W = 87,
		         VK_ESCAPE = 27 },
		ExposedMembers = {}, Events = {}, LuaEvents = {},
	}
	for _, name in ipairs({ "LocalPlayerTurnBegin", "LocalPlayerTurnEnd" }) do env.Events[name] = event(name) end
	for _, name in ipairs({ "PlayerChange_Close", "CivvisProbeGlass" }) do env.LuaEvents[name] = event(name) end
	return env, handlers
end
local function run(name, env)
	local chunk = assert(loadstring(read(name)))
	setfenv(chunk, env)
	chunk()
end
local function message(kind, extra)
	local input = { GetMessageType = function() return kind end }
	for k, v in pairs(extra or {}) do input[k] = function() return v end end
	return input
end

-- 1. The gameplay script: one call, offered through ExposedMembers. -----------
do
	local set = {}
	local env = base({}, { logs = {} })
	env.PlayerManager = { SetLocalObserverTo = function(pid) set[#set + 1] = pid end,
	                      GetAliveMajors = function() end }
	env.Game = { GetLocalObserver = function() return 7 end, GetLocalPlayer = function() return 1 end }
	run("CivvisHotseatObserver.lua", env)
	local ok, why = env.ExposedMembers.CivvisObserver.set(0)
	assert(ok == true and why == "" and #set == 1 and set[1] == 0)
	local names, observer, player = env.ExposedMembers.CivvisObserver.surface()
	assert(names == "SetLocalObserverTo:function" and observer == 7 and player == 1, names)
	-- A game without the call says so and breaks nothing.
	env.PlayerManager = {}
	ok, why = env.ExposedMembers.CivvisObserver.set(0)
	assert(ok == false and why ~= "", "the reason is returned: " .. tostring(why))
end

-- 2. The glass. ---------------------------------------------------------------
local function glass(config)
	local did = { logs = {}, queued = {}, dequeued = 0, zoom = {}, pans = {}, drags = {}, hidden = nil }
	local env, handlers = base(config, did)
	local mouse = { x = 0.5, y = 0.5 }
	env.UIManager = {
		QueuePopup = function(self, ctx, priority, params)
			did.queued[#did.queued + 1] = { priority = priority, params = params }
		end,
		DequeuePopup = function() did.dequeued = did.dequeued + 1 end,
		GetNormalizedMousePos = function() return mouse.x, mouse.y end,
	}
	env.UI = {
		GetMapZoom = function() return 0.5 end,
		SetMapZoom = function(z, x, y) did.zoom[#did.zoom + 1] = { z, x, y } end,
		PanMap = function(x, y) did.pans[#did.pans + 1] = x .. "," .. y end,
		GetMapLookAtWorldTarget = function() return 10, 20 end,
		GetWorldFromNormalizedScreenPos_NoWrap = function(x, y) return x * 100, y * 100 end,
		DragMap = function(...) did.drags[#did.drags + 1] = { ... } end,
	}
	env.ContextPtr = {
		SetHide = function(self, hide) did.hidden = hide end,
		SetInputHandler = function(self, f, struct) handlers.input, did.struct = f, struct end,
	}
	run("CivvisHotseatGlass.lua", env)
	return env.CivvisGlass, did, handlers, env, mouse
end
for _, config in ipairs({ {}, { Hotseat = true }, { HotseatProbe = "still" }, { Hotseat = true, HotseatProbe = false } }) do
	local g, did, handlers = glass(config)
	assert(handlers.input == nil and handlers.CivvisProbeGlass == nil, "not asked for: nothing registered")
	assert(did.hidden == true, "and the context is hidden")
end
do
	local g, did, handlers, env, mouse = glass({ Hotseat = true, HotseatProbe = "still" })
	assert(did.hidden == true and did.struct == true, "hidden until queued; the struct handler (the wheel)")
	assert(handlers.input(message(4, { GetWheel = 120 })) == false, "down: every message passes")
	-- Up, consuming only: in the cover's place, and nothing moves the map.
	env.LuaEvents.CivvisProbeGlass(true, false)
	assert(#did.queued == 1 and did.queued[1].priority == 102
		and did.queued[1].params.AlwaysVisibleInQueue == true, "queued as the cover is")
	for _, input in ipairs({ message(4, { GetWheel = 120 }), message(1), message(3), message(2),
			message(10, { GetKey = 68 }), message(11, { GetKey = 68 }), message(5), message(99) }) do
		assert(handlers.input(input) == true, "consumed")
	end
	assert(#did.zoom == 0 and #did.pans == 0 and #did.drags == 0, "consuming only")
	-- Its own pan and zoom: the tally of the first phase is reported on the change.
	env.LuaEvents.CivvisProbeGlass(true, true)
	assert(#did.queued == 1, "not queued twice")
	local tally = did.logs[#did.logs - 3]
	assert(tally:find('"kind":"probe_glass"', 1, true) and tally:find("MouseWheel=1", 1, true)
		and tally:find("KeyDown=1", 1, true) and tally:find("msg99=1", 1, true)
		and tally:find('"wheel":120.0000', 1, true), tally)
	-- The wheel: WorldInput.lua:1455-1465.
	handlers.input(message(4, { GetWheel = 120 }))
	assert(#did.zoom == 1 and math.abs(did.zoom[1][1] - (0.5 - 120 / 12000 * 6)) < 1e-9
		and did.zoom[1][2] == 0 and did.zoom[1][3] == 0)
	-- A drag: WorldInput.lua:520-526, :578. No move before the button is down.
	handlers.input(message(3))
	assert(#did.drags == 0)
	handlers.input(message(1))
	mouse.x = 0.3
	handlers.input(message(3))
	local d = did.drags[1]
	assert(#did.drags == 1 and d[1] == 0.3 and d[2] == 0.5 and d[3] == 50 and d[4] == 50
		and d[5] == 10 and d[6] == 20, "x, y, start world, start focus")
	handlers.input(message(2))
	handlers.input(message(3))
	assert(#did.drags == 1, "the button is up")
	-- Keys: held keys add up, a release stops.
	handlers.input(message(10, { GetKey = 68 }))
	handlers.input(message(10, { GetKey = 87 }))
	handlers.input(message(11, { GetKey = 68 }))
	handlers.input(message(11, { GetKey = 87 }))
	assert(table.concat(did.pans, " ") == "1,0 1,1 0,1 0,0", table.concat(did.pans, " "))
	assert(handlers.input(message(10, { GetKey = 27 })) == true and #did.pans == 4, "any other key: consumed, no pan")
	-- Down: the pan is stopped and the popup dequeued.
	env.LuaEvents.CivvisProbeGlass(false, false)
	assert(did.dequeued == 1 and did.pans[#did.pans] == "0,0" and not g.up)
	assert(handlers.input(message(1)) == false)
end

-- 3. The probe. ---------------------------------------------------------------
local function probe(config)
	local did = { logs = {}, hides = {}, observed = {}, glass = {}, pans = {}, step = nil, hidden = nil }
	local env, handlers = base(config, did)
	local world = { pid = 0, turn = 1, observer = 0, zoom = 0.5 }
	local controls = {}
	for _, path in ipairs({ "/InGame/HUD", "/InGame/Screens", "/InGame/TopLevelHUD",
			"/InGame/PartialScreens", "/InGame/WorldViewControls/BannerAndFlags" }) do
		controls[path] = { SetHide = function(self, hide) did.hides[#did.hides + 1] = path .. "=" .. tostring(hide) end }
	end
	env.Game = { GetLocalObserver = function() return world.observer end,
	             GetLocalPlayer = function() return world.pid end,
	             GetCurrentGameTurn = function() return world.turn end }
	env.UI = { GetMapLookAtWorldTarget = function() return 1, 2 end, GetMapZoom = function() return world.zoom end,
	           SetMapZoom = function(z) world.zoom = z end, GetInterfaceMode = function() return 1 end,
	           GetHeadSelectedUnit = function() return nil end,
	           PanMap = function(x, y) did.pans[#did.pans + 1] = x .. "," .. y end }
	env.ExposedMembers.CivvisObserver = {
		set = function(pid) did.observed[#did.observed + 1] = pid; world.observer = pid; return true, "" end,
		surface = function() return "SetLocalObserverTo:function", world.observer, world.pid end,
	}
	env.Controls = { Step = { SetText = function(self, text) did.step = text end } }
	env.ContextPtr = {
		LookUpControl = function(self, path) return controls[path] end,
		SetHide = function(self, hide) did.hidden = hide end,
		SetUpdate = function(self, f) handlers.update = f end,
		SetShutdown = function(self, f) handlers.shutdown = f end,
	}
	handlers.CivvisProbeGlass = function(up, own) did.glass[#did.glass + 1] = tostring(up) .. "/" .. tostring(own) end
	run("CivvisHotseatProbe.lua", env)
	return env.CivvisProbe, world, did, handlers, env
end
local function steps(did)
	local out = {}
	for _, line in ipairs(did.logs) do
		local step = line:match('"kind":"probe_step".-"step":"([^"]*)"')
		if step then out[#out + 1] = step end
	end
	return out
end
for _, config in ipairs({ {}, { Hotseat = true }, { Hotseat = true, HotseatProbe = "other" }, { HotseatProbe = "still" } }) do
	local p, world, did, handlers = probe(config)
	assert(handlers.update == nil and handlers.PlayerChange_Close == nil and did.hidden == true,
		"not asked for: nothing registered, the context hidden")
end
do
	local config = { Hotseat = true, HotseatProbe = "still", HotseatHumanSeats = { 0 }, HotseatSeats = { 1 } }
	local p, world, did, handlers = probe(config)
	handlers.update(30)
	assert(#steps(did) == 0, "nothing until the person's hand-over closes")
	handlers.PlayerChange_Close(1)
	assert(#steps(did) == 0, "a CIVVIS seat's hand-over is not the start")
	handlers.PlayerChange_Close(0)
	handlers.PlayerChange_Close(0)
	assert(#steps(did) == 1 and steps(did)[1] == "started", "started once")
	-- The whole list, a second a frame: in order, each step once.
	local at = {}
	for second = 1, 240 do
		handlers.update(1)
		for i, name in ipairs(steps(did)) do at[name] = at[name] or second end
	end
	local names = steps(did)
	assert(names[2] == "baseline" and names[#names] == "done", names[#names])
	local seen = {}
	for _, name in ipairs(names) do
		assert(not seen[name], "once: " .. name)
		seen[name] = true
	end
	assert(at["baseline"] == 4 and at["observer to the CIVVIS seat (gameplay script)"] == 8)
	-- The observer goes to the CIVVIS seat and always back to the person.
	assert(did.observed[1] == 1 and did.observed[#did.observed] == 0 and world.observer == 0)
	-- Every group hidden is shown again, and only paths that exist are touched.
	local hidden = {}
	for _, change in ipairs(did.hides) do
		local path, state = change:match("^(.-)=(%a+)$")
		hidden[path] = state == "true" or nil
	end
	assert(next(hidden) == nil, "nothing is left hidden")
	assert(#did.hides == 10, "five paths resolve, each hidden and shown: " .. #did.hides)
	-- The driver's input three times: open, consuming glass, glass with pan and zoom.
	local acts = 0
	for _, name in ipairs(names) do if name:find("^act:") then acts = acts + 1 end end
	assert(acts == 15, "five actions, three phases: " .. acts)
	assert(table.concat(did.glass, " ") == "true/false true/true false/false", table.concat(did.glass, " "))
	assert(table.concat(did.pans, " ") == "1,0 0,0" and world.zoom == 0.5, "the pan stopped, the zoom put back")
	assert(did.step == "PROBE: done", "each step is named on screen")
	-- A record carries what the step began with.
	local found = false
	for _, line in ipairs(did.logs) do
		if line:find('"step":"baseline"', 1, true) then
			found = line:find('"gameplay_bridge":"function"', 1, true) and line:find('"observer":0', 1, true)
		end
	end
	assert(found, "the baseline says what each side has")
end
do
	local config = { Hotseat = true, HotseatProbe = "turns", HotseatHumanSeats = { 0 }, HotseatSeats = { 1 } }
	local p, world, did, handlers = probe(config)
	handlers.PlayerChange_Close(0)
	assert(#steps(did) == 0, "`turns` has no timed list")
	-- Turn 2: recorded, nothing touched.
	world.pid, world.turn, world.observer = 1, 2, 1
	handlers.LocalPlayerTurnBegin()
	assert(#did.observed == 0 and #did.hides == 0)
	-- Turn 5: the observer to the person, every HUD group hidden; samples follow.
	world.turn = 5
	handlers.LocalPlayerTurnBegin()
	assert(did.observed[#did.observed] == 0 and #did.hides == 4)
	for _ = 1, 20 do handlers.update(0.1) end
	local samples = 0
	for _, line in ipairs(did.logs) do if line:find('"kind":"probe_sample"', 1, true) then samples = samples + 1 end end
	assert(samples == 8, "eight samples, 0.2 s apart: " .. samples)
	handlers.LocalPlayerTurnEnd()
	-- The person's turn: everything shown again, the observer theirs.
	world.pid, world.turn, world.observer = 0, 6, 1
	handlers.LocalPlayerTurnBegin()
	assert(#did.hides == 8 and world.observer == 0, "restored on the person's turn")
	-- And on shutdown.
	world.pid = 1
	handlers.LocalPlayerTurnBegin()
	assert(#did.hides == 9)
	handlers.shutdown()
	assert(#did.hides == 10 and did.hides[10]:find("=false", 1, true))
end

print("hotseat_glass_test: ok")
