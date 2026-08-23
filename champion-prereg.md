# Pre-registration: the bred champion, decided on WINS

Written before the run.

## What is being tested
A 40-gene champion from `genome_breed`, bred on victory-lane progress against
the shipped genome. Holdout on 48 disjoint maps: **0.5886 ± 0.0280, edge
+0.0886 (3.2 SE)**, fitted gap only +0.0180.

Largest live-gene moves: `mil_per_city` 1.00→3.98, `settler_min_pop` 2.00→4.52,
`builder_per_city` 0.50→0.23, `focus_fire` 2.50→4.50, `attack_floor` 0→10.5.
Roughly: much more army per city, slower and taller expansion, fewer builders.

## The run
`genome_breed --wins <genes> --maps 100 --players 4 --turns 500 --seed 3300000`
Paired, seat-mirrored, fresh seed the search never saw.

## Decision rule, fixed now
- **PASS** requires map directions FOR > AGAINST with sign p < 0.05.
- A capped game with no victor is neutral, not a defeat.
- Anything else is a NULL and the shipped genome stands.

## What each outcome means
- **PASS**: the first genuine strength improvement of this loop, and lane
  progress is validated on a third case. Then confirm at the `strategic_deep`
  budget before proposing promotion.
- **NULL**: lane progress joins score share as a statistic that nominates
  changes which do not convert. That would be a serious result against my own
  fitness finding, and `docs/GENOME.md` would need it stated plainly.

## Prior
I do not have a confident prediction. Every previous nomination in this loop
died here, which argues null. But this is the first one selected on a
statistic validated against wins, and +0.0886 is three times the lane-progress
edge of the one change known to win.
