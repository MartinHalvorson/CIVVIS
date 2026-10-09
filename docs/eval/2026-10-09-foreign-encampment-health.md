# Foreign Encampment health observations

This task investigates a public reconstruction gap; no isolated native winning
effect has been established. The completed external game
`civvis-20261009T103858Z` records 18 native Encampment combats, including one
zero-damage second Bomber attack in frame 184f0. Its private controller differs
substantially, and the public cause of that attack is unproven.

The shipped primary API is
`DLC/Expansion2/UI/CityBanners/CityBannerManager.lua:715-718`:
`GetMaxDamage` and `GetDamage` for `DefenseTypes.DISTRICT_GARRISON` and
`DefenseTypes.DISTRICT_OUTER`. Before this change, plot export carried district location,
completion and pillage but no defense health. Rival city records omit their
district roster. The former foreign infrastructure path installed the
district without assigning its independent health; the danger reader requires
positive Encampment garrison and outer health to include its shot.

The initial checkpoint added reproduction fixtures and test registration only.
Production remained unchanged for the complete baseline. Complete local baseline
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
The original failure log is retained. Production changed only after this
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
once at `e47d0a14a` before candidate validation. Validation below records
the final corrected implementation.


The first published candidate `ebe0454f8` fails compilation with E0624: the
mirror called the private `district_is_family` helper. The corrected code uses
existing crate-visible `district_family`, including replacement chains; no
engine visibility or rule changed. CI `37929854240` executes no Rust cases.
Owned local focus `61466` is intentionally stopped only after that concrete
compiler failure, Cargo SIGINT -2 (wrapper 254), with no tests credited.
Its original logs and source registrations remain preserved. Lua gate passed.
The initial focused source pathspec omitted root Rust files; a supplemental
clean committed-head manifest covers all 619 tracked source/input files.
Both subsequent validations used whole-directory discovery from the beginning.


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


Corrected ordered candidate `9e4387a03` passes all 388 local mirror cases,
including all nine new cases, the actual legal Encampment shot and the shared
persistent-sync regression. Two existing cases remain ignored. Local focus
`88412` exits 0 with all 619 registered source/input hashes unchanged.
The dependent required full local Cargo run `37351` starts only after this
exact passing receipt, with clean unchanged source. It exits 101: 4,475 library
cases pass, one unchanged spectator timing case fails, and 51 existing cases
remain ignored. Later targets are not run. `two_viewers_each_see_every_turn`
requires four consecutive turns inside its two-second watch; the slow reader
sees [2, 3, 4]. The entire test file is byte-identical to merged main
`9a2b0dfe6` (SHA256 4244fa8b89775e3b1289c51a8babccd4d8e8eb8af6fa4b6d3108c135c559d849).
Exact isolated run `98093` passes that one test in 3.06 seconds, with all 619
source/input hashes unchanged. The original full failure is retained.
The required unfiltered process-isolated Nextest run `68306` executes all
4,758 cases successfully across all five target binaries, with one worker,
zero retries and 51 existing skips, in 496.848 seconds. Both spectator timing
cases pass in the complete run. No source, assertion or runtime setting changes.

The first Nextest wrapper exits 1 because its receipt regex omitted the
optional `(1 slow)` annotation; Nextest itself exits 0. Its immutable log
contains all 4,758 PASS lines, nine new cases and all 388 mirror controls.
The original parser receipt remains preserved. Correcting the external parser
and verifying that log does not rerun any Rust case. Documentation run
`64821` then exits 0, with zero failures and four existing ignored examples.
All 619 registered source/input hashes remain unchanged through both runs.
The combined receipt records the actual passing full validation.

Commands:

```text
cargo nextest run --cargo-profile ci --locked --test-threads 1 --retries 0 --no-fail-fast --no-tests fail --status-level pass --failure-output immediate-final
cargo test --profile ci --locked --doc
```

Independent CI `37932473961` on the same `9e4387a03` head passes all 4,758 Rust
tests, with 51 existing skipped cases and four ignored documentation examples.
All 388 mirror cases, all nine new cases, all 53 siege controls and all 35
preservation controls pass; the overlapping tournament/provenance sets of
71, 21 and 22 also pass. Quality, collaboration, overwrite, mod, publication
and security checks pass.

Ordered cost `37932473942` passes: -1.72% median over five pairs and 600 turns
per arm, IQR 1.30pp [-2.09%, -0.27%], resolution ±0.86%, pooled -1.48%, within
the +8% budget. This is simulator CPU cost on the ordinary six-player cost
profile; it does not establish native export cost or a Gran Colombia victory
effect.

Receipts and original logs are retained under
`civvis-tactics-results/2026-10-08/public-campus-finish-before-defender/`:
`foreign-encampment-health-ordered-candidate-focused-receipt.json`,
`foreign-encampment-health-ordered-candidate-full-rust-receipt.json`,
`foreign-encampment-health-local-spectator-failure-receipt.json`,
`foreign-encampment-health-spectator-isolated-receipt.json`,
`foreign-encampment-health-process-isolated-full-receipt.json`,
`foreign-encampment-health-ordered-candidate-rust-ci-receipt.json`, and
`foreign-encampment-health-ordered-candidate-cost-ci-receipt.json`, alongside
the unchanged baseline, first compile failure, corrected ordering failure and
Lua fixture/cache failure records. No source assertion was weakened to clear
a gate. All manual checkpoints carry `Computer: MacBook Pro`.

Public integration and native adoption are separate facts. No native pin,
private source, installed mod, native process or runtime setting was changed.
A matched completed Domination trial and an isolated win-rate effect remain
unverified.
