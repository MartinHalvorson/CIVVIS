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

## 10. What the existing per-civilization code is worth — the ablation

§9 measured what *being* a civilization is worth. This measures what the
planner's civilization-**aware code** is worth, which is the ceiling any better
per-civilization play has to beat. Per `docs/GENOME.md`'s rule, bound the
subsystem by ablation before optimising inside it.

`advanced_civ_blind` ignores all six by-name civilization signals in the
decision layer:

| site | what it does today |
|---|---|
| `victory_focus` | Greece prefers the Culture lane |
| `victory_focus` | Greece gets a +45 culture progress floor |
| `victory_focus` | China gets a +45 science progress floor |
| `tech_value` | +55 for a node unlocking this civilization's unique unit |
| `production_value` ×2 | Egypt and China are exempt from the wonder refusal |

It deliberately does **not** touch which unique unit or district a civilization
may build. That is mechanics; ablating it would measure the uniques rather than
the decisions about them.

**Fires-check (the lesson from §7 applied).** On the `--swap` probe the mean
distinct openings per map falls **8.88 → 8.38 of 21**. The flag bites — unlike
`parallel_settlers`, which was inert and was never taken to an eval — but only
slightly inside a 40-turn window. That is itself informative: **most of the
per-civilization opening divergence in §1 is mechanics, not the by-name code.**
Three of the six sites (the lane floors, the wonder exemption) act over the
whole game, so the window understates them and a full eval is the right test.

**Pre-registered and run:**

```text
ai_eval advanced_civ_blind advanced --pairs 120 --players 4 --turns 500 \
        --width 24 --height 16 --seed 310000
```

### The result: the civilization-aware decision layer is worth nothing measurable

120 maps, 240 games, 4 players, average 152.8 turns.

| measure | value |
|---|---|
| paired-map score for `advanced_civ_blind` | **49.2%** (95% Wilson 40.4%–58.0%) |
| Elo-equivalent | **−6** (CI −68 … +56) |
| paired direction on wins | 10 for / 98 neutral / 12 against, sign **p = 0.8318** |
| paired terminal score | **49.8%**, 56 for / 61 against, sign **p = 0.7117** |
| promotion gate | **INCONCLUSIVE** |

**Deleting every by-name civilization signal from the decision layer costs 0.8%
of paired score, which this population cannot distinguish from zero** — on
wins or on terminal score.

**Read the resolution before the headline.** Wins rest on only 22 of 120 maps
that broke, which is thin. Terminal score rests on **117 of 120** and is the
higher-resolution column, and it says 49.8% at p = 0.7117. Two independent
columns, the better-resolved one included, both find nothing.

**What the ablation did change**, from the diagnostics — so this is a bound on
a layer that fires, not a report on an inert one:

| | `advanced_civ_blind` | `advanced` |
|---|---|---|
| religious victories | 110 | 101 |
| diplomatic victories | 3 | 12 |
| faith | 198.8 | 238.1 |
| science / culture | 17.6 / 22.9 | 18.1 / 23.7 |

The Greece and China lane floors really do route seats to different victory
types, exactly as the fires-check predicted. It simply does not convert. Note
also that every economic column is *slightly* worse for the ablated arm and the
sign is consistent across all of them — weak evidence of a small real cost,
each individually null. The honest statement is that the layer is worth **at
most a few Elo and cannot be distinguished from zero**.

### What this means for per-civilization play

This is the ceiling any better per-civilization decision-making has to beat, and
it is low. Set beside the rest of the ledger:

| layer | bound |
|---|---|
| opening book, deleted entirely | −0.003 |
| technology order, randomised | below 0.09 resolution |
| civic order, randomised | below 0.09 resolution |
| **civilization-aware decisions, deleted entirely** | **−0.8%, p = 0.83** |
| being a particular civilization (§9) | η² = 0.064 on score, nothing on wins |

⚠ **It bounds the existing code, not every possible per-civilization rule.** A
`leader_trait`-aware opening is still untested, and this measurement cannot
prove it would be null. But the existing code covers the cases with the most
obvious value — a lane preference for the culture civilization, tech priority
for the unique unit — and deleting all of it costs nothing detectable. The
prior that a seventh by-name rule would do better is weak.

**The conclusion this line of work reaches:** openings do set the tempo in this
engine, but the tempo is set by the capital's food and the settler economy
(§7–§8), not by which units get built or by who the civilization is. Every
build-order and per-civilization lever anyone has measured — six of them now —
has come back at or near zero.

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
4. ~~Bound the civilization-aware channels by ablation.~~ **Done — NULL, see
   §10.** −0.8% paired, p = 0.83 on wins and p = 0.71 on terminal score.
