-- Test the real shipped dispatcher against a fake native command host.
local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end

local EXPORTS = { CivvisApplyOrder = true, CivvisResolveActions = true, CivvisLedger = true }
setmetatable(_G, { __index = function(_, key)
	if EXPORTS[key] then return rawget(_G, key); end
	return stub()
end })

local LOG = {}
Automation = { Log = function(line) LOG[#LOG + 1] = line end }

-- `StrikePreview` off keeps the host's combat simulation out of the ATTACK
-- case; the strike is still recorded, which is all this test reads.
CivvisControlConfig = { StrikePreview = false, CapMovesToReach = false }

UnitCommandTypes = { PARAM_UNIT_PLAYER = "owner", PARAM_UNIT_ID = "unit_id" }
UnitOperationTypes = { PARAM_X = "x", PARAM_Y = "y", PARAM_MODIFIERS = "modifiers" }
UnitOperationMoveModifiers = { ATTACK = 4, MOVE_IGNORE_UNEXPLORED_DESTINATION = 8 }

-- Every name the agent resolves gets a distinct, checkable hash.
local HASHES = {}
local function hashFor(name)
	if HASHES[name] == nil then
		HASHES[name] = "hash:" .. name
	end
	return HASHES[name]
end
local function hashTable()
	return setmetatable({}, { __index = function(_, name)
		if type(name) ~= "string" then return nil; end
		return { Type = name, Hash = hashFor(name) }
	end })
end
GameInfo = setmetatable({}, { __index = function(_, key)
	if key == "UnitCommands" or key == "UnitOperations" then return hashTable(); end
	if key == "Units" then
		return setmetatable({}, { __index = function(_, name)
			return { UnitType = tostring(name), Combat = 0, RangedCombat = 0 }
		end })
	end
	return stub()
end })

-- ---------------------------------------------------------------- fake host
local host = { units = {}, allow = {}, calls = {} }
local PID = 0

local function unitObject(u)
	return {
		GetID = function() return u.id end,
		GetX = function() return u.x end,
		GetY = function() return u.y end,
		GetType = function() return u.kind end,
		GetUnitType = function() return u.kind end,
		GetDamage = function() return 0 end,
		GetMovesRemaining = function() return 2 end,
	}
end

UnitManager = {
	GetUnit = function(pid, id)
		local u = host.units[tostring(pid) .. ":" .. tostring(id)]
		if u == nil then return nil; end
		return unitObject(u)
	end,
	CanStartCommand = function(unit, hash, a, b)
		host.calls[#host.calls + 1] = { what = "CanStartCommand", hash = hash, params = a }
		return host.allow[hash] == true
	end,
	RequestCommand = function(unit, hash, params)
		host.calls[#host.calls + 1] = { what = "RequestCommand", hash = hash, params = params }
	end,
	CanStartOperation = function(unit, hash, _, params)
		host.calls[#host.calls + 1] = { what = "CanStartOperation", hash = hash, params = params }
		return host.allow[hash] == true
	end,
	RequestOperation = function(...)
		local argc = select("#", ...)
		local unit, hash, params = ...
		host.calls[#host.calls + 1] = {
			what = "RequestOperation", hash = hash, params = params, argc = argc
		}
	end,
}
Game = { GetLocalPlayer = function() return PID end, GetCurrentGameTurn = function() return 118 end }

local chunk, err = loadfile(here .. "/CivvisControlAgent.lua")
assert(chunk, "could not load agent: " .. tostring(err))
local ran, runtimeErr = pcall(chunk)
assert(ran, "CivvisControlAgent.lua raised at chunk load: " .. tostring(runtimeErr))

local applyOrder = rawget(_G, "CivvisApplyOrder")
assert(type(applyOrder) == "function", "CivvisControlAgent.lua did not export CivvisApplyOrder")
rawget(_G, "CivvisResolveActions")()


local requests = {}
local raise_request = false
UnitManager.CanStartCommand = function(unit, hash, loose, strict)
 assert(loose == false and strict == true, "use the strict native command gate")
 return host.allow[hash] == true
end
UnitManager.RequestCommand = function(...)
 local unit, hash = ...
 assert(select("#", ...) == 2, "parameterless command must have exactly two arguments")
 if raise_request then error("fake host request failed") end
 requests[#requests+1] = {unit=unit:GetID(), hash=hash}
end
host.units["0:41"] = {player=0,id=41,x=4,y=4,kind="UNIT_BUILDER"}
local row = {kind="unit",subject=41,verb="PROJECT_PRODUCTION",x=4,y=4}
local hash = hashFor("UNITCOMMAND_PROJECT_PRODUCTION")
local player = stub()
local ok, why = applyOrder(player,PID,row,118)
assert(not ok and why == "cannot_project_production")
assert(#requests == 0, "refusal must not request")
host.allow[hash] = true
ok, why = applyOrder(player,PID,row,118)
assert(ok and why == "project_contribution_requested")
assert(#requests == 1 and requests[1].hash == hash and requests[1].unit == 41)
row.x = 5
ok, why = applyOrder(player,PID,row,118)
assert(not ok and why == "project_position_changed" and #requests == 1)
row.x = nil
ok, why = applyOrder(player,PID,row,118)
assert(not ok and why == "no_project_position" and #requests == 1)
row.x = 4
raise_request = true
ok, why = applyOrder(player,PID,row,118)
assert(not ok and tostring(why):find("fake host request failed",1,true) and #requests == 1)
local commands = GameInfo.UnitCommands
GameInfo.UnitCommands = setmetatable({}, {__index=function(_, name)
 if name == "UNITCOMMAND_PROJECT_PRODUCTION" then return nil end
 return commands[name]
end})
rawget(_G, "CivvisResolveActions")()
ok, why = applyOrder(player,PID,row,118)
assert(not ok and why == "unknown_cmd_PROJECT_PRODUCTION" and #requests == 1)
print("Royal Society command: strict refusal, two-argument request, stale/missing positions, missing command and request errors pass")
