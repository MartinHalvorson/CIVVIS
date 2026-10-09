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


The candidate observes raw defense pools for visible completed defending
non-city-center districts, using the shipped `AttackRange` rows to include
Encampment replacements. A health-only change joins the tile signature and
emits a delta. Fog and failed getters retain prior observed pools; new
ownership, district type or a changed visible district ID discards old health.
The first candidate's added rebuilt-district test caught a stale cache entry;
the cache is now cleared, with its original failure retained.

Foreign Encampment-family districts import garrison damage on the existing
0..100 remaining-health scale, outer health as its measured remainder, and
pillage state. A measured zero outer capacity stays zero. Missing/failed
observations keep the existing own-city conservative fallback. The tile
snapshot and exporter retain actual observations across fog. Own city roster
health and engine mechanics are unchanged.

All 86 discovered Lua5.1 suites pass, including 19 new export cases. All 91
Lua scripts compile. The original eight Rust cases and their health/damage assertions are retained; the
physical-shot fixture now makes the defending owner current before applying
its shot. Baseline failures precede that stage. Main `9a2b0dfe6` was merged
once at `e47d0a14a` before candidate validation. Complete local candidate
mirror/Rust validation and final published-head checks are pending.


The first published candidate `ebe0454f8` fails compilation with E0624: the
mirror called the private `district_is_family` helper. The corrected code uses
existing crate-visible `district_family`, including replacement chains; no
engine visibility or rule changed. CI `37929854240` executes no Rust cases.
Owned local focus `61466` is intentionally stopped only after that concrete
compiler failure, Cargo SIGINT -2 (wrapper 254), with no tests credited.
Its original logs and source registrations remain preserved. Lua gate passed.
The initial focused source pathspec omitted root Rust files; a supplemental
clean committed-head manifest covers all 619 tracked source/input files.
Corrected validation will use whole-directory discovery from the beginning.


Corrected candidate `778e40f03` compiles and passes all seven other new cases
in CI `37930377135`, including the actual legal Encampment shot. Its legacy
unknown-pool case fails outer health 0 versus final city maximum 200: foreign
infrastructure is applied before city-banner facts set the fallback maximum.
The shared city-metrics step now resolves foreign defenses again after those
facts, on both rebuild and persistent sync; explicit measured zero still wins.
An added persistent-sync test covers missing pools, a changed city maximum,
measured zero, destruction and partial damage. The original eight cases and
assertions are retained. CI later coverage is unscheduled after fail-fast.

The obsolete owned local focus `91931` is stopped only after this concrete
independent failure, Cargo SIGINT -2/wrapper 254, with no local Rust cases
credited. Its 619 registered hashes are unchanged. Dependent full runner
`76448` exits 1 without starting Cargo when its focused prerequisite fails.
Both original logs remain. Cost `37930377190` passes: -0.05% median over five
pairs/600 turns per arm, inside the ±1% noise floor and +8% budget; IQR 0.28pp
[-0.28%, +0.09%], resolution ±0.19%, pooled -0.11%. No speed gain is claimed.
