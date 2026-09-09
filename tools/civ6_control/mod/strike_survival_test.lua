-- Selected survival policy must reject lethal host previews at actual issue time.
-- Fixture follows order_queue_test.lua and loads the production controller.
local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end

local EXPORTS = {
	CivvisApplyOrders = true, CivvisQueue = true, CivvisResolveActions = true,
	CivvisApplyOrder = true,
}
-- Real tables the agent indexes with real keys.
local LOG = {}
Automation = { Log = function(line) LOG[#LOG + 1] = line end }
UnitOperationTypes = { PARAM_X = "x", PARAM_Y = "y" }
ActivityTypes = { ACTIVITY_OPERATION = "operation" }
UnitCommandTypes = {}
Map = {
	GetPlotDistance = function(x1, y1, x2, y2)
		-- Manhattan-ish is enough for a radius test on an offset grid.
		return math.max(math.abs(x1 - x2), math.abs(y1 - y2))
	end,
	GetPlot = function() return nil end,
}
GameInfo = setmetatable({}, { __index = function(_, k)
	if k == "UnitOperations" or k == "UnitCommands" then
		return setmetatable({}, { __index = function(_, name) return { Hash = name } end })
	end
	if k == "Units" then
		return setmetatable({}, { __index = function(_, name)
			if name == "UNIT_SETTLER" then return { UnitType = name, Combat = 0, RangedCombat = 0 } end
			if name == "UNIT_ARCHER" then return { UnitType = name, Combat = 15, RangedCombat = 25 } end
			return { UnitType = name, Combat = 20, RangedCombat = 0 }
		end })
	end
	return stub()
end })
setmetatable(_G, { __index = function(_, k)
	if EXPORTS[k] then return rawget(_G, k) end
	return stub()
end })

-- ---------------------------------------------------------------- fake host
local host = { units = {}, ops = {}, commands = {}, allow_cancel = false,
	defer_cancel = false,
	refuse = {}, contacts = {}, governor_requests = {}, cities = {} }
local PID = 0
local function unitObject(u)
	return {
		GetID = function() return u.id end,
		GetX = function() return u.x end,
		GetY = function() return u.y end,
		GetMovesRemaining = function() return u.moves end,
		GetUnitType = function() return u.kind end,
		GetType = function() return u.kind end,
		GetDamage = function() return u.damage or 0 end,
		GetMaxDamage = function() return u.maximum or 100 end,
		GetComponentID = function() return { player = PID, id = u.id, type = 1 } end,
		GetRangedCombat = function() return 25 end,
		GetBombardCombat = function() return 0 end,
		GetGreatPerson = function() return nil end,
		GetFortifyTurns = function() return 0 end,
		GetFormationUnitCount = function() return 1 end,
		GetAttacksRemaining = function() return u.attacks or 1 end,
		GetActivityType = function() return u.activity end,
	}
end
UnitManager = {
	GetUnit = function(pid, id)
		local u = host.units[id]
		if u == nil or u.gone then return nil end
		return unitObject(u)
	end,
	GetActivityType = function(unit)
		return host.units[unit.GetID()].activity
	end,
	CanStartOperation = function(unit, hash, _, params)
		local id = unit.GetID()
		if host.refuse[id] and host.refuse[id][hash] then return false end
		return true
	end,
	RequestOperation = function(unit, hash, params)
		local u = host.units[unit.GetID()]
		host.ops[#host.ops + 1] = { id = u.id, op = hash,
			x = params and params.x or nil, y = params and params.y or nil }
		if hash == "UNITOPERATION_MOVE_TO" then
			-- Asynchronous, like the host: the unit arrives only when the
			-- test says so (`host.arrive`), unless the walk was priced dead.
			u.pendingX, u.pendingY = params.x, params.y
			if u.active_operation then u.activity = "operation" end
		end
	end,
	CanStartCommand = function(_, hash)
		return hash == "UNITCOMMAND_CANCEL" and host.allow_cancel
	end,
	RequestCommand = function(unit, hash)
		host.commands[#host.commands + 1] = { id = unit.GetID(), command = hash }
		if hash == "UNITCOMMAND_CANCEL" and not host.defer_cancel then
			host.units[unit.GetID()].activity = nil
		end
	end,
}
PlayerOperations = {
	PARAM_GOVERNOR_TYPE = "governor_type",
	PARAM_PLAYER_ONE = "player_one",
	PARAM_CITY_DEST = "city_dest",
	ASSIGN_GOVERNOR = "assign_governor",
}
GameInfo.Governors = {
	GOVERNOR_THE_MERCHANT = {
		Hash = "GOVERNOR_THE_MERCHANT", Index = 17,
	},
}
CityManager = {
	GetCity = function(owner, id)
		return host.cities[id] and { owner = owner, id = id } or nil
	end,
}
UI = {
	RequestPlayerOperation = function(pid, operation, params)
		host.governor_requests[#host.governor_requests + 1] = {
			pid = pid, operation = operation, params = params,
		}
	end,
}
function host.arrive(id)
	local u = host.units[id]
	if u.pendingX ~= nil then
		u.x, u.y = u.pendingX, u.pendingY
		u.pendingX, u.pendingY = nil, nil
		u.moves = math.max(0, (u.moves or 0) - 1)
	end
end
function host.deactivate(id)
	host.units[id].activity = nil
end
local function members(list)
	return function()
		local i = 0
		return function()
			i = i + 1
			if list[i] == nil then return nil end
			return i, list[i]
		end
	end
end
local player = setmetatable({
	GetGovernors = function()
		return {
			HasGovernor = function(_, hash) return hash == "GOVERNOR_THE_MERCHANT" end,
			GetGovernor = function() return nil end,
		}
	end,
	GetUnits = function()
		local objs = {}
		for _, u in pairs(host.units) do
			if not u.gone then objs[#objs + 1] = unitObject(u) end
		end
		table.sort(objs, function(a, b) return a.GetID() < b.GetID() end)
		return { Members = members(objs) }
	end,
	GetCities = function() return { Members = members({}) } end,
	GetDiplomacy = function() return { IsAtWarWith = function() return false end } end,
	GetScore = function() return 0 end,
	GetTreasury = function() return { GetGoldBalance = function() return 0 end } end,
	IsTurnActive = function() return true end,
}, { __index = function() return stub() end })
Players = setmetatable({}, { __index = function(_, pid)
	if pid == PID then return player end
	-- One barbarian seat with the hostile units the test plants.
	return setmetatable({
		IsBarbarian = function() return true end,
		GetUnits = function()
			local objs = {}
			for _, u in ipairs(host.contacts) do objs[#objs + 1] = unitObject(u) end
			return { Members = members(objs) }
		end,
		GetCities = function() return { Members = members({}) } end,
	}, { __index = function() return stub() end })
end })
PlayerManager = { GetAliveIDs = function() return { PID, 63 } end,
                  GetAliveMajorIDs = function() return { PID } end }
PlayersVisibility = setmetatable({}, { __index = function()
	return { IsVisible = function() return true end, IsRevealed = function() return true end }
end })
Game = { GetLocalPlayer = function() return PID end, GetCurrentGameTurn = function() return 7 end }

ComponentType = { UNIT = 1 }
CombatTypes = { MELEE = 1, RANGED = 2, BOMBARD = 3 }
CombatResultParameters = { ATTACKER = "att", DEFENDER = "def", DAMAGE_TO = "damage", DEFENSE_DAMAGE_TO = "walls", COMBAT_STRENGTH = "strength" }
UI.IsGameCoreBusy = function() return false end
CombatManager = { SimulateAttackInto = function()
    host.simulations = (host.simulations or 0) + 1
    if host.preview == nil then return nil end
    return { att = { damage = host.preview }, def = { damage = 1 } }
end }

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtime_err = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtime_err))

local applyOrders = rawget(_G, "CivvisApplyOrders")
local queue = rawget(_G, "CivvisQueue")
local resolveActions = rawget(_G, "CivvisResolveActions")
assert(type(applyOrders) == "function", "CivvisApplyOrders is not exported")
assert(type(queue) == "table", "CivvisQueue is not exported")
assert(type(resolveActions) == "function", "CivvisResolveActions is not exported")
resolveActions()

local failures = 0
local function check(name, got, want)
	if got ~= want then
		failures = failures + 1
		print(string.format("FAIL %s: got %s want %s", name, tostring(got), tostring(want)))
	else
		print(string.format("ok   %s = %s", name, tostring(got)))
	end
end
local function ops(id)
	local out = {}
	for _, o in ipairs(host.ops) do
		if id == nil or o.id == id then out[#out + 1] = o.op end
	end
	return table.concat(out, ",")
end
local function lastEvent(kind)
	for i = #LOG, 1, -1 do
		if LOG[i]:find('"kind":"' .. kind .. '"', 1, true) then return LOG[i] end
	end
	return nil
end
local function field(line, name)
	if line == nil then return nil end
	local v = line:match('"' .. name .. '":(%-?%d+)')
	return v and tonumber(v) or line:match('"' .. name .. '":"([^"]*)"')
end
local function row(subject, verb, x, y)
	return { kind = "unit", subject = subject, verb = verb, x = x, y = y }
end
local function reset()
	host.units, host.ops, host.commands, host.refuse, host.contacts = {}, {}, {}, {}, {}
	host.governor_requests, host.cities = {}, {}
	host.allow_cancel, host.defer_cancel = false, false
	queue.reset(7)
	host.preview, host.simulations = 100, 0
	LOG = {}
end

local function scout()
    host.units[10] = { id = 10, kind = "UNIT_SCOUT", x = 31, y = 43, moves = 5 }
end
local function policy()
    return { kind = "combat_policy", verb = "DOOMED_BLOW_VETO" }
end

reset(); scout()
applyOrders(player, PID, 7, { row(10, "ATTACK", 31, 42) })
check("off policy retains recorded attack", ops(10), "UNITOPERATION_MOVE_TO")

reset(); scout()
applyOrders(player, PID, 7, { row(10, "ATTACK", 31, 42), policy() })
check("host predicts scout death: no attack", ops(10), "")
check("policy consumption acknowledged", field(lastEvent("combat_policy_applied"), "policy"), "DOOMED_BLOW_VETO")
check("metadata not counted as an action", field(lastEvent("orders"), "seen"), 1)
check("refusal names actual HP", field(lastEvent("strike_survival_refused"), "hp"), 100)
check("refused strike not recorded as issued", lastEvent("strike"), nil)

reset(); scout(); host.preview = 99
applyOrders(player, PID, 7, { policy(), row(10, "ATTACK", 31, 42) })
check("surviving attack remains legal", ops(10), "UNITOPERATION_MOVE_TO")

reset(); scout(); host.units[10].damage = 60; host.preview = 40
applyOrders(player, PID, 7, { policy(), row(10, "ATTACK", 31, 42) })
check("current HP sets lethal threshold", ops(10), "")

reset(); scout(); host.preview = nil
applyOrders(player, PID, 7, { policy(), row(10, "ATTACK", 31, 42) })
check("unavailable preview retains native decision", ops(10), "UNITOPERATION_MOVE_TO")

reset(); scout(); host.preview = 0
applyOrders(player, PID, 7, { policy(), row(10, "MOVE_TO", 30, 43), row(10, "ATTACK", 31, 42) })
check("only approach issued initially", ops(10), "UNITOPERATION_MOVE_TO")
host.arrive(10); host.preview = 100
queue.noteUnitEvent(PID, PID, 10)
for _ = 1, 8 do queue.drain(player, PID, 7) end
check("queued attack rechecks after approach", ops(10), "UNITOPERATION_MOVE_TO")

reset(); scout(); host.preview = 100
applyOrders(player, PID, 7, { policy(), row(10, "ATTACK", 31, 42) })
host.ops = {}
applyOrders(player, PID, 7, { row(10, "ATTACK", 31, 42) })
check("policy does not leak into next batch", ops(10), "UNITOPERATION_MOVE_TO")

reset(); scout(); host.units[10].maximum = 120; host.preview = 100
applyOrders(player, PID, 7, { policy(), row(10, "ATTACK", 31, 42) })
check("host maximum HP is authoritative", ops(10), "UNITOPERATION_MOVE_TO")

reset(); scout(); host.preview = 0
applyOrders(player, PID, 7, { policy(), row(10, "RANGE_ATTACK", 31, 42) })
check("safe ranged strike preserved", ops(10), "UNITOPERATION_RANGE_ATTACK")

if failures > 0 then error(tostring(failures) .. " failures") end
print("strike survival controls passed")
