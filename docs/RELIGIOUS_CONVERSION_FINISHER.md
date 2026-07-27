# Religious conversion finisher

## Hypothesis

The strongest search agent loses strength in victory conversion rather than
empire development, and religious victory decides most of its multiplayer
games. Its macro planner can commit to Religion, but the last action is still a
greedy scripted unit order.

There is a concrete targeting seam. `Action::Spread` names a unit but not a
city. When more than one city is in range, the engine spreads by deterministic
adjacency order, which need not be the city the missionary planner
routed toward.

`strategic_deep_conversion` searches every immediately legal religious action
and every legal religious move followed by one such action on cloned states.
It executes a sequence only when it strictly improves this lexicographic value:

1. an actual religious win;
2. rival civilizations already over their city-majority threshold;
3. progress of the least-converted rival;
4. total cities following the founder;
5. continuous founder-pressure share.

The ordering matches the victory condition: finishing the last holdout outranks
farming easy cities in an empire already converted. Continuous pressure is
last so the search can see a useful spread before the city majority flips.
Every action is applied through `Game::apply`; the ordinary controller then
takes the rest of the turn. The default flag is off.

## Pre-registered evaluation

The development screen compares the new evaluator-only agent directly with
`strategic_deep`. Both use the same genome, 20-turn review cadence, 80-round
horizon, and warm branch state:

```text
cargo run --profile ci --bin ai_eval -- \
  strategic_deep_conversion strategic_deep \
  --pairs 30 --players 4 --width 24 --height 16 \
  --turns 200 --seed 106000 --jobs 12
```

Only a favorable development direction earns a disjoint 120-map gate at seed
107000. Only wins can promote it; terminal score, faith, pressure, and plan
labels are diagnostics.

## Result

Pending.