5. ~~Per-civilization openings.~~ **Do not build one on this evidence.** (4)
   returned the null it was pre-registered to look for, so a
   `leader_trait`-aware opening should not be attempted until something
   changes the ceiling. The honest conclusion is that this agent's strength is
   not in its openings.
6. **Food, if anything.** §8 leaves exactly one live lever: `citizen_strategy`
   weights production 1.55 against food 1.25, in a capital whose growth gates
   every settler. Measuring its ceiling needs `workable_tile_yields` (private,
   in `src/game.rs`) and therefore a wider ownership claim than this task
   took. That is the next thing worth a paired eval — and it is an economy
   change, not an opening one.

Note the ordering has changed since the first draft: (1) and (2) are about
tempo and outranked the per-civilization work once the expansion numbers came
in. Openings do set the tempo here — just not by *which* six things get built.

## 11. The food ceiling — the first lever in this document that is not zero

§8 left one live question: the capital's growth gates every settler, and
`Game::citizen_strategy` weights **production 1.55 against food 1.25**. How
much food is the capital giving up?

Measured without changing any behaviour. `city_yields_weighted` and
`city_citizen_plan_weighted` are additive: passing `None` is exactly what the
engine already does, and neither is on the cached path. They let the census ask
what a capital *would* work under different appetites while the engine keeps
running its own. The comparison arm is deliberately extreme — food 10.0 against
production 1.0 — because this is a **ceiling**, not a proposal.

60 maps, 4 players, 32×22, capital only:

| turn | population | food yield → greedy | food **surplus** → greedy | production → greedy |
|---|---|---|---|---|
| 25 | 2.36 | 7.15 → 8.23 | **2.43 → 3.51** (+44%) | 9.41 → 7.68 (−18%) |
| 50 | 3.46 | 9.75 → 11.80 | **2.83 → 4.88** (+72%) | 12.66 → 9.44 (−25%) |
| 75 | 4.68 | 12.55 → 15.31 | **3.19 → 5.95** (+87%) | 14.71 → 10.79 (−27%) |
| 100 | 5.96 | 16.08 → 19.20 | **4.16 → 7.28** (+75%) | 18.10 → 14.07 (−22%) |

**Read the surplus column, not the yield column.** Food consumption is two per
population, and growth runs on what is left over — so a 15–22% gain in gross
food is a **44–87% gain in the surplus that actually grows the city**. That is
the largest headroom anything in this document has found, by an order of
magnitude.

**And read the production column beside it.** The same reassignment costs
18–27% of capital production. That is not a footnote: a settler costs 80, then
110, then 140 production, so the food-greedy capital reaches the population
threshold sooner and then takes longer to build the thing it unlocked. Growth
gates the settler; production pays for it. Which wins is not derivable from
these numbers and is exactly what a paired eval is for.

**Why this is worth doing when six other levers were not.** Every earlier lever
here was bounded at or near zero *before* any eval — the opening book at
−0.003, tech and civic order below resolution, the civilization-aware layer at
p = 0.83. This one has a measured ceiling of +44–87% on the quantity that
§7–§8 showed to be binding. It is the first candidate in this line of work with
real headroom rather than a hypothesis.

⚠ **Three cautions before anyone builds it.**

1. The extreme weighting is the ceiling, not the proposal. The treatment worth
   evaluating is a modest shift — swapping the two constants, or a food bonus
   that decays once the empire is at its city target — not food 10.0.
2. `citizen_strategy` is **engine-side and applies to every player**, including
   a human seat's auto-managed cities. This is not an `AdvancedAi` flag, and
   the change is correspondingly heavier. It belongs behind a treatment arm for
   evaluation, and its promotion is a game-balance decision as well as a
   strength one.
3. The trade is against production in a repository where `docs/GENOME.md`'s
   `gene_leverage` found **economy** to be the one load-bearing gene block.
   Moving the food/production exchange rate is moving exactly that block, so
   the prior that it is already near a local optimum deserves weight — and the
   expansion sub-block was also the one where *scrambling helped*, which cuts
   the other way. The measurement will settle it; argument will not.

## 12. ⚠ Growing the capital does not speed expansion — §8's attribution retracted

§11 found real headroom, so §12 spent it. `advanced_food_first` gives an
empire's governors an extra food appetite while it is short of its city target,
and withdraws it once the target is met. The bias is a number, so the response
curve is measurable rather than a single guess.

60 maps, 4 players, 32×22:

