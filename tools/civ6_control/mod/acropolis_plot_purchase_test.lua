-- Exercise the real menu exporter and purchase dispatcher against a fake host.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local function stub()
    return setmetatable({}, { __index = function() return stub() end,
        __call = function() return stub() end, __newindex = function() end })
end
setmetatable(_G, { __index = function() return stub() end })
Automation = { Log = function() end }
Game = { GetLocalPlayer = function() return 0 end, GetCurrentGameTurn = function() return 67 end }
GameInfo = setmetatable({ Districts = { DISTRICT_ACROPOLIS = { Index = 12 } } }, { __index = function() return stub() end })
CityCommandTypes = { PURCHASE = 1, PARAM_PLOT_PURCHASE = "plot", PARAM_X = "x", PARAM_Y = "y" }
CityCommandResults = { PLOTS = "plots" }
local prices, objects, hidden, denied = {}, {}, {}, {}
local requests, targetFailure, purchaseAllowed, priceFailure = 0, false, true, false
local function plot(index, x, suitable, owner)
    local object = { GetIndex = function() return index end, GetX = function() return x end,
        GetY = function() return 10 end, GetOwner = function() return owner or -1 end,
        CanHaveDistrict = function(_, district, player, city)
            assert(district == 12 and player == 0 and city == 7)
            return suitable
        end }
    objects[index] = object
    prices[index] = 60
    return object
end
plot(11, 30, true); plot(12, 31, false); plot(13, 32, true); plot(14, 33, true); plot(15, 34, true, 1)
hidden[13] = true; denied[33] = true
local targets = { 11, 12, 13, 14, 15, 11 }
local wallet = { GetPlotPurchaseCost = function(_, index)
    if priceFailure then error("price unavailable") end
    return prices[index]
end }
local city = { GetOwner = function() return 0 end, GetID = function() return 7 end,
    GetGold = function() return wallet end }
PlayersVisibility = { [0] = { IsRevealed = function(_, index) return not hidden[index] end } }
Map = { GetPlotByIndex = function(index) return objects[index] end,
    GetPlot = function(x, y)
        for _, object in pairs(objects) do if object:GetX() == x and object:GetY() == y then return object end end
    end }
CityManager = {
    GetCommandTargets = function(actor, command, parameters)
        assert(actor == city and command == 1 and parameters.plot == true)
        if targetFailure then error("targets unavailable") end
        return { plots = targets }
    end,
    CanStartCommand = function(actor, command, parameters)
        assert(actor == city and command == 1 and parameters.plot == true)
        return purchaseAllowed and not denied[parameters.x]
    end,
    RequestCommand = function(actor, command, parameters)
        assert(actor == city and command == 1 and parameters.x == 30 and parameters.y == 10)
        requests = requests + 1
    end,
}
assert(pcall(assert(loadfile(here .. "/CivvisControlAgent.lua"))))
local menus = assert(rawget(_G, "CivvisMenus"))
local apply = assert(rawget(_G, "CivvisApplyOrder"))
local rows = assert(menus.plot_purchases(city))
assert(#rows == 2 and rows[1].x == 30 and rows[1].g == 60 and rows[1].can_have_acropolis == true)
assert(rows[2].x == 31 and rows[2].can_have_acropolis == false)
assert(requests == 0, "observing plots must never submit a purchase")
prices[12] = -1; assert(#menus.plot_purchases(city) == 1)
prices[12] = math.huge; assert(#menus.plot_purchases(city) == 1)
prices[12] = 0/0; assert(#menus.plot_purchases(city) == 1)
prices[12] = 60
priceFailure = true; assert(#menus.plot_purchases(city) == 0); priceFailure = false
targetFailure = true; assert(menus.plot_purchases(city) == nil); targetFailure = false
local oldTargets = targets; targets = {}; assert(#menus.plot_purchases(city) == 0); targets = oldTargets
local player = { GetCities = function() return { FindID = function(_, id) if id == 7 then return city end end } end }
local function buy(verb)
    return apply(player, 0, { kind = "buy_plot", subject = "7", verb = verb, x = "30", y = "10" }, 67)
end
local ok, why = buy("GOLD_CAP=60.000000"); assert(ok and requests == 1, tostring(why))
prices[11] = 70; ok, why = buy("GOLD_CAP=60.000000"); assert(not ok and why == "plot_price_changed" and requests == 1)
prices[11] = 55; ok, why = buy("GOLD_CAP=60.000000"); assert(ok and requests == 2, tostring(why))
priceFailure = true; ok, why = buy("GOLD_CAP=60.000000"); assert(not ok and why == "plot_price_unknown" and requests == 2); priceFailure = false
purchaseAllowed = false; ok, why = buy("GOLD_CAP=60.000000"); assert(not ok and why == "cannot_buy_plot" and requests == 2); purchaseAllowed = true
ok, why = buy("bad-cap"); assert(not ok and why == "invalid_plot_price_cap" and requests == 2)
prices[11] = 0; ok, why = buy("GOLD_CAP=0.000000"); assert(ok and requests == 3, tostring(why))
local file = assert(io.open(here .. "/CivvisControlAgent.lua", "r")); local source = file:read("*a"); file:close()
assert(source:find("local menu = CivvisMenus.plot_purchases(city);", 1, true), "the observation must reach each exported city")
print("native plot observations and price-capped purchases: passed")
