# Openings: what each civilization actually plays

The scripted opening has been bounded twice and the ordering layers around it
once. `docs/GENOME.md` records all three:

| layer | bound | source |
|---|---|---|
| opening book, swept | holdout `-0.0019 ± 0.0148` | `opening_sweep` |
| opening book, deleted entirely | `-0.003` | ablation |
| technology order, randomised | below the 0.09 resolution | `order_ablate` |
| civic order, randomised | below the 0.09 resolution | `order_ablate` |

Every one of those pooled all civilizations together and asked whether a
**global** opening could be improved. None of them asked the prior question:
**does the civilization change the opening at all, and if so, is the answer any
good?** `src/bin/opening_census.rs` is the instrument for that.

```text
opening_census --swap --maps 8 --players 6 --window 40    # does the civ matter?
opening_census --maps 60 --players 6 --turns 500          # what does it play?
```

## 1. The civilization does reach the opening — 8.88 of 21 distinct

`--swap` holds the map, the seed, the rivals **and the start tile** fixed,
overwrites one seat's civilization name, and replays the opening window. The
overwrite happens after construction, so Civilization VI's start bias is
deliberately given up: anything that diverges is the decision layer reading who
the seat is, and nothing else.

8 maps, seed 300000, 6 players, 24×16, window 40, depth 6, 21 civilizations:

| map | distinct openings |
|---|---|
| 300000 | 10 |
| 300001 | 4 |
| 300002 | 2 |
| 300003 | 14 |
| 300004 | 7 |
| 300005 | 6 |
| 300006 | 15 |
| 300007 | 13 |

**0 of 8 maps had every civilization open identically. Mean 8.88 distinct
openings out of 21.** The separations are visible in the sequences: Aztec picks
up `eagle_warrior`, Russia queues `[lavra]`, and a religious branch runs
`[holy_site] > shrine > project:holy_site_prayers` where the generic line runs
`monument > settler`.

### ⚠ A hypothesis this retracts

I predicted the opposite, from a static read: `Game::leader_trait`
(`src/game.rs:15439`) exposes the shipped preference-trait vector — over the 21
seated majors, `expansionist` ×6, `low_religious_preference` ×6,
`cultural_major_civ` ×5, `science_major_civ` ×5, `aggressive_military` ×4 and
six rarer ones — and it has **zero callers**. The planner's only by-name
civilization code is three string literals (`"Greece"` twice in
`victory_focus`, `"China"` once) plus a `+55` unique-unit bonus in tech
valuation. From that I expected a civilization-blind opening.

That was wrong, and it is the `gene_probe` lesson from `docs/GENOME.md` running
backwards: **a channel being unread does not mean the signal is absent**, it
means the signal arrives some other way. Here it arrives through *mechanics* —
which unit and district the civilization can build at all — rather than through
preference. `leader_trait` is still unread, and that remains a real unexploited
axis, but it is not the reason openings differ.

## 2. Among survivors the opening is nearly a constant

60 maps × 6 players, 500 turns, `randomize_civs`, window 50, depth 6.

Seating had to be fixed before this mode meant anything: `seat_civs` hands the
stock roster out in seat order and `randomize_civs` defaults off, so every map
seated Rome at seat 0 and a per-civilization table would really have been a
per-seat table.

- 3 of 360 seats never held a capital inside the window.
- **116 distinct build sequences over 357 seats.**
- Second city: mean turn **20–34** by civilization. Cities at turn 50: **≈1.4**.

Every opening that reached six seats shares one stem:

| seats | wins | score share | opening |
|---|---|---|---|
| 8 | 1 | 0.3481 ± 0.0508 | warrior > settler > builder > monument > **[holy_site] > shrine** |
| 7 | 0 | 0.2436 ± 0.0490 | warrior > settler > builder > monument > **slinger > trader** |
| 7 | 1 | 0.2226 ± 0.0424 | warrior > settler > builder > monument > **trader > slinger** |
| 13 | 0 | 0.2006 ± 0.0370 | warrior > **builder > monument > settler** > trader > slinger |
| 7 | 3 | 0.1790 ± 0.0468 | warrior > settler > builder > monument > **galley > settler** |
| 9 | 0 | 0.1589 ± 0.0250 | warrior > settler > builder > monument > **galley > trader** |

Five of the six are `warrior > settler > builder > monument` and differ only in
slots five and six. **The 8.88-way divergence the swap probe measures is not
what the surviving population plays.** It is mostly the difference between the
generic line and branches taken by seats that go on to do badly or die.

⚠ **This table is correlational and the start is inside both columns.** It
describes the population; it does not say an opening wins. The causal
instrument is a paired `ai_eval` against a seat forced onto the candidate
opening, the way `opening_sweep`'s holdout is.

