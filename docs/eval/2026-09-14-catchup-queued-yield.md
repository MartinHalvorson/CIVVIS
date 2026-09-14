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

## Activation results

Both probes completed all six games and all 36 intended seats on clean
source `2dca52cd201f861b12a98c608f5d142aff0e5aef`, binary SHA-256
`4db89b85cd5a0f7d9f916827b84a53bdcdd827fcfa474a93138a4208177554f5`.
Their gene fingerprint is
`1a768338d2fc47b1cab53d81ba3cdcb8cab8fcf8a72202d5beb5cc6ddc31eaff`.

```sh
cargo build --profile ci --locked --features developer-tools --bin gene_screen
target/ci/gene_screen --genes culture-building-catchup-3 --games 6 \
  --start-seed 914357000 --jobs 4 --difficulty emperor --out culture.jsonl
target/ci/gene_screen --genes research-building-catchup,research-building-catchup-2,research-building-catchup-3 \
  --games 6 --start-seed 914357100 --jobs 4 --difficulty emperor --out research.jsonl
target/ci/gene_screen --analyze culture.jsonl \
  --json docs/gene_screens/fires/2026-09-14-catchup-queued-yield-culture.json
target/ci/gene_screen --analyze research.jsonl \
  --json docs/gene_screens/fires/2026-09-14-catchup-queued-yield-research.json
```

Culture v3 appeared on 10 seats and won twice; the 26 other seats won four
times. Its win difference is +4.62 pp (SE 7.79), and score-share difference is
−0.51 pp (SE 1.00). Three games ended in science, two in culture and one by
score. Both measurements are unresolved.

Research v3 appeared on seven seats and won once; the 29 other seats won five
times. Its win difference is −2.96 pp (SE 12.21), and score-share difference
is −1.79 pp (SE 0.84). That adverse share signal deserves follow-up; six games
with only seven v3 seats do not establish a reliable win-rate change. All six
games ended in science. Those other seats include older family versions, so
the marginal row is not a comparison against the family-off level alone.

Both artifacts pass the repository's gene-firing evidence gate. Seven focused
regressions separately cover the queue decisions, protections and live
identity: forcing a successor reports only the actually active family member.
The identity correction came after the frozen probe and does not change its
investment behavior.

The two predeclared 192-game family comparisons will use the merged source,
which includes upstream's correction for a missing city-owner lookup. The
old probe build remains identified above; it is not reused for new long runs.
The new versions remain default-off while those comparisons are evaluated.
