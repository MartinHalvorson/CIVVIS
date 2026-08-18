# The wonders the chosen victory actually needs

_2026-08-18 · `agent/mbp-m5-pro-64/claude-opus5-20260818`_

## What was asked

The `Item::Wonder` arm of `production_value` prices a wonder's yields, housing,
Amenities, great-work slots and great-person points. It prices **nothing in
`spec.effects`** — the `Item::Building` arm beside it has priced building
effects since it was written, and the wonder arm never did.

That is not only a mispricing. `lane_opens` is true for a Culture plan, a Score
target, or an untargeted Egypt or China, and every other agent takes the
`-10_000` refusal sentinel on every wonder in the game. Read against the data:

* `DIPLOMATIC_VICTORY_POINTS` is 20 and three wonders grant seven of them
  outright — Statue of Liberty 4, Mahabodhi Temple 2, Potala Palace 1.
  **Thirty-five percent of a diplomatic victory sits in the build menu and the
  Diplomacy-targeted agent may not touch it**, while `docs/CIV6_LADDER.md`'s
  census of 199 terminal host events has a rival taking that lane 41 times —
  the largest single way this project loses a live game.
* Science is the lane `victory_eval` completes least often (0/16 at the ladder's
  profile), and Oxford University, Amundsen-Scott and Ruhr Valley are refused by
  the same line.

So: **price a wonder's effects in the lane's own currency, and open a production
lane for the wonders that carry the win condition.** Does the lane land more
often, and is the agent stronger?

## How it was measured

Two instruments, because they answer different questions.

**Lane completion** — `victory_eval` at the ladder's profile (`--players 6
--turns 250 --speed online`), the arm against `--without strategic-wonders` on
the same seeds. This is the goal metric: it says whether the chosen victory is
reached inside the clock.

⚠ **It is a mirror, and a mirror is biased against this treatment.** All six
seats get it, a wonder is exclusive, and production put into one another seat
finishes first is lost. In deployment only our seat pays that. Read it as a
pacing floor, not as a strength estimate.

**Strength** — `ai_eval advanced advanced_without_strategic_wonders
--deployment-comparison --pairs 200 --players 6 --width 74 --height 46
--city-states 9 --turns 250 --speed online --seed 28000000`. 200 pairs because
`docs/EVAL.md`'s resolution table puts a 40-map screen's smallest promotable
edge at +97 Elo-equivalent and this repository's changes are +40 to +55.

## What it measured

**Two arms were cut on the evidence before the screen ran.**

*Conquest, removed.* Pricing the Statue of Zeus' seven free units, the Terracotta
Army and the Venetian Arsenal for a Domination lane looked obvious and cost the
lane two games in eight (`--start-seed 21000000`, 8 games a lane): domination
**1/8 with the arm against 3/8 without**, both losses ending on the turn-250
tally rather than on a capital. A domination victory is not bought in the build
menu, and the production a wonder takes is the production that was going to take
a capital — the same trade the 2026-08-14 war-half removal paid +38 Elo to make
in the other direction. With the arm gone the lane reads 2/24 both ways
(`--start-seed 24000000`).

*A flat value floor, replaced by a cost-proportional one.* A flat 900 buys a
victory point at any price: the Potala Palace is 1 060 production for **one** of
twenty. Diplomacy — the lane the whole change was motivated by — completed
**16/24 with the arm against 19/24 without** (`--start-seed 24000000`). Adding
`STRATEGIC_WONDER_VALUE_PER_COST` (a wonder must return 2.0 of lane value per
point of production, a fifth of a Library's raw density in pure victory
currency) refuses the Potala Palace and keeps the Mahabodhi Temple (3.5) and the
Statue of Liberty (2.3). On a disjoint stream the lane then reads **24/32 both
ways** (`--start-seed 27000000`).

Lane completion with the shipped arm, paired on identical seeds:

| lane | seeds 24000000 (24 a lane) | seeds 27000000 (32 a lane) |
|---|---|---|
| science | 0/24 vs 0/24 | — |
| culture | **15/24** vs 12/24 | 17/32 vs **19/32** |
| religious | 11/24 vs 11/24 | 13/28 vs 14/29 |
| diplomatic | (see above) | 24/32 vs 24/32 |
| domination | 2/24 vs 2/24 | — |
| score | 24/24 vs 24/24 | — |

**No lane regresses, and none of the movements replicates across streams.** On
the mirror instrument this is a null, which is the honest reading of a
±3-in-24 swing that changes sign.

## What was decided

<!-- filled in when the 200-pair screen lands -->
