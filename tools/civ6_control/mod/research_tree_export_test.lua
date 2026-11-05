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
assert(type(menus.research_tree) == "function", "production tree helper missing")

local rows = {
 [0] = { Index = 0, TechnologyType = "TECH_MINING" },
 [4] = { Index = 4, TechnologyType = "TECH_ROBOTICS" },
 [8] = { Index = 8, TechnologyType = "TECH_SMART_MATERIALS" },
 [9] = { Index = 9, TechnologyType = "TECH_HIDDEN" },
}
GameInfo = { Technologies = rows }
local nodes = {
 { TechType = 8, PrereqTechTypes = { 4, 0 } },
 { TechType = 0, PrereqTechTypes = {} },
 { TechType = 4, PrereqTechTypes = { 0 } },
 { TechType = 9, PrereqTechTypes = { 0 } },
}
Game = { GetTechs = function() return { GetActiveTechNodes = function() return nodes end } end }
local hidden = { [9] = true }
local techs = {
 IsTechRevealed = function(_, index) return not hidden[index] end,
 CanResearch = function(_, index) return index == 4 end,
}
local tree = assert(menus.research_tree(techs))
assert(#tree == 3 and tree[1].t == "TECH_MINING" and tree[2].t == "TECH_ROBOTICS")
assert(#tree[1].requires == 0 and tree[1].can_research == false)
assert(tree[2].can_research == true)
assert(tree[3].t == "TECH_SMART_MATERIALS" and tree[3].can_research == false)
assert(#tree[3].requires == 2 and tree[3].requires[1] == "TECH_MINING" and tree[3].requires[2] == "TECH_ROBOTICS")
-- An unrevealed prerequisite must not escape through a revealed successor.
hidden[4] = true
tree = assert(menus.research_tree(techs))
assert(#tree == 1 and tree[1].t == "TECH_MINING")
hidden[4] = nil
-- An unresolved prerequisite omits its entire row, never a shortened graph.
nodes[1].PrereqTechTypes = { 100 }
tree = assert(menus.research_tree(techs))
assert(#tree == 2)
-- Unknown legality is omitted rather than fabricated as false.
techs.CanResearch = function() error("unsupported") end
tree = assert(menus.research_tree(techs))
assert(tree[1].can_research == nil)
assert(menus.research_tree(nil) == nil)
techs.IsTechRevealed = function() error("unsupported") end
assert(menus.research_tree(techs) == nil)
Game.GetTechs = function() error("unavailable") end
assert(menus.research_tree(techs) == nil)
assert(source:find("research_tree = CivvisMenus.research_tree(ptechs),", 1, true), "tree observation is not wired to state")
print("native revealed research tree export checks passed")
