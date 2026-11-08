-- Run the real observer against the Band component used by the shipped panel.
local here = arg[0]:match("(.*)/[^/]*$") or "."
local function stub()
    return setmetatable({}, {
        __index = function() return stub() end,
        __call = function() return stub() end,
        __newindex = function() end,
    })
end
setmetatable(_G, { __index = function() return stub() end })
assert(loadfile(here .. "/CivvisControlAgent.lua"))()
local read = rawget(_G, "CivvisRockBandProgress")
assert(type(read) == "function")
GameInfo = { Units = {
    [1] = { UnitType = "UNIT_ROCK_BAND" },
    [2] = { UnitType = "UNIT_WARRIOR" },
} }
local function unit(kind, level, sales)
    return {
        GetType = function() return kind end,
        GetExperience = function() error("generic promotion level is not a Band statistic") end,
        GetRockBand = function() return {
            GetRockBandLevel = function() return level end,
            GetAlbumSales = function() return sales end,
        } end,
    }
end
local band = read(unit(1, 1, 350))
assert(band.rock_band_level == 1 and band.album_sales == 350)
band = read(unit(1, 0, 0))
assert(band.rock_band_level == 0 and band.album_sales == 0, "zero counters are observations")
local other = unit(2, 9, 900)
other.GetRockBand = function() error("non-Band must not access the component") end
assert(read(other) == nil)
local missing = unit(1, 1, 100)
missing.GetRockBand = function() error("unavailable component") end
band = read(missing)
assert(band.rock_band_level == nil and band.album_sales == nil)
local partial = unit(1, 2, 100)
partial.GetRockBand = function() return {
    GetRockBandLevel = function() error("unreadable level") end,
    GetAlbumSales = function() return 250 end,
} end
band = read(partial)
assert(band.rock_band_level == nil and band.album_sales == 250)
for _, bad in ipairs({ -1, 1.5, math.huge, "2" }) do
    band = read(unit(1, bad, bad))
    assert(band.rock_band_level == nil and band.album_sales == nil)
end
band = read(unit(1, 0 / 0, 0 / 0))
assert(band.rock_band_level == nil and band.album_sales == nil)
local concert = rawget(_G, "CivvisOnRockBandConcert")
assert(type(concert) == "function")
Game = { GetLocalPlayer = function() return 0 end, GetCurrentGameTurn = function() return 42 end }
GameInfo.Unit_RockbandResults_XP2 = {
    [4] = { Dies = true, GainsLevel = false, ExtraPromotion = false },
}
Players = setmetatable({}, { __index = function() error("retired unit must not be resolved") end })
local emitted = {}
local realPrint = print
print = function(line)
    if type(line) == "string" and line:find('"kind":"rock_band_concert"', 1, true) then
        emitted[#emitted + 1] = line
    end
end
concert(1, 7, 4, 5, 4, 750)
assert(#emitted == 0, "foreign concerts are not our private ledger")
concert(0, 7, 4, 5, 4, 750)
assert(#emitted == 1 and emitted[1]:find('"tourism":750', 1, true))
assert(emitted[1]:find('"dies":true', 1, true))
assert(emitted[1]:find('"gains_level":false', 1, true))
concert(0, 8, 4, 5, 99, 0)
assert(#emitted == 2 and emitted[2]:find('"tourism":0', 1, true))
assert(not emitted[2]:find('"dies":', 1, true), "unknown results stay unknown")
print = realPrint
print("all native Rock Band progress and concert checks passed")
