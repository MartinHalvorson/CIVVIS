# Barbarian-bargain randomized pilot

_2026-08-21 · native board · completed subset of a pre-registered randomized screen_

## Decision

`barbarian-bargain` remains **off and unresolved**.  The completed pilot leans
against the current global eight-point attack discount, but it is far too small
to justify removing, retuning, or promoting any gene.  In particular, a
post-hoc change to the discount would turn this exploratory result into the
selection criterion for its own confirmation.

The production implementation and its independent on/off gate therefore stay
unchanged.  The existing focused unit test continues to pin the intended
behavior: a soldier prices a barbarian target eight points lower, while a
rival's unit and a Scout do not receive that treatment.

## Completed evidence

The screen randomly varied the nine native genes that had no prior native
on/off result.  It used a foldover design: the second arm replayed the same
map and seat with the exact complementary nine-gene genome.  Every one of the
six major seats was treated and civilizations were shuffled, so ten completed
map pairs yield 60 clustered paired comparisons.

```bash
target/ci/gene_screen \
  --pairs 24 --players 6 --width 60 --height 38 --city-states 6 \
  --speed online --map pangaea --turns 250 \
  --genes barbarian-hunt,barbarian-bargain,barbarian-ranged-answer,builder-worked-tile-priority,settler-threat-detour,district-lookahead-settle,priced-tile-purchase,governor-victory-lanes,governor-expansion-lane \
  --baseline best --field advanced --design foldover \
  --all-seats --randomize-civs --start-seed 80000000
```

The run was intentionally stopped after the ten fully flushed map pairs
(seeds 80,000,000 through 80,000,009; 20 games).  No partial game entered the
analysis.  The resulting profile was six-player 60×38 Pangaea, six city
states, Online speed, all victory types, and a 250-turn limit.  Religious wins
ended 60% of the completed games (median turn 171), score 25%, and culture
15%.  The machine-readable, re-runnable analysis is
[`2026-08-21-barbarian-bargain-pilot-9g-6p-allseats-60-pairs.json`](../gene_screens/2026-08-21-barbarian-bargain-pilot-9g-6p-allseats-60-pairs.json).

| Gene | Win Δ (on − off) | Win z | Score-share Δ | Share z | Pilot read |
|---|---:|---:|---:|---:|---|
| `barbarian-hunt` | +3.3 pp | +0.43 | +1.06 pp | +1.09 | unresolved |
| `barbarian-bargain` | **−13.3 pp** | **−1.81** | −1.08 pp | −1.01 | unresolved |
| `barbarian-ranged-answer` | +0.0 pp | +0.00 | +0.58 pp | +0.47 | unresolved |
| `builder-worked-tile-priority` | +13.3 pp | +1.81 | +0.51 pp | +0.57 | unresolved |
| `settler-threat-detour` | −10.0 pp | −1.96 | +0.10 pp | +0.14 | unresolved |
| `district-lookahead-settle` | −13.3 pp | −1.81 | −0.09 pp | −0.10 | unresolved |
| `priced-tile-purchase` | +0.0 pp | +0.00 | −0.64 pp | −0.94 | unresolved |
| `governor-victory-lanes` | −10.0 pp | −1.96 | **−2.35 pp** | **−4.54** | share screen flag only |
| `governor-expansion-lane` | +6.7 pp | +0.80 | −0.02 pp | −0.02 | unresolved |

For `barbarian-bargain`, the clustered 95% win interval is **−27.8 to +1.1
points** and the whole-sign-matrix adjusted estimate is **−13.3 ± 7.3
points**.  That is a useful negative-direction hypothesis, not an elimination
result.  The run's 80%-power resolution was ±20.6 win points (±2.56
score-share points), and its nine-gene family-wise threshold was |z| ≥ 2.77.
It has one chronological replication cell containing all 60 pairs; it has not
earned the three independent 10k windows required for a reproducibility claim.

The `governor-victory-lanes` share flag is likewise not a deployment decision:
the pilot had one chance-sized window and the field varied eight other genes.
It is recorded here so later disjoint direct runs can test the direction rather
than rediscover it.

## What was and was not changed

No default or ledger verdict changed.  The ledger is intentionally reserved
for dedicated, adequately resolved sources; importing this 60-pair pilot would
make a transient, multi-gene screen override the existing deployment evidence.
The ranking's "unmeasured" entries consequently remain the honest deployment
state rather than being relabeled from a single incomplete pilot.

The next valid experiment, if this gene is revisited, is a pre-registered
single-gene foldover against `--baseline best` on disjoint seeds, followed by
the normal matrix/live-withhold gate only if it resolves.  It must compare a
specified replacement rule with the present gate; it must not tune the
eight-point discount against these same rows.
