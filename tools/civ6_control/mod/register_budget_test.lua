-- Civ VI's Lua (Havok Script) refuses a function that needs more than 200
-- registers, and says nothing: no Lua.log exists on this build. On pins
-- 2ddcd5889 and 66de612ed (G86, G88, 2026-10-04) a 20-item table built at the
-- agent's top level took its main chunk from 199 to 212 peak registers, and
-- the whole agent failed to load; every other context ran and the decider got
-- no board. Reference Lua 5.1 allows 250, so every other test passed.
--
-- This compiles every shipped mod script as the installer writes it (blank
-- and comment-only lines dropped, see install.installed_lua), dumps it, and
-- reads each function prototype's `maxstacksize` from the Lua 5.1 bytecode.
-- Any function at or past REGISTER_CAP fails here.
--
-- ⚠ The agent's main chunk sits close to the cap: ~190 top-level locals stay
-- live for the whole chunk. A new top-level `local`, or a table constructor
-- with many items at top level, spends that headroom.
--
-- Run: lua5.1 tools/civ6_control/mod/register_budget_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."
local REGISTER_CAP = 200
local SCRIPTS = {
	"CivvisControlAgent.lua", "CivvisControlAutoClose.lua", "CivvisControlHeartbeat.lua",
	"CivvisControlMapView.lua", "CivvisControlSetup.lua",
}

local function installed(text)
	if text:find("%[=*%[") then return text end
	local out = {}
	for line in (text .. "\n"):gmatch("([^\n]*)\n") do
		local trimmed = line:match("^%s*(.-)%s*$")
		if trimmed ~= "" and trimmed:sub(1, 2) ~= "--" then out[#out + 1] = line end
	end
	return table.concat(out, "\n") .. "\n"
end

-- A minimal Lua 5.1 bytecode walker: every prototype's first line and
-- maxstacksize, sized from the dump's own header.
local function prototypes(bytes)
	assert(bytes:sub(1, 4) == "\27Lua" and bytes:byte(5) == 0x51, "not Lua 5.1 bytecode")
	local little = bytes:byte(7) == 1
	local sizeInt, sizeT, sizeInstr, sizeNum = bytes:byte(8), bytes:byte(9), bytes:byte(10), bytes:byte(11)
	local pos = 13
	local function uint(n)
		local v = 0
		for i = 0, n - 1 do
			local b = little and bytes:byte(pos + i) or bytes:byte(pos + n - 1 - i)
			v = v + b * 256 ^ i
		end
		pos = pos + n
		return v
	end
	local function skipString() local n = uint(sizeT); pos = pos + n end
	local out = {}
	local function walk()
		skipString()
		local line = uint(sizeInt); uint(sizeInt)
		pos = pos + 3                       -- nups, numparams, is_vararg
		local maxstack = bytes:byte(pos); pos = pos + 1
		local codeSize = uint(sizeInt)        -- read before adding: `uint` moves `pos`
		pos = pos + codeSize * sizeInstr
		for _ = 1, uint(sizeInt) do
			local t = bytes:byte(pos); pos = pos + 1
			if t == 1 then pos = pos + 1
			elseif t == 3 then pos = pos + sizeNum
			elseif t == 4 then skipString() end
		end
		out[#out + 1] = { line = line, maxstack = maxstack }
		for _ = 1, uint(sizeInt) do walk() end
		local lineSize = uint(sizeInt)
		pos = pos + lineSize * sizeInt                -- lineinfo
		for _ = 1, uint(sizeInt) do skipString(); uint(sizeInt); uint(sizeInt) end
		for _ = 1, uint(sizeInt) do skipString() end
	end
	walk()
	return out
end

local failures = 0
for _, name in ipairs(SCRIPTS) do
	local handle = assert(io.open(here .. "/" .. name, "rb"))
	local text = installed(handle:read("*a"))
	handle:close()
	local chunk = assert(loadstring(text, name))
	local protos = prototypes(string.dump(chunk))
	local worst, main = protos[1], protos[1].maxstack
	for _, p in ipairs(protos) do if p.maxstack > worst.maxstack then worst = p end end
	if worst.maxstack >= REGISTER_CAP then
		failures = failures + 1
		print(string.format("FAIL %s: a function at installed line %d needs %d registers (cap %d)",
			name, worst.line, worst.maxstack, REGISTER_CAP))
	else
		print(string.format("ok   %s: main chunk %d, worst function %d registers of %d",
			name, main, worst.maxstack, REGISTER_CAP))
	end
end

-- The walker itself, on a function that plainly needs more than the cap: a
-- call with 210 arguments holds them all in registers at once. (A table
-- constructor does not: SETLIST flushes every 50 items.)
local over = prototypes(string.dump(assert(loadstring(
	"local f = print f(" .. string.rep("1,", 209) .. "1)"))))
if over[1].maxstack < REGISTER_CAP then
	failures = failures + 1
	print("FAIL the walker reads a 210-argument call at " .. over[1].maxstack .. " registers")
else
	print("ok   the walker reads a 210-argument call at " .. over[1].maxstack .. " registers")
end

if failures > 0 then
	print(string.format("\n%d check(s) failed", failures))
	os.exit(1)
end
print("\nall register-budget checks passed")
