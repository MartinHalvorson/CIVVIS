# Aluminum for the first Domination Bomber wing

This is a simulator source comparison and a regression record. It is not
native Civilization VI verification or evidence of consistent Domination wins.

## Registered comparison

PR #3802 started from `2bd913cad94749b554b9c5e9091bc096b4f2574b`.
Eight fresh seeds, `38020000` through `38020007`, were registered before
policy and campaign outcomes. The separate known seed `37930100` is diagnostic
and is excluded from fresh strength claims.

Both libraries use the same Prince player/barbarian, Simón/Gran Colombia focal
explicit Domination, four-major/six-minor Tiny Pangaea, Online250, all-victories
profile. The same 21 forced focal rows have SHA256
`439757684071b807eb4151e055b4a9adf6ac24b7b8b6c0b5da7327b5344dcea0`.
`beeline-orders-by-value` stays at its compiled default off. Strike Reach on
is the primary matched source comparison; every off arm and loss is retained.
Rivals are adaptive deployed CIVVIS controllers, not Firaxis AI. They receive
no forced focal rows. The allocation guard requires explicit Domination;
the corrected fuel forecast also applies to any other enabled air-surge
controller, so source comparisons must not be described as an exclusively
focal change.

Frozen policy source: `11560dbe0c255030a02c2a5866a5d653d50dac15`.
`11ee24c70` only removes three needless borrows in the tests.
No policy changes or partial-outcome inspection are allowed within the block.
The read-only observer records full contiguous canonical actions, Bomber class
including Jets, city queues/ownership/Loyalty, gross Aluminum rate, stock and
shortages, and unit fuel maintenance/free upkeep/formation. Observer SHA256:
`1ee0e84f3fd9ae54b5d43d51d82b4895cd0f85826dcd47870d2f696a6412c04d`.

## Actual regression and resulting behavior

Failed-first checkpoint `d04384872d274c24eb68cb741e51255e9bea2415` contains
three regressions. Both direct and observed full `Ai::take_turn` dispatchers
upgrade all four Cavalry to Helicopters, spend Aluminum 8 down to 4, and commit
zero Bombers despite two legal productive Aerodromes. The other test launches
an actual resource-processing deficit across the speed-scaled grace period;
the old forecast accepts a bank which `EndTurn` demonstrably exhausts.

The candidate buys one Bomber and commits one production order while retaining
the four Cavalry. The engine completes the remaining aircraft, and both planes
survive 14 resource ticks without Aluminum shortages. These tests use actual
unit placement, production commitments and maintenance processing, not a
mirror of the forecast.

The discretionary guard follows the legal resource bill through the shared
upgrade ranking, named offensive upgrades, conversion and late upgrade passes,
and unit production/purchase scores. A named upgrade package rechecks after
every applied upgrade. The direct air package also honors the same reservation.
Standing and queued other fuel users reduce the wing budget; negative income
is retained for the bank calculation. Queued/live Bombers and Jets fulfill the
training commitment instead of charging for the same wing again.

Spare fuel still supports ground modernization. Two Aluminum sources fund
exactly two Helicopter upgrades while retaining a two-plane sustainable wing;
a pending Helicopter queue reduces that to one. Free upkeep and Oil upgrades
retain their own budgets. Disabled air policy, other victory targets, missing
technology/airfield, expired launch clock, and an immediate city threat retain
the shared canonical upgrade actions exactly. Existing queues are preserved.

## Validation and campaign outcomes

- Focused regression/control suite: 12 passed.
- Full `cargo test --profile ci --locked`: 4383 passed, 0 failed, 53 ignored.
- Changed-line Rust quality: passed after test-only needless-borrow cleanup.
- Prototype campaigns: all 36 source/arm runs completed and full action counts
  were verified contiguous. In the primary eight Strike Reach on games,
  Domination wins stayed 0/8; other wins went 0/8 to1/8 (Score). Observed major
  cities held across games went 10 to 16, entirely in seed 38020001. Seven games
  still captured none. Four games produced a two-plane wing in the parent;
  five did in the candidate. This is not consistent Domination success.
  Off arms and the known diagnostic remain in the external analysis; some
  first action differences are rival actors, as expected from the shared
  forecast correction. The known on diagnostic still has no aircraft.
- Native game: no root UI, process, OS, runtime or orders-database mutation.
  Verification remains with the active game owner at a suitable boundary.

The prototype misses an integration requirement: two physically produced
Bombers plus four retained Cavalry still leave a newly appointed plan in Arm
after its readiness review, because the chosen capture body is Helicopter.
The failed-first test is retained before refining this escort choice. The
prototype required an escort refinement. Eight separate fresh seeds 38021000–38021007
were registered at 03:32:56UTC before refinement. They will be reported as a
separate block, preserving the prototype's null and negative results.

The refinement chooses healthy held capture cavalry when spending for the
preferred successor would displace the wing. New appointment, standing
census, launch estimate and existing appointment review use the same choice.
Ready Helicopters and spare supply keep the original choice. The real-engine
integration now produces two aircraft, legally rebases one forward, declares
with the retained Cavalry, and executes spotting before the volley and capture
of a weakened city. Focused suite: 16 passed; full suite 4,387 passed, 0 failed,
53 ignored; changed-line quality and the explicit library/native-CLI build
passed. No native result is inferred from this fixture.

