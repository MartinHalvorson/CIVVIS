# Independent defending district observations

Investigation in progress. No production mechanic or AI policy is changed.

The installed `DLC/Expansion2/UI/CityBanners/CityBannerManager.lua:693-705`
gives an Oppidum the defending-district banner and district strike callback.
At lines 2539-2546 that callback finds the district, selects it, and enters
`InterfaceModeTypes.DISTRICT_RANGE_ATTACK`.
`DLC/Byzantium_Gaul/Data/Byzantium_Gaul_Expansion2.xml:70` supplies
`<Row DistrictType="DISTRICT_OPPIDUM" AttackRange="2"/>` in `Districts_XP2`.

The current model's strike and combat-target lookup uses the Encampment family.
Oppidum keeps its Industrial Zone economic family. Seven fresh mirror cases
exercise a lone healthy Oppidum, two defending districts in one observed city,
independent depleted pools, a pillaged fort, a positive Encampment shot, and
preservation of the Industrial Zone family and both roster entries. Targets
lie outside city-center strike range. Shot assertions require both a legal
action and actual unit damage on a clone, rather than a future action's name.

The unchanged-production baseline is pending. Preserve all results, including
fixture or control failures; a static interface mismatch alone is not a
validated engine defect. A candidate must preserve two independent defense
and strike budgets without changing the Oppidum's economic family. Do not
ship this investigation as an implemented fix.

No native lane, private pin, installed mod, runtime settings or game process
is changed. No controlled native candidate games or win-rate gain are claimed.
