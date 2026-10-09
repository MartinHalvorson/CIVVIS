# Foreign Encampment health observations

This task investigates a public reconstruction gap; no isolated native winning
effect has been established. The completed external game
`civvis-20261009T103858Z` records 18 native Encampment combats, including one
zero-damage second Bomber attack in frame 184f0. Its private controller differs
substantially, and the public cause of that attack is unproven.

The shipped primary API is
`DLC/Expansion2/UI/CityBanners/CityBannerManager.lua:715-718`:
`GetMaxDamage` and `GetDamage` for `DefenseTypes.DISTRICT_GARRISON` and
`DefenseTypes.DISTRICT_OUTER`. Plot export currently carries district location,
completion and pillage but no defense health. Rival city records omit their
district roster. The reconstruction's foreign infrastructure path installs the
district without assigning its independent health; the danger reader requires
positive Encampment garrison and outer health to include its shot.

The initial checkpoint adds reproduction fixtures and test registration only.
Production remains unchanged. Baseline results are pending; production changes
must wait for completed, assertion-level baseline proof. Unknown health is
tested separately from measured zero, matching the existing own-city conservative
fallback. Native input adoption and matched completed games remain unverified.
