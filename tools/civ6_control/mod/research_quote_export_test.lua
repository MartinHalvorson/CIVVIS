-- Execute the shipped quote helper and export loop, not a parallel exporter.
-- Primary contract: TechTree.lua:1112/1114 costs and progress for every node.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local function stub()
 return setmetatable({}, {
  __index = function() return stub() end,
  __call = function() return stub() end,
  __newindex = function() end,
 })
end
setmetatable(_G, { __index = function() return stub() end })
assert(loadfile(here .. "/CivvisControlAgent.lua"))()
local menus = assert(rawget(_G, "CivvisMenus"))
assert(type(menus.research_quote) == "function", "production quote helper missing")
local cost, progress = 150, 70
local techs = {
 GetResearchCost = function(_, index) assert(index == 2); return cost end,
 GetResearchProgress = function(_, index) assert(index == 2); return progress end,
}
local row = { Index = 2, Hash = 987654, TechnologyType = "TECH_RADIO" }
local quote = assert(menus.research_quote(techs, row))
assert(quote.t == "TECH_RADIO" and quote.c == 150 and quote.p == 70)
-- Quotes already carry host-adjusted costs/progress: never apply a boost or
-- a speed multiplier in the bridge. Zero and progress above cost are facts.
cost, progress = 0, 0
quote = assert(menus.research_quote(techs, row))
assert(quote.c == 0 and quote.p == 0)
cost, progress = 10, 20
assert(menus.research_quote(techs, row).p == 20)
for _, bad in ipairs({ -1, math.huge, -math.huge, "unknown" }) do
 cost, progress = bad, 0
 assert(menus.research_quote(techs, row) == nil)
 cost, progress = 100, bad
 assert(menus.research_quote(techs, row) == nil)
end
cost, progress = 0/0, 0
assert(menus.research_quote(techs, row) == nil)
cost, progress = 100, 0/0
assert(menus.research_quote(techs, row) == nil)
assert(menus.research_quote(nil, row) == nil)
assert(menus.research_quote({}, row) == nil)
techs.GetResearchCost = function() error("unavailable") end
assert(menus.research_quote(techs, row) == nil)

-- Run the exact state-export loop with a completed tech, a boosted pending
-- tech, a locked pending tech, and an unsupported node. Non-current nodes
-- must cross too; HasBoost is not a discount to subtract again.
local rows = {
 { Index = 1, TechnologyType = "TECH_MINING" },
 { Index = 2, TechnologyType = "TECH_RADIO" },
 { Index = 3, TechnologyType = "TECH_ADVANCED_FLIGHT" },
 { Index = 4, TechnologyType = "TECH_FLIGHT" },
}
techs = {
 HasTech = function(_, index) return index == 1 end,
 HasBoostBeenTriggered = function(_, index) return index == 2 end,
 GetResearchCost = function(_, index)
  if index == 4 then error("unsupported node") end
  return index * 100
 end,
 GetResearchProgress = function(_, index) return index * 30 end,
 CanResearch = function() error("locked nodes must still be quoted") end,
}
local block = assert(source:match("(local techs, civics = %{%}, %{%};.-)\n\tlocal research, research_progress;"))
local env = setmetatable({
 try = function(fn, fallback) local ok, value = pcall(fn); if ok then return value end; return fallback end,
 player = { GetTechs = function() return techs end },
 CivvisMenus = menus,
 GameInfo = { Technologies = function()
  local i = 0; return function() i = i + 1; return rows[i] end
 end },
}, { __index = _G })
local export = assert(loadstring(block .. "\nreturn techs, boosted_techs, research_quotes"))
setfenv(export, env)
local completed, boosted, quotes = export()
assert(#completed == 1 and completed[1] == "TECH_MINING")
assert(#boosted == 1 and boosted[1] == "TECH_RADIO")
assert(#quotes == 2 and quotes[1].t == "TECH_RADIO" and quotes[2].t == "TECH_ADVANCED_FLIGHT")
assert(quotes[1].c == 200 and quotes[1].p == 60)
assert(quotes[2].c == 300 and quotes[2].p == 90)
assert(source:find("research_quotes = research_quotes,", 1, true), "quote list is not wired to the state event")
techs = nil
completed, boosted, quotes = export()
assert(#completed == 0 and #boosted == 0 and #quotes == 0)
print("native research quote export checks passed")
