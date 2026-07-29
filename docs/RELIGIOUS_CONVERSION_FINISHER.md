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

### Development screen

Across 30 fresh mirrored maps (60 games), the candidate split wins 30-30 and
map directions 1-1. The win gate was therefore neutral: paired score 50.0%,
95% Wilson interval 33.2%-66.8%, and exact sign-test `p = 1.0`.

The diagnostic did move favorably enough to spend the pre-registered gate:
terminal score share was 50.8%, with 21 favorable map directions, one neutral,
and eight adverse (`p = 0.0241`). The candidate also averaged 156.0 score to
146.6, 2.54 cities to 2.37, and 16.3 population to 15.5. These are development
signals only; none can promote the treatment.

Religious outcomes warned against assuming that exact local conversion gains
compose into stronger play. Candidate seats won 15 religious and 15 score
victories, while control seats won 20 religious and 10 score victories.
Religion-dominant plans fell from 6/27 to 2/23 seats, and stored faith fell
from 320.9 to 299.6. The disjoint gate decides whether the empire-development
gain converts to wins or is merely a behavioral trade.

### Disjoint promotion gate

The exact pre-registered gate command was:

```text
cargo run --profile ci --bin ai_eval -- \
  strategic_deep_conversion strategic_deep \
  --pairs 120 --players 4 --width 24 --height 16 \
  --turns 200 --seed 107000 --jobs 12
```

Across 120 fresh mirrored maps (240 games), the candidate lost 114-126:

- paired score 47.5%, 95% Wilson interval 38.8%-56.4%, Elo-equivalent
  -17 (interval -79 to +45);
- five favorable map directions, 104 neutral, and 11 adverse; exact sign-test
  `p = 0.2101`;
- no anytime-valid crossing in either direction; promotion gate
  **INCONCLUSIVE**;
- terminal score share 50.2%, with 59 favorable directions, 11 neutral, and
  50 adverse (`p = 0.4437`).

The larger sample resolved the screen's tension in the direction wins require.
The candidate still developed slightly larger empires (147.2 score to 141.8,
2.55 cities to 2.43, 15.9 population to 15.1), but converted that advantage
into fewer religious victories: 65 to the control's 81. Plan exposure barely
moved, including religious commitment at 30.5% versus 31.1%, which is
consistent with a turn-level conversion effect rather than macro-route
selection.

Exact local improvement is therefore not a safe proxy for game conversion.
The treatment stays available as an evaluator-only negative control, off by
default. `strategic_deep` is unchanged.
