-- The leader screen's closer beside Quick Deals (CivvisControlAutoClose.lua,
-- the chain before the shipped screen loads). Quick Deals (Workshop 2460661464)
-- replaces DiplomacyActionView at LoadOrder 99999 and its file includes the
-- stock chain itself (diplomacyactionview_qd.lua:4-16) and caches
-- `QD_BASE_LateInitialize` (:21); a hotseat install now sits above it
-- (install.py HOTSEAT_LOAD_ORDER), so the closer must load it, not drop it.
-- Run: lua5.1 tools/civ6_control/mod/quick_deals_chain_test.lua
local here = arg[0]:match("(.*)/[^/]*$") or "."
local handle = assert(io.open(here .. "/CivvisControlAutoClose.lua", "r"))
local source = handle:read("*a")
handle:close()
local block = assert(source:match("\n(%-%- ★ QUICK DEALS%..-\nif CHAINED%[NAME%] and type%(QD_BASE_LateInitialize%) ~= \"function\" then\n.-\nend\n)"),
	"the Quick Deals chain sits before the shipped screen loads")
assert(source:find(block, 1, true) < source:find("if not haveScreen() then pcall(function() include(NAME); end); end", 1, true))

local function load(name, files)
	local loaded = {}
	local env = setmetatable({ NAME = name, CHAINED = {
		DiplomacyActionView = "DiplomacyActionView_Expansion2", WonderBuiltPopup = "Suk_WonderBuiltPopup" } },
		{ __index = _G })
	env.include = function(file)
		loaded[#loaded + 1] = file
		local defines = files[file]
		if defines then defines(env) end   -- an include that finds no file does nothing
	end
	local chunk = assert(loadstring(block, "quick deals chain"))
	setfenv(chunk, env)
	chunk()
	return loaded, env
end
-- Quick Deals installed: its file, which brings the stock chain itself; nothing on top of it.
local qd = { diplomacyactionview_qd = function(env)
	env.QD_BASE_LateInitialize = function() end
	env.OnDiplomacyStatement = "quick deals"
end }
local loaded, env = load("DiplomacyActionView", qd)
assert(#loaded == 1 and loaded[1] == "diplomacyactionview_qd", table.concat(loaded, ","))
assert(env.OnDiplomacyStatement == "quick deals", "Quick Deals' handler is not overwritten by the stock chain")
-- Without it: the Expansion2 chain, as before.
loaded = load("DiplomacyActionView", {})
assert(#loaded == 2 and loaded[1] == "diplomacyactionview_qd" and loaded[2] == "DiplomacyActionView_Expansion2")
-- Every other screen: its own chain only.
loaded = load("WonderBuiltPopup", qd)
assert(#loaded == 1 and loaded[1] == "Suk_WonderBuiltPopup")
loaded = load("HistoricMoments", qd)
assert(#loaded == 0)

print("quick_deals_chain_test: ok")