## 3. The confound that had to be removed, and the finding inside it

The first run of this table ranked `warrior` (a one-item sequence) and
`warrior > builder` at the bottom with score shares of 0.0005 and 0.0024 over
90 and 43 seats. Those are not openings. **Sequence length is a proxy for early
survival**: a seat whose capital falls on turn 12 records two builds and then
scores nothing, so pooling short sequences in ranks openings by how long their
owner lived.

Measured directly:

> seats recording all 6 builds score **0.3007 ± 0.0121** (187 seats); seats
> recording fewer score **0.0222 ± 0.0062** (170 seats).

A 13× gap, far larger than any difference *between* openings (0.16–0.35). The
outcome table is now restricted to full-depth sequences, and §4 checks the
proxy against measured survival rather than leaving it to stand.

## 4. ⚠ The elimination rate is my map, not the agent — retracted

The first version of this document led with "47.6% of major seats are out
before six capital builds", measured at 6 players on 24×16. Survival is now
tracked directly rather than inferred, and the death rate turns out to be a
function of how crowded I made the map:

| players × map | land tiles per seat | lost every city | mean death turn | cities @50 | peak cities |
|---|---|---|---|---|---|
| 6 × 24×16 | 64 | **50.1%** | 29 | 1.38 | 2.43 |
| 4 × 24×16 (the `ai_eval` convention) | 96 | 30.7% | 72 | 1.78 | 3.17 |
| 4 × 32×22 | 176 | 12.9% | 205 | 1.95 | 4.42 |

**Barbarians are not the cause**: with `--barbarians 0` at 6 players the rate is
51.8% at the same mean death turn 29. It is rivals, and it is density. Every
seat founds its capital — `0 never founded at all` in all six runs — so nothing
is losing its opening settler either.

**Read the 6-player row as a property of the configuration.** The repository's
own evaluation geometry is 4 players on 24×16, and any claim about elimination
should be made there or roomier.

## 5. What survives every configuration: the opening does not expand

`cities at turn 50` is **1.38 / 1.78 / 1.95** across the three rows above. Even
with 176 land tiles per seat and 87% of seats alive at the end, the agent holds
**fewer than two cities at turn 50** and peaks at **4.42 ever**.

That is not an accident of tuning, it is written down. `AdvancedAi::plan`
(`src/ai/advanced.rs:1431`):

```rust
let map_capacity = (2 + land / 55).clamp(3, 9);
let city_cadence = g.standard_duration(90).max(1) as usize;
let desired_cities = (3 + g.turn as usize / city_cadence)
    .min(map_capacity)
    .min(6);
```

So the target is **3 cities for the first 90 turns** and **6 for the rest of
the game, forever, on any map**. On 32×22 `map_capacity` is not even binding —
the `.min(6)` and the 90-turn cadence are. Two things follow, and they are
different problems:

1. **The target is low.** Competent Civilization VI play on a standard map is
   4–6 cities by turn 50 and well past 6 by the midgame. The `.min(6)` is a
   hard ceiling nothing can lift.
2. **The agent does not even reach its own target.** 1.95 cities at turn 50
   against a `desired_cities` of 3. That is a failure to execute the opening,
   not a policy choice, and it is the more interesting of the two.

### The mechanism for (2): one settler

Three cities takes two settlers out of the capital. Over 112 full-depth seats
at depth 8 on 32×22, the capital queues **1.72 ± 0.05** settlers in its first
eight builds, and the split matters:

| settlers in first 8 builds | seats | score share | wins | peak cities |
|---|---|---|---|---|
| 1 | 36 | 0.2320 ± 0.0223 | 5 (13.9%) | 4.31 |
| 2 | 71 | 0.2661 ± 0.0162 | **22 (31.0%)** | 4.70 |

The same signal appears in the six-slot table in §2: of the six openings that
reached six seats, exactly one contains a second settler
(`… > galley > settler`), and it holds 3 wins in 7 seats where the other five
hold 0–1 in 7–13.

⚠ **Correlational, and the obvious confound is the strong one.** A seat that
builds two settlers is a seat with room and safety, so the start is inside both
columns. This is a hypothesis with a mechanism attached, not a result.

## 6. Why the second settler is missing: expansion is serialized empire-wide

The production gate for a settler (`src/ai/advanced.rs:6699`):

```rust
if city_count + counts.settlers < plan.desired_cities
    && counts.settlers == 0
    && city.pop >= 2
    && expansion_open
    && site.is_some()
```

`counts` is an `EmpireCounts`, so `counts.settlers == 0` means **at most one
settler may exist in the whole empire at a time**. The first clause already
caps cities-plus-settlers at the target, so this one adds no cap — it is purely
a serialization constraint. A four-city empire expands no faster than a
one-city empire: build a settler, walk it, found, and only then may the next
one start.

