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

## Visible-only pressure baseline

After the visible-only city-pressure repair, and before the battlefront
observation repair below, a current-main spot check at
seed `872000` (`16` maps, the same four-player 44×28/200-turn shape, 12
probes/map, six workers) produced 177 valid controls. All modified tensors,
save/load tensors, and no-op decisions matched; 6/177 decisions diverged
(3.4%): four plan reports and two action traces. The sample had 90 hidden unit
positions, 45 unit HP values, 42 city HP values, and 135 active-war targets.
This smaller run is not a replacement for the 64-map release census, but it
confirms that filtering one pressure path reduced neither the broader
information-set problem nor the need for a fog-honest controller.

`advanced_belief_pressure` is a default-off evaluator arm that now consumes
last-seen, player-visible military observations only for that repaired pressure
path. Its memory term is intentionally too narrow to affect the census's other
full-state reads, so it must not be cited as a fog-honesty repair.

## Battlefront observation repair

`AdvancedAi` now refreshes a controller-local `BeliefState` before every
major turn. A foreign City Center sighting retains only the combat-relevant
state that was visible then: owner, HP, wall HP, and the displayed city
strength. Campaign scoring uses that report rather than a hidden city's live
heal, breach, or garrison-derived strength; a never-seen city gets a
conservative generic defense estimate instead of its private live state.

The battlefront layer now also filters current hostile units and City Centers
through the acting seat's live vision when it chooses a domain objective,
focus target, local force ratio, and force posture. Last-seen military
contacts remain available only through the existing bounded belief memory.
That is an information-set repair for these decision boundaries, not a claim
that the full controller is fog honest.

The same `872000` spot-check shape now has 178 valid controls and 3/178
divergences (1.7%): no plan-report divergence remains, and all three remaining
witnesses are unit-position action differences. On the larger fixed
`862000..862063` release shape, the pre-repair controller yielded 28/717
divergences (3.9%; 11 plan reports); the repaired controller yielded 18/725
(2.5%; zero plan reports). The probe counts differ because the altered
controller reaches a slightly different sequence of live decision points, so
this is a reproducible before/after regression measure, **not** a paired
effect-size estimate.

The repaired run's treatment breakdown is 14 hidden-unit-position witnesses,
4 hidden-unit-HP witnesses, and **0 hidden-city-HP witnesses**. `fog_census`
now prints that breakdown and both actions at the first action difference,
which makes the remaining lower-level tactical leaks actionable without
mistaking a reduced rate for a full proof of fog honesty.

As a gameplay safety screen, the repaired controller and its immediately
preceding build were both run against `basic` on identical seed prefixes. The
compact 60-map/120-game screen was exactly equal at 113 `advanced` wins. On a
24-map/48-game six-player 74×46 Online/planet deployment shape, the preceding
build had 16 `advanced` wins and the repaired build had 14; terminal-score
share was effectively unchanged (57.2% and 57.3%). This is not a
promotion-strength comparison, and the small deployment difference is recorded
as a regression screen rather than interpreted as an effect.

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
