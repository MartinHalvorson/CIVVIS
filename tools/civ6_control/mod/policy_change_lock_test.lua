-- Native policy-change windows and the automatic slot filler.
CivvisK = CivvisK or { guard = function(fn) pcall(fn) end, attempt = function(fn) return (pcall(fn)) end }
local here = arg[0]:match("(.*)/[^/]*$") or ".";
local f = assert(io.open(here .. "/CivvisControlAgent.lua", "r"));
local source = f:read("*a"); f:close();
CivvisPolicy = {};
GlobalParameters = { GOVERNMENT_UNLOCK_WITH_FAITH = 0 };
local helpers = assert(source:match("(CivvisPolicy%.unlockObservation = function.-)\n%-%- Per%-city production"));
assert(loadstring(helpers))();
local cost, made = 0, false;
local calls = 0;
local culture = {
	GetCostToUnlockPolicies = function() return cost end,
	PolicyChangeMade = function() return made end,
	GetNumPolicySlotsOpen = function() return 1 end,
	GetNumPolicySlots = function() return 1 end,
	GetSlotPolicy = function() return -1 end,
	GetSlotType = function() return 1 end,
	IsPolicyUnlocked = function() return true end,
	IsPolicyObsolete = function() return false end,
	RequestPolicyChanges = function(_, clear, add)
		calls = calls + 1;
		assert(clear[1] == 0 and add[0] == 55);
	end,
};
local function window(want, reason)
	local ok, why = CivvisPolicy.changeWindow(culture);
	assert(ok == want and why == reason, tostring(why));
end
window(true, nil);
cost = 115; window(false, "policy_change_locked");
cost = 0; made = true; window(false, "policy_change_pending");
made = nil; window(false, "policy_change_unknown");
made = false;
for _, bad in ipairs({ -1, "0", false, 0/0, math.huge, -math.huge }) do
	cost = bad; window(false, "policy_change_unknown");
end
cost = nil; window(false, "policy_change_unknown");
cost = 0;
culture.GetCostToUnlockPolicies = function() error("unavailable") end;
window(false, "policy_change_unknown");
culture.GetCostToUnlockPolicies = function() return cost end;
culture.PolicyChangeMade = function() error("unavailable") end;
window(false, "policy_change_unknown");
culture.PolicyChangeMade = function() return made end;

-- Exercise the actual automatic filler, which is also the invalid-deck
-- fallback. A policy lock must prevent it from silently choosing another card.
local events = {};
local row = { Index = 1, Hash = 55, GovernmentSlotType = "SLOT_MILITARY" };
GameInfo = {
	Policies = setmetatable({ [1] = row }, { __call = function()
		local once = false;
		return function() if not once then once = true; return row end end;
	end }),
	GovernmentSlots = { [1] = { GovernmentSlotType = "SLOT_MILITARY" } },
};
local fillSource = assert(source:match("(local function fillPolicies%(player%).-)\nend\n"));
local chunk = assert(loadstring(fillSource .. "\nend\nreturn fillPolicies"));
setfenv(chunk, setmetatable({
	CivvisPolicy = CivvisPolicy, GameInfo = GameInfo,
	try = function(fn, fallback) local ok, v = pcall(fn); if ok then return v end; return fallback end,
	emit = function(kind, value) events[#events + 1] = { kind = kind, value = value } end,
}, { __index = _G }));
local fill = chunk();
local player = { GetCulture = function() return culture end };
cost = 115; assert(fill(player) == nil and calls == 0);
cost = 0; made = true; assert(fill(player) == nil and calls == 0);
made = nil; assert(fill(player) == nil and calls == 0);
made = false; cost = nil; assert(fill(player) == nil and calls == 0);
cost = 0; assert(fill(player) == "policies+1" and calls == 1);
assert(events[1].kind == "policy_change_deferred"
	and events[1].value.why == "policy_change_locked");
print("ok   native policy windows gate deck fallback and automatic slot filling");