| food bias | capital pop @50 | food surplus @50 | production @50 | city 2 | city 3 | city 4 |
|---|---|---|---|---|---|---|
| 0 (shipped) | 3.46 | 2.83 | 12.66 | **37.0** | **71.0** | **89.5** |
| 0.6 | 3.58 | 2.89 | 12.75 | 39.1 | 74.4 | 97.4 |
| 2 | 3.77 | 3.30 | 12.52 | 39.4 | 73.4 | 95.0 |
| 6 | 4.08 | 3.89 | 11.84 | 38.2 | 77.5 | 100.7 |

**The lever works exactly as designed and produces the opposite of what it was
built for.** Population rises monotonically (3.46 → 4.08), food surplus rises
monotonically (2.83 → 3.89, +37%), production falls (12.66 → 11.84) — and
**every city after the first arrives later, monotonically in the dose.**

**This retracts §8.** I wrote there that the capital's ~23-turn population
regrowth interval "*is*" the founding cadence, because the two numbers matched.
They do match, and the causal direction is wrong: making the capital grow
faster does not make cities arrive sooner. It makes them arrive later.

That is the same error as §6→§7, made twice in the same document — a real
correlation, an attribution to the mechanism that happened to sit next to it,
and a dose-response experiment that refuted it. The correlation between growth
rate and founding rate is a *shared consequence* of the capital's total yield,
not a causal chain from one to the other.

**What is left standing:** the settler's binding cost is **production**, not
population. `pop >= 2` is satisfied from turn ~25 onward at pop 2.36, so the
population gate is rarely what a seat is waiting on; the 80/110/140 production
is. Trading production for food therefore pays for a threshold that was not
binding, with the currency that was.

**What is not established:** the precise mechanism for the *size* of the delay.
One turn of production per settler does not account for cities arriving 8–11
turns later, so something else — the empire-wide reach of the bias, or its
interaction with `citizen_strategy`'s existing `expansion` focus, which already
adds +0.55 food and +1.15 production when a settler is queued — is carrying
most of it. That is a further measurement, not a conclusion.

**Not taken to an `ai_eval`.** The treatment is worse on the metric it was
designed to move, in a clean monotone dose-response over 240 seats a point.
Spending forty minutes of paired evaluation to discover its win rate is also
worse is not a good use of the gate. The entrant ships at bias 0.6 with this
result recorded, on the `advanced_lane_reachable` precedent, so the axis can be
re-measured rather than re-derived.

### Where that leaves the ledger

| lever | bound |
|---|---|
| opening book, swept | −0.0019 ± 0.0148 |
| opening book, deleted | −0.003 |
| technology order, randomised | below 0.09 resolution |
| civic order, randomised | below 0.09 resolution |
| civilization-aware decisions, deleted | −0.8%, p = 0.83 |
| being a particular civilization | η² = 0.064 score, nothing on wins |
| one settler at a time, lifted | inert |
| **capital food appetite, raised** | **wrong direction, monotone in the dose** |

Eight levers. The build-order and per-civilization layers are bounded near
zero; the two economy levers with real headroom both point the wrong way when
pushed. **The remaining candidate this work can name is production, not food:
what a capital could produce if its citizens were assigned for it, and whether
the settler's 80/110/140 is what actually gates the founding cadence.** That is
a different measurement and it inherits none of §8's assumptions.

## 13. What actually gates the founding cadence: settler transit

§12 left production as the last candidate. It is not that either.

Decomposing every turn a seat spends below its city target — 60 maps, 4
players, 32×22, with #492's livelock fix in the base:

| where the time goes | turns per seat | share |
|---|---|---|
| **paying** for a settler (it is at the head of a queue) | 26.0 ± 0.8 | 20% |
| **walking** one (it exists and has not founded) | **57.1 ± 3.1** | **44%** |
| neither | 45.3 ± 3.6 | 35% |

**A settler spends more than twice as long in transit as its city spends
building it.** Production is 20% of the deficit. The 80/110/140 is not what
the cadence is waiting on, and neither — per §12 — is population.

**It is travel, not dithering.** Splitting the settler-turns by whether the
unit actually changed tile:

> 53.7 ± 2.3 turns **moved** (81%), 12.9 ± 1.6 turns **stood still** (19%).

Four fifths of a settler's life is spent in motion. That rules out the
comfortable explanations — it is not waiting for an escort, and it is not
stuck against a blocked path.

**Measured before and after the livelock fix**, because #492 had just landed
and "walking in circles" is exactly the confound that would fake this result:
57.1 turns with the fix against 60.8 without. The finding is not livelock; it
survives the repair almost unchanged.

### What is *not* established

I guessed the site search had no distance penalty. It does —
`settle_sites` subtracts `wdist(from, pos) × 0.9` inside radius 11, and ×0.45
for the long-range naval case — so that guess is withdrawn before it reaches a
conclusion.