**Fires-check — the founding cadence, 60 maps, 4 players, 32×22:**

| N cities | seats | mean turn first held | gap |
|---|---|---|---|
| 2 | 232 | 37.0 ± 1.3 | |
| 3 | 195 | 71.0 ± 2.2 | +34.0 |
| 4 | 157 | 89.5 ± 2.5 | +18.5 |
| 5 | 87 | 118.7 ± 4.5 | +29.2 |
| 6 | 50 | 150.2 ± 7.3 | +31.5 |

**The gaps do not shrink.** A five-city empire adds its sixth as slowly as a
two-city empire added its third, which is what serialization looks like and is
not what a compounding expansion looks like. Expansion is supposed to be the
one thing in the game that accelerates.

And the blocked time is measured directly: a seat spends **60.8 ± 3.8 turns**
short of its city target *with a settler already walking* — time in which
`counts.settlers == 0` forbids starting another — against **68.5 ± 4.2 turns**
short with no settler anywhere, which that clause does not explain (site
availability, `pop >= 2`, the expansion window, or simply losing the production
argmax to something else).

So roughly **half** of all time-below-target is attributable to this one
clause, and the other half needs its own diagnosis.

## 7. ⚠ The serialization story is wrong — the fires-check killed it

§6's attribution was tested and does not survive. `advanced_parallel_settlers`
lifts `counts.settlers == 0` to the shortfall against the target. Over the same
60 maps at 4 players on 32×22:

| metric | control | parallel settlers |
|---|---|---|
| cities at turn 50 | 1.95 ± 0.05 | 1.95 ± 0.05 |
| peak cities ever | 4.42 ± 0.15 | 4.53 ± 0.16 |
| first held 2 cities | 37.0 | 37.6 |
| 3 | 71.0 (+34.0) | 71.0 (+33.4) |
| 4 | 89.5 (+18.5) | 89.1 (+18.1) |
| 5 | 118.7 (+29.2) | 117.6 (+28.5) |
| 6 | 150.2 (+31.5) | 148.7 (+31.1) |
| turns short with a settler walking | 60.8 | 58.6 |

**Near-inert.** It was not taken to an eval; an inert treatment cannot change a
decision, and this repository's convention is to screen before spending forty
minutes of `ai_eval`.

**Why it is inert.** `counts.settlers == 0` is redundant on top of engine rules
that bind harder. A settler requires `pop >= 2` and **consumes a population**
when it completes, and successive settlers cost 80, then 110, then 140
production. A one- or two-city empire cannot *afford* a second settler whether
or not the AI permits one — the capital has to regrow the population it spent
before it may spend another.

So the 60.8 ± 3.8 turns a seat spends short of target with a settler already
walking are **not** turns the clause forbids a second settler. They are turns
the empire could not pay for one. The measurement was right; the attribution
was mine and it was wrong.

**What this reframes.** The slow expansion in §5 is not an AI serialization
bug. It is the settler economy — which is faithful to Civilization VI, where
expansion is likewise paid for in population and escalating cost. The open
question is no longer "why won't the AI queue a second settler" but **"why is
the capital not growing fast enough to afford one"**, which is a food, tile-
improvement and district-timing question, not a build-order one.

The entrant is kept with its null recorded, on the `advanced_lane_reachable`
precedent, so the axis can be re-measured if the settler economy ever changes.

### ⚠ The obvious objection, and why it does not land

`docs/GENOME.md` records that **`city_target` saturates above six** — swept a
point at a time, 7 through 12 buy nothing. It is fair to ask whether that
already settles expansion.

It does not, because it is a different lever. `city_target` sets *how many*
cities the empire wants; `counts.settlers == 0` sets *how fast* it may acquire
them. An empire that reaches six cities on turn 150 and one that reaches six on
turn 90 have the same target and very different games — the second compounds
those yields for sixty more turns. Nothing in the `city_target` sweep varies
rate, and nothing in it could have.

Two further reasons the sweep does not transfer: it ran at **20 mirrored maps a
point**, and this repository's own rule is that twenty-map runs on this
evaluator are anti-evidence (two published conclusions inverted at 120 maps).
And `city_target` is only consulted by `AdvancedAi` as a *fallback* when no
plan exists (`src/ai/advanced.rs:2602`); with a plan in force the hard-coded
`desired_cities` is what binds.

## 8. What the capital can afford, which closes §7's question

60 maps, 4 players, 32×22. Capital population, housing and food at fixed
checkpoints:

