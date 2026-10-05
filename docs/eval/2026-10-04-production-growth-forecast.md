# Early Granary reservations priced by Production return

The projected-return reservation fails the early-production goal. Fresh
confirmation lowers mean Production at turns 75 and 100 on both Emperor and
Deity. No confirmation pair improves Production at turn 75. Reject the runtime
prototype; preserve its source, fixtures, frozen artifacts and every game result.

## Frozen comparison

Control runtime is `afba14116415131f730a462dff4e1d7811aced93`; the report-only
merge `c351dca598cbe30abef98ed4b3405e8b3cf7aba9` has identical runtime inputs.
Candidate runtime inputs are frozen at `50594d440`. Both binaries link CI-profile
libraries with the same compiler and the same standalone probe. The manifest
records library, dependency, binary, harness and ledger hashes. Embedded Git
build identity may name the preceding checkpoint; the runtime inputs match the
recorded source commit.

Four majors, 60×38 Pangaea, six city-states, Online, 150-turn cap, all victories,
barbarians, Gran Colombia explicitly targeting Domination, only focal seat 0
handicap-exempt. Rivals use the same adaptive `AdvancedAi::fleet` policy. Each arm
loads the same deployed gene ledger. No opt-in Granary policy is enabled. The new
runtime guard applies to explicit Domination seats; only the focal seat qualifies
in this harness. Rival outcomes still respond to the focal seat's changed play.

Pilot: Emperor `61006400..61006403`, Deity `61006500..61006503` (eight pairs).
Fresh confirmation: Emperor `61006600..61006611`, Deity `61006700..61006707`
(twenty pairs). The policy and binaries stayed fixed through confirmation.
The numeric acceptance criteria were recorded before candidate play. The decision
JSON evaluates the preregistered conditions without tuning after the result.

## Prototype

An idle city can reserve one Granary only when the candidate's additional empire
Production over sixty cost-scaled Standard turns (thirty Online) returns at least
110% of its actual remaining bill. Independent board clones use the existing
citizen assignment, consumption, housing and amenity calculations, advancing
known timed effects at the next-turn boundary. Growth precedes build completion,
as in the engine. Other city populations, research, borders, improvements and
actions remain fixed. Empire Production includes amenity reallocation effects on
other cities. This approximation does not price the best alternative military or
expansion action or simulate future attacks.

The reservation is early explicit Domination only, at least two cities, no
Recovery, named threat, recent attack, low loyalty, negative amenities or negative
net income. It requires fielded land defenders at least equal to the city count;
queued units do not count. It skips visible hostile military within three tiles,
existing queues and an active Granary elsewhere, and preserves the capital's
Expansion Settler need. Higher-priority survival, research, growth and income
reservations remain ahead of this hook. Ordinary production bids still run if
the forecast fails. No engine rules or yields change.

## Fresh confirmation

Means are across seeds with a checkpoint in both arms, retaining observed
eliminated seats at zero cities/Production. Completed world games have no later
checkpoint: they are reported as missing, without forward filling. Every final
winner, alive state and ending turn is retained. Late smaller samples cannot
establish the early-production goal.

| Difficulty | Turn | Matched pairs | Production control → candidate | Change | Cumulative P change | Science change | Culture change | P / strongest rival control → candidate |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| Emperor | 75 | 12 | 58.100 → 56.558 | -2.65% | -1.32% | -7.16% | -0.29% | 0.497 → 0.481 |
| Emperor | 100 | 11 | 87.664 → 84.609 | -3.48% | -2.51% | -3.97% | +0.05% | 0.475 → 0.463 |
| Deity | 75 | 8 | 45.275 → 43.925 | -2.98% | +1.08% | -1.55% | -15.21% | 0.225 → 0.235 |
| Deity | 100 | 7 | 66.257 → 63.986 | -3.43% | -0.55% | +9.34% | -0.83% | 0.182 → 0.198 |

Emperor turn 75 has zero higher / two lower / ten equal pairs; Deity has zero
higher / two lower / six equal. Turn 100 is two higher / four lower / five equal
on Emperor, and one higher / one lower / five equal on Deity. Deity's relative
Production ratio rises despite lower focal Production; changed rivals are part
of the resulting games. This does not establish faster focal Production or AI
parity. Deity military power falls 7.34% at turn 100. Emperor Science falls 7.16%
at turn 75; Deity Culture falls 15.21% there.

Confirmation outcomes: no focal wins in either arm, no Emperor eliminations,
and one Deity elimination in each arm. The pilot has unchanged Emperor results;
Deity Production rises 3.12% at turn 75 but falls 21.46% at turn 100, and
eliminations rise from one to two. In seed `61006501`, early population gains are
followed by elimination by turn 100. These are policy outcomes, not proof of a
particular combat mechanism.

## Validation and disposition

Four focused tests pass, including a sixteen-turn comparison against actual
`Action::Produce` and `EndTurn`: population, empire Production and Granary
completion match on a fixture that isolates growth/construction. Other checks
prove actual governor dispatch, refusal of population with no Production return,
and queue, threat, expansion, income, timing and fielded-army guards. The fast
local run explicitly uses `CARGO_PROFILE_CI_OPT_LEVEL=0`; the ordinary optimized
CI source `50594d440` also passes cargo-test, rust-quality, collaboration-policy,
overwrite-guard, paired-cost, published-build and control-mod. The cost gate's
adaptive shape does not measure this explicit Domination forecast's overhead.

The runtime prototype is removed. Final runtime inputs match current trunk.
The final local optimized suite passes 4,548 tests with 54 ignored; changed-line
Rust quality passes. The prototype suite passed 4,552 tests with 54 ignored.
Exact tested sources and commands are recorded in the manifest. The prototype is preserved as a patch against the control
report merge and in PR #3900's checkpoint history. Neither the real Firaxis
controller, its pin, private profile, nor any running game was altered.

A correct fixed-board growth recurrence is insufficient evidence for a strategic
investment. This candidate failed the strength gate; the production growth goal
remains active. The previously retained productive-improvement research routing
remains deployed. This experiment adds no runtime improvement.

Recompute the summaries with:

```sh
python3 docs/eval/2026-10-04-production-growth-forecast-summarize.py pilot
python3 docs/eval/2026-10-04-production-growth-forecast-summarize.py confirmation
```
