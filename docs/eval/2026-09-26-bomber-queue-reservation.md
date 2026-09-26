# A launch Bomber keeps an idle queue during deterrence

Native run `civvis-20260926T194755Z` completed Advanced Flight on turn 201.
Cumaná already had an active Aerodrome. Aluminum income became 2 on turn
208, and the host offered a Bomber from turn 209. Nevertheless, the city
started a Ranger on turn 212 and remained occupied with it through turn 227.

An immutable prefix ending at the first turn-212 board records an empty
Cumaná queue, Aluminum stock 8/income 2, 639 Gold, and a 21-turn Bomber.
The mirrored game independently offers and accepts that Bomber. A fresh
late-start controller instead queues a Cuirassier. Withholding only
`peacetime-deterrence` changes that choice to a Bomber. The native controller
had persistent memories; the replay's different land-unit choice is not
claimed to reproduce that history.

The cause is a queue ordering conflict. The peacetime branch of
`redirect_repeatable_projects_for_force_gap` claims all idle cities for land
units before `air_surge_production` runs. Its candidate list excludes aircraft.
The normal air production pass cannot use a queue already occupied by that
earlier pass.

The fix is restricted to an idle city that can legally start a launch Bomber
under the existing Domination air-readiness policy. Its technology, wing
count, home safety, fuel, treasury and completion-time gates remain in force.
Deterrence continues through other cities. Active queues, project redirection,
wartime force gaps and emergency defense retain their existing behavior.
No host bridge, engine rule or deployment defaults change.

## Validation protocol

The regression runs deterrence before air production, matching the live
policy order. It failed before the fix because deterrence consumed the
airfield's queue. Controls exercise the cases where a launch Bomber cannot
be reserved, plus the existing readiness safety tests.

Before implementation, freeze a baseline CLI and evaluator from production
source `1155d4ee6` (test-only checkpoint records the reproducer). Compare the
candidate against that baseline on the immutable turn-212 prefix using
`--serve --fresh-board`, one stdin request and immediate EOF. This offline
process never opens a native orders database or controls the running game.

Run two complete simulator pairs per source with the existing
`--domination-pair air-surge-2` profile, seeds 37820000 and 37820001. The fixed
profile is King, Gran Colombia, four majors, six city-states, 60×38 Tiny
Pangaea, Online, all victories, barbarians and the natural 250-turn clock.
Rivals are CIVVIS controllers, not Firaxis AI. Keep every outcome, including
the off arm, and do not substitute seeds or stop for results. This small
diagnostic does not establish a win-rate improvement or justify promotion.

Artifacts are retained under
`~/civvis-tactics-results/2026-09-26/bomber-queue-reservation/`, with native
prefixes and exploratory evidence in the neighboring `air-readiness/` directory.
Native execution and stronger Domination outcomes remain to be measured at
the verification agent's normal completed-game boundaries.

## Frozen replay result

Both CLIs use the same prefix, SHA-256
`34ee7aab2c4740fc3af98cd4ce31334736b8e617ce6526b33907375368a087d9`,
the recorded forced-policy list, and one `212` stdin request. Baseline
`ffd4803b5` starts a Cuirassier in Cumaná and a Ranger in Santa Marta.
Candidate `e01c848b8` starts a Bomber in Cumaná and a Cuirassier in Santa
Marta, retaining deterrence in the other idle city. Withholding deterrence
entirely in the baseline instead makes Santa Marta build a University.
These are fresh-agent proposals; no native acceptance is implied.

| Frozen binary | SHA-256 |
|---|---|
| Baseline CLI | `6c3e2c598710f81b6e43387e26f19c8c4ae57c45142620271d08d7927ec6cc9d` |
| Candidate CLI | `91ecafe0fb0c1ed4a3a97a381aafd475de2cb3a0b8de0b47c84e67749bebd82c` |
| Baseline evaluator | `e4a1b83782b5ab6f1a331ef05ccdd04ee6f0e32e0149c527cc9170401ca80562` |
| Candidate evaluator | `f05e20fb9f551599661673aba9081ba6d046609981e9de4abc85ad3ed81c41e2` |

All nine readiness tests pass, including the reproduced queue conflict and
eight refusal cases compared with the existing deterrence policy. The initial
late-game control incorrectly demanded a land unit with only one turn left;
the corrected control checks unchanged policy behavior instead.
Main was integrated afterward through `be1492357`, including the independent
Aluminum Builder and typed native war-permission changes. The frozen binaries
exclude that integration so the small before/after comparison remains isolated.

The integrated `cargo test --profile ci --locked` suite passed 4,326 tests
with 53 existing ignores. `QUALITY_BASE=origin/main python3 tools/rust_quality.py`
passed; formatting and whitespace checks are clean. No Lua source changed in
this policy fix.

## Completed simulator comparison

Both sources completed both pairs without a crash or an early stop. All four
focal result rows are identical between sources, including their action count
and conquest telemetry. Every focal game lost to Science, with zero foreign
major cities ever observed held and zero foreign original capitals held at
the end. The off arm is unchanged as expected. These seeds show no outcome
benefit from the fix.

| Seed | Air-surge-2 arm | Turn, both sources | Score, both sources |
|---|---|---|---|
| 37820000 | off | 201 | 659 |
| 37820000 | on | 209 | 675 |
| 37820001 | off | 206 | 717 |
| 37820001 | on | 210 | 742 |

The native-board replay and regression establish the narrower correction:
deterrence can no longer consume that ready launch-Bomber queue. They do not
establish a completed native launch, a capture, or a stronger win rate.