What the numbers do not yet say is **how far the chosen sites actually are**,
and whether a settler reaches one directly. 53.7 moving turns per seat over
roughly three or four settlers is ~15 moving turns each, which is a long time
to cross a radius-11 search at a settler's movement rate. Three candidates,
none tested:

1. the sites really are near the edge of the search radius, and 0.9/hex is
   simply too cheap against `settle_value`'s scale (the accept threshold is 12);
2. terrain makes the effective rate about one tile a turn;
3. the settler re-targets en route as the map reveals, and the path is not
   monotone toward any one site.

Those want a settler-level trace — origin, chosen site, `wdist` between them,
turns taken, tiles actually stepped — which the census does not currently
record. **That is the next measurement, and it is the first one in this
document pointing at movement rather than economy.**

## 14. The settler trace: fifteen turns to travel five hexes

§13's three candidates, settled. Every completed settler journey on 60 maps,
4 players, 32×22 — **963 of them** — traced from spawn to disappearance:

| | |
|---|---|
| turns alive | **15.0 ± 0.7** |
| tiles actually stepped | **12.1 ± 0.6** |
| straight-line hexes, spawn to end | **5.2 ± 0.2** |
| detour ratio (steps ÷ straight line) | **2.32** |
| pace (steps ÷ turns) | **0.81 tiles per turn** |

A settler's shipped `moves` is **2**. So a settler takes **fifteen turns to end
up five hexes from where it was built**, walking two and a third times further
than the straight line, at forty percent of its movement allowance.
Going straight at full pace it would arrive in three.

**Candidate 1 is refuted.** The sites are not far. 5.2 hexes is well inside the
radius-11 search and well inside what a 0.9-per-hex penalty is pricing. Nothing
here says the AI settles too far away.

**Candidates 2 and 3 both survive, and contribute about equally.** The 2.32
detour and the 2.47 pace shortfall multiply to ≈5.7, which is the whole
15-turns-for-a-3-turn-trip overhead. Neither alone accounts for it.

### What these two numbers do and do not prove

⚠ **The pace shortfall may be entirely legitimate.** Civilization VI charges 2
movement for hills and forest, so a settler crossing broken ground genuinely
makes one tile a turn. 0.81 is consistent with mostly-rough terrain and proves
no waste on its own.

⚠ **Part of the detour is legitimate too.** Straight-line hex distance ignores
mountains, water and rival borders; a path around them is longer by necessity,
not by error. The measurement is deliberately conservative in one respect — it
ignores world wrap, so a journey that would be shorter the other way round the
globe is scored as *longer*, which inflates the straight-line figure and
therefore **understates** the detour ratio.

**But 2.32 is large for detour-by-necessity**, and it is the number worth
pursuing: a settler that walks 12 tiles to sit 5 from home either re-targeted
partway or was routed badly. The census cannot yet distinguish those, because it
records where a settler *ended*, not the site it was *aimed at* when it spawned.

**The next measurement is the aim.** Record `best_settle_site`'s answer at spawn
and compare it to where the settler actually founded. If they differ often, the
settler is re-targeting and the fix is commitment, not pathing. If they agree,
the path is bad and the fix is pathing. That is one field and it decides between
two entirely different repairs — and, unlike everything in §§5–12, it is a
movement question, where this repository's ledger of what works is untested
rather than uniformly zero.

## 15. The settler does not keep a destination

§14 posed a binary: bad path, or changing destination? `AdvancedAi` keeps a
`settler_targets` map, so the question is answerable directly by sampling what
each settler was aimed at, every turn it lived.

17,701 settler-turns, 60 maps, 4 players, 32×22:

| | count | share of settler-turns |
|---|---|---|
| ended aimed **somewhere else** than the turn before | 625 | **3.5%** |
| ended holding **no destination**, having held one | 4,799 | **27.1%** |

**A settler that chose a site and walked to it would show zero of both.**

The mechanism is one line, and it is not a re-plan — it is a dropped
commitment. `AdvancedAi::settler_step`:

```rust
let moved = self.base.settler_step_toward(g, pid, uid, target);
if !moved {
    self.settler_targets.remove(&uid);   // any blocked turn forgets the site
}
```

The target is also filtered out whenever `g.route_step(uid, target, 0)` is
momentarily `None` — a friendly unit in the way, a zone of control, a tile not
yet revealed. Neither condition means the site got worse. Both throw the
decision away and re-run the search from scratch next turn.

That squares with §14's other column: 19% of settler-turns are spent standing
still, and standing still is exactly the condition that discards the target.

