local here = arg[0]:match("(.*)/[^/]*$") or "."
local file = assert(io.open(here .. "/CivvisControlAgent.lua"))
local source = file:read("*a"); file:close()
-- Execute the actual order handlers, including the legacy handler on an old
-- source copy. The fallback lets this fixture measure the unchanged baseline.
local first = source:find('\tif kind == "district_strike" then', 1, true)
    or assert(source:find('\tif kind == "encampment_strike" then', 1, true))
local last = assert(source:find('\tif kind == "war" then', first, true))
local section = source:sub(first, last - 1)

local function fixture()
    local state = { requests = {}, checked = {}, war = nil, allowed = true }
    local city = { GetOwner = function() return 0 end, GetID = function() return 31 end }
    local names = { "DISTRICT_ENCAMPMENT", "DISTRICT_OPPIDUM", "DISTRICT_CAMPUS",
                    "DISTRICT_IKANDA", "DISTRICT_THANH", "DISTRICT_CITY_CENTER" }
    local base, xp2 = {}, {}
    for index, name in ipairs(names) do
        local row = { Index = index, DistrictType = name }
        base[index], base[name] = row, row
        if index ~= 3 then xp2[name] = { AttackRange = 2 } end
    end
    local function district(kind, x)
        local value = { kind = kind, x = x, parent = city, complete = true, pillaged = false }
        value.GetType = function(self) return self.kind end
        value.GetID = function(self) return self.x + 100 end
        value.GetCity = function(self) return self.parent end
        value.IsComplete = function(self) return self.complete end
        value.IsPillaged = function(self) return self.pillaged end
        return value
    end
    state.enc, state.opp = district(1, 7), district(2, 13)
    state.at = { ["7:10"] = state.enc, ["13:10"] = state.opp }
    city.GetDistricts = function()
        return { Members = function()
            local i, values = 0, { state.opp, state.enc }
            return function() i = i + 1; if values[i] then return i, values[i] end end
        end }
    end
    local env = {
        try = function(callback, fallback)
            local ok, value = pcall(callback); if ok then return value end; return fallback
        end,
        GameInfo = { Districts = base, Districts_XP2 = xp2 },
        UnitOperationTypes = { PARAM_X = "x", PARAM_Y = "y" },
        CityCommandTypes = { RANGE_ATTACK = "range" },
        CityManager = {
            GetDistrictAt = function(x, y) return state.at[tostring(x) .. ":" .. tostring(y)] end,
            GetCity = function(pid, id) if pid == 0 and id == 31 then return city end end,
            CanStartCommand = function(actor, command, params)
                state.checked[#state.checked + 1] = { actor = actor, command = command, params = params }
                return state.allowed
            end,
            RequestCommand = function(actor, command, params)
                if state.throw then error("native request failed") end
                state.requests[#state.requests + 1] = { actor = actor, command = command, params = params }
            end,
        },
        CivvisLedger = { refuseWarStarter = function(actor, subject, verb, x, y, turn)
            state.warActor = actor; return state.war
        end },
    }
    setmetatable(env, { __index = _G })
    local chunk = assert(loadstring("return function(kind, subject, verb, x, y)\nlocal pid, turn = 0, 180\n"
        .. section .. "\nreturn false, 'unknown_order'\nend"))
    setfenv(chunk, env)
    return state, env.GameInfo, chunk()
end

local failures, cases = {}, 0
local function test(name, callback)
    cases = cases + 1
    local ok, err = pcall(callback)
    if ok then print("PASS " .. name)
    else failures[#failures + 1] = name; print("FAIL " .. name .. ": " .. tostring(err)) end
end

test("Oppidum order selects its district object rather than its city's Encampment", function()
    local s, _, apply = fixture()
    assert(apply("district_strike", 31, "13:10", 14, 10))
    local order = assert(s.requests[1]); assert(order.actor == s.opp)
    assert(order.command == "range" and order.params.x == 14 and order.params.y == 10)
    assert(s.checked[1].actor == s.opp and s.warActor == s.opp)
end)
test("same-city forts dispatch independently", function()
    local s, _, apply = fixture()
    assert(apply("encampment_strike", 31, "", 6, 10))
    assert(apply("district_strike", 31, "13:10", 14, 10))
    assert(#s.requests == 2 and s.requests[1].actor == s.enc and s.requests[2].actor == s.opp)
end)
for _, kind in ipairs({4, 5}) do
    test("legacy Encampment order selects replacement " .. kind, function()
        local s, _, apply = fixture(); s.enc.kind = kind
        assert(apply("encampment_strike", 31, "", 6, 10))
        assert(s.requests[1].actor == s.enc)
    end)
end
test("native command refusal produces no request", function()
    local s, _, apply = fixture(); s.allowed = false
    local ok, why = apply("district_strike", 31, "13:10", 14, 10)
    assert(not ok and why == "district_strike_refused" and #s.requests == 0)
end)
test("surprise-war refusal produces no request", function()
    local s, _, apply = fixture(); s.war = "would_declare_war:2"
    local ok, why = apply("district_strike", 31, "13:10", 14, 10)
    assert(not ok and why == s.war and #s.requests == 0)
end)
for _, fault in ipairs({"wrong city", "wrong owner", "missing parent", "incomplete", "pillaged"}) do
    test("rejects " .. fault .. " at source", function()
        local s, _, apply = fixture()
        if fault == "wrong city" then s.opp.parent = { GetOwner = function() return 0 end, GetID = function() return 32 end }
        elseif fault == "wrong owner" then s.opp.parent = { GetOwner = function() return 2 end, GetID = function() return 31 end }
        elseif fault == "missing parent" then s.opp.parent = nil
        elseif fault == "incomplete" then s.opp.complete = false
        else s.opp.pillaged = true end
        assert(not apply("district_strike", 31, "13:10", 14, 10)); assert(#s.requests == 0)
    end)
end
test("rejects an economic Campus at source", function()
    local s, _, apply = fixture(); s.opp.kind = 3
    assert(not apply("district_strike", 31, "13:10", 14, 10)); assert(#s.requests == 0)
end)
test("rejects City Center as a non-center district source", function()
    local s, _, apply = fixture(); s.opp.kind = 6
    assert(not apply("district_strike", 31, "13:10", 14, 10)); assert(#s.requests == 0)
end)
test("zero XP2 range overrides a base-table range", function()
    local s, info, apply = fixture()
    info.Districts[2].AttackRange = 2; info.Districts_XP2.DISTRICT_OPPIDUM.AttackRange = 0
    assert(not apply("district_strike", 31, "13:10", 14, 10)); assert(#s.requests == 0)
end)
test("missing source and malformed coordinates produce no request", function()
    for _, source in ipairs({"14:10", "13", "13:10:1", "13.0:10", "-13:10", ""}) do
        local s, _, apply = fixture()
        assert(not apply("district_strike", 31, source, 14, 10)); assert(#s.requests == 0)
    end
end)
test("missing target produces no request", function()
    local s, _, apply = fixture()
    assert(not apply("district_strike", 31, "13:10", nil, 10)); assert(#s.requests == 0)
end)
test("native request exception is a refusal", function()
    local s, _, apply = fixture(); s.throw = true
    local ok, why = apply("district_strike", 31, "13:10", 14, 10)
    assert(not ok and why == "district_strike_throw" and #s.requests == 0)
end)
print("RESULT " .. cases .. " cases; " .. #failures .. " failed")
assert(#failures == 0, "defending district strike failures: " .. table.concat(failures, ", "))
