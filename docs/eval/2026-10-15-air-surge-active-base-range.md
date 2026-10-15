# Plan Bomber campaigns from active airbase tiles

Computer: MacBook Pro

## Defect and native contract

`air_surge_bases` lists only owned city centers. The engine's existing
`air_capacity_at` also recognizes the actual tiles of active Aerodromes,
airstrips, and owned carriers. A forward field can bring a city into Bomber
range even when its parent city center cannot. The campaign appointment,
readiness, and retargeting all consult this incomplete base list.

Installed Gathering Storm source
`DLC/Expansion2/UI/CityBanners/CityBannerManager.lua:350` associates the banner
with the district's coordinates. Line 367 reads
`iAirCapacity = pDistrict:GetAirSlots();`, and line 368 reads its aircraft.
Lines 404-409 instead read an airstrip plot and its slots.
`Base/Assets/UI/UnitFlagManager.lua:399-414` reads a carrier unit's air slots
and aircraft. These are separate physical bases.

Native run `civvis-20261015T120650Z` reports Bombers stationed on the Aerodrome
at `(15,21)`, separate from Bogotá's center `(17,18)`. That game had four
Bombers by turn 165, an air campaign appointment at turn 189, and no city
dispositions before a rival science victory at 209. Its first nearby
Scythian Neapolis appeared on the same turn as that appointment. Earlier
known objectives were farther away. Neither the range-list omission nor a
campaign-priority gate has been established as the cause of that loss or
delay; this change targets the independently reproducible base-list defect.

## Regression under construction

The controlled board places an owned city center twelve hexes from the
objective and its Aerodrome ten hexes away. Two ordinary Bombers rebase
legally to that district. The campaign must recognize their objective as
reachable. Equivalent controls cover an owned airstrip and carrier;
pillaged and foreign bases supply no reach. City-center reach remains valid,
and one Jet Bomber cannot lend its range to an ordinary partner.

The baseline tests are not yet executed. No repaired native execution,
performance benefit, campaign capture, or win-rate improvement is claimed.