## Separate escort refinement block

Policy source `4da495555e652bd6c95c2f72e6268db34197f2a7` was frozen at 03:43:10UTC.
All runs were terminal by03:55:36UTC before inspection. The observer's first
launch failed opening absent external trace directories before play; logs and
the failure receipt were retained. Creating those directories allowed the same
registered seeds, profile and binaries to run. No seed or policy was changed.
The known parent diagnostic reuses its already completed exact frozen parent
library/executable and action stream. Every registered arm is retained and
its full contiguous action count agrees with its outcome receipt.

| Eight fresh primary games | Parent | Refined allocation |
| --- | ---: | ---: |
| Domination wins | 0 | 0 |
| Any wins | 0 | 0 |
| Major cities ever observed held, summed across games | 5 | 6 |
| Foreign capitals ever observed held | 0 | 0 |
| Games with two standing Bomber-class aircraft | 4 | 4 |
| Observed aircraft losses | 6 | 7 |
| Observed Aluminum shortage turns | 39 | 35 |
| Applied Air Strikes | 166 | 174 |
| Applied ground / air pillages | 5 / 140 | 5 / 142 |
| Own capital lost at end | 0 | 0 |

The extra major city is concentrated in 38021000, which still loses to Science
at 245 rather than 244. It ends with zero aircraft rather than one. Seven games
still hold no foreign major city. This does not establish consistent conquest
or a stronger Domination win rate. Off arms keep 14 observed major cities, one
foreign capital and seven aircraft losses in each source; neither wins.
Twelve of 16 fresh source-matched canonical action streams are identical. The
first primary difference in 38021000 is a rival actor's queue; the other changed
primary game 38021006 first differs in the focal Spy assignment versus Denounce.
The known on diagnostic remains without aircraft and loses by Score at 250;
its observed major cities go 0 to 1, with a rival first difference. It remains
excluded from strength claims.

The bounded repair is supported by failed-first real dispatcher, production,
resource-processing and launch/capture regressions. These campaigns provide
null/negative strength evidence alongside those correctness results. Thought
journal rollover and truncation counters are retained; complete canonical
actions do not imply a lossless reasoning journal.

## Independent observed-income limitation

A separate external read-only fixture against the frozen parent exposes another
supply obstacle: a met city-state with one non-center Aluminum mine grants the
observer +2 Aluminum in the world, but its decision view reports 0. Both boards
agree on Suzerain 0, have equal stockpiles, and can see the Aluminum mine. The
view's public city retains only the center in `owned_tiles`, so derived source
income omits the improved surrounding tile. This is independent of rival Amani
and remains outside this AI allocation change. The game owner has the exact
probe and shipped the view repair separately in #3804, merge
`871b872c4d55ff92b8acf6494441bf22ac20062f`. The registered libraries were frozen
before that change and do not contain it. These comparisons cannot attribute
any gain to the income repair or establish the final merged tree's win rate.

## Final combined-tree validation

Merged `origin/main` at `871b872c4d55ff92b8acf6494441bf22ac20062f` once,
without conflicts. Validation source `23f3a0734e602d908a71c2e1b1854076f50ea7a6`:
`cargo test --profile ci --locked` passed 4,405 tests, 0 failed, 53 ignored;
changed-line Rust quality and the explicit library/native-orders CLI build
passed. The owned allocation policy and tests are identical to the frozen
refinement source; subsequent changes record evidence only.

The original independent public-income probe now reports world +2/view +2,
with equal stock/Suzerainty and the same redacted center-only foreign city.
This confirms the separate upstream projection repair on the combined tree.
It is not a native gameplay or new campaign-strength result. Native startup
remains with the game owner and no native Bomber sortie has been verified by
this task.

## Reproducible artifacts

Protocol, source and artifact freezes, full actions/reasoning traces, both
libraries and observer executables, and build/validation logs are retained at:
`/Users/martbot-mbp-m5-max-128/civvis-tactics-results/2026-09-27/air-aluminum-allocation/`.

Parent library SHA256:
`bffb3338dbfc54cfbb3646d8b7cc79204cde1c49617403aa23bab16b1680ea55`.
Candidate library SHA256:
`1f8186b33ee34a51aa4100fadc655cf0a709707c5248a108aa49a2e69a100cf3`.
Parent observer executable SHA256:
`b6ed6ebc2c2e22b8322a510ea3d4d56163e4b9cb56c9bb620d75c31c61f2c880`.
Candidate observer executable SHA256:
`822fb3a99074c63ca1909af26e1bd8c96896d8343d0d4066460e7288ee644a9d`.

Refined library SHA256:
`8e44cec67b044a69ed545bde260bfe8abd1de23f844dd530f8d1e73397ebeebc`.
Refined observer executable SHA256:
`8133b7bcb06c7aeea2e2bd1a8a4822d31760895b0f5a8bc84d373328cad1abd7`.
