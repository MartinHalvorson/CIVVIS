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
Production remains unchanged at this checkpoint. Complete local baseline
`67306` exits 101: all 379 existing mirror controls and one new unbuilt-fort
control pass; exactly seven new independent-health assertions fail. Two
existing mirror cases remain ignored. All 522 registered hashes are unchanged;
there are no fixture errors. Independent baseline CI `37927641062` confirms
city-state garrison 0 versus expected 70, then fail-fast leaves later new
Rust cases unscheduled.

All 85 discovered existing Lua suites pass. The first new suite has six
intended failures plus one visibility-fixture error: false visibility is
exported as absent. The corrected fixture also supplies the shipped Encampment
AttackRange 2 and reruns against unchanged production: 14 cases, seven passes
and seven intended missing-health/delta failures, no fixture errors.
The original failure log is retained. Production may now change after this
complete baseline proof. Unknown health is
tested separately from measured zero, matching the existing own-city conservative
fallback. Native input adoption and matched completed games remain unverified.
