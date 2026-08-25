# The deployment shape keeps one gene, downgrades one, and kills two

_2026-08-19 · `agent/martbot-mbp-m5-max-128/claude-opus5-a7710669`_

## What was asked

The first `gene_screen` round
(`docs/eval/2026-08-19-gene-screen-random-genome-factorial-screen.md`) flagged
five genes at 4 majors on 60×38 — three past the family-wise bar, two at
|z| ≈ 2.4 where 57 genes throw ~2.6 flags by chance — and closed with **no
controller change on this evidence** and a named next step: re-run exactly those
five at the deployment seat count and map, on disjoint seeds, with only those
five screened so the residual background variance is a fraction of the 57-gene
run's.

**Does anything survive?**

## How it was measured

`gene_screen`, foldover-paired as before: arm 2 replays the same map seed and
the same seat with the complement genome, so each gene is on in exactly one arm
of every pair and the map cancels out of every difference.

- **80 foldover pairs (160 games)**, seeds 27000000–27000079 — disjoint from
  round one's 26081900–26082219.
- **6 majors, 74×46 Pangaea, 9 city-states, Online speed to its own 250-turn
  clock** — the deployment shape, against a field of production `advanced`.
- `--genes governor-every-lane,war-economy,score-horizon,district-coverage,wide-map-capacity`;
  every other gene held at the native repair bundle.
- Resolution at this size: win Δ **±12.2 pp**, score-share Δ **±1.90 pp** at 80%
  power. Five genes, so the family-wise 5% bar is |z| ≥ 2.58.

```sh
gene_screen --pairs 80 --players 6 --width 74 --height 46 --city-states 9 \
  --genes governor-every-lane,war-economy,score-horizon,district-coverage,wide-map-capacity \
  --start-seed 27000000 --jobs 8 --out screen-6p-deployment.jsonl
```

Cost: about 2 hours at `--jobs 8`. A 6p/74×46 game is roughly five times a
4p/60×38 one, which is the reason this run is 80 pairs and not 300.

## What it measured

| gene | 4p, 300 pairs | 6p deployment, 80 pairs | verdict |
|---|---|---|---|
| `war-economy` | −6.0 pp win (z −2.42), −2.57 pp share (z −5.00) | **−12.5 pp win (z −3.03, CI −20.6..−4.4), −2.62 pp share (z −4.15)**, adjusted −12.6 ± 4.3 | **CONFIRMED, both axes, both shapes** |
| `wide-map-capacity` | +2.89 pp share (z +5.69), win −0.7 pp | **+2.23 pp share (z +3.43), win Δ exactly 0.0** | **CONFIRMED, and still buys no wins** |
| `governor-every-lane` | −4.02 pp share (z −8.34) | −1.50 pp share (z −2.22) | direction holds, **below the bar here** |
| `score-horizon` | +6.0 pp win (z +2.42) | +2.5 pp win (z +0.57) | **did not replicate** |
| `district-coverage` | +6.0 pp win (z +2.42) | +0.0 pp win (z −0.04) | **did not replicate** |

**`war-economy` is the result.** It is the only gene that hurts on both axes at
both shapes, on disjoint seeds, and the deployment shape makes it worse rather
than milder: a treated seat carrying it won **2 of 80** paired arms against
**12 of 80** without it. Its OLS coefficient over the whole sign matrix
(−12.6 ± 4.3) matches its marginal Δ, so it is not another gene's imbalance
wearing its name.

**`wide-map-capacity` reproduces its strange shape exactly.** More score share
at z +3.4, and a win Δ of *precisely zero*. Round one's anchors said the same
thing about the bundle as a whole (+3.45 cities, z +6.98, and no wins); this is
the single gene that carries it. A bigger empire that does not convert.

**`score-horizon` and `district-coverage` were the two flags round one
explicitly refused to call findings** — |z| ≈ 2.4 against ~2.6 flags expected by
chance from 57 genes. Neither survives. That is the multiplicity warning doing
its job, and it is worth recording that the warning was written *before* the
replication, not after it.

**`governor-every-lane` is the awkward one.** Same sign, a third of the
magnitude, and it does not clear the family-wise bar at 80 pairs. It remains the
gene with the strongest independent corroboration — `advanced_every_lane` at
−62 Elo compact / −95 deployment over 400 pairs per gate
(`docs/eval/2026-08-18-pricing-the-governor-s-routing-and-the-settling-asymmetry.md`,
PR #1955) — so the honest reading is *consistent, not re-established here*.

### Interactions: nothing, again

Ten gene pairs tested from the pair sums: **0 at |z| ≥ 2 against 0 expected** on
the win axis, 1 against 0 on score share, and **0 past the family-wise bar**
(|z| ≥ 2.81) on either. Round one said the same about 1,596 pairs. Two runs, two
shapes, no visible pairwise coupling among these repairs.

### The religion census, which did not change with the shape

At 6 majors the treated seat founded a religion in **41%** of games, launched an
Inquisition in **17%**, ended with **4.1 of 7.3 cities under a foreign faith**,
and left **2,014 faith in the bank**. Over the **56 games it lost to a rival's
religion**: 29% had founded a religion of their own, **5.2 of 6.5 cities had
flipped**, and **752 faith was still unspent**.

Reading `AdvancedAi::religious_defense` explains the bank. A non-founder buys a
defensive Missionary only from a city whose majority faith is **not** the threat
— a flipped city is skipped, correctly, because the engine assigns the purchased
unit the purchase city's majority religion and buying there would fund the
rival. A city with no majority is skipped too. So the defence has somewhere to
buy from *until the conversion starts winning*, and nothing to spend on
afterwards.

⚠ This quantifies a known problem rather than discovering one:
`AdvancedAi::advanced_great_people` already carries a comment about a live run
ending with 1,999 Faith banked. What is new is the rate — it is the median
experience of a losing seat at the deployment shape, not an anecdote — and the
mechanism above.

## What was decided

**Nothing shipped in a controller, and `war-economy` goes to the gate.** A
screen licenses a confirmation arm, never a ship decision, and the arm already
exists:

```sh
ai_eval live live_without_war_economy --matrix --pairs 120 --jobs 12 --seed 31000000
```

`war-economy` is a live-bridge treatment and a member of
`ENGINE_REPAIR_WAR_TREATMENTS`, so it reaches `advanced_synergy`,
`advanced_synergy_war` and the deployed bridge, and **not** production
`advanced`. Two profiles agreeing on both axes is the strongest signal this
instrument has produced; it is still one instrument, and the matrix gate is what
decides.

⚠ **Do not change what `advanced_synergy` means in order to act on this.** The
arm names are the identity recorded results are filed under; removing a repair
from the bundle would silently re-date every number already published against
it. If the gate confirms, the change is a *new* arm beside the old one.

**`score-horizon` and `district-coverage` are closed as noise** unless something
else raises them. Recording a non-replication is the point of having run the
replication.

**The religion lane is where the next repair belongs, and the first one landed
during this round.** PR #2114 fixed the promotion chooser — both choosers rank
Apostle promotions by raw magnitude across incommensurable units, and
`AdvancedAi::promotion_value` has a *calibrated* weight table for military
effects that religion was never added to, so every religious effect takes the
`_ => 2.0` fallthrough. Its reachability census is also the cautionary tale of
this round: it caught that treatment wired to a function the deployed agent
never calls, twice, byte-identically, before a single game was spent on a win
rate.

**Still open, and named so it is not lost:** a non-founder that cannot buy a
defensive Missionary has no other sink for its Faith, and ends 250 turns with
750–2,000 of it. That is the largest measured hole on the board.