### ⚠ How much of the 2.32× detour this explains — not established

3.5% of 15 turns is about **half a re-target per journey**. A single mid-course
change does inflate the ratio badly, because §14's straight line is measured
spawn-to-*end* and therefore misses the first leg entirely — so half a change
per journey is compatible with a 2.32 mean ratio. It is not proof of it.

The 27.1% column mostly costs searches rather than distance: dropping a target
and re-acquiring *the same* site wastes compute, not movement. Only the 3.5%
moves the unit somewhere new.

**So: the destination is measurably unstable, the mechanism is identified and
is a commitment failure rather than a judgement, and it is a plausible but
unproven cause of the detour.** What would settle it is recording the aim at
spawn and the founding site per journey and correlating change-count against
that journey's own detour ratio — a per-journey join the census does not do yet.

### ⚠ An instrument bug this caught, worth recording

The first version of this measurement removed the settler's remembered aim when
the agent dropped it. A different site acquired afterwards therefore compared
against nothing and was never counted as a change. It read **1.9% changes and
10.6% drops**; retaining the aim across a drop gives **3.5% and 27.1%**. Both
published numbers would have understated the instability by roughly half, in
the direction that made the agent look better.

## 16. ⚠ The instability was the cure, not the disease — §15's fix refuted

§15 named a candidate repair: hold the settler's site across a turn it could
not move. `advanced_settler_commit` does exactly that, bounded at three
consecutive stalled turns so it cannot re-create #492's livelock.

It works. It is also worse at everything it was meant to improve.

| | control | `settler_commit` |
|---|---|---|
| ended aimed somewhere else than the turn before | 3.5% | **2.5%** |
| ended holding no destination | 27.1% | **11.9%** |
| settler-turns spent moving | 81% | **73%** |
| settler-turns standing still | 19% | **27%** |
| turns alive per journey | 15.0 ± 0.7 | **16.5 ± 0.8** |
| tiles stepped | 12.1 ± 0.6 | **13.2 ± 0.7** |
| straight-line hexes covered | 5.2 ± 0.2 | 5.1 ± 0.2 |
| **detour ratio** | **2.32** | **2.56** |
| walking turns per seat, below target | 57.1 ± 3.1 | **74.4 ± 4.6** |

**The commitment target is hit squarely — instability more than halves — and
every transit measure moves the wrong way.** Settlers live longer, stand still
half again as often, walk further, and end up no closer to home.

### What that means, and what it retracts

`if !moved { self.settler_targets.remove(&uid); }` **is not a bug.** It is what
lets a blocked settler go somewhere it can actually reach. Holding the site
makes the unit *wait* for a path instead of taking an available one, which is
why standing still rises from 19% to 27%.

So §15's framing was wrong in its causal direction, and this is the third time
this document has made that error: **the destination instability is the
adaptive response to being blocked, not the cause of the detour.** Measuring it
as a defect and repairing it directly produced a stubborner, slower settler.

This also survives the obvious objection about tuning. The stall limit is 3;
any *larger* limit waits longer and any *smaller* one converges on the shipped
behaviour, so the direction is not a bad constant — a settler that waits is
worse than a settler that re-routes, and that is the whole result.

**Not taken to an `ai_eval`**, for the third time in this document, and for the
same reason: a treatment that is worse on its own target metric over 900+
journeys does not need forty minutes of paired evaluation to establish that it
is also worse on wins. The entrant ships off by default with the result
recorded.

### Where the movement thread actually stands

What remains unexplained is the thing underneath both results: in the *control*,
a settler stands still on 19% of its turns and averages 0.81 tiles per turn
against a shipped `moves` of 2. Neither commitment nor re-targeting explains
that — a blocked settler is blocked by something, and this document has not
identified what. Congestion against the empire's own units, zones of control,
and terrain cost are the candidates, and separating them is a movement-layer
question rather than an opening one.

## Closing summary: ten levers, and what the opening actually is

| lever | verdict |
|---|---|
| opening book, swept | −0.0019 ± 0.0148 |
| opening book, deleted entirely | −0.003 |
| technology order, randomised | below 0.09 resolution |
| civic order, randomised | below 0.09 resolution |
| civilization-aware decisions, deleted | −0.8%, p = 0.83 |
| being a particular civilization | η² = 0.064 on score, nothing on wins |
| one-settler-at-a-time clause, lifted | inert |
| capital food appetite, raised | wrong direction, monotone in the dose |
| settler destination commitment | wrong direction on every transit measure |
| **what does bind** | **settler transit: 44% of all time below the city target** |