| turn | population | housing | food yield | at housing cap |
|---|---|---|---|---|
| 25 | 2.36 ± 0.04 | 5.29 ± 0.08 | 7.15 ± 0.13 | 14% |
| 50 | 3.46 ± 0.05 | 6.58 ± 0.11 | 9.75 ± 0.17 | 8% |
| 75 | 4.68 ± 0.06 | 8.43 ± 0.13 | 12.55 ± 0.21 | 7% |
| 100 | 5.96 ± 0.09 | 9.95 ± 0.15 | 16.08 ± 0.34 | 5% |

**Housing is not the constraint.** Only 5–14% of capitals sit within one
population of their housing cap, and the headroom *widens* over time — 9.95
housing against 5.96 population by turn 100. Whatever is slowing growth, it is
not the Civilization VI housing ceiling.

**The numbers close the loop on §7.** The capital gains roughly **one
population per 23 turns** (2.36 → 3.46 over turns 25–50). A settler requires
`pop >= 2` and consumes one on completion. So a capital sitting at 2.36
population on turn 25 can afford exactly one settler, drops to ~1.4, and cannot
afford another until it has regrown — about 23 turns, plus the build.

Compare that against the founding cadence in §6: **+34.0 / +18.5 / +29.2 /
+31.5 turns.** The population regrowth interval and the city founding interval
are the same number. The cadence was never an AI scheduling decision; it is the
capital's growth rate, and the `counts.settlers == 0` clause was sitting on top
of a constraint that already bound — which is precisely why removing it
measured inert.

**The lever, if there is one, is food** — not housing, not build order, and not
the settler permission.

## 9. Which civilization does best in the end — mostly noise

300 maps, 4 players, 32×22, 500 turns, `randomize_civs`: **1191 major seats
over 50 civilizations**, roughly 24 seats each. Ranked by terminal score share
the table runs from Maya at 0.3460 ± 0.0328 (50.0% wins) down to Persia at
0.1873 ± 0.0223 (11.5% wins) — a range of 0.159 that looks decisive and mostly
is not.

| test | result |
|---|---|
| score share, one-way across 50 civilizations | F(49, 1141) = **1.585**, **p = 0.0069**, **η² = 0.064** |
| observed range of civilization means | 0.1587 |
| range expected from noise alone (50 groups, mean SE 0.0280) | ~0.126 |
| win rate, pooled 25.2% | χ²(49) = **52.9** against an expected 49 ± 9.9 |

**So: the civilization has a small but real effect on terminal score — 6.4% of
the variance — and no effect on wins that this population can detect.** The
eye-catching top-to-bottom spread is only slightly wider than 50 groups of 24
produce by chance, and the win-rate spread is exactly chance.

⚠ **One caveat that cuts against the F-test.** Score share is compositional —
the four majors on a map share out ~1.0 between them — so seats within a map
are negatively correlated and are not the independent samples the test assumes.
Read p = 0.0069 as optimistic. The win-rate χ², which does not have this
problem, finds nothing.

⚠ **And this bounds the wrong quantity for the operator's question.** It
measures what *being* a civilization is worth under near-civilization-blind
play — mostly the mechanical value of its unique unit, district and ability. It
does **not** bound what better per-civilization *decisions* could be worth,
which is untested. The two are different, and only the first is measured here.

One incidental finding worth recording: at depth 8 the modal opening takes only
**5–22%** of a civilization's seats, and most civilizations have nearly as many
distinct openings as seats. Openings barely repeat once you look eight builds
deep — §2's "one stem" result is a property of the first four builds, not of
the opening as a whole.

## What to measure next, in order

1. ~~Relax `counts.settlers == 0`.~~ **Done, and null — see §7.** Kept as an
   entrant with the null recorded.
2. ~~Ask why the capital cannot afford a settler.~~ **Done — see §8.** It is
   growth, not housing: the capital gains one population per ~23 turns and that
   interval *is* the founding cadence. The remaining lever is **food** — which
   tiles the capital works, when the first builder's charges land, and whether
   a granary arrives before or after the second settler. That is the next
   thing to instrument, and it is an economy question, not a build-order one.
3. **Lift `.min(6)`, separately.** Independent of the above, and
   `docs/GENOME.md`'s rule about candidate-set changes moving effective
   thresholds applies to expansion targets too.
4. **Bound the civilization-aware channels by ablation, on wins.** Strip the
   unique-unit tech bonus and the unique-district preference and play it paired
   against stock. The 8.88-way divergence in §1 says the channel *fires*;
   reachability is not leverage.
5. **Only then, per-civilization openings.** If (4) returns null the way the
   opening book did, a `leader_trait`-aware opening is very likely null too.

Note the ordering has changed since the first draft: (1) and (2) are about
tempo and outranked the per-civilization work once the expansion numbers came
in. Openings do set the tempo here — just not by *which* six things get built.
