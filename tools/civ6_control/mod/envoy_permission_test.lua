-- Execute the shipped exporter, including false and unknown permission reads.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end
setmetatable(_G, { __index = function() return stub() end })
local chunk = assert(loadfile(here .. "/CivvisControlAgent.lua"))
local ok, err = pcall(chunk)
assert(ok, tostring(err))
local permission = rawget(_G, "CivvisCanSendEnvoy")
assert(type(permission) == "function", "shipped exporter is missing")

local function player(general, target)
	return { GetInfluence = function()
		return {
			CanGiveInfluence = function() return general end,
			CanGiveTokensToPlayer = function(_, id)
				assert(id == 42, "host player id must be passed unchanged")
				return target
			end,
		}
	end }
end
assert(permission(player(true, true), 42) == true)
assert(permission(player(false, true), 42) == false)
assert(permission(player(true, false), 42) == false)
assert(permission(player(false, false), 42) == false)
assert(permission(player(nil, true), 42) == nil)
assert(permission(player(true, nil), 42) == nil)
assert(permission(player(1, true), 42) == nil)
assert(permission(player(true, "false"), 42) == nil)
assert(permission({}, 42) == nil)
assert(permission({ GetInfluence = function() error("unreadable") end }, 42) == nil)
assert(permission({ GetInfluence = function() return {} end }, 42) == nil)

-- Ensure the tested helper actually feeds the minor export. The order arm
-- independently checks these same native permissions again before requesting.
local src = assert(io.open(here .. "/CivvisControlAgent.lua")):read("*a")
local export = assert(src:find("minors[#minors + 1] = {", 1, true))
local field = assert(src:find("can_send_envoy = CivvisCanSendEnvoy(player, mid)", export, true))
local nextField = assert(src:find("envoys = influence", export, true))
assert(field < nextField, "permission must be inside the minor export")
print("ok envoy permissions: native true/false, unknown fallback, export wiring")
