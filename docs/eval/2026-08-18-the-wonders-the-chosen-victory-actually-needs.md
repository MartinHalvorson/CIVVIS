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

### 3. Where it does reach, it picks the right wonder every time

The Culture lane is where the arm can act — its wonders need no district the
lane does not already build — and the census shows it acting, cleanly. Over 32
250-turn six-player culture games a side (seeds 24000000–31),
**Cristo Redentor is finished in 29 of 32 with the arm and 0 of 32 without it**.
It is `seaside_resort_tourism_pct: 100` plus `religious_tourism_unreduced` — the
densest tourism in the table, and a culture victory is tourists.

The first six games, side by side:

| seed | with the arm | without it |
|---|---|---|
| 00 | machu_picchu · pyramids · **cristo_redentor** | machu_picchu · pyramids |
| 01 | **cristo_redentor** · forbidden_city · colosseum | forbidden_city · colosseum |
| 02 | **cristo_redentor** · forbidden_city · temple_artemis | forbidden_city · temple_artemis |
| 03 | forbidden_city · great_library · petra · **cristo_redentor** | forbidden_city · great_library · petra · temple_artemis |
| 04 | **cristo_redentor** · great_bath · apadana | apadana |
| 05 | **st_basils_cathedral** · **cristo_redentor** · … | hanging_gardens · … |

The control builds the Hanging Gardens and the Temple of Artemis instead, which
are food and Amenities. St Basil's Cathedral
(`city_religious_tourism_pct: 100`) arrives the same way.

And the Religion lane the same way, on 32 games a side (seeds 24000000–31):

| | with the arm | without it |
|---|---:|---:|
| games finishing a wonder | **19 of 32** | **0 of 32** |
| which wonder | kotoku_in ×19 | — |

Kotoku-in is `city_faith_pct: 20` and four free Warrior Monks — faith is the
fuel a religious victory burns, and the control finishes no wonder at all in any
of the 32. Note this is the arm's **rule 2 being city-specific**: Kotoku-in
clears the cost bar only where the city's faith is already large enough for
+20% of it to be worth 1 420, which is why a flat data-sheet valuation could
never have expressed it.

So the mechanism is not merely firing, it is firing in the intended direction
and choosing the intended wonder in two different lanes. What it does not do is
move the win rate (§5) — which is a statement about effect size, not about
correctness.

### 4. Two arms cut on evidence before the screen ran

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

### 5. Lane completion, paired on identical seeds

The shipped arm, 32 games a lane at seeds 24000000, against `--without
strategic-wonders` on the same seeds:

| lane | with | without |
|---|---:|---:|
| culture | **17/32** | 15/32 |
| religious | 16/32 | 16/32 |
| diplomatic | 27/32 | 27/32 |

25 of the 96 games differ; the other 71 are identical, which is what a lane with
no qualifying reachable wonder looks like. A disjoint stream (27000000, 32 a
lane) reads culture 17/32 against 19/32 and diplomatic 24/32 both ways, and the
earlier all-six sweep at 24000000 read science 0/24 both ways, domination 2/24
both ways and score 24/24 both ways.

**No lane regresses on any stream, and the one that moves changes sign between
them.** Cristo Redentor in 29 of 32 games and a win rate that does not move is
the expected shape for this lane at this clock: Cristo Redentor is an Atomic-era
wonder, so it lands around t180 of 250 and has forty turns to compound a
tourism multiplier that wants two hundred.

## What was decided

**Shipped, and stated for what it is.** The wonder arm now prices
`spec.effects`, which was a real hole — the building arm beside it has done so
since it was written. It changes the deployed native agent on **zero of 200**
deployment maps, regresses no lane over ~480 paired games, and is registered as
`advanced_without_strategic_wonders` so it stays priceable.

It is **not** shipped as a strength win, because nothing here measured one.
What is measured is that it does what it was written to do — Cristo Redentor in
29 of 32 culture games and Kotoku-in in 19 of 32 religious ones, against a
control that finishes neither in any — and that doing so costs nothing.

**★★★★ THE PREREQUISITE CENSUS, WHICH GENERALISES §2.** 31 of the 53 wonders
name an `adjacent_district`, and the districts they hang off are the ones a lane
may never build: harbor 6, campus 4, holy_site 4, encampment 3, city_center 3,
entertainment_complex 2, commercial_hub 2, industrial_zone 2, theater_square 2.
So "the agent does not build the wonders its victory needs" is a district
question for more than half the table, and only the remainder can ever be
answered by pricing the wonder itself.

⚠ **It does reach the live seat**, which the deployment screen does not cover.
`civvis_orders` builds `AdvancedAi::targeting(target)` and then
`enable_live_bridge()`, so the live agent has both the flag (from
`promoted_policy_envoy`) and a chosen victory. On the ladder's default lane —
`diplomatic` — §2 says no qualifying wonder is reachable, so no change is
expected there; on a Culture or Score seat the live race's wonder *choice* can
shift, and the value gate above is what keeps a wonder the bars refused from
being boosted by the race.

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
