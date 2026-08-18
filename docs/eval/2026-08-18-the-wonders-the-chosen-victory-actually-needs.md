# The wonders the chosen victory actually needs

_2026-08-18 · `agent/mbp-m5-pro-64/claude-opus5-20260818`_

## What was asked

Work backwards from the win condition instead of forwards from the build menu:
**which wonders does each victory actually need, and does the controller build
them?**

The `Item::Wonder` arm of `production_value` prices a wonder's yields, housing,
Amenities, great-work slots and great-person points and **nothing in
`spec.effects`** — where every wonder that decides a victory keeps its payload.
The `Item::Building` arm beside it has priced building effects since it was
written. And `lane_opens` is true only for a Culture plan, a Score target or an
untargeted Egypt/China, so every other agent takes the `-10_000` refusal on
every wonder in the game.

Read against the data that looks damning:

* `DIPLOMATIC_VICTORY_POINTS` is 20 and three wonders grant seven of them
  outright — Statue of Liberty 4, Mahabodhi Temple 2, Potala Palace 1. **Thirty-
  five percent of a diplomatic victory sits in the build menu**, while
  `docs/CIV6_LADDER.md`'s census of 199 terminal host events has a rival taking
  that lane 41 times.
* Science is the lane `victory_eval` completes least often, and Oxford
  University, Amundsen-Scott and Ruhr Valley are refused by the same line.

## How it was measured

**Lane completion** — `victory_eval` at the ladder's profile (`--players 6
--turns 250 --speed online`), the arm against `--without strategic-wonders` on
identical seeds. `--without` is new in this change; the tool could previously
say how often a lane lands and never what any behaviour contributed to it.

⚠ It is a mirror and a mirror is biased against this treatment: all six seats
get it, a wonder is exclusive, and production put into one another seat finishes
first is lost. Read it as a pacing floor.

**Strength** — `ai_eval advanced advanced_without_strategic_wonders
--deployment-comparison --pairs 200 --players 6 --width 74 --height 46
--city-states 9 --turns 250 --speed online --seed 28000000`.

**Artifact** — `victory_eval` now prints the wonders each game finished, owner
tagged. A valuation is a claim about intent; a wonder on the map is the fact.

## What it measured

### 1. The deployment screen is a non-measurement, and that is the useful half

> ⚠ nothing differed: all 200 maps were neutral on wins AND on terminal score,
> so `advanced` and `advanced_without_strategic_wonders` played the same games.

200 of 200 maps byte-identical. The arm keys off the lane the agent is playing
for, and the deployed native agent is **untargeted** — `victory_target: None` —
so it has no chosen victory to work backwards from and falls through to
`plan.strategy`, which on this profile never reaches a qualifying wonder. The
shipped controller is provably unchanged by this change. Per
`docs/EVAL_INTEGRITY.md` that verdict is not evidence about the treatment; it is
evidence about where the treatment can reach.

### 2. ★★★★★ THE DIPLOMATIC LANE FINISHES ZERO WONDERS, AND PRICING CANNOT FIX IT

The census is unambiguous. Six 250-turn six-player diplomatic games, seeds
24000000–05, with the arm and without it:

```
WONDERS seed=24000000 target=diplomatic ->
WONDERS seed=24000001 target=diplomatic ->
WONDERS seed=24000002 target=diplomatic ->
```

Empty, both arms, every game. The same tool on the same seeds:

```
target=culture -> 1:cristo_redentor 3:machu_picchu 4:forbidden_city
target=score   -> 1:great_library 4:hanging_gardens 1:panama_canal 1:pyramids …
```

So the seven points are not being declined on price. **They are unreachable.**
The Mahabodhi Temple needs a founded religion, a Holy Site and a Temple, on a
forest tile beside the district; the Statue of Liberty needs a Harbor, a coastal
water tile and Civil Engineering. A diplomatic empire builds none of those
prerequisites, so `can_produce` never offers the wonder and no valuation is ever
consulted. The one DVP wonder that *is* reachable — the Potala Palace, which
needs only Astronomy and a hill beside a mountain — is 1 060 production for
**one** of twenty points, and pricing it in cost the lane games (below).

This inverts the premise the change started from. The binding constraint on
"build the wonders the victory needs" is the **prerequisite chain**, not the
valuation, and it is a different piece of work.

### 3. Two arms cut on evidence before the screen ran

*Conquest, removed.* Pricing the Statue of Zeus, Terracotta Army and Venetian
Arsenal for a Domination lane cost it two games in eight (`--start-seed
21000000`): **1/8 with the arm, 3/8 without**, both losses ending on the
turn-250 tally rather than on a capital. A domination victory is not bought in
the build menu; the production a wonder takes is the production that was going
to take a capital — the trade the 2026-08-14 war-half removal paid +38 Elo to
make in the other direction. With the arm gone the lane reads 2/24 both ways.
`the_conquest_lane_buys_no_wonders_and_that_is_deliberate` pins the emptiness.

*A flat value floor, replaced by a cost-proportional one.* A flat floor buys a
victory point at any price. Diplomacy — the lane the change existed for —
completed **16/24 with the arm against 19/24 without** (`--start-seed
24000000`). `STRATEGIC_WONDER_VALUE_PER_COST` asks a wonder for 2.0 of lane
value per point of production, a fifth of a Library's raw density in pure
victory currency; it refuses the Potala Palace (1.2) and keeps the Mahabodhi
Temple (3.5) and the Statue of Liberty (2.3). The lane then reads 24/32 both
ways on a disjoint stream.

*And the bars had to gate the value, not only the lane.* The first draft applied
them to lane-opening alone and still added the raw figure to the score, so a
wonder the bars had just refused was boosted wherever some other gate had opened
a lane — the live seat's race, or a Culture plan. One qualification now serves
both.

### 4. Lane completion, paired on identical seeds

| lane | 24000000 (24 a lane) | 27000000 (32 a lane) |
|---|---|---|
| science | 0/24 vs 0/24 | — |
| culture | **15/24** vs 12/24 | 17/32 vs **19/32** |
| religious | 11/24 vs 11/24 | 13/28 vs 14/29 |
| diplomatic | 24/32 vs 24/32 (post-bar) | 24/32 vs 24/32 |
| domination | 2/24 vs 2/24 | — |
| score | 24/24 vs 24/24 | — |

**No lane regresses and no movement replicates across streams.** A ±3-in-24
swing that changes sign is a null.

## What was decided

**Shipped, and stated for what it is.** The wonder arm now prices
`spec.effects`, which was a real hole — the building arm beside it has done so
since it was written. It changes the deployed native agent on **zero of 200**
deployment maps, regresses no lane over ~480 paired games, and is registered as
`advanced_without_strategic_wonders` so it stays priceable.

It is **not** shipped as a strength win, because nothing here measured one.

**The finding worth more than the change** is §2: the diplomatic lane finishes
no wonders at all, and the reason is prerequisites rather than price. That is
the next piece of work — a Diplomacy plan that builds a Harbor, or a Holy Site
and a Temple, unlocks six of the seven points this change can only price. Until
then the honest answer to "which strategic wonders help us reach a diplomatic
victory" is **none that this agent can build**.

Two instruments came out of this and outlast it: `victory_eval --without`, so a
lane table can attribute rather than only describe; and the finished-wonder
census, which is what turned a pricing argument into a reachability fact in one
run.