The opening in this engine is not a build order. Every build-order and
per-civilization lever is bounded at or near zero, and the two economy levers
with real headroom both reversed when pushed. What sets the tempo is how long a
settler takes to turn into a city — fifteen turns to cover five hexes — and
that is a movement problem this document has localised but not solved.

## 17. Why the settler stands still: it has nowhere to go

§16 left one unexplained fact — in the control a settler stands still on
roughly a fifth of its turns, and neither commitment nor re-targeting accounts
for it. Classifying every such turn, 60 maps, 4 players, 32×22:

| why a settler stood still | turns | share |
|---|---|---|
| **held no destination at all** | 2,889 | **70%** |
| every neighbouring tile was occupied | 927 | 22% |
| unexplained | 311 | 8% |
| had spent its movement | 19 | **0%** |

**It is not congestion and it is not terrain.** Movement exhaustion accounts
for 19 turns out of 4,146. The settler is standing there because the site
search returned nothing it could use.

### The asymmetry that would produce exactly this

The gate that *authorises* a settler and the search that *places* one are not
the same search:

| | call | reachability |
|---|---|---|
| production gate (`production_value`, settler arm) | `best_settle_site(g, pid, city.pos, 11)` | **not checked** |
| settler placement (`advanced_settler_step`) | `best_reachable_settle_site(g, pid, uid, …)` | `first_reachable_settle_site` **filters on it** |

`best_settle_site` returns the highest-valued candidate from `settle_sites`,
which screens tiles on terrain, ownership and the four-hex spacing rule but
never asks whether the unit could get there. So a city can be told "there is a
site worth building a settler for" and the settler it produces can then find
nothing it is able to reach — and stand still holding no destination, which is
exactly the 70% row.

### ⚠ Stated as a mechanism, not a proven cause — deliberately

This document has now attributed a measured effect to an adjacent mechanism
three times and been wrong three times: the settler-serialization clause (§7),
capital growth (§12), and destination commitment (§16). Each was a real
correlation with a plausible mechanism, and each reversed under a treatment.

So: the 70% is measured, the asymmetry is real and is in the code, and the
second would produce the first. **That is not the same as the second causing
the first**, and the population of "no destination" turns certainly also
contains settlers whose sites were legitimately consumed — a rival founding
inside the four-hex spacing rule invalidates a target through no fault of
either search.

**The test is cheap and specific:** make the production gate ask the reachable
question, and see whether the 70% row falls. If it does not, the sites are
being invalidated rather than never reachable, and the fix is elsewhere. That
is a fires-check on a number this document already has a control value for,
which is the cheapest kind of experiment available here — and on this
document's record, it is more likely to refute the story than confirm it.

### The fires-check refutes it — for the fourth time

The gate cannot literally be made to test reachability: at production time
there is no settler yet to test it with. So the decisive observable is whether
the gate keeps *authorising* settlers while one already has nowhere to go.

> Of the 2,889 no-destination turns, **0 (0%)** happened while the empire was
> paying for another settler.

**Zero.** The gate never over-authorises, and the reason is the clause §7
measured as inert: `counts.settlers == 0` permits one settler in existence
empire-wide, so an empire cannot start a second while the first is stranded.
The asymmetry between the two searches is real, and it **cannot compound**,
because the one-settler rule stands between them.

So §17's mechanism is withdrawn. That is the fourth attribution in this
document refuted by its own check, and the prediction that it would be was
written down before the number came back.

**What the 70% actually is, then:** a settler that genuinely has nowhere to go
— every candidate either unreachable, below the value-12 threshold, or
invalidated by the four-hex spacing rule after it set out. Not a gate error.
That is site *scarcity* on a settled map, which is a mapgen and settle-value
question, not a bug in the expansion pipeline.

### Closing the transit thread

Settler transit is 44% of all time below the city target, and it decomposes as:
a settler that moves 77–81% of its turns at 0.81 tiles a turn against terrain,
and stands still the rest of the time because there is nowhere left worth
going. Neither half is a defect with a repair attached. **The expansion
pipeline is working as designed; the design's ceiling is the map.**

## 18. 2026-07-29 preregistration: give the last city time to exist

§17's closing claim does not transfer to the larger map. #559 repeated the
settler-conjunct census at 6p/74x46 and found the result invert: a missing site
blocked **0.0%** of 993 short city-turns, while the expansion window blocked
31.2% and was the sole failed conjunct 310 times. On that geometry the map is
not the ceiling. This section fixes the next hypothesis before changing play.

The old valuation and production-preemption routes are not reopened. A settler
worth 100 times normal was inert because it rarely reached an empty-queue
decision, and preemption changed queues without buying one additional city.
Food-first also moved founding cadence in the wrong direction. The remaining
single intervention supported by the new census is the window itself.

