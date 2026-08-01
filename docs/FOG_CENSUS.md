# Fog information-set census

`fog_census` is a fast, headless differential test for the question that a
normal replay cannot answer: *did a controller use a fact that its seated
player could not observe?*

It is a diagnostic, not a fog-honest controller. Its first purpose is to make
the production-policy integrity gap measurable before treating a replacement
controller as a fix.

## Run it

```sh
cargo run --release --locked --bin fog_census -- \
  --maps 64 --probes 12 --players 4 --width 44 --height 28 \
  --turns 200 --seed 862000 --jobs 8
```

`--probes` is a per-map cap. The executable spaces probes through the match
instead of spending the whole cap in the opening, where neither diplomacy nor
military pressure has had time to become meaningful. It runs majors only and
prefers at-war targets. A failed integrity check exits with status 2.

## What one probe proves

At a live `AdvancedAi` decision point, the census:

1. selects one enemy fact that is currently hidden from the acting seat;
2. rebuilds a no-op branch and a modified branch through the public save
   format, preserving `Game`'s private occupancy indexes;
3. checks that the source, no-op, and modified worlds have byte-identical
   `obs_tensor` values for that seat; and
4. replays cloned controller state through the complete current turn on all
   three worlds.

The no-op branch must reproduce both the full action trace and the observer
`PlanReport`. That makes serialization, fresh derived caches, and controller
cloning part of the control rather than unexamined assumptions. If it does not,
the whole run is invalid and no treatment rate is reported.

The modified branch cycles through three treatments:

- changing a hidden military unit's HP;
- changing a hidden city's HP; and
- moving a hidden military unit to a legal, still-fogged neighbouring tile.

Position treatments rank an at-war unit crossing into an owned city's
six-hex pressure radius first. HP treatments likewise rank hidden hostile
units already in that radius. Those are not arbitrary map edits: they target
the exact full-world `AdvancedAi::city_pressure` input that can redirect a
plan toward recovery.

A treatment is a **controlled witness** when either the action trace or the
`PlanReport` differs. The latter matters: a controller can expose hidden-state
dependence in its current policy update even when that update has not yet
changed an order during this turn. An action difference is reported separately,
including its first differing action index.

## Fixed-seed release census

The fixed-seed release census (`4p`, `44x28`, 200-turn cap, 12 spaced
probes/map, 64 maps, eight workers) completed in 49.6 wall-clock seconds after
the one-time optimized build and yielded 714 valid probes:

| check | result |
|---|---:|
| modified tensor matches | 714 / 714 |
| save/load tensor matches | 714 / 714 |
| no-op controller decisions match | 714 / 714 |
| decision divergences | 42 / 714 (5.9%) |
| plan-report divergences | 31 / 714 |
| action-trace divergences | 18 / 714 |

The sample contained 366 unit-position, 181 unit-HP, and 167 city-HP
treatments; 545 targeted an active war. A separate 32-map run at seed 860000
found 24 decision divergences in 348 valid probes (6.9%), including five action
traces. A four-map seed-863000 spot check produced the exact same counts and
witnesses with one worker and four workers. On a larger deployment-style
six-player 74x46 profile (12 maps, 250-turn cap, seed 864000), all 142 controls
also matched and 16 decisions diverged (11.3%; eight plan reports and nine
action traces). Together, those divergences are evidence that `AdvancedAi`
reads full `Game` state that the tensor does not reveal. They do **not** measure
every possible leak, and a zero result on any finite treatment set would not
establish fog honesty.

## Scope and limitations

The census uses `obs_tensor` as the implementation's fog-honest input
contract. It disables presentation-only fog memory to match ordinary headless
simulation; explored ground and live visibility remain in the world. A future
tensor plane that reveals a selected fact automatically invalidates that probe
rather than silently weakening the test.

The test is deliberately black-box. It does not assert which internal function
read the fact, nor does it prove that every branch with an unchanged report is
fog safe. It establishes a reproducible lower bound on observable
information-set violations and supplies compact seed/turn/seat witnesses for a
subsequent fog-controller refactor.

## Battlefront-observation evaluator result

`--fog-honest-pressure --battlefront-observation` enables the retained
`advanced_fog_battlefront` evaluator arm. Its fixed 64-map release run at seed
862000 completed 722 valid probes with 722/722 treatment-tensor, save/load,
and null-control matches. It found nine action-trace divergences (six hidden
unit-position, three hidden unit-HP) and zero hidden-city-HP or plan-report
divergences. This is an integrity improvement over the archived 28/717
baseline, but not a claim of whole-controller fog honesty: the arm did not
pass its independent deployment-strength gate and is not the production AI.

The census also prints divergence treatment-family totals and first differing
source/treated actions. When a plan report differs, it prints both reports;
that made an earlier same-turn vision-frame leak reproducible and repairable
without weakening the tensor or null controls.
