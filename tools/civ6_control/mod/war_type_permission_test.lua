local here = arg[0]:match("(.*)/[^/]*$") or "."
local function stub()
 return setmetatable({}, {
  __index = function() return stub() end,
  __call = function() return stub() end,
  __newindex = function() end,
 })
end
setmetatable(_G, { __index = function() return stub() end })
CivvisControlConfig = { Play = false }
assert(loadfile(here .. "/CivvisControlAgent.lua"))()
local war = rawget(_G, "CivvisWarDeclarations")
local allowed = { DIPLOACTION_DECLARE_SURPRISE_WAR = true, DIPLOACTION_DECLARE_FORMAL_WAR = false }
local diplomacy = {
 IsDiplomaticActionValid = function(_, action, target, testVisible)
  assert(target == 42 and testVisible == true)
  return allowed[action]
 end,
}
local function facts()
 local rows, byType = war.permissions(diplomacy, 42), {}
 local count = 0
 for statement in pairs(war.statements) do count = count + 1 end
 assert(#rows == count, "every supported type must remain present, even unknown")
 for index, row in ipairs(rows) do
  assert(war.statements[row.statement] ~= nil)
  assert(index == 1 or rows[index - 1].statement < row.statement, "stable export order")
  byType[row.statement] = row
 end
 return byType
end
local byType = facts()
assert(war.canDeclareAny(diplomacy,42) == true)
assert(byType.DECLARE_SURPRISE_WAR.allowed == true)
assert(byType.DECLARE_FORMAL_WAR.allowed == false, "aggregate true must not turn Formal War true")
assert(byType.DECLARE_HOLY_WAR.allowed == nil, "missing native fact is unknown")
allowed.DIPLOACTION_DECLARE_FORMAL_WAR = true
allowed.DIPLOACTION_DECLARE_SURPRISE_WAR = false
byType = facts()
assert(byType.DECLARE_FORMAL_WAR.allowed == true and byType.DECLARE_SURPRISE_WAR.allowed == false)
allowed.DIPLOACTION_DECLARE_FORMAL_WAR = 1
assert(facts().DECLARE_FORMAL_WAR.allowed == nil, "non-boolean is not permission")
diplomacy.IsDiplomaticActionValid = function() error("unavailable") end
for _, row in pairs(facts()) do assert(row.allowed == nil) end
diplomacy.IsDiplomaticActionValid = nil
for _, row in pairs(facts()) do assert(row.allowed == nil) end
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
local aggregate = assert(source:find("can_declare = CivvisWarDeclarations.canDeclareAny(diplomacy, otherId)",1,true))
assert(source:find("war_declarations = CivvisWarDeclarations.permissions(diplomacy, otherId)",aggregate,true),
 "typed facts must be wired into the actual rival record")
print("war type permissions: exact true/false/unknown, every type, stable order and export wiring passed")