### The timing mismatch

Adaptive expansion currently uses:

```text
deadline = standard_duration(300)
             .min(max_turns - standard_duration(50))
```

On the production Online/250-turn profile, those two deadlines are turn 150
and turn 225, so the first one closes expansion at turn 150. But the planned
city target rises on a `standard_duration(90)` cadence: on Online it asks for
the fourth city at turn 45, fifth at 90, and sixth at **135**. The agent gives
itself only 15 turns to produce and place the last city it just decided it
wants. Settler transit alone was measured at roughly 15 turns; unlike costs and
strategic durations, map movement does not become twice as fast on Online.

The treatment removes only the absolute turn-300 cap:

```text
deadline = max_turns - standard_duration(50)
```

Thus the focal Online window is `[150, 225)`, 75 newly eligible turns, while
the final 25-turn reserve remains closed. The six-city target, one-settler
serialization, population cost and floor, site requirement, production value,
queue behavior, route choice, and assigned-victory-lane cutoff all remain
unchanged. The default remains bit-identical behind an evaluator-only
`advanced_late_expansion` switch.

### Hypothesis and fires-check

> The absolute cap expires before the sixth-city plan can execute on Online.
> Removing only that cap will produce and found late settlers often enough to
> change development, without granting a unit or relaxing any affordability
> rule.

Implementation starts only after the active owner of the AI/evaluator paths
clears. Unit fixtures must first establish that the control closes at turn 150,
the treatment remains open through turn 224 and closes at 225, and assigned
victory targets retain their separate cutoff.

Before any win evaluation, a fixed matched mechanism census uses eight fresh
maps at seed 9991500 on the exact 8p/84x54-requested (105x44 realized)
Continents/Planet, 12-city-state, Online/250, randomized-civilization,
Science/Culture/Domination profile. It runs one all-control and one
all-treatment game per map and samples all 64 major seat-maps. The treatment
earns an A/B only if:

- at least eight seat-maps actually start a Settler during the newly opened
  turns 150–224; and
- treatment games finish with at least four more founded cities in aggregate
  than their matched controls.

The first term proves reach; the second proves the extra permission becomes a
city rather than a doomed late queue. Failure stops the line without reading a
win seed.

### Fixed direct-policy screen

Passing the fires-check earns exactly this untouched 120-map screen:

```text
ai_eval advanced_late_expansion advanced --players 8 --width 84 --height 54 \
  --city-states 12 --pairs 120 --turns 250 --speed online \
  --map continents --shape planet --poles poles --randomize-civs \
  --victories science,culture,domination --seed 9992000 --jobs 6
```

The evaluator must report the share of treatment seat-games that actually
start a Settler in the new window. The screen advances only if coverage is at
least 10%, paired win score is at least 52%, favorable map directions outnumber
adverse directions, paired terminal-score share is at least 50%, and the
unchanged promotion gate does not retain `advanced`. This is a development
screen and cannot promote a default.

Passing every term earns one disjoint 240-map confirmation at seed 9993000 on
the identical profile. That confirmation must pass the repository's unchanged
win promotion gate; terminal score and mechanism coverage remain diagnostics.
There is no seed retry, threshold change, or pooled rescue. Even a confirmed
Advanced result remains evaluator-only until a separately preregistered test
shows that the policy transfers through the strongest `strategic_deep`
controller; controller interactions have reversed too many prior policies to
assume that transfer.

## 19. 2026-07-29 preregistration: let adaptive Expansion reach Advanced production

This follow-up is fixed before either §18's seed 9991500 or the adaptive-plan
transition census reports a result. It does not reinterpret a failed fires-check.
It separates two mechanisms that the code currently confounds.

`AdvancedAi::assess` can ask an adaptive empire to reach four, five, and then
six cities. After the opening book, however, an untargeted adaptive agent calls
`advanced_production` only for `Recovery`; every other adaptive plan ends by
calling `BasicAi::cities`. That Basic governor has its own four-city target and
turn-150 deadline. The six-city target, the fixed/payback expansion window, and
§18's late-window switch all live in Advanced production. Thus changing an
Advanced deadline and changing whether adaptive Expansion reaches it are
distinct interventions.

### Frozen dispatcher treatment

Add a default-off evaluator switch named `advanced_expansion_dispatch`. After
the four-build opening, it changes only the existing Advanced-production
dispatcher condition:

```text
Recovery OR explicit victory target
```

becomes:

```text
Recovery OR explicit victory target
         OR (advanced_expansion_dispatch AND adaptive Expansion)
```

