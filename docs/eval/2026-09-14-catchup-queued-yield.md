# Catch-up investments count the yield already on its way

The earlier culture and research catch-up genes stop reserving buildings as
soon as any matching building appears anywhere in the empire's queues. A
Library adding two science can therefore close a much larger shortfall, even
if it will take several dozen turns or sits behind unrelated work.

`culture-building-catchup-3` and `research-building-catchup-3` price the
remaining deficit. They count matching buildings at the front of active
queues, using item-specific remaining production and the existing build-time
estimator. Only yield arriving within the proposed new building's completion
time plus the existing twenty-standard-turn investment window earns credit.
A new reservation receives at most the remaining yield deficit, then competes
by turns per credited yield. Sufficient timely work leaves other queues free;
a small or slow answer allows another city to contribute. Each invocation
still places at most one order and never replaces work already in progress.

Culture retains v2's median-rival target. Research retains v2's exclusion of
Spaceport cities. Both retain recovery, military alarm, threatened-city,
first-trader, and end-of-game limits. Other investment families keep their
original queue behavior. Both new versions are exclusive and default off.

Yield credit uses the same base building yields as the earlier scorer. It
does not forecast future amenity changes, buildings' powered halves, civic
multipliers, or subsequent production growth. The twenty-turn window is a
heuristic inherited from admission, not a measured optimal delay.

## Predeclared experiments

Before running any games:

- Culture activation: six games, seeds 914357000–914357005, only
  `culture-building-catchup-3` varied.
- Research activation: six games, seeds 914357100–914357105, all three research
  catch-up versions varied because the older version defaults on.
- Culture family comparison: 192 games, seeds 914377000–914377191.
- Research family comparison: 192 games, seeds 914387000–914387191.

All runs use Emperor, six players, 74×46 Continents, nine city-states, Online
speed to its own 250-turn clock, all victory conditions, the best-genome
baseline, and standard genome probabilities. The two families are evaluated
separately. These samples expose activation and broad regressions, and do not
justify changing a deployment default or declaring a win-rate improvement
from a positive point estimate alone.

Exact commands and results will be added after building the clean source
checkpoint and completing the experiments.
