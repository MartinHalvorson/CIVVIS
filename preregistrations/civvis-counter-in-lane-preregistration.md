# Pre-registration — race the leader instead of declaring on them

Written 2026-07-27 by the `loop-counter-leader` session, before the run.
PR #516. Second pre-registration of this loop; the first
(`civvis-counter-leader-preregistration.md`) predicted the ablation's null and
was correct at 24×16.

## What the evidence says so far

At the **deployment** profile (60×38 / 4p / 6 city-states, and 74×46 / 6p):

- An empire at war with **one** rival wins 4.4% of its seats (2 of 45); with
  **two**, 10.7% (3 of 28). Base rate 16.7%. War is measurably costly to the
  empire fighting it.
- Deleting the whole denial response is a dead heat on wins (51.2%,
  p=0.5488) but the blind arm is **ahead on terminal score, 65 map-directions
  to 44, sign p=0.0549** across 109 resolving maps, with more gold (734 vs
  623), cities, population, science and production. The shipped response costs
  development and buys no wins.
- Score is the instrument that predicts (62% at 200 turns out against a 16.7%
  base; settles 135 turns before the end), and at deployment scale 11 of 16
  games are decided on score.

Four of the seven races already answer themselves — culture with culture,
religion with religion, diplomacy with diplomacy. The two that answer with an
army are **Science** and **Expansion**, which is exactly the response the three
readings above argue against.

## The run

```
ai_eval advanced advanced_counter_in_lane \
  --players 4 --city-states 6 --width 60 --height 38 \
  --pairs 120 --turns 400 --seed 960000 --jobs 6
```

`advanced_counter_in_lane` is identical to `advanced` except that
`victory_denial` answers a Science threat with Science and an Expansion threat
with Expansion instead of Conquest. The alarm, its timing and its target are
unchanged — only what it asks for changes. Paired against `advanced` this
isolates the response's *shape*; `advanced_blind_to_leaders` already bounds its
*existence* from the other side.

Smoke-tested at 3 pairs: the arms diverge on faith (1375.8 vs 1547.6),
tourists (23.0 vs 32.1), military (510.4 vs 499.8) and religions founded
(2.00 vs 1.67). Not a silent no-op.

## Prediction

**Wins: null (48–52%, sign p > 0.05).** Almost everything in this repo
measures null on wins, wins rest on ~10% of maps, and I have no reason to
expect this to be the exception.

**Terminal score: `advanced_counter_in_lane` ahead**, by roughly the margin the
blind arm showed (~60/40 in map-directions, p < 0.15). That is the reading with
109 of 120 maps behind it, and it is the one the mechanism predicts: dropping
two wars should return the development the ablation showed the wars cost.

## What would refute it

Terminal score flat or against the treatment. That would mean the cost the
ablation measured comes from the *alarm* — the replanning, the target lock, the
lost tempo — rather than from the wars it starts, and the lane rewrite cannot
recover it.

## Not to be read as

A promotion. Terminal score is a diagnostic, not a gate input, and one seed at
one seat count is not a deployment claim.

---

# Confirmation run — registered 2026-07-27, after the first result, before the second

## What the first run said (seed 960000, 120 pairs, 60x38/4p/6 CS)

| reading | value |
|---|---|
| game-win share | `advanced` 111/240, **`advanced_counter_in_lane` 129/240 (53.8%)** |
| paired score for `advanced` | 46.2% (Wilson 37.6–55.1), Elo-equivalent **−26** |
| paired direction | 2 / 107 neutral / **11**, sign **p=0.0225 SIGNIFICANT** |
| terminal score | 23 / 58 / **39**, sign **p=0.0559** |
| gate | INCONCLUSIVE (Wilson has not cleared parity at 120 maps) |

Gold 720.9 vs 550.9 (+31%), faith 817 vs 714, science 116.5 vs 113.6. Victory
mix moves toward diplomacy (13 vs 5) and score (33 vs 27).

**My registered prediction was wrong.** I predicted wins null and terminal
score ahead; both favour the treatment and it is *wins* that reached
significance. Recorded as an error, not smoothed over.

## Why this is not yet a result

`46.2%` is the identical first reading PR #411 got, and that one "turned out to
be a selection artifact, not the depth" once repaired. Wins here rest on 13 of
120 maps that broke. The gate is INCONCLUSIVE by construction at this n: at
53.8% the Wilson bound needs roughly 450 maps, the same arithmetic the warm-
branch promotion had to satisfy.

## The run

```
ai_eval advanced advanced_counter_in_lane \
  --players 4 --city-states 6 --width 60 --height 38 \
  --pairs 360 --turns 400 --seed 970000 --jobs 6
```

Disjoint seed, 3x the maps, everything else identical.

## Prediction

**The treatment holds between 52% and 56%, sign p < 0.05 on its own, and the
pooled two-seed direction is unambiguous.** If the mechanism is real — dropping
two wars returns the development those wars cost — it should not be seed-
specific.

## What would refute it

Regression to 49–51%. That makes the first run a selection artifact of the kind
this repo has already recorded once, and the lane rewrite is a null like
everything else in this loop.