The existing support-production and `BasicAi::cities` passes still run after
that call and fill only queues left idle. The treatment does not change
`production_value`, desired-city cadence, the one-settler limit, site search,
population gates, production costs, deadline, assigned-target behavior, or
movement. It does not modify Basic weights. The production call may choose any
item its already-shipped scoring prefers; that breadth is part of the policy
being tested and must not be narrowed after results are seen.

`advanced_expansion_complete` composes exactly that dispatcher switch with
§18's already-frozen `advanced_late_expansion` switch. The four factorial arms
are therefore:

| arm | dispatcher | late window |
|---|---|---|
| stock `advanced` | off | off |
| `advanced_late_expansion` | off | on |
| `advanced_expansion_dispatch` | on | off |
| `advanced_expansion_complete` | on | on |

All switches remain evaluator-only and default off. Instrument cumulative
successful `Action::Produce` calls made by the newly exposed adaptive-Expansion
invocation, including the subset that starts Settlers and the turn of each such
start. Also retain §18's arm-independent count of every successful Advanced
late-Settler start, so late-only Recovery behavior remains observable. An
attempted value, an open predicate, or a pre-action queue snapshot is not a
fire. Unit fixtures must show that stock adaptive Expansion skips Advanced
production, dispatcher-only enters it while retaining the turn-150 Settler
cutoff, combined enters it with Settlers eligible through turn 224 and closed
at turn 225, and assigned victory targets retain their separate cutoff.

### Fixed 2×2 mechanism census

Before reading a win seed, run all four arms on each of eight fresh maps at
seed 9994000. Every game uses the exact 8p/84x54-requested (105x44 realized),
12-city-state, Online/250, Continents/Planet/Poles,
randomized-civilization, Science/Culture/Domination profile. All eight major
seats use the same arm, yielding 64 major seat-maps per arm. Report actual new
dispatcher calls, successful production actions, Settlers started before turn
150, Settlers started during turns 150--224, and final founded cities excluding
captures, by map and in aggregate.

The combined policy earns an outcome screen only if every prospective term
passes:

1. dispatcher-only makes at least one successful production action through the
   new adaptive-Expansion call on at least 16/64 seat-maps and starts a Settler
   through that call on at least 8/64;
2. dispatcher-only finishes with at least four more founded cities in aggregate
   than stock;
3. combined starts a Settler through that call during turns 150--224 on at
   least 8/64 seat-maps, and records any Advanced late-Settler start on more
   seat-maps than late-only; and
4. combined finishes with at least four more founded cities in aggregate than
   both dispatcher-only and late-only.

These are interaction gates: (1--2) establish that routing reaches and executes
the six-city policy; (3--4) establish that the late permission adds execution
and cities after that route exists. A failure stops the line. No arm, threshold,
map count, or seed is substituted, and seeds 9994500 onward remain unread.

### Advanced outcome screen and confirmation

Passing all four mechanism terms earns exactly one 120-map comparison:

```text
ai_eval advanced_expansion_complete advanced --players 8 --width 84 \
  --height 54 --city-states 12 --pairs 120 --turns 250 --speed online \
  --map continents --shape planet --poles poles --randomize-civs \
  --victories science,culture,domination --seed 9994500 --jobs 6
```

At least 10% of treatment seat-games must record a successful late Settler
start through the exposed call. The screen advances only if paired win score is
at least 52%, favorable map directions outnumber adverse directions, paired
terminal-score share is at least 50%, and the repository's unchanged promotion
gate does not retain `advanced`. Failure stops without a retry or a component
arm outcome search.

A complete pass earns one disjoint 240-map confirmation at seed 9995000 on the
identical profile. The same coverage floor and unchanged win promotion gate
must pass; terminal score remains diagnostic. This confirms the composed
Advanced policy but still cannot change a deployed default.

### Strongest-controller transfer

Only that Advanced confirmation earns a new default-off entrant that applies
the same two switches to every Advanced parent/candidate used by
`strategic_deep`; no rollout horizon, candidate set, value function, or routing
rule changes. Compare it with stock `strategic_deep` for 120 maps at seed
9996000 on the same deployment profile. It advances only with at least 10%
late-start coverage, paired win score at least 52%, favorable directions greater
than adverse, terminal-score share at least 50%, and a non-retention verdict
from the unchanged promotion gate.

One disjoint 240-map confirmation at seed 9997000 is then allowed. Only a pass
of the same coverage and win gates may enable the composed policy in the
deployed strongest controller. Advanced confirmation cannot rescue a failed
transfer, pooled seeds cannot rescue either confirmation, and no result here
authorizes changing the standalone Advanced default.
