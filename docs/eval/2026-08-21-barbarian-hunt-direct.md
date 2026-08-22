# Barbarian-hunt direct screen

_2026-08-21 · native board · pre-registered 300-map-pair foldover_

## Decision

`barbarian-hunt` remains **off and unresolved**.  The direct screen measured a
small positive win-rate contrast, **+0.56 percentage points** (paired
z = **+0.51**), while score share leaned negative by **−0.245 points**
(paired z = **−1.86**).  Neither reading clears the single-gene family-wise
bar of |z| ≥ 1.96, and the directions do not reinforce one another.

No code, threshold, ledger verdict, or deployment default changes from this
result.  The existing gate remains available for future disjoint replication.
It admits a narrow Advanced-controller field-civilian threat response; it is
not a global order to hunt barbarians.

## Completed evidence

The pilot that routed this gene to direct confirmation measured +3.3 win
points (z = +0.43) and +1.06 score-share points (z = +1.09) in 60 paired-seat
comparisons.  This confirmation held every other gene at the current best
baseline and varied only `barbarian-hunt`:

```bash
target/ci/gene_screen \
  --pairs 300 --players 6 --width 60 --height 38 --city-states 6 \
  --speed online --map pangaea --turns 250 \
  --genes barbarian-hunt \
  --baseline best --field advanced --design foldover \
  --all-seats --randomize-civs --start-seed 81000000 \
  --out target/gene_screens/2026-08-21-barbarian-hunt-direct-6p-300-pairs.jsonl
```

All **300 map pairs / 600 games / 1,800 clustered seat-pairs** completed on
the pre-registered seed window 81,000,000 through 81,000,299.  Both arms and
all six seats are present for every seed; no partial pair entered the
analysis.  The machine-readable result is
[`2026-08-21-barbarian-hunt-direct-6p-allseats-300-pairs.json`](../gene_screens/2026-08-21-barbarian-hunt-direct-6p-allseats-300-pairs.json).

| Reading | On | Off | Delta | Paired z | Read |
|---|---:|---:|---:|---:|---|
| Win rate | 16.94% | 16.39% | +0.56 pp | +0.51 | unresolved |
| Score share | — | — | −0.245 pp | −1.86 | unresolved |

The clustered 95% win interval is approximately **−1.6 to +2.7 points** and
the whole-sign-matrix adjusted estimate is **+0.56 ± 1.09 points**.  At this
size the run's 80%-power resolution is ±3.1 win points and ±0.37 score-share
points.  It contains one chronological replication window, so it cannot make
a reproducibility claim even if a chance-sized screen flag had appeared.

Religious victories accounted for 55% of seat outcomes (median turn 157),
score for 32% (turn 250), culture for 9% (median turn 223), science for 4%
(median turn 244), and diplomacy for less than 1% (median turn 245).

## What it means

The direct result does not reproduce the pilot's positive score-share
direction and is too imprecise to distinguish a modest win effect from zero.
Promoting from the pilot, removing from the negative share lean, or tuning the
admission rule against these rows would all be post-hoc selection.  The honest
action is therefore to retain the independently gated implementation, leave
it disabled, and require a new disjoint confirmation before reconsidering it.
